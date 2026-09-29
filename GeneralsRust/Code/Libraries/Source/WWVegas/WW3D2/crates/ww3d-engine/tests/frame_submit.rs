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

#[test]
fn repeated_screenshot_readbacks_retire_metal_command_buffers() {
    let _guard = EngineTestGuard::new();
    init_headless_blocking(EngineConfig {
        width: 16,
        height: 16,
        ..EngineConfig::default()
    })
    .expect("screenshot stress init");

    let directory =
        std::env::temp_dir().join(format!("ww3d_screenshot_stress_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).expect("create screenshot stress directory");
    let device = device().expect("device");

    // Every frame queues a GPU copy and MAP_READ callback. Crossing 4096
    // requests distinguishes readback lifetime from the ordinary frame submit
    // and discarded-encoder stress tests above.
    for frame_index in 0..4_100 {
        // The PNG writer runs on a background thread. Distinct paths keep its
        // atomic temporary-file promotion independent of GPU readback cadence.
        let path = directory.join(format!("{frame_index}.png"));
        make_screenshot(&path).expect("queue stress screenshot");
        let frame = begin_render().expect("screenshot stress begin");
        end_render(frame).expect("screenshot stress end");
        assert_eq!(last_frame_submit_count(), 1, "frame {frame_index}");
        assert_eq!(last_out_of_frame_submit_count(), 1, "frame {frame_index}");
        if frame_index % 64 == 63 {
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("screenshot batch submissions complete");
        }
    }

    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("screenshot stress submissions complete");
    // The map callbacks are drained during end_render, then PNG writing runs
    // on background threads. Wait for the files before removing the test dir.
    for _ in 0..4 {
        let frame = begin_render().expect("readback drain begin");
        end_render(frame).expect("readback drain end");
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let completed = std::fs::read_dir(&directory)
            .expect("list stress screenshots")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
            .count();
        if completed == 4_100 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "only {completed}/4100 screenshot writes completed"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::remove_dir_all(directory).expect("remove screenshot stress directory");
}
