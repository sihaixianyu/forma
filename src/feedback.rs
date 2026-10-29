use std::time::Duration;
use web_time::Instant;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Working,
    Success,
    Info,
    Error,
}

pub struct Feedback {
    pub text: String,
    pub kind: Kind,
    pub created: Instant,
}

impl Feedback {
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind,
            created: Instant::now(),
        }
    }
    pub fn expired(&self) -> bool {
        self.kind != Kind::Working
            && self.created.elapsed()
                >= Duration::from_secs(if self.kind == Kind::Error { 7 } else { 4 })
    }
}
