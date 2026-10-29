use super::*;

pub(super) struct EditorViewport<'a> {
    pub(super) salt: &'static str,
    pub(super) wrap: bool,
    pub(super) height: f32,
    pub(super) offset: Option<f32>,
    pub(super) palette: Palette,
    pub(super) left: bool,
    pub(super) first_line: usize,
    pub(super) base: usize,
    pub(super) alignment: Option<&'a json_lens::alignment::Alignment>,
    pub(super) syntax_error: Option<&'a crate::syntax::SyntaxError>,
}

pub(super) fn editor_scroll(
    ui: &mut egui::Ui,
    view: EditorViewport<'_>,
    contents: impl FnOnce(&mut egui::Ui) -> egui::text_edit::TextEditOutput,
) -> egui::scroll_area::ScrollAreaOutput<egui::text_edit::TextEditOutput> {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let gutter = ui
            .allocate_exact_size(Vec2::new(64.0, view.height), egui::Sense::hover())
            .0;
        let mut area = (if view.wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        })
        .id_salt(view.salt)
        .auto_shrink([false, false])
        .max_height(view.height);
        if let Some(offset) = view.offset {
            area = area
                .vertical_scroll_offset(offset)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
        }
        let scroll = area.show(ui, contents);
        let output = &scroll.inner;
        let gutter = egui::Rect::from_min_max(
            egui::pos2(gutter.left(), scroll.inner_rect.top()),
            egui::pos2(gutter.right(), scroll.inner_rect.bottom()),
        );
        let painter = ui.painter().with_clip_rect(gutter);
        let p = view.palette;
        painter.rect_filled(gutter, 8, p.subtle);
        let side = view
            .alignment
            .map(|alignment| {
                if view.left {
                    &alignment.left
                } else {
                    &alignment.right
                }
            })
            .filter(|side| side.source == output.galley.job.text);
        let color = if view.left { p.red } else { p.green };
        let text_painter = ui.painter().with_clip_rect(scroll.inner_rect);
        let mut line = 0;
        let mut first_visual_row = true;
        for row in &output.galley.rows {
            let index = line;
            let number = first_visual_row;
            first_visual_row = row.ends_with_newline;
            line += usize::from(row.ends_with_newline);
            let y = output.galley_pos.y + row.pos.y;
            if y > gutter.bottom() {
                break;
            }
            if y + row.size.y < gutter.top() {
                continue;
            }
            let invalid = view.syntax_error.filter(|error| {
                !output.response.changed()
                    && error
                        .location
                        .is_some_and(|(line, _)| line == index + view.first_line)
            });
            if let Some(error) = invalid {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(gutter.left(), y),
                    Vec2::new(gutter.width(), row.size.y),
                )
                .intersect(gutter);
                painter.rect_filled(rect, 2, p.red.gamma_multiply(0.16));
                painter.rect_filled(
                    egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                    1,
                    p.red,
                );
                let response = ui
                    .interact(
                        rect,
                        ui.id().with(("syntax_error", index)),
                        egui::Sense::click(),
                    )
                    .on_hover_text(&error.message);
                if response.clicked()
                    && let Some(offset) = error
                        .offset
                        .and_then(|offset| offset.checked_sub(view.base))
                    && let Some(before) = output.galley.job.text.get(..offset)
                {
                    let mut state = output.state.clone();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(before.chars().count()),
                        )));
                    state.store(ui.ctx(), output.response.id);
                    ui.ctx()
                        .memory_mut(|memory| memory.request_focus(output.response.id));
                }
                if number {
                    painter.text(
                        egui::pos2(gutter.left() + 10.0, y),
                        egui::Align2::LEFT_TOP,
                        "!",
                        FontId::monospace(13.0),
                        p.red,
                    );
                }
            }
            let changed = side.is_some_and(|side| side.changed.get(index) == Some(&true));
            if changed {
                let marker = egui::Rect::from_min_size(
                    egui::pos2(gutter.left() + 4.0, y),
                    Vec2::new(3.0, row.size.y),
                )
                .intersect(gutter.shrink2(Vec2::new(0.0, 8.0)));
                if marker.is_positive() {
                    painter.rect_filled(marker, 2, color);
                }
                text_painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(scroll.inner_rect.left(), y),
                        Vec2::new(scroll.inner_rect.width(), row.size.y),
                    ),
                    0,
                    color.gamma_multiply(0.06),
                );
            }
            if number {
                painter.text(
                    egui::pos2(gutter.right() - 8.0, y),
                    egui::Align2::RIGHT_TOP,
                    (index + view.first_line).to_string(),
                    FontId::monospace(13.0),
                    if invalid.is_some() {
                        p.red
                    } else if changed {
                        color
                    } else {
                        p.muted
                    },
                );
            }
        }
        scroll
    })
    .inner
}

pub(super) fn highlight_difference(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    range: &std::ops::Range<usize>,
    base: usize,
    color: Color32,
) {
    if range.start < base || range.end > base + text.len() {
        return;
    }
    let start = text[..range.start - base].chars().count();
    let end = start + text[range.start - base..range.end - base].chars().count();
    let painter = ui.painter().with_clip_rect(output.text_clip_rect);
    let mut offset = 0;
    for row in &output.galley.rows {
        let row_end = offset + row.glyphs.len();
        if start <= row_end && end > offset {
            let a = output
                .galley
                .pos_from_cursor(egui::text::CCursor::new(start.max(offset)));
            let b = output
                .galley
                .pos_from_cursor(egui::text::CCursor::new(end.min(row_end)));
            let rect = egui::Rect::from_min_max(
                egui::pos2(a.min.x, row.pos.y),
                egui::pos2(b.min.x.max(a.min.x + 3.0), row.pos.y + row.size.y),
            )
            .translate(output.galley_pos.to_vec2());
            painter.rect_filled(rect, 2, color.gamma_multiply(0.12));
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.5_f32, color),
            );
        }
        offset = row_end + usize::from(row.ends_with_newline);
    }
}

#[cfg(test)]
mod tests;
