use super::*;

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn repeated_native_post_frame_failures_finish_and_submit_each_frame() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(Default::default()).unwrap();
    let mut failures = 0;
    let mut native = |kind, _: &mut ww3d_engine::RenderFrame| {
        assert_eq!(kind, ScopedNativePass::ClientEffects);
        failures += 1;
        Err(RendererError::InvalidOperation(
            "expected missing effect frame".into(),
        ))
    };
    let initial_submits = ww3d_engine::total_owned_submits(&ww3d_engine::frame_submit().expect("fs").lock());
    for _ in 0..4100 {
        renderer.enqueue_scoped_post_frame_pass(ScopedNativePass::ClientEffects);
        renderer.begin_frame().unwrap();
        renderer
            .end_frame_with_scoped_native_passes(false, None, Some(&mut native))
            .unwrap();
        assert!(!ww3d_engine::frame_is_active(&ww3d_engine::frame_submit().expect("fs").lock()));
    }
    drop(native);
    assert_eq!(failures, 4100);
    assert_eq!(ww3d_engine::total_owned_submits(&ww3d_engine::frame_submit().expect("fs").lock()) - initial_submits, 4100);
    drop(renderer);
    ww3d_engine::shutdown().unwrap();
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn failed_frame_acquisition_discards_the_unsubmitted_callbacks() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut owner = WgpuMainRenderer::from_engine(Default::default()).unwrap();
    let mut peer = WgpuMainRenderer::from_engine(Default::default()).unwrap();
    owner.enqueue_scoped_post_frame_pass(ScopedNativePass::TerrainWater);
    owner.begin_frame().unwrap();
    owner.discard_queued_frame_callbacks();
    assert_eq!(
        owner.post_frame_callbacks.len(),
        1,
        "an active frame owns its queued work"
    );
    let active_start = owner.frame_start;
    assert!(owner.begin_frame().is_err());
    assert_eq!(
        owner.frame_start, active_start,
        "the active frame keeps its timing"
    );
    assert_eq!(
        owner.post_frame_callbacks.len(),
        1,
        "the active frame keeps its work"
    );
    for _ in 0..64 {
        peer.enqueue_scoped_pre_scene_pass(ScopedNativePass::TerrainGround);
        peer.enqueue_scoped_post_frame_pass(ScopedNativePass::ClientEffects);
        assert!(peer.begin_frame().is_err());
        assert!(peer.pre_scene_callbacks.is_empty());
        assert!(peer.post_frame_callbacks.is_empty());
    }
    let mut observed = Vec::new();
    let mut record = |kind, _: &mut ww3d_engine::RenderFrame| {
        observed.push(kind);
        Ok(())
    };
    owner
        .end_frame_with_scoped_native_passes(false, None, Some(&mut record))
        .unwrap();
    peer.enqueue_scoped_post_frame_pass(ScopedNativePass::ClientEffects);
    peer.begin_frame().unwrap();
    peer.end_frame_with_scoped_native_passes(false, None, Some(&mut record))
        .unwrap();
    drop(record);
    assert_eq!(
        observed,
        [
            ScopedNativePass::TerrainWater,
            ScopedNativePass::ClientEffects
        ]
    );
    drop(peer);
    drop(owner);
    ww3d_engine::shutdown().unwrap();
}
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
    assert!(!ww3d_engine::frame_is_active(&ww3d_engine::frame_submit().expect("fs").lock()));
    drop(renderer);
    ww3d_engine::shutdown().unwrap();
}
