use json_lens::{Document, Format, format_document, process};
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
    eprintln!(
        "Native parser worker threads: {}",
        rayon::current_num_threads()
    );
    // All numbers are exactly representable in JS, so both runtimes retain the same values.
    let row = r#"{"id":123456,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}"#;
    println!("input_mib,implementation,operation,median_ms");
    for mib in [1, 5, 20] {
        let count = mib * 1024 * 1024 / (row.len() + 1);
        let text = Arc::new(format!("[{}]", vec![row; count].join(",")));
        for (implementation, parse) in [
            (
                "json-lens",
                process as fn(Arc<String>, Format) -> Result<Document, String>,
            ),
            ("serde-json", reference_process),
        ] {
            for (name, format) in [
                ("parse", Format::Keep),
                ("parse_pretty", Format::Pretty),
                ("parse_compact", Format::Compact),
            ] {
                let elapsed = median(|| parse(text.clone(), format).unwrap());
                println!("{mib},{implementation},{name},{elapsed:.3}");
            }
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

// The original production path, with identical precision, key order and formatting.
fn reference_process(text: Arc<String>, format: Format) -> Result<Document, String> {
    let value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    format_document(
        &Document {
            text,
            value: Arc::new(value),
        },
        format,
    )
}
