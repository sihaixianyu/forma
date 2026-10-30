use crate::worker::{Sender, Task};
use eframe::egui;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::mpsc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};
use web_time::Instant;

trait Pending {
    fn kind(&self) -> &'static str;
    fn encode(&self, force: bool) -> Result<Vec<u8>, String>;
    fn finish(self: Box<Self>, result: Result<&[u8], String>);
}

struct Typed<T, R> {
    value: T,
    results: mpsc::Sender<R>,
    ctx: egui::Context,
}
impl<T: Task<R>, R: 'static> Pending for Typed<T, R> {
    fn kind(&self) -> &'static str {
        T::KIND
    }
    fn encode(&self, force: bool) -> Result<Vec<u8>, String> {
        self.value.encode(force)
    }
    fn finish(self: Box<Self>, result: Result<&[u8], String>) {
        let value = match result {
            Ok(bytes) => self.value.decode(bytes),
            Err(error) => self.value.failed(error),
        };
        let _ = self.results.send(value);
        self.ctx.request_repaint();
    }
}

struct Entry {
    id: usize,
    lane: usize,
    pending: Box<dyn Pending>,
    queued: Option<Instant>,
    retry: bool,
}
type MessageHandler = Closure<dyn FnMut(MessageEvent)>;
type ErrorHandler = Closure<dyn FnMut(ErrorEvent)>;
struct Hub {
    worker: Worker,
    ready: Cell<bool>,
    dead: Cell<bool>,
    queue: RefCell<VecDeque<Entry>>,
    active: RefCell<Option<Entry>>,
    on_message: RefCell<Option<MessageHandler>>,
    on_error: RefCell<Option<ErrorHandler>>,
}

thread_local! { static HUB: RefCell<Option<Result<Rc<Hub>, String>>> = const { RefCell::new(None) }; }
fn get(value: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(value, &name.into()).unwrap_or(JsValue::UNDEFINED)
}
fn set(value: &JsValue, name: &str, field: &JsValue) {
    let _ = js_sys::Reflect::set(value, &name.into(), field);
}

impl Hub {
    fn new() -> Result<Rc<Self>, String> {
        let config = get(&js_sys::global(), "jsonLensWorkerConfig");
        let url = get(&config, "workerUrl")
            .as_string()
            .ok_or("Worker runtime configuration missing")?;
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker = Worker::new_with_options(&url, &options)
            .map_err(|error| format!("Cannot start processing Worker: {error:?}"))?;
        let hub = Rc::new(Self {
            worker,
            ready: Cell::new(false),
            dead: Cell::new(false),
            queue: RefCell::new(VecDeque::new()),
            active: RefCell::new(None),
            on_message: RefCell::new(None),
            on_error: RefCell::new(None),
        });
        let weak = Rc::downgrade(&hub);
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Some(hub) = weak.upgrade() {
                hub.message(event.data());
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        hub.worker
            .set_onmessage(Some(message.as_ref().unchecked_ref()));
        *hub.on_message.borrow_mut() = Some(message);
        let weak = Rc::downgrade(&hub);
        let error = Closure::wrap(Box::new(move |event: ErrorEvent| {
            event.prevent_default();
            if let Some(hub) = weak.upgrade() {
                hub.fail(format!(
                    "Processing Worker failed: {}. Reload the page.",
                    event.message()
                ));
            }
        }) as Box<dyn FnMut(ErrorEvent)>);
        hub.worker.set_onerror(Some(error.as_ref().unchecked_ref()));
        *hub.on_error.borrow_mut() = Some(error);
        set(&config, "type", &"init".into());
        hub.worker
            .post_message(&config)
            .map_err(|error| format!("Worker initialization failed: {error:?}"))?;
        Ok(hub)
    }

    fn submit(&self, entry: Entry, coalesce: bool) {
        if self.dead.get() {
            entry
                .pending
                .finish(Err("Processing Worker unavailable. Reload the page.".into()));
            return;
        }
        if coalesce {
            self.queue.borrow_mut().retain(|older| {
                if older.lane == entry.lane {
                    log::debug!(target: "json_lens", "event=job_coalesced dropped_job={} replacement_job={} backend=web_worker", older.id, entry.id);
                    false
                } else { true }
            });
        }
        self.queue.borrow_mut().push_back(entry);
        self.pump();
    }

    fn pump(&self) {
        if !self.ready.get() || self.dead.get() || self.active.borrow().is_some() {
            return;
        }
        let Some(entry) = self.queue.borrow_mut().pop_front() else {
            return;
        };
        self.send(entry);
    }

    fn send(&self, entry: Entry) {
        json_lens::diagnostics::in_job(entry.id, || {
            if let Some(queued) = entry.queued {
                log::debug!(target: "json_lens", "event=job_dequeued queue_ms={:.3} backend=web_worker", queued.elapsed().as_secs_f64()*1000.0);
            }
        });
        let encoded = json_lens::diagnostics::in_job(entry.id, || {
            let _stage = json_lens::diagnostics::Stage::new("worker_encode", 0);
            entry.pending.encode(entry.retry)
        });
        let bytes = match encoded {
            Ok(bytes) => bytes,
            Err(error) => {
                entry.pending.finish(Err(error));
                self.pump();
                return;
            }
        };
        let data = js_sys::Object::new();
        set(&data, "type", &"run".into());
        set(&data, "id", &JsValue::from_f64(entry.id as f64));
        set(&data, "kind", &entry.pending.kind().into());
        let bytes = js_sys::Uint8Array::from(bytes.as_slice());
        set(&data, "payload", &bytes);
        let transfer = js_sys::Array::of1(&bytes.buffer());
        *self.active.borrow_mut() = Some(entry);
        if let Err(error) = self.worker.post_message_with_transfer(&data, &transfer) {
            self.fail(format!("Worker message failed: {error:?}"));
        }
    }

    fn message(&self, data: JsValue) {
        match get(&data, "type").as_string().as_deref() {
            Some("ready") => {
                self.ready.set(true);
                log::debug!(target: "json_lens", "event=worker_ready backend=web_worker");
                self.pump();
            }
            Some("debug") => {
                if let Some(text) = get(&data, "text").as_string() {
                    web_sys::console::debug_1(&text.into());
                }
            }
            Some("fatal") => self.fail(
                get(&data, "error")
                    .as_string()
                    .unwrap_or_else(|| "Worker startup failed".into()),
            ),
            Some("result" | "error") => {
                let Some(mut entry) = self.active.borrow_mut().take() else {
                    return;
                };
                if get(&data, "id").as_f64() != Some(entry.id as f64) {
                    self.fail("Worker response ID mismatch".into());
                    entry.pending.finish(Err("Worker protocol error".into()));
                    return;
                }
                if get(&data, "type").as_string().as_deref() == Some("error") {
                    let error = get(&data, "error")
                        .as_string()
                        .unwrap_or_else(|| "Worker operation failed".into());
                    if error == "cache_miss" && !entry.retry {
                        log::debug!(target: "json_lens", "event=worker_cache_retry job_id={}", entry.id);
                        entry.retry = true;
                        self.send(entry);
                        return;
                    }
                    entry.pending.finish(Err(error));
                } else {
                    let payload = js_sys::Uint8Array::new(&get(&data, "payload"));
                    json_lens::diagnostics::in_job(entry.id, || {
                        let _stage = json_lens::diagnostics::Stage::new(
                            "worker_decode",
                            payload.byte_length() as usize,
                        );
                        let bytes = payload.to_vec();
                        if let Some(queued) = entry.queued {
                            log::debug!(target: "json_lens", "event=worker_roundtrip elapsed_ms={:.3}", queued.elapsed().as_secs_f64()*1000.0);
                        }
                        entry.pending.finish(Ok(&bytes));
                    });
                }
                self.pump();
            }
            _ => {}
        }
    }

    fn fail(&self, error: String) {
        self.dead.set(true);
        self.worker.terminate();
        log::error!(target: "json_lens", "event=worker_failed error={error:?}");
        if let Some(entry) = self.active.borrow_mut().take() {
            entry.pending.finish(Err(error.clone()));
        }
        let queue = self.queue.borrow_mut().drain(..).collect::<Vec<_>>();
        for entry in queue {
            entry.pending.finish(Err(error.clone()));
        }
    }
}

pub fn spawn<T: Task<R>, R: Send + 'static>(
    ctx: egui::Context,
    results: mpsc::Sender<R>,
    coalesce: bool,
) -> Sender<T> {
    let hub = HUB.with(|slot| slot.borrow_mut().get_or_insert_with(Hub::new).clone());
    let lane = json_lens::diagnostics::next_job();
    Sender {
        submit: Rc::new(move |value| {
            let pending = Box::new(Typed {
                value,
                results: results.clone(),
                ctx: ctx.clone(),
            });
            match &hub {
                Ok(hub) => hub.submit(
                    Entry {
                        id: json_lens::diagnostics::next_job(),
                        lane,
                        pending,
                        queued: log::log_enabled!(target: "json_lens", log::Level::Debug)
                            .then(Instant::now),
                        retry: false,
                    },
                    coalesce,
                ),
                Err(error) => {
                    log::error!(target: "json_lens", "event=worker_failed phase=startup error={error:?}");
                    pending.finish(Err(error.clone()));
                }
            }
            Ok(())
        }),
    }
}
