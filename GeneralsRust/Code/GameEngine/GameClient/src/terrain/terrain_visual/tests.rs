// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;

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
            Vec3::new(0.0, -1.0, 0.0),
            [0.6, 0.5, 0.4],
            [0.2, 0.2, 0.2],
        );
        assert_eq!(diffuse, [0.8, 0.7, 0.6, 1.0]);

        let clamped = TerrainVisualImpl::terrain_static_diffuse_from_normal(
            Vec3::Y,
            Vec3::new(0.0, -1.0, 0.0),
            [0.8, 0.8, 0.8],
            [0.5, 0.4, 0.3],
        );
        assert_eq!(clamped, [1.0, 1.0, 1.0, 1.0]);
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
        assert!(!upload.is_empty(), "GPU extra-blend upload must be non-empty");
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
}
