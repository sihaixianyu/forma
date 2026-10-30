use eframe::egui::{self, Color32, RichText, Vec2};

/// Cache the hovered value so moving into the interactive tooltip keeps it open.
pub fn show(response: &egui::Response, value: Option<&str>) {
    let id = response.id.with("decoded_preview");
    let rect_id = id.with("popup_rect");
    let anchor_id = id.with("popup_anchor");
    let pointer = response.ctx.input(|input| input.pointer.hover_pos());
    // Include the small gap between the source text and popup so the pointer
    // can reach its buttons without retargeting the text underneath.
    let inside_popup = response.ctx.data(|data| {
        data.get_temp::<egui::Rect>(rect_id)
            .is_some_and(|rect| pointer.is_some_and(|pos| rect.expand(12.0).contains(pos)))
    });
    if response.hovered() && !inside_popup {
        response.ctx.data_mut(|data| {
            if let Some(value) = value {
                if data.get_temp::<String>(id).as_deref() != Some(value) {
                    data.remove::<egui::Rect>(rect_id);
                    data.remove::<egui::Pos2>(anchor_id);
                }
                data.insert_temp(id, value.to_owned());
            } else {
                data.remove::<String>(id);
                data.remove::<egui::Rect>(rect_id);
                data.remove::<egui::Pos2>(anchor_id);
            }
        });
    }
    let value = response.ctx.data(|data| data.get_temp::<String>(id));
    if let Some(value) = value
        && !json_lens::decode::previews(&value).is_empty()
    {
        let mut tooltip = if inside_popup {
            egui::Tooltip::for_widget(response)
        } else {
            egui::Tooltip::for_enabled(response)
        };
        // Anchor once when opened, rather than tracking every pointer move.
        let anchor = response
            .ctx
            .data(|data| data.get_temp::<egui::Pos2>(anchor_id))
            .or(pointer);
        if let Some(anchor) = anchor {
            tooltip.popup = tooltip.popup.anchor(anchor);
        }
        let popup = tooltip.show(|ui| card(ui, &value));
        response.ctx.data_mut(|data| {
            if let Some(popup) = popup {
                data.insert_temp(rect_id, popup.response.rect);
                if let Some(anchor) = anchor {
                    data.insert_temp(anchor_id, anchor);
                }
            } else {
                data.remove::<egui::Rect>(rect_id);
                data.remove::<egui::Pos2>(anchor_id);
            }
        });
    }
}

fn card(ui: &mut egui::Ui, value: &str) {
    let dark = ui.visuals().dark_mode;
    let accent = if dark {
        Color32::from_rgb(167, 151, 255)
    } else {
        Color32::from_rgb(110, 83, 218)
    };
    let header = if dark {
        Color32::from_rgb(48, 40, 77)
    } else {
        Color32::from_rgb(239, 235, 253)
    };
    let content = ui.visuals().extreme_bg_color;
    ui.set_width(340.0);
    for (encoding, text) in json_lens::decode::previews(value) {
        ui.push_id(encoding, |ui| {
            egui::Frame::new()
                .fill(header)
                .corner_radius(egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(316.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(encoding).strong().color(accent));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let copied = ui.id().with("copied");
                            let now = ui.input(|input| input.time);
                            let recent = ui
                                .ctx()
                                .data(|data| data.get_temp::<f64>(copied))
                                .is_some_and(|at| now - at < 1.5);
                            if ui
                                .add(
                                    egui::Button::new(if recent { "Copied" } else { "Copy" })
                                        .min_size(Vec2::new(58.0, 26.0))
                                        .corner_radius(5),
                                )
                                .clicked()
                            {
                                ui.ctx().copy_text(text.clone());
                                ui.ctx().data_mut(|data| data.insert_temp(copied, now));
                                ui.ctx()
                                    .request_repaint_after(std::time::Duration::from_millis(1500));
                            }
                        });
                    });
                });
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::Frame::new()
                .fill(content)
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 8,
                    se: 8,
                })
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_width(316.0);
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(json_lens::preview(&text, 8192)).monospace(),
                                )
                                .wrap()
                                .selectable(true),
                            );
                        });
                });
            ui.add_space(6.0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_into_popup_retains_decoded_value() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        let mut pointer = egui::pos2(30.0, 30.0);
        for frame in 0..5 {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(frame as f64),
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response =
                            ui.allocate_response(egui::vec2(600.0, 400.0), egui::Sense::hover());
                        show(&response, (frame < 3).then_some("5Lit5paH"));
                        let id = response.id.with("decoded_preview");
                        if frame >= 2 {
                            let rect = ctx.data(|data| {
                                assert_eq!(
                                    data.get_temp::<String>(id).as_deref(),
                                    Some("5Lit5paH")
                                );
                                data.get_temp::<egui::Rect>(id.with("popup_rect")).unwrap()
                            });
                            pointer = rect.center();
                        }
                    });
                },
            );
        }
    }

    #[test]
    fn copy_button_copies_the_full_decoded_value() {
        let ctx = egui::Context::default();
        let mut button = egui::Pos2::ZERO;
        for frame in 0..3 {
            let events = if frame == 0 {
                Vec::new()
            } else {
                vec![
                    egui::Event::PointerMoved(button),
                    egui::Event::PointerButton {
                        pos: button,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            let output = ctx.run(
                egui::RawInput {
                    time: Some(frame as f64 * 0.1),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| card(ui, "5Lit5paH"));
                },
            );
            if frame == 0 {
                button = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape
                            && text.galley.job.text == "Copy"
                        {
                            Some(text.pos + text.galley.rect.center().to_vec2())
                        } else {
                            None
                        }
                    })
                    .unwrap();
            }
            if frame == 2 {
                assert!(output.platform_output.commands.iter().any(|command| {
                    matches!(command, egui::OutputCommand::CopyText(text) if text == "中文")
                }));
            }
        }
    }
}
