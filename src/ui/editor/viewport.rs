use super::*;

/// Prefix offsets for visual rows. Selection changes mesh colors, but not these offsets.
pub(super) struct CodeRows {
    job: Arc<egui::text::LayoutJob>,
    pub(super) bytes: Vec<usize>,
    pub(super) chars: Vec<usize>,
    pub(super) lines: Vec<usize>,
}

impl CodeRows {
    pub(super) fn row_for_cursor(
        &self,
        galley: &egui::Galley,
        cursor: egui::text::CCursor,
    ) -> usize {
        let mut row = self
            .chars
            .partition_point(|start| *start <= cursor.index)
            .saturating_sub(1)
            .min(galley.rows.len().saturating_sub(1));
        if row > 0
            && !cursor.prefer_next_row
            && !galley.rows[row - 1].ends_with_newline
            && self.chars[row] == cursor.index
        {
            row -= 1;
        }
        row
    }
}

pub(super) fn code_rows(ctx: &egui::Context, id: egui::Id, galley: &egui::Galley) -> Arc<CodeRows> {
    let key = id.with("code_rows");
    if let Some(rows) = ctx.data(|data| data.get_temp::<Arc<CodeRows>>(key))
        && Arc::ptr_eq(&rows.job, &galley.job)
        && rows.lines.len() == galley.rows.len()
    {
        return rows;
    }
    let mut rows = CodeRows {
        job: galley.job.clone(),
        bytes: Vec::with_capacity(galley.rows.len() + 1),
        chars: Vec::with_capacity(galley.rows.len() + 1),
        lines: Vec::with_capacity(galley.rows.len()),
    };
    let (mut byte, mut character, mut line) = (0, 0, 0);
    for row in &galley.rows {
        rows.bytes.push(byte);
        rows.chars.push(character);
        rows.lines.push(line);
        let newline = usize::from(row.ends_with_newline);
        byte += row
            .glyphs
            .iter()
            .map(|glyph| glyph.chr.len_utf8())
            .sum::<usize>()
            + newline;
        character += row.glyphs.len() + newline;
        line += newline;
    }
    rows.bytes.push(byte);
    rows.chars.push(character);
    let rows = Arc::new(rows);
    ctx.data_mut(|data| data.insert_temp(key, rows.clone()));
    rows
}

/// Keep the complete galley for editing, cursor positions and scrollbar extent.
/// Only the paint shape is sliced: epaint otherwise reserves a mesh for the entire
/// document and walks every row before culling, on every frame.
pub(super) fn paint_visible_rows(
    ui: &egui::Ui,
    start: egui::layers::ShapeIdx,
    galley: &Arc<egui::Galley>,
) {
    if galley.rows.len() < 256 {
        return;
    }
    // Clone a tiny galley as a template rather than cloning the full row vector.
    let template = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            String::new(),
            FontId::monospace(EDITOR_FONT_SIZE),
            Color32::TRANSPARENT,
        )
    });
    ui.ctx().graphics_mut(|graphics| {
        let list = graphics.entry(ui.layer_id());
        let end = list.next_idx();
        for index in start.0..end.0 {
            list.mutate_shape(egui::layers::ShapeIdx(index), |clipped| {
                let egui::Shape::Text(shape) = &mut clipped.shape else {
                    return;
                };
                if !Arc::ptr_eq(&shape.galley, galley) {
                    return;
                }
                let clip = clipped.clip_rect.expand(2.0);
                let top = clip.top() - shape.pos.y;
                let bottom = clip.bottom() - shape.pos.y;
                let first = galley
                    .rows
                    .partition_point(|row| row.pos.y + row.size.y < top);
                let last = galley.rows.partition_point(|row| row.pos.y <= bottom);
                let mut visible = (*template).clone();
                visible.job = galley.job.clone();
                visible.rows = galley.rows[first..last]
                    .iter()
                    .filter(|row| row.size.y > 0.0)
                    .cloned()
                    .collect();
                visible.elided = true;
                visible.pixels_per_point = galley.pixels_per_point;
                let origin = visible.rows.first().map_or(0.0, |row| row.pos.y);
                visible.num_vertices = 0;
                visible.num_indices = 0;
                visible.mesh_bounds = egui::Rect::NOTHING;
                visible.rect = egui::Rect::ZERO;
                for row in &mut visible.rows {
                    row.pos.y -= origin;
                    visible.num_vertices += row.visuals.mesh.vertices.len();
                    visible.num_indices += row.visuals.mesh.indices.len();
                    visible.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
                    visible.rect |= row.rect();
                }
                shape.pos.y += origin;
                shape.galley = Arc::new(visible);
            });
        }
    });
}
