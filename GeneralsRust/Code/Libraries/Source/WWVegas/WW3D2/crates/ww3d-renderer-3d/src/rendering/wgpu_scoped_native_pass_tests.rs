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

fn authored_static_mesh(name: &str, level: u32, color: glam::Vec3) -> Arc<MeshClass> {
    let mut mesh = frame_sort_mesh(name, false);
    let model = Arc::get_mut(Arc::get_mut(&mut mesh).unwrap().model.as_mut().unwrap()).unwrap();
    model.sort_level = level;
    let material = Arc::get_mut(model.material_passes[0].vertex_material.as_mut().unwrap()).unwrap();
    material.diffuse = color;
    material.emissive = color;
    material.opacity = 0.5;
    mesh
}

fn static_sort_pixel_readback(renderer: &mut WgpuMainRenderer) -> wgpu::Buffer {
    let buffer = renderer.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("static-sort-public-call-pixel"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let destination = buffer.clone();
    renderer.enqueue_frame_work(FramePhase::Overlay2D, move |frame| {
        let (texture, encoder) = frame.color_texture_and_encoder();
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 8, y: 8, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &destination,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        Ok(())
    });
    buffer
}

fn mapped_static_sort_pixel(renderer: &WgpuMainRenderer, buffer: &wgpu::Buffer) -> [u8; 4] {
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
    renderer.device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().unwrap();
    let pixel = bytes[..4].try_into().unwrap();
    drop(bytes);
    buffer.unmap();
    pixel
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn authored_material_and_instance_opacity_multiply_once_on_gpu() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        color_format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    }).expect("native GPU");
    let mut renderer = WgpuMainRenderer::from_engine(engine, Default::default()).unwrap();
    let mut camera = CameraClass::new();
    camera.set_clip_planes(0.1, 100.0);
    camera.set_view_matrix(glam::Mat4::IDENTITY);
    camera.set_projection_matrix(glam::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0));
    renderer.renderer.lock().unwrap().set_camera(camera);
    for (authored, instance, expected_red) in [
        (1.0, 1.0, 255_i16),
        (0.5, 1.0, 128),
        (1.0, 0.5, 128),
        (0.5, 0.5, 64),
        (0.0, 0.5, 0),
    ] {
        let mut mesh = authored_static_mesh("authored-times-instance", 20, glam::Vec3::X);
        let owned = Arc::get_mut(&mut mesh).unwrap();
        let model = Arc::get_mut(owned.model.as_mut().unwrap()).unwrap();
        Arc::get_mut(model.material_passes[0].vertex_material.as_mut().unwrap()).unwrap().opacity = authored;
        owned.set_presentation_opacity(instance);
        renderer.begin_frame().unwrap();
        renderer.renderer.lock().unwrap().queue_mesh(mesh).unwrap();
        let readback = static_sort_pixel_readback(&mut renderer);
        renderer.end_frame().unwrap();
        assert_eq!(renderer.stats().draw_calls, 1);
        let pixel = mapped_static_sort_pixel(&renderer, &readback);
        assert!((i16::from(pixel[0]) - expected_red).abs() <= 3,
            "authored {authored} × instance {instance} must apply each factor once: {pixel:?}");
        assert_eq!(&pixel[1..3], &[0, 0]);
    }
}

#[test]
#[ignore = "requires a native GPU; run explicitly with --include-ignored"]
fn public_mesh_queue_static_sort_is_owner_local_ordered_and_flushes_once() {
    use crate::rendering::frame_graph::FrameGraphPass;
    use crate::rendering::mesh_system::StaticSortManager;

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        color_format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    }).expect("native GPU");
    let mut first = WgpuMainRenderer::from_engine(engine.clone(), Default::default()).unwrap();
    let mut second = WgpuMainRenderer::from_engine(engine, Default::default()).unwrap();
    for owner in [&mut first, &mut second] {
        let mut camera = CameraClass::new();
        camera.set_clip_planes(0.1, 100.0);
        camera.set_view_matrix(glam::Mat4::IDENTITY);
        camera.set_projection_matrix(glam::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0));
        owner.renderer.lock().unwrap().set_camera(camera);
    }
    let red_high = authored_static_mesh("same-red-id", 30, glam::Vec3::X);
    let green_low = authored_static_mesh("same-green-id", 10, glam::Vec3::Y);
    let red_low = authored_static_mesh("same-red-id", 10, glam::Vec3::X);
    let green_high = authored_static_mesh("same-green-id", 30, glam::Vec3::Y);

    first.begin_frame().unwrap();
    // Reverse authored order deliberately. The peer's queued work must survive
    // this owner's flush and its own later begin_frame.
    first.renderer.lock().unwrap().queue_mesh(Arc::clone(&green_low)).unwrap();
    second.renderer.lock().unwrap().queue_mesh(Arc::clone(&red_low)).unwrap();
    first.renderer.lock().unwrap().queue_mesh(Arc::clone(&red_high)).unwrap();
    second.renderer.lock().unwrap().queue_mesh(Arc::clone(&green_high)).unwrap();
    for (owner, expected) in [
        (&first, [&green_low, &red_high]),
        (&second, [&red_low, &green_high]),
    ] {
        let (entries, levels) = StaticSortManager::snapshot_static_sort_list(&owner.renderer_3d).unwrap();
        assert_eq!(levels, [10, 30]);
        assert_eq!(entries.len(), 2);
        for (entry, mesh) in entries.iter().zip(expected) {
            assert!(Arc::ptr_eq(&entry.mesh_arc().unwrap(), mesh));
        }
        let prepared = owner.renderer.lock().unwrap().frame_graph
            .node_mut(FrameGraphPass::Main).prepare(glam::Vec3::ZERO);
        assert!(prepared.opaque.is_empty());
        assert!(prepared.combined_translucent().is_empty(), "static admission must not also submit a normal draw");
    }
    let first_pixel = static_sort_pixel_readback(&mut first);
    first.end_frame().unwrap();
    assert_eq!(first.stats().meshes_rendered, 2);
    assert_eq!(first.stats().draw_calls, 2);
    assert!(StaticSortManager::snapshot_static_sort_list(&first.renderer_3d).is_none());
    assert!(StaticSortManager::snapshot_static_sort_list(&second.renderer_3d).is_some());
    let pixel = mapped_static_sort_pixel(&first, &first_pixel);
    assert!(u16::from(pixel[1]) > u16::from(pixel[0]) + 20, "lower green bin must draw last: {pixel:?}");

    second.begin_frame().unwrap();
    let second_pixel = static_sort_pixel_readback(&mut second);
    // Drive the actual public render_frame a second time in the same frame.
    // It must not replay the nonzero-level meshes or recurse into admission.
    let receiver = second.renderer_handle();
    second.enqueue_frame_work(FramePhase::StaticSort, move |frame| {
        let mut renderer = receiver.lock().unwrap();
        assert_eq!(renderer.mesh_stats().meshes_rendered, 2);
        renderer.render_frame(frame, None, None)?;
        assert_eq!(renderer.mesh_stats().meshes_rendered, 0);
        assert_eq!(renderer.mesh_stats().draw_calls, 0);
        Ok(())
    });
    second.end_frame().unwrap();
    assert!(StaticSortManager::snapshot_static_sort_list(&second.renderer_3d).is_none());
    let pixel = mapped_static_sort_pixel(&second, &second_pixel);
    assert!(u16::from(pixel[0]) > u16::from(pixel[1]) + 20, "peer's lower red bin must draw last: {pixel:?}");

    first.begin_frame().unwrap();
    first.end_frame().unwrap();
    assert_eq!(first.stats().meshes_rendered, 0, "no static mesh replay next frame");

    // Disabling static lists and entering a flush both take the direct route,
    // without losing the mesh or leaving a second deferred copy behind.
    let mut instance_sorted = frame_sort_mesh("instance-static-override", true);
    Arc::get_mut(&mut instance_sorted).unwrap().set_sort_level(30);
    for during_flush in [false, true] {
        first.begin_frame().unwrap();
        first.ww3d.set_static_sort_lists_enabled(during_flush).unwrap();
        let guard = during_flush.then(|| StaticSortManager::begin_flush(Arc::clone(&first.renderer_3d)));
        first.renderer.lock().unwrap().queue_mesh(Arc::clone(&instance_sorted)).unwrap();
        assert!(StaticSortManager::snapshot_static_sort_list(&first.renderer_3d).is_none());
        drop(guard);
        let receiver = first.renderer_handle();
        first.enqueue_frame_work(FramePhase::StaticSort, move |_| {
            assert_eq!(receiver.lock().unwrap().mesh_render_manager.sorted_batch_count(), 0,
                "an instance static level must bypass triangle sorting too");
            Ok(())
        });
        first.end_frame().unwrap();
        assert_eq!(first.stats().meshes_rendered, 1);
        assert_eq!(first.stats().draw_calls, 1);
    }
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
