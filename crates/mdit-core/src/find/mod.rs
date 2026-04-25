//! Plattformneutrale Find-Logik: liefert Byte-Offsets aller Vorkommen einer
//! Suchanfrage in einem `&str`. Die UI-Schicht übersetzt die Bytes in das
//! Range-Format ihres Toolkits (NSRange auf macOS / GPUI-Range später).
//!
//! Ersetzt die NSString-basierte Variante aus `src/app/helpers.rs`.

/// Eine einzelne Fundstelle, ausgedrückt in UTF-8-Byte-Offsets `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Findet alle (nicht-überlappenden) Vorkommen von `query` in `text`.
///
/// `case_insensitive` führt einen ASCII-Lowercase-Vergleich durch — für die
/// reine Markdown-Editing-Use-Case in mdit ausreichend; volle Unicode-Casefold
/// kann später ergänzt werden, falls gewünscht.
///
/// Leere Queries und leere Texte liefern eine leere Liste.
pub fn find_all_ranges(text: &str, query: &str, case_insensitive: bool) -> Vec<ByteRange> {
    if text.is_empty() || query.is_empty() {
        return Vec::new();
    }

    if case_insensitive {
        find_all_ascii_ci(text, query)
    } else {
        find_all_exact(text, query)
    }
}

fn find_all_exact(text: &str, query: &str) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < text.len() {
        match text[start..].find(query) {
            Some(rel) => {
                let abs_start = start + rel;
                let abs_end = abs_start + query.len();
                ranges.push(ByteRange { start: abs_start, end: abs_end });
                start = abs_end.max(abs_start + 1);
            }
            None => break,
        }
    }
    ranges
}

fn find_all_ascii_ci(text: &str, query: &str) -> Vec<ByteRange> {
    let lower_text = text.to_ascii_lowercase();
    let lower_query = query.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < lower_text.len() {
        match lower_text[start..].find(&lower_query) {
            Some(rel) => {
                let abs_start = start + rel;
                let abs_end = abs_start + lower_query.len();
                ranges.push(ByteRange { start: abs_start, end: abs_end });
                start = abs_end.max(abs_start + 1);
            }
            None => break,
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_yield_no_matches() {
        assert!(find_all_ranges("", "x", false).is_empty());
        assert!(find_all_ranges("hello", "", false).is_empty());
    }

    #[test]
    fn finds_multiple_non_overlapping_occurrences() {
        let r = find_all_ranges("ababab", "ab", false);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], ByteRange { start: 0, end: 2 });
        assert_eq!(r[1], ByteRange { start: 2, end: 4 });
        assert_eq!(r[2], ByteRange { start: 4, end: 6 });
    }

    #[test]
    fn case_insensitive_matches_mixed_case() {
        let r = find_all_ranges("Hello hello HELLO", "hello", true);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], ByteRange { start: 0, end: 5 });
    }

    #[test]
    fn case_sensitive_distinguishes_case() {
        let r = find_all_ranges("Hello hello", "hello", false);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], ByteRange { start: 6, end: 11 });
    }

    #[test]
    fn handles_query_longer_than_text() {
        let r = find_all_ranges("hi", "hello", false);
        assert!(r.is_empty());
    }

    #[test]
    fn no_overlapping_matches_for_repeated_chars() {
        let r = find_all_ranges("aaaa", "aa", false);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], ByteRange { start: 0, end: 2 });
        assert_eq!(r[1], ByteRange { start: 2, end: 4 });
    }

    #[test]
    fn unicode_text_byte_offsets() {
        let r = find_all_ranges("café x café", "café", false);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].start, 0);
        assert_eq!(r[0].end, 5); // "café" = 5 bytes (é = 2 bytes)
        // "café x café": "café" = 5 bytes (é = 0xC3 0xA9), then " x " = 3 bytes,
        // so the second match starts at byte 8.
        assert_eq!(r[1].start, 8);
        assert_eq!(r[1].end, 13);
    }
}
