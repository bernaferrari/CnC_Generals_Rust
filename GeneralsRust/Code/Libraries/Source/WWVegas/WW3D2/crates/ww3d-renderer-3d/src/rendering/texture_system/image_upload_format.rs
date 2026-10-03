//! Map decoded asset encodings at the GPU boundary.
use crate::core::error::Error;
use ww3d_assets::image::{ImageDecodeError, ImageFormat};

pub fn image_format_to_wgpu(format: ImageFormat) -> wgpu::TextureFormat {
    match format {
        ImageFormat::Bc1RgbaUnorm => wgpu::TextureFormat::Bc1RgbaUnorm,
        ImageFormat::Bc1RgbaUnormSrgb => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        ImageFormat::Bc2RgbaUnorm => wgpu::TextureFormat::Bc2RgbaUnorm,
        ImageFormat::Bc2RgbaUnormSrgb => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        ImageFormat::Bc3RgbaUnorm => wgpu::TextureFormat::Bc3RgbaUnorm,
        ImageFormat::Bc3RgbaUnormSrgb => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        ImageFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        ImageFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        ImageFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        ImageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        ImageFormat::Rgba16Unorm => wgpu::TextureFormat::Rgba16Unorm,
        ImageFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        ImageFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        ImageFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
    }
}

pub(super) fn image_decode_error(error: ImageDecodeError) -> Error {
    match error {
        ImageDecodeError::Generic(message) => Error::Generic(message),
        ImageDecodeError::InvalidData(message) => Error::InvalidData(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_mapping_keeps_linear_and_srgb_block_formats_distinct() {
        assert_eq!(
            image_format_to_wgpu(ImageFormat::Bc1RgbaUnorm),
            wgpu::TextureFormat::Bc1RgbaUnorm
        );
        assert_eq!(
            image_format_to_wgpu(ImageFormat::Bc1RgbaUnormSrgb),
            wgpu::TextureFormat::Bc1RgbaUnormSrgb
        );
        assert_eq!(
            image_format_to_wgpu(ImageFormat::Rgba8Unorm),
            wgpu::TextureFormat::Rgba8Unorm
        );
        assert_eq!(
            image_format_to_wgpu(ImageFormat::Rgba8UnormSrgb),
            wgpu::TextureFormat::Rgba8UnormSrgb
        );
    }

    #[test]
    fn image_errors_keep_renderer_category_and_message() {
        assert_eq!(
            image_decode_error(ImageDecodeError::InvalidData("DDS file truncated".into())),
            Error::InvalidData("DDS file truncated".into())
        );
        assert_eq!(
            image_decode_error(ImageDecodeError::Generic("read failure".into())),
            Error::Generic("read failure".into())
        );
    }
}
