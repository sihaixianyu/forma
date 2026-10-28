use eframe::egui;
use json_lens::search::{TextMatches, TreeMatches, find_text, find_tree};
use serde_json::Value;
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

enum Request {
    Text(u64, Arc<String>, String),
    Right(u64, Arc<String>, String),
    Tree(u64, Arc<Value>, String),
}
enum Result {
    Text(u64, TextMatches),
    Right(u64, TextMatches),
    Tree(u64, TreeMatches),
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
    sender: mpsc::Sender<Request>,
    tree_sender: mpsc::Sender<Request>,
    right_sender: mpsc::Sender<Request>,
    receiver: mpsc::Receiver<Result>,
}

impl Search {
    pub fn new(ctx: egui::Context) -> Self {
        let (results, receiver) = mpsc::channel();
        let mut senders = Vec::new();
        for _ in 0..3 {
            let (sender, requests) = mpsc::channel();
            let results = results.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Ok(mut request) = requests.recv() {
                    while let Ok(newer) = requests.try_recv() {
                        request = newer;
                    }
                    let result = match request {
                        Request::Text(id, text, query) => {
                            Result::Text(id, find_text(&text, &query))
                        }
                        Request::Right(id, text, query) => {
                            Result::Right(id, find_text(&text, &query))
                        }
                        Request::Tree(id, value, query) => {
                            Result::Tree(id, find_tree(&value, &query))
                        }
                    };
                    if results.send(result).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            senders.push(sender);
        }
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
        }
    }

    pub fn tick(
        &mut self,
        ctx: &egui::Context,
        text: Arc<String>,
        value: Option<Arc<Value>>,
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
                _ => {}
            }
        }
        if self.text.due.is_some() || self.tree.due.is_some() || self.right.due.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
