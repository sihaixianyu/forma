#[allow(dead_code, unused_imports)]
#[path = "../src/canvas.rs"]
mod canvas;
#[path = "../src/ui/decode.rs"]
#[allow(dead_code, unused_imports)]
mod decode_ui;
use eframe::egui;
use json_lens::{Format, format_document, graph::build_graph, process, search::find_tree};
use std::{hint::black_box, sync::Arc, time::Instant};

fn measure(name: &str, mut run: impl FnMut()) {
    run();
    let mut samples = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        run();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!("{name}: {:.3} ms median", samples[3]);
}

fn main() {
    let row = r#"{"id":12345678901234567890,"name":"中文 example","active":true,"data":[1,2,3]}"#;
    let text = Arc::new(format!("[{}]", vec![row; 9998].join(",")));
    let doc = process(text, Format::Keep).unwrap();
    let graph = build_graph(&doc.value);
    println!(
        "{} bytes · {} graph cards",
        doc.text.len(),
        graph.nodes.len()
    );
    measure("graph build", || {
        black_box(build_graph(&doc.value));
    });
    measure("1000 pointer lookups", || {
        for i in 0..1000 {
            black_box(graph.container_for(&format!("/{}/name", i * 7)));
        }
    });
    measure("structured search", || {
        black_box(find_tree(&doc.value, "example"));
    });
    measure("format plus rebuilt graph", || {
        let formatted = format_document(&doc, Format::Pretty).unwrap();
        black_box(build_graph(&formatted.value));
    });
    measure("cached format plus graph", || {
        let formatted = format_document(&doc, Format::Pretty).unwrap();
        if Arc::ptr_eq(&formatted.value, &doc.value) {
            black_box(&graph);
        } else {
            black_box(build_graph(&formatted.value));
        }
    });
    let ctx = egui::Context::default();
    let mut view = canvas::Canvas::default();
    let mut pointer = "/4000/name".to_owned();
    let mut time = 0.0;
    measure("10 static canvas frames", || {
        for _ in 0..10 {
            time += 1.0 / 60.0;
            black_box(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 700.0),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        view.show(ui, &graph, false, &mut pointer);
                    });
                },
            ));
        }
    });
}
