//! W3DGameWindow (device-backed window wrapper).
//!
//! C++ reference: W3DDevice/GameClient/W3DGameWindow.h

use game_client_rust::gui::{
    with_window_manager_ref,
    GameFont as ClientGameFont,
    GameWindow,
    WindowMessage,
    WindowMsgData,
    WindowMsgHandled,
    WindowResult,
};

/// W3D-backed game window.
///
/// This wrapper forwards behavior to the core GameWindow while preserving the
/// W3D-specific entry points used by device code.
pub struct WthreeDGameWindow {
    inner: GameWindow,
}

impl WthreeDGameWindow {
    pub fn new() -> Self {
        Self {
            inner: GameWindow::new(),
        }
    }

    pub fn inner(&self) -> &GameWindow {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut GameWindow {
        &mut self.inner
    }

    /// Draw borders for this window only (delegates to core draw logic).
    pub fn win_draw_border(&self) {
        self.inner.draw_border_w3d();
    }

    pub fn win_set_position(&mut self, x: i32, y: i32) -> WindowResult<()> {
        self.inner.set_position(x, y)
    }

    pub fn win_set_text(&mut self, text: &str) -> WindowResult<()> {
        self.inner.set_text(text)
    }

    pub fn win_set_font(&mut self, font: ClientGameFont) {
        self.inner.set_font(font);
    }

    pub fn get_text_size(&self) -> (i32, i32) {
        let mut width = 0;
        let mut height = 0;
        if let Some(font) = self.inner.get_font() {
            let text = self.inner.get_text();
            let _ = with_window_manager_ref(|manager| {
                manager.win_get_text_size(font, text, Some(&mut width), Some(&mut height), 0);
            });
        }
        (width, height)
    }

    pub fn set_text_loc(&mut self, x: i32, y: i32) {
        let _ = self.inner.set_cursor_position(x, y);
    }

    pub fn draw_text(&self) {
        self.inner.draw();
    }

    pub fn set_visible(&mut self, visible: bool) {
        let _ = self.inner.hide(!visible);
    }

    pub fn is_visible(&self) -> bool {
        !self.inner.is_hidden()
    }

    pub fn set_size(&mut self, width: i32, height: i32) -> WindowResult<()> {
        self.inner.set_size(width, height)
    }

    pub fn get_size(&self) -> (i32, i32) {
        self.inner.get_size()
    }

    pub fn handle_input(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        self.inner.send_input_message(msg, data1, data2)
    }
}

impl Default for WthreeDGameWindow {
    fn default() -> Self {
        Self::new()
    }
}
