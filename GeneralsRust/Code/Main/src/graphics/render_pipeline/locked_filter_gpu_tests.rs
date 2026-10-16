//! Actual ForwardPass scene/filter/specific-draw ordering witness.
//! CPP W3DView.cpp1525–1574; this is not full rendered-match equivalence.
use super::*;
use game_client::display::shader_filter::ShaderFilterResources;
use game_client::display::view::{FilterMode, FilterType, Vector2};
use game_client::display::view_filter::ViewFilterState;
use gamelogic::common::types::ObjectShroudStatus;

fn triangle(name: &str, color: Vec3) -> Arc<MeshModelClass> {
    let mut model = MeshModelClass::new(name);
    model.vertices = [(-0.25, -0.35), (0.25, -0.35), (0.0, 0.35)]
        .into_iter()
        .map(|(x, y)| W3dVectorStruct { x, y, z: 0.5 })
        .collect();
    for vindex in [[0, 1, 2], [0, 2, 1]] {
        model.triangles.push(W3dTriangleStruct {
            vindex,
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.,
                y: 0.,
                z: 1.,
            },
            distance: 0.,
        });
    }
    model.vertex_count = 3;
    model.index_count = 6;
    let mut material = VertexMaterialClass::new(name);
    material.diffuse = color;
    material.ambient = color;
    material.use_lighting = false;
    let mut pass = MaterialPassClass::new();
    pass.vertex_material = Some(Arc::new(material));
    pass.shader = ShaderClass::get_opaque_shader();
    pass.shader.set_texturing(TexturingType::Disable);
    model.material_passes.push(pass);
    Arc::new(model)
}

fn item(id: u32, name: &str, position: Vec3) -> RenderItem {
    let mut item = RenderItem::new(
        ObjectID(id),
        name.into(),
        0,
        position,
        Mat4::from_translation(position),
        &W3DMaterial::default(),
        RenderPass::ForwardOpaque,
    );
    item.set_frozen_direct_scene_shroud(FrozenDirectSceneShroudRenderState {
        final_status: ObjectShroudStatus::Clear,
        pushes_projected_shroud_pass: false,
    });
    item
}

fn readback_buffer(device: &wgpu::Device, name: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(name),
        size: 256 * 32,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}
fn copy_frame(frame: &mut ww3d_engine::RenderFrame, buffer: &wgpu::Buffer) {
    let (texture, encoder) = frame.color_texture_and_encoder();
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(32),
            },
        },
        wgpu::Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
    );
}
fn pixels(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let bytes = mapped.to_vec();
    drop(mapped);
    buffer.unmap();
    bytes
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 3] {
    let offset = y * 256 + x * 4;
    bytes[offset..offset + 3].try_into().unwrap()
}
fn brightness(rgb: [u8; 3]) -> u8 {
    *rgb.iter().max().unwrap()
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated GPU acceptance"]
fn locked_drawable_is_excluded_from_capture_redrawn_with_post_filter_camera_before_overlay() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("actual ForwardPass GPU witness requires a native device");
    struct EngineScope;
    impl Drop for EngineScope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = EngineScope;
    let device = ww3d_engine::device().unwrap();
    let queue = ww3d_engine::queue().unwrap();
    let format = ww3d_engine::color_format().unwrap();
    let graphics = GraphicsSystem::new(
        device.clone(),
        queue.clone(),
        format,
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    let mut forward = ForwardPass::initialize(&graphics).unwrap();
    let locked = item(7, "locked", Vec3::new(-0.45, -0.2, 0.));
    let ordinary = item(8, "ordinary", Vec3::new(0., 0.65, 0.));
    for (item, color) in [
        (&locked, Vec3::new(1., 0., 0.)),
        (&ordinary, Vec3::new(0., 1., 0.)),
    ] {
        let key = format!(
            "{}::{}::{}",
            item.model_name, item.mesh_index, item.material_key
        );
        forward
            .mesh_cache
            .insert(key, triangle(&item.model_name, color));
    }
    let binding = FrozenDirectDrawableSceneCandidate {
        host_epoch: 3,
        object_id: ObjectID(7),
        drawable_id: 9,
        binding_generation: 11,
        raw_status: ObjectShroudStatus::Clear,
        effectively_dead: false,
    };
    let items = [locked, ordinary];
    let mut filter = ViewFilterState::new();
    filter.set_filter(FilterType::BlackAndWhite);
    filter.set_mode(FilterMode::BWBlackAndWhite);
    let mut resources = ShaderFilterResources::default();
    let mut overlay_calls = 0;
    for skip_scene in [false, true] {
        let scene = readback_buffer(&device, "pre-filter-scene");
        let filtered = readback_buffer(&device, "post-filter-before-locked");
        let overlay = readback_buffer(&device, "overlay-observes-specific-draw");
        let mut post = |frame: &mut ww3d_engine::RenderFrame| {
            copy_frame(frame, &scene);
            let composite = filter.composite(Vector2::zero());
            assert!(resources.prepare(&device, &queue, format, 32, 32, &composite));
            let step = filter.post_render(1, Vector2::zero());
            let dest = frame.color_view_arc();
            let (texture, encoder) = frame.color_texture_and_encoder();
            assert!(resources.composite_retained_view_filter(
                &device,
                &queue,
                encoder,
                &dest,
                texture,
                format,
                32,
                32,
                &composite,
                &step,
                !skip_scene
            ));
            copy_frame(frame, &filtered);
            Ok(Some(FilterCameraMatrices {
                view: Mat4::from_translation(Vec3::new(0.9, 0., 0.)),
                projection: Mat4::IDENTITY,
                position: Vec3::ZERO,
            }))
        };
        let mut paint_overlay = |frame: &mut ww3d_engine::RenderFrame| {
            overlay_calls += 1;
            copy_frame(frame, &overlay);
            Ok(())
        };
        forward
            .render(
                &graphics,
                &items,
                &Mat4::IDENTITY,
                &Mat4::IDENTITY,
                Vec3::ZERO,
                None,
                None,
                skip_scene,
                Some(&mut post),
                Some(binding),
                Some(&mut paint_overlay),
            )
            .unwrap();
        let scene = pixels(&device, &scene);
        let filtered = pixels(&device, &filtered);
        let final_image = pixels(&device, &overlay);
        assert!(resources.has_scene_capture_for(32, 32, format));
        if !skip_scene {
            assert_eq!(
                forward.renderer.stats().meshes_rendered,
                2,
                "current frame statistics include the ordinary and post-filter batches"
            );
            assert!(
                brightness(pixel(&scene, 9, 20)) < 8,
                "locked triangle must not enter the actual scene/filter capture"
            );
            assert!(
                brightness(pixel(&scene, 16, 5)) > 32,
                "ordinary mesh must enter the actual scene"
            );
            assert!(
                brightness(pixel(&filtered, 23, 20)) < 8,
                "capture and filter must complete before the specific draw"
            );
            assert!(
                brightness(pixel(&final_image, 23, 20)) > 32,
                "overlay must observe actual locked mesh at the updated camera position"
            );
            assert!(
                brightness(pixel(&final_image, 9, 20)) < 8,
                "specific draw must use the postRender camera, not the old camera"
            );
            let ordinary = pixel(&final_image, 16, 5);
            assert!(
                ordinary.iter().max().unwrap() - ordinary.iter().min().unwrap() <= 3,
                "ordinary green geometry must actually pass through black-and-white filter"
            );
            let locked = pixel(&final_image, 23, 20);
            assert!(
                locked.iter().max().unwrap() - locked.iter().min().unwrap() > 32,
                "red locked geometry must remain unfiltered after the actual specific draw"
            );
        } else {
            assert_eq!(
                forward.renderer.stats().meshes_rendered,
                0,
                "retained offers must not publish the prior scene's counts"
            );
            assert_eq!(
                pixel(&filtered, 23, 20),
                pixel(&final_image, 23, 20),
                "skipRender must not redraw the locked mesh over the retained filtered capture"
            );
            assert!(
                brightness(pixel(&final_image, 23, 20)) < 8,
                "retained capture must not contain the prior frame's locked redraw"
            );
        }
    }
    assert_eq!(overlay_calls, 2, "overlay belongs to each current frame");
}
