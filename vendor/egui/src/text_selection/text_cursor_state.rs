//! Text cursor changes/interaction, without modifying the text.

use epaint::text::{Galley, cursor::CCursor};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{NumExt as _, Rect, Response, Ui, epaint};

use super::CCursorRange;

/// The state of a text cursor selection.
///
/// Used for [`crate::TextEdit`] and [`crate::Label`].
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct TextCursorState {
    ccursor_range: Option<CCursorRange>,
}

impl From<CCursorRange> for TextCursorState {
    fn from(ccursor_range: CCursorRange) -> Self {
        Self {
            ccursor_range: Some(ccursor_range),
        }
    }
}

impl TextCursorState {
    pub fn is_empty(&self) -> bool {
        self.ccursor_range.is_none()
    }

    /// The currently selected range of characters.
    pub fn char_range(&self) -> Option<CCursorRange> {
        self.ccursor_range
    }

    /// The currently selected range of characters, clamped within the character
    /// range of the given [`Galley`].
    pub fn range(&self, galley: &Galley) -> Option<CCursorRange> {
        self.ccursor_range.map(|mut range| {
            range.primary = galley.clamp_cursor(&range.primary);
            range.secondary = galley.clamp_cursor(&range.secondary);
            range
        })
    }

    /// Sets the currently selected range of characters.
    pub fn set_char_range(&mut self, ccursor_range: Option<CCursorRange>) {
        self.ccursor_range = ccursor_range;
    }
}

impl TextCursorState {
    /// Handle clicking and/or dragging text.
    ///
    /// Returns `true` if there was interaction.
    pub fn pointer_interaction(
        &mut self,
        ui: &Ui,
        response: &Response,
        cursor_at_pointer: CCursor,
        galley: &Galley,
        is_being_dragged: bool,
    ) -> bool {
        let text = galley.text();

        if response.double_clicked() {
            // Select word:
            let ccursor_range = select_word_at(text, cursor_at_pointer);
            self.set_char_range(Some(ccursor_range));
            true
        } else if response.triple_clicked() {
            // Select line:
            let ccursor_range = select_line_at(text, cursor_at_pointer);
            self.set_char_range(Some(ccursor_range));
            true
        } else if response.sense.senses_drag() {
            if response.hovered() && ui.input(|i| i.pointer.any_pressed()) {
                // The start of a drag (or a click).
                if ui.input(|i| i.modifiers.shift) {
                    if let Some(mut cursor_range) = self.range(galley) {
                        cursor_range.primary = cursor_at_pointer;
                        self.set_char_range(Some(cursor_range));
                    } else {
                        self.set_char_range(Some(CCursorRange::one(cursor_at_pointer)));
                    }
                } else {
                    self.set_char_range(Some(CCursorRange::one(cursor_at_pointer)));
                }
                true
            } else if is_being_dragged {
                // Drag to select text:
                if let Some(mut cursor_range) = self.range(galley) {
                    cursor_range.primary = cursor_at_pointer;
                    self.set_char_range(Some(cursor_range));
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

fn select_word_at(text: &str, ccursor: CCursor) -> CCursorRange {
    // Unicode word navigation reverses its input to find the previous boundary.
    // Keep that work local when selecting a word in a large document. Include
    // the surrounding separators so segmentation keeps its boundary context.
    let byte = byte_index_from_char_index(text, ccursor.index);
    if byte == text.len() && ccursor.index > text.chars().count() {
        return select_word_at_full_text(text, ccursor);
    }
    let is_separator = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '{' | '}' | '[' | ']');
    let start = text[..byte].rfind(is_separator).unwrap_or(0);
    let end = text[byte..]
        .find(is_separator)
        .map_or(text.len(), |offset| byte + offset + 1);
    let local = &text[start..end];
    let local_index = text[start..byte].chars().count();
    let offset = ccursor.index - local_index;
    let range = select_word_at_full_text(
        local,
        CCursor {
            index: local_index,
            ..ccursor
        },
    );
    let [min, max] = range.sorted_cursors();
    // Whitespace selection can cross separators. Preserve the original behavior
    // whenever the local result could depend on text outside this window.
    if (start > 0 && min.index == 0) || (end < text.len() && max.index == local.chars().count()) {
        return select_word_at_full_text(text, ccursor);
    }
    CCursorRange::two(
        CCursor {
            index: range.secondary.index + offset,
            ..range.secondary
        },
        CCursor {
            index: range.primary.index + offset,
            ..range.primary
        },
    )
}

fn select_word_at_full_text(text: &str, ccursor: CCursor) -> CCursorRange {
    if ccursor.index == 0 {
        CCursorRange::two(ccursor, ccursor_next_word(text, ccursor))
    } else {
        let it = text.chars();
        let mut it = it.skip(ccursor.index - 1);
        if let Some(char_before_cursor) = it.next() {
            if let Some(char_after_cursor) = it.next() {
                if is_word_char(char_before_cursor) && is_word_char(char_after_cursor) {
                    let min = ccursor_previous_word(text, ccursor + 1);
                    let max = ccursor_next_word(text, min);
                    CCursorRange::two(min, max)
                } else if is_word_char(char_before_cursor) {
                    let min = ccursor_previous_word(text, ccursor);
                    let max = ccursor_next_word(text, min);
                    CCursorRange::two(min, max)
                } else if is_word_char(char_after_cursor) {
                    let max = ccursor_next_word(text, ccursor);
                    CCursorRange::two(ccursor, max)
                } else {
                    let min = ccursor_previous_word(text, ccursor);
                    let max = ccursor_next_word(text, ccursor);
                    CCursorRange::two(min, max)
                }
            } else {
                let min = ccursor_previous_word(text, ccursor);
                CCursorRange::two(min, ccursor)
            }
        } else {
            let max = ccursor_next_word(text, ccursor);
            CCursorRange::two(ccursor, max)
        }
    }
}

fn select_line_at(text: &str, ccursor: CCursor) -> CCursorRange {
    if ccursor.index == 0 {
        CCursorRange::two(ccursor, ccursor_next_line(text, ccursor))
    } else {
        let it = text.chars();
        let mut it = it.skip(ccursor.index - 1);
        if let Some(char_before_cursor) = it.next() {
            if let Some(char_after_cursor) = it.next() {
                if (!is_linebreak(char_before_cursor)) && (!is_linebreak(char_after_cursor)) {
                    let min = ccursor_previous_line(text, ccursor + 1);
                    let max = ccursor_next_line(text, min);
                    CCursorRange::two(min, max)
                } else if !is_linebreak(char_before_cursor) {
                    let min = ccursor_previous_line(text, ccursor);
                    let max = ccursor_next_line(text, min);
                    CCursorRange::two(min, max)
                } else if !is_linebreak(char_after_cursor) {
                    let max = ccursor_next_line(text, ccursor);
                    CCursorRange::two(ccursor, max)
                } else {
                    let min = ccursor_previous_line(text, ccursor);
                    let max = ccursor_next_line(text, ccursor);
                    CCursorRange::two(min, max)
                }
            } else {
                let min = ccursor_previous_line(text, ccursor);
                CCursorRange::two(min, ccursor)
            }
        } else {
            let max = ccursor_next_line(text, ccursor);
            CCursorRange::two(ccursor, max)
        }
    }
}

pub fn ccursor_next_word(text: &str, ccursor: CCursor) -> CCursor {
    CCursor {
        index: next_word_boundary_char_index(text, ccursor.index),
        prefer_next_row: false,
    }
}

fn ccursor_next_line(text: &str, ccursor: CCursor) -> CCursor {
    CCursor {
        index: next_line_boundary_char_index(text.chars(), ccursor.index),
        prefer_next_row: false,
    }
}

pub fn ccursor_previous_word(text: &str, ccursor: CCursor) -> CCursor {
    let num_chars = text.chars().count();
    let reversed: String = text.graphemes(true).rev().collect();
    CCursor {
        index: num_chars
            - next_word_boundary_char_index(&reversed, num_chars - ccursor.index).min(num_chars),
        prefer_next_row: true,
    }
}

fn ccursor_previous_line(text: &str, ccursor: CCursor) -> CCursor {
    let num_chars = text.chars().count();
    CCursor {
        index: num_chars
            - next_line_boundary_char_index(text.chars().rev(), num_chars - ccursor.index),
        prefer_next_row: true,
    }
}

fn next_word_boundary_char_index(text: &str, cursor_ci: usize) -> usize {
    // Track character offsets once instead of rescanning the full prefix at every boundary.
    let mut word_ci = 0;
    for word in text.split_word_bounds() {
        // We consider `.` a word boundary.
        // At least that's how Mac works when navigating something like `www.example.com`.
        for (dot_ci_offset, chr) in word.chars().enumerate() {
            let dot_ci = word_ci + dot_ci_offset;
            if chr == '.' && cursor_ci < dot_ci {
                return dot_ci;
            }
        }

        // Splitting considers contiguous whitespace as one word, such words must be skipped,
        // this handles cases for example ' abc' (a space and a word), the cursor is at the beginning
        // (before space) - this jumps at the end of 'abc' (this is consistent with text editors
        // or browsers)
        if cursor_ci < word_ci && !all_word_chars(word) {
            return word_ci;
        }
        word_ci += word.chars().count();
    }

    word_ci
}

fn all_word_chars(text: &str) -> bool {
    text.chars().all(is_word_char)
}

fn next_line_boundary_char_index(it: impl Iterator<Item = char>, mut index: usize) -> usize {
    let mut it = it.skip(index);
    if let Some(_first) = it.next() {
        index += 1;

        if let Some(second) = it.next() {
            index += 1;
            for next in it {
                if is_linebreak(next) != is_linebreak(second) {
                    break;
                }
                index += 1;
            }
        }
    }
    index
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_linebreak(c: char) -> bool {
    c == '\r' || c == '\n'
}

/// Accepts and returns character offset (NOT byte offset!).
pub fn find_line_start(text: &str, current_index: CCursor) -> CCursor {
    // We know that new lines, '\n', are a single byte char, but we have to
    // work with char offsets because before the new line there may be any
    // number of multi byte chars.
    // We need to know the char index to be able to correctly set the cursor
    // later.
    let chars_count = text.chars().count();

    let position = text
        .chars()
        .rev()
        .skip(chars_count - current_index.index)
        .position(|x| x == '\n');

    match position {
        Some(pos) => CCursor::new(current_index.index - pos),
        None => CCursor::new(0),
    }
}

pub fn byte_index_from_char_index(s: &str, char_index: usize) -> usize {
    for (ci, (bi, _)) in s.char_indices().enumerate() {
        if ci == char_index {
            return bi;
        }
    }
    s.len()
}

pub fn char_index_from_byte_index(input: &str, byte_index: usize) -> usize {
    for (ci, (bi, _)) in input.char_indices().enumerate() {
        if bi == byte_index {
            return ci;
        }
    }

    input.char_indices().last().map_or(0, |(i, _)| i + 1)
}

pub fn slice_char_range(s: &str, char_range: std::ops::Range<usize>) -> &str {
    assert!(
        char_range.start <= char_range.end,
        "Invalid range, start must be less than end, but start = {}, end = {}",
        char_range.start,
        char_range.end
    );
    let start_byte = byte_index_from_char_index(s, char_range.start);
    let end_byte = byte_index_from_char_index(s, char_range.end);
    &s[start_byte..end_byte]
}

/// The thin rectangle of one end of the selection, e.g. the primary cursor, in local galley coordinates.
pub fn cursor_rect(galley: &Galley, cursor: &CCursor, row_height: f32) -> Rect {
    let mut cursor_pos = galley.pos_from_cursor(*cursor);

    // Handle completely empty galleys
    cursor_pos.max.y = cursor_pos.max.y.at_least(cursor_pos.min.y + row_height);

    cursor_pos = cursor_pos.expand(1.5); // slightly above/below row

    cursor_pos
}

#[cfg(test)]
mod test {
    use crate::text_selection::text_cursor_state::next_word_boundary_char_index;

    #[test]
    fn test_next_word_boundary_char_index() {
        // ASCII only
        let text = "abc d3f g_h i-j";
        assert_eq!(next_word_boundary_char_index(text, 1), 3);
        assert_eq!(next_word_boundary_char_index(text, 3), 7);
        assert_eq!(next_word_boundary_char_index(text, 9), 11);
        assert_eq!(next_word_boundary_char_index(text, 12), 13);
        assert_eq!(next_word_boundary_char_index(text, 13), 15);
        assert_eq!(next_word_boundary_char_index(text, 15), 15);

        assert_eq!(next_word_boundary_char_index("", 0), 0);
        assert_eq!(next_word_boundary_char_index("", 1), 0);

        // ASCII only
        let text = "abc.def.ghi";
        assert_eq!(next_word_boundary_char_index(text, 1), 3);
        assert_eq!(next_word_boundary_char_index(text, 3), 7);
        assert_eq!(next_word_boundary_char_index(text, 7), 11);

        // Unicode graphemes, some of which consist of multiple Unicode characters,
        // !!! Unicode character is not always what is tranditionally considered a character,
        // the values below are correct despite not seeming that way on the first look,
        // handling of and around emojis is kind of weird and is not consistent across
        // text editors and browsers
        let text = "❤️👍 skvělá knihovna 👍❤️";
        assert_eq!(next_word_boundary_char_index(text, 0), 2);
        assert_eq!(next_word_boundary_char_index(text, 2), 3); // this does not skip the space between thumbs-up and 'skvělá'
        assert_eq!(next_word_boundary_char_index(text, 6), 10);
        assert_eq!(next_word_boundary_char_index(text, 9), 10);
        assert_eq!(next_word_boundary_char_index(text, 12), 19);
        assert_eq!(next_word_boundary_char_index(text, 15), 19);
        assert_eq!(next_word_boundary_char_index(text, 19), 20);
        assert_eq!(next_word_boundary_char_index(text, 20), 21);
    }
}

#[cfg(test)]
mod forma_word_boundary_tests {
    use super::*;

    #[test]
    fn large_json_selection_keeps_global_unicode_offsets() {
        for separator in [",\n", ","] {
            let row = "{\"name\":\"中文 café work\",\"active\":true}";
            let mut text = vec![row; 25_000].join(separator);
            text.push_str(",\"last\":\"selected\"");
            let start = text[..text.rfind("selected").unwrap()].chars().count();
            let range = select_word_at(&text, CCursor::new(start + 3));
            assert_eq!(range.as_sorted_char_range(), start..start + 8);
            assert_eq!(range.slice_str(&text), "selected");
        }
    }

    #[test]
    fn local_selection_preserves_word_and_whitespace_ranges() {
        let cases = [
            "",
            "work",
            "  work  next \n\n last\t",
            "{\"work\":\"hello_world\",\"url\":\"www.example.com\"}",
            "[123,456,1.23,\"can't\",\"a:b\"]\r\nwork",
            "前缀 {\"中文\": \"café ❤️👍 👩‍💻\"} 后缀",
            "א\"ב [אב] ภาษาไทย 中文 e\u{301} \u{301}work",
            "🇨🇳🇬🇧 {a\u{301}} [\u{301}b] \r\n\tend",
        ];
        for text in cases {
            for index in 0..=text.chars().count() + 1 {
                let cursor = CCursor::new(index);
                assert_eq!(
                    select_word_at(text, cursor),
                    select_word_at_full_text(text, cursor),
                    "selection at {index} in {text:?}",
                );
            }
        }
    }

    #[test]
    #[ignore = "manual large-document selection benchmark"]
    fn large_document_word_selection_timings() {
        use std::{hint::black_box, time::Instant};
        for separator in [",\n", ","] {
            let row = "{\"name\":\"中文 work example\",\"active\":true}";
            let text = vec![row; 25_000].join(separator);
            for fraction in [0, 2, 4] {
                let from = byte_index_from_char_index(&text, text.chars().count() * fraction / 5);
                let byte = text[from..].find("work").unwrap() + from;
                let index = text[..byte].chars().count() + 2;
                let cursor = CCursor::new(index);
                assert_eq!(
                    select_word_at(&text, cursor),
                    select_word_at_full_text(&text, cursor)
                );
                for (label, select) in [
                    (
                        "before",
                        select_word_at_full_text as fn(&str, CCursor) -> CCursorRange,
                    ),
                    ("after", select_word_at),
                ] {
                    let start = Instant::now();
                    for _ in 0..10 {
                        black_box(select(black_box(&text), cursor));
                    }
                    eprintln!(
                        "{label} bytes={} position={fraction}/5 pretty={} ms={:.3}",
                        text.len(),
                        separator.contains('\n'),
                        start.elapsed().as_secs_f64() * 100.0
                    );
                }
            }
        }
    }

    #[test]
    fn incremental_offsets_preserve_unicode_word_boundaries() {
        let text = "{\"hello_world\": \"中文 café 👩‍💻 www.example.com\", \"value\": 123}\n";
        for cursor in 0..=text.chars().count() {
            let expected = text
                .split_word_bound_indices()
                .find_map(|(byte, word)| {
                    let start = char_index_from_byte_index(text, byte);
                    for (offset, ch) in word.chars().enumerate() {
                        if ch == '.' && cursor < start + offset {
                            return Some(start + offset);
                        }
                    }
                    (cursor < start && !all_word_chars(word)).then_some(start)
                })
                .unwrap_or_else(|| text.chars().count());
            assert_eq!(next_word_boundary_char_index(text, cursor), expected);
        }
    }
}
