//! `cMiscUtil` — GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/miscutil.{h,cpp}
//!
//! Intentional deviations from C++:
//! - `get_text_time` returns an owned `String` (C++ returned a static `ctime` buffer).
//! - `file_exists` / `file_is_read_only` / `remove_file` use `std::fs` directly.
//!   C++ `File_Exists` went through `_TheFileFactory`; callers that need BIG-archive
//!   overlays should query the factory themselves.
//! - `get_file_id_string` uses file size + modified unix time when a PE TimeDateStamp
//!   is unavailable (C++ always read `IMAGE_FILE_HEADER.TimeDateStamp`).

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Namespace mirroring C++ `cMiscUtil` static methods.
pub struct MiscUtil;

impl MiscUtil {
    /// `cMiscUtil::Get_Text_Time`
    ///
    /// C++ returned `ctime(&time(NULL))` with the trailing newline stripped (locale
    /// dependent). Rust returns the same shape in UTC: `Www Mmm dd hh:mm:ss yyyy`.
    pub fn get_text_time() -> String {
        use chrono::{Utc, Datelike, Timelike};
        let now = Utc::now();
        // ctime pad: day-of-month is space-padded width 2 (" 5" vs "05").
        let day = now.day();
        let day_s = if day < 10 {
            format!(" {day}")
        } else {
            format!("{day}")
        };
        format!(
            "{} {} {} {:02}:{:02}:{:02} {}",
            now.format("%a"),
            now.format("%b"),
            day_s,
            now.hour(),
            now.minute(),
            now.second(),
            now.year(),
        )
    }

    /// `cMiscUtil::Seconds_To_Hms`
    pub fn seconds_to_hms(seconds: f32) -> (i32, i32, i32) {
        debug_assert!(seconds >= 0.0);
        let mut seconds = seconds;
        let h = (seconds / 3600.0) as i32;
        seconds -= h as f32 * 3600.0;
        let m = (seconds / 60.0) as i32;
        seconds -= m as f32 * 60.0;
        let s = seconds as i32;
        debug_assert!(h >= 0);
        debug_assert!((0..60).contains(&m));
        debug_assert!((0..60).contains(&s));
        (h, m, s)
    }

    /// `cMiscUtil::Is_String_Same` — case-insensitive.
    pub fn is_string_same(a: &str, b: &str) -> bool {
        a.eq_ignore_ascii_case(b)
    }

    /// `cMiscUtil::Is_String_Different`
    pub fn is_string_different(a: &str, b: &str) -> bool {
        !Self::is_string_same(a, b)
    }

    /// `cMiscUtil::File_Exists`
    pub fn file_exists(filename: &str) -> bool {
        Path::new(filename).is_file()
    }

    /// `cMiscUtil::File_Is_Read_Only`
    pub fn file_is_read_only(filename: &str) -> bool {
        let Ok(meta) = fs::metadata(filename) else {
            return false;
        };
        meta.permissions().readonly()
    }

    /// `cMiscUtil::Is_Alphabetic`
    pub fn is_alphabetic(c: char) -> bool {
        c.is_ascii_alphabetic()
    }

    /// `cMiscUtil::Is_Numeric`
    pub fn is_numeric(c: char) -> bool {
        c.is_ascii_digit()
    }

    /// `cMiscUtil::Is_Alphanumeric`
    pub fn is_alphanumeric(c: char) -> bool {
        Self::is_alphabetic(c) || Self::is_numeric(c)
    }

    /// `cMiscUtil::Is_Whitespace` — space or tab only (matches C++).
    pub fn is_whitespace(c: char) -> bool {
        c == ' ' || c == '\t'
    }

    /// `cMiscUtil::Trim_Trailing_Whitespace`
    pub fn trim_trailing_whitespace(text: &mut String) {
        while text
            .chars()
            .last()
            .is_some_and(Self::is_whitespace)
        {
            text.pop();
        }
    }

    /// `cMiscUtil::Get_File_Id_String`
    pub fn get_file_id_string(filename: &str) -> String {
        let path = Path::new(filename);
        let filesize = fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        let time_date_stamp = fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut working = filename.replace('/', "\\").to_ascii_uppercase();
        if let Some(idx) = working.rfind('\\') {
            working = working[idx + 1..].to_string();
        }
        format!("{working} {filesize} {time_date_stamp}")
    }

    /// `cMiscUtil::Remove_File`
    pub fn remove_file(filename: &str) -> std::io::Result<()> {
        fs::remove_file(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_to_hms_basic() {
        assert_eq!(MiscUtil::seconds_to_hms(3661.0), (1, 1, 1));
        assert_eq!(MiscUtil::seconds_to_hms(59.9), (0, 0, 59));
    }

    #[test]
    fn string_compare_case_insensitive() {
        assert!(MiscUtil::is_string_same("AbC", "abc"));
        assert!(MiscUtil::is_string_different("a", "b"));
    }

    #[test]
    fn char_classifiers() {
        assert!(MiscUtil::is_alphabetic('Z'));
        assert!(MiscUtil::is_numeric('5'));
        assert!(MiscUtil::is_alphanumeric('a'));
        assert!(MiscUtil::is_whitespace('\t'));
        assert!(!MiscUtil::is_whitespace('\n'));
    }

    #[test]
    fn trim_trailing() {
        let mut s = String::from("hi \t");
        MiscUtil::trim_trailing_whitespace(&mut s);
        assert_eq!(s, "hi");
    }
}
