use ww3d_renderer_3d::rendering::wgpu_main_renderer::WgpuMainRenderer;

#[test]
#[ignore = "requires a native GPU"]
fn a_real_headless_frame_does_not_claim_a_window_surface_present() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(Default::default()).unwrap();
    assert!(!renderer.last_surface_presented());
    renderer.begin_frame().unwrap();
    assert!(!renderer.last_surface_presented());
    renderer.end_frame_with_scene_policy(true, None).unwrap();
    assert!(!renderer.last_surface_presented());
    renderer.reset_surface_presented();
    assert!(!renderer.last_surface_presented());
    drop(renderer);
    ww3d_engine::shutdown().unwrap();
}
