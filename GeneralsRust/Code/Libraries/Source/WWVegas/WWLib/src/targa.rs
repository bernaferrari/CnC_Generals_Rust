// Auto-generated C++ compatibility shim for TGA loading
use std::io::{self, Read};

#[derive(Debug, Clone)]
pub struct TgaImage {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,
}

pub fn load_tga<R: Read>(mut reader: R) -> io::Result<TgaImage> {
    let mut header = [0u8; 18];
    reader.read_exact(&mut header)?;
    let id_len = header[0] as usize;
    let color_map_type = header[1];
    let image_type = header[2];
    if color_map_type != 0 || image_type != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported TGA",
        ));
    }
    let width = u16::from_le_bytes([header[12], header[13]]);
    let height = u16::from_le_bytes([header[14], header[15]]);
    let bpp = header[16];
    if bpp != 24 && bpp != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported bpp",
        ));
    }
    if id_len > 0 {
        let mut skip = vec![0u8; id_len];
        reader.read_exact(&mut skip)?;
    }
    let pixel_size = (bpp / 8) as usize;
    let mut data = vec![0u8; width as usize * height as usize * pixel_size];
    reader.read_exact(&mut data)?;
    Ok(TgaImage {
        width,
        height,
        data,
    })
}
