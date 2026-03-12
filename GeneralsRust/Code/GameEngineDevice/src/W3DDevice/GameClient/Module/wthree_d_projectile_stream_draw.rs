//! W3D projectile stream draw module data (port of W3DProjectileStreamDraw.h/cpp).

#[derive(Debug, Clone)]
pub struct W3DProjectileStreamDrawModuleData {
    pub texture_name: String,
    pub width: f32,
    pub tile_factor: f32,
    pub scroll_rate: f32,
    pub max_segments: i32,
}

impl W3DProjectileStreamDrawModuleData {
    pub fn new() -> Self {
        Self {
            texture_name: String::new(),
            width: 0.0,
            tile_factor: 0.0,
            scroll_rate: 0.0,
            max_segments: 0,
        }
    }
}

impl Default for W3DProjectileStreamDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
