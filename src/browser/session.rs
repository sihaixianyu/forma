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
}

#[derive(serde::Deserialize)]
pub struct Inputs {
    pub left: String,
    pub right: String,
    pub compare: bool,
}

pub fn inputs() -> Option<Inputs> {
    restored().and_then(|text| serde_json::from_str(&text).ok())
}
