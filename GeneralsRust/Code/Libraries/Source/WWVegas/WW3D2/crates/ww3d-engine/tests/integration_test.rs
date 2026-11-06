//! Integration tests for the WW3D engine with host-owned SharedEngine handles.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use ww3d_engine::*;

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn test_engine_init_and_drop() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).expect("init");
    let info = engine.lock().adapter_info().clone();
    assert!(!info.name.is_empty());
    drop(engine);
}

#[test]
fn test_frame_timing() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();
    let timing = engine.lock().timing();
    assert_eq!(timing.frame_number, 0);
    assert!(timing.delta_time > Duration::ZERO);
}

#[test]
fn test_subsystem_registration_and_update() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();

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

    engine
        .lock()
        .register_subsystem(Box::new(TestSubsystem { update_count: 0 }));
    engine.lock().update().unwrap();
}

#[test]
fn test_input_events() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();

    struct TestInputHandler {
        events_received: Vec<String>,
    }
    impl InputHandler for TestInputHandler {
        fn handle_input(&mut self, event: &InputEvent) {
            match event {
                InputEvent::KeyPressed { key } => {
                    self.events_received.push(format!("KeyPressed: {key}"));
                }
                InputEvent::MousePressed { button, .. } => {
                    self.events_received
                        .push(format!("MousePressed: {button}"));
                }
                _ => {}
            }
        }
    }

    engine
        .lock()
        .set_input_handler(Box::new(TestInputHandler {
            events_received: Vec::new(),
        }));
    engine.lock().queue_input(InputEvent::KeyPressed {
        key: "A".into(),
    });
    engine.lock().queue_input(InputEvent::MousePressed {
        button: 0,
        x: 1.0,
        y: 2.0,
    });
    engine.lock().update().unwrap();
}

#[test]
fn test_begin_end_render() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();
    let frame = engine.lock().begin_render().unwrap();
    engine.lock().end_render(frame).unwrap();
}

#[test]
fn test_device_and_queue_arcs() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();
    let device = engine.lock().device();
    let queue = engine.lock().queue();
    assert!(Arc::strong_count(&device) >= 1);
    assert!(Arc::strong_count(&queue) >= 1);
}

#[test]
fn test_engine_resize() {
    let _guard = test_lock();
    let config = EngineConfig::default();
    let initial = (config.width, config.height);
    let engine = init_headless_blocking(config).unwrap();
    assert_eq!(engine.lock().surface_size(), initial);
    engine.lock().resize(1920, 1080);
    assert_eq!(engine.lock().surface_size(), (1920, 1080));
}

#[test]
fn test_with_subsystems() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).unwrap();
    engine.lock().register_subsystem(Box::new(DummySubsystem));
    let count = engine.lock().subsystems().subsystem_count();
    assert!(count >= 1);
}

struct DummySubsystem;
impl Subsystem for DummySubsystem {
    fn update(&mut self, _timing: &FrameTiming) {}
    fn name(&self) -> &str {
        "Dummy"
    }
}
