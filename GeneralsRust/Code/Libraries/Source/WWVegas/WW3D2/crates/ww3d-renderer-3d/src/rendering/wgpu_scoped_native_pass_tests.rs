use super::*;

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn repeated_native_post_frame_failures_finish_and_submit_each_frame() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut failures = 0;
    let mut native = |kind, _: &mut ww3d_engine::RenderFrame| {
        assert_eq!(kind, ScopedNativePass::ClientEffects);
        failures += 1;
        Err(RendererError::InvalidOperation(
            "expected missing effect frame".into(),
        ))
    };
    let initial_submits = ww3d_engine::total_owned_submits(&engine.lock().frame_submit_state().lock());
    for _ in 0..4100 {
        renderer.enqueue_scoped_native_pass(FramePhase::Particles, ScopedNativePass::ClientEffects);
        renderer.begin_frame().unwrap();
        renderer
            .end_frame_with_scoped_native_passes(false, None, Some(&mut native))
            .unwrap();
        assert!(!ww3d_engine::frame_is_active(&engine.lock().frame_submit_state().lock()));
    }
    drop(native);
    assert_eq!(failures, 4100);
    assert_eq!(ww3d_engine::total_owned_submits(&engine.lock().frame_submit_state().lock()) - initial_submits, 4100);
    drop(renderer);
    drop(engine);
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn failed_frame_acquisition_discards_the_unsubmitted_callbacks() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut owner = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut peer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    owner.enqueue_scoped_native_pass(FramePhase::StaticSort, ScopedNativePass::TerrainWater);
    owner.begin_frame().unwrap();
    owner.discard_queued_frame_callbacks();
    assert_eq!(
        owner.queued_frame_work(FramePhase::StaticSort),
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
        owner.queued_frame_work(FramePhase::StaticSort),
        1,
        "the active frame keeps its work"
    );
    for _ in 0..64 {
        peer.enqueue_scoped_native_pass(FramePhase::Terrain, ScopedNativePass::TerrainGround);
        peer.enqueue_scoped_native_pass(FramePhase::Particles, ScopedNativePass::ClientEffects);
        assert!(peer.begin_frame().is_err());
        assert!(!peer.has_queued_frame_work());
    }
    let mut observed = Vec::new();
    let mut record = |kind, _: &mut ww3d_engine::RenderFrame| {
        observed.push(kind);
        Ok(())
    };
    owner
        .end_frame_with_scoped_native_passes(false, None, Some(&mut record))
        .unwrap();
    peer.enqueue_scoped_native_pass(FramePhase::Particles, ScopedNativePass::ClientEffects);
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
    drop(engine);
}
#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn scoped_native_passes_keep_callback_positions_and_borrow_only_the_driving_frame() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let first = send.clone();
    renderer.enqueue_frame_work(FramePhase::Terrain, move |_| {
        first.send("pre-before").unwrap();
        Ok(())
    });
    renderer.enqueue_scoped_native_pass(FramePhase::Terrain, ScopedNativePass::TerrainGround);
    let second = send.clone();
    renderer.enqueue_frame_work(FramePhase::Terrain, move |_| {
        second.send("pre-after").unwrap();
        Ok(())
    });
    let third = send.clone();
    renderer.enqueue_frame_work(FramePhase::StaticSort, move |_| {
        third.send("post-before").unwrap();
        Ok(())
    });
    renderer.enqueue_scoped_native_pass(FramePhase::StaticSort, ScopedNativePass::TerrainWater);
    let fourth = send.clone();
    renderer.enqueue_frame_work(FramePhase::StaticSort, move |_| {
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
    renderer.enqueue_scoped_native_pass(FramePhase::Terrain, ScopedNativePass::TerrainGround);
    renderer.enqueue_scoped_native_pass(FramePhase::StaticSort, ScopedNativePass::TerrainWater);
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
    renderer.enqueue_scoped_native_pass(FramePhase::Terrain, ScopedNativePass::TerrainGround);
    renderer.begin_frame().unwrap();
    assert!(
        renderer.end_frame().is_err(),
        "required native pass has no substitute global owner"
    );
    assert!(!ww3d_engine::frame_is_active(&engine.lock().frame_submit_state().lock()));
    drop(renderer);
    drop(engine);
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn frame_work_records_in_schedule_order_not_submission_order() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    // Submit in reverse schedule order; recording must follow FRAME_SCHEDULE.
    for phase in FRAME_SCHEDULE.iter().rev().filter(|phase| !phase.is_builtin()) {
        let send = send.clone();
        let phase = *phase;
        renderer.enqueue_frame_work(phase, move |frame| {
            send.send((phase, frame.frame_index())).unwrap();
            Ok(())
        });
    }
    renderer.begin_frame().unwrap();
    renderer.end_frame().unwrap();
    let recorded: Vec<FramePhase> = recv.try_iter().map(|(phase, _)| phase).collect();
    let expected: Vec<FramePhase> = FRAME_SCHEDULE
        .iter()
        .copied()
        .filter(|phase| !phase.is_builtin())
        .collect();
    assert_eq!(recorded, expected);
    drop(renderer);
    drop(engine);
}
