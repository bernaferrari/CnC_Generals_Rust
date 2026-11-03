//! Given a headless WW3D frame, when several subsystems record work, then
//! `end_render` issues exactly one `queue.submit`.

use std::sync::{Mutex, OnceLock};
use ww3d_engine::*;

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn dummy_buffer(device: &wgpu::Device, label: &str) -> wgpu::CommandBuffer {
    device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
        .finish()
}

#[test]
fn n_subsystem_records_and_empty_frame_each_submit_once() {
    let _guard = test_lock();
    let engine = init_headless_blocking(EngineConfig::default()).expect("headless init");

    {
        let mut eng = engine.lock();
        let empty = eng.begin_render().expect("begin_render empty");
        eng.end_render(empty).expect("end_render empty");
        assert_eq!(last_frame_submit_count(&eng.frame_submit_state().lock()), 1);
    }

    let (device, queue) = {
        let eng = engine.lock();
        (eng.device(), eng.queue())
    };
    let frame = engine.lock().begin_render().expect("begin_render");
    assert!(frame_is_active(&engine.lock().frame_submit_state().lock()));

    for label in ["ghost", "laser", "particles", "up"] {
        let fs = engine.lock().frame_submit_state();
        submit_recorded(
            &fs.lock(),
            &queue,
            FrameCommandPhase::Overlay,
            dummy_buffer(&device, label),
            OutOfFrameReason::StandaloneW3dRenderer,
        );
    }
    {
        let fs = engine.lock().frame_submit_state();
        submit_recorded(
            &fs.lock(),
            &queue,
            FrameCommandPhase::Upload,
            dummy_buffer(&device, "upload"),
            OutOfFrameReason::StandaloneW3dRenderer,
        );
    }
    engine.lock().end_render(frame).expect("end_render");
    assert!(!frame_is_active(&engine.lock().frame_submit_state().lock()));
    assert_eq!(last_frame_submit_count(&engine.lock().frame_submit_state().lock()), 1);
    let _ = last_out_of_frame_submit_count(&engine.lock().frame_submit_state().lock());
}

#[test]
fn two_shared_engines_keep_independent_session_state() {
    let _guard = test_lock();
    // GPU device authority may reuse one adapter/device; Engine session state
    // (surface size, frame_submit) must still be per SharedEngine handle.
    let a = init_headless_blocking(EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .expect("engine a");
    let b = init_headless_blocking(EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("engine b");
    assert_eq!(a.lock().surface_size(), (16, 16));
    assert_eq!(b.lock().surface_size(), (32, 32));
    a.lock().resize(64, 48);
    assert_eq!(a.lock().surface_size(), (64, 48));
    assert_eq!(b.lock().surface_size(), (32, 32));
    // Distinct frame-submit counters.
    {
        let mut ea = a.lock();
        let frame = ea.begin_render().expect("a begin");
        ea.end_render(frame).expect("a end");
        assert_eq!(last_frame_submit_count(&ea.frame_submit_state().lock()), 1);
    }
    assert_eq!(last_frame_submit_count(&b.lock().frame_submit_state().lock()), 0);
}
