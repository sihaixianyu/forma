use super::*;

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
    let matches = json_lens::search::find_text(&text, "中文🙂");
    let mut cache = CodeCache::default();
    let mut painted = 0;
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
        if let egui::Shape::Rect(rect) = shape.shape
            && (rect.fill == palette(false).accent.gamma_multiply(0.10)
                || rect.fill == palette(false).amber.gamma_multiply(0.23))
        {
            painted += 1;
        }
    }
    assert!(
        painted > 0 && painted < 10,
        "only visible matches should create highlight shapes: {painted}"
    );
}

#[test]
fn cached_source_layout_tracks_font_atlas_rebuilds() {
    let ctx = egui::Context::default();
    let mut cache = CodeCache::default();
    let text = "{\n  \"name\": \"JSON Lens\",\n  \"value\": 123\n}";
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
