use super::*;

pub(super) fn editor_id_key(salt: &str) -> egui::Id {
    egui::Id::new(("json_lens_editor", salt))
}

#[derive(Clone)]
struct EditSnapshot {
    text: Arc<String>,
    cursor: egui::text::CCursorRange,
}
#[derive(Default)]
struct CodeHistory {
    current: Option<EditSnapshot>,
    past: std::collections::VecDeque<EditSnapshot>,
    future: Vec<EditSnapshot>,
    past_bytes: usize,
    last_change: f64,
    boundary: bool,
}
impl CodeHistory {
    fn feed(&mut self, text: &str, cursor: egui::text::CCursorRange, now: f64, separate: bool) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.text.as_str() == text)
        {
            let current = self.current.as_mut().unwrap();
            if current.cursor != cursor {
                self.boundary = true;
                current.cursor = cursor;
            }
            return; // Moving/selecting the caret never invalidates redo.
        }
        if let Some(current) = self.current.take()
            && (self.boundary || separate || now - self.last_change > 0.6 || self.past.is_empty())
        {
            self.past_bytes += current.text.len();
            self.past.push_back(current);
        }
        self.current = Some(EditSnapshot {
            text: Arc::new(text.into()),
            cursor,
        });
        self.future.clear();
        self.last_change = now;
        self.boundary = false;
        while self.past.len() > 1 && (self.past.len() > 100 || self.past_bytes > 32 * 1024 * 1024) {
            self.past_bytes -= self.past.pop_front().unwrap().text.len();
        }
    }
    fn undo(&mut self, redo: bool) -> Option<EditSnapshot> {
        let next = if redo {
            self.future.pop()?
        } else {
            let next = self.past.pop_back()?;
            self.past_bytes -= next.text.len();
            next
        };
        if let Some(current) = self.current.replace(next.clone()) {
            if redo {
                self.past_bytes += current.text.len();
                self.past.push_back(current);
            } else {
                self.future.push(current);
            }
        }
        self.boundary = true;
        Some(next)
    }
}
fn code_history(ctx: &egui::Context, salt: &str) -> Arc<std::sync::Mutex<CodeHistory>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<std::sync::Mutex<CodeHistory>>>(
            editor_id_key(salt).with("history"),
        )
        .clone()
    })
}

pub(super) fn preserve_editor_undo(ctx: &egui::Context, salt: &str, text: &str) {
    let Some(id) = ctx.data(|data| data.get_temp::<egui::Id>(editor_id_key(salt))) else {
        return;
    };
    let Some(state) = egui::text_edit::TextEditState::load(ctx, id) else {
        return;
    };
    if let Some(range) = state.cursor.char_range() {
        let history = code_history(ctx, salt);
        let mut history = history.lock().unwrap();
        history.feed(text, range, ctx.input(|input| input.time), true);
        history.boundary = true;
        ctx.data_mut(|data| data.insert_temp(editor_id_key(salt).with("restore_focus"), true));
    }
}

/// Insert two literal spaces; block indentation and outdent keep selection/undo state.
pub(super) fn prepare_code_input(ui: &egui::Ui, salt: &str, text: &mut String) -> bool {
    let Some(id) = ui
        .ctx()
        .data(|data| data.get_temp::<egui::Id>(editor_id_key(salt)))
    else {
        return false;
    };
    let restore = editor_id_key(salt).with("restore_focus");
    if ui
        .ctx()
        .data(|data| data.get_temp::<bool>(restore).unwrap_or(false))
    {
        ui.ctx().memory_mut(|memory| memory.request_focus(id));
        ui.ctx().data_mut(|data| data.remove::<bool>(restore));
    }
    if !ui.ctx().memory(|memory| memory.has_focus(id)) {
        return false;
    }
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let mut changed = false;
    let history = code_history(ui.ctx(), salt);
    ui.ctx().input_mut(|input| {
        input.events.retain_mut(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            if (modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
                && !modifiers.alt
                && matches!(key, egui::Key::Z | egui::Key::Y)
            {
                if let Some(snapshot) = history
                    .lock()
                    .unwrap()
                    .undo(*key == egui::Key::Y || modifiers.shift)
                {
                    *text = (*snapshot.text).clone();
                    state.cursor.set_char_range(Some(snapshot.cursor));
                    changed = true;
                }
                return false;
            }
            if *key != egui::Key::Tab || modifiers.ctrl || modifiers.command || modifiers.alt {
                return true;
            }
            let Some(mut range) = state.cursor.char_range() else {
                return true;
            };
            if !modifiers.shift && range.is_empty() {
                history.lock().unwrap().boundary = true;
                *event = egui::Event::Text("  ".into());
                return true;
            }
            let chars = range.as_sorted_char_range();
            let byte_at = |index| {
                text.char_indices()
                    .nth(index)
                    .map_or(text.len(), |(byte, _)| byte)
            };
            let start = byte_at(chars.start);
            let end = byte_at(chars.end);
            let first = text[..start].rfind('\n').map_or(0, |byte| byte + 1);
            let last = if end > start && text[..end].ends_with('\n') {
                end - 1
            } else {
                end
            };
            let mut lines = vec![first];
            lines.extend(
                text[first..last]
                    .match_indices('\n')
                    .map(|(offset, _)| first + offset + 1),
            );
            for byte in lines.into_iter().rev() {
                let position = text[..byte].chars().count();
                let remove = if modifiers.shift {
                    if text[byte..].starts_with('\t') {
                        1
                    } else {
                        text[byte..]
                            .bytes()
                            .take(2)
                            .take_while(|byte| *byte == b' ')
                            .count()
                    }
                } else {
                    0
                };
                let insert = if modifiers.shift { "" } else { "  " };
                if remove == 0 && insert.is_empty() {
                    continue;
                }
                text.replace_range(byte..byte + remove, insert);
                for cursor in [&mut range.primary, &mut range.secondary] {
                    if cursor.index >= position {
                        cursor.index = position
                            + insert.len()
                            + cursor.index.saturating_sub(position + remove);
                    }
                }
                changed = true;
            }
            if changed {
                state.cursor.set_char_range(Some(range));
            }
            false
        })
    });
    if changed {
        state.store(ui.ctx(), id);
    }
    changed
}

pub(super) fn show_code_editor(
    ui: &mut egui::Ui,
    salt: &str,
    changed: bool,
    editor: egui::TextEdit<'_>,
) -> egui::text_edit::TextEditOutput {
    let stroke = ui.visuals().text_cursor.stroke;
    ui.visuals_mut().text_cursor.stroke.color = Color32::TRANSPARENT;
    let mut output = editor.show(ui);
    ui.visuals_mut().text_cursor.stroke = stroke;
    if changed {
        output.response.mark_changed();
    }
    let separate = changed
        || !output.response.changed()
        || ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Paste(_)))
        });
    let range = output
        .state
        .cursor
        .char_range()
        .unwrap_or_else(|| egui::text::CCursorRange::one(egui::text::CCursor::new(0)));
    code_history(ui.ctx(), salt).lock().unwrap().feed(
        &output.galley.job.text,
        range,
        ui.input(|input| input.time),
        separate,
    );
    // CodeHistory owns replay; release egui's duplicate snapshots.
    output.state.clear_undoer();
    let restore = editor_id_key(salt).with("restore_focus");
    if output.response.sense.interactive()
        && ui
            .ctx()
            .data(|data| data.get_temp::<bool>(restore).unwrap_or(false))
    {
        output.response.request_focus();
        ui.ctx().data_mut(|data| data.remove::<bool>(restore));
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(editor_id_key(salt), output.response.id));
    if output.response.has_focus()
        && output.response.sense.interactive()
        && let Some(range) = output.cursor_range
    {
        let now = ui.input(|input| input.time);
        let key = output.response.id.with("caret_blink");
        let previous = ui
            .ctx()
            .data(|data| data.get_temp::<(egui::text::CCursorRange, f64)>(key));
        let at = previous
            .filter(|(old, _)| {
                *old == range && !output.response.changed() && !output.response.gained_focus()
            })
            .map_or(now, |(_, at)| at);
        ui.ctx().data_mut(|data| data.insert_temp(key, (range, at)));
        let mut rect = output.galley.pos_from_cursor(range.primary);
        let font = ui.style().text_styles[&egui::TextStyle::Monospace].clone();
        if rect.height() == 0.0 {
            rect.max.y = rect.min.y + ui.fonts_mut(|fonts| fonts.row_height(&font));
        }
        let rect = egui::Rect::from_center_size(rect.center(), Vec2::new(0.0, font.size + 2.0))
            .translate(output.galley_pos.to_vec2());
        egui::text_selection::visuals::paint_text_cursor(
            ui,
            &ui.painter().with_clip_rect(output.text_clip_rect),
            rect,
            now - at,
        );
    }
    output
}

pub(super) fn decoded_hover(ui: &egui::Ui, output: &egui::text_edit::TextEditOutput, text: &str) {
    let value = if output.response.hovered() {
        ui.input(|input| input.pointer.hover_pos())
            .and_then(|mouse| {
                let cursor = output.galley.cursor_from_pos(mouse - output.galley_pos);
                let rect = output
                    .galley
                    .pos_from_cursor(cursor)
                    .translate(output.galley_pos.to_vec2());
                if (rect.min.x - mouse.x).abs() > 12.0 || (rect.center().y - mouse.y).abs() > 15.0 {
                    return None;
                }
                let offset = text
                    .char_indices()
                    .nth(cursor.index)
                    .map_or(text.len(), |(offset, _)| offset);
                json_lens::decode::string_at(text, offset)
            })
    } else {
        None
    };
    crate::decode_ui::show(&output.response, value.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_byte_budget_remains_correct_after_trimming_undo_and_redo() {
        let mut history = CodeHistory::default();
        let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
        for i in 0..120 {
            history.feed(
                &format!("{i}:{}", "a".repeat(400_000)),
                cursor,
                i as f64,
                true,
            );
        }
        assert!(history.past_bytes <= 32 * 1024 * 1024);
        assert_eq!(
            history.past_bytes,
            history.past.iter().map(|s| s.text.len()).sum::<usize>()
        );
        for redo in [false, false, true, true] {
            history.undo(redo).unwrap();
            assert_eq!(
                history.past_bytes,
                history.past.iter().map(|s| s.text.len()).sum::<usize>()
            );
        }
    }
}
