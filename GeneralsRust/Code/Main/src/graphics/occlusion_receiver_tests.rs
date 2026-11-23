use super::*;
use glam::Mat4;
use std::time::Duration;
use ww3d_core::w3d_format::{W3dTriangleStruct, W3dVectorStruct};
use ww3d_renderer_3d::rendering::{camera_system::CameraClass, mesh_system::MeshModelClass};

fn mesh(name: &str, positions: &[[f32; 3]], faces: &[[u32; 3]]) -> Arc<MeshClass> {
    let mut model = MeshModelClass::new(name);
    model.vertices = positions
        .iter()
        .map(|&[x, y, z]| W3dVectorStruct { x, y, z })
        .collect();
    model.triangles = faces
        .iter()
        .map(|&vindex| W3dTriangleStruct {
            vindex,
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            distance: 0.0,
        })
        .collect();
    model.vertex_count = positions.len() as u32;
    model.index_count = (faces.len() * 3) as u32;
    let mut mesh = MeshClass::new();
    mesh.model = Some(Arc::new(model));
    Arc::new(mesh)
}

#[test]
#[ignore = "requires a native GPU device"]
fn native_building_marker_tints_only_the_occluded_unit_mesh() {
    const SIZE: u32 = 64;
    const ROW: u32 = SIZE * 4;
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: SIZE,
        height: SIZE,
        ..Default::default()
    })
    .expect("headless WGPU");
    let (device, color, depth) = {
        let engine = engine.lock();
        (
            engine.device(),
            engine.color_format(),
            engine.depth_format().unwrap(),
        )
    };
    let gpu = Arc::new(OcclusionOverlayPipeline::new(
        Arc::clone(&device),
        color,
        depth,
    ));
    let mut color_bindings = OcclusionColorBindGroupCache::default();
    let mut renderer = WgpuMainRenderer::from_engine(engine, Default::default()).unwrap();
    let primitive = [[0, 1, 2], [0, 2, 3]];
    let terrain_back = mesh(
        "terrain behind unit",
        &[
            [-1., -1., 0.88],
            [0., -1., 0.88],
            [0., 1., 0.88],
            [-1., 1., 0.88],
        ],
        &primitive,
    );
    let terrain_near = mesh(
        "terrain in front of unit",
        &[
            [0., -1., 0.40],
            [1., -1., 0.40],
            [1., 1., 0.40],
            [0., 1., 0.40],
        ],
        &primitive,
    );
    let building = mesh(
        "command center wall",
        &[
            [-0.8, -0.7, 0.20],
            [0., -0.7, 0.20],
            [0., 0.7, 0.20],
            [-0.8, 0.7, 0.20],
        ],
        &primitive,
    );
    let unit = mesh(
        "dozer silhouette",
        &[
            [-0.66, -0.55, 0.70],
            [0.65, -0.55, 0.70],
            [-0.02, 0.55, 0.70],
        ],
        &[[0, 1, 2]],
    );

    let mut paint = |markers: bool| {
        renderer.begin_frame().unwrap();
        let handle = renderer.renderer_handle();
        {
            let mut renderer = handle.lock().unwrap();
            let mut camera = CameraClass::new();
            camera.set_view_matrix(Mat4::IDENTITY);
            camera.set_projection_matrix(Mat4::IDENTITY);
            renderer.set_camera(camera);
            for mesh in [&terrain_back, &terrain_near, &building, &unit] {
                renderer.queue_mesh(Arc::clone(mesh)).unwrap();
            }
        }
        if markers {
            OcclusionFrame {
                roles: HashMap::new(),
                buildings: vec![Arc::clone(&building)],
                units: vec![Unit {
                    id: ObjectId(2),
                    distance: 1.0,
                    color: [0., 0., 1., 0.5],
                    meshes: vec![Arc::clone(&unit)],
                }],
                gpu: Arc::clone(&gpu),
            }
            .enqueue(&mut renderer, &mut color_bindings);
        } else {
            // Accepted unit meshes without a building, then a building without
            // accepted unit meshes, must both fail before binding allocation.
            for (buildings, units) in [
                (
                    Vec::new(),
                    vec![Unit {
                        id: ObjectId(2),
                        distance: 1.0,
                        color: [0., 0., 1., 0.5],
                        meshes: vec![Arc::clone(&unit)],
                    }],
                ),
                (
                    vec![Arc::clone(&building)],
                    vec![Unit {
                        id: ObjectId(2),
                        distance: 1.0,
                        color: [0., 0., 1., 0.5],
                        meshes: Vec::new(),
                    }],
                ),
            ] {
                OcclusionFrame {
                    roles: HashMap::new(),
                    buildings,
                    units,
                    gpu: Arc::clone(&gpu),
                }
                .enqueue(&mut renderer, &mut color_bindings);
            }
        }

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("behind-building witness"),
            size: u64::from(ROW * SIZE),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        renderer
            .end_frame_with_post_render(Some(&mut |frame| {
                let (texture, encoder) = frame.color_texture_and_encoder();
                encoder.copy_texture_to_buffer(
                    texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(ROW),
                            rows_per_image: Some(SIZE),
                        },
                    },
                    wgpu::Extent3d {
                        width: SIZE,
                        height: SIZE,
                        depth_or_array_layers: 1,
                    },
                );
                Ok(())
            }))
            .unwrap();
        let slice = readback.slice(..);
        let (sent, received) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sent.send(result).unwrap()
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();
        received
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        let pixels = slice.get_mapped_range().unwrap().to_vec();
        readback.unmap();
        (pixels, color_bindings.creation_count())
    };
    let (baseline, creations_after_baseline) = paint(false);
    assert_eq!(
        creations_after_baseline, 0,
        "empty marker frames allocate no color bindings"
    );
    let (marked, creations_after_first_marked_frame) = paint(true);
    assert_eq!(creations_after_first_marked_frame, 1);
    let (marked_again, creations_after_second_marked_frame) = paint(true);
    assert_eq!(
        creations_after_second_marked_frame, 1,
        "same typed player color reuses its cached uniform binding"
    );
    assert_eq!(
        marked, marked_again,
        "cached binding preserves exact pixels"
    );
    for index in 0..=MAX_CACHED_OCCLUSION_COLORS {
        color_bindings.get_or_create(
            &gpu.device,
            &gpu.color_layout,
            [index as f32 / 32.0, 0.25, 0.75, OCCLUDED_COLOR_ALPHA],
        );
    }
    assert_eq!(
        color_bindings.entry_count(),
        MAX_CACHED_OCCLUSION_COLORS,
        "color bindings remain bounded under changing typed colors"
    );
    let at = |pixels: &[u8], x: usize, y: usize| -> [u8; 4] {
        let offset = y * ROW as usize + x * 4;
        pixels[offset..offset + 4].try_into().unwrap()
    };
    assert_ne!(
        at(&baseline, 21, 39),
        at(&marked, 21, 39),
        "a real unit fragment behind the building must tint the wall"
    );
    assert_eq!(
        at(&baseline, 37, 39),
        at(&marked, 37, 39),
        "even where terrain is in front of the unit, it cannot receive its behind-building tint"
    );
    assert_eq!(
        at(&baseline, 14, 20),
        at(&marked, 14, 20),
        "pixels on a building outside the actual unit silhouette stay unchanged"
    );
}
