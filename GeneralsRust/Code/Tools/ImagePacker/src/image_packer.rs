//! C++ parity model for `ImagePacker.h`.

use bitflags::bitflags;
use std::path::PathBuf;

pub const MAX_OUTPUT_FILE_LEN: usize = 128;
pub const DEFAULT_TARGET_SIZE: u32 = 512;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GapMethod: u32 {
        const EXTEND_RGB = 0x0000_0001;
        const GUTTER = 0x0000_0002;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePackerSettings {
    pub target_size: (u32, u32),
    pub use_sub_folders: bool,
    pub output_file: String,
    pub output_directory: PathBuf,
    pub gap_method: GapMethod,
    pub gutter_size: u32,
    pub output_alpha: bool,
    pub create_ini: bool,
    pub compress_textures: bool,
    pub use_texture_preview: bool,
}

impl Default for ImagePackerSettings {
    fn default() -> Self {
        Self {
            target_size: (DEFAULT_TARGET_SIZE, DEFAULT_TARGET_SIZE),
            use_sub_folders: true,
            output_file: String::new(),
            output_directory: PathBuf::new(),
            gap_method: GapMethod::EXTEND_RGB,
            gutter_size: 1,
            output_alpha: true,
            create_ini: true,
            compress_textures: false,
            use_texture_preview: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagePackerState {
    pub directory_count: usize,
    pub image_count: usize,
    pub page_count: usize,
    pub images_in_directories: usize,
    pub target_preview_page: usize,
}

impl ImagePackerState {
    pub fn reset_for_new_process(&mut self) {
        self.image_count = 0;
        self.page_count = 0;
        self.target_preview_page = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{GapMethod, ImagePackerSettings};

    #[test]
    fn matches_cpp_default_options() {
        let settings = ImagePackerSettings::default();
        assert_eq!(settings.target_size, (512, 512));
        assert!(settings.use_sub_folders);
        assert!(settings.gap_method.contains(GapMethod::EXTEND_RGB));
        assert_eq!(settings.gutter_size, 1);
        assert!(settings.output_alpha);
        assert!(settings.create_ini);
        assert!(!settings.compress_textures);
    }
}
