use super::*;

#[test]
fn long_json_tokens_fill_wrapped_rows_instead_of_stranding_the_opening_quote() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let text = format!("\"{}\"", "abcdef0123456789".repeat(80));
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for width in [80.0, 180.0, 320.0] {
                let galley = CodeCache::default().layout_width(ui, &text, false, "", width);
                for row in &galley.rows[..galley.rows.len() - 1] {
                    assert!(row.glyphs.len() > 1);
                    assert!(row.size.x > width * 0.8);
                    assert!(row.size.x <= width + 1.0);
                }
                let rendered: String = galley
                    .rows
                    .iter()
                    .flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr))
                    .collect();
                assert_eq!(rendered, text);
            }
        });
    });
}

#[test]
fn typography_clamps_to_supported_ranges_and_rebuilds_layouts() {
    assert_eq!(
        EditorTypography::default(),
        EditorTypography {
            font_size: 12.0,
            line_height: 18.0,
        },
        "editor defaults should use a 12px font and 18px line height"
    );

    let mut typography = EditorTypography {
        font_size: 40.0,
        line_height: 9.0,
    };
    typography.clamp();
    assert_eq!(typography.font_size, 32.0);
    assert_eq!(typography.line_height, 32.0);

    let ctx = egui::Context::default();
    configure(&ctx, false);
    let mut cache = CodeCache::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let normal = cache.layout_width(ui, "{\n  \"value\": 1\n}", false, "", 180.0);
            let custom = EditorTypography {
                font_size: 20.0,
                line_height: 28.0,
            };
            let resized = cache.layout_width_with_typography(
                ui,
                "{\n  \"value\": 1\n}",
                false,
                "",
                180.0,
                custom,
            );
            assert!(!Arc::ptr_eq(&normal, &resized));
            assert!(
                resized
                    .rows
                    .iter()
                    .all(|row| (row.size.y - 28.0).abs() < 0.1)
            );
            assert!(resized.job.sections.iter().all(|section| {
                section.format.font_id.size == 20.0 && section.format.line_height == Some(28.0)
            }));
        });
    });
}

#[test]
fn retained_layout_survives_cache_eviction_and_rebuilds_for_new_font_atlas() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let text = "{\n  \"label\": \"retained text layout\"\n}";
    let mut cache = CodeCache::default();
    let mut retained = None;
    for frame in 0..5 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let galley = cache.layout_width(ui, text, false, "", 180.0);
                if let Some(previous) = &retained {
                    assert!(Arc::ptr_eq(previous, &galley));
                } else {
                    retained = Some(galley.clone());
                }
                if frame == 4 {
                    let fresh =
                        ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
                    assert_eq!(galley.rows, fresh.rows);
                    assert_eq!(galley.rect, fresh.rect);
                }
            });
        });
    }
    let _ = ctx.run(
        egui::RawInput {
            max_texture_side: Some(1024),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let galley = cache.layout_width(ui, text, false, "", 180.0);
                assert!(!Arc::ptr_eq(retained.as_ref().unwrap(), &galley));
                let fresh =
                    ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
                assert_eq!(galley.rows, fresh.rows);
                assert_eq!(galley.rect, fresh.rect);
            });
        },
    );
}

#[test]
fn search_highlights_preserve_wrapping_and_only_paint_visible_unicode_matches() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    configure(&ctx, false);
    let text = "  \"中文🙂\": \"hello wrapped string\",\r\n".repeat(80);
    let matches = forma::search::find_text(&text, "中文🙂");
    let mut cache = CodeCache::default();
    let mut painted = 0;
    let mut active = 0;
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let original = cache.layout_width(ui, &text, false, "", 100.0);
            let queried = cache.layout_width(ui, &text, false, "中文🙂", 100.0);
            assert!(
                Arc::ptr_eq(&original, &queried),
                "search must not rebuild the text galley"
            );
            let mut layouter = |_: &egui::Ui, _: &dyn egui::TextBuffer, _: f32| queried.clone();
            let mut buffer = text.clone();
            let mut editor = egui::TextEdit::multiline(&mut buffer)
                .layouter(&mut layouter)
                .show(ui);
            editor.text_clip_rect =
                egui::Rect::from_min_size(editor.galley_pos, egui::vec2(100.0, 40.0));
            cache.paint_matches(ui, &editor, &matches.ranges, 0, 0, palette(false));
            let row_index = cache.search_rows.as_ref().unwrap();
            assert_eq!(*row_index.starts.last().unwrap(), text.len());
            for range in &matches.ranges {
                assert_eq!(&text[range.clone()], "中文🙂");
            }
            assert_eq!(buffer, text);
        });
    });
    for shape in output.shapes {
        if let egui::Shape::Rect(rect) = shape.shape {
            if rect.fill == palette(false).search_match {
                painted += 1;
            } else if rect.fill == palette(false).search_active {
                painted += 1;
                active += 1;
                assert_eq!(rect.stroke.width, 1.5);
                assert_eq!(rect.stroke.color, palette(false).search_active.to_opaque());
            }
        }
    }
    assert!(
        painted > 0 && painted < 10,
        "only visible matches should create highlight shapes: {painted}"
    );
    assert_eq!(active, 1, "the selected match must have a distinct outline");
}

#[test]
fn cached_source_layout_tracks_font_atlas_rebuilds() {
    let ctx = egui::Context::default();
    let mut cache = CodeCache::default();
    let text = "{\n  \"name\": \"Forma\",\n  \"value\": 123\n}";
    let mut previous = None;
    for dark in [false, true, true, false, false, true] {
        configure(&ctx, dark);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Populate the atlas differently after each reset to expose stale glyph UVs.
                ui.label("abcdefghijklmnopqrstuvwxyz 0123456789");
                let actual = cache.layout(ui, text, dark, "name");
                let expected =
                    ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
                assert!(Arc::ptr_eq(&actual, &expected));
                if let Some((old_dark, old)) = &previous
                    && *old_dark != dark
                {
                    assert!(!Arc::ptr_eq(&actual, old));
                }
                assert_eq!(actual.job.text, text);
                previous = Some((dark, actual));
            });
        });
    }
    // Same colors and text, but reloading fonts still invalidates the old galley.
    let mut definitions = egui::FontDefinitions::default();
    let mono = definitions.families[&egui::FontFamily::Monospace][0].clone();
    let font = definitions.font_data[&mono].clone();
    definitions
        .font_data
        .insert("atlas-reset-test".into(), font);
    definitions
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .insert(0, "atlas-reset-test".into());
    ctx.set_fonts(definitions);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let actual = cache.layout(ui, text, true, "name");
            assert!(!Arc::ptr_eq(&actual, &previous.as_ref().unwrap().1));
            let expected =
                ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
            assert!(Arc::ptr_eq(&actual, &expected));
        });
    });
}

#[test]
fn giant_single_line_keeps_wrapped_rows_bounded_and_preserves_unicode_cursor_offsets() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let text = format!(
        "[\n{}\n]",
        vec![r#"{"id":123,"value":"中文🙂 long scrolling text"}"#; 360_000].join(",")
    );
    let mut cache = CodeCache::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let galley = cache.layout_width(ui, &text, false, "", 520.0);
            assert_eq!(galley.job.text, text);
            assert_eq!(
                galley
                    .rows
                    .iter()
                    .filter(|row| row.ends_with_newline)
                    .count(),
                2
            );
            assert!(
                galley.rows.iter().all(|row| row.glyphs.len() < 200),
                "no row may collapse millions of glyphs"
            );
            assert!(
                galley
                    .rows
                    .iter()
                    .all(|row| row.visuals.mesh.vertices.len() < 800)
            );
            let mut character = 0;
            for row in &galley.rows {
                for glyph in &row.glyphs {
                    character += 1;
                    assert!(glyph.pos.x < 600.0);
                }
                character += usize::from(row.ends_with_newline);
            }
            assert_eq!(character, text.chars().count());
            let reconstructed: String = galley
                .rows
                .iter()
                .flat_map(|row| {
                    row.glyphs
                        .iter()
                        .map(|glyph| glyph.chr)
                        .chain(row.ends_with_newline.then_some('\n'))
                })
                .collect();
            assert_eq!(reconstructed, text);
            let offset = text[..text.rfind("中文").unwrap()].chars().count();
            let cursor = egui::text::CCursor::new(offset);
            let rect = galley.pos_from_cursor(cursor);
            assert_eq!(
                galley.cursor_from_pos(rect.center().to_vec2()).index,
                offset
            );
        });
    });
}

#[test]
fn incremental_layout_matches_full_layout_after_edits_and_style_changes() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let mut cache = CodeCache::default();
    let padding = "  \"padding\": 123,\n".repeat(1100);
    let inputs = [
        format!("{{\n{padding}\"中文🙂\": \"escaped \\\" quote\"\n}}"),
        format!("{{\n{padding}\"中文🙂\"\n : \"changed\",\n\"last\": true\n}}\n"),
        format!("{{\n{padding}\"unfinished\n string\\\"\n still open"),
        format!("{{\n{padding}\"closed\n string\": 5\n}}"),
        format!("{{\n\"inserted\": [],\n{padding}\"last\": null\n}}"),
    ];
    for (iteration, text) in inputs.iter().chain(inputs.iter().rev()).enumerate() {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let dark = iteration >= 5 && iteration % 2 == 0;
                let width = if iteration >= 5 && iteration % 3 == 0 {
                    210.0
                } else {
                    370.0
                };
                let actual = cache.layout_width(ui, text, dark, "", width);
                let expected = CodeCache::default().layout_leaf(
                    ui,
                    text,
                    dark,
                    width,
                    EditorTypography::default(),
                );
                assert_eq!(actual.rows.len(), expected.rows.len());
                assert_eq!(actual.size(), expected.size());
                for (a, b) in actual.rows.iter().zip(&expected.rows) {
                    assert_eq!(a.pos, b.pos);
                    assert_eq!(a.ends_with_newline, b.ends_with_newline);
                    assert_eq!(
                        a.glyphs.iter().map(|g| g.chr).collect::<String>(),
                        b.glyphs.iter().map(|g| g.chr).collect::<String>()
                    );
                }
                for at in text.char_indices().map(|(at, _)| at) {
                    let color = |job: &egui::text::LayoutJob| {
                        job.sections
                            .iter()
                            .find(|s| s.byte_range.contains(&at))
                            .map(|s| s.format.color)
                    };
                    // Newline colors are not rendered; adjacent blocks may merge them differently.
                    if text.as_bytes()[at] != b'\n' {
                        assert_eq!(color(&actual.job), color(&expected.job), "byte {at}");
                    }
                }
            });
        });
    }
}

#[test]
#[ignore = "explicit CPU layout benchmark"]
fn incremental_layout_benchmark() {
    use std::time::Instant;
    let bytes = std::env::var("FORMA_BENCH_BYTES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1_048_576);
    for pretty in [true, false] {
        let row = if pretty {
            "  {\"id\":12345,\"name\":\"中文🙂 example\"},\n"
        } else {
            "{\"id\":12345,\"name\":\"中文🙂 example\"},"
        };
        let mut text = format!(
            "[\n{}null\n]",
            (0..bytes / row.len())
                .map(|i| row.replace("example", &format!("example {i}")))
                .collect::<String>()
        );
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut full = CodeCache::default();
        let mut incremental = CodeCache::default();
        let mut timings = [Vec::new(), Vec::new()];
        for i in 0..21 {
            let at = text.find("1234").unwrap();
            text.replace_range(at..at + 5, if i % 2 == 0 { "12345" } else { "12346" });
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for which in if i % 2 == 0 { [0, 1] } else { [1, 0] } {
                        let start = Instant::now();
                        let galley = if which == 0 {
                            full.layout_leaf(ui, &text, false, 520.0, EditorTypography::default())
                        } else {
                            incremental.layout_width(ui, &text, false, "", 520.0)
                        };
                        std::hint::black_box(galley);
                        if i > 0 {
                            timings[which].push(start.elapsed().as_secs_f64() * 1000.0);
                        }
                    }
                });
            });
        }
        for (which, samples) in timings.iter_mut().enumerate() {
            samples.sort_by(f64::total_cmp);
            println!(
                "layout bytes={} pretty={pretty} incremental={} p50_ms={:.3} p95_ms={:.3}",
                text.len(),
                which == 1,
                samples[10],
                samples[18]
            );
        }
    }
}

#[test]
fn unchanged_selection_reuses_painted_galley_and_invalidates_on_theme_or_range() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let id = egui::Id::new("selection_cache_regression");
    let mut cache = CodeCache::default();
    let mut text = "{\"name\":\"中文 work example\"}\n".repeat(1000);
    let mut last: Option<Arc<egui::Galley>> = None;
    for frame in 0..6 {
        if frame == 5 {
            text.push_str("{}");
        }
        let dark = frame >= 3;
        configure(&ctx, dark);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ctx.memory_mut(|memory| memory.request_focus(id));
                let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
                state.external_undo = true;
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(2),
                        egui::text::CCursor::new(if frame >= 4 { 5 } else { 6 }),
                    )));
                state.store(ctx, id);
                let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width| {
                    cache.layout_width(ui, buffer.as_str(), dark, "", width)
                };
                let output = egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .layouter(&mut layouter)
                    .show(ui);
                if let Some(previous) = &last {
                    if frame == 2 {
                        assert!(Arc::ptr_eq(previous, &output.galley));
                    } else if frame >= 3 {
                        assert!(!Arc::ptr_eq(previous, &output.galley));
                    }
                }
                last = Some(output.galley);
            });
        });
    }
}
