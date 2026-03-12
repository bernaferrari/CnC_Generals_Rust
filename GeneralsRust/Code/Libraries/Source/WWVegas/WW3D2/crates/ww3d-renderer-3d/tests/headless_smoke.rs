use glam::Vec3;
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use ww3d_core::ww3d::WW3D;
use ww3d_engine::{self, EngineConfig};
use ww3d_renderer_3d::rendering::wgpu_main_renderer::WgpuMainRenderer;
use ww3d_renderer_3d::rendering::wgpu_renderer::wgpu_wrapper::WgpuWrapper;

lazy_static! {
    static ref ENGINE_TEST_MUTEX: Mutex<()> = Mutex::new(());
}

#[test]
fn headless_wrapper_supports_basic_lifecycle() {
    let mut wrapper = WgpuWrapper::new_headless((32, 32), wgpu::TextureFormat::Bgra8Unorm)
        .expect("failed to construct headless wrapper");

    wrapper.set_ambient_color(Vec3::new(0.1, 0.2, 0.3));
    wrapper.set_z_planes(0.1, 5000.0);
    wrapper.set_z_bias(2);
    wrapper.set_fog(true, Vec3::new(0.3, 0.4, 0.5), 5.0, 500.0);

    assert!(wrapper.fog_settings().is_some());
    assert_eq!(wrapper.z_planes(), (0.1, 5000.0));
    assert_eq!(wrapper.z_bias(), 2);

    {
        let mut frame = wrapper.begin_frame().expect("begin_frame failed");
        frame.clear(true, true, Vec3::splat(0.0), 1.0, 1.0, 0);
        frame.finish().expect("finish frame");
    }

    // Ensure the texture manager is reachable for callers needing legacy hooks.
    let _ = wrapper.texture_manager();
}

#[test]
fn ww3d_stats_bridge_updates_after_frame() {
    let _guard = ENGINE_TEST_MUTEX.lock().expect("engine mutex poisoned");

    // Ensure any previous renderer/engine state is torn down between tests.
    WW3D::unregister_renderer();
    let _ = ww3d_engine::shutdown();

    let backend = WgpuWrapper::new_headless((32, 32), wgpu::TextureFormat::Bgra8Unorm)
        .expect("failed to construct headless wrapper");
    let mut renderer =
        WgpuMainRenderer::from_backend(Arc::new(Mutex::new(backend)), Default::default());

    renderer.begin_frame().expect("begin frame");
    renderer.end_frame().expect("end frame");
    let stats = renderer.snapshot_stats();
    let core_stats = WW3D::current_frame_stats().expect("stats bridge");
    assert_eq!(core_stats.draw_calls, stats.draw_calls);
    assert_eq!(core_stats.triangles_rendered, stats.triangles_rendered);
    WW3D::unregister_renderer();
}

#[test]
fn engine_headless_flow() {
    let _guard = ENGINE_TEST_MUTEX.lock().expect("engine mutex poisoned");

    WW3D::unregister_renderer();
    let _ = ww3d_engine::shutdown();

    let mut config = EngineConfig::default();
    config.width = 32;
    config.height = 32;
    config.enable_depth = false;
    ww3d_engine::init_headless_blocking(config).expect("init headless engine");

    {
        let mut renderer =
            WgpuMainRenderer::from_engine(Default::default()).expect("renderer from engine");
        renderer.begin_frame().expect("begin frame");
        renderer.end_frame().expect("end frame");
    }

    ww3d_engine::shutdown().expect("shutdown engine");
}
