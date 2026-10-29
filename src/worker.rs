use eframe::egui;
use std::sync::mpsc;

/// Native threads and browser-local tasks share the same non-blocking mailbox API.
pub struct Sender<T> {
    #[cfg(not(target_arch = "wasm32"))]
    inner: mpsc::Sender<T>,
    #[cfg(target_arch = "wasm32")]
    submit: std::rc::Rc<dyn Fn(T)>,
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), mpsc::SendError<T>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.send(value)
        }
        #[cfg(target_arch = "wasm32")]
        {
            (self.submit)(value);
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> From<mpsc::Sender<T>> for Sender<T> {
    fn from(inner: mpsc::Sender<T>) -> Self {
        Self { inner }
    }
}

pub fn spawn<T: Send + 'static, R: Send + 'static>(
    ctx: egui::Context,
    results: mpsc::Sender<R>,
    coalesce: bool,
    process: impl Fn(T) -> R + Send + 'static,
) -> Sender<T> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for mut request in &receiver {
                if coalesce {
                    while let Ok(newer) = receiver.try_recv() {
                        request = newer;
                    }
                }
                if results.send(process(request)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        sender.into()
    }
    #[cfg(target_arch = "wasm32")]
    {
        use std::{
            cell::{Cell, RefCell},
            collections::VecDeque,
            rc::Rc,
        };
        let queue = Rc::new(RefCell::new(VecDeque::<T>::new()));
        let running = Rc::new(Cell::new(false));
        let process = Rc::new(process);
        Sender {
            submit: Rc::new(move |request| {
                if coalesce {
                    queue.borrow_mut().clear();
                }
                queue.borrow_mut().push_back(request);
                if running.replace(true) {
                    return;
                }
                let (queue, running, process, results, ctx) = (
                    queue.clone(),
                    running.clone(),
                    process.clone(),
                    results.clone(),
                    ctx.clone(),
                );
                wasm_bindgen_futures::spawn_local(async move {
                    loop {
                        // Let browser events and rendering run before the next CPU operation.
                        yield_to_browser().await;
                        let request = queue.borrow_mut().pop_front();
                        let Some(request) = request else {
                            break;
                        };
                        if results.send(process(request)).is_err() {
                            break;
                        }
                        ctx.request_repaint();
                    }
                    running.set(false);
                });
            }),
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn yield_to_browser() {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(error) = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
        {
            let _ = reject.call1(&wasm_bindgen::JsValue::NULL, &error);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
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
