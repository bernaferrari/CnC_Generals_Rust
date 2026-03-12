// Auto-generated C++ compatibility shim for TGA->DXT
use crate::targa::TgaImage;

pub fn tga_to_dxt(image: &TgaImage) -> Vec<u8> {
    image.data.clone()
}
