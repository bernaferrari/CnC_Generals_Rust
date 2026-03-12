// Auto-generated C++ compatibility shim for PCX loading
use std::io::{self, Read};

#[derive(Debug, Clone)]
pub struct PcxImage {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,
}

pub fn load_pcx<R: Read>(mut reader: R) -> io::Result<PcxImage> {
    let mut header = [0u8; 128];
    reader.read_exact(&mut header)?;
    let xmin = u16::from_le_bytes([header[4], header[5]]);
    let ymin = u16::from_le_bytes([header[6], header[7]]);
    let xmax = u16::from_le_bytes([header[8], header[9]]);
    let ymax = u16::from_le_bytes([header[10], header[11]]);
    let width = xmax - xmin + 1;
    let height = ymax - ymin + 1;
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    Ok(PcxImage {
        width,
        height,
        data,
    })
}
