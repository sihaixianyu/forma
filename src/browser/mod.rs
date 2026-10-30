#[cfg(feature = "bench-web")]
mod bench;

use eframe::egui;
use std::{cell::Cell, rc::Rc, sync::mpsc};

pub enum FileEvent {
    Opened { name: String, text: String },
    Error(String),
}

pub struct Files {
    sender: mpsc::Sender<FileEvent>,
    pub events: mpsc::Receiver<FileEvent>,
    pending: Rc<Cell<bool>>,
    ctx: egui::Context,
}

impl Files {
    pub fn new(ctx: egui::Context) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            sender,
            events,
            pending: Rc::new(Cell::new(false)),
            ctx,
        }
    }

    pub fn open(&self) {
        if self.pending.replace(true) {
            return;
        }
        let (sender, pending, ctx) = (self.sender.clone(), self.pending.clone(), self.ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("JSON / CSV", &["json", "csv", "txt"])
                .pick_file()
                .await
            {
                let name = file.file_name();
                let result = match String::from_utf8(file.read().await) {
                    Ok(text) => FileEvent::Opened { name, text },
                    Err(_) => FileEvent::Error("Choose a UTF-8 JSON or CSV file".into()),
                };
                let _ = sender.send(result);
            }
            pending.set(false);
            ctx.request_repaint();
        });
    }
}

fn install_find_shortcut(document: &web_sys::Document) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsCast, closure::Closure};
    let page = document.clone();
    let handler =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            if (event.meta_key() || event.ctrl_key())
                && !event.alt_key()
                && !event.is_composing()
                && (event.key().eq_ignore_ascii_case("f")
                    || (event.shift_key() && event.key().eq_ignore_ascii_case("e")))
                && page.get_element_by_id("loading").is_none()
                && page.get_element_by_id("rfd-card").is_none()
            {
                // Reserve app shortcuts synchronously while letting egui receive the event.
                // Document capture also covers eframe's hidden IME text input.
                event.prevent_default();
            }
        });
    document.add_event_listener_with_callback_and_bool(
        "keydown",
        handler.as_ref().unchecked_ref(),
        true,
    )?;
    handler.forget(); // One listener for the lifetime of this page.
    Ok(())
}

pub fn start() {
    use wasm_bindgen::JsCast;
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id("json-lens")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let runner = eframe::WebRunner::new();
        match runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(crate::App::new(cc)))),
            )
            .await
        {
            Ok(()) => {
                if let Err(error) = install_find_shortcut(&document) {
                    web_sys::console::error_1(&error);
                }
                if let Some(root) = document.document_element() {
                    let _ = root.set_attribute(
                        "data-build-revision",
                        option_env!("JSON_LENS_BUILD_REVISION").unwrap_or("local"),
                    );
                    let _ = root.set_attribute(
                        "data-build-mode",
                        if cfg!(debug_assertions) {
                            "debug"
                        } else {
                            "release"
                        },
                    );
                    let _ = root.set_attribute(
                        "data-wasm-simd",
                        if cfg!(target_feature = "simd128") {
                            "true"
                        } else {
                            "false"
                        },
                    );
                }
                if let Some(loading) = document.get_element_by_id("loading") {
                    loading.remove();
                }
            }
            Err(error) => {
                web_sys::console::error_1(&error);
                if let Some(loading) = document.get_element_by_id("loading") {
                    loading.set_text_content(Some(
                        "Could not start JSON Lens. Enable WebGPU or WebGL2 and reload.",
                    ));
                }
            }
        }
    });
}
