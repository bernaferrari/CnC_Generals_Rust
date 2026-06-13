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
            },
        );

        true
    }

    pub fn close_browser_window(&mut self, win: &GameWindow) {
        self.active_windows
            .remove(win.instance_data().decorated_name.as_str());
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
