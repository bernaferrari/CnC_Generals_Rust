use game_client_rust::{
    system::SubsystemInterface,
    terrain::{height_map::HeightMap, terrain_visual::TerrainVisualImpl},
};
use glam::Mat4;

fn loaded_visual_with_border() -> TerrainVisualImpl {
    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.set_raw_height(2, 3, 100);
    heightmap.set_raw_height(3, 3, 120);

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");
    visual
}

#[test]
fn raw_map_height_only_lowers_and_reads_logic_height() {
    let mut visual = loaded_visual_with_border();

    assert_eq!(visual.get_raw_map_height(1, 2), 100);

    visual.set_raw_map_height(1, 2, 130);
    assert_eq!(visual.get_raw_map_height(1, 2), 100);

    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 80);

    assert_eq!(visual.get_raw_map_height(2, 2), 120);
}

#[test]
fn raw_map_height_returns_zero_without_loaded_logic_map() {
    let mut visual = TerrainVisualImpl::new();

    assert_eq!(visual.get_raw_map_height(1, 2), 0);
    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 0);
}

fn option_names(names: &[Option<String>; 5]) -> [Option<&str>; 5] {
    std::array::from_fn(|i| names[i].as_deref())
}

#[test]
fn skybox_replacement_tracks_initial_and_current_names_without_gpu() {
    let mut visual = TerrainVisualImpl::new();
    let old = ["old0", "old1", "old2", "old3", "old4"];
    let first = ["new0", "new1", "new2", "new3", "new4"];
    let second = ["new0", "alt1", "new2", "alt3", "new4"];

    visual.replace_skybox_textures(&old, &first).unwrap();
    assert_eq!(
        option_names(visual.initial_skybox_texture_names()),
        [
            Some("old0"),
            Some("old1"),
            Some("old2"),
            Some("old3"),
            Some("old4")
        ]
    );
    assert_eq!(
        option_names(visual.current_skybox_texture_names()),
        [
            Some("new0"),
            Some("new1"),
            Some("new2"),
            Some("new3"),
            Some("new4")
        ]
    );

    visual.replace_skybox_textures(&old, &second).unwrap();
    assert_eq!(
        option_names(visual.initial_skybox_texture_names()),
        [
            Some("old0"),
            Some("old1"),
            Some("old2"),
            Some("old3"),
            Some("old4")
        ]
    );
    assert_eq!(
        option_names(visual.current_skybox_texture_names()),
        [
            Some("new0"),
            Some("alt1"),
            Some("new2"),
            Some("alt3"),
            Some("new4")
        ]
    );
}

#[test]
fn skybox_reset_restores_current_names_to_initial_names() {
    let mut visual = TerrainVisualImpl::new();
    let old = ["old0", "old1", "old2", "old3", "old4"];
    let new = ["new0", "new1", "new2", "new3", "new4"];

    visual.replace_skybox_textures(&old, &new).unwrap();
    visual.reset().unwrap();

    assert_eq!(
        option_names(visual.initial_skybox_texture_names()),
        [
            Some("old0"),
            Some("old1"),
            Some("old2"),
            Some("old3"),
            Some("old4")
        ]
    );
    assert_eq!(
        option_names(visual.current_skybox_texture_names()),
        [
            Some("old0"),
            Some("old1"),
            Some("old2"),
            Some("old3"),
            Some("old4")
        ]
    );
}

#[test]
fn water_grid_starts_disabled_after_init() {
    let visual = TerrainVisualImpl::new();

    assert!(!visual.water_grid_enabled());
}

#[test]
fn water_grid_height_returns_none_when_disabled() {
    let mut visual = TerrainVisualImpl::new();
    visual.set_water_grid_resolution(4.0, 4.0, 10.0);
    visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
    assert!(visual.change_water_height(10.0, 10.0, 3.0));

    assert_eq!(visual.get_water_grid_height(10.0, 10.0), None);
}

#[test]
fn water_grid_transform_resolution_and_clamps_round_trip() {
    let mut visual = TerrainVisualImpl::new();

    visual.set_water_grid_height_clamps(-2.0, 9.0);
    visual.set_water_attenuation_factors(1.0, 2.0, 3.0, 4.0);
    visual.set_water_transform(0.0, 10.0, 20.0, 5.0);
    visual.set_water_grid_resolution(4.0, 5.0, 10.0);

    assert_eq!(visual.water_grid_state().height_clamps, (-2.0, 9.0));
    assert_eq!(visual.water_grid_state().attenuation, (1.0, 2.0, 3.0, 4.0));
    assert_eq!(visual.water_grid_resolution(), (4.0, 5.0, 10.0));
    assert_eq!(visual.water_transform().w_axis.z, 5.0);
    visual.add_water_velocity(20.0, 30.0, 1.5, 7.0);
    assert_eq!(visual.water_grid_state().velocity_events.len(), 0);

    visual.enable_water_grid(true);
    assert_eq!(visual.get_water_grid_height(20.0, 30.0), Some(5.0));
    assert!(visual.change_water_height(20.0, 30.0, 2.5));
    assert_eq!(visual.get_water_grid_height(20.0, 30.0), Some(7.5));
    assert_eq!(visual.get_water_grid_height(100.0, 30.0), None);

    let transform = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
    visual.set_water_transform_matrix(transform);
    assert_eq!(visual.water_transform(), transform);
}

#[test]
fn water_grid_rejects_cpp_world_to_grid_outside_edges() {
    let mut visual = TerrainVisualImpl::new();
    visual.set_water_grid_resolution(4.0, 4.0, 10.0);
    visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
    visual.enable_water_grid(true);

    assert_eq!(visual.get_water_grid_height(-0.1, 0.0), None);
    assert_eq!(visual.get_water_grid_height(30.0, 30.0), Some(5.0));
    assert_eq!(visual.get_water_grid_height(30.1, 30.0), None);
    assert!(!visual.change_water_height(30.1, 30.0, 1.0));
}

#[test]
fn water_grid_change_height_uses_cpp_attenuation_and_clamps() {
    let mut visual = TerrainVisualImpl::new();
    visual.set_water_grid_resolution(4.0, 4.0, 10.0);
    visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
    visual.set_water_grid_height_clamps(-1.0, 3.0);
    visual.set_water_attenuation_factors(1.0, 1.0, 0.0, 10.0);
    visual.enable_water_grid(true);

    assert!(visual.change_water_height(10.0, 10.0, 4.0));

    assert_eq!(visual.get_water_grid_height(10.0, 10.0), Some(8.0));
    let diagonal = visual.get_water_grid_height(0.0, 0.0).unwrap();
    assert!(diagonal > 6.65 && diagonal < 6.66);
}

#[test]
fn water_grid_velocity_only_applies_when_enabled_and_in_bounds() {
    let mut visual = TerrainVisualImpl::new();
    visual.set_water_grid_resolution(4.0, 4.0, 10.0);
    visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
    visual.set_water_attenuation_factors(1.0, 0.0, 0.0, 10.0);

    visual.add_water_velocity(10.0, 10.0, 1.5, 7.0);
    assert!(visual.water_grid_state().point_motions.is_empty());
    assert!(visual.water_grid_state().velocity_events.is_empty());

    visual.enable_water_grid(true);
    visual.add_water_velocity(10.0, 10.0, 1.5, 7.0);
    visual.add_water_velocity(100.0, 10.0, 1.5, 7.0);

    assert_eq!(visual.water_grid_state().velocity_events.len(), 1);
    let center_motion = visual
        .water_grid_state()
        .point_motions
        .get(&(1, 1))
        .expect("center water vertex should be in motion");
    assert_eq!(center_motion.velocity, 1.5);
    assert_eq!(center_motion.preferred_height, 7.0);
    assert!(center_motion.in_motion);
}
