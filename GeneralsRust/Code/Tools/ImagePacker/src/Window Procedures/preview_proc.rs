//! C++ parity state for `PreviewProc.cpp`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreviewState {
    pub open: bool,
    pub page: u32,
    pub page_count: u32,
    pub use_texture_preview: bool,
}

impl PreviewState {
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn next_page(&mut self) {
        if self.page < self.page_count {
            self.page += 1;
        }
    }

    pub fn previous_page(&mut self) {
        if self.page > 1 {
            self.page -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PreviewState;

    #[test]
    fn clamps_preview_page_navigation() {
        let mut state = PreviewState {
            open: true,
            page: 1,
            page_count: 2,
            use_texture_preview: false,
        };
        state.previous_page();
        assert_eq!(state.page, 1);
        state.next_page();
        state.next_page();
        assert_eq!(state.page, 2);
    }
}
