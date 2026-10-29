use eframe::egui;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, mpsc},
};

pub enum FileEvent {
    Opened { name: String, text: String },
    Saved(String),
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

    pub fn save(&self, data: Arc<Vec<u8>>, name: String) {
        let event = match download(&data, &name) {
            Ok(()) => FileEvent::Saved(name),
            Err(error) => FileEvent::Error(format!("Download failed: {error:?}")),
        };
        let _ = self.sender.send(event);
        self.ctx.request_repaint();
    }
}

fn download(data: &[u8], name: &str) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsCast, closure::Closure};
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let bytes = js_sys::Uint8Array::from(data);
    let parts = js_sys::Array::of1(&bytes);
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let result = (|| {
        let anchor = document
            .create_element("a")?
            .dyn_into::<web_sys::HtmlAnchorElement>()?;
        anchor.set_href(&url);
        anchor.set_download(name);
        document.body().unwrap().append_child(&anchor)?;
        anchor.click();
        anchor.remove();
        Ok(())
    })();
    // Retain the URL long enough for the browser to begin the download.
    let revoke = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 1000)?;
    result
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
                Box::new(|cc| Ok(Box::new(super::App::new(cc)))),
            )
            .await
        {
            Ok(()) => {
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
