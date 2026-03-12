//! IO helper structs (ported from WWLib iostruct.h).

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IOVector2Struct {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IOVector3Struct {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IOVector4Struct {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IOQuaternionStruct {
    pub q: [f32; 4],
}
