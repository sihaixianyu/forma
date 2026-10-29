use json_lens::{Format, format_document, process};
use std::{hint::black_box, sync::Arc, time::Instant};

fn median(mut run: impl FnMut() -> json_lens::Document) -> f64 {
    for _ in 0..3 {
        drop(black_box(run()));
    }
    let mut samples = Vec::new();
    for _ in 0..11 {
        let start = Instant::now();
        let result = black_box(run());
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        drop(result);
    }
    samples.sort_by(f64::total_cmp);
    samples[5]
}

fn main() {
    // All numbers are exactly representable in JS, so both runtimes retain the same values.
    let row = r#"{"id":123456,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}"#;
    println!("input_mib,implementation,operation,median_ms");
    for mib in [1, 5, 20] {
        let count = mib * 1024 * 1024 / (row.len() + 1);
        let text = Arc::new(format!("[{}]", vec![row; count].join(",")));
        for (name, format) in [
            ("parse", Format::Keep),
            ("parse_pretty", Format::Pretty),
            ("parse_compact", Format::Compact),
        ] {
            let elapsed = median(|| process(text.clone(), format).unwrap());
            println!("{mib},json-lens,{name},{elapsed:.3}");
        }
        let doc = process(text, Format::Keep).unwrap();
        for (name, format) in [
            ("cached_pretty", Format::Pretty),
            ("cached_compact", Format::Compact),
        ] {
            let elapsed = median(|| format_document(&doc, format).unwrap());
            println!("{mib},json-lens,{name},{elapsed:.3}");
        }
    }
}
