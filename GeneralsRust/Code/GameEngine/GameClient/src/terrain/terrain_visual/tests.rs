// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;

    fn texture_ranking_fixture(size: u32) -> TerrainVisualImpl {
        let mut visual = TerrainVisualImpl::new();
        let mut heightmap = HeightMap::new(size, size, 255.0, 10.0);
        heightmap.tile_ndxes = (0..size * size).map(|i| ((i % 8) * 16) as i16).collect();
        visual.height_map = Some(heightmap);
        for i in 0..8 {
            let name = format!("TerrainRankFixture{i}");
            visual.source_tile_classes.push(TerrainSourceTileClass {
                first_tile: i * 4,
                num_tiles: 4,
                width: 2,
                name: name.clone(),
            });
            visual.texture_system.register_texture(TerrainTexture::new(
                0,
                name.clone(),
                format!("Art/Terrain/{name}.tga"),
            ));
        }
        visual
    }

    #[test]
    fn chunk_texture_ranking_reuses_unchanged_map_work() {
        let mut visual = texture_ranking_fixture(512);
        let expected = visual.select_stable_chunk_texture_ids(&[]);
        let started = std::time::Instant::now();
        for _ in 0..60 {
            assert_eq!(visual.select_stable_chunk_texture_ids(&[]), expected);
        }
        eprintln!(
            "terrain ranking: 60 unchanged selections {:?}, map scans {}",
            started.elapsed(),
            visual.texture_class_rank_builds
        );
        assert_eq!(
            visual.texture_class_rank_builds, 1,
            "unchanged frames must not rescan all map tiles"
        );
    }

    #[test]
    fn chunk_texture_ranking_rebuilds_for_map_and_class_replacement() {
        let mut visual = texture_ranking_fixture(8);
        let first = visual.select_stable_chunk_texture_ids(&[]);
        let mut classes = visual.source_tile_classes.clone();
        classes.reverse();
        visual
            .load_source_tiles_from_texture_classes(&classes)
            .unwrap();
        assert_eq!(
            visual.select_stable_chunk_texture_ids(&[]),
            first,
            "class-vector reordering must preserve authored class selection"
        );
        assert_eq!(visual.texture_class_rank_builds, 2);

        let mut replacement = HeightMap::new(8, 8, 255.0, 10.0);
        replacement.tile_ndxes = vec![16; 64];
        visual
            .load_heightmap_from_data(replacement, None, None)
            .unwrap();
        assert_eq!(
            visual.select_stable_chunk_texture_ids(&[])[0],
            first[1],
            "the replacement map's most-used class must win"
        );
        assert_eq!(visual.texture_class_rank_builds, 3);
    }

    #[test]
    fn chunk_texture_ranking_keeps_late_texture_lookup_live() {
        let mut visual = texture_ranking_fixture(8);
        visual.texture_system = TerrainTextures::new();
        assert_eq!(
            visual.select_stable_chunk_texture_ids(&[]),
            [0; MAX_TEXTURES_PER_CHUNK]
        );
        let id = visual.texture_system.register_texture(TerrainTexture::new(
            0,
            "TerrainRankFixture0".into(),
            "Art/Terrain/TerrainRankFixture0.tga".into(),
        ));
        assert_eq!(
            visual.select_stable_chunk_texture_ids(&[]),
            [id; MAX_TEXTURES_PER_CHUNK]
        );
        assert_eq!(
            visual.texture_class_rank_builds, 1,
            "late art changes lookup, not immutable map ranking"
        );
    }

    #[test]
    fn chunk_texture_ranking_is_instance_owned_and_reset_invalidates() {
        let mut first = texture_ranking_fixture(8);
        let mut second = texture_ranking_fixture(8);
        first.select_stable_chunk_texture_ids(&[]);
        second.select_stable_chunk_texture_ids(&[]);
        first.reset().unwrap();
        assert_eq!(
            first.select_stable_chunk_texture_ids(&[]),
            [0; MAX_TEXTURES_PER_CHUNK]
        );
        assert_eq!(first.texture_class_rank_builds, 2);
        second.select_stable_chunk_texture_ids(&[]);
        assert_eq!(second.texture_class_rank_builds, 1);
    }

    #[test]
    fn chunk_texture_ranking_survives_height_only_changes() {
        let mut visual = texture_ranking_fixture(8);
        visual.height_map.as_mut().unwrap().heights[0] = 0.5;
        let expected = visual.select_stable_chunk_texture_ids(&[]);
        visual.set_raw_map_height(0, 0, 64);
        visual.apply_logic_height_map_bytes(&[96; 64]);
        assert_eq!(visual.select_stable_chunk_texture_ids(&[]), expected);
        assert_eq!(visual.texture_class_rank_builds, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn terrain_test_gpu_device_and_queue() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("terrain shroud GPU regression requires an adapter");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("terrain shroud test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .expect("terrain shroud GPU regression requires a device")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn terrain_test_gpu_device() -> wgpu::Device {
        terrain_test_gpu_device_and_queue().0
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn shroud_delta_preserves_static_chunk_gpu_mesh() {
        // C++ W3DShroud changes its destination texture and replays a material
        // pass; it does not rebuild HeightMapRenderObjClass vertex buffers.
        let device = terrain_test_gpu_device();
        let buffer = || {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("static chunk marker"),
                size: 16,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            })
        };
        let mut visual = TerrainVisualImpl::new();
        visual.chunk_meshes.insert(
            42,
            GpuChunkMesh {
                vertex_buffer: buffer(),
                index_buffer: buffer(),
                index_count: 6,
                revision: 7,
                uploaded_with_dynamic_lights: false,
            },
        );

        visual.set_shroud_overlay_r8(2, 2, 40.0, [0.0, 0.0], &[255, 0, 255, 0]);
        assert_eq!(
            visual.chunk_meshes.get(&42).map(|mesh| mesh.revision),
            Some(7)
        );
        visual.set_shroud_overlay_r8(2, 2, 40.0, [0.0, 0.0], &[0, 0, 255, 0]);
        assert_eq!(
            visual.chunk_meshes.get(&42).map(|mesh| mesh.revision),
            Some(7)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn expired_pulse_restores_cached_gpu_mesh_after_chunk_returns_to_draw_window() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            drain_display_light_pulses,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        let (device, queue) = terrain_test_gpu_device_and_queue();
        let mut visual = TerrainVisualImpl::new();
        visual.device = Some(Arc::new(device));
        visual.queue = Some(Arc::new(queue));
        visual.config.world_size = (11_000.0, 11_000.0);
        visual
            .chunk_manager
            .create_chunks_for_region(
                Vec3::new(10_000.0, 0.0, 10_000.0),
                Vec3::new(10_064.0, 0.0, 10_064.0),
                64.0,
            )
            .unwrap();
        let center = Vec3::new(10_032.0, 0.0, 10_032.0);
        let chunk = visual
            .chunk_manager
            .get_chunk_at_position_mut(&center)
            .unwrap();
        chunk.heights = vec![vec![0.0; 2]; 2];
        chunk.generate_geometry(3).unwrap();
        chunk.visible = true;
        let chunk_id = chunk.id;

        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [10_032.0, 10_032.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 80.0,
            increase_frames: 0,
            decay_frames: 0,
        }));
        visual.update_chunk_meshes().unwrap();
        let lit_buffer = visual
            .chunk_meshes
            .get(&chunk_id)
            .unwrap()
            .vertex_buffer
            .clone();
        assert!(visual.had_dynamic_lights);

        visual
            .chunk_manager
            .get_chunk_at_position_mut(&center)
            .unwrap()
            .visible = false;
        visual.config.world_size = (1.0, 1.0);
        assert!(visual.chunk_ids_for_gpu_draw().is_empty());
        clear_scene_dynamic_lights();
        visual.update_chunk_meshes().unwrap();
        assert!(!visual.had_dynamic_lights);
        assert_eq!(
            visual.chunk_meshes.get(&chunk_id).unwrap().vertex_buffer,
            lit_buffer
        );

        visual
            .chunk_manager
            .get_chunk_at_position_mut(&center)
            .unwrap()
            .visible = true;
        visual.config.world_size = (11_000.0, 11_000.0);
        visual.update_chunk_meshes().unwrap();
        let restored_buffer = visual
            .chunk_meshes
            .get(&chunk_id)
            .unwrap()
            .vertex_buffer
            .clone();
        assert_ne!(
            restored_buffer, lit_buffer,
            "returned lit mesh needs one static upload"
        );

        visual.update_chunk_meshes().unwrap();
        assert_eq!(
            visual.chunk_meshes.get(&chunk_id).unwrap().vertex_buffer,
            restored_buffer
        );
        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn disabled_water_grid_consumes_dirty_flag_without_rebuilding_all_overlays() {
        // No W3DWater grid is allocated when the map has no active water
        // table. An empty result is still a completed bake, not a request to
        // rebuild every overlay on every presentation frame.
        let device = terrain_test_gpu_device();
        let mut visual = TerrainVisualImpl::new();
        visual.water_grid_enabled = false;
        visual.overlay.water_grid_dirty = true;
        visual.upload_water_grid_mesh(&device);
        assert!(visual.water_grid_mesh.is_none());
        assert!(!visual.overlay.water_grid_dirty);
    }

    #[test]
    fn terrain_draw_window_tracks_visible_ground_instead_of_camera_eye() {
        // At C++'s default pitch, the view eye sits hundreds of world units
        // behind the visible terrain. Centering 129 cells on the eye clips the
        // far rows even while they pass frustum culling.
        let eye = Vec3::new(1_750.0, 310.0, 1_310.0);
        let look = Vec3::new(1_750.0, 0.0, 1_714.0);
        let view = Mat4::look_at_rh(eye, look, Vec3::Y);
        let projection = Mat4::perspective_rh(38.5_f32.to_radians(), 4.0 / 3.0, 10.0, 12_000.0);
        let center = frustum_ground_footprint_center(view, projection, 0.0)
            .expect("pitched tactical view must meet the terrain plane");
        assert!((center.x - look.x).abs() < 0.1);
        assert!(
            center.y > look.z,
            "footprint should extend ahead of look-at"
        );

        let mut visual = TerrainVisualImpl::new();
        visual.config.world_size = (3_500.0, 3_500.0);
        visual.height_map = Some(HeightMap::new(350, 350, 255.0, 10.0));
        visual.reset_draw_area_state();
        visual.recenter_draw_area_for_view(view, projection);
        let eye_centered_origin = (eye.z / 10.0 - visual.draw_height as f32 / 2.0) as i32;
        assert!(
            visual.draw_origin_y > eye_centered_origin + 20,
            "draw window must move toward the visible far terrain"
        );

        let horizon_view = Mat4::look_at_rh(eye, eye + Vec3::Z, Vec3::Y);
        visual.recenter_draw_area_for_view(horizon_view, projection);
        assert_eq!(visual.draw_origin_y, eye_centered_origin);
    }

    #[test]
    fn bordered_draw_window_uses_authored_heightmap_sample_coordinates() {
        // C++ HeightMap.cpp ADJUST_FROM_INDEX_TO_REAL and updateCenter use
        // MAP_XY_FACTOR plus the stored border, even though camera bounds use
        // only the playable interior. A 200-sample map with 20 border samples
        // has 1600 playable world units but 10 world units per sample.
        let mut heightmap = HeightMap::new(200, 200, 255.0, 10.0);
        heightmap.border_size = 20;
        let mut visual = TerrainVisualImpl::new();
        visual.config.world_size = (1_600.0, 1_600.0);
        visual.height_map = Some(heightmap);
        visual.reset_draw_area_state();

        visual.recenter_draw_area_on_world_position(1_000.0, 1_000.0);
        assert_eq!(visual.draw_origin_x, 55);
        assert_eq!(visual.draw_origin_y, 55);
        assert_eq!(
            visual.draw_area_bounds_world(),
            (350.0, 350.0, 1_630.0, 1_630.0)
        );
    }

    #[test]
    fn draw_window_casts_after_border_offset_for_negative_fractional_world_position() {
        let mut heightmap = HeightMap::new(200, 200, 255.0, 10.0);
        heightmap.border_size = 70;
        let mut visual = TerrainVisualImpl::new();
        visual.config.world_size = (600.0, 600.0);
        visual.height_map = Some(heightmap);
        visual.reset_draw_area_state();

        // C++: Int((-9.9 / 10) + 70 - (129 / 2.0)) == 4.
        visual.recenter_draw_area_on_world_position(-9.9, -9.9);
        assert_eq!((visual.draw_origin_x, visual.draw_origin_y), (4, 4));
        assert_eq!(
            visual.draw_area_bounds_world(),
            (-660.0, -660.0, 620.0, 620.0)
        );
    }

    fn runtime_road_segment(
        start: [f32; 3],
        end: [f32; 3],
        width: f32,
        width_in_texture: f32,
        road_type_id: u32,
        start_is_join: bool,
    ) -> RuntimeRoadVisualSegment {
        RuntimeRoadVisualSegment {
            start,
            end,
            width,
            template_name: String::new(),
            width_in_texture,
            road_type_id,
            start_is_angled: false,
            start_is_join,
            end_is_angled: false,
            end_is_join: false,
            curve_radius: 0.0,
        }
    }

    #[test]
    fn alpha_join_synthesis_scales_texture_width_not_road_width_like_cpp() {
        let source_width = 10.0;
        let segments = vec![
            runtime_road_segment(
                [0.0, 0.0, 0.0],
                [20.0, 0.0, 0.0],
                source_width,
                2.0,
                1,
                true,
            ),
            runtime_road_segment([-5.0, 0.0, -5.0], [5.0, 0.0, 5.0], 10.0, 1.0, 2, false),
        ];
        let topology = vec![
            RuntimeRoadEndpointTopology {
                start_count: 0,
                end_count: 1,
                start_last: true,
                end_last: true,
            },
            RuntimeRoadEndpointTopology::default(),
        ];

        let (joins, _) =
            TerrainVisualImpl::synthesize_runtime_cross_type_join_segments(&segments, &topology);

        assert_eq!(joins.len(), 1);
        assert!((joins[0].width - source_width).abs() < 0.001);
        assert!(joins[0].width_in_texture > source_width);
    }

    #[test]
    fn terrain_static_diffuse_uses_negative_light_position_and_clamps() {
        let diffuse = TerrainVisualImpl::terrain_static_diffuse_from_normal(
            Vec3::Y,
            &[(Vec3::new(0.0, -1.0, 0.0), [0.6, 0.5, 0.4])],
            [0.2, 0.2, 0.2],
        );
        assert_eq!(diffuse, [0.8, 0.7, 0.6, 1.0]);

        let clamped = TerrainVisualImpl::terrain_static_diffuse_from_normal(
            Vec3::Y,
            &[(Vec3::new(0.0, -1.0, 0.0), [0.8, 0.8, 0.8])],
            [0.5, 0.4, 0.3],
        );
        assert_eq!(clamped, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn terrain_static_diffuse_sums_all_global_lights_like_do_the_light() {
        // C++ doTheLight (BaseHeightMap.cpp:556-566) adds clamp(N·L,0,1)*diffuse
        // for every global light on top of light-0 ambient.
        let multi = TerrainVisualImpl::terrain_static_diffuse_from_normal(
            Vec3::Y,
            &[
                (Vec3::new(0.0, -1.0, 0.0), [0.3, 0.3, 0.3]),
                (Vec3::new(0.0, -1.0, 0.0), [0.3, 0.3, 0.3]),
            ],
            [0.2, 0.2, 0.2],
        );
        assert_eq!(multi, [0.8, 0.8, 0.8, 1.0]);
    }

    #[test]
    fn terrain_shader_fogs_only_when_fog_range_is_set() {
        let shader = include_str!("../../shaders/terrain.wgsl");
        assert!(
            shader.contains("fog_span > 0.0"),
            "terrain FS must skip fog when fog_end <= fog_start"
        );
        assert!(
            !shader.contains("final_color = mix(final_color, uniforms.fog_color, fog_factor);")
                || shader.contains("if (fog_span > 0.0)"),
            "terrain FS must not unconditionally mix fog_color"
        );

        let mut visual = TerrainVisualImpl::new();
        let (start, end) = visual.fog_range_span();
        assert!(
            end <= start,
            "default fog disabled like C++ FogEnabled(false), got {start}..{end}"
        );

        visual.set_lighting(None, None, None, Some([0.9, 0.71, 0.6]), None);
        let (start, end) = visual.fog_range_span();
        assert!(
            end <= start,
            "fog_color without fog_range must not enable peach distance fog"
        );

        visual.set_lighting(None, None, None, None, Some((100.0, 500.0)));
        assert_eq!(visual.fog_range_span(), (100.0, 500.0));
    }

    #[test]
    fn extra_blend_gpu_upload_is_called_with_non_empty_positions() {
        let mut heightmap = HeightMap::new(3, 3, 255.0, 1.0);
        let mut extra = vec![0i16; 9];
        extra[0] = 2; // cell (0,0)
        extra[4] = 3; // cell (1,1)
        heightmap.assign_extra_blend_tile_ndxes(extra);

        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(heightmap, None, None)
            .expect("runtime heightmap should load");

        assert_eq!(visual.extra_blend_tile_count(), 2);
        assert_eq!(
            visual.extra_blend_tile_positions(),
            &[0 | (0 << 16), 1 | (1 << 16)]
        );

        visual.upload_extra_blend_overlay();
        let upload = visual.last_extra_blend_gpu_upload();
        assert!(
            !upload.is_empty(),
            "GPU extra-blend upload must be non-empty"
        );
        assert_eq!(upload.tile_count, 2);
        assert_eq!(upload.positions, vec![0 | (0 << 16), 1 | (1 << 16)]);
        assert!(
            upload.vertex_count >= 12,
            "two extra-blend tiles must upload at least 12 verts"
        );
        assert!(
            upload.index_count >= 12,
            "two extra-blend tiles must upload 6 indices each"
        );
    }

    #[test]
    fn extra_blend_ndxes_produce_non_empty_draw_mesh() {
        let mut heightmap = HeightMap::new(3, 3, 255.0, 1.0);
        let mut extra = vec![0i16; 9];
        extra[0] = 2;
        heightmap.assign_extra_blend_tile_ndxes(extra);

        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(heightmap, None, None)
            .expect("runtime heightmap should load");

        let mesh = visual.build_extra_blend_draw_mesh();
        assert!(
            mesh.vertex_count() >= 6,
            "one extra-blend tile must emit two triangles (vert count >= 6)"
        );
        assert_eq!(mesh.index_count(), 6);
        assert_eq!(mesh.tile_count, 1);
    }

    #[test]
    fn extra_blend_draw_increments_when_tiles_exist_and_stays_zero_when_none() {
        let mut with_tiles = HeightMap::new(3, 3, 255.0, 1.0);
        let mut extra = vec![0i16; 9];
        extra[0] = 2;
        with_tiles.assign_extra_blend_tile_ndxes(extra);

        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(with_tiles, None, None)
            .expect("runtime heightmap should load");
        visual.upload_extra_blend_overlay();
        assert_eq!(visual.extra_blend_draw_count(), 0);
        assert!(visual.extra_blend_draw());
        assert_eq!(visual.extra_blend_draw_count(), 1);
        assert!(visual.extra_blend_draw());
        assert_eq!(visual.extra_blend_draw_count(), 2);

        let empty_map = HeightMap::new(3, 3, 255.0, 1.0);
        let mut empty_visual = TerrainVisualImpl::new();
        empty_visual
            .load_heightmap_from_data(empty_map, None, None)
            .expect("empty heightmap should load");
        empty_visual.upload_extra_blend_overlay();
        assert_eq!(empty_visual.extra_blend_draw_count(), 0);
        assert!(!empty_visual.extra_blend_draw());
        assert_eq!(empty_visual.extra_blend_draw_count(), 0);
    }

    #[test]
    fn water_tracks_flush_is_called_from_live_water_record() {
        // C++ WaterTracksRenderSystem::flush (W3DWaterTracks.cpp) is invoked
        // from the live water record (W3DWater.cpp / TerrainVisual update).
        let mut visual = TerrainVisualImpl::new();
        let handle = visual
            .water_tracks_mut()
            .bind_track(crate::terrain::WaterTrackType::Pond)
            .expect("bind pond wake");
        visual.water_tracks_mut().track_mut(handle).unwrap().init(
            18.0,
            28.0,
            Vec2::new(10.0, 20.0),
            Vec2::new(10.0, 21.0),
            "wave256.tga",
            0,
        );
        visual.flush_water_tracks();
        let flush = visual.last_water_tracks_flush();
        assert!(
            !flush.vertices.is_empty(),
            "live water record must flush wakes"
        );
        assert!(!flush.indices.is_empty());
        assert_eq!(flush.ranges[0].texture_name, "wave256.tga");
    }

    #[test]
    fn runtime_bridges_use_authored_model_scale_towers_not_granite() {
        // C++ W3DBridge::load (W3DBridgeBuffer.cpp:182-191) uses findBridge
        // BridgeModelName + BridgeScale + TowerObjectName*, not granite ribbons.
        use crate::terrain::roads::{RoadType, StoneType};
        use game_engine::common::ascii_string::AsciiString;

        const TEMPLATE: &str = "hq_mlu40_AuthoredBridge";
        {
            let mut roads = game_engine::common::ini::get_terrain_roads_mut();
            if roads.find_bridge(TEMPLATE).is_none() {
                let bridge = roads.new_bridge(AsciiString::from(TEMPLATE));
                bridge.bridge_model_name = AsciiString::from("CBBridgeSt");
                bridge.bridge_scale = 0.7;
                bridge.tower_object_name[0] = AsciiString::from("TowerFromLeft");
                bridge.tower_object_name[1] = AsciiString::from("TowerFromRight");
                bridge.tower_object_name[2] = AsciiString::from("TowerToLeft");
                bridge.tower_object_name[3] = AsciiString::from("TowerToRight");
            }
        }

        let mut visual = TerrainVisualImpl::new();
        visual
            .set_runtime_bridge_segments(&[(
                [0.0, 0.0, 0.0],
                [120.0, 0.0, 0.0],
                12.0,
                TEMPLATE.to_string(),
            )])
            .expect("authored bridge should bake");

        let mut found = 0usize;
        visual
            .road_system
            .for_each_visible_overlay_source(|road, segment| {
                found += 1;
                assert_eq!(road.name, TEMPLATE);
                assert!(
                    !matches!(
                        road.road_type,
                        RoadType::StoneBridge {
                            stone_type: StoneType::Granite,
                            ..
                        }
                    ),
                    "must not invent granite StoneBridge ribbons"
                );
                let override_text = segment
                    .properties
                    .texture_override
                    .as_deref()
                    .expect("authored override");
                assert!(override_text.contains("BridgeModelName=CBBridgeSt"));
                assert!(override_text.contains("BridgeScale=0.700000"));
                assert!(override_text.contains("TowerObjectNameFromLeft=TowerFromLeft"));
                assert!(override_text.contains("TowerObjectNameToRight=TowerToRight"));
                let geometry = segment.geometry.as_ref().expect("authored span mesh");
                assert!(!geometry.vertices.is_empty());
                assert!(!geometry.indices.is_empty());
            });
        assert_eq!(found, 1);

        let mut visual_unknown = TerrainVisualImpl::new();
        visual_unknown
            .set_runtime_bridge_segments(&[(
                [0.0, 0.0, 0.0],
                [80.0, 0.0, 0.0],
                10.0,
                "hq_mlu40_MissingBridge".to_string(),
            )])
            .expect("unknown template is fail-closed skip");
        let mut unknown = 0usize;
        visual_unknown
            .road_system
            .for_each_visible_overlay_source(|_, _| unknown += 1);
        assert_eq!(unknown, 0, "unknown template must not bake granite");
    }

    #[test]
    fn live_chunk_upload_bakes_do_the_dynamic_light() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            do_the_dynamic_light, drain_display_light_pulses, scene_dynamic_lights,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [0.0, 0.0, 3.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 20.0,
            increase_frames: 1,
            decay_frames: 1,
        }));
        let lights = scene_dynamic_lights();
        let static_rgba = [0.2, 0.2, 0.2, 1.0];
        let baked = TerrainVisualImpl::bake_terrain_vertex_dynamic_light(
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            static_rgba,
            &lights,
        );
        let expected = {
            let packed = ((0.2 * 255.0) as u32)
                | (((0.2 * 255.0) as u32) << 8)
                | (((0.2 * 255.0) as u32) << 16)
                | (255 << 24);
            let lit = do_the_dynamic_light([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], packed, &lights);
            [
                ((lit >> 16) & 0xFF) as f32 / 255.0,
                ((lit >> 8) & 0xFF) as f32 / 255.0,
                (lit & 0xFF) as f32 / 255.0,
                ((lit >> 24) & 0xFF) as f32 / 255.0,
            ]
        };
        assert_eq!(baked, expected);
        assert_ne!(
            baked, static_rgba,
            "pulse must change sun-lit terrain color"
        );

        let src = crate::terrain::terrain_visual::TERRAIN_VISUAL_SRC;
        assert!(
            src.contains("bake_terrain_vertex_dynamic_light") && src.contains("needs_light_rebake"),
            "live TerrainVisual chunk upload must rebake createLightPulse"
        );
        clear_scene_dynamic_lights();
    }

    #[test]
    fn add_terrain_scorch_forces_visual_rebuild_and_exscorch01() {
        use crate::terrain::scorch_mesh::{
            add_terrain_scorch, bake_terrain_scorch_gpu_mesh, clear_terrain_scorches,
            terrain_scorch_count, terrain_scorches_in_buffer,
        };

        clear_terrain_scorches();
        let visual = TerrainVisualImpl::new();
        assert!(
            !visual.scorches_need_gpu_rebuild(),
            "empty buffer should not rebuild"
        );
        assert!(add_terrain_scorch([12.0, 8.0, 0.0], 18.0, 2));
        assert_eq!(terrain_scorches_in_buffer(), 0);
        assert!(terrain_scorch_count() > 0);
        assert!(
            visual.scorches_need_gpu_rebuild(),
            "addScorch zeros m_scorchesInBuffer so drawScorches rebuilds"
        );

        let height = HeightMap::new(8, 8, 0.0, 10.0);
        let mesh = bake_terrain_scorch_gpu_mesh(&height, 0xffff_ffff);
        assert!(!mesh.vertices.is_empty());
        assert!(terrain_scorches_in_buffer() > 0);

        assert_eq!(
            TerrainVisualImpl::scorch_overlay_texture_name(),
            "EXScorch01.tga"
        );
        let src = crate::terrain::terrain_visual::TERRAIN_VISUAL_SRC;
        assert!(
            src.contains("scorch_texture_bind_group")
                && src.contains("EXScorch01.tga")
                && src.contains("scorches_need_gpu_rebuild"),
            "live draw must bind EXScorch01, not road texture"
        );
        clear_terrain_scorches();
    }

    fn bib_test_visual() -> TerrainVisualImpl {
        let mut visual = TerrainVisualImpl::new();
        let heightmap = HeightMap::new(32, 32, 255.0, 1.0);
        visual
            .load_heightmap_from_data(heightmap, None, None)
            .expect("bib test heightmap should load");
        visual
    }

    /// C++ InGameUI placement lifecycle: preview adds highlight bibs for the
    /// current blockers (W3DBibBuffer::addBib dedupes per owner, so re-adding
    /// per drag frame must not grow the set), the sync prunes owners that no
    /// longer block, and the placement end clears the Object-kind session
    /// bibs and un-highlights what remains (destroyPlacementIcons /
    /// removeHighlighting).
    #[test]
    fn placement_highlight_bib_session_adds_prunes_and_clears() {
        let mut visual = bib_test_visual();
        let object = TerrainBibOwnerKind::Object;
        let drawable = TerrainBibOwnerKind::Drawable;
        let transform = Mat4::IDENTITY;

        assert!(
            visual.add_faction_bib(1, object, transform, 20.0, 20.0, false, 0.0, 0.0, true, 0.0)
        );
        assert!(
            visual.add_faction_bib(2, object, transform, 20.0, 20.0, false, 0.0, 0.0, true, 0.0)
        );
        // Same owner re-add (drag frame): update, not growth.
        assert!(
            visual.add_faction_bib(2, object, transform, 21.0, 21.0, false, 0.0, 0.0, true, 0.0)
        );
        assert_eq!(visual.terrain_bibs().len(), 2);

        // Cursor moved: only blocker 1 still blocks — owner 2 is pruned.
        visual.retain_placement_highlight_bibs(&[1]);
        assert_eq!(visual.terrain_bibs().len(), 1);
        assert_eq!(visual.terrain_bibs()[0].owner_id, 1);

        // The client place icon carries its own Drawable-kind bib; the
        // placement-end clear must keep it (minus highlight) and drop the
        // host preview bibs.
        assert!(visual.add_faction_bib(
            7, drawable, transform, 20.0, 20.0, false, 0.0, 0.0, true, 0.0
        ));
        visual.clear_placement_highlight_bibs();
        assert_eq!(visual.terrain_bibs().len(), 1);
        assert_eq!(visual.terrain_bibs()[0].owner_kind, drawable);
        assert!(!visual.terrain_bibs()[0].highlight);
    }

    /// Bib art: with TBBib.tga/TBRedBib.tga the texture drives the draw
    /// (white tint, opaque); without the art (this install ships neither
    /// TGA) the stand-in is the C++ red highlight read at partial alpha.
    #[test]
    fn bib_stand_in_is_translucent_red_when_tga_missing() {
        let (highlight_color, highlight_alpha) = bib_stand_in_appearance(true, false);
        let (normal_color, normal_alpha) = bib_stand_in_appearance(false, false);
        let (art_color, art_alpha) = bib_stand_in_appearance(true, true);
        assert!(highlight_color[0] > 0.8 && highlight_color[1] < 0.4);
        assert!((0.0..1.0).contains(&highlight_alpha));
        assert!(normal_color[0] > 0.8);
        assert!((0.0..1.0).contains(&normal_alpha));
        assert_eq!(art_color, [1.0, 1.0, 1.0]);
        assert!((art_alpha - 1.0).abs() < 1.0e-6);
    }

    /// C++ W3DWater.cpp:2242-2343: the wave grid exists only over actual
    /// water — a cell fully above the water plane gets no geometry.
    #[test]
    fn water_grid_cell_is_wet_tracks_water_plane_vs_terrain() {
        let mut heightmap = HeightMap::new(8, 8, 255.0, 10.0);
        // Left half flat at height 10 (wet under a plane at 15), right half
        // at height 200 (dry).
        for y in 0..8 {
            for x in 0..8 {
                heightmap.set_raw_height(x, y, if x < 4 { 10 } else { 200 });
            }
        }
        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(heightmap, None, None)
            .expect("water grid test heightmap should load");

        let wet_corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let dry_corners = [[50.0, 0.0], [60.0, 0.0], [60.0, 10.0], [50.0, 10.0]];
        assert!(visual.water_grid_cell_is_wet(&wet_corners, 15.0));
        assert!(!visual.water_grid_cell_is_wet(&dry_corners, 15.0));
        // Rising the plane above the terrain floods the "dry" cell.
        assert!(visual.water_grid_cell_is_wet(&dry_corners, 500.0));
    }

    /// The 4-slot chunk texture selection must rank the map's texture classes
    /// by placed-tile usage (C++ keeps every class addressable via the
    /// combiner atlas), not by map order — a rarely used class listed first
    /// must not consume a slot a heavily used class needs.
    #[test]
    fn chunk_texture_selection_ranks_classes_by_tile_usage() {
        let mut visual = TerrainVisualImpl::new();
        let mut heightmap = HeightMap::new(8, 8, 255.0, 1.0);
        // 62 samples of class "Common" (firstTile 4), 2 of "Rare"
        // (firstTile 0). Map order lists Rare first.
        // C++ WorldHeightMap.cpp:2312 strips the two quadrant bits before
        // matching firstTile=4. Raw 17 encodes tile 4 / quadrant 1; raw 5
        // would select tile 1 in Rare instead.
        let mut tiles = vec![17_i16; 64];
        tiles[0] = 0;
        tiles[1] = 1;
        heightmap.tile_ndxes = tiles;
        visual
            .load_heightmap_from_data(heightmap, None, None)
            .expect("selection test heightmap should load");

        let mut textures = TerrainTextures::new();
        let rare = textures.register_texture(TerrainTexture::new(
            0,
            "Rare".to_string(),
            "Art/Terrain/Rare.tga".to_string(),
        ));
        let common = textures.register_texture(TerrainTexture::new(
            0,
            "Common".to_string(),
            "Art/Terrain/Common.tga".to_string(),
        ));
        visual.texture_system = textures;
        visual.source_tile_classes = vec![
            TerrainSourceTileClass {
                first_tile: 0,
                num_tiles: 4,
                width: 2,
                name: "Rare".to_string(),
            },
            TerrainSourceTileClass {
                first_tile: 4,
                num_tiles: 4,
                width: 2,
                name: "Common".to_string(),
            },
        ];

        let selected = visual.select_stable_chunk_texture_ids(&[]);
        assert_eq!(selected[0], common, "most-used class must take slot 0");
        assert_eq!(selected[1], rare);
    }
}
