use gamelogic::common::types::ObjectShroudStatus;
use generals_main::fow_rendering::PresentationFowGrid;

#[test]
fn props_sample_four_neighboring_cells_on_the_original_ground_plane() {
    let mut grid = PresentationFowGrid {
        width: 2,
        height: 2,
        world_origin_xy: [100.0, 200.0],
        cell_size: 10.0,
        cells: vec![2; 4],
        active: true,
    };
    assert_eq!(
        grid.prop_shroud_status(110.0, 210.0),
        ObjectShroudStatus::Clear
    );
    grid.cells[3] = 1;
    assert_eq!(
        grid.prop_shroud_status(110.0, 210.0),
        ObjectShroudStatus::PartialClear
    );
    grid.cells.fill(1);
    assert_eq!(
        grid.prop_shroud_status(110.0, 210.0),
        ObjectShroudStatus::Fogged
    );
    grid.cells.fill(0);
    assert_eq!(
        grid.prop_shroud_status(110.0, 210.0),
        ObjectShroudStatus::Shrouded
    );
    grid.cells.fill(2);
    assert_eq!(
        grid.prop_shroud_status(100.0, 200.0),
        ObjectShroudStatus::PartialClear
    );
    assert_eq!(
        grid.prop_shroud_status(0.0, 0.0),
        ObjectShroudStatus::Shrouded
    );
    grid.active = false;
    assert_eq!(grid.prop_shroud_status(0.0, 0.0), ObjectShroudStatus::Clear);
}

#[test]
fn authored_prop_meshes_keep_terrain_identity_pose_and_shroud() {
    use game_client::terrain::terrain_visual::TerrainPropRecord;
    use generals_main::assets::{W3DMesh, W3DModel};
    use generals_main::graphics::render_item::RenderItemOwner;
    use generals_main::graphics::render_pipeline::append_terrain_prop_model_items;
    use glam::{Mat4, Vec3};

    let prop = TerrainPropRecord {
        drawable_id: 42,
        position: [10.0, 20.0, 7.0],
        angle: std::f32::consts::FRAC_PI_2,
        scale: 2.0,
        model_name: "tree".into(),
    };
    let mut model = W3DModel::new("tree".into());
    let mut mesh = W3DMesh::new("trunk".into());
    mesh.transform = Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0));
    model.meshes.push(mesh);
    let mut items = Vec::new();
    append_terrain_prop_model_items(
        &prop,
        &model,
        ObjectShroudStatus::PartialClear,
        Vec3::ZERO,
        &mut items,
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].owner, RenderItemOwner::TerrainProp(42));
    assert_eq!(items[0].model_name, "tree");
    assert!(items[0].pushes_projected_shroud_pass());
    let p = (items[0].world_matrix * items[0].mesh_local_transform).transform_point3(Vec3::ZERO);
    assert!((p - Vec3::new(10.0, 7.0, 26.0)).length() < 1e-4);
    items.clear();
    append_terrain_prop_model_items(
        &prop,
        &model,
        ObjectShroudStatus::Shrouded,
        Vec3::ZERO,
        &mut items,
    );
    assert!(items.is_empty());
}
