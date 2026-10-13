//! Bounded source inspection without splitting a UTF-8 code point.
pub(crate) fn window<'a>(source: &'a str, marker: &str, max_bytes: usize) -> &'a str {
    let Some(start) = source.find(marker) else {
        return "";
    };
    let mut end = source.len().min(start.saturating_add(max_bytes));
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    &source[start..end]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_windows_keep_the_byte_budget_and_marker() {
        assert_eq!(window("x A–z", "A", 2), "A");
        assert_eq!(window("x A–z", "A", 4), "A–");
        assert_eq!(window("x A–z", "A", usize::MAX), "A–z");
        assert_eq!(window("x A–z", "missing", 3), "");
        assert_eq!(window("x A–z", "A", 0), "");
    }
}
