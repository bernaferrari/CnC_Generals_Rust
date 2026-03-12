// Auto-generated C++ compatibility shim for tag blocks
#[derive(Debug, Clone)]
pub struct TagBlock {
    pub tag: u32,
    pub data: Vec<u8>,
}

impl TagBlock {
    pub fn new(tag: u32, data: Vec<u8>) -> Self {
        Self { tag, data }
    }
}
