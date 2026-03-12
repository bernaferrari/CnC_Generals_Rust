// Auto-generated C++ compatibility shim for surface rectangle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl SurfRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}
