use super::*;

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON / CSV", &["json", "csv", "txt"])
            .pick_file()
        {
            self.filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into();
            self.submit(Job::Open(path));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn save_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&self.filename)
            .save_file()
        {
            let text = self.source();
            self.submit(Job::Save(text, path));
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn open_file(&mut self) {
        self.files.open();
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn save_file(&mut self) {
        let text = self.source();
        self.files
            .save(Arc::new(text.as_bytes().to_vec()), self.filename.clone());
    }
}
