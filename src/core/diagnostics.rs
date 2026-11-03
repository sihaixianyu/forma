use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};
use web_time::{Instant, SystemTime, UNIX_EPOCH};

static LOGGER: DebugLogger = DebugLogger;
static NAVIGATION_LOG: AtomicBool = AtomicBool::new(false);
#[cfg(not(target_arch = "wasm32"))]
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
static NEXT_JOB: AtomicUsize = AtomicUsize::new(1);
thread_local! { static JOB_ID: Cell<usize> = const { Cell::new(0) }; }

struct DebugLogger;
impl Log for DebugLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level() && metadata.target().starts_with("forma")
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
            "[Forma {}] ts_ms={} job={} realm={} {}",
            record.level(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis()),
            job,
            realm,
            record.args()
        );
        #[cfg(not(target_arch = "wasm32"))]
        {
            eprintln!("{text}");
            if let Ok(mut file) = LOG_FILE.lock()
                && let Some(file) = file.as_mut()
            {
                let _ = writeln!(file, "{text}");
            }
        }
        #[cfg(target_arch = "wasm32")]
        match record.level() {
            Level::Error => web_sys::console::error_1(&text.into()),
            Level::Warn => web_sys::console::warn_1(&text.into()),
            Level::Debug | Level::Trace => web_sys::console::debug_1(&text.into()),
            Level::Info => web_sys::console::info_1(&text.into()),
        }
    }

    fn flush(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(mut file) = LOG_FILE.lock()
            && let Some(file) = file.as_mut()
        {
            let _ = file.flush();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn initialize_file(debug: bool) -> Option<PathBuf> {
    if !debug {
        return None;
    }
    let path = std::env::var_os("FORMA_LOG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("forma-debug.log"));
    match OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => {
            *LOG_FILE.lock().ok()? = Some(file);
            Some(path)
        }
        Err(error) => {
            eprintln!(
                "Forma could not create debug log {}: {error}",
                path.display()
            );
            None
        }
    }
}

/// Debug builds record low-volume navigation diagnostics. Full timing logs remain opt-in.
pub fn init() {
    #[cfg(not(target_arch = "wasm32"))]
    let debug = std::env::var("FORMA_LOG").is_ok_and(|value| value.eq_ignore_ascii_case("debug"));
    #[cfg(target_arch = "wasm32")]
    let debug = web_debug_requested();
    NAVIGATION_LOG.store(cfg!(debug_assertions) || debug, Ordering::Relaxed);
    #[cfg(not(target_arch = "wasm32"))]
    let file = initialize_file(NAVIGATION_LOG.load(Ordering::Relaxed));
    configure(debug);
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = file {
        navigation(format_args!("event=debug_log_file path={path:?}"));
    }
    #[cfg(target_arch = "wasm32")]
    navigation(format_args!("event=navigation_log_ready realm=main"));
}

#[cfg(target_arch = "wasm32")]
fn web_debug_requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| {
            query
                .trim_start_matches('?')
                .split('&')
                .any(|pair| pair == "debug=1")
        })
}

/// Low-volume caret/Path/Graph trace used to diagnose navigation races.
pub fn navigation(args: std::fmt::Arguments<'_>) {
    if !NAVIGATION_LOG.load(Ordering::Relaxed) {
        return;
    }
    let text = format!(
        "[Forma NAV] ts_ms={} job={} {}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis()),
        JOB_ID.with(Cell::get),
        args
    );
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(mut file) = LOG_FILE.lock()
        && let Some(file) = file.as_mut()
    {
        let _ = writeln!(file, "{text}");
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::debug_1(&text.clone().into());
        if web_debug_requested()
            && let Some(window) = web_sys::window()
        {
            let message =
                percent_encoding::utf8_percent_encode(&text, percent_encoding::NON_ALPHANUMERIC);
            let request =
                window.fetch_with_str(&format!("http://127.0.0.1:8081/log?message={message}"));
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(request).await;
            });
        }
    }
}

pub fn configure(debug: bool) {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    });
    log::debug!(target: "forma", "event=debug_enabled platform={} simd128={}",
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
            start: log::log_enabled!(target: "forma", Level::Debug).then(Instant::now),
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
                log::debug!(target: "forma", "event=stage stage={} bytes={} elapsed_ms={:.3}", self.name, self.bytes, ms);
            }
        }
    }
}
