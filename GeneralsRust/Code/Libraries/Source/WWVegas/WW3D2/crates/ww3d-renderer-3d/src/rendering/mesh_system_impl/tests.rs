use super::*;
use crate::rendering::camera_system::CameraClass;
use std::sync::Arc;
use ww3d_assets::prototypes::{HierarchyPrototype, MeshPrototype};
use ww3d_core::w3d_format::{W3dVectorStruct, W3dVertInfStruct};

fn source_mesh_prototype(name: &str) -> MeshPrototype {
    let mut prototype = MeshPrototype::new(name.to_string());
    prototype.vertices = vec![
        W3dVectorStruct {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        W3dVectorStruct {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        W3dVectorStruct {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
    ];
    prototype
}

fn hierarchy_that_must_not_supply_skin_links() -> HierarchyPrototype {
    let mut hierarchy = HierarchyPrototype::new("unrelated_hierarchy".to_string());
    hierarchy.bind_transforms = vec![Mat4::IDENTITY];
    hierarchy
}

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
fn mesh_prototype_without_influences_never_fabricates_hierarchy_bone_zero_skinning() {
    let prototype = source_mesh_prototype("no_source_influences");
    let hierarchy = hierarchy_that_must_not_supply_skin_links();

    let model = MeshModelClass::from_mesh_prototype(&prototype, Some(&hierarchy))
        .expect("a rigid prototype does not require an HTree skin table");

    assert!(!model.is_skinned());
    assert!(!model.get_flag(MeshGeometryClass::SKIN));
    assert!(model.vertex_influences().is_none());
    assert!(model.vertex_bone_links().is_none());
}

#[test]
fn mesh_prototype_with_misaligned_influences_fails_closed_before_wgpu_skin_setup() {
    let mut prototype = source_mesh_prototype("misaligned_source_influences");
    prototype.vertex_influences = Some(vec![
        W3dVertInfStruct {
            bone_idx: 3,
            pad: [1; 6],
        },
        W3dVertInfStruct {
            bone_idx: 4,
            pad: [2; 6],
        },
    ]);

    let model = MeshModelClass::from_mesh_prototype(&prototype, None)
        .expect("a malformed skin table must leave the mesh rigid, not reject all geometry");

    assert!(!model.is_skinned());
    assert!(!model.get_flag(MeshGeometryClass::SKIN));
    assert!(model.vertex_influences().is_none());
    assert!(model.vertex_bone_links().is_none());
}

#[test]
fn mesh_prototype_with_complete_source_influences_preserves_indices_for_wgpu_skinning() {
    let mut prototype = source_mesh_prototype("complete_source_influences");
    prototype.vertex_influences = Some(vec![
        W3dVertInfStruct {
            bone_idx: 2,
            pad: [1, 2, 3, 4, 5, 6],
        },
        W3dVertInfStruct {
            bone_idx: 1,
            pad: [7, 8, 9, 10, 11, 12],
        },
        W3dVertInfStruct {
            bone_idx: 0,
            pad: [13, 14, 15, 16, 17, 18],
        },
    ]);

    let model = MeshModelClass::from_mesh_prototype(&prototype, None)
        .expect("an exact source table produces a skinned renderer model");

    assert!(model.is_skinned());
    assert!(model.get_flag(MeshGeometryClass::SKIN));
    assert_eq!(model.vertex_bone_links(), Some(&[2, 1, 0][..]));
    let influences = model
        .vertex_influences()
        .expect("the WGPU mesh retains every exact source record");
    assert_eq!(influences[0].bone_idx, 2);
    assert_eq!(influences[0].pad, [1, 2, 3, 4, 5, 6]);
    assert_eq!(influences[2].bone_idx, 0);
    assert_eq!(influences[2].pad, [13, 14, 15, 16, 17, 18]);

    let (indices, weights) = model.vertex_influence_view(0);
    assert_eq!(indices, [2, 0, 0, 0]);
    assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
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
fn frozen_fow_visibility_is_clear_by_default_and_survives_mesh_clone() {
    let mut mesh = MeshClass::new();
    assert_eq!(mesh.frozen_fow_visibility(), FrozenFowVisibility::CLEAR);

    let frozen = FrozenFowVisibility::new(0.23, 0.61, 0.47);
    mesh.set_frozen_fow_visibility(frozen);

    assert_eq!(
        mesh.frozen_fow_visibility().model_uniform_values(),
        (0.23, 0.61, 0.47)
    );
    assert_eq!(mesh.clone_mesh().frozen_fow_visibility(), frozen);
}

#[test]
fn frozen_fow_visibility_propagates_to_existing_and_rebuilt_decal_meshes() {
    let mut parent = MeshClass::new();
    parent.decal_meshes.push(Arc::new(MeshClass::new()));
    let frozen = FrozenFowVisibility::new(0.23, 0.61, 0.47);

    parent.set_frozen_fow_visibility(frozen);
    assert_eq!(parent.decal_meshes[0].frozen_fow_visibility(), frozen);

    // Rebuilding uses the parent snapshot when it creates a new child.
    let mesh_source = include_str!("mesh.rs");
    assert!(mesh_source.contains("decal_mesh.frozen_fow_visibility = self.frozen_fow_visibility;"));
}

#[test]
fn normal_and_additional_material_passes_share_the_frozen_fow_bind_path() {
    let source = MESH_SYSTEM_SRC;
    let normal_passes = source
        .split_once("for pass in &prepared.material_passes {")
        .expect("normal material-pass loop must exist")
        .1
        .split_once("for extra_pass in &render_info.additional_material_passes {")
        .expect("additional material-pass loop must follow normal passes")
        .0;
    assert!(normal_passes.contains("self.draw_material_pass("));

    let additional_passes = source
        .split_once("for extra_pass in &render_info.additional_material_passes {")
        .expect("additional material-pass loop must exist")
        .1
        .split_once("resources.clear();")
        .expect("additional pass loop must end before resources clear")
        .0;
    assert!(additional_passes.contains("self.draw_material_pass("));

    let draw_material_pass = source
        .split_once("fn draw_material_pass")
        .expect("mesh renderer must bind model uniforms per material pass")
        .1;
    assert!(draw_material_pass.contains("mesh.frozen_fow_visibility().model_uniform_values()"));
    assert!(draw_material_pass.contains("Some(visibility_alpha)"));
    assert!(draw_material_pass.contains("Some(visibility_falloff)"));
    assert!(draw_material_pass.contains("Some(is_explored)"));
    assert!(draw_material_pass.contains("mesh.presentation_opacity()"));
    assert!(draw_material_pass.contains("set_alpha_blend_enable(true)"));
}

#[test]
fn projected_shroud_eligibility_is_exact_instance_state_not_scalar_fow() {
    let mut mesh = MeshClass::new();
    mesh.set_frozen_fow_visibility(FrozenFowVisibility::new(0.3, 1.0, 1.0));
    assert!(
        !mesh.projected_shroud_eligible(),
        "fog alpha alone cannot invent the C++ scene decision"
    );

    mesh.set_projected_shroud_eligible(true);
    assert!(mesh.projected_shroud_eligible());
    assert!(mesh.clone_mesh().projected_shroud_eligible());

    mesh.set_frozen_fow_visibility(FrozenFowVisibility::CLEAR);
    assert!(
        mesh.projected_shroud_eligible(),
        "exact PartialClear eligibility must survive a clear scalar value"
    );
}

#[test]
fn projected_shroud_draw_is_after_authored_passes_for_rigid_and_skinned_meshes() {
    let source = include_str!("render_manager.rs");
    let authored = source
        .find("for extra_pass in &render_info.additional_material_passes")
        .expect("authored additional passes must exist");
    let projected = source
        .find("self.draw_projected_shroud_pass(")
        .expect("eligible meshes must invoke the projected pass");
    assert!(
        authored < projected,
        "shroud pass must follow authored passes"
    );
    assert!(source.contains("VertexFormat::ProjectedShroudBasic"));
    assert!(source.contains("VertexFormat::ProjectedShroudSkinned"));
    assert!(source.contains("ProjectedShroudMaterialPassContract::CXX"));
    assert!(source.contains("create_projected_shroud_bind_group"));

    let rigid = include_str!("../shader_system/projected_shroud_basic.wgsl");
    let skinned = include_str!("../shader_system/projected_shroud_skinned.wgsl");
    for shader in [rigid, skinned] {
        assert!(shader.contains("world_position.xz"));
        assert!(shader.contains("textureSample(shroud_texture"));
        assert!(shader.contains("model.material_diffuse"));
    }
    assert!(skinned.contains("bones.bones"));
}

#[test]
fn presentation_opacity_is_instance_alpha_without_changing_fow() {
    let mut mesh = MeshClass::new();
    assert!(!mesh.is_alpha());
    assert_eq!(mesh.frozen_fow_visibility(), FrozenFowVisibility::CLEAR);

    mesh.set_presentation_opacity(0.5);
    assert!(mesh.is_alpha());
    assert_eq!(mesh.sort_level, SORT_LEVEL_BIN1);
    assert_eq!(mesh.frozen_fow_visibility(), FrozenFowVisibility::CLEAR);

    let cloned = mesh.clone_mesh();
    assert!((cloned.presentation_opacity() - 0.5).abs() < f32::EPSILON);
    assert_eq!(cloned.frozen_fow_visibility(), FrozenFowVisibility::CLEAR);

    mesh.set_presentation_opacity(f32::NAN);
    assert!((mesh.presentation_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn frozen_fow_opaque_fogged_output_has_rgb_darkening_even_with_replace_blending() {
    let fogged = FrozenFowVisibility::new(0.3, 1.0, 1.0);
    assert!(fogged.is_explored > 0.5 && fogged.visibility_alpha < 0.5);

    let opaque_shader = include_str!("../shader_system/opaque.wgsl");
    assert!(opaque_shader.contains("if is_explored > 0.5 && visibility_alpha < 0.5"));
    assert!(opaque_shader.contains("result.rgb * 0.5"));

    let pipeline_manager = include_str!("../wgpu_renderer/wgpu_pipeline_manager.rs");
    assert!(pipeline_manager.contains("wgpu::BlendState::REPLACE"));

    // The current scalar approximation leaves `visibility_falloff` as a
    // transported uniform. Do not invent a curve while C++ still uses a
    // projected terrain shroud texture instead.
    for shader in [
        opaque_shader,
        include_str!("../shader_system/alpha.wgsl"),
        include_str!("../shader_system/additive.wgsl"),
        include_str!("../shader_system/skinned.wgsl"),
        include_str!("../shader_system/decal.wgsl"),
    ] {
        assert!(!shader.contains("model.visibility_falloff"));
    }
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
