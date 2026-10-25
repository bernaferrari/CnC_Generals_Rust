use super::*;
#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn scoped_native_passes_keep_callback_positions_and_borrow_only_the_driving_frame() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(Default::default()).unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let first = send.clone();
    renderer.enqueue_pre_scene_callback(move |_| {
        first.send("pre-before").unwrap();
        Ok(())
    });
    renderer.enqueue_scoped_pre_scene_pass(ScopedNativePass::TerrainGround);
    let second = send.clone();
    renderer.enqueue_pre_scene_callback(move |_| {
        second.send("pre-after").unwrap();
        Ok(())
    });
    let third = send.clone();
    renderer.enqueue_post_frame_callback(move |_| {
        third.send("post-before").unwrap();
        Ok(())
    });
    renderer.enqueue_scoped_post_frame_pass(ScopedNativePass::TerrainWater);
    let fourth = send.clone();
    renderer.enqueue_post_frame_callback(move |_| {
        fourth.send("post-after").unwrap();
        Ok(())
    });
    let mut native_count = 0;
    let mut scoped = |kind, _: &mut ww3d_engine::RenderFrame| {
        native_count += 1;
        send.send(match kind {
            ScopedNativePass::TerrainGround => "ground",
            ScopedNativePass::TerrainWater => "water",
            ScopedNativePass::ClientEffects => "effects",
        })
        .unwrap();
        Ok(())
    };
    renderer.begin_frame().unwrap();
    renderer
        .end_frame_with_scoped_native_passes(false, None, Some(&mut scoped))
        .unwrap();
    assert_eq!(
        recv.try_iter().collect::<Vec<_>>(),
        [
            "pre-before",
            "ground",
            "pre-after",
            "post-before",
            "water",
            "post-after"
        ]
    );
    renderer.begin_frame().unwrap();
    renderer
        .end_frame_with_scoped_native_passes(false, None, Some(&mut scoped))
        .unwrap();
    assert!(recv.try_iter().next().is_none());
    renderer.enqueue_scoped_pre_scene_pass(ScopedNativePass::TerrainGround);
    renderer.enqueue_scoped_post_frame_pass(ScopedNativePass::TerrainWater);
    renderer.begin_frame().unwrap();
    renderer
        .end_frame_with_scoped_native_passes(true, None, Some(&mut scoped))
        .unwrap();
    assert!(
        recv.try_iter().next().is_none(),
        "retained scene discards both markers"
    );
    drop(scoped);
    assert_eq!(
        native_count, 2,
        "scoped handler never escapes its driving frame"
    );
    renderer.enqueue_scoped_pre_scene_pass(ScopedNativePass::TerrainGround);
    renderer.begin_frame().unwrap();
    assert!(
        renderer.end_frame().is_err(),
        "required native pass has no substitute global owner"
    );
    assert!(!ww3d_engine::frame_is_active());
    drop(renderer);
    ww3d_engine::shutdown().unwrap();
}
