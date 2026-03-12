//! URLLaunch - Cross-platform URL launching functionality
//! Converted from Windows-specific implementation to cross-platform Rust

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use winapi::shared::minwindef::DWORD;
#[cfg(windows)]
use winapi::shared::winerror::ERROR_SUCCESS;
#[cfg(windows)]
use winapi::um::errhandlingapi::GetLastError;
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;
#[cfg(windows)]
use winapi::um::minwinbase::STARTUPINFOW;
#[cfg(windows)]
use winapi::um::processthreadsapi::{CreateProcessW, PROCESS_INFORMATION};
#[cfg(windows)]
use winapi::um::winnt::{KEY_READ, LPCWSTR, LPWSTR};
#[cfg(windows)]
use winapi::um::winreg::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CLASSES_ROOT};

// Type aliases for compatibility
pub type HResult = i32;

// HRESULT constants
const S_OK: HResult = 0;
const E_FAIL: HResult = -1;
const E_INVALIDARG: HResult = -2;
const E_OUTOFMEMORY: HResult = -3;

const FILE_PREFIX: &str = "file://";

/// Make an escaped URL from a potentially unsafe input URL.
/// Mirrors C++ MakeEscapedURL by escaping specific ASCII characters and optionally
/// prepending file:// when no scheme is present.
pub fn make_escaped_url(input_url: &str) -> Result<String, HResult> {
    if input_url.is_empty() {
        return Err(E_INVALIDARG);
    }

    // Check if we need to prepend file://
    let needs_file_prefix = !input_url.contains("://");

    // Characters that need to be escaped in URLs (ASCII set)
    const CHARS_TO_ESCAPE: &[u16] = &[
        b' ' as u16,
        b'#' as u16,
        b'$' as u16,
        b'%' as u16,
        b'&' as u16,
        b'\\' as u16,
        b'+' as u16,
        b',' as u16,
        b';' as u16,
        b'=' as u16,
        b'@' as u16,
        b'[' as u16,
        b']' as u16,
        b'^' as u16,
        b'{' as u16,
        b'}' as u16,
    ];

    // Work in UTF-16 code units to mirror Windows wchar_t behavior.
    let utf16: Vec<u16> = input_url.encode_utf16().collect();
    let escape_count = utf16.iter().filter(|c| CHARS_TO_ESCAPE.contains(c)).count();

    // Calculate needed capacity (approx; UTF-16 code units map to 1+ bytes in UTF-8).
    let mut capacity = input_url.len() + (2 * escape_count);
    if needs_file_prefix {
        capacity += FILE_PREFIX.len();
    }

    let mut result = String::with_capacity(capacity);
    if needs_file_prefix {
        result.push_str(FILE_PREFIX);
    }

    for code_unit in utf16 {
        if CHARS_TO_ESCAPE.contains(&code_unit) {
            result.push('%');
            result.push_str(&format!("{:02x}", code_unit));
        } else {
            if let Some(ch) = std::char::from_u32(code_unit as u32) {
                result.push(ch);
            } else {
                // Preserve ill-formed code units as escaped hex.
                result.push('%');
                result.push_str(&format!("{:02x}", code_unit));
            }
        }
    }

    Ok(result)
}

/// Launch a URL using the system's default browser or application
/// Cross-platform implementation that works on Windows, macOS, and Linux
pub fn launch_url(url: &str) -> HResult {
    if url.is_empty() {
        return E_INVALIDARG;
    }

    // Try to launch the URL using platform-specific commands
    let result = if cfg!(target_os = "windows") {
        launch_url_windows(url)
    } else if cfg!(target_os = "macos") {
        launch_url_macos(url)
    } else if cfg!(target_os = "linux") {
        launch_url_linux(url)
    } else {
        // Unsupported platform
        E_FAIL
    };

    result
}

/// Windows-specific URL launching (matches C++ URLLaunch.cpp behavior).
#[cfg(windows)]
fn launch_url_windows(url: &str) -> HResult {
    let shell_open_command = match get_shell_open_command() {
        Ok(cmd) => cmd,
        Err(hr) => return hr,
    };

    let launch_command = build_launch_command(&shell_open_command, url);
    let exe = extract_exe_from_command(&shell_open_command);

    let exe_w = to_wide(&exe);
    let mut cmd_w = to_wide(&launch_command);

    unsafe {
        let mut proc_info: PROCESS_INFORMATION = std::mem::zeroed();
        let mut startup: STARTUPINFOW = std::mem::zeroed();
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as DWORD;

        let success = CreateProcessW(
            exe_w.as_ptr(),
            cmd_w.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            0,
            ptr::null_mut(),
            ptr::null(),
            &mut startup,
            &mut proc_info,
        );

        if success == 0 {
            return hresult_from_win32(GetLastError());
        }

        if !proc_info.hThread.is_null() {
            CloseHandle(proc_info.hThread);
        }
        if !proc_info.hProcess.is_null() {
            CloseHandle(proc_info.hProcess);
        }
    }

    S_OK
}

/// Fallback for non-Windows when compiling on Windows (shouldn't be called)
#[cfg(not(windows))]
fn launch_url_windows(_url: &str) -> HResult {
    E_FAIL
}

/// macOS-specific URL launching
#[cfg(target_os = "macos")]
fn launch_url_macos(url: &str) -> HResult {
    match Command::new("open").arg(url).spawn() {
        Ok(_) => S_OK,
        Err(_) => E_FAIL,
    }
}

/// Fallback for non-macOS when compiling on macOS (shouldn't be called)
#[cfg(not(target_os = "macos"))]
fn launch_url_macos(_url: &str) -> HResult {
    E_FAIL
}

/// Linux-specific URL launching
#[cfg(target_os = "linux")]
fn launch_url_linux(url: &str) -> HResult {
    // Try xdg-open first (most common)
    if let Ok(_) = Command::new("xdg-open").arg(url).spawn() {
        return S_OK;
    }

    // Fallback to other common browsers
    let browsers = ["firefox", "google-chrome", "chromium-browser", "konqueror"];

    for browser in &browsers {
        if let Ok(_) = Command::new(browser).arg(url).spawn() {
            return S_OK;
        }
    }

    E_FAIL
}

/// Fallback for non-Linux when compiling on Linux (shouldn't be called)
#[cfg(not(target_os = "linux"))]
fn launch_url_linux(_url: &str) -> HResult {
    E_FAIL
}

/// Enhanced URL launching with error handling and validation
pub fn launch_url_safe(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("URL cannot be empty".to_string());
    }

    // Validate URL format (basic check)
    if !is_valid_url(url) {
        return Err("Invalid URL format".to_string());
    }

    match launch_url(url) {
        S_OK => Ok(()),
        E_INVALIDARG => Err("Invalid argument".to_string()),
        E_FAIL => Err("Failed to launch URL".to_string()),
        _ => Err("Unknown error".to_string()),
    }
}

/// Basic URL validation
fn is_valid_url(url: &str) -> bool {
    // Check for common URL schemes or local file paths
    url.contains("://")
        || url.starts_with("www.")
        || url.contains('.') && (url.ends_with(".html") || url.ends_with(".htm"))
        || Path::new(url).exists()
}

/// Open a local file in the default application
pub fn open_local_file(file_path: &str) -> HResult {
    if file_path.is_empty() {
        return E_INVALIDARG;
    }

    let path = Path::new(file_path);
    if !path.exists() {
        return E_FAIL;
    }

    // Convert to file:// URL and launch
    match make_escaped_url(file_path) {
        Ok(escaped_url) => launch_url(&escaped_url),
        Err(e) => e,
    }
}

/// Launch URL with a specific application (advanced usage)
pub fn launch_url_with_application(url: &str, application: &str) -> HResult {
    if url.is_empty() || application.is_empty() {
        return E_INVALIDARG;
    }

    match Command::new(application).arg(url).spawn() {
        Ok(_) => S_OK,
        Err(_) => E_FAIL,
    }
}

/// Get the default browser command for the current platform
pub fn get_default_browser_command() -> Option<String> {
    if cfg!(target_os = "windows") {
        #[cfg(windows)]
        {
            if let Ok(cmd) = get_shell_open_command() {
                return Some(cmd);
            }
        }
        Some("start".to_string())
    } else if cfg!(target_os = "macos") {
        // On macOS, we use 'open'
        Some("open".to_string())
    } else if cfg!(target_os = "linux") {
        // On Linux, try to find xdg-open or common browsers
        if Command::new("which").arg("xdg-open").output().is_ok() {
            Some("xdg-open".to_string())
        } else {
            // Try to find a common browser
            let browsers = ["firefox", "google-chrome", "chromium-browser"];
            for browser in &browsers {
                if Command::new("which").arg(browser).output().is_ok() {
                    return Some(browser.to_string());
                }
            }
            None
        }
    } else {
        None
    }
}

#[cfg(windows)]
fn get_shell_open_command() -> Result<String, HResult> {
    if let Ok(file_type) = read_registry_string(HKEY_CLASSES_ROOT, ".html", None) {
        let key = format!("{}\\shell\\open\\command", file_type);
        if let Ok(cmd) = read_registry_string(HKEY_CLASSES_ROOT, &key, None) {
            return Ok(cmd);
        }
    }

    read_registry_string(HKEY_CLASSES_ROOT, "http\\shell\\open\\command", None)
}

#[cfg(windows)]
fn read_registry_string(
    hkey: HKEY,
    path: &str,
    value_name: Option<&str>,
) -> Result<String, HResult> {
    let path_wide = to_wide(path);
    let value_wide = value_name.map(to_wide);

    unsafe {
        let mut key: HKEY = std::mem::zeroed();
        let result = RegOpenKeyExW(hkey, path_wide.as_ptr(), 0, KEY_READ, &mut key);
        if result != ERROR_SUCCESS {
            return Err(hresult_from_win32(result));
        }

        let mut data_type: DWORD = 0;
        let mut data_size: DWORD = 0;
        let result = RegQueryValueExW(
            key,
            value_wide
                .as_ref()
                .map(|v| v.as_ptr())
                .unwrap_or(ptr::null()) as LPCWSTR,
            ptr::null_mut(),
            &mut data_type,
            ptr::null_mut(),
            &mut data_size,
        );

        if result != ERROR_SUCCESS {
            RegCloseKey(key);
            return Err(hresult_from_win32(result));
        }

        let mut buffer: Vec<u8> = vec![0u8; data_size as usize];
        let result = RegQueryValueExW(
            key,
            value_wide
                .as_ref()
                .map(|v| v.as_ptr())
                .unwrap_or(ptr::null()) as LPCWSTR,
            ptr::null_mut(),
            &mut data_type,
            buffer.as_mut_ptr(),
            &mut data_size,
        );

        RegCloseKey(key);
        if result != ERROR_SUCCESS {
            return Err(hresult_from_win32(result));
        }

        let wide: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|&c| c != 0)
            .collect();
        let os = OsString::from_wide(&wide);
        Ok(os.to_string_lossy().trim().to_string())
    }
}

#[cfg(windows)]
fn build_launch_command(shell_command: &str, url: &str) -> String {
    if let Some(idx) = shell_command.find("\"%1\"") {
        let prefix = &shell_command[..idx];
        let suffix = &shell_command[idx + 4..];
        return format!("{prefix}{url}{suffix}");
    }
    if let Some(idx) = shell_command.find("\"%*\"") {
        let prefix = &shell_command[..idx];
        let suffix = &shell_command[idx + 4..];
        return format!("{prefix}{url}{suffix}");
    }
    format!("{shell_command} {url}")
}

#[cfg(windows)]
fn extract_exe_from_command(shell_command: &str) -> String {
    let trimmed = shell_command.trim_start();
    if trimmed.starts_with('"') {
        if let Some(end) = trimmed[1..].find('"') {
            return trimmed[1..1 + end].to_string();
        }
        return trimmed.trim_matches('"').to_string();
    }
    if let Some(end) = trimmed.find(' ') {
        return trimmed[..end].to_string();
    }
    trimmed.to_string()
}

#[cfg(windows)]
fn to_wide<S: AsRef<OsStr>>(value: S) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn hresult_from_win32(code: DWORD) -> HResult {
    if code <= 0 {
        code as HResult
    } else {
        ((code & 0x0000_FFFF) | 0x8007_0000) as HResult
    }
}

/// Utility function to check if a URL launching capability exists on the system
pub fn can_launch_urls() -> bool {
    get_default_browser_command().is_some()
}

/// Launch a URL and wait for the process to complete (blocking)
pub fn launch_url_blocking(url: &str) -> HResult {
    if url.is_empty() {
        return E_INVALIDARG;
    }

    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "/wait", "", url])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").args(["-W", url]).status()
    } else if cfg!(target_os = "linux") {
        Command::new("xdg-open").arg(url).status()
    } else {
        return E_FAIL;
    };

    match result {
        Ok(status) => {
            if status.success() {
                S_OK
            } else {
                E_FAIL
            }
        }
        Err(_) => E_FAIL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_escaped_url() {
        let result = make_escaped_url("test file.html");
        assert!(result.is_ok());
        let escaped = result.unwrap();
        assert!(escaped.starts_with("file://"));
        assert!(escaped.contains("%20")); // space should be escaped
    }

    #[test]
    fn test_make_escaped_url_with_protocol() {
        let result = make_escaped_url("https://www.example.com/test page.html");
        assert!(result.is_ok());
        let escaped = result.unwrap();
        assert!(!escaped.starts_with("file://")); // shouldn't add file:// to URLs with protocols
        assert!(escaped.contains("%20")); // space should still be escaped
    }

    #[test]
    fn test_make_escaped_url_empty() {
        let result = make_escaped_url("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), E_INVALIDARG);
    }

    #[test]
    fn test_make_escaped_url_special_chars() {
        let result = make_escaped_url("test file with spaces & symbols.html");
        assert!(result.is_ok());
        let escaped = result.unwrap();
        assert!(escaped.contains("%20")); // space
        assert!(escaped.contains("%26")); // ampersand
    }

    #[test]
    fn test_is_valid_url() {
        assert!(is_valid_url("https://www.example.com"));
        assert!(is_valid_url("http://test.com"));
        assert!(is_valid_url("www.example.com"));
        assert!(is_valid_url("test.html"));
        assert!(!is_valid_url("invalid"));
        assert!(!is_valid_url(""));
    }

    #[test]
    fn test_launch_url_empty() {
        assert_eq!(launch_url(""), E_INVALIDARG);
    }

    #[test]
    fn test_launch_url_safe() {
        let result = launch_url_safe("");
        assert!(result.is_err());

        let result = launch_url_safe("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_can_launch_urls() {
        // This should work on most systems
        let can_launch = can_launch_urls();
        // We can't assert true/false definitively since it depends on the test environment
        // but we can at least verify the function doesn't panic
        println!("Can launch URLs: {}", can_launch);
    }

    #[test]
    fn test_get_default_browser_command() {
        let command = get_default_browser_command();
        // Similar to above, we can't assert a specific value
        // but we can verify it doesn't panic
        println!("Default browser command: {:?}", command);
    }

    #[test]
    fn test_launch_url_with_application_empty() {
        assert_eq!(launch_url_with_application("", "browser"), E_INVALIDARG);
        assert_eq!(
            launch_url_with_application("http://example.com", ""),
            E_INVALIDARG
        );
    }

    #[test]
    fn test_open_local_file_empty() {
        assert_eq!(open_local_file(""), E_INVALIDARG);
    }

    #[test]
    fn test_open_local_file_nonexistent() {
        assert_eq!(open_local_file("/nonexistent/file.txt"), E_FAIL);
    }

    // Integration tests that actually try to launch URLs would be platform-specific
    // and might not be suitable for automated testing, but could be useful for manual testing

    #[test]
    #[ignore] // Only run manually since it actually opens a browser
    fn test_launch_real_url() {
        let result = launch_url("https://www.google.com");
        // On most systems this should work, but we can't guarantee it in CI
        println!("Launch result: {}", result);
    }
}
