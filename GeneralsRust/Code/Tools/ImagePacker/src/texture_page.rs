//! C++ parity model for `TexturePage.h`.

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageStatus: u32 {
        const READY = 0x0000_0001;
        const PAGE_ERROR = 0x0000_0002;
        const CANT_ALLOCATE_PACKED_IMAGE = 0x0000_0004;
        const CANT_ADD_IMAGE_DATA = 0x0000_0008;
        const NO_TEXTURE_DATA = 0x0000_0010;
        const ERROR_DURING_SAVE = 0x0000_0020;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasCell {
    Free,
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePlacement {
    pub image_index: usize,
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub rotated: bool,
}

#[derive(Debug, Clone)]
pub struct TexturePage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub status: PageStatus,
    pub placements: Vec<ImagePlacement>,
}

impl TexturePage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            id: 0,
            width,
            height,
            status: PageStatus::READY,
            placements: Vec::new(),
        }
    }

    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    pub fn add_image_placement(&mut self, placement: ImagePlacement) {
        self.placements.push(placement);
    }

    pub fn get_first_image(&self) -> Option<&ImagePlacement> {
        self.placements.first()
    }
}

#[cfg(test)]
mod tests {
    use super::{ImagePlacement, TexturePage};

    #[test]
    fn keeps_first_placement_accessible() {
        let mut page = TexturePage::new(512, 512);
        page.add_image_placement(ImagePlacement {
            image_index: 0,
            left: 5,
            top: 7,
            right: 35,
            bottom: 39,
            rotated: false,
        });
        assert_eq!(page.get_first_image().map(|p| p.left), Some(5));
    }
}
