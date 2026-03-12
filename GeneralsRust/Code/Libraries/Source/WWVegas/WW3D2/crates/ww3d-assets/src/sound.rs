use ww3d_core::w3d_format::{w3d_string_from_bytes, W3dSoundRObjHeaderStruct};

/// Sound render object metadata captured from W3D assets.
#[derive(Debug, Clone)]
pub struct SoundRenderObject {
    pub name: String,
    pub version: u32,
    pub flags: u32,
    pub raw_definition: Vec<u8>,
}

impl SoundRenderObject {
    pub fn from_parts(header: W3dSoundRObjHeaderStruct, raw_definition: Vec<u8>) -> Self {
        Self {
            name: w3d_string_from_bytes(&header.name),
            version: header.version,
            flags: header.flags,
            raw_definition,
        }
    }
}
