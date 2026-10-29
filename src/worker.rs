use eframe::egui;
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use web_time::Instant;

#[cfg(not(target_arch = "wasm32"))]
struct Queued<T> {
    value: T,
    id: usize,
    enqueued: Option<Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> Queued<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            id: json_lens::diagnostics::next_job(),
            enqueued: log::log_enabled!(target: "json_lens", log::Level::Debug).then(Instant::now),
        }
    }

    fn run<R>(self, process: impl FnOnce(T) -> R) -> R {
        json_lens::diagnostics::in_job(self.id, || {
            if let Some(enqueued) = self.enqueued {
                log::debug!(target: "json_lens", "event=job_dequeued queue_ms={:.3}", enqueued.elapsed().as_secs_f64()*1000.0);
            }
            let _stage = json_lens::diagnostics::Stage::new("worker_job", 0);
            process(self.value)
        })
    }
}

/// Native threads and browser-local tasks share the same non-blocking mailbox API.
pub struct Sender<T> {
    #[cfg(not(target_arch = "wasm32"))]
    inner: Channel<T>,
    #[cfg(target_arch = "wasm32")]
    pub(super) submit: std::rc::Rc<dyn Fn(T) -> Result<(), mpsc::SendError<T>>>,
}

#[cfg(not(target_arch = "wasm32"))]
enum Channel<T> {
    Queued(mpsc::Sender<Queued<T>>),
    #[cfg(test)]
    Direct(mpsc::Sender<T>),
}

#[cfg(all(test, not(target_arch = "wasm32")))]
impl<T> From<mpsc::Sender<T>> for Sender<T> {
    fn from(sender: mpsc::Sender<T>) -> Self {
        Self {
            inner: Channel::Direct(sender),
        }
    }
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), mpsc::SendError<T>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &self.inner {
                Channel::Queued(sender) => sender
                    .send(Queued::new(value))
                    .map_err(|error| mpsc::SendError(error.0.value)),
                #[cfg(test)]
                Channel::Direct(sender) => sender.send(value),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            (self.submit)(value)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub trait Task<R>: Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static, R> Task<R> for T {}

#[cfg(target_arch = "wasm32")]
pub trait Task<R>: Send + 'static {
    const KIND: &'static str;
    fn encode(&self, force: bool) -> Result<Vec<u8>, String>;
    fn decode(self, bytes: &[u8]) -> R;
    fn failed(self, error: String) -> R;
}

pub fn spawn<T: Task<R>, R: Send + 'static>(
    ctx: egui::Context,
    results: mpsc::Sender<R>,
    coalesce: bool,
    process: impl Fn(T) -> R + Send + 'static,
) -> Sender<T> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = mpsc::channel::<Queued<T>>();
        std::thread::spawn(move || {
            for mut request in &receiver {
                if coalesce {
                    while let Ok(newer) = receiver.try_recv() {
                        log::debug!(target: "json_lens", "event=job_coalesced dropped_job={} replacement_job={}", request.id, newer.id);
                        request = newer;
                    }
                }
                if results.send(request.run(&process)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Sender {
            inner: Channel::Queued(sender),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = process;
        crate::web_worker::spawn(ctx, results, coalesce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Barrier},
        time::Duration,
    };

    #[test]
    fn ordered_jobs_and_coalesced_searches_use_the_same_mailbox() {
        let (results, receiver) = mpsc::channel();
        let sender = spawn(egui::Context::default(), results, false, |value: usize| {
            value * 2
        });
        for value in 0..3 {
            sender.send(value).unwrap();
        }
        for expected in [0, 2, 4] {
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
                expected
            );
        }
        let barrier = Arc::new(Barrier::new(2));
        let gate = barrier.clone();
        let (results, receiver) = mpsc::channel();
        let sender = spawn(egui::Context::default(), results, true, move |value| {
            if value == 0 {
                gate.wait();
                gate.wait();
            }
            value
        });
        sender.send(0).unwrap();
        barrier.wait();
        sender.send(1).unwrap();
        sender.send(2).unwrap();
        barrier.wait();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(3)).unwrap(), 0);
        assert_eq!(receiver.recv_timeout(Duration::from_secs(3)).unwrap(), 2);
    }
}
