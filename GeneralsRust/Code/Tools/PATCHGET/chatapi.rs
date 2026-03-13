//! Chat API Module
//!
//! Corresponds to C++ file: Tools/PATCHGET/CHATAPI.CPP
//! Source header: Tools/PATCHGET/CHATAPI.H
//!
//! Port of the C++ Chat API and patch download system. The original C++ code
//! handles:
//! - GameSpy HTTP patch checking
//! - FTP-based patch downloading with progress callbacks
//! - Windows dialog-based download UI
//! - Chat/server list API (mostly commented out in C++)
//!
//! In Rust, the Windows-specific COM chat API is replaced with a cross-platform
//! abstraction. The download manager integration is preserved via the
//! `download_manager` module.

use std::collections::VecDeque;

/// Event types for the download system, matching C++ enum EVENT_TYPES
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EventType {
    /// No update needed
    NoUpdate = 0,
    /// Download aborted
    Abort = 1,
}

/// Number of event types, matching C++ NUM_EVENTS constant
pub const NUM_EVENTS: usize = 2;

/// Game name constant matching C++ GAME_NAME
pub const GAME_NAME: &str = "Command & Conquer";

/// Download status codes matching C++ download status constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DownloadStatus {
    Connecting = 0,
    FindingFile = 1,
    Downloading = 2,
}

/// Download finish states matching C++ g_Finished values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// Download in progress
    InProgress = 0,
    /// Download completed successfully
    Finished = 1,
    /// Download failed with error
    Failed = -1,
}

/// Queued download entry matching the C++ QueuedDownload struct
#[derive(Debug, Clone, Default)]
pub struct QueuedDownload {
    /// Server hostname
    pub server: String,
    /// Username for authentication
    pub user_name: String,
    /// Password for authentication
    pub password: String,
    /// Remote file path
    pub file: String,
    /// Local file path
    pub local_file: String,
    /// Registry key for tracking
    pub reg_key: String,
    /// Whether to attempt resume
    pub try_resume: bool,
}

/// Global download state mirroring C++ globals
pub struct DownloadStateTracker {
    /// Current finish state (mirrors g_Finished)
    pub finished: i32,
    /// Time remaining string (mirrors g_DLTimeRem)
    pub time_remaining: String,
    /// Bytes left string (mirrors g_DLBytesLeft)
    pub bytes_left: String,
    /// Bytes per second string (mirrors g_DLBPS)
    pub bps: String,
    /// Update string for filename (mirrors g_UpdateString)
    pub update_string: String,
    /// Whether we are checking for patches
    pub checking_for_patch: bool,
    /// Number of checks left
    pub checks_left: i32,
    /// Whether connection failed
    pub cant_connect: bool,
}

impl Default for DownloadStateTracker {
    fn default() -> Self {
        Self {
            finished: 0,
            time_remaining: String::new(),
            bytes_left: String::new(),
            bps: String::new(),
            update_string: String::new(),
            checking_for_patch: false,
            checks_left: 0,
            cant_connect: false,
        }
    }
}

/// Patch download queue mirroring C++ queuedDownloads list
pub struct PatchQueue {
    downloads: VecDeque<QueuedDownload>,
}

impl PatchQueue {
    pub fn new() -> Self {
        Self {
            downloads: VecDeque::new(),
        }
    }

    /// Queue a patch for download. Mirrors C++ queuePatch() function.
    /// Parses FTP-style URLs of the form "ftp://server:user@pass/path/file"
    pub fn queue_patch(&mut self, mandatory: bool, download_url: &str) -> bool {
        let q = match parse_download_url(download_url) {
            Some(q) => q,
            None => return false,
        };

        // Don't add duplicate local files (mirrors C++ duplicate check)
        if self.downloads.iter().any(|d| d.local_file == q.local_file) {
            return false;
        }

        self.downloads.push_back(q);
        true
    }

    /// Get the next download from the queue
    pub fn pop_front(&mut self) -> Option<QueuedDownload> {
        self.downloads.pop_front()
    }

    /// Check if queue has pending downloads
    pub fn has_pending(&self) -> bool {
        !self.downloads.is_empty()
    }

    /// Get number of pending downloads
    pub fn len(&self) -> usize {
        self.downloads.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.downloads.is_empty()
    }
}

impl Default for PatchQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a download URL into a QueuedDownload entry.
/// Mirrors the C++ queuePatch() URL parsing logic.
fn parse_download_url(url: &str) -> Option<QueuedDownload> {
    let mut url = url.to_string();
    let mut q = QueuedDownload::default();

    // Extract connection type (e.g., "ftp")
    let connection_type = next_token(&mut url, ":")?;
    if connection_type.is_empty() {
        return None;
    }

    // Extract server
    q.server = next_token(&mut url, ":/")?;

    // Extract user
    q.user_name = next_token(&mut url, ":@")?;

    // Extract password
    q.password = next_token(&mut url, "@/")?;

    // Remaining is file path
    q.file = url.trim().to_string();

    // If no user/pass, shift file into proper place
    if q.password.is_empty() && !q.user_name.is_empty() {
        q.file = q.user_name.clone();
        q.user_name.clear();
    }

    // Build local file path
    let file_name = q
        .file
        .rfind('/')
        .map(|pos| q.file[pos + 1..].to_string())
        .unwrap_or_else(|| q.file.clone());
    q.local_file = format!("patches\\{}", file_name);
    q.try_resume = true;

    Some(q)
}

/// Tokenize a string by separator, matching C++ nextToken() behavior.
fn next_token(base: &mut String, seps: &str) -> Option<String> {
    if base.is_empty() {
        return None;
    }

    let start = base.find(|c: char| !seps.contains(c)).unwrap_or(base.len());
    let trimmed = base[start..].to_string();

    if trimmed.is_empty() {
        base.clear();
        return Some(String::new());
    }

    let end = trimmed
        .find(|c: char| seps.contains(c))
        .unwrap_or(trimmed.len());

    let token = trimmed[..end].to_string();
    *base = trimmed[end..].to_string();

    Some(token)
}

/// String trimming utility matching C++ trim() function
pub fn trim(s: &str, delim: &str) -> String {
    let chars: Vec<char> = delim.chars().collect();
    s.trim_matches(|c| chars.contains(&c)).to_string()
}

/// Get next line from string, matching C++ getNextLine() behavior
pub fn get_next_line(input: &str) -> (String, String) {
    let line_end = input.find(|c| c == '\n' || c == '\r').unwrap_or(0);
    if line_end < 1 {
        return (input.to_string(), String::new());
    }

    let out = trim(&input[..line_end], "\r\n\t ");
    let remainder = trim(&input[line_end + 1..], "\r\n\t ");

    (out, remainder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_download_url() {
        let url = "ftp://ftp.ea.com:user@pass/pub/patches/update.rtp";
        let q = parse_download_url(url);
        assert!(q.is_some());
        let q = q.unwrap();
        assert_eq!(q.server, "ftp.ea.com");
        assert_eq!(q.user_name, "user");
        assert_eq!(q.password, "pass");
        assert_eq!(q.file, "/pub/patches/update.rtp");
    }

    #[test]
    fn test_patch_queue() {
        let mut queue = PatchQueue::new();
        assert!(queue.is_empty());

        let url = "ftp://server:u@p/path/file.rtp";
        assert!(queue.queue_patch(true, url));
        assert_eq!(queue.len(), 1);

        // Duplicate should not be added
        assert!(!queue.queue_patch(true, url));
        assert_eq!(queue.len(), 1);

        let item = queue.pop_front();
        assert!(item.is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  ", " "), "hello");
        assert_eq!(trim("\r\nhello\r\n", "\r\n"), "hello");
    }
}
