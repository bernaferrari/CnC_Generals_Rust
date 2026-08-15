//! Given a headless WW3D frame, when several subsystems record work, then
//! `end_render` issues exactly one `queue.submit`.

use std::sync::{Mutex, OnceLock};
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
        reset_submit_debug();
        Self { _lock: lock }
    }
}

impl Drop for EngineTestGuard {
    fn drop(&mut self) {
        let _ = shutdown();
        reset_submit_debug();
    }
}

fn dummy_buffer(device: &wgpu::Device, label: &str) -> wgpu::CommandBuffer {
    device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
        .finish()
}

fn ensure_headless() {
    match init_headless_blocking(EngineConfig::default()) {
        Ok(()) | Err(EngineError::AlreadyInitialised) => {}
        Err(err) => panic!("headless init: {err:?}"),
    }
}

#[test]
fn n_subsystem_records_and_empty_frame_each_submit_once() {
    let _guard = EngineTestGuard::new();
    ensure_headless();

    let empty = begin_render().expect("begin_render empty");
    end_render(empty).expect("end_render empty");
    assert_eq!(last_frame_submit_count(), 1);

    let device = device().expect("device");
    let queue = queue().expect("queue");
    let frame = begin_render().expect("begin_render");
    assert!(frame_is_active());

    for label in ["ghost", "laser", "particles", "up"] {
        submit_recorded(
            &queue,
            FrameCommandPhase::Overlay,
            dummy_buffer(&device, label),
            OutOfFrameReason::StandaloneW3dRenderer,
        );
    }
    submit_recorded(
        &queue,
        FrameCommandPhase::Upload,
        dummy_buffer(&device, "upload"),
        OutOfFrameReason::StandaloneW3dRenderer,
    );

    end_render(frame).expect("end_render");

    assert_eq!(last_frame_submit_count(), 1);
    assert_eq!(last_out_of_frame_submit_count(), 0);
    assert!(!frame_is_active());
}
