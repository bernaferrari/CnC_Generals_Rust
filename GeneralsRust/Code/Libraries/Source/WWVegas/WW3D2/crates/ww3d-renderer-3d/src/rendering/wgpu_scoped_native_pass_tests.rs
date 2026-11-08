use super::*;

fn frame_sort_mesh(name: &str, triangle_sort: bool) -> Arc<MeshClass> {
    use crate::material_system::{MaterialPassClass, VertexMaterialClass};
    use crate::rendering::mesh_system::{MeshGeometryClass, MeshModelClass};
    use ww3d_core::w3d_format::{W3dTriangleStruct, W3dVectorStruct};

    let mut model = MeshModelClass::new(name);
    model.vertices = [(-1.0, -1.0, -3.0), (1.0, -1.0, -3.0), (0.0, 1.0, -3.0)]
        .into_iter()
        .map(|(x, y, z)| W3dVectorStruct { x, y, z })
        .collect();
    model.triangles = vec![W3dTriangleStruct {
        vindex: [0, 1, 2],
        attributes: 0,
        normal: W3dVectorStruct { x: 0.0, y: 0.0, z: 1.0 },
        distance: 0.0,
    }];
    model.vertex_count = 3;
    model.index_count = 3;
    model.set_flag(MeshGeometryClass::SORT, triangle_sort);
    let mut material = VertexMaterialClass::new("frame-sort");
    material.diffuse = glam::Vec3::ONE;
    material.specular = glam::Vec3::ZERO;
    let mut pass = MaterialPassClass::new();
    pass.vertex_material = Some(Arc::new(material));
    pass.shader = crate::rendering::shader_system::shader::ShaderClass::get_alpha_shader();
    model.material_passes.push(pass);
    let mut mesh = MeshClass::new();
    mesh.model = Some(Arc::new(model));
    Arc::new(mesh)
}

// Both exit policies bypass the ordinary static-sort flush. Keep a second
// renderer's pending mesh alive while abandoning the first one's submission,
// then drive real GPU draws to prove only the receiving frame was discarded.
#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn failed_or_skipped_scene_discards_only_its_frame_sorting() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).expect("native GPU");
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut peer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mesh = frame_sort_mesh("abandoned", false);
    let peer_mesh = frame_sort_mesh("peer", false);
    let state = engine.lock().frame_submit_state();

    for skip_scene in [false, true] {
        renderer.begin_frame().unwrap();
        renderer.ww3d.add_to_static_sort_list(Arc::clone(&mesh), 1).unwrap();
        peer.ww3d.add_to_static_sort_list(Arc::clone(&peer_mesh), 1).unwrap();
        renderer.enqueue_frame_work(FramePhase::Terrain, |_| {
            Err(RendererError::InvalidOperation("expected terrain failure".into()))
        });
        let before = ww3d_engine::total_owned_submits(&state.lock());
        let result = renderer.end_frame_with_scene_policy(skip_scene, None);
        if skip_scene {
            result.unwrap();
        } else {
            assert!(format!("{:?}", result.unwrap_err()).contains("expected terrain failure"));
        }
        assert_eq!(ww3d_engine::total_owned_submits(&state.lock()), before + 1);
        assert!(!ww3d_engine::frame_is_active(&state.lock()));
        assert!(mesh_system::StaticSortManager::snapshot_static_sort_list(&renderer.renderer_3d).is_none());
        assert!(mesh_system::StaticSortManager::snapshot_static_sort_list(&peer.renderer_3d).is_some());

        renderer.begin_frame().unwrap();
        renderer.end_frame().unwrap();
        assert_eq!(renderer.stats().meshes_rendered, 0, "frame N+1 must not draw frame N's mesh");
        assert_eq!(renderer.stats().draw_calls, 0);
        peer.begin_frame().unwrap();
        peer.end_frame().unwrap();
        assert_eq!(peer.stats().meshes_rendered, 1, "the peer still draws its own mesh");
        assert!(peer.stats().draw_calls > 0);

        // New commands are for the next receiving frame. A stray end without
        // begin must not discard them.
        renderer.ww3d.add_to_static_sort_list(Arc::clone(&mesh), 1).unwrap();
        assert!(renderer.end_frame().is_err());
        renderer.begin_frame().unwrap();
        renderer.end_frame().unwrap();
        assert_eq!(renderer.stats().meshes_rendered, 1);
    }
    engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

#[test]
#[ignore = "requires a native GPU supporting two headless devices; run explicitly with --include-ignored"]
fn abandoned_scene_discards_already_recorded_triangle_batches() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).expect("native GPU");
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let peer_engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).expect("peer native GPU");
    let mut peer = WgpuMainRenderer::from_engine(peer_engine.clone(), Default::default()).unwrap();
    let peer_mesh = frame_sort_mesh("peer-triangle-batches", true);
    let mesh = frame_sort_mesh("recorded-before-failure", true);
    for skip_scene in [false, true] {
        peer.begin_frame().unwrap();
        peer.renderer.lock().unwrap().queue_mesh(Arc::clone(&peer_mesh)).unwrap();
        let mut peer_frame = peer.pending_frame.take().unwrap();
        peer.record_mesh_flush(&mut peer_frame, Some(wgpu::Color::BLACK)).unwrap();
        peer.pending_frame = Some(peer_frame);
        let peer_batches = peer.renderer.lock().unwrap().mesh_render_manager.sorted_batch_count();
        assert!(peer_batches > 0);
        renderer.begin_frame().unwrap();
        renderer.renderer.lock().unwrap().queue_mesh(Arc::clone(&mesh)).unwrap();
        if skip_scene {
            let mut frame = renderer.pending_frame.take().unwrap();
            renderer.record_mesh_flush(&mut frame, Some(wgpu::Color::BLACK)).unwrap();
            renderer.pending_frame = Some(frame);
            assert!(renderer.renderer.lock().unwrap().mesh_render_manager.sorted_batch_count() > 0);
        } else {
            let handle = renderer.renderer_handle();
            renderer.enqueue_frame_work(FramePhase::Terrain, move |frame| {
                // Fail after real mesh recording has produced triangle batches,
                // before the main renderer can reach its Sorted phase.
                let mut scene = handle.lock().unwrap();
                scene.render_frame(frame, Some(wgpu::Color::BLACK), None)?;
                assert!(scene.mesh_render_manager.sorted_batch_count() > 0);
                Err(RendererError::InvalidOperation("expected scene failure".into()))
            });
        }
        let result = renderer.end_frame_with_scene_policy(skip_scene, None);
        if skip_scene {
            result.unwrap();
        } else {
            assert!(format!("{:?}", result.unwrap_err()).contains("expected scene failure"));
        }
        assert_eq!(renderer.renderer.lock().unwrap().mesh_render_manager.sorted_batch_count(), 0);
        assert_eq!(peer.renderer.lock().unwrap().mesh_render_manager.sorted_batch_count(), peer_batches);
        assert!(peer.has_active_frame());
        peer.end_frame().unwrap();
        assert_eq!(peer.renderer.lock().unwrap().mesh_render_manager.sorted_batch_count(), 0);
        renderer.begin_frame().unwrap();
        renderer.end_frame().unwrap();
        assert_eq!(renderer.stats().draw_calls, 0);
    }
    engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
    peer_engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

// This exercises native wgpu submission, not a simulated callback queue. It
// needs an adapter that can create two headless devices; on Metal it also
// crosses the historical command-buffer exhaustion threshold. Periodic waits
// bound legitimate in-flight GPU work, rather than relying on driver speed.
#[test]
#[ignore = "requires a native GPU supporting two headless devices; run explicitly with --include-ignored"]
fn repeated_completion_errors_submit_and_do_not_disturb_a_peer_world() {
    let config = ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    };
    let engine = ww3d_engine::init_headless_blocking(config).expect("first GPU device");
    let peer_engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).expect("second GPU device");
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut peer = WgpuMainRenderer::from_engine(peer_engine.clone(), Default::default()).unwrap();
    let state = engine.lock().frame_submit_state();
    let peer_state = peer_engine.lock().frame_submit_state();
    let initial = ww3d_engine::total_owned_submits(&state.lock());
    let peer_initial = ww3d_engine::total_owned_submits(&peer_state.lock());
    let (send, recv) = std::sync::mpsc::channel();
    let mut completions = 0;
    for index in 0..4100 {
        let before = send.clone();
        renderer.enqueue_frame_work(FramePhase::Overlay2D, move |_| {
            before.send(0).unwrap();
            Err(RendererError::InvalidOperation("expected post work failure".into()))
        });
        let after = send.clone();
        renderer.enqueue_frame_work(FramePhase::Overlay2D, move |_| {
            after.send(1).unwrap();
            Ok(())
        });
        let peer_active = index % 32 == 0;
        if peer_active {
            peer.begin_frame().unwrap();
        }
        renderer.begin_frame().unwrap();
        let mut complete = |_: &mut ww3d_engine::RenderFrame| {
            completions += 1;
            Err(RendererError::InvalidOperation("expected completion failure".into()))
        };
        let err = renderer.end_frame_with_post_render(Some(&mut complete)).unwrap_err();
        assert!(format!("{err:?}").contains("expected completion failure"));
        assert_eq!(recv.try_iter().collect::<Vec<_>>(), [0, 1]);
        assert!(!ww3d_engine::frame_is_active(&state.lock()));
        assert_eq!(ww3d_engine::frame_is_active(&peer_state.lock()), peer_active);
        assert_eq!(ww3d_engine::total_owned_submits(&state.lock()) - initial, index + 1);
        if peer_active {
            peer.end_frame().unwrap();
            engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
            peer_engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
    assert_eq!(completions, 4100);
    assert_eq!(ww3d_engine::total_owned_submits(&peer_state.lock()) - peer_initial, 129);
    engine.lock().device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn dropping_an_acquired_renderer_finishes_only_its_own_frame() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).unwrap();
    let mut owner = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut peer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let state = engine.lock().frame_submit_state();
    let initial = ww3d_engine::total_owned_submits(&state.lock());
    owner.begin_frame().unwrap();
    let runtime = Arc::clone(&owner.renderer_3d);
    let mesh = frame_sort_mesh("drop-owned-frame", false);
    owner.ww3d.add_to_static_sort_list(mesh, 1).unwrap();
    assert!(peer.begin_frame().is_err());
    drop(owner);
    assert!(mesh_system::StaticSortManager::snapshot_static_sort_list(&runtime).is_none());
    assert!(!ww3d_engine::frame_is_active(&state.lock()));
    assert_eq!(ww3d_engine::total_owned_submits(&state.lock()) - initial, 1);
    peer.begin_frame().unwrap();
    peer.end_frame().unwrap();
    assert_eq!(ww3d_engine::total_owned_submits(&state.lock()) - initial, 2);
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn renderer_prepare_failure_releases_its_acquired_frame() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    }).unwrap();
    let mut renderer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mesh = frame_sort_mesh("prepare-failure", false);
    renderer.ww3d.add_to_static_sort_list(mesh, 1).unwrap();
    let handle = renderer.renderer.clone();
    let _ = std::thread::spawn(move || {
        let _guard = handle.lock().unwrap();
        panic!("poison renderer mutex");
    }).join();
    let state = engine.lock().frame_submit_state();
    let initial = ww3d_engine::total_owned_submits(&state.lock());
    assert!(renderer.begin_frame().is_err());
    assert!(mesh_system::StaticSortManager::snapshot_static_sort_list(&renderer.renderer_3d).is_none());
    assert!(!ww3d_engine::frame_is_active(&state.lock()));
    assert_eq!(ww3d_engine::total_owned_submits(&state.lock()) - initial, 1);
    let mut peer = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    peer.begin_frame().unwrap();
    peer.end_frame().unwrap();
}

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
