//! C++ parity state for `ImagePackerProc.cpp` dialog options and controls.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetSizeOption {
    Size128,
    Size256,
    #[default]
    Size512,
    Custom(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePackerDialogState {
    pub status_message: String,
    pub output_filename: String,
    pub recurse_subfolders: bool,
    pub output_alpha: bool,
    pub create_ini: bool,
    pub use_bitmap_preview: bool,
    pub compress_textures: bool,
    pub gap_extend_rgb: bool,
    pub gap_gutter: bool,
    pub gutter_size: u32,
    pub target_size: TargetSizeOption,
    pub selected_preview_page: u32,
}

impl Default for ImagePackerDialogState {
    fn default() -> Self {
        Self {
            status_message: "Select options and click 'Start'.".to_string(),
            output_filename: "NewImage".to_string(),
            recurse_subfolders: true,
            output_alpha: true,
            create_ini: true,
            use_bitmap_preview: false,
            compress_textures: false,
            gap_extend_rgb: true,
            gap_gutter: false,
            gutter_size: 1,
            target_size: TargetSizeOption::Size512,
            selected_preview_page: 1,
        }
    }
}

impl ImagePackerDialogState {
    pub fn effective_target_size(&self) -> u32 {
        match self.target_size {
            TargetSizeOption::Size128 => 128,
            TargetSizeOption::Size256 => 256,
            TargetSizeOption::Size512 => 512,
            TargetSizeOption::Custom(value) => value.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImagePackerDialogState, TargetSizeOption};

    #[test]
    fn keeps_cpp_defaults() {
        let state = ImagePackerDialogState::default();
        assert_eq!(state.output_filename, "NewImage");
        assert!(state.recurse_subfolders);
        assert!(state.output_alpha);
        assert_eq!(state.target_size, TargetSizeOption::Size512);
    }
}
