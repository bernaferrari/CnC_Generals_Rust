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
    if percent >= 90 || percent <= 0 {
        return color;
    }

    let percent = percent as u32;
    let (r, g, b, a) = game_get_color_components(color);
    let darken = |c: u8| c - ((c as u32 * percent / 100) as u8);
    game_make_color(darken(r), darken(g), darken(b), a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_darken_color_returns_original_outside_cpp_range() {
        let color = game_make_color(100, 150, 200, 255);

        assert_eq!(game_darken_color(color, 0), color);
        assert_eq!(game_darken_color(color, -1), color);
        assert_eq!(game_darken_color(color, 90), color);
        assert_eq!(game_darken_color(color, 100), color);
    }

    #[test]
    fn game_darken_color_uses_cpp_integer_truncation() {
        let color = game_make_color(100, 150, 200, 255);

        assert_eq!(
            game_darken_color(color, 33),
            game_make_color(67, 101, 134, 255)
        );
    }
}
