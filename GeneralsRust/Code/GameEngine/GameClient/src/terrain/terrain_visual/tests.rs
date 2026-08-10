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
}
