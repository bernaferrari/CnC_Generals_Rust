use glam::Vec3;
use lazy_static::lazy_static;
use std::sync::Mutex;
use ww3d_engine::{self, EngineConfig};
use ww3d_renderer_3d::rendering::wgpu_main_renderer::WgpuMainRenderer;

lazy_static! {
    static ref ENGINE_TEST_MUTEX: Mutex<()> = Mutex::new(());
}

#[test]
fn engine_headless_flow() {
    let _guard = ENGINE_TEST_MUTEX.lock().expect("engine mutex poisoned");

    // owned Ww3dContext drops with renderer
    let _ = Ok::<(), ww3d_engine::EngineError>(()) /* engine drop */;

    let mut config = EngineConfig::default();
    config.width = 32;
    config.height = 32;
    config.enable_depth = false;
    let engine = ww3d_engine::init_headless_blocking(config).expect("init headless engine");

    {
        let mut renderer =
            WgpuMainRenderer::from_engine(engine.clone(), Default::default()).expect("renderer from engine");
        renderer.begin_frame().expect("begin frame");
        renderer.end_frame().expect("end frame");
    }

    Ok::<(), ww3d_engine::EngineError>(()) /* engine drop */.expect("shutdown engine");
}
