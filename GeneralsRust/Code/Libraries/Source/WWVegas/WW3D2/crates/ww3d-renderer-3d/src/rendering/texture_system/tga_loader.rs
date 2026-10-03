//! Renderer adapter for the canonical CPU TGA decoder in ww3d-assets.
use super::image_upload_format::image_decode_error;
use crate::core::error::RendererResult;
use std::path::Path;

pub use ww3d_assets::image::tga::{TgaData, TgaHeader};

pub fn load_tga_file<P: AsRef<Path>>(path: P) -> RendererResult<TgaData> {
    ww3d_assets::image::load_tga_file(path).map_err(image_decode_error)
}

pub fn load_tga_from_memory(data: &[u8]) -> RendererResult<TgaData> {
    ww3d_assets::image::load_tga_from_memory(data).map_err(image_decode_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::Error;
    use bytemuck::Zeroable;

    #[test]
    fn renderer_adapter_keeps_tga_channels_origin_and_alpha_depth() {
        let mut header = TgaHeader::zeroed();
        header.width = 1;
        header.height = 2;
        header.bits_per_pixel = 32;
        header.image_type = 2;
        header.image_descriptor = 8; // bottom origin with eight alpha bits
        let mut bytes = bytemuck::bytes_of(&header).to_vec();
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let decoded = load_tga_from_memory(&bytes).unwrap();
        assert_eq!(
            (decoded.width, decoded.height, decoded.bits_per_pixel),
            (1, 2, 32)
        );
        assert_eq!(decoded.data, [7, 6, 5, 8, 3, 2, 1, 4]);
    }

    #[test]
    fn renderer_adapter_preserves_tga_error_category() {
        assert!(
            matches!(load_tga_from_memory(&[]), Err(Error::InvalidData(message)) if message == "TGA file too small")
        );
    }
}
