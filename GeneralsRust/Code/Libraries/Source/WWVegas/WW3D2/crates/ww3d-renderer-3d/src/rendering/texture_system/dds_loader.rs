//! Renderer adapter for the canonical CPU DDS decoder in ww3d-assets.
use super::image_upload_format::image_decode_error;
use crate::core::error::RendererResult;
use std::path::Path;

pub use ww3d_assets::image::dds::{
    DdsCaps, DdsCompression, DdsData, DdsHeader, DdsHeaderDxt10, DdsPixelFormat, DdsTextureType,
};

pub fn load_dds_file<P: AsRef<Path>>(path: P) -> RendererResult<DdsData> {
    ww3d_assets::image::load_dds_file(path).map_err(image_decode_error)
}

pub fn load_dds_from_memory(data: &[u8]) -> RendererResult<DdsData> {
    ww3d_assets::image::load_dds_from_memory(data).map_err(image_decode_error)
}

pub fn decode_dxt1(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    ww3d_assets::image::decode_dxt1(data, width, height).map_err(image_decode_error)
}

pub fn decode_dxt3(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    ww3d_assets::image::decode_dxt3(data, width, height).map_err(image_decode_error)
}

pub fn decode_dxt5(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    ww3d_assets::image::decode_dxt5(data, width, height).map_err(image_decode_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::Error;
    use bytemuck::Zeroable;

    #[test]
    fn renderer_adapter_keeps_compressed_image_and_mip_metadata() {
        let mut header = DdsHeader::zeroed();
        header.size = 124;
        header.flags = 0x1007;
        header.width = 4;
        header.height = 4;
        header.pixel_format.size = 32;
        header.pixel_format.flags = 4;
        header.pixel_format.four_cc = u32::from_le_bytes(*b"DXT1");
        let payload = [0, 0xf8, 0, 0, 0, 0, 0, 0];
        let mut bytes = b"DDS ".to_vec();
        bytes.extend_from_slice(bytemuck::bytes_of(&header));
        bytes.extend_from_slice(&payload);
        let decoded = load_dds_from_memory(&bytes).unwrap();
        assert_eq!(decoded.compression, Some(DdsCompression::Dxt1));
        assert_eq!(decoded.get_level_data(0), Some(payload.as_slice()));
        assert_eq!(decoded.level_offsets, [0]);
        assert_eq!(decoded.level_sizes, [8]);
        assert_eq!(
            decode_dxt1(decoded.get_level_data(0).unwrap(), 4, 4).unwrap(),
            [255, 0, 0, 255].repeat(16)
        );
    }

    #[test]
    fn renderer_adapter_preserves_dds_and_dxt_error_categories() {
        assert!(
            matches!(load_dds_from_memory(b"FAIL"), Err(Error::InvalidData(message)) if message == "Invalid DDS magic number")
        );
        assert_eq!(
            decode_dxt1(&[], 4, 4),
            Err(Error::InvalidData("DXT1 data truncated".into()))
        );
        assert_eq!(
            decode_dxt3(&[], 4, 4),
            Err(Error::InvalidData("DXT3 data truncated".into()))
        );
        assert_eq!(
            decode_dxt5(&[], 4, 4),
            Err(Error::InvalidData("DXT5 data truncated".into()))
        );
    }
}
