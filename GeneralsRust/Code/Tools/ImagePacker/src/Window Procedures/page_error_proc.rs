//! C++ parity state for `PageErrorProc.cpp`.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageError {
    pub page_id: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageErrorDialogState {
    pub errors: Vec<PageError>,
}

impl PageErrorDialogState {
    pub fn add_error(&mut self, page_id: u32, reason: impl Into<String>) {
        self.errors.push(PageError {
            page_id,
            reason: reason.into(),
        });
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::PageErrorDialogState;

    #[test]
    fn tracks_page_errors() {
        let mut state = PageErrorDialogState::default();
        state.add_error(2, "unable to save texture");
        assert!(state.has_errors());
    }
}
