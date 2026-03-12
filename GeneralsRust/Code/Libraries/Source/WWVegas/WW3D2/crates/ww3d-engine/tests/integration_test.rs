//! Integration tests for the WW3D engine with all subsystems

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use ww3d_engine::*;

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EngineTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EngineTestGuard {
    fn new() -> Self {
        let lock = TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("ww3d-engine test lock poisoned");
        let _ = shutdown();
        Self { _lock: lock }
    }
}

impl Drop for EngineTestGuard {
    fn drop(&mut self) {
        let _ = shutdown();
    }
}

/// Test basic engine initialization and shutdown
#[test]
fn test_engine_init_shutdown() {
    let _guard = EngineTestGuard::new();
    // Initialize headless engine
    let config = EngineConfig::default();
    let result = init_headless_blocking(config);
    assert!(result.is_ok(), "Engine initialization failed: {:?}", result);

    // Check that the engine is initialized
    let info_result = adapter_info();
    assert!(info_result.is_ok(), "Failed to get adapter info");

    // Shutdown
    let shutdown_result = shutdown();
    assert!(
        shutdown_result.is_ok(),
        "Shutdown failed: {:?}",
        shutdown_result
    );
}

/// Test frame timing calculation
#[test]
fn test_frame_timing() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Get timing
    let timing_result = timing();
    assert!(timing_result.is_ok());

    let timing = timing_result.unwrap();
    assert_eq!(timing.frame_number, 0);
    assert!(timing.delta_time > Duration::ZERO);

    shutdown().unwrap();
}

/// Test subsystem registration
#[test]
fn test_subsystem_registration() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Create a simple test subsystem
    struct TestSubsystem {
        update_count: usize,
    }

    impl Subsystem for TestSubsystem {
        fn update(&mut self, _timing: &FrameTiming) {
            self.update_count += 1;
        }

        fn name(&self) -> &str {
            "TestSubsystem"
        }
    }

    let test_subsystem = Box::new(TestSubsystem { update_count: 0 });
    let register_result = register_subsystem(test_subsystem);
    assert!(register_result.is_ok());

    // Update engine
    let update_result = update();
    assert!(update_result.is_ok());

    shutdown().unwrap();
}

/// Test input event handling
#[test]
fn test_input_events() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Create test input handler
    struct TestInputHandler {
        events_received: Vec<String>,
    }

    impl InputHandler for TestInputHandler {
        fn handle_input(&mut self, event: &InputEvent) {
            match event {
                InputEvent::KeyPressed { key } => {
                    self.events_received.push(format!("KeyPressed: {}", key));
                }
                InputEvent::MouseMoved { x, y } => {
                    self.events_received
                        .push(format!("MouseMoved: {}, {}", x, y));
                }
                _ => {}
            }
        }
    }

    let handler = Box::new(TestInputHandler {
        events_received: Vec::new(),
    });

    let set_handler_result = set_input_handler(handler);
    assert!(set_handler_result.is_ok());

    // Queue some input events
    let key_event = InputEvent::KeyPressed {
        key: "A".to_string(),
    };
    let queue_result = queue_input(key_event);
    assert!(queue_result.is_ok());

    let mouse_event = InputEvent::MouseMoved { x: 100.0, y: 200.0 };
    let queue_result2 = queue_input(mouse_event);
    assert!(queue_result2.is_ok());

    // Update to process events
    update().unwrap();

    shutdown().unwrap();
}

/// Test render frame lifecycle
#[test]
fn test_render_frame_lifecycle() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Begin render
    let frame_result = begin_render();
    assert!(
        frame_result.is_ok(),
        "Begin render failed: {:?}",
        frame_result
    );

    let mut frame = frame_result.unwrap();

    // Check frame properties
    assert!(frame.device().limits().max_texture_dimension_2d > 0);
    assert_eq!(frame.frame_index(), 1);

    // Check timing is available
    assert!(frame.timing.delta_time > Duration::ZERO);

    // Simulate recording some render commands
    {
        let _encoder = frame.encoder();
        // In a real test, we would record actual render passes here
    }

    // End render
    let end_result = end_render(frame);
    assert!(end_result.is_ok(), "End render failed: {:?}", end_result);

    shutdown().unwrap();
}

/// Test multiple frames
#[test]
fn test_multiple_frames() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    for i in 0..5 {
        // Update subsystems
        update().unwrap();

        // Render frame
        let frame = begin_render().unwrap();
        assert_eq!(frame.frame_index(), (i + 1) as u64);
        end_render(frame).unwrap();
    }

    // Check FPS is calculated
    let fps_result = fps();
    assert!(fps_result.is_ok());
    let fps_value = fps_result.unwrap();
    // FPS should be positive after multiple frames
    assert!(fps_value >= 0.0);

    shutdown().unwrap();
}

/// Test device and queue access
#[test]
fn test_device_queue_access() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    let device_result = device();
    assert!(device_result.is_ok());

    let device = device_result.unwrap();
    assert!(Arc::strong_count(&device) >= 1);

    let queue_result = queue();
    assert!(queue_result.is_ok());

    let queue = queue_result.unwrap();
    assert!(Arc::strong_count(&queue) >= 1);

    shutdown().unwrap();
}

/// Test surface resize
#[test]
fn test_resize() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    let initial_size = (config.width, config.height);
    init_headless_blocking(config).unwrap();

    // Initial size
    let size1 = surface_size().unwrap();
    assert_eq!(size1, initial_size);

    // Resize
    let resize_result = resize(1920, 1080);
    assert!(resize_result.is_ok());

    // Check new size
    let size2 = surface_size().unwrap();
    assert_eq!(size2, (1920, 1080));

    shutdown().unwrap();
}

/// Test screenshot functionality
#[test]
fn test_screenshot() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Create temp directory for screenshot
    let temp_dir = std::env::temp_dir().join("ww3d_test_screenshots");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let screenshot_path = temp_dir.join("test_screenshot.png");

    // Queue screenshot
    let screenshot_result = make_screenshot(&screenshot_path);
    assert!(screenshot_result.is_ok());

    // Render a frame to trigger screenshot
    let frame = begin_render().unwrap();
    end_render(frame).unwrap();

    // Check that screenshot was created
    assert!(screenshot_path.exists(), "Screenshot file was not created");

    // Clean up
    std::fs::remove_file(&screenshot_path).ok();
    std::fs::remove_dir(&temp_dir).ok();

    shutdown().unwrap();
}

/// Test subsystems accessor
#[test]
fn test_subsystems_access() {
    let _guard = EngineTestGuard::new();
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Test read access
    let result = with_subsystems(|subsystems| subsystems.subsystem_count());

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // No subsystems registered yet

    // Test write access
    struct DummySubsystem;
    impl Subsystem for DummySubsystem {
        fn update(&mut self, _timing: &FrameTiming) {}
    }

    register_subsystem(Box::new(DummySubsystem)).unwrap();

    let result2 = with_subsystems(|subsystems| subsystems.subsystem_count());

    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), 1); // One subsystem registered

    shutdown().unwrap();
}

/// Test error handling
#[test]
fn test_error_handling() {
    let _guard = EngineTestGuard::new();
    // Try to use engine before initialization
    let begin_result = begin_render();
    assert!(begin_result.is_err());

    if let Err(e) = begin_result {
        assert!(matches!(e, EngineError::NotInitialised));
    }

    // Initialize
    let config = EngineConfig::default();
    init_headless_blocking(config).unwrap();

    // Try to initialize again (should fail)
    let config2 = EngineConfig::default();
    let init_result = init_headless_blocking(config2);
    assert!(init_result.is_err());

    if let Err(e) = init_result {
        assert!(matches!(e, EngineError::AlreadyInitialised));
    }

    // Try to begin render twice
    let frame = begin_render().unwrap();
    let second_begin = begin_render();
    assert!(second_begin.is_err());

    if let Err(e) = second_begin {
        assert!(matches!(e, EngineError::FrameInProgress));
    }

    end_render(frame).unwrap();
    shutdown().unwrap();
}
