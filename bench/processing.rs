use json_lens::{Format, format_document, process};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    json_lens::diagnostics::init();
    let row = r#"{"id":12345678901234567890,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}"#;
    println!("size MiB | operation | median ms | MiB/s");
    for mib in [1, 5, 20] {
        let count = mib * 1024 * 1024 / (row.len() + 1);
        let text = Arc::new(format!("[{}]", vec![row; count].join(",")));
        for format in [Format::Keep, Format::Pretty, Format::Compact] {
            drop(process(text.clone(), format).unwrap());
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                let doc = black_box(process(text.clone(), format).unwrap());
                times.push(start.elapsed().as_secs_f64());
                drop(doc);
            }
            times.sort_by(f64::total_cmp);
            let seconds = times[times.len() / 2];
            let size = text.len() as f64 / (1024.0 * 1024.0);
            println!(
                "{size:.2} | {format:?} | {:.2} | {:.2}",
                seconds * 1000.0,
                size / seconds
            );
        }
        let parsed = process(text.clone(), Format::Keep).unwrap();
        for format in [Format::Pretty, Format::Compact] {
            drop(format_document(&parsed, format).unwrap());
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                let doc = black_box(format_document(&parsed, format).unwrap());
                times.push(start.elapsed().as_secs_f64());
                drop(doc);
            }
            times.sort_by(f64::total_cmp);
            let seconds = times[times.len() / 2];
            let size = text.len() as f64 / (1024.0 * 1024.0);
            println!(
                "{size:.2} | cached {format:?} | {:.2} | {:.2}",
                seconds * 1000.0,
                size / seconds
            );
        }
    }
}
