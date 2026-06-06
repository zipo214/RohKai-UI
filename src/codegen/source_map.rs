use std::ops::{Range, RangeInclusive};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub bytes: Range<usize>,
    pub lines: RangeInclusive<usize>,
}

impl SourceSpan {
    pub fn new(bytes: Range<usize>, lines: RangeInclusive<usize>) -> Self {
        Self { bytes, lines }
    }

    pub fn extend_to(&mut self, byte_end: usize, line_end: usize) {
        self.bytes.end = self.bytes.end.max(byte_end);
        let line_start = *self.lines.start();
        let current_end = *self.lines.end();
        self.lines = line_start..=current_end.max(line_end);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetSourceSpan {
    pub widget_id: Uuid,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerSourceSpan {
    pub handler_name: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedCodeDocument {
    pub text: String,
    pub widget_spans: Vec<WidgetSourceSpan>,
    pub handler_spans: Vec<HandlerSourceSpan>,
}

pub fn line_byte_spans(text: &str) -> Vec<Range<usize>> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let end = start + line.len();
        spans.push(start..end);
        start = end;
    }
    if start < text.len() {
        spans.push(start..text.len());
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_spans_preserve_newlines_and_final_line() {
        let text = "one\ntwo\nthree";
        let spans = line_byte_spans(text);
        let lines: Vec<&str> = spans.iter().map(|span| &text[span.clone()]).collect();
        assert_eq!(lines, vec!["one\n", "two\n", "three"]);
    }
}
