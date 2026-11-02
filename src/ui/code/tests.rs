use super::*;

#[test]
fn typography_clamps_to_supported_ranges_and_rebuilds_layouts() {
    assert_eq!(
        EditorTypography::default(),
        EditorTypography {
            font_size: 13.0,
            line_height: 20.0,
        },
        "editor defaults should match json4u's 13px Monaco font and 20px macOS line height"
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
