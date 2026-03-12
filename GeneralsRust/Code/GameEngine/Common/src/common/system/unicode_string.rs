// FILE: unicode_string.rs /////////////////////////////////////////////////////
// Unicode string handling functionality
///////////////////////////////////////////////////////////////////////////////

pub type UnicodeString = String;

pub fn unicode_string_to_ascii(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).collect()
}

pub fn ascii_string_to_unicode(s: &str) -> String {
    s.to_string()
}
