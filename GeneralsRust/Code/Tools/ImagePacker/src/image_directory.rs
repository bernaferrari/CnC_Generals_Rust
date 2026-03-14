//! C++ parity model for `ImageDirectory.h`.

use std::path::{Path, PathBuf};

/// Directory descriptor used by the packer while collecting source images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDirectory {
    pub path: PathBuf,
    pub image_count: u32,
}

impl ImageDirectory {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            image_count: 0,
        }
    }

    pub fn with_image_count(path: impl AsRef<Path>, image_count: u32) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            image_count,
        }
    }

    pub fn increment_image_count(&mut self) {
        self.image_count = self.image_count.saturating_add(1);
    }
}

impl Default for ImageDirectory {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::ImageDirectory;

    #[test]
    fn increments_image_count_safely() {
        let mut dir = ImageDirectory::new("Art/");
        dir.increment_image_count();
        dir.increment_image_count();
        assert_eq!(dir.image_count, 2);
    }
}
