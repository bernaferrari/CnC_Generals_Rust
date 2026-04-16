#[derive(Debug, Clone)]
pub struct BinkVideoDecoder {
    #[allow(dead_code)]
    version_char: u8,
    #[allow(dead_code)]
    flags: u32,
    #[allow(dead_code)]
    width: usize,
    #[allow(dead_code)]
    height: usize,
}

impl BinkVideoDecoder {
    pub fn new(version_char: u8, flags: u32, width: usize, height: usize) -> Result<Self, ()> {
        Ok(Self {
            version_char,
            flags,
            width,
            height,
        })
    }
}
