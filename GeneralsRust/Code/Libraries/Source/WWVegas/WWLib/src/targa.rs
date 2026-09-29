// C++ WWLib Targa used by TextureLoadTask (not the max2w3d copy).
// Type 2 and type 10 (RLE). Bit 5 of the image descriptor clear means the
// file is bottom-up; those rows are flipped so the result matches a type-2
// top-down image (TextureLoadTask's net Y-origin handling).
use std::io::{self, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    // Truecolor uncompressed (2) and RLE (10). Grayscale and color-mapped
    // images are not what TextureLoadTask uploads.
    if image_type != 2 && image_type != 10 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported TGA",
        ));
    }
    let width = u16::from_le_bytes([header[12], header[13]]);
    let height = u16::from_le_bytes([header[14], header[15]]);
    let bpp = header[16];
    let descriptor = header[17];
    if bpp != 24 && bpp != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported bpp",
        ));
    }
    if id_len > 0 {
        skip_bytes(&mut reader, id_len)?;
    }
    // Truecolor files may still carry a color map; C++ skips it before pixels.
    if color_map_type != 0 {
        let map_len = u16::from_le_bytes([header[5], header[6]]) as usize;
        let map_entry_bytes = (header[7] as usize).div_ceil(8);
        let map_bytes = map_len.checked_mul(map_entry_bytes).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "TGA color map overflow")
        })?;
        skip_bytes(&mut reader, map_bytes)?;
    }
    let pixel_size = (bpp / 8) as usize;
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "TGA dimensions overflow"))?;
    let image_bytes = pixel_count.checked_mul(pixel_size).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "TGA image size overflow")
    })?;
    let mut data = vec![0u8; image_bytes];
    if image_type == 2 {
        reader.read_exact(&mut data)?;
    } else {
        decode_rle(&mut reader, &mut data, pixel_size)?;
    }
    // Bit 5 clear: origin is bottom-left, first stored row is the bottom.
    if descriptor & 0x20 == 0 {
        flip_rows(&mut data, (width as usize) * pixel_size, height as usize);
    }
    Ok(TgaImage {
        width,
        height,
        data,
    })
}

fn skip_bytes<R: Read>(reader: &mut R, mut remaining: usize) -> io::Result<()> {
    let mut buf = [0u8; 256];
    while remaining > 0 {
        let n = remaining.min(buf.len());
        reader.read_exact(&mut buf[..n])?;
        remaining -= n;
    }
    Ok(())
}

fn decode_rle<R: Read>(reader: &mut R, output: &mut [u8], pixel_size: usize) -> io::Result<()> {
    if pixel_size == 0 || pixel_size > 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TGA RLE bytes-per-pixel is invalid",
        ));
    }
    let mut filled = 0usize;
    let mut packet = [0u8; 1];
    while filled < output.len() {
        reader.read_exact(&mut packet).map_err(|err| {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "TGA RLE packet header is truncated",
                )
            } else {
                err
            }
        })?;
        let count = ((packet[0] & 0x7f) as usize) + 1;
        let bytes = count.checked_mul(pixel_size).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "TGA RLE packet size overflow")
        })?;
        if filled
            .checked_add(bytes)
            .map(|end| end > output.len())
            .unwrap_or(true)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "TGA RLE packet overruns image data",
            ));
        }
        if packet[0] & 0x80 != 0 {
            let mut pixel = [0u8; 4];
            reader.read_exact(&mut pixel[..pixel_size]).map_err(|err| {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "TGA RLE repeated pixel is truncated",
                    )
                } else {
                    err
                }
            })?;
            for _ in 0..count {
                output[filled..filled + pixel_size].copy_from_slice(&pixel[..pixel_size]);
                filled += pixel_size;
            }
        } else {
            reader
                .read_exact(&mut output[filled..filled + bytes])
                .map_err(|err| {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        io::Error::new(io::ErrorKind::InvalidData, "TGA RLE raw packet is truncated")
                    } else {
                        err
                    }
                })?;
            filled += bytes;
        }
    }
    Ok(())
}

fn flip_rows(data: &mut [u8], row_bytes: usize, height: usize) {
    if height < 2 || row_bytes == 0 || data.len() < row_bytes.saturating_mul(height) {
        return;
    }
    for y in 0..height / 2 {
        let top = y * row_bytes;
        let bottom = (height - 1 - y) * row_bytes;
        for i in 0..row_bytes {
            data.swap(top + i, bottom + i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::load_tga;
    use std::io::Cursor;

    fn header(image_type: u8, descriptor: u8, bpp: u8) -> Vec<u8> {
        let mut data = vec![0u8; 18];
        data[2] = image_type;
        data[12] = 2; // width
        data[14] = 2; // height
        data[16] = bpp;
        data[17] = descriptor;
        data
    }

    /// Top row then bottom row, BGR. This is what a type-2 top-down file stores.
    fn top_down_pixels() -> Vec<u8> {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    }

    fn bottom_up_pixels() -> Vec<u8> {
        vec![7, 8, 9, 10, 11, 12, 1, 2, 3, 4, 5, 6]
    }

    #[test]
    fn rle_and_bottom_up_match_type2_top_down() {
        let mut top_down = header(2, 0x20, 24);
        top_down.extend_from_slice(&top_down_pixels());

        let mut bottom_up = header(2, 0x00, 24);
        bottom_up.extend_from_slice(&bottom_up_pixels());

        // Bottom-up RLE: raw bottom row, then raw top row.
        let mut rle = header(10, 0x00, 24);
        rle.push(0x01);
        rle.extend_from_slice(&[7, 8, 9, 10, 11, 12]);
        rle.push(0x01);
        rle.extend_from_slice(&[1, 2, 3, 4, 5, 6]);

        // Top-down RLE, same pixels, no flip.
        let mut rle_top = header(10, 0x20, 24);
        rle_top.push(0x01);
        rle_top.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        rle_top.push(0x01);
        rle_top.extend_from_slice(&[7, 8, 9, 10, 11, 12]);

        let expected = top_down_pixels();
        for bytes in [top_down, bottom_up, rle, rle_top] {
            let image = load_tga(Cursor::new(bytes)).unwrap();
            assert_eq!(image.width, 2);
            assert_eq!(image.height, 2);
            assert_eq!(image.data, expected);
        }
    }

    #[test]
    fn rle_run_packet_and_alpha_survive_bottom_up_flip() {
        // 2x2 BGRA. Bottom row is an RLE run; top row is a raw packet.
        let mut data = header(10, 0x00, 32);
        data.push(0x81);
        data.extend_from_slice(&[9, 8, 7, 6]);
        data.push(0x01);
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);

        let image = load_tga(Cursor::new(data)).unwrap();
        assert_eq!(
            image.data,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 7, 6, 9, 8, 7, 6]
        );
    }

    #[test]
    fn truncated_rle_is_rejected() {
        let mut data = header(10, 0x20, 24);
        data.extend_from_slice(&[0x82, 1, 2]);
        let err = load_tga(Cursor::new(data)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
