//! C++ parity model for `ImageInfo.h`.

use bitflags::bitflags;
use std::path::{Path, PathBuf};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImageStatus: u32 {
        const UNPACKED = 0x0000_0001;
        const PACKED = 0x0000_0002;
        const TOOBIG = 0x0000_0004;
        const ROTATED90C = 0x0000_0008;
        const CANTPROCESS = 0x0000_0010;
        const INVALIDCOLORDEPTH = 0x0000_0020;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FitBits: u32 {
        const X_GUTTER = 0x0000_0001;
        const Y_GUTTER = 0x0000_0002;
        const X_BORDER_RIGHT = 0x0000_0004;
        const X_BORDER_LEFT = 0x0000_0008;
        const Y_BORDER_TOP = 0x0000_0010;
        const Y_BORDER_BOTTOM = 0x0000_0020;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Region {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub filename_only: String,
    pub filename_only_no_ext: String,
    pub color_depth: u8,
    pub width: u32,
    pub height: u32,
    pub area: u32,
    pub status: ImageStatus,
    pub page_id: Option<u32>,
    pub page_pos: Region,
    pub fit_bits: FitBits,
    pub gutter_used: (u32, u32),
}

impl ImageInfo {
    pub fn new(path: impl AsRef<Path>, width: u32, height: u32, color_depth: u8) -> Self {
        let path = path.as_ref().to_path_buf();
        let filename_only = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let filename_only_no_ext = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        Self {
            path,
            filename_only,
            filename_only_no_ext,
            color_depth,
            width,
            height,
            area: width.saturating_mul(height),
            status: ImageStatus::UNPACKED,
            page_id: None,
            page_pos: Region::default(),
            fit_bits: FitBits::empty(),
            gutter_used: (0, 0),
        }
    }

    pub fn can_process_for_target(&mut self, target_width: u32, target_height: u32) -> bool {
        let mut ok = true;
        if self.width > target_width || self.height > target_height {
            self.status
                .insert(ImageStatus::TOOBIG | ImageStatus::CANTPROCESS);
            ok = false;
        }
        if self.color_depth != 24 && self.color_depth != 32 {
            self.status
                .insert(ImageStatus::INVALIDCOLORDEPTH | ImageStatus::CANTPROCESS);
            ok = false;
        }
        ok
    }

    pub fn mark_packed(&mut self, page_id: u32, page_pos: Region, rotated: bool) {
        self.page_id = Some(page_id);
        self.page_pos = page_pos;
        self.status.remove(ImageStatus::UNPACKED);
        self.status.insert(ImageStatus::PACKED);
        if rotated {
            self.status.insert(ImageStatus::ROTATED90C);
        } else {
            self.status.remove(ImageStatus::ROTATED90C);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageInfo, ImageStatus, Region};

    #[test]
    fn marks_invalid_color_depth_as_unprocessable() {
        let mut info = ImageInfo::new("a.tga", 64, 64, 16);
        assert!(!info.can_process_for_target(128, 128));
        assert!(info.status.contains(ImageStatus::INVALIDCOLORDEPTH));
        assert!(info.status.contains(ImageStatus::CANTPROCESS));
    }

    #[test]
    fn marks_image_as_packed() {
        let mut info = ImageInfo::new("a.tga", 32, 32, 32);
        info.mark_packed(
            1,
            Region {
                left: 0,
                top: 0,
                right: 31,
                bottom: 31,
            },
            true,
        );
        assert!(info.status.contains(ImageStatus::PACKED));
        assert!(info.status.contains(ImageStatus::ROTATED90C));
        assert_eq!(info.page_id, Some(1));
    }
}
