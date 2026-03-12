// Auto-generated C++ compatibility shim for DIB
#[derive(Debug, Clone)]
pub struct DIB {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl DIB {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data,
        }
    }

    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
        }
    }
}
