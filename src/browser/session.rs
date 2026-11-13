use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/browser/session.js")]
extern "C" {
    #[wasm_bindgen(js_name = loadSession)]
    pub async fn load();
    #[wasm_bindgen(js_name = restoredSession)]
    fn restored() -> Option<String>;
    #[wasm_bindgen(js_name = saveSession)]
    pub fn save(left: &str, right: &str, compare: bool);
    #[wasm_bindgen(js_name = sessionError)]
    pub fn error() -> Option<String>;
    #[wasm_bindgen(js_name = reportSessionError)]
    fn report_error(message: &str);
}

pub type Inputs = forma::share::Snapshot;

pub fn inputs() -> Option<Inputs> {
    if let Some(window) = web_sys::window() {
        match forma::share::from_fragment(&window.location().hash().unwrap_or_default()) {
            Ok(Some(snapshot)) => return Some(snapshot),
            Err(error) => report_error(&format!(
                "Could not open shared JSON: {error}. Local inputs retained."
            )),
            Ok(None) => {}
        }
    }
    restored().and_then(|text| serde_json::from_str(&text).ok())
}
