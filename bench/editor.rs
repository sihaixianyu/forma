//! Actual workspace UI and tessellation CPU timings; no GPU or window required.
use super::{App, View, ui};
use eframe::egui;
use std::{hint::black_box, sync::Arc, time::Instant};

fn frame(app: &mut App, ctx: &egui::Context, time: f64, scroll: bool) -> (f64, f64) {
    let start = Instant::now();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            time: Some(time),
            events: if scroll {
                vec![
                    egui::Event::PointerMoved(egui::pos2(300.0, 400.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -160.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            },
            ..Default::default()
        },
        |ctx| app.workspace(ctx),
    );
    let ui_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
    (ui_ms, start.elapsed().as_secs_f64() * 1000.0)
}

pub fn run() {
    for pretty in [false, true] {
        let row = if pretty {
            "  {\n    \"id\": 12345,\n    \"name\": \"JSON Lens performance example\",\n    \"active\": true,\n    \"data\": [1, 2, 3]\n  }"
        } else {
            "{\"id\":12345,\"name\":\"JSON Lens performance example\",\"active\":true,\"data\":[1,2,3]}"
        };
        let text = Arc::new(format!(
            "[\n{}\n]",
            vec![row; 1_000_000 / (row.len() + 2)].join(if pretty { ",\n" } else { "," })
        ));
        for view in [View::Graph, View::Compare] {
            let ctx = egui::Context::default();
            ui::install_fonts(&ctx);
            ui::configure(&ctx, false);
            let mut app = App::create(ctx.clone(), "CPU benchmark".into());
            app.editor = (*text).clone();
            app.full_text = text.clone();
            app.search_source = text.clone();
            app.editable = true;
            let doc = Arc::new(json_lens::process(text.clone(), json_lens::Format::Keep).unwrap());
            app.graph = Arc::new(json_lens::graph::build_graph(&doc.value));
            app.document = Some(doc);
            app.view = view;
            if app.view == View::Compare {
                app.compare_text = (*text).clone();
                app.compare_alignment = Some(Arc::new(json_lens::alignment::align(&text, &text)));
                app.comparison_done = true;
            }
            let cold = frame(&mut app, &ctx, 0.0, false);
            for scroll in [false, true] {
                let mut samples = Vec::new();
                for i in 0..8 {
                    let result = frame(
                        &mut app,
                        &ctx,
                        (i + 1 + usize::from(scroll) * 8) as f64 / 60.0,
                        scroll,
                    );
                    samples.push(result.0 + result.1);
                }
                samples.sort_by(f64::total_cmp);
                println!(
                    "{}",
                    serde_json::json!({
                        "bytes":text.len(), "pretty":pretty, "mode":if app.view == View::Graph {"graph"} else {"compare"},
                        "phase":if scroll {"scroll"} else {"idle"}, "cold_ui_ms":cold.0,
                        "cold_tessellation_ms":cold.1, "median_frame_ms":samples[4], "max_frame_ms":samples[7]
                    })
                );
            }
        }
    }
}
