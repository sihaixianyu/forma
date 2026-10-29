use super::*;

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
    let mut job =
        egui::text::LayoutJob::simple(other.into(), FontId::monospace(13.0), Color32::WHITE, width);
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
