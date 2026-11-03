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
        Self { _lock: lock }
    }
}

impl Drop for EngineTestGuard {
    fn drop(&mut self) {
        let _ = shutdown();
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

fn with_submit<R>(f: impl FnOnce(&FrameSubmitState) -> R) -> R {
    let fs = frame_submit().expect("frame_submit after init");
    f(&fs.lock())
}

#[test]
fn n_subsystem_records_and_empty_frame_each_submit_once() {
    let _guard = EngineTestGuard::new();
    ensure_headless();

    let empty = begin_render().expect("begin_render empty");
    end_render(empty).expect("end_render empty");
    assert_eq!(with_submit(|s| last_frame_submit_count(s)), 1);

    let device = device().expect("device");
    let queue = queue().expect("queue");
    let frame = begin_render().expect("begin_render");
    assert!(with_submit(|s| frame_is_active(s)));

    for label in ["ghost", "laser", "particles", "up"] {
        with_submit(|s| {
            submit_recorded(
                s,
                &queue,
                FrameCommandPhase::Overlay,
                dummy_buffer(&device, label),
                OutOfFrameReason::StandaloneW3dRenderer,
            )
        });
    }
    with_submit(|s| {
        submit_recorded(
            s,
            &queue,
            FrameCommandPhase::Upload,
            dummy_buffer(&device, "upload"),
            OutOfFrameReason::StandaloneW3dRenderer,
        )
    });
    end_render(frame).expect("end_render");
    assert!(!with_submit(|s| frame_is_active(s)));
    assert_eq!(with_submit(|s| last_frame_submit_count(s)), 1);
    let _ = with_submit(|s| last_out_of_frame_submit_count(s));
}
