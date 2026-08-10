use super::*;
use crate::rendering::camera_system::CameraClass;
use std::sync::Arc;

    #[test]
    fn compute_pass_index_ranges_uses_vertex_count_for_non_indexed_meshes() {
        let mut model = MeshModelClass::new("no_indices");
        model.vertex_count = 24;

        let ranges = compute_pass_index_ranges(&model, &[]);
        assert_eq!(ranges, vec![(0, 24)]);
    }

    #[test]
    fn compute_pass_index_ranges_groups_polygon_renderers_by_material_pass() {
        let mut model = MeshModelClass::new("per_pass");

        let mut pass0 = MaterialPassClass::new();
        pass0.set_pass_index(0);
        let mut pass1 = MaterialPassClass::new();
        pass1.set_pass_index(1);
        model.material_passes = vec![pass0.clone(), pass1.clone()];

        let mut renderer_a = DX8PolygonRendererClass::new();
        renderer_a.index_count = 6;
        renderer_a.material_pass = Some(Arc::new(pass1.clone()));

        let mut renderer_b = DX8PolygonRendererClass::new();
        renderer_b.index_count = 3;
        renderer_b.material_pass = Some(Arc::new(pass1));

        let mut renderer_c = DX8PolygonRendererClass::new();
        renderer_c.index_count = 9;
        renderer_c.material_pass = Some(Arc::new(pass0));

        model.polygon_renderer_list = vec![
            Arc::new(renderer_a),
            Arc::new(renderer_b),
            Arc::new(renderer_c),
        ];

        let index_data = vec![0_u32; 18];
        let ranges = compute_pass_index_ranges(&model, &index_data);

        assert_eq!(ranges[1], (0, 9));
        assert_eq!(ranges[0], (9, 9));
    }

    #[test]
    fn update_skin_and_get_deformed_vertices_use_bone_palette() {
        let mut model = MeshModelClass::new("skin_mesh");
        model.vertices.push(W3dVectorStruct {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        model.set_vertex_bone_links(vec![1]);

        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        mesh.set_bone_palette_slice(&[
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        ]);

        mesh.update_skin();

        let mut deformed = Vec::new();
        mesh.get_deformed_vertices(&mut deformed);
        assert_eq!(deformed.len(), 1);
        assert!((deformed[0].x - 3.0).abs() < 1.0e-4);
        assert!((deformed[0].y - 2.0).abs() < 1.0e-4);
        assert!((deformed[0].z - 3.0).abs() < 1.0e-4);
    }

    #[test]
    fn animation_hidden_state_affects_visibility_checks() {
        let mut mesh = MeshClass::new();
        assert!(!mesh.is_animation_hidden());
        assert!(mesh.is_not_hidden_at_all());

        mesh.set_animation_hidden(true);
        assert!(mesh.is_animation_hidden());
        assert!(!mesh.is_not_hidden_at_all());
    }

    #[test]
    fn frustum_culling_accepts_mesh_in_front_of_right_handed_camera() {
        let mut camera = CameraClass::new();
        camera.set_clip_planes(1.0, 1000.0);
        camera.look_at(Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let render_info = RenderInfoClass::new(Arc::new(camera));

        let mut mesh = MeshClass::new();
        mesh.bounding_sphere = SphereClass::new(Vec3::ZERO, 1.0);
        mesh.set_transform(Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)));

        assert!(mesh.should_render_with_frustum_culling(&render_info));
    }
