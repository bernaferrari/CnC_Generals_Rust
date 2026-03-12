//! C++-style Color utilities.

/// Packed RGBA color value.
pub type Color = u32;

/// Undefined color constant (white with zero alpha).
pub const GAME_COLOR_UNDEFINED: Color = 0x00FF_FFFF;

/// Pack RGBA into a color.
pub fn game_make_color(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
    ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

/// Unpack color into RGBA components.
pub fn game_get_color_components(color: Color) -> (u8, u8, u8, u8) {
    let alpha = ((color >> 24) & 0xFF) as u8;
    let red = ((color >> 16) & 0xFF) as u8;
    let green = ((color >> 8) & 0xFF) as u8;
    let blue = (color & 0xFF) as u8;
    (red, green, blue, alpha)
}

/// Unpack color into floating point components.
pub fn game_get_color_components_real(color: Color) -> (f32, f32, f32, f32) {
    let (r, g, b, a) = game_get_color_components(color);
    (
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    )
}

/// Darken a color by a percentage.
pub fn game_darken_color(color: Color, percent: i32) -> Color {
    let percent = percent.clamp(0, 100) as u32;
    let (r, g, b, a) = game_get_color_components(color);
    let factor = (100 - percent) as f32 / 100.0;
    let darken = |c: u8| (c as f32 * factor).round() as u8;
    game_make_color(darken(r), darken(g), darken(b), a)
}
