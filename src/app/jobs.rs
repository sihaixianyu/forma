use super::*;

pub(crate) enum Job {
    RequestFormat(Format),
    Process(Arc<String>, Format),
    Transform(Arc<Document>, Format, Arc<json_lens::graph::Graph>),
    Locate(Arc<Document>, String),
    SourcePointer(Arc<Document>, usize),
    FocusGraph(Arc<Document>, String),
    Inspect(Arc<Document>, String),
    Edit(Arc<Document>, String, String, bool),
    #[cfg(not(target_arch = "wasm32"))]
    Open(PathBuf),
    Import(Arc<String>, String),
    Compare(Arc<String>, Arc<String>),
    FormatRight(Arc<String>),
    LocateDiff(Arc<String>, Arc<String>, String),
}

impl Job {
    pub(crate) fn label(&self) -> &'static str {
        let format = match self {
            Job::RequestFormat(format) | Job::Process(_, format) | Job::Transform(_, format, _) => {
                Some(format)
            }
            _ => None,
        };
        if let Some(format) = format {
            return match format {
                Format::Keep => "Validate",
                Format::Pretty => "Format",
                Format::Compact => "Minify",
                Format::SortAscending => "Sort ascending",
                Format::SortDescending => "Sort descending",
                Format::Nested => "Expand nested JSON",
            };
        }
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Job::Open(_) => "Open file",
            Job::Import(..) => "Open file",
            Job::Compare(..) => "Compare JSON",
            Job::FormatRight(..) => "Format right JSON",
            Job::LocateDiff(..) => "Locate difference",
            Job::Locate(..) => "Locate source",
            Job::SourcePointer(..) => "Locate graph node",
            Job::FocusGraph(..) => "Focus graph",
            Job::Inspect(..) => "Inspect node",
            Job::Edit(_, _, _, true) => "Rename property",
            Job::Edit(..) => "Edit node",
            _ => "Process JSON",
        }
    }
}
pub(crate) enum Output {
    Document(Document, Arc<json_lens::graph::Graph>),
    Invalid(Arc<String>, String),
    InvalidComparison(Option<SyntaxError>, Option<SyntaxError>),
    Comparison(
        Vec<Difference>,
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
    FocusedGraph(Arc<json_lens::graph::Graph>, String),
    Editing(ui::EditDialog),
}
pub(crate) struct Work {
    pub(crate) job: Job,
    pub(crate) revision: u64,
    pub(crate) reset_view: bool,
    pub(crate) label: &'static str,
    pub(crate) user_action: bool,
}

pub(crate) struct QueuedAction {
    pub(crate) job: Job,
    pub(crate) revision: u64,
}

pub(crate) struct Completed {
    pub(crate) target: Option<String>,
    pub(crate) revision: u64,
    pub(crate) reset_view: bool,
    pub(crate) result: Result<Output, String>,
    pub(crate) elapsed: Duration,
    pub(crate) label: &'static str,
    pub(crate) user_action: bool,
}

pub(crate) fn diff_ranges(
    left: &str,
    right: &str,
    pointer: &str,
) -> (
    Option<std::ops::Range<usize>>,
    Option<std::ops::Range<usize>>,
) {
    let pointer = if pointer == "/ (root)" { "" } else { pointer };
    (
        json_lens::edit::source_range(left, pointer).ok(),
        json_lens::edit::source_range(right, pointer).ok(),
    )
}

pub(crate) fn document_output(doc: Document) -> Output {
    let graph = Arc::new(json_lens::graph::build_graph(&doc.value));
    Output::Document(doc, graph)
}

pub(crate) fn execute(job: Job) -> Result<Output, String> {
    log::debug!(target: "json_lens", "event=job_start operation={:?}", job.label());
    let _stage = json_lens::diagnostics::Stage::new("execute_job", 0);
    let result = (|| match job {
        Job::RequestFormat(_) => Err("Unresolved format request".into()),
        Job::Transform(doc, format, graph) => {
            let formatted = format_document(&doc, format)?;
            if Arc::ptr_eq(&doc.value, &formatted.value) {
                log::debug!(target: "json_lens", "event=graph_cache reused=true");
                Ok(Output::Document(formatted, graph))
            } else {
                log::debug!(target: "json_lens", "event=graph_cache reused=false");
                Ok(document_output(formatted))
            }
        }
        Job::Locate(doc, pointer) => {
            json_lens::edit::source_range(&doc.text, &pointer).map(Output::Located)
        }
        Job::SourcePointer(doc, offset) => {
            json_lens::edit::pointer_at_offset(&doc.text, offset).map(Output::SourcePointer)
        }
        Job::FocusGraph(doc, pointer) => json_lens::graph::build_graph_at(&doc.value, &pointer)
            .map(|graph| Output::FocusedGraph(Arc::new(graph), pointer))
            .ok_or_else(|| "Pointer not found".into()),
        Job::Inspect(doc, pointer) => {
            let value = doc.value.pointer(&pointer).ok_or("Pointer not found")?;
            let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
            if json.len() > EDIT_LIMIT {
                return Err("Select a smaller node to edit (maximum 8 MiB)".into());
            }
            let key = pointer.rsplit_once('/').and_then(|(parent, key)| {
                doc.value
                    .pointer(parent)
                    .filter(|value| value.is_object())
                    .map(|_| key.replace("~1", "/").replace("~0", "~"))
            });
            Ok(Output::Editing(ui::EditDialog {
                pointer,
                json,
                key,
                rename: false,
                error: String::new(),
            }))
        }
        Job::Edit(doc, pointer, replacement, rename) => if rename {
            json_lens::edit::rename_key(&doc, &pointer, &replacement)
        } else {
            json_lens::edit::replace_value(&doc, &pointer, &replacement)
        }
        .map(document_output),
        Job::Process(text, format) => Ok(match process(text.clone(), format) {
            Ok(doc) => document_output(doc),
            Err(error) => Output::Invalid(text, error),
        }),
        #[cfg(not(target_arch = "wasm32"))]
        Job::Open(path) => {
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            execute(Job::Import(
                Arc::new(text),
                path.to_string_lossy().into_owned(),
            ))
        }
        Job::Import(text, name) => {
            let text = if name
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
            {
                Arc::new(
                    serde_json::to_string_pretty(&csv_to_json(&text)?)
                        .map_err(|e| e.to_string())?,
                )
            } else {
                text
            };
            Ok(match process(text.clone(), Format::Keep) {
                Ok(doc) => document_output(doc),
                Err(error) => Output::Invalid(text, error),
            })
        }
        Job::Compare(left, right) => {
            let a = json_lens::parse_json(&left);
            let b = json_lens::parse_json(&right);
            let (a, b) = match (a, b) {
                (Ok(a), Ok(b)) => (a, b),
                (a, b) => {
                    return Ok(Output::InvalidComparison(
                        a.err().map(|message| SyntaxError::new(&message, &left)),
                        b.err().map(|message| SyntaxError::new(&message, &right)),
                    ));
                }
            };
            let diffs = {
                let _stage =
                    json_lens::diagnostics::Stage::new("compare_walk", left.len() + right.len());
                compare(&a, &b, 1000)
            };
            log::debug!(target: "json_lens", "event=compare_result differences={}", diffs.len());
            let (a, b) = diffs
                .first()
                .map(|d| diff_ranges(&left, &right, &d.pointer))
                .unwrap_or_default();
            let alignment = {
                let _stage = json_lens::diagnostics::Stage::new(
                    "compare_alignment",
                    left.len() + right.len(),
                );
                Arc::new(json_lens::alignment::align(&left, &right))
            };
            Ok(Output::Comparison(diffs, alignment, a, b))
        }
        Job::FormatRight(text) => {
            process(text, Format::Pretty).map(|doc| Output::RightText(doc.text))
        }
        Job::LocateDiff(left, right, pointer) => {
            let (a, b) = diff_ranges(&left, &right, &pointer);
            Ok(Output::DiffLocation(a, b))
        }
    })();
    let outcome = match &result {
        Ok(Output::Invalid(..) | Output::InvalidComparison(..)) => "invalid_json",
        Ok(_) => "ok",
        Err(_) => "error",
    };
    log::debug!(target: "json_lens", "event=job_finish outcome={outcome}");
    result
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SyntaxError {
    pub(crate) message: String,
    pub(crate) location: Option<(usize, usize)>,
    pub(crate) offset: Option<usize>,
}

impl SyntaxError {
    pub(crate) fn new(message: &str, text: &str) -> Self {
        let location = message.rsplit_once(" at line ").and_then(|(_, position)| {
            let (line, column) = position.split_once(" column ")?;
            Some((line.parse::<usize>().ok()?, column.parse::<usize>().ok()?))
        });
        let offset = location.and_then(|(line, column)| {
            let start = if line == 1 {
                0
            } else {
                text.match_indices('\n')
                    .nth(line.checked_sub(2)?)
                    .map(|(at, _)| at + 1)?
            };
            let line_text = text[start..].split('\n').next()?;
            let mut byte = column.saturating_sub(1).min(line_text.len());
            while !line_text.is_char_boundary(byte) {
                byte -= 1;
            }
            Some(start + byte)
        });
        Self {
            message: message.into(),
            location,
            offset,
        }
    }

    pub(crate) fn summary(&self) -> String {
        match self.location {
            Some((line, column)) => format!("Invalid JSON · {line}:{column}"),
            None => "Invalid JSON".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_byte_columns_map_to_unicode_cursors_and_eof() {
        for text in ["{\n  \"中文🙂\": }", "{\n", "[true,]", ""] {
            let error = serde_json::from_str::<serde_json::Value>(text).unwrap_err();
            let diagnostic = SyntaxError::new(&error.to_string(), text);
            assert_eq!(diagnostic.location, Some((error.line(), error.column())));
            let offset = diagnostic.offset.unwrap();
            assert!(text.is_char_boundary(offset));
            if text.contains('}') {
                assert_eq!(text[offset..].chars().next(), Some('}'));
            }
        }
    }
}
