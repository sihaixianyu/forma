//! Syntax diagnostics reuse the parser's error location without reparsing on the UI thread.
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
