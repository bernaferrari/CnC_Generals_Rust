//! W3D laser draw module data (port of W3DLaserDraw.h).

#[derive(Debug, Clone, Copy)]
pub struct ColorRGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRGBA {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_real(self) -> (f32, f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        )
    }
}

#[derive(Debug, Clone)]
pub struct W3DLaserDrawModuleData {
    pub inner_color: ColorRGBA,
    pub outer_color: ColorRGBA,
    pub inner_beam_width: f32,
    pub outer_beam_width: f32,
    pub scroll_rate: f32,
    pub tile: bool,
    pub num_beams: u32,
    pub max_intensity_frames: u32,
    pub fade_frames: u32,
    pub texture_name: String,
    pub segments: u32,
    pub arc_height: f32,
    pub segment_overlap_ratio: f32,
    pub tiling_scalar: f32,
}

impl W3DLaserDrawModuleData {
    pub fn new() -> Self {
        Self {
            inner_color: ColorRGBA::new(0, 0, 0, 0),
            outer_color: ColorRGBA::new(0, 0, 0, 0),
            inner_beam_width: 0.0,
            outer_beam_width: 1.0,
            scroll_rate: 0.0,
            tile: false,
            num_beams: 1,
            max_intensity_frames: 0,
            fade_frames: 0,
            texture_name: String::new(),
            segments: 1,
            arc_height: 0.0,
            segment_overlap_ratio: 0.0,
            tiling_scalar: 1.0,
        }
    }
}

impl Default for W3DLaserDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
