use super::*;

pub(crate) enum Job {
    RequestFormat(Format),
    Process(Arc<String>, Format),
    Transform(Arc<Document>, Format, Arc<forma::graph::Graph>),
    Locate(Arc<Document>, String),
    SourcePointer(Arc<Document>, usize),
    RightSourcePointer(Arc<String>, usize),
    FocusGraph(Arc<Document>, String),
    Inspect(Arc<Document>, String),
    Edit(Arc<Document>, String, String, bool),
    #[cfg(not(target_arch = "wasm32"))]
    Open(PathBuf),
    Import(Arc<String>, String),
    Compare(Arc<String>, Arc<String>),
    CompareOriginal(Arc<String>, Arc<String>),
    CompareWithOptions(Arc<String>, Arc<String>, forma::review::Options),
    FormatRight(Arc<String>),
    SortBoth(Arc<String>, Arc<String>, Format),
    LocateDiff(Arc<String>, Arc<String>, Difference),
}

impl Job {
    pub(crate) fn label(&self) -> &'static str {
        let format = match self {
            Job::RequestFormat(format)
            | Job::Process(_, format)
            | Job::Transform(_, format, _)
            | Job::SortBoth(_, _, format) => Some(format),
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
            Job::Compare(..) | Job::CompareOriginal(..) | Job::CompareWithOptions(..) => {
                "Compare JSON"
            }
            Job::FormatRight(..) => "Format right JSON",
            Job::LocateDiff(..) => "Locate difference",
            Job::Locate(..) => "Locate source",
            Job::SourcePointer(..) | Job::RightSourcePointer(..) => "Locate JSON path",
            Job::FocusGraph(..) => "Focus graph",
            Job::Inspect(..) => "Inspect node",
            Job::Edit(_, _, _, true) => "Rename property",
            Job::Edit(..) => "Edit node",
            _ => "Process JSON",
        }
    }
}
pub(crate) enum Output {
    Document(Document, Arc<forma::graph::Graph>),
    SortedBoth(Document, Arc<forma::graph::Graph>, Arc<String>),
    Invalid(Arc<String>, String),
    InvalidComparison(Option<SyntaxError>, Option<SyntaxError>),
    Comparison(
        Vec<Difference>,
        Arc<forma::alignment::Alignment>,
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
        Option<Arc<String>>,
    ),
    RightText(Arc<String>),
    DiffLocation(
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
    ),
    Located(std::ops::Range<usize>),
    SourcePointer(usize, Option<String>),
    RightSourcePointer(usize, Option<String>),
    FocusedGraph(Arc<forma::graph::Graph>, String),
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
    left: &Arc<String>,
    right: &Arc<String>,
    difference: &Difference,
) -> (
    Option<std::ops::Range<usize>>,
    Option<std::ops::Range<usize>>,
) {
    if difference.left_range.is_some() || difference.right_range.is_some() {
        return (
            difference.left_range.clone(),
            difference.right_range.clone(),
        );
    }
    let range = |source: &Arc<String>, pointer: &Option<String>| {
        pointer.as_deref().and_then(|pointer| {
            forma::source_index::cached(source)
                .ok()?
                .source_range(pointer)
                .ok()
        })
    };
    (
        range(left, &difference.left_pointer),
        range(right, &difference.right_pointer),
    )
}

pub(crate) fn document_output(doc: Document) -> Output {
    let graph = Arc::new(forma::graph::build_graph(&doc.value));
    Output::Document(doc, graph)
}

fn execute_review(
    left: Arc<String>,
    right: Arc<String>,
    options: &forma::review::Options,
    normalize: bool,
) -> Result<Output, String> {
    let (a, b) = match (forma::parse_json(&left), forma::parse_json(&right)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            return Ok(Output::InvalidComparison(
                a.err().map(|message| SyntaxError::new(&message, &left)),
                b.err().map(|message| SyntaxError::new(&message, &right)),
            ));
        }
    };
    let report = forma::review::build(left, right, &a, b, options, normalize, 1000)?;
    let first = report.differences.first();
    let ranges = first
        .map(|d| (d.left_range.clone(), d.right_range.clone()))
        .unwrap_or_default();
    Ok(Output::Comparison(
        report.differences,
        Arc::new(report.alignment),
        ranges.0,
        ranges.1,
        report.reordered,
    ))
}

pub(crate) fn execute(job: Job) -> Result<Output, String> {
    log::debug!(target: "forma", "event=job_start operation={:?}", job.label());
    let _stage = forma::diagnostics::Stage::new("execute_job", 0);
    let keep_order = matches!(&job, Job::CompareOriginal(..));
    let result = (|| match job {
        Job::RequestFormat(_) => Err("Unresolved format request".into()),
        Job::Transform(doc, format, graph) => {
            let formatted = format_document(&doc, format)?;
            if Arc::ptr_eq(&doc.value, &formatted.value) {
                log::debug!(target: "forma", "event=graph_cache reused=true");
                Ok(Output::Document(formatted, graph))
            } else {
                log::debug!(target: "forma", "event=graph_cache reused=false");
                Ok(document_output(formatted))
            }
        }
        Job::Locate(doc, pointer) => forma::source_index::cached(&doc.text)?
            .source_range(&pointer)
            .map(Output::Located),
        Job::SourcePointer(doc, offset) => {
            let pointer = forma::source_index::cached(&doc.text)?.pointer_at_offset(offset)?;
            forma::diagnostics::navigation(format_args!(
                "event=source_pointer_resolved side=left offset={} pointer={:?}",
                offset, pointer
            ));
            Ok(Output::SourcePointer(offset, pointer))
        }
        Job::RightSourcePointer(text, offset) => {
            let pointer = forma::source_index::cached(&text)?.pointer_at_offset(offset)?;
            forma::diagnostics::navigation(format_args!(
                "event=source_pointer_resolved side=right offset={} pointer={:?}",
                offset, pointer
            ));
            Ok(Output::RightSourcePointer(offset, pointer))
        }
        Job::FocusGraph(doc, pointer) => forma::graph::build_graph_at(&doc.value, &pointer)
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
            forma::edit::rename_key(&doc, &pointer, &replacement)
        } else {
            forma::edit::replace_value(&doc, &pointer, &replacement)
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
        Job::Compare(left, right) | Job::CompareOriginal(left, right) => {
            execute_review(left, right, &forma::review::Options::default(), !keep_order)
        }
        Job::CompareWithOptions(left, right, options) => {
            execute_review(left, right, &options, true)
        }
        Job::FormatRight(text) => {
            process(text, Format::Pretty).map(|doc| Output::RightText(doc.text))
        }
        Job::SortBoth(left, right, format) => {
            let left = process(left, format).map_err(|error| format!("Left JSON: {error}"))?;
            let right = process(right, format).map_err(|error| format!("Right JSON: {error}"))?;
            let graph = Arc::new(forma::graph::build_graph(&left.value));
            Ok(Output::SortedBoth(left, graph, right.text))
        }
        Job::LocateDiff(left, right, difference) => {
            let (a, b) = diff_ranges(&left, &right, &difference);
            Ok(Output::DiffLocation(a, b))
        }
    })();
    let outcome = match &result {
        Ok(Output::Invalid(..) | Output::InvalidComparison(..)) => "invalid_json",
        Ok(_) => "ok",
        Err(_) => "error",
    };
    log::debug!(target: "forma", "event=job_finish outcome={outcome}");
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
