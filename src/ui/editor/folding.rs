use super::*;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct FoldBlock {
    start: usize,
    end: usize,
}

/// Match delimiters without interpreting braces or escaped quotes in strings.
/// Incomplete containers remain unfolded while the user is typing.
fn blocks(text: &str) -> BTreeMap<usize, FoldBlock> {
    let mut result = BTreeMap::new();
    let mut stack = Vec::new();
    let (mut line, mut quoted, mut escaped) = (0, false, false);
    for byte in text.bytes() {
        if byte == b'\n' {
            line += 1;
        }
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => stack.push((byte, line)),
            b'}' | b']' => {
                if let Some((open, start)) = stack.pop() {
                    if (open == b'{') != (byte == b'}') {
                        stack.clear();
                        continue;
                    }
                    if line > start + 1 {
                        // For containers sharing an opening line, expose the outer one.
                        result.insert(start, FoldBlock { start, end: line });
                    }
                }
            }
            _ => {}
        }
    }
    result
}

#[derive(Clone, Default)]
struct FoldState {
    sources: Vec<Arc<String>>,
    collapsed: BTreeMap<usize, usize>,
}

#[derive(Clone)]
struct FoldLayout {
    base: Arc<egui::Galley>,
    blocks: Arc<BTreeMap<usize, FoldBlock>>,
    lines: Arc<Vec<usize>>,
    key: egui::Id,
    collapsed: BTreeMap<usize, usize>,
    result: Arc<egui::Galley>,
}

fn state_key(id: egui::Id, aligned: bool) -> egui::Id {
    if aligned {
        egui::Id::new("forma_compare_folds")
    } else {
        id.with("fold_state")
    }
}

// Include alignment gaps and the taller of the two wrapped logical lines.
// Both panes must remove exactly the same vertical interval when folded.
fn display_tops(
    ui: &egui::Ui,
    galley: &egui::Galley,
    alignment: &forma::alignment::Alignment,
    left: bool,
) -> Vec<f32> {
    let (side, other) = if left {
        (&alignment.left, &alignment.right)
    } else {
        (&alignment.right, &alignment.left)
    };
    let mut job = (*galley.job).clone();
    job.text = other.source.clone();
    job.sections.truncate(1);
    if let Some(section) = job.sections.first_mut() {
        section.byte_range = 0..job.text.len();
    }
    let opposite = crate::ui::code::layout_bounded_job(ui, job);
    let mut heights = vec![0.0_f32; alignment.height];
    for (layout, side) in [(galley, side), (opposite.as_ref(), other)] {
        let mut line = 0;
        let mut height = 0.0;
        for row in &layout.rows {
            height += row.size.y;
            if row.ends_with_newline {
                if let Some(display) = side.rows.get(line) {
                    heights[*display] = heights[*display].max(height);
                }
                line += 1;
                height = 0.0;
            }
        }
        if let Some(display) = side.rows.get(line) {
            heights[*display] = heights[*display].max(height);
        }
    }
    let mut top = 0.0;
    heights
        .into_iter()
        .map(|height| {
            let result = top;
            top += height;
            result
        })
        .collect()
}

pub(in crate::ui) fn expand_editor_folds(ctx: &egui::Context, id: egui::Id) {
    for key in [state_key(id, false), state_key(id, true)] {
        ctx.data_mut(|data| {
            if let Some(mut state) = data.get_temp::<FoldState>(key) {
                state.collapsed.clear();
                data.insert_temp(key, state);
            }
        });
    }
}

pub(in crate::ui) fn fold_galley(
    ui: &egui::Ui,
    id: egui::Id,
    galley: Arc<egui::Galley>,
    alignment: Option<&forma::alignment::Alignment>,
    left: bool,
) -> Arc<egui::Galley> {
    let alignment = alignment.filter(|alignment| {
        let side = if left {
            &alignment.left
        } else {
            &alignment.right
        };
        side.source == galley.job.text
    });
    let key = state_key(id, alignment.is_some());
    let mut state = ui
        .ctx()
        .data(|data| data.get_temp::<FoldState>(key))
        .unwrap_or_default();
    let sources: Vec<&str> = if let Some(alignment) = alignment {
        vec![&alignment.left.source, &alignment.right.source]
    } else {
        vec![&galley.job.text]
    };
    if sources.len() != state.sources.len()
        || sources
            .iter()
            .zip(&state.sources)
            .any(|(a, b)| *a != b.as_str())
    {
        state.sources = sources
            .iter()
            .map(|text| Arc::new((*text).to_owned()))
            .collect();
        state.collapsed.clear();
        ui.ctx()
            .data_mut(|data| data.insert_temp(key, state.clone()));
    }
    let cache_key = id.with("fold_layout");
    let previous = ui.ctx().data(|data| data.get_temp::<FoldLayout>(cache_key));
    let same_base = previous
        .as_ref()
        .filter(|cache| Arc::ptr_eq(&cache.base, &galley) && cache.key == key);
    if let Some(cache) = same_base
        && cache.collapsed == state.collapsed
    {
        return cache.result.clone();
    }
    let (blocks, lines) = if let Some(cache) = same_base {
        (cache.blocks.clone(), cache.lines.clone())
    } else {
        let side = alignment.map(|a| if left { &a.left } else { &a.right });
        let display_line = |line: usize| {
            side.and_then(|side| side.rows.get(line))
                .copied()
                .unwrap_or(line)
        };
        let blocks = blocks(&galley.job.text)
            .into_iter()
            .map(|(line, block)| {
                (
                    line,
                    FoldBlock {
                        start: display_line(block.start),
                        end: display_line(block.end),
                    },
                )
            })
            .collect();
        let mut line = 0;
        let lines = galley
            .rows
            .iter()
            .map(|row| {
                let display = display_line(line);
                line += usize::from(row.ends_with_newline);
                display
            })
            .collect();
        (Arc::new(blocks), Arc::new(lines))
    };
    let result = if state.collapsed.is_empty() {
        galley.clone()
    } else {
        let mut folded = (*galley).clone();
        let tops = alignment.map(|alignment| display_tops(ui, &galley, alignment, left));
        let mut removed = 0.0;
        let mut covered_until = 0;
        // Keep the opening and closing lines. Source glyphs and newlines remain
        // in zero-height rows so egui selection/copy still uses original offsets.
        for (&start, &end) in &state.collapsed {
            if start < covered_until {
                continue;
            }
            covered_until = end;
            let first = lines.partition_point(|line| *line <= start);
            let last = lines.partition_point(|line| *line < end);
            if last >= galley.rows.len() {
                continue;
            }
            let top = tops
                .as_ref()
                .and_then(|tops| tops.get(start + 1))
                .copied()
                .unwrap_or(galley.rows[first].pos.y);
            let bottom = tops
                .as_ref()
                .and_then(|tops| tops.get(end))
                .copied()
                .unwrap_or(galley.rows[last].pos.y);
            for row in &mut folded.rows[first..last] {
                row.pos.y = top;
                let row = Arc::make_mut(&mut row.row);
                row.size.y = 0.0;
                // egui recolors selected glyph vertices by index. Retain that
                // topology, but make every hidden triangle degenerate.
                for vertex in &mut row.visuals.mesh.vertices {
                    vertex.pos = egui::Pos2::ZERO;
                    vertex.color = Color32::TRANSPARENT;
                }
                row.visuals.mesh_bounds = egui::Rect::NOTHING;
            }
            for row in &mut folded.rows[last..] {
                row.pos.y -= bottom - top;
            }
            // Prior folds already shifted the suffix; hidden rows use the same shift.
            for row in &mut folded.rows[first..last] {
                row.pos.y -= removed;
            }
            removed += bottom - top;
        }
        folded.rect.max.y -= removed;
        folded.mesh_bounds = egui::Rect::NOTHING;
        folded.num_vertices = 0;
        folded.num_indices = 0;
        for row in &folded.rows {
            folded.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
            folded.num_vertices += row.visuals.mesh.vertices.len();
            folded.num_indices += row.visuals.mesh.indices.len();
        }
        Arc::new(folded)
    };
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            cache_key,
            FoldLayout {
                base: galley,
                blocks,
                lines,
                key,
                collapsed: state.collapsed,
                result: result.clone(),
            },
        )
    });
    result
}

pub(super) fn paint_fold_toggle(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    line: usize,
    rect: egui::Rect,
    p: Palette,
) {
    let Some(cache) = ui
        .ctx()
        .data(|data| data.get_temp::<FoldLayout>(output.response.id.with("fold_layout")))
    else {
        return;
    };
    let Some(block) = cache.blocks.get(&line) else {
        return;
    };
    let collapsed = cache.collapsed.contains_key(&block.start);
    if collapsed {
        let last = cache
            .lines
            .partition_point(|line| *line <= block.start)
            .saturating_sub(1);
        if let Some(row) = output.galley.rows.get(last) {
            ui.painter().with_clip_rect(output.text_clip_rect).text(
                output.galley_pos
                    + row.pos.to_vec2()
                    + egui::vec2(row.size.x + 4.0, row.size.y * 0.5),
                egui::Align2::LEFT_CENTER,
                "…",
                FontId::monospace(EDITOR_FONT_SIZE),
                p.muted,
            );
        }
    }
    let response = ui
        .interact(
            rect,
            output.response.id.with(("fold", line)),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if collapsed {
            "Unfold JSON"
        } else {
            "Fold JSON"
        });
    let center = rect.center();
    let points = if collapsed {
        vec![
            center + egui::vec2(-2.0, -4.0),
            center + egui::vec2(2.0, 0.0),
            center + egui::vec2(-2.0, 4.0),
        ]
    } else {
        vec![
            center + egui::vec2(-4.0, -2.0),
            center + egui::vec2(0.0, 2.0),
            center + egui::vec2(4.0, -2.0),
        ]
    };
    ui.painter().with_clip_rect(rect).add(egui::Shape::line(
        points,
        Stroke::new(
            1.5_f32,
            if collapsed || response.hovered() {
                p.accent
            } else {
                p.muted
            },
        ),
    ));
    if response.clicked() {
        ui.ctx().data_mut(|data| {
            let mut state = data.get_temp::<FoldState>(cache.key).unwrap_or_default();
            if collapsed {
                state.collapsed.remove(&block.start);
            } else {
                state.collapsed.insert(block.start, block.end);
            }
            data.insert_temp(cache.key, state);
        });
        // A folded selection must not leave the caret invisibly inside its body.
        let rows = code_rows(ui.ctx(), output.response.id, &output.galley);
        if let Some(index) = rows.lines.iter().position(|value| *value == line) {
            let mut state = output.state.clone();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(rows.chars[index]),
                )));
            state.store(ui.ctx(), output.response.id);
        }
        ui.ctx().request_repaint();
    }
}
