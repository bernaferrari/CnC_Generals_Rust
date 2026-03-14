//! C++ parity state for `ImageErrorProc.cpp`.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageValidationError {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageErrorDialogState {
    pub errors: Vec<ImageValidationError>,
    pub proceed_with_valid_images: bool,
}

impl ImageErrorDialogState {
    pub fn add_error(&mut self, file: impl Into<String>, reason: impl Into<String>) {
        self.errors.push(ImageValidationError {
            file: file.into(),
            reason: reason.into(),
        });
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ImageErrorDialogState;

    #[test]
    fn tracks_validation_errors() {
        let mut state = ImageErrorDialogState::default();
        state.add_error("a.tga", "unsupported color depth");
        assert!(state.has_errors());
    }
}
