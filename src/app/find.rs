use eframe::egui;
use json_lens::Document;
use json_lens::search::{TextMatches, TreeMatches, find_text, find_tree};
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use web_time::Instant;

enum Request {
    Text(u64, Arc<String>, String),
    Right(u64, Arc<String>, String),
    Tree(u64, Arc<Document>, String),
    #[cfg(target_arch = "wasm32")]
    Pointer(u64, Arc<Document>, String),
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
enum RemoteRequest {
    Text(u64, Arc<String>, String),
    Right(u64, Arc<String>, String),
    TextCached(u64, SourceRef, String),
    RightCached(u64, SourceRef, String),
    Tree(u64, crate::web_tasks::DocRef, String),
    Pointer(u64, crate::web_tasks::DocRef, String),
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SOURCE_KEYS: std::cell::RefCell<std::collections::HashMap<usize, (std::sync::Weak<String>, u64)>> = std::cell::RefCell::new(std::collections::HashMap::new());
    static NEXT_SOURCE: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    static SOURCES: std::cell::RefCell<std::collections::VecDeque<(u64, Arc<String>)>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
struct SourceRef {
    key: u64,
    text: Option<Arc<String>>,
}

#[cfg(target_arch = "wasm32")]
impl SourceRef {
    fn new(text: &Arc<String>, force: bool) -> Self {
        SOURCE_KEYS.with(|keys| {
            let mut keys = keys.borrow_mut();
            let address = Arc::as_ptr(text) as usize;
            if let Some((weak, key)) = keys.get(&address)
                && weak
                    .upgrade()
                    .is_some_and(|original| Arc::ptr_eq(&original, text))
            {
                return Self {
                    key: *key,
                    text: force.then(|| text.clone()),
                };
            }
            keys.retain(|_, (weak, _)| weak.strong_count() > 0);
            let key = NEXT_SOURCE.with(|next| {
                let key = next.get();
                next.set(key.checked_add(1).expect("search source IDs exhausted"));
                key
            });
            keys.insert(address, (Arc::downgrade(text), key));
            Self {
                key,
                text: Some(text.clone()),
            }
        })
    }

    fn resolve(self) -> std::result::Result<Arc<String>, String> {
        SOURCES.with(|sources| {
            let mut sources = sources.borrow_mut();
            if let Some(index) = sources.iter().position(|(key, _)| *key == self.key) {
                let cached = sources.remove(index).unwrap();
                let text = cached.1.clone();
                sources.push_back(cached);
                log::debug!(target: "json_lens", "event=search_source cache_hit=true bytes={}", text.len());
                return Ok(text);
            }
            let text = self.text.ok_or("cache_miss")?;
            log::debug!(target: "json_lens", "event=search_source cache_hit=false bytes={}", text.len());
            sources.push_back((self.key, text.clone()));
            while sources.len() > 1
                && (sources.len() > 4 || sources.iter().map(|(_, text)| text.len()).sum::<usize>() > 32 * 1024 * 1024)
            {
                sources.pop_front();
            }
            Ok(text)
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn process_remote(bytes: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let request: RemoteRequest =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let result = match request {
        RemoteRequest::Text(id, text, query) => Result::Text(id, find_text(&text, &query)),
        RemoteRequest::Right(id, text, query) => Result::Right(id, find_text(&text, &query)),
        RemoteRequest::TextCached(id, source, query) => {
            Result::Text(id, find_text(&source.resolve()?, &query))
        }
        RemoteRequest::RightCached(id, source, query) => {
            Result::Right(id, find_text(&source.resolve()?, &query))
        }
        RemoteRequest::Tree(id, reference, query) => {
            Result::Tree(id, find_tree(&reference.resolve()?.0.value, &query))
        }
        RemoteRequest::Pointer(id, reference, pointer) => {
            Result::Pointer(id, reference.resolve()?.0.value.pointer(&pointer).is_some())
        }
    };
    serde_json::to_vec(&result).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
impl crate::worker::Task<Result> for Request {
    const KIND: &'static str = "search";
    fn encode(&self, force: bool) -> std::result::Result<Vec<u8>, String> {
        let request = match self {
            Self::Text(id, text, query) => {
                RemoteRequest::TextCached(*id, SourceRef::new(text, force), query.clone())
            }
            Self::Right(id, text, query) => {
                RemoteRequest::RightCached(*id, SourceRef::new(text, force), query.clone())
            }
            Self::Tree(id, doc, query) => RemoteRequest::Tree(
                *id,
                crate::web_tasks::DocRef::new(doc, force),
                query.clone(),
            ),
            Self::Pointer(id, doc, pointer) => RemoteRequest::Pointer(
                *id,
                crate::web_tasks::DocRef::new(doc, force),
                pointer.clone(),
            ),
        };
        serde_json::to_vec(&request).map_err(|error| error.to_string())
    }
    fn decode(self, bytes: &[u8]) -> Result {
        match serde_json::from_slice(bytes) {
            Ok(result) => result,
            Err(error) => self.failed(error.to_string()),
        }
    }
    fn failed(self, error: String) -> Result {
        log::error!(target: "json_lens", "event=search_worker_failed error={error:?}");
        match self {
            Self::Text(id, ..) => Result::Text(id, TextMatches::default()),
            Self::Right(id, ..) => Result::Right(id, TextMatches::default()),
            Self::Tree(id, ..) => Result::Tree(id, TreeMatches::default()),
            Self::Pointer(id, ..) => Result::Pointer(id, false),
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
enum Result {
    Text(u64, TextMatches),
    Right(u64, TextMatches),
    Tree(u64, TreeMatches),
    #[cfg(target_arch = "wasm32")]
    Pointer(u64, bool),
}

pub struct FindState<T> {
    pub visible: bool,
    pub query: String,
    pub matches: T,
    pub selected: usize,
    pub pending: bool,
    pub focus: bool,
    generation: u64,
    due: Option<Instant>,
}

impl<T: Default> Default for FindState<T> {
    fn default() -> Self {
        Self {
            visible: false,
            query: String::new(),
            matches: T::default(),
            selected: 0,
            pending: false,
            focus: false,
            generation: 0,
            due: None,
        }
    }
}
impl<T: Default> FindState<T> {
    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }
    pub fn open(&mut self) {
        self.visible = true;
        self.focus = true;
    }
    pub fn close(&mut self) {
        self.visible = false;
        self.focus = false;
        self.query.clear();
        self.changed();
    }
    pub fn changed(&mut self) {
        self.generation += 1;
        self.matches = T::default();
        self.selected = 0;
        self.pending = !self.query.is_empty();
        self.due = self
            .pending
            .then(|| Instant::now() + Duration::from_millis(150));
    }
    pub fn next(&mut self, count: usize, backwards: bool) {
        if count > 0 {
            self.selected = if backwards {
                (self.selected + count - 1) % count
            } else {
                (self.selected + 1) % count
            };
        }
    }
}

pub struct Search {
    pub text: FindState<TextMatches>,
    pub tree: FindState<TreeMatches>,
    pub right: FindState<TextMatches>,
    pub right_jump: bool,
    pub jump: bool,
    sender: crate::worker::Sender<Request>,
    tree_sender: crate::worker::Sender<Request>,
    right_sender: crate::worker::Sender<Request>,
    receiver: mpsc::Receiver<Result>,
    #[cfg(target_arch = "wasm32")]
    pointer_sender: crate::worker::Sender<Request>,
    #[cfg(target_arch = "wasm32")]
    pub pointer_valid: bool,
    #[cfg(target_arch = "wasm32")]
    pub pointer_pending: bool,
    #[cfg(target_arch = "wasm32")]
    pub pointer_reset: bool,
    #[cfg(target_arch = "wasm32")]
    pointer_state: (u64, String, std::sync::Weak<Document>, bool),
}

impl Search {
    pub fn new(ctx: egui::Context) -> Self {
        let (results, receiver) = mpsc::channel();
        let mut senders = Vec::new();
        for _ in 0..if cfg!(target_arch = "wasm32") { 4 } else { 3 } {
            let sender =
                crate::worker::spawn(
                    ctx.clone(),
                    results.clone(),
                    true,
                    |request| match request {
                        Request::Text(id, text, query) => {
                            Result::Text(id, find_text(&text, &query))
                        }
                        Request::Right(id, text, query) => {
                            Result::Right(id, find_text(&text, &query))
                        }
                        Request::Tree(id, value, query) => {
                            Result::Tree(id, find_tree(&value.value, &query))
                        }
                        #[cfg(target_arch = "wasm32")]
                        Request::Pointer(id, doc, pointer) => {
                            Result::Pointer(id, doc.value.pointer(&pointer).is_some())
                        }
                    },
                );
            senders.push(sender);
        }
        #[cfg(target_arch = "wasm32")]
        let pointer_sender = senders.pop().unwrap();
        let right_sender = senders.pop().unwrap();
        let tree_sender = senders.pop().unwrap();
        let sender = senders.pop().unwrap();
        Self {
            text: FindState::default(),
            tree: FindState::default(),
            right: FindState::default(),
            right_jump: false,
            jump: false,
            sender,
            tree_sender,
            right_sender,
            receiver,
            #[cfg(target_arch = "wasm32")]
            pointer_sender,
            #[cfg(target_arch = "wasm32")]
            pointer_valid: true,
            #[cfg(target_arch = "wasm32")]
            pointer_pending: false,
            #[cfg(target_arch = "wasm32")]
            pointer_reset: false,
            #[cfg(target_arch = "wasm32")]
            pointer_state: (0, String::new(), std::sync::Weak::new(), false),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn check_pointer(&mut self, doc: Arc<Document>, pointer: &str) {
        let same_document = self
            .pointer_state
            .2
            .upgrade()
            .is_some_and(|old| Arc::ptr_eq(&old, &doc));
        if self.pointer_state.1 == pointer
            && self
                .pointer_state
                .2
                .upgrade()
                .is_some_and(|old| Arc::ptr_eq(&old, &doc))
        {
            return;
        }
        self.pointer_state.0 += 1;
        self.pointer_state.1 = pointer.into();
        self.pointer_state.2 = Arc::downgrade(&doc);
        self.pointer_state.3 = !same_document;
        self.pointer_reset = false;
        self.pointer_valid = true;
        self.pointer_pending = true;
        let _ =
            self.pointer_sender
                .send(Request::Pointer(self.pointer_state.0, doc, pointer.into()));
    }

    pub fn tick(
        &mut self,
        ctx: &egui::Context,
        text: Arc<String>,
        value: Option<Arc<Document>>,
        right: Arc<String>,
    ) {
        let now = Instant::now();
        if self.right.due.is_some_and(|due| due <= now) {
            self.right.due = None;
            let _ = self.right_sender.send(Request::Right(
                self.right.generation,
                right,
                self.right.query.clone(),
            ));
        }
        if self.text.due.is_some_and(|due| due <= now) {
            self.text.due = None;
            let _ = self.sender.send(Request::Text(
                self.text.generation,
                text,
                self.text.query.clone(),
            ));
        }
        if self.tree.due.is_some_and(|due| due <= now) {
            self.tree.due = None;
            if let Some(value) = value {
                let _ = self.tree_sender.send(Request::Tree(
                    self.tree.generation,
                    value,
                    self.tree.query.clone(),
                ));
            } else {
                self.tree.pending = false;
            }
        }
        while let Ok(result) = self.receiver.try_recv() {
            match result {
                Result::Text(id, matches) if id == self.text.generation => {
                    self.text.matches = matches;
                    self.text.pending = false;
                    self.jump = true;
                }
                Result::Tree(id, matches) if id == self.tree.generation => {
                    self.tree.matches = matches;
                    self.tree.pending = false;
                }
                Result::Right(id, matches) if id == self.right.generation => {
                    self.right.matches = matches;
                    self.right.pending = false;
                    self.right_jump = true;
                }
                #[cfg(target_arch = "wasm32")]
                Result::Pointer(id, valid) if id == self.pointer_state.0 => {
                    self.pointer_valid = valid;
                    self.pointer_pending = false;
                    self.pointer_reset = !valid && self.pointer_state.3;
                }
                _ => {}
            }
        }
        if self.text.due.is_some() || self.tree.due.is_some() || self.right.due.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
