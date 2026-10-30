use crate::{Completed, Job, Output, Work, execute};
use json_lens::{Document, Format, graph::Graph};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
    time::Duration,
};
use wasm_bindgen::prelude::*;
use web_time::Instant;

type CachedDocument = (u32, Arc<Document>, Arc<Graph>);
thread_local! {
    static TOKENS: RefCell<HashMap<usize, (Weak<Value>, u32)>> = RefCell::new(HashMap::new());
    static DOCUMENTS: RefCell<VecDeque<CachedDocument>> = const { RefCell::new(VecDeque::new()) };
    static NEXT_DOCUMENT: RefCell<u32> = const { RefCell::new(1) };
}

pub fn token(value: &Arc<Value>) -> Option<u32> {
    TOKENS.with(|tokens| {
        tokens
            .borrow()
            .get(&(Arc::as_ptr(value) as usize))
            .and_then(|(weak, id)| {
                weak.upgrade()
                    .filter(|original| Arc::ptr_eq(original, value))
                    .map(|_| *id)
            })
    })
}

fn register(value: &Arc<Value>, id: u32) {
    TOKENS.with(|tokens| {
        let mut tokens = tokens.borrow_mut();
        tokens.retain(|_, (weak, _)| weak.strong_count() > 0);
        tokens.insert(Arc::as_ptr(value) as usize, (Arc::downgrade(value), id));
    });
}

pub fn cached(id: u32) -> Option<(Arc<Document>, Arc<Graph>)> {
    DOCUMENTS.with(|documents| {
        documents
            .borrow()
            .iter()
            .find(|(key, _, _)| *key == id)
            .map(|(_, doc, graph)| (doc.clone(), graph.clone()))
    })
}

#[derive(Serialize, Deserialize)]
pub(crate) struct DocRef {
    id: Option<u32>,
    text: Option<Arc<String>>,
}
impl DocRef {
    pub(crate) fn new(doc: &Document, force: bool) -> Self {
        let id = token(&doc.value);
        Self {
            id,
            text: (force || id.is_none()).then(|| doc.text.clone()),
        }
    }
    pub(crate) fn resolve(self) -> Result<(Arc<Document>, Arc<Graph>), String> {
        if let Some(cached) = self.id.and_then(cached) {
            return Ok(cached);
        }
        let text = self.text.ok_or("cache_miss")?;
        let doc = Arc::new(json_lens::process(text, Format::Keep)?);
        let graph = Arc::new(json_lens::graph::build_graph(&doc.value));
        Ok((doc, graph))
    }
}

#[derive(Serialize, Deserialize)]
enum RemoteJob {
    Process(Arc<String>, Format),
    Transform(DocRef, Format),
    Locate(DocRef, String),
    SourcePointer(DocRef, usize),
    FocusGraph(DocRef, String),
    Inspect(DocRef, String),
    Edit(DocRef, String, String, bool),
    Import(Arc<String>, String),
    Compare(Arc<String>, Arc<String>),
    FormatRight(Arc<String>),
    LocateDiff(Arc<String>, Arc<String>, String),
}
impl RemoteJob {
    fn from_job(job: &Job, force: bool) -> Result<Self, String> {
        Ok(match job {
            Job::Process(text, format) => Self::Process(text.clone(), *format),
            Job::Transform(doc, format, _) => Self::Transform(DocRef::new(doc, force), *format),
            Job::Locate(doc, pointer) => Self::Locate(DocRef::new(doc, force), pointer.clone()),
            Job::SourcePointer(doc, offset) => {
                Self::SourcePointer(DocRef::new(doc, force), *offset)
            }
            Job::FocusGraph(doc, pointer) => {
                Self::FocusGraph(DocRef::new(doc, force), pointer.clone())
            }
            Job::Inspect(doc, pointer) => Self::Inspect(DocRef::new(doc, force), pointer.clone()),
            Job::Edit(doc, pointer, replacement, rename) => Self::Edit(
                DocRef::new(doc, force),
                pointer.clone(),
                replacement.clone(),
                *rename,
            ),
            Job::Import(text, name) => Self::Import(text.clone(), name.clone()),
            Job::Compare(left, right) => Self::Compare(left.clone(), right.clone()),
            Job::FormatRight(text) => Self::FormatRight(text.clone()),
            Job::LocateDiff(left, right, pointer) => {
                Self::LocateDiff(left.clone(), right.clone(), pointer.clone())
            }
            Job::RequestFormat(_) => return Err("Unresolved format request".into()),
        })
    }
    fn into_job(self) -> Result<Job, String> {
        Ok(match self {
            Self::Process(text, format) => Job::Process(text, format),
            Self::Transform(doc, format) => {
                let (doc, graph) = doc.resolve()?;
                Job::Transform(doc, format, graph)
            }
            Self::Locate(doc, pointer) => Job::Locate(doc.resolve()?.0, pointer),
            Self::SourcePointer(doc, offset) => Job::SourcePointer(doc.resolve()?.0, offset),
            Self::FocusGraph(doc, pointer) => Job::FocusGraph(doc.resolve()?.0, pointer),
            Self::Inspect(doc, pointer) => Job::Inspect(doc.resolve()?.0, pointer),
            Self::Edit(doc, pointer, replacement, rename) => {
                Job::Edit(doc.resolve()?.0, pointer, replacement, rename)
            }
            Self::Import(text, name) => Job::Import(text, name),
            Self::Compare(left, right) => Job::Compare(left, right),
            Self::FormatRight(text) => Job::FormatRight(text),
            Self::LocateDiff(left, right, pointer) => Job::LocateDiff(left, right, pointer),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Request {
    job: RemoteJob,
    revision: u64,
    user_action: bool,
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Document {
        id: u32,
        text: Option<Arc<String>>,
        graph: Option<Arc<Graph>>,
    },
    Invalid(Option<Arc<String>>, String),
    InvalidComparison(
        Option<crate::jobs::SyntaxError>,
        Option<crate::jobs::SyntaxError>,
    ),
    Comparison(
        Vec<json_lens::Difference>,
        Arc<json_lens::alignment::Alignment>,
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
    ),
    RightText(Arc<String>),
    DiffLocation(
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
    ),
    Located(std::ops::Range<usize>),
    SourcePointer(Option<String>),
    FocusedGraph(Arc<Graph>, String),
    Editing(crate::ui::EditDialog),
}
#[derive(Serialize, Deserialize)]
struct Response {
    result: Result<Reply, String>,
    elapsed_ms: f64,
    target: Option<String>,
}

fn source(job: &Job) -> Option<Arc<String>> {
    match job {
        Job::Process(text, _) | Job::Import(text, _) | Job::FormatRight(text) => Some(text.clone()),
        Job::Transform(doc, _, _)
        | Job::Locate(doc, _)
        | Job::SourcePointer(doc, _)
        | Job::FocusGraph(doc, _)
        | Job::Inspect(doc, _)
        | Job::Edit(doc, ..) => Some(doc.text.clone()),
        _ => None,
    }
}

fn process_request(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let request: Request = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let job = request.job.into_job()?;
    let original = source(&job);
    let original_graph = match &job {
        Job::Transform(_, _, graph) => Some(graph.clone()),
        _ => None,
    };
    let target = match &job {
        Job::Edit(_, pointer, key, true) => pointer
            .rsplit_once('/')
            .map(|(parent, _)| json_lens::pointer_child(parent, key)),
        _ => None,
    };
    log::debug!(target: "json_lens", "event=work_metadata revision={} user_action={} backend=web_worker", request.revision, request.user_action);
    let start = Instant::now();
    let result = execute(job).map(|output| match output {
        Output::Document(doc, graph) => {
            let text = if original
                .as_ref()
                .is_some_and(|text| Arc::ptr_eq(text, &doc.text))
            {
                None
            } else {
                Some(doc.text.clone())
            };
            let send_graph = if original_graph
                .as_ref()
                .is_some_and(|original| Arc::ptr_eq(original, &graph))
            {
                None
            } else {
                Some(graph.clone())
            };
            let id = NEXT_DOCUMENT.with(|next| {
                let id = *next.borrow();
                *next.borrow_mut() = id.wrapping_add(1);
                id
            });
            DOCUMENTS.with(|documents| {
                let mut documents = documents.borrow_mut();
                documents.push_back((id, Arc::new(doc), graph));
                while documents.len() > 4 {
                    documents.pop_front();
                }
            });
            Reply::Document {
                id,
                text,
                graph: send_graph,
            }
        }
        Output::Invalid(text, error) => Reply::Invalid(
            if original
                .as_ref()
                .is_some_and(|original| Arc::ptr_eq(original, &text))
            {
                None
            } else {
                Some(text)
            },
            error,
        ),
        Output::Comparison(diffs, alignment, left, right) => {
            Reply::Comparison(diffs, alignment, left, right)
        }
        Output::InvalidComparison(left, right) => Reply::InvalidComparison(left, right),
        Output::RightText(text) => Reply::RightText(text),
        Output::DiffLocation(left, right) => Reply::DiffLocation(left, right),
        Output::Located(range) => Reply::Located(range),
        Output::SourcePointer(pointer) => Reply::SourcePointer(pointer),
        Output::FocusedGraph(graph, pointer) => Reply::FocusedGraph(graph, pointer),
        Output::Editing(dialog) => Reply::Editing(dialog),
    });
    let response = Response {
        result,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        target,
    };
    let _stage = json_lens::diagnostics::Stage::new("worker_result_encode", 0);
    serde_json::to_vec(&response).map_err(|error| error.to_string())
}

impl crate::worker::Task<Completed> for Work {
    const KIND: &'static str = "work";
    fn encode(&self, force: bool) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&Request {
            job: RemoteJob::from_job(&self.job, force)?,
            revision: self.revision,
            user_action: self.user_action,
        })
        .map_err(|error| error.to_string())
    }
    fn decode(self, bytes: &[u8]) -> Completed {
        let response = serde_json::from_slice::<Response>(bytes).map_err(|error| error.to_string());
        let response = match response {
            Ok(response) => response,
            Err(error) => return self.failed(error),
        };
        let result = response.result.and_then(|reply| {
            Ok(match reply {
                Reply::Document { id, text, graph } => {
                    let text = text
                        .or_else(|| source(&self.job))
                        .ok_or("Worker omitted source")?;
                    let graph = graph
                        .or_else(|| match &self.job {
                            Job::Transform(_, _, graph) => Some(graph.clone()),
                            _ => None,
                        })
                        .ok_or("Worker omitted graph")?;
                    // The full value tree stays in the Worker. UI actions reference it by token.
                    let value = Arc::new(Value::Null);
                    register(&value, id);
                    Output::Document(Document { text, value }, graph)
                }
                Reply::Invalid(text, error) => Output::Invalid(
                    text.or_else(|| source(&self.job))
                        .ok_or("Worker omitted source")?,
                    error,
                ),
                Reply::Comparison(diffs, mut alignment, left, right) => {
                    if let Job::Compare(a, b) = &self.job {
                        let alignment = Arc::make_mut(&mut alignment);
                        alignment.left.source = (**a).clone();
                        alignment.right.source = (**b).clone();
                    }
                    Output::Comparison(diffs, alignment, left, right)
                }
                Reply::InvalidComparison(left, right) => Output::InvalidComparison(left, right),
                Reply::RightText(text) => Output::RightText(text),
                Reply::DiffLocation(left, right) => Output::DiffLocation(left, right),
                Reply::Located(range) => Output::Located(range),
                Reply::SourcePointer(pointer) => Output::SourcePointer(pointer),
                Reply::FocusedGraph(graph, pointer) => Output::FocusedGraph(graph, pointer),
                Reply::Editing(dialog) => Output::Editing(dialog),
            })
        });
        Completed {
            target: response.target,
            revision: self.revision,
            reset_view: self.reset_view,
            result,
            elapsed: Duration::from_secs_f64(response.elapsed_ms.max(0.0) / 1000.0),
            label: self.label,
            user_action: self.user_action,
        }
    }
    fn failed(self, error: String) -> Completed {
        Completed {
            target: None,
            revision: self.revision,
            reset_view: self.reset_view,
            result: Err(error),
            elapsed: Duration::ZERO,
            label: self.label,
            user_action: self.user_action,
        }
    }
}

#[wasm_bindgen]
pub fn worker_configure(debug: bool) {
    json_lens::diagnostics::configure(debug);
}

#[wasm_bindgen]
pub fn worker_dispatch(kind: &str, bytes: &[u8], id: usize) -> Result<Vec<u8>, JsValue> {
    if web_sys::window().is_some() {
        return Err(JsValue::from_str("Processing must run in a Web Worker"));
    }
    json_lens::diagnostics::in_job(id, || {
        let _stage = json_lens::diagnostics::Stage::new("worker_job", bytes.len());
        match kind {
            "work" => process_request(bytes),
            "search" => crate::find::process_remote(bytes),
            _ => Err("Unknown Worker task".into()),
        }
        .map_err(|error| JsValue::from_str(&error))
    })
}
