//! C++ parity state for `DirectorySelect.cpp`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectorySelectState {
    pub selected_directory: Option<PathBuf>,
    pub include_sub_folders: bool,
}

impl DirectorySelectState {
    pub fn set_directory(&mut self, path: impl AsRef<Path>) {
        self.selected_directory = Some(path.as_ref().to_path_buf());
    }

    pub fn confirm(self) -> Option<(PathBuf, bool)> {
        self.selected_directory
            .map(|directory| (directory, self.include_sub_folders))
    }
}

#[cfg(test)]
mod tests {
    use super::DirectorySelectState;

    #[test]
    fn returns_selection_and_subfolder_option() {
        let mut state = DirectorySelectState::default();
        state.set_directory("Art/China/");
        state.include_sub_folders = true;
        let confirmed = state.confirm().expect("selection should exist");
        assert!(confirmed.1);
    }
}
