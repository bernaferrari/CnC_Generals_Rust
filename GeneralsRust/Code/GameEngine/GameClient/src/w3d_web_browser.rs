//! W3DWebBrowser - W3D client adapter for embedded web browser windows.
//!
//! C++ source: `Source/W3DDevice/GameClient/W3DWebBrowser.cpp`.

use crate::gui::game_window::GameWindow;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_webpage_url::get_web_browser;
use std::collections::HashMap;

pub const BROWSER_OPTION_SCROLLBARS: u32 = 0x0001;
pub const BROWSER_OPTION_3D_BORDER: u32 = 0x0002;
pub const W3D_BROWSER_OPTIONS: u32 = BROWSER_OPTION_SCROLLBARS | BROWSER_OPTION_3D_BORDER;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserWindowRecord {
    pub window_name: String,
    pub tag: AsciiString,
    pub url: AsciiString,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub options: u32,
    pub presented: bool,
}

#[derive(Debug, Clone)]
pub struct W3DWebBrowser {
    active_windows: HashMap<String, BrowserWindowRecord>,
    dispatch_available: bool,
}

impl Default for W3DWebBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DWebBrowser {
    pub fn new() -> Self {
        Self {
            active_windows: HashMap::new(),
            dispatch_available: true,
        }
    }

    pub fn set_dispatch_available(&mut self, available: bool) {
        self.dispatch_available = available;
    }

    pub fn dispatch_available(&self) -> bool {
        self.dispatch_available
    }

    pub fn create_browser_window(&mut self, tag: &str, win: &GameWindow) -> bool {
        let tag = AsciiString::from(tag);
        let Some(browser) = get_web_browser() else {
            return false;
        };
        let Some(url) = browser.find_url(&tag).cloned() else {
            return false;
        };
        drop(browser);

        if !self.dispatch_available {
            return false;
        }

        let window_name = win.instance_data().decorated_name.clone();
        let (width, height) = win.get_size();
        let (x, y) = win.get_screen_position();
        let presented =
            native_browser::present_page(&window_name, url.url.as_str(), x, y, width, height);

        self.active_windows.insert(
            window_name.clone(),
            BrowserWindowRecord {
                window_name,
                tag,
                url: url.url,
                x,
                y,
                width,
                height,
                options: W3D_BROWSER_OPTIONS,
                presented,
            },
        );

        true
    }

    pub fn close_browser_window(&mut self, win: &GameWindow) {
        let name = win.instance_data().decorated_name.clone();
        native_browser::destroy_page(&name);
        self.active_windows.remove(name.as_str());
    }

    pub fn active_window(&self, window_name: &str) -> Option<&BrowserWindowRecord> {
        self.active_windows.get(window_name)
    }

    pub fn active_window_count(&self) -> usize {
        self.active_windows.len()
    }

    pub fn clear(&mut self) {
        self.active_windows.clear();
    }
}

mod native_browser {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    struct PresentedPage {
        url: String,
        #[cfg(target_os = "macos")]
        window: usize,
    }

    static PAGES: LazyLock<Mutex<HashMap<String, PresentedPage>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    pub fn present_page(
        window_name: &str,
        url: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> bool {
        #[cfg(target_os = "macos")]
        {
            if let Some(handle) = present_wkwebview(url, x, y, width, height) {
                PAGES.lock().unwrap_or_else(|e| e.into_inner()).insert(
                    window_name.to_string(),
                    PresentedPage {
                        url: url.to_string(),
                        window: handle,
                    },
                );
                return true;
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (x, y, width, height);
            PAGES.lock().unwrap_or_else(|e| e.into_inner()).insert(
                window_name.to_string(),
                PresentedPage {
                    url: url.to_string(),
                },
            );
            return present_platform_page(url);
        }
        let _ = url;
        false
    }

    pub fn destroy_page(window_name: &str) {
        if let Some(page) = PAGES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(window_name)
        {
            #[cfg(target_os = "macos")]
            destroy_wkwebview(page.window);
            let _ = page.url;
        }
    }

    #[cfg(target_os = "macos")]
    fn present_wkwebview(url: &str, x: i32, y: i32, width: i32, height: i32) -> Option<usize> {
        use cocoa::base::{id, nil};
        use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
        use objc::{class, msg_send, sel, sel_impl};

        #[link(name = "WebKit", kind = "framework")]
        // SAFETY: Empty extern block exists only to link the WebKit framework; it
        // SAFETY: declares no functions, so there is nothing unsafe to call from it.
        unsafe extern "C" {}

        // SAFETY: Objective-C runtime calls on WKWebView/NSURL/NSWindow with valid
        // SAFETY: allocations, each nil-checked before further use; all id handles
        // SAFETY: stay live inside this scope under the owning macOS runloop.
        unsafe {
            let frame = NSRect::new(
                NSPoint::new(x as f64, y as f64),
                NSSize::new(width.max(1) as f64, height.max(1) as f64),
            );
            let config: id = msg_send![class!(WKWebViewConfiguration), new];
            if config == nil {
                return None;
            }
            let alloc: id = msg_send![class!(WKWebView), alloc];
            let webview: id = msg_send![alloc, initWithFrame: frame configuration: config];
            if webview == nil {
                return None;
            }
            let ns_url_str = NSString::alloc(nil).init_str(url);
            let ns_url: id = msg_send![class!(NSURL), URLWithString: ns_url_str];
            if ns_url == nil {
                return None;
            }
            let request: id = msg_send![class!(NSURLRequest), requestWithURL: ns_url];
            let _: () = msg_send![webview, loadRequest: request];

            let style: u64 = 1 << 0; // titled
            let window_alloc: id = msg_send![class!(NSWindow), alloc];
            let window: id = msg_send![
                window_alloc,
                initWithContentRect: frame
                styleMask: style
                backing: 2u64
                defer: cocoa::base::NO
            ];
            if window == nil {
                return None;
            }
            let _: () = msg_send![window, setContentView: webview];
            let _: () = msg_send![window, makeKeyAndOrderFront: nil];
            Some(window as usize)
        }
    }

    #[cfg(target_os = "macos")]
    fn destroy_wkwebview(handle: usize) {
        use cocoa::base::id;
        use objc::{msg_send, sel, sel_impl};
        // SAFETY: `handle` is the NSWindow pointer stored by present_wkwebview and
        // SAFETY: still alive per the create/destroy pairing; orderOut:/close are the
        // SAFETY: documented teardown messages sent exactly once.
        unsafe {
            let window = handle as id;
            let _: () = msg_send![window, orderOut: cocoa::base::nil];
            let _: () = msg_send![window, close];
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn present_platform_page(url: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
            #[link(name = "shell32")]
            extern "system" {
                fn ShellExecuteW(
                    hwnd: *mut core::ffi::c_void,
                    op: *const u16,
                    file: *const u16,
                    params: *const u16,
                    dir: *const u16,
                    show: i32,
                ) -> isize;
            }
            let open: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
            // SAFETY: wide/open are NUL-terminated UTF-16 buffers outliving the
            // SAFETY: call; null hwnd/params/dir are documented allowed values and
            // SAFETY: the verb is the constant "open".
            let result = unsafe {
                ShellExecuteW(
                    core::ptr::null_mut(),
                    open.as_ptr(),
                    wide.as_ptr(),
                    core::ptr::null(),
                    core::ptr::null(),
                    1,
                )
            };
            return result > 32;
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("xdg-open")
                .arg(url)
                .spawn()
                .is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::ini_webpage_url::get_web_browser_mut;

    fn register_url(tag: &str, url: &str) {
        let mut browser = get_web_browser_mut().expect("web browser singleton");
        let entry = browser.make_new_url(AsciiString::from(tag));
        entry.url = AsciiString::from(url);
    }

    fn make_window(name: &str, x: i32, y: i32, width: i32, height: i32) -> GameWindow {
        let mut window = GameWindow::new();
        window.set_name(name);
        window.set_position(x, y).unwrap();
        window.set_size(width, height).unwrap();
        window
    }

    #[test]
    fn create_browser_window_records_cpp_window_name_url_and_rect() {
        register_url("W3DWebBrowserTermsOfService", "https://example.invalid/tos");
        let window = make_window("TOSListBox", 11, 12, 320, 240);
        let mut browser = W3DWebBrowser::new();

        assert!(browser.create_browser_window("W3DWebBrowserTermsOfService", &window));

        let record = browser
            .active_window("TOSListBox")
            .expect("browser window record");
        assert_eq!(record.window_name, "TOSListBox");
        assert_eq!(record.tag.as_str(), "W3DWebBrowserTermsOfService");
        assert_eq!(record.url.as_str(), "https://example.invalid/tos");
        assert_eq!(
            (record.x, record.y, record.width, record.height),
            (11, 12, 320, 240)
        );
        assert_eq!(record.options, W3D_BROWSER_OPTIONS);
    }

    #[test]
    fn create_browser_window_fails_for_missing_url_or_dispatch() {
        let window = make_window("MessageBoard", 0, 0, 100, 80);
        let mut browser = W3DWebBrowser::new();

        assert!(!browser.create_browser_window("W3DWebBrowserMissingUrl", &window));

        register_url(
            "W3DWebBrowserMessageBoardDispatch",
            "https://example.invalid/forum",
        );
        browser.set_dispatch_available(false);
        assert!(!browser.create_browser_window("W3DWebBrowserMessageBoardDispatch", &window));
        assert_eq!(browser.active_window_count(), 0);
    }

    #[test]
    fn close_browser_window_destroys_record_by_decorated_name() {
        register_url(
            "W3DWebBrowserMessageBoardClose",
            "https://example.invalid/forum",
        );
        let window = make_window("MessageBoardWindow", 0, 0, 100, 80);
        let mut browser = W3DWebBrowser::new();
        assert!(browser.create_browser_window("W3DWebBrowserMessageBoardClose", &window));

        browser.close_browser_window(&window);

        assert!(browser.active_window("MessageBoardWindow").is_none());
        assert_eq!(browser.active_window_count(), 0);
    }
}
