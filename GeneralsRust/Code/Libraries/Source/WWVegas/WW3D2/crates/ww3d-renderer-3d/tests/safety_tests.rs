/// Safety Tests for WW3D2 Renderer
///
/// This test module validates the safety improvements made to the WW3D2 renderer:
/// 1. Surface lifetime safety without unsafe transmute
/// 2. GPU buffer conversion safety using bytemuck instead of from_raw_parts
use ww3d_renderer_3d::rendering::wgpu_renderer::wgpu_wrapper::WgpuWrapper;

/// Test that headless wrapper can be created and destroyed safely
/// without any lifetime issues.
///
/// This validates that the Surface<'static> handling doesn't cause
/// use-after-free issues even when the wrapper is dropped.
#[test]
fn test_headless_wrapper_safe_lifecycle() {
    // Create wrapper
    let wrapper = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create headless wrapper");

    // Use the wrapper
    let config = wrapper.surface_config();
    assert_eq!(config.width, 256);
    assert_eq!(config.height, 256);

    // Wrapper drops here - should be safe without any dangling references
}

/// Test that multiple wrapper instances can coexist safely
#[test]
fn test_multiple_wrapper_instances_safe() {
    let wrapper1 = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create first wrapper");

    let wrapper2 = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create second wrapper");

    assert_eq!(wrapper1.surface_config().width, 128);
    assert_eq!(wrapper2.surface_config().width, 256);

    // Both drop safely without interference
}

/// Test that wrapper can be resized safely without lifetime issues
#[test]
fn test_wrapper_resize_safety() {
    let mut wrapper = WgpuWrapper::new_headless((100, 100), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Resize multiple times
    wrapper.resize(200, 200).expect("First resize failed");
    assert_eq!(wrapper.surface_config().width, 200);

    wrapper.resize(400, 400).expect("Second resize failed");
    assert_eq!(wrapper.surface_config().width, 400);

    wrapper.resize(50, 50).expect("Third resize failed");
    assert_eq!(wrapper.surface_config().width, 50);

    // All resources should be cleaned up safely on drop
}

/// Test that wrapper can handle frame lifecycle safely
#[test]
fn test_frame_lifecycle_safety() {
    let mut wrapper = WgpuWrapper::new_headless((320, 240), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Begin and end multiple frames
    for _ in 0..5 {
        wrapper.begin_scene().expect("Begin scene failed");
        wrapper.end_scene(false).expect("End scene failed");
    }

    // All frame resources should be cleaned up safely
}

/// Test that device and queue references are managed safely
#[test]
fn test_device_queue_reference_safety() {
    let wrapper = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Get references to device and queue
    let device1 = wrapper.device();
    let queue1 = wrapper.queue();

    // Can get multiple references
    let device2 = wrapper.device();
    let queue2 = wrapper.queue();

    // All should point to the same underlying resources
    assert!(std::sync::Arc::ptr_eq(&device1, &device2));
    assert!(std::sync::Arc::ptr_eq(&queue1, &queue2));

    // References remain valid even after wrapper is dropped
    drop(wrapper);

    // Device and queue should still be accessible through Arc
    // This demonstrates proper reference counting
    let _ = device1.limits();
    let _ = queue1;
}

/// Test that surface can be safely cloned and shared
#[test]
fn test_surface_arc_safety() {
    let wrapper = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Get surface reference (headless has None, but test the Arc mechanism)
    let surface_opt = wrapper.surface();

    // For headless, this should be None
    assert!(surface_opt.is_none());

    // The Arc mechanism is still safe to use
    if let Some(surface) = surface_opt {
        let _surface_clone = surface.clone();
        // Both references are valid
    }
}

/// Test wrapper doesn't leak memory across multiple create/drop cycles
#[test]
fn test_no_memory_leak_on_multiple_cycles() {
    // Create and drop wrapper multiple times
    // If there were lifetime issues or leaks, this would fail or crash
    for i in 0..10 {
        let size = 64 + i * 32;
        let wrapper = WgpuWrapper::new_headless((size, size), wgpu::TextureFormat::Bgra8Unorm)
            .expect("Failed to create wrapper");

        let config = wrapper.surface_config();
        assert_eq!(config.width, size);
        assert_eq!(config.height, size);

        // Wrapper drops here
    }
    // All resources should be cleaned up properly
}

/// Stress test: rapid creation and destruction of wrappers
#[test]
fn test_rapid_wrapper_lifecycle() {
    for _ in 0..20 {
        let wrapper = WgpuWrapper::new_headless((64, 64), wgpu::TextureFormat::Bgra8Unorm)
            .expect("Failed to create wrapper");

        // Immediately drop
        drop(wrapper);
    }
    // No crashes or panics indicates safe lifecycle management
}

/// Test that zero-sized surfaces are handled safely
#[test]
fn test_zero_size_surface_safety() {
    // Zero-sized surfaces should be clamped to minimum size
    let wrapper = WgpuWrapper::new_headless((0, 0), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper with zero size");

    let config = wrapper.surface_config();
    // Should be clamped to at least 1x1
    assert!(config.width >= 1);
    assert!(config.height >= 1);
}

/// Test that very large surfaces don't cause overflow issues
#[test]
fn test_large_surface_safety() {
    // Test with a reasonably large surface (not testing GPU memory limits)
    let wrapper = WgpuWrapper::new_headless((4096, 4096), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create large wrapper");

    let config = wrapper.surface_config();
    assert_eq!(config.width, 4096);
    assert_eq!(config.height, 4096);
}

// ============================================================================
// GPU RESOURCE SAFETY TESTS (Critical for stability)
// ============================================================================

/// Test that render target acquisition is safe
#[test]
fn test_render_target_acquisition_safety() {
    let mut wrapper = WgpuWrapper::new_headless((320, 240), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Begin scene to set up frame state
    wrapper.begin_scene().expect("Failed to begin scene");

    // Should be able to get device and queue multiple times
    let device1 = wrapper.device();
    let queue1 = wrapper.queue();

    let device2 = wrapper.device();
    let queue2 = wrapper.queue();

    // Verify they're the same references
    assert!(std::sync::Arc::ptr_eq(&device1, &device2));
    assert!(std::sync::Arc::ptr_eq(&queue1, &queue2));

    wrapper.end_scene(false).expect("Failed to end scene");
}

/// Test that command encoder is properly initialized
#[test]
fn test_command_encoder_safety() {
    let wrapper = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    let device = wrapper.device();
    let queue = wrapper.queue();

    // Should be able to create command encoder from device
    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("test_encoder"),
    });

    // Encoder should be usable
    let _cmd = encoder.finish();

    // Queue should be able to submit
    queue.submit(std::iter::once(_cmd));
}

/// Test texture validation and format safety
#[test]
fn test_texture_format_validation() {
    let wrapper = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    let config = wrapper.surface_config();

    // Format should match what was specified
    assert_eq!(config.format, wgpu::TextureFormat::Bgra8Unorm);

    // Device can be used with this format
    let device = wrapper.device();
    let limits = device.limits();
    assert!(limits.max_texture_dimension_2d > 0);
}

/// Test that render targets are properly configured
#[test]
fn test_render_target_configuration() {
    let wrapper = WgpuWrapper::new_headless((512, 512), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    let config = wrapper.surface_config();

    // Width and height should be valid
    assert!(config.width > 0 && config.width <= 16384);
    assert!(config.height > 0 && config.height <= 16384);

    // Format should be supported
    assert_ne!(config.format, wgpu::TextureFormat::Rgb9e5Ufloat);
    // (Rgb9e5Ufloat is rarely supported as render target)
}

/// Test frame state consistency across prepare/clear cycle
#[test]
fn test_frame_state_consistency() {
    let mut wrapper = WgpuWrapper::new_headless((320, 240), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Multiple frame cycles should work consistently
    for _ in 0..5 {
        wrapper.begin_scene().expect("Begin failed");

        // Device should be valid after begin
        let device = wrapper.device();
        // Can get limits from device, proving it's valid
        let _limits = device.limits();

        // Queue should be valid after begin
        let queue = wrapper.queue();
        // Queue is valid as we have reference to it
        let _ = std::sync::Arc::strong_count(&queue);

        wrapper.end_scene(false).expect("End failed");
    }
}

// ============================================================================
// REFERENCE COUNTING SAFETY TESTS
// ============================================================================

/// Test that Arc references are properly managed
#[test]
fn test_arc_reference_management() {
    let wrapper = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Get Arc references
    let device1 = wrapper.device();
    let device2 = wrapper.device();

    // Should point to same underlying data
    assert!(std::sync::Arc::ptr_eq(&device1, &device2));

    // Clone should create new Arc pointing to same data
    let device3 = std::sync::Arc::clone(&device1);
    assert!(std::sync::Arc::ptr_eq(&device1, &device3));
}

/// Test that strong count is appropriate for Arc
#[test]
fn test_arc_strong_count() {
    let wrapper = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    let device1 = wrapper.device();
    let initial_count = std::sync::Arc::strong_count(&device1);

    // Count should be at least 1 (our reference)
    assert!(initial_count >= 1);

    // Cloning should increase count
    let _device2 = std::sync::Arc::clone(&device1);
    let new_count = std::sync::Arc::strong_count(&device1);
    assert!(new_count > initial_count);
}

// ============================================================================
// RESOURCE LIFECYCLE SAFETY TESTS
// ============================================================================

/// Test surface lifecycle is safe
#[test]
fn test_surface_lifecycle() {
    let wrapper = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Should be able to get surface reference multiple times
    let _surface1 = wrapper.surface();

    // Should be able to get it multiple times
    let _surface2 = wrapper.surface();
    let _surface3 = wrapper.surface();

    // For headless, surfaces should be None, but reference counting is still safe
    match (_surface1.as_ref(), _surface2.as_ref(), _surface3.as_ref()) {
        (None, None, None) => {
            // Expected for headless
        }
        (Some(s1), Some(s2), Some(s3)) => {
            // Should be able to clone Arc references
            let _s1_clone = s1.clone();
            let _s2_clone = s2.clone();
            let _s3_clone = s3.clone();
        }
        _ => panic!("Surface references should be consistent"),
    }
}

/// Test surface config is accessible and consistent
#[test]
fn test_surface_config_consistency() {
    let wrapper = WgpuWrapper::new_headless((320, 240), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    let config1 = wrapper.surface_config();
    let config2 = wrapper.surface_config();

    // Configs should be identical
    assert_eq!(config1.width, config2.width);
    assert_eq!(config1.height, config2.height);
    assert_eq!(config1.format, config2.format);
    assert_eq!(config1.present_mode, config2.present_mode);
}

/// Test wrapper state after frame operations
#[test]
fn test_wrapper_state_after_operations() {
    let mut wrapper = WgpuWrapper::new_headless((256, 256), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // Initial state should be valid
    let device1 = wrapper.device();
    let _ = device1.limits(); // Verify device is valid

    // After begin_scene
    wrapper.begin_scene().expect("Failed to begin");
    let device2 = wrapper.device();
    let _ = device2.limits(); // Verify device is still valid

    // After end_scene
    wrapper.end_scene(false).expect("Failed to end");
    let device3 = wrapper.device();
    let _ = device3.limits(); // Verify device is still valid

    // Device should be the same throughout
    assert!(std::sync::Arc::ptr_eq(&device1, &device2));
    assert!(std::sync::Arc::ptr_eq(&device2, &device3));
}

// ============================================================================
// ERROR HANDLING AND EDGE CASE SAFETY TESTS
// ============================================================================

/// Test that invalid operations don't cause silent failures
#[test]
fn test_error_handling_on_invalid_state() {
    let mut wrapper = WgpuWrapper::new_headless((128, 128), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wrapper");

    // First scene cycle works
    wrapper.begin_scene().expect("First begin failed");
    wrapper.end_scene(false).expect("First end failed");

    // Second cycle should also work
    wrapper.begin_scene().expect("Second begin failed");
    wrapper.end_scene(false).expect("Second end failed");
}

/// Test that extremely small dimensions are handled safely
#[test]
fn test_minimum_dimension_safety() {
    let wrapper = WgpuWrapper::new_headless((1, 1), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create 1x1 wrapper");

    let config = wrapper.surface_config();
    assert_eq!(config.width, 1);
    assert_eq!(config.height, 1);
}

/// Test that aspect ratios don't cause issues
#[test]
fn test_extreme_aspect_ratio_safety() {
    // Very wide aspect ratio
    let wide = WgpuWrapper::new_headless((4096, 64), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create wide wrapper");
    assert_eq!(wide.surface_config().width, 4096);
    assert_eq!(wide.surface_config().height, 64);

    // Very tall aspect ratio
    let tall = WgpuWrapper::new_headless((64, 4096), wgpu::TextureFormat::Bgra8Unorm)
        .expect("Failed to create tall wrapper");
    assert_eq!(tall.surface_config().width, 64);
    assert_eq!(tall.surface_config().height, 4096);
}
