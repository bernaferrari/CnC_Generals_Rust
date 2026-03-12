//! # C++ API Bindings for VideoDevice
//!
//! Provides C-compatible API for integration with C++ code while maintaining Rust safety.

use super::render_device::Vertex;
use super::{ColorFormat, Resolution, VideoDevice, VideoDeviceConfig, VideoStatistics};
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::slice;
// libc types - using Rust standard types for now
type c_int = i32;
type c_uint = u32;
type size_t = usize;

/// C-compatible error codes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CVideoResult {
    Success = 0,
    ErrorInvalidParameter = -1,
    ErrorInitializationFailed = -2,
    ErrorResourceNotFound = -3,
    ErrorOutOfMemory = -4,
    ErrorUnsupported = -5,
    ErrorInternal = -6,
}

/// C-compatible video device handle (opaque pointer)
#[repr(C)]
pub struct CVideoDevice {
    _private: [u8; 0],
}

/// C-compatible video statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CVideoStatistics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub gpu_memory_usage: u64,
    pub draw_calls: u32,
    pub triangle_count: u32,
    pub gpu_utilization: f32,
    pub textures_loaded: u32,
    pub buffers_allocated: u32,
}

/// C-compatible vertex structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

impl From<CVertex> for Vertex {
    fn from(c_vertex: CVertex) -> Self {
        Self {
            position: c_vertex.position,
            normal: c_vertex.normal,
            tex_coords: c_vertex.tex_coords,
            color: c_vertex.color,
        }
    }
}

impl From<Vertex> for CVertex {
    fn from(vertex: Vertex) -> Self {
        Self {
            position: vertex.position,
            normal: vertex.normal,
            tex_coords: vertex.tex_coords,
            color: vertex.color,
        }
    }
}

impl From<VideoStatistics> for CVideoStatistics {
    fn from(stats: VideoStatistics) -> Self {
        Self {
            fps: stats.fps,
            frame_time_ms: stats.frame_time_ms,
            gpu_memory_usage: stats.gpu_memory_usage,
            draw_calls: stats.draw_calls,
            triangle_count: stats.triangle_count,
            gpu_utilization: stats.gpu_utilization,
            textures_loaded: stats.textures_loaded,
            buffers_allocated: stats.buffers_allocated,
        }
    }
}

/// Global runtime for async operations
static mut TOKIO_RUNTIME: Option<tokio::runtime::Runtime> = None;
static RUNTIME_INIT: std::sync::Once = std::sync::Once::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME_INIT.call_once(|| unsafe {
        TOKIO_RUNTIME = Some(
            tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime for C++ bindings"),
        );
    });

    unsafe { TOKIO_RUNTIME.as_ref().unwrap() }
}

/// Helper macro for error handling in C bindings
macro_rules! c_try {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                tracing::error!("C binding error: {:?}", err);
                return CVideoResult::ErrorInternal;
            }
        }
    };
}

/// Initialize the video system
/// Returns a handle to the video device, or null on failure
#[no_mangle]
pub unsafe extern "C" fn video_device_create() -> *mut CVideoDevice {
    let rt = get_runtime();

    match rt.block_on(async { VideoDevice::new().await }) {
        Ok(device) => {
            let boxed_device = Box::new(device);
            Box::into_raw(boxed_device) as *mut CVideoDevice
        }
        Err(err) => {
            tracing::error!("Failed to create video device: {:?}", err);
            ptr::null_mut()
        }
    }
}

/// Initialize video device with specific parameters
#[no_mangle]
pub unsafe extern "C" fn video_device_initialize(
    device: *mut CVideoDevice,
    width: c_uint,
    height: c_uint,
    fullscreen: c_int,
) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &mut *(device as *mut VideoDevice);
    let rt = get_runtime();

    c_try!(rt.block_on(async { device.initialize(width, height, fullscreen != 0).await }));

    CVideoResult::Success
}

/// Destroy the video device and free resources
#[no_mangle]
pub unsafe extern "C" fn video_device_destroy(device: *mut CVideoDevice) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = Box::from_raw(device as *mut VideoDevice);
    let rt = get_runtime();

    c_try!(rt.block_on(async { device.shutdown().await }));

    // Device is automatically dropped here
    CVideoResult::Success
}

/// Create a texture
#[no_mangle]
pub unsafe extern "C" fn video_device_create_texture(
    device: *mut CVideoDevice,
    width: c_uint,
    height: c_uint,
    format: c_uint,
) -> c_uint {
    if device.is_null() {
        return 0; // Invalid texture ID
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    let color_format = match format {
        0 => ColorFormat::Rgba8,
        1 => ColorFormat::Bgra8,
        2 => ColorFormat::Rgba16,
        3 => ColorFormat::Rgba32Float,
        _ => ColorFormat::Rgba8,
    };

    match rt.block_on(async { device.create_texture(width, height, color_format).await }) {
        Ok(texture_id) => texture_id,
        Err(err) => {
            tracing::error!("Failed to create texture: {:?}", err);
            0
        }
    }
}

/// Set render target
#[no_mangle]
pub unsafe extern "C" fn video_device_set_render_target(
    device: *mut CVideoDevice,
    texture_id: c_uint,
) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    c_try!(rt.block_on(async { device.set_render_target(texture_id).await }));

    CVideoResult::Success
}

/// Draw primitive with vertices and optional indices
#[no_mangle]
pub unsafe extern "C" fn video_device_draw_primitive(
    device: *mut CVideoDevice,
    vertices: *const CVertex,
    vertex_count: c_uint,
    indices: *const u16,
    index_count: c_uint,
) -> CVideoResult {
    if device.is_null() || vertices.is_null() || vertex_count == 0 {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    // Convert C vertices to Rust vertices
    let c_vertices = slice::from_raw_parts(vertices, vertex_count as usize);
    let rust_vertices: Vec<Vertex> = c_vertices.iter().map(|&v| v.into()).collect();

    let rust_indices = if !indices.is_null() && index_count > 0 {
        Some(slice::from_raw_parts(indices, index_count as usize))
    } else {
        None
    };

    c_try!(rt.block_on(async { device.draw_primitive(&rust_vertices, rust_indices).await }));

    CVideoResult::Success
}

/// Present the current frame
#[no_mangle]
pub unsafe extern "C" fn video_device_present(device: *mut CVideoDevice) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    c_try!(rt.block_on(async { device.present().await }));

    CVideoResult::Success
}

/// Get device statistics
#[no_mangle]
pub unsafe extern "C" fn video_device_get_statistics(
    device: *mut CVideoDevice,
    stats: *mut CVideoStatistics,
) -> CVideoResult {
    if device.is_null() || stats.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    match rt.block_on(async { device.get_statistics().await }) {
        video_stats => {
            *stats = video_stats.into();
            CVideoResult::Success
        }
    }
}

/// Set display resolution
#[no_mangle]
pub unsafe extern "C" fn video_device_set_resolution(
    device: *mut CVideoDevice,
    width: c_uint,
    height: c_uint,
) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &mut *(device as *mut VideoDevice);
    let rt = get_runtime();

    let display_mode = super::DisplayMode::new(
        Resolution::new(width, height),
        super::RefreshRate::rate_60hz(),
        32,
    );

    c_try!(rt.block_on(async { device.set_display_mode(display_mode).await }));

    CVideoResult::Success
}

/// Toggle fullscreen mode
#[no_mangle]
pub unsafe extern "C" fn video_device_set_fullscreen(
    device: *mut CVideoDevice,
    fullscreen: c_int,
) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &mut *(device as *mut VideoDevice);
    let rt = get_runtime();

    c_try!(rt.block_on(async { device.set_fullscreen(fullscreen != 0).await }));

    CVideoResult::Success
}

/// Set VSync mode
#[no_mangle]
pub unsafe extern "C" fn video_device_set_vsync(
    device: *mut CVideoDevice,
    vsync_mode: c_uint,
) -> CVideoResult {
    if device.is_null() {
        return CVideoResult::ErrorInvalidParameter;
    }

    let device = &mut *(device as *mut VideoDevice);
    let rt = get_runtime();

    let vsync = match vsync_mode {
        0 => super::VSync::Disabled,
        1 => super::VSync::Enabled,
        2 => super::VSync::Adaptive,
        3 => super::VSync::Fast,
        _ => super::VSync::Enabled,
    };

    c_try!(rt.block_on(async { device.set_vsync(vsync).await }));

    CVideoResult::Success
}

/// Get adapter name (caller must free the returned string)
#[no_mangle]
pub unsafe extern "C" fn video_device_get_adapter_name(device: *mut CVideoDevice) -> *mut c_char {
    if device.is_null() {
        return ptr::null_mut();
    }

    let device = &*(device as *const VideoDevice);
    let adapter_name = &device.get_render_device().get_adapter_info().name;

    match CString::new(adapter_name.as_str()) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string allocated by the video device API
#[no_mangle]
pub unsafe extern "C" fn video_device_free_string(string: *mut c_char) {
    if !string.is_null() {
        let _ = CString::from_raw(string);
    }
}

/// Get error message for the last error (thread-local)
thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
}

#[no_mangle]
pub unsafe extern "C" fn video_device_get_last_error() -> *const c_char {
    LAST_ERROR.with(|error| match error.borrow().as_ref() {
        Some(c_string) => c_string.as_ptr(),
        None => ptr::null(),
    })
}

/// Check if video device is initialized
#[no_mangle]
pub unsafe extern "C" fn video_device_is_initialized(device: *mut CVideoDevice) -> c_int {
    if device.is_null() {
        return 0;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    match rt.block_on(async { device.get_status().await }) {
        Ok(status) => {
            if status.initialized {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

/// Get GPU memory usage in bytes
#[no_mangle]
pub unsafe extern "C" fn video_device_get_gpu_memory_usage(device: *mut CVideoDevice) -> u64 {
    if device.is_null() {
        return 0;
    }

    let device = &*(device as *const VideoDevice);
    let rt = get_runtime();

    rt.block_on(async { device.get_statistics().await.gpu_memory_usage })
}

/// Utility function to set last error message
fn set_last_error(message: &str) {
    LAST_ERROR.with(|error| {
        *error.borrow_mut() = CString::new(message).ok();
    });
}

// Additional utility functions for C++ integration

/// Create vertex from components
#[no_mangle]
pub extern "C" fn create_vertex(
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    norm_x: f32,
    norm_y: f32,
    norm_z: f32,
    tex_u: f32,
    tex_v: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    color_a: f32,
) -> CVertex {
    CVertex {
        position: [pos_x, pos_y, pos_z],
        normal: [norm_x, norm_y, norm_z],
        tex_coords: [tex_u, tex_v],
        color: [color_r, color_g, color_b, color_a],
    }
}

/// Create a simple quad (2 triangles)
#[no_mangle]
pub unsafe extern "C" fn create_quad_vertices(vertices: *mut CVertex, indices: *mut u16) -> c_int {
    if vertices.is_null() || indices.is_null() {
        return -1;
    }

    // Create quad vertices
    let quad_vertices = [
        CVertex {
            position: [-1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tex_coords: [0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        CVertex {
            position: [1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tex_coords: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        CVertex {
            position: [1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tex_coords: [1.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        CVertex {
            position: [-1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tex_coords: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
    ];

    let quad_indices = [0u16, 1, 2, 0, 2, 3];

    // Copy to output buffers
    ptr::copy_nonoverlapping(quad_vertices.as_ptr(), vertices, 4);
    ptr::copy_nonoverlapping(quad_indices.as_ptr(), indices, 6);

    0 // Success
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_conversion() {
        let c_vertex = CVertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            tex_coords: [0.5, 0.5],
            color: [1.0, 0.0, 0.0, 1.0],
        };

        let rust_vertex: Vertex = c_vertex.into();
        assert_eq!(rust_vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(rust_vertex.normal, [0.0, 1.0, 0.0]);
        assert_eq!(rust_vertex.tex_coords, [0.5, 0.5]);
        assert_eq!(rust_vertex.color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(CVideoResult::Success as i32, 0);
        assert!((CVideoResult::ErrorInvalidParameter as i32) < 0);
    }
}
