// FILE: w3_d_convert.rs
// Ported from C++ W3DConvert.h/.cpp

/// Convert W3D logical screen coordinates (-1..1, origin at center) to pixel screen coords.
///
/// C++: W3DLogicalScreenToPixelScreen
pub fn w3d_logical_screen_to_pixel_screen(
    log_x: f32,
    log_y: f32,
    screen_width: i32,
    screen_height: i32,
) -> (i32, i32) {
    let screen_x = ((screen_width as f32 * (log_x + 1.0)) / 2.0).trunc() as i32;
    let screen_y = ((screen_height as f32 * (-log_y + 1.0)) / 2.0).trunc() as i32;
    (screen_x, screen_y)
}

/// Convert W3D logical screen coordinates to pixel screen coords (out parameters).
pub fn w3d_logical_screen_to_pixel_screen_out(
    log_x: f32,
    log_y: f32,
    screen_x: &mut i32,
    screen_y: &mut i32,
    screen_width: i32,
    screen_height: i32,
) {
    let (x, y) = w3d_logical_screen_to_pixel_screen(log_x, log_y, screen_width, screen_height);
    *screen_x = x;
    *screen_y = y;
}

/// Convert pixel screen coordinates to W3D logical screen coords (-1..1).
///
/// C++: PixelScreenToW3DLogicalScreen
pub fn pixel_screen_to_w3d_logical_screen(
    screen_x: i32,
    screen_y: i32,
    screen_width: i32,
    screen_height: i32,
) -> (f32, f32) {
    let log_x = ((2.0 * screen_x as f32) / screen_width as f32) - 1.0;
    let log_y = -(((2.0 * screen_y as f32) / screen_height as f32) - 1.0);
    (log_x, log_y)
}

/// Convert pixel screen coordinates to W3D logical screen coords (out parameters).
pub fn pixel_screen_to_w3d_logical_screen_out(
    screen_x: i32,
    screen_y: i32,
    log_x: &mut f32,
    log_y: &mut f32,
    screen_width: i32,
    screen_height: i32,
) {
    let (x, y) =
        pixel_screen_to_w3d_logical_screen(screen_x, screen_y, screen_width, screen_height);
    *log_x = x;
    *log_y = y;
}
