use super::*;

pub(super) const EDITOR_FONT_SIZE: f32 = 14.0;
pub(super) const EDITOR_LINE_HEIGHT: f32 = 17.0;

pub(super) fn aligned_wrapped_galley(
    ui: &egui::Ui,
    galley: Arc<egui::Galley>,
    alignment: Option<&json_lens::alignment::Alignment>,
    left: bool,
    other: &str,
    width: f32,
) -> Arc<egui::Galley> {
    if !width.is_finite() {
        return aligned_galley(ui, galley, alignment, left);
    }
    let Some(alignment) = alignment else {
        return galley;
    };
    let (side, other_side) = if left {
        (&alignment.left, &alignment.right)
    } else {
        (&alignment.right, &alignment.left)
    };
    if side.source != galley.job.text || other_side.source != other {
        return galley;
    }
    if side.source == other_side.source {
        return galley;
    }
    let mut job = egui::text::LayoutJob::simple(
        other.into(),
        FontId::monospace(EDITOR_FONT_SIZE),
        Color32::WHITE,
        width,
    );
    if let Some(section) = job.sections.first_mut() {
        section.format.line_height = Some(EDITOR_LINE_HEIGHT);
    }
    job.wrap.break_anywhere = galley.job.wrap.break_anywhere;
    let opposite = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let mut heights = vec![0.0_f32; alignment.height];
    for (layout, side) in [(&*galley, side), (&*opposite, other_side)] {
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
    let mut tops = vec![0.0; heights.len()];
    let mut total = 0.0;
    for (top, height) in tops.iter_mut().zip(heights) {
        *top = total;
        total += height;
    }
    let mut result = (*galley).clone();
    result.mesh_bounds = egui::Rect::NOTHING;
    let mut line = 0;
    let mut within = 0.0;
    for row in &mut result.rows {
        let Some(display) = side.rows.get(line) else {
            return galley;
        };
        row.pos.y = tops[*display] + within;
        result.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
        within += row.size.y;
        if row.ends_with_newline {
            line += 1;
            within = 0.0;
        }
    }
    result.rect.max.y = total;
    Arc::new(result)
}

pub(super) fn aligned_galley(
    _ui: &egui::Ui,
    galley: Arc<egui::Galley>,
    alignment: Option<&json_lens::alignment::Alignment>,
    left: bool,
) -> Arc<egui::Galley> {
    let Some(alignment) = alignment else {
        return galley;
    };
    let side = if left {
        &alignment.left
    } else {
        &alignment.right
    };
    if side.source != galley.job.text || side.rows.len() != galley.rows.len() {
        return galley;
    }
    if alignment.height == galley.rows.len()
        && side
            .rows
            .iter()
            .enumerate()
            .all(|(index, row)| index == *row)
    {
        return galley;
    }
    // Galley rows already have pixel-rounded heights. Reusing raw font metrics here
    // changes the spacing cumulatively, especially at fractional display scales.
    let line_height = galley.rows.first().map_or(0.0, |row| row.size.y);
    let mut aligned = (*galley).clone();
    aligned.mesh_bounds = egui::Rect::NOTHING;
    for (row, display) in aligned.rows.iter_mut().zip(&side.rows) {
        row.pos.y = (*display as f32 * line_height * aligned.pixels_per_point).round()
            / aligned.pixels_per_point;
        aligned.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
    }
    aligned.rect.max.y = alignment.height as f32 * line_height;
    Arc::new(aligned)
}

#[derive(Default)]
pub struct CodeCache {
    dark: bool,
    pixels_per_point: f32,
    job: Option<egui::text::LayoutJob>,
    galley: Option<Arc<egui::Galley>>,
    font_epoch: Option<Arc<egui::Galley>>,
    aligned: Option<AlignedLayout>,
    search_rows: Option<SearchRowIndex>,
}

struct SearchRowIndex {
    galley: Arc<egui::Galley>,
    starts: Vec<usize>,
}

struct AlignedLayout {
    base: Arc<egui::Galley>,
    alignment: Arc<json_lens::alignment::Alignment>,
    result: Arc<egui::Galley>,
}

impl CodeCache {
    pub(super) fn paint_matches(
        &mut self,
        ui: &egui::Ui,
        output: &egui::text_edit::TextEditOutput,
        ranges: &[std::ops::Range<usize>],
        selected: usize,
        base: usize,
        p: Palette,
    ) {
        let _stage = json_lens::diagnostics::Stage::slow(
            "search_highlights",
            output.galley.job.text.len(),
            8.0,
        );
        if self
            .search_rows
            .as_ref()
            .is_some_and(|index| !Arc::ptr_eq(&index.galley, &output.galley))
        {
            self.search_rows = None;
        }
        if ranges.is_empty() {
            return;
        }
        let index = self.search_rows.get_or_insert_with(|| {
            let mut byte = 0;
            let mut starts = Vec::with_capacity(output.galley.rows.len() + 1);
            for row in &output.galley.rows {
                starts.push(byte);
                byte += row
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.chr.len_utf8())
                    .sum::<usize>();
                byte += usize::from(row.ends_with_newline);
            }
            starts.push(byte);
            debug_assert_eq!(byte, output.galley.job.text.len());
            SearchRowIndex {
                galley: output.galley.clone(),
                starts,
            }
        });
        let top = output.text_clip_rect.top() - output.galley_pos.y;
        let bottom = output.text_clip_rect.bottom() - output.galley_pos.y;
        let first = output
            .galley
            .rows
            .partition_point(|row| row.pos.y + row.size.y < top);
        let painter = ui.painter().with_clip_rect(output.text_clip_rect);
        for (row_index, row) in output.galley.rows.iter().enumerate().skip(first) {
            if row.pos.y > bottom {
                break;
            }
            let start = index.starts[row_index];
            let end = index.starts[row_index + 1] - usize::from(row.ends_with_newline);
            let Some(text) = output.galley.job.text.get(start..end) else {
                continue;
            };
            let first_match = ranges.partition_point(|range| range.end <= base + start);
            for (match_index, range) in ranges.iter().enumerate().skip(first_match) {
                if range.start >= base + end {
                    break;
                }
                let a = range.start.max(base + start) - base - start;
                let b = range.end.min(base + end) - base - start;
                let (Some(before), Some(found)) = (text.get(..a), text.get(a..b)) else {
                    continue;
                };
                let column = before.chars().count();
                let x = row.x_offset(column);
                let right = row.x_offset(column + found.chars().count());
                let rect = egui::Rect::from_min_size(
                    output.galley_pos + egui::vec2(row.pos.x + x, row.pos.y),
                    egui::vec2((right - x).max(3.0), row.size.y),
                );
                let active = match_index == selected;
                let color = if active { p.amber } else { p.accent };
                painter.rect_filled(
                    rect,
                    2,
                    color.gamma_multiply(if active { 0.23 } else { 0.10 }),
                );
                painter.line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(if active { 1.5_f32 } else { 1.0_f32 }, color),
                );
            }
        }
    }

    pub(super) fn align(
        &mut self,
        ui: &egui::Ui,
        galley: Arc<egui::Galley>,
        alignment: Option<&Arc<json_lens::alignment::Alignment>>,
        left: bool,
        other: &str,
        width: f32,
    ) -> Arc<egui::Galley> {
        let Some(alignment) = alignment else {
            self.aligned = None;
            return galley;
        };
        let (side, opposite) = if left {
            (&alignment.left, &alignment.right)
        } else {
            (&alignment.right, &alignment.left)
        };
        if side.source != galley.job.text || opposite.source != other {
            self.aligned = None;
            return galley;
        }
        if let Some(cached) = &self.aligned
            && Arc::ptr_eq(&cached.base, &galley)
            && Arc::ptr_eq(&cached.alignment, alignment)
        {
            return cached.result.clone();
        }
        let result =
            aligned_wrapped_galley(ui, galley.clone(), Some(alignment), left, other, width);
        self.aligned = Some(AlignedLayout {
            base: galley,
            alignment: alignment.clone(),
            result: result.clone(),
        });
        result
    }

    pub(super) fn layout_width(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        dark: bool,
        query: &str,
        width: f32,
    ) -> Arc<egui::Galley> {
        let _stage = json_lens::diagnostics::Stage::slow("editor_layout", text.len(), 8.0);
        self.layout_at(ui, text, dark, query, width)
    }
    #[cfg(test)]
    pub(super) fn layout(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        dark: bool,
        query: &str,
    ) -> Arc<egui::Galley> {
        self.layout_at(ui, text, dark, query, f32::INFINITY)
    }
    fn layout_at(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        dark: bool,
        _query: &str,
        width: f32,
    ) -> Arc<egui::Galley> {
        // egui clears the entire galley cache when fonts or their atlas are recreated.
        // Resolve a tiny job each pass to detect that epoch without cloning/hashing
        // the full document and every syntax section on each scroll or caret blink.
        let epoch = ui.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                " ".into(),
                FontId::monospace(EDITOR_FONT_SIZE),
                Color32::TRANSPARENT,
            )
        });
        if self.dark == dark
            && self.pixels_per_point == ui.ctx().pixels_per_point()
            && let Some(job) = &self.job
            && job.text == text
            && job.wrap.max_width == width
        {
            if self
                .font_epoch
                .as_ref()
                .is_some_and(|previous| Arc::ptr_eq(previous, &epoch))
                && let Some(galley) = &self.galley
            {
                return galley.clone();
            }
            let galley = ui.fonts_mut(|fonts| fonts.layout_job(job.clone()));
            self.galley = Some(galley.clone());
            self.font_epoch = Some(epoch);
            return galley;
        }
        let _stage = json_lens::diagnostics::Stage::new("syntax_layout", text.len());
        let p = palette(dark);
        let mut job = egui::text::LayoutJob {
            text: text.to_owned(),
            ..Default::default()
        };
        if text.is_empty() {
            job.append(
                "",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(EDITOR_FONT_SIZE),
                    line_height: Some(EDITOR_LINE_HEIGHT),
                    color: p.text,
                    ..Default::default()
                },
            );
        }
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let start = index;
            let color = match bytes[index] {
                b'"' => {
                    index += 1;
                    while index < bytes.len() {
                        match bytes[index] {
                            b'\\' => index = (index + 2).min(bytes.len()),
                            b'"' => {
                                index += 1;
                                break;
                            }
                            _ => index += 1,
                        }
                    }
                    let next = text[index..].trim_start().as_bytes().first().copied();
                    if next == Some(b':') {
                        p.accent
                    } else {
                        p.green
                    }
                }
                b'-' | b'0'..=b'9' => {
                    index += 1;
                    while index < bytes.len()
                        && matches!(bytes[index], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                    {
                        index += 1;
                    }
                    p.amber
                }
                b't' | b'f' | b'n' => {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                        index += 1;
                    }
                    p.accent
                }
                _ => {
                    index += text[index..].chars().next().unwrap().len_utf8();
                    p.text
                }
            };
            // Invalid Unicode escape sequences remain editable; never split a UTF-8 codepoint.
            while index < text.len() && !text.is_char_boundary(index) {
                index += 1;
            }
            if let Some(section) = job.sections.last_mut()
                && section.format.color == color
            {
                section.byte_range.end = index;
            } else {
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: start..index,
                    format: egui::TextFormat {
                        font_id: FontId::monospace(EDITOR_FONT_SIZE),
                        line_height: Some(EDITOR_LINE_HEIGHT),
                        color,
                        ..Default::default()
                    },
                });
            }
        }
        job.wrap.max_width = width;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job.clone()));
        self.dark = dark;
        self.pixels_per_point = ui.ctx().pixels_per_point();
        self.job = Some(job);
        self.galley = Some(galley.clone());
        self.font_epoch = Some(epoch);
        galley
    }
}

#[cfg(test)]
mod tests;
