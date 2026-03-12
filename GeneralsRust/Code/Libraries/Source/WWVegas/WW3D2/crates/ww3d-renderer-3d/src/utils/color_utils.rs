//! Color conversion and manipulation utilities.
//!
//! Provides color format conversions, blending, and colorspace operations
//! ported from the C++ WW3D rendering system.

use glam::Vec3;

/// RGBA color represented as 4 bytes.
pub type ColorRGBA = [u8; 4];

/// RGBA color represented as 4 floats (0.0 - 1.0 range).
pub type ColorRGBAF = [f32; 4];

/// Converts ARGB format (0xAARRGGBB) to RGBA bytes.
///
/// # C++ Reference
/// Common pattern in DX8 rendering code
#[inline]
pub fn argb_to_rgba(argb: u32) -> ColorRGBA {
    [
        ((argb >> 16) & 0xFF) as u8, // R
        ((argb >> 8) & 0xFF) as u8,  // G
        (argb & 0xFF) as u8,         // B
        ((argb >> 24) & 0xFF) as u8, // A
    ]
}

/// Converts RGBA bytes to ARGB format (0xAARRGGBB).
///
/// # C++ Reference
/// DirectX color format conversion
#[inline]
pub fn rgba_to_argb(rgba: ColorRGBA) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[2] as u32)
}

/// Converts ABGR format (0xAABBGGRR) to RGBA bytes.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Convert_To_ABGR`
#[inline]
pub fn abgr_to_rgba(abgr: u32) -> ColorRGBA {
    [
        (abgr & 0xFF) as u8,         // R
        ((abgr >> 8) & 0xFF) as u8,  // G
        ((abgr >> 16) & 0xFF) as u8, // B
        ((abgr >> 24) & 0xFF) as u8, // A
    ]
}

/// Converts RGBA bytes to ABGR format (0xAABBGGRR).
#[inline]
pub fn rgba_to_abgr(rgba: ColorRGBA) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[2] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[0] as u32)
}

/// Converts floating point RGBA (0.0-1.0) to packed u32 ARGB.
///
/// # C++ Reference
/// DirectX color packing
#[inline]
pub fn float_color_to_u32(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let r_byte = (r.clamp(0.0, 1.0) * 255.0) as u32;
    let g_byte = (g.clamp(0.0, 1.0) * 255.0) as u32;
    let b_byte = (b.clamp(0.0, 1.0) * 255.0) as u32;
    let a_byte = (a.clamp(0.0, 1.0) * 255.0) as u32;

    (a_byte << 24) | (r_byte << 16) | (g_byte << 8) | b_byte
}

/// Converts packed u32 ARGB to floating point RGBA (0.0-1.0).
///
/// # C++ Reference
/// DirectX color unpacking
#[inline]
pub fn u32_to_float_color(color: u32) -> ColorRGBAF {
    [
        ((color >> 16) & 0xFF) as f32 / 255.0, // R
        ((color >> 8) & 0xFF) as f32 / 255.0,  // G
        (color & 0xFF) as f32 / 255.0,         // B
        ((color >> 24) & 0xFF) as f32 / 255.0, // A
    ]
}

/// Linear interpolation between two colors.
///
/// # C++ Reference
/// Color blending operations
#[inline]
pub fn blend_colors(c1: u32, c2: u32, factor: f32) -> u32 {
    let f1 = u32_to_float_color(c1);
    let f2 = u32_to_float_color(c2);

    let t = factor.clamp(0.0, 1.0);
    float_color_to_u32(
        f1[0] + (f2[0] - f1[0]) * t,
        f1[1] + (f2[1] - f1[1]) * t,
        f1[2] + (f2[2] - f1[2]) * t,
        f1[3] + (f2[3] - f1[3]) * t,
    )
}

/// Multiplies two colors component-wise.
///
/// # C++ Reference
/// Texture/lighting multiplication
#[inline]
pub fn multiply_colors(c1: u32, c2: u32) -> u32 {
    let f1 = u32_to_float_color(c1);
    let f2 = u32_to_float_color(c2);

    float_color_to_u32(f1[0] * f2[0], f1[1] * f2[1], f1[2] * f2[2], f1[3] * f2[3])
}

/// Adds two colors component-wise (clamped to 1.0).
///
/// # C++ Reference
/// Additive blending
#[inline]
pub fn add_colors(c1: u32, c2: u32) -> u32 {
    let f1 = u32_to_float_color(c1);
    let f2 = u32_to_float_color(c2);

    float_color_to_u32(
        (f1[0] + f2[0]).min(1.0),
        (f1[1] + f2[1]).min(1.0),
        (f1[2] + f2[2]).min(1.0),
        (f1[3] + f2[3]).min(1.0),
    )
}

/// Converts RGB to HSV color space.
///
/// Returns (h, s, v) where:
/// - h: Hue in degrees [0, 360) (negative if undefined/monochrome)
/// - s: Saturation [0, 1]
/// - v: Value [0, 1]
///
/// # C++ Reference
/// `colorspace.h`: `RGB_To_HSV`
pub fn rgb_to_hsv(rgb: Vec3) -> Vec3 {
    let max = rgb.x.max(rgb.y).max(rgb.z);
    let min = rgb.x.min(rgb.y).min(rgb.z);

    // Value
    let v = max;

    // Saturation
    let s = if max != 0.0 { (max - min) / max } else { 0.0 };

    // Hue
    let h = if s == 0.0 {
        -1.0 // Undefined (monochrome)
    } else {
        let delta = max - min;
        let h_temp = if rgb.x == max {
            (rgb.y - rgb.z) / delta
        } else if rgb.y == max {
            2.0 + (rgb.z - rgb.x) / delta
        } else {
            4.0 + (rgb.x - rgb.y) / delta
        };

        let mut h_degrees = h_temp * 60.0;
        if h_degrees < 0.0 {
            h_degrees += 360.0;
        }
        h_degrees
    };

    Vec3::new(h, s, v)
}

/// Converts HSV to RGB color space.
///
/// Input (h, s, v) where:
/// - h: Hue in degrees [0, 360)
/// - s: Saturation [0, 1]
/// - v: Value [0, 1]
///
/// # C++ Reference
/// `colorspace.h`: `HSV_To_RGB`
pub fn hsv_to_rgb(hsv: Vec3) -> Vec3 {
    if hsv.y == 0.0 {
        // Monochrome
        return Vec3::new(hsv.z, hsv.z, hsv.z);
    }

    let mut h = hsv.x;
    if h == 360.0 {
        h = 0.0;
    }

    h /= 60.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = hsv.z * (1.0 - hsv.y);
    let q = hsv.z * (1.0 - (hsv.y * f));
    let t = hsv.z * (1.0 - (hsv.y * (1.0 - f)));

    match i {
        0 => Vec3::new(hsv.z, t, p),
        1 => Vec3::new(q, hsv.z, p),
        2 => Vec3::new(p, hsv.z, t),
        3 => Vec3::new(p, q, hsv.z),
        4 => Vec3::new(t, p, hsv.z),
        _ => Vec3::new(hsv.z, p, q),
    }
}

/// Recolors an RGB value by shifting HSV components.
///
/// # Arguments
/// * `rgb` - Original RGB color (0.0-1.0)
/// * `hsv_shift` - HSV shift to apply (h in degrees, s and v in 0.0-1.0)
///
/// # C++ Reference
/// `colorspace.h`: `Recolor`
pub fn recolor(rgb: Vec3, hsv_shift: Vec3) -> Vec3 {
    let mut hsv = rgb_to_hsv(rgb);

    // If hue is undefined (monochrome), only shift value
    if hsv.x < 0.0 {
        hsv += Vec3::new(0.0, 0.0, hsv_shift.z);
    } else {
        hsv += hsv_shift;
    }

    // Angular modulo for hue
    if hsv.x < 0.0 {
        hsv.x += 360.0;
    }
    if hsv.x > 360.0 {
        hsv.x -= 360.0;
    }

    // Clamp saturation and value
    hsv.y = hsv.y.clamp(0.0, 1.0);
    hsv.z = hsv.z.clamp(0.0, 1.0);

    hsv_to_rgb(hsv)
}

/// Recolors a packed ARGB color by shifting HSV components.
///
/// # C++ Reference
/// `colorspace.h`: `Recolor` (u32 overload)
pub fn recolor_u32(argb: u32, hsv_shift: Vec3) -> u32 {
    let rgba_f = u32_to_float_color(argb);
    let rgb = Vec3::new(rgba_f[0], rgba_f[1], rgba_f[2]);
    let recolored = recolor(rgb, hsv_shift);

    float_color_to_u32(recolored.x, recolored.y, recolored.z, rgba_f[3])
}

/// Premultiplies RGB by alpha.
///
/// Used for alpha blending optimization.
#[inline]
pub fn premultiply_alpha(color: u32) -> u32 {
    let rgba = u32_to_float_color(color);
    float_color_to_u32(
        rgba[0] * rgba[3],
        rgba[1] * rgba[3],
        rgba[2] * rgba[3],
        rgba[3],
    )
}

/// Un-premultiplies RGB by alpha.
#[inline]
pub fn unpremultiply_alpha(color: u32) -> u32 {
    let rgba = u32_to_float_color(color);
    if rgba[3] == 0.0 {
        return 0;
    }

    let inv_alpha = 1.0 / rgba[3];
    float_color_to_u32(
        rgba[0] * inv_alpha,
        rgba[1] * inv_alpha,
        rgba[2] * inv_alpha,
        rgba[3],
    )
}

/// Converts linear RGB to sRGB (gamma correction).
#[inline]
pub fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts sRGB to linear RGB.
#[inline]
pub fn srgb_to_linear(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts linear RGB color to sRGB.
pub fn vec3_linear_to_srgb(linear: Vec3) -> Vec3 {
    Vec3::new(
        linear_to_srgb(linear.x),
        linear_to_srgb(linear.y),
        linear_to_srgb(linear.z),
    )
}

/// Converts sRGB color to linear RGB.
pub fn vec3_srgb_to_linear(srgb: Vec3) -> Vec3 {
    Vec3::new(
        srgb_to_linear(srgb.x),
        srgb_to_linear(srgb.y),
        srgb_to_linear(srgb.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    #[test]
    fn test_argb_rgba_conversion() {
        let argb: u32 = 0xFF8040C0;
        let rgba = argb_to_rgba(argb);
        assert_eq!(rgba, [0x80, 0x40, 0xC0, 0xFF]);

        let converted_back = rgba_to_argb(rgba);
        assert_eq!(converted_back, argb);
    }

    #[test]
    fn test_float_color_conversion() {
        let color = float_color_to_u32(1.0, 0.5, 0.25, 0.75);
        let rgba_f = u32_to_float_color(color);

        assert!((rgba_f[0] - 1.0).abs() < EPSILON);
        assert!((rgba_f[1] - 0.5).abs() < 0.01); // More tolerance due to byte quantization
        assert!((rgba_f[2] - 0.25).abs() < 0.01);
        assert!((rgba_f[3] - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_blend_colors() {
        let white = 0xFFFFFFFF;
        let black = 0xFF000000;

        let gray = blend_colors(white, black, 0.5);
        let rgba = u32_to_float_color(gray);

        assert!((rgba[0] - 0.5).abs() < 0.01);
        assert!((rgba[1] - 0.5).abs() < 0.01);
        assert!((rgba[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_multiply_colors() {
        let full = float_color_to_u32(1.0, 1.0, 1.0, 1.0);
        let half = float_color_to_u32(0.5, 0.5, 0.5, 0.5);

        let result = multiply_colors(full, half);
        let rgba = u32_to_float_color(result);

        assert!((rgba[0] - 0.5).abs() < 0.01);
        assert!((rgba[1] - 0.5).abs() < 0.01);
        assert!((rgba[2] - 0.5).abs() < 0.01);
        assert!((rgba[3] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_rgb_hsv_conversion() {
        let red = Vec3::new(1.0, 0.0, 0.0);
        let hsv = rgb_to_hsv(red);

        assert!((hsv.x - 0.0).abs() < EPSILON); // Hue = 0 (red)
        assert!((hsv.y - 1.0).abs() < EPSILON); // Saturation = 1
        assert!((hsv.z - 1.0).abs() < EPSILON); // Value = 1

        let rgb_back = hsv_to_rgb(hsv);
        assert!((rgb_back - red).length() < EPSILON);
    }

    #[test]
    fn test_rgb_hsv_gray() {
        let gray = Vec3::new(0.5, 0.5, 0.5);
        let hsv = rgb_to_hsv(gray);

        assert!(hsv.x < 0.0); // Hue undefined (monochrome)
        assert!((hsv.y - 0.0).abs() < EPSILON); // Saturation = 0
        assert!((hsv.z - 0.5).abs() < EPSILON); // Value = 0.5

        let rgb_back = hsv_to_rgb(hsv);
        assert!((rgb_back - gray).length() < EPSILON);
    }

    #[test]
    fn test_recolor() {
        let blue = Vec3::new(0.0, 0.0, 1.0);
        let shift = Vec3::new(120.0, 0.0, 0.0); // Shift hue by 120 degrees (blue 240° + 120° = 360°/red)

        let recolored = recolor(blue, shift);

        // Should be approximately red (blue hue 240° + 120° shift = 360°/0°)
        assert!(recolored.x > 0.9);
        assert!(recolored.y < 0.1);
        assert!(recolored.z < 0.1);
    }

    #[test]
    fn test_premultiply_alpha() {
        let color = float_color_to_u32(1.0, 0.5, 0.25, 0.5);
        let premul = premultiply_alpha(color);
        let rgba = u32_to_float_color(premul);

        assert!((rgba[0] - 0.5).abs() < 0.01); // 1.0 * 0.5
        assert!((rgba[1] - 0.25).abs() < 0.01); // 0.5 * 0.5
        assert!((rgba[2] - 0.125).abs() < 0.02); // 0.25 * 0.5
        assert!((rgba[3] - 0.5).abs() < 0.01); // Alpha unchanged
    }

    #[test]
    fn test_gamma_correction() {
        let linear = 0.5f32;
        let srgb = linear_to_srgb(linear);
        let back_to_linear = srgb_to_linear(srgb);

        assert!((back_to_linear - linear).abs() < EPSILON);
    }
}
