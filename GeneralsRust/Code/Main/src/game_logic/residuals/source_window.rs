//! Bounded source inspection without splitting a UTF-8 code point.
pub(crate) fn window<'a>(source: &'a str, marker: &str, max_bytes: usize) -> &'a str {
    let Some(start) = source.find(marker) else {
        return "";
    };
    at(source, start, max_bytes)
}

/// Inspect a byte offset obtained by a source search; invalid offsets fail closed.
pub(crate) fn at(source: &str, start: usize, max_bytes: usize) -> &str {
    if source.get(start..).is_none() {
        return "";
    }
    let mut end = source.len().min(start.saturating_add(max_bytes));
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    &source[start..end]
}
/// Keep only complete code points within a requested byte range.
pub(crate) fn range(source: &str, start: usize, end: usize) -> &str {
    let mut start = start.min(source.len());
    let mut end = end.min(source.len());
    while !source.is_char_boundary(start) {
        start += 1;
    }
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    if start > end { "" } else { &source[start..end] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_offsets_keep_last_match_and_reject_invalid_boundaries() {
        let source = "A first A–z";
        let last = source.rfind('A').unwrap();
        assert_eq!(at(source, last, 2), "A");
        assert_eq!(at(source, last, 4), "A–");
        assert_eq!(at(source, last, usize::MAX), "A–z");
        assert_eq!(at(source, last + 2, 10), "");
        assert_eq!(at(source, usize::MAX, 10), "");
        assert_eq!(at(source, source.len(), usize::MAX), "");
        assert_eq!(at("fn short", 0, 1400), "fn short");
    }

    #[test]
    fn context_ranges_round_inward_without_exceeding_the_budget() {
        assert_eq!(range("x–y", 2, 5), "y");
        assert_eq!(range("x–y", 0, 2), "x");
        assert_eq!(range("x–y", 2, 3), "");
        assert_eq!(range("x–y", 5, 1), "");
        assert_eq!(range("x–y", usize::MAX, usize::MAX), "");
        assert_eq!(range("x–y", 0, usize::MAX), "x–y");
    }

    #[test]
    fn every_boundary_in_mixed_width_source_stays_inside_requested_bytes() {
        let source = "Aé–🦀Z";
        for start in 0..=source.len() + 1 {
            for end in 0..=source.len() + 1 {
                let actual = range(source, start, end);
                assert!(actual.len() <= end.saturating_sub(start));
                if !actual.is_empty() {
                    let offset = actual.as_ptr() as usize - source.as_ptr() as usize;
                    assert!(offset >= start && offset + actual.len() <= end);
                }
            }
            for budget in 0..=source.len() + 1 {
                let actual = at(source, start, budget);
                assert!(actual.len() <= budget);
                if !actual.is_empty() {
                    assert_eq!(actual.as_ptr(), source[start..].as_ptr());
                }
            }
        }
    }

    #[test]
    fn unicode_windows_keep_the_byte_budget_and_marker() {
        assert_eq!(window("x A–z", "A", 2), "A");
        assert_eq!(window("x A–z", "A", 4), "A–");
        assert_eq!(window("x A–z", "A", usize::MAX), "A–z");
        assert_eq!(window("x A–z", "missing", 3), "");
        assert_eq!(window("x A–z", "A", 0), "");
    }
}
