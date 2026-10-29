use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use web_time::Instant;

static LOGGER: DebugLogger = DebugLogger;
static NEXT_JOB: AtomicUsize = AtomicUsize::new(1);
thread_local! { static JOB_ID: Cell<usize> = const { Cell::new(0) }; }

struct DebugLogger;
impl Log for DebugLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level() && metadata.target().starts_with("json_lens")
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let job = JOB_ID.with(Cell::get);
        let realm = if cfg!(target_arch = "wasm32") {
            #[cfg(target_arch = "wasm32")]
            {
                if web_sys::window().is_some() {
                    "main"
                } else {
                    "worker"
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                "native"
            }
        } else {
            "native"
        };
        let text = format!(
            "[JSON Lens {}] job={} realm={} {}",
            record.level(),
            job,
            realm,
            record.args()
        );
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{text}");
        #[cfg(target_arch = "wasm32")]
        match record.level() {
            Level::Error => web_sys::console::error_1(&text.into()),
            Level::Warn => web_sys::console::warn_1(&text.into()),
            Level::Debug | Level::Trace => web_sys::console::debug_1(&text.into()),
            Level::Info => web_sys::console::info_1(&text.into()),
        }
    }

    fn flush(&self) {}
}

/// Native: JSON_LENS_LOG=debug. Browser: ?debug=1 (Console verbose level).
pub fn init() {
    #[cfg(not(target_arch = "wasm32"))]
    let debug =
        std::env::var("JSON_LENS_LOG").is_ok_and(|value| value.eq_ignore_ascii_case("debug"));
    #[cfg(target_arch = "wasm32")]
    let debug = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| {
            query
                .trim_start_matches('?')
                .split('&')
                .any(|pair| pair == "debug=1")
        });
    configure(debug);
}

pub fn configure(debug: bool) {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    });
    log::debug!(target: "json_lens", "event=debug_enabled platform={} simd128={}",
        if cfg!(target_arch = "wasm32") { "wasm" } else { "native" }, cfg!(target_feature = "simd128"));
}

pub fn next_job() -> usize {
    NEXT_JOB.fetch_add(1, Ordering::Relaxed)
}

/// Correlate all synchronous stages within a job; restore context after nested jobs.
pub fn in_job<T>(id: usize, run: impl FnOnce() -> T) -> T {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            JOB_ID.with(|job| job.set(self.0));
        }
    }
    let _restore = Restore(JOB_ID.with(|job| job.replace(id)));
    run()
}

/// Stage wall time. Disabled logs do not read the clock or allocate text.
pub struct Stage {
    name: &'static str,
    bytes: usize,
    start: Option<Instant>,
    min_ms: f64,
}

impl Stage {
    pub fn new(name: &'static str, bytes: usize) -> Self {
        Self {
            name,
            bytes,
            start: log::log_enabled!(target: "json_lens", Level::Debug).then(Instant::now),
            min_ms: 0.0,
        }
    }

    pub fn slow(name: &'static str, bytes: usize, min_ms: f64) -> Self {
        let mut stage = Self::new(name, bytes);
        stage.min_ms = min_ms;
        stage
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            if ms >= self.min_ms {
                log::debug!(target: "json_lens", "event=stage stage={} bytes={} elapsed_ms={:.3}", self.name, self.bytes, ms);
            }
        }
    }
}
