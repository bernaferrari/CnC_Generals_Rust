use game_client_rust::{
    system::SubsystemInterface,
    terrain::{
        height_map::HeightMap,
        terrain_visual::{TerrainBibOwnerKind, TerrainVisualImpl},
        TerrainTrackHeightProvider, TerrainTracksConfig,
    },
};
use glam::{Mat4, Vec3};

struct FlatTrackTerrain;

impl TerrainTrackHeightProvider for FlatTrackTerrain {
    fn ground_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
        (10.0, Vec3::Z)
    }
}

fn track_config(max_edges: usize) -> TerrainTracksConfig {
    TerrainTracksConfig {
        max_terrain_tracks: 2,
        max_tank_track_edges: max_edges,
        max_tank_track_opaque_edges: max_edges / 2,
        max_tank_track_fade_delay: 100,
        make_track_marks: true,
    }
}

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
fn faction_bib_requires_loaded_heightmap_and_matches_cpp_corners() {
    let mut visual = TerrainVisualImpl::new();
    let transform = Mat4::from_translation(glam::Vec3::new(100.0, 200.0, 0.0));

    assert!(!visual.add_faction_bib(
        42,
        TerrainBibOwnerKind::Object,
        transform,
        10.0,
        5.0,
        true,
        2.0,
        3.0,
        true,
        1.0,
    ));

    let mut visual = loaded_visual_with_border();
    assert!(visual.add_faction_bib(
        42,
        TerrainBibOwnerKind::Object,
        transform,
        10.0,
        5.0,
        true,
        2.0,
        3.0,
        true,
        1.0,
    ));

    let bib = &visual.terrain_bibs()[0];
    assert_eq!(bib.owner_id, 42);
    assert_eq!(bib.owner_kind, TerrainBibOwnerKind::Object);
    assert!(bib.highlight);
    assert_eq!(
        bib.corners,
        [
            [86.0, 191.0, 0.0],
            [116.0, 191.0, 0.0],
            [116.0, 209.0, 0.0],
            [86.0, 209.0, 0.0],
        ]
    );
}

#[test]
fn faction_bib_owner_replacement_removal_and_highlight_clear_match_cpp() {
    let mut visual = loaded_visual_with_border();
    let transform = Mat4::IDENTITY;

    assert!(visual.add_faction_bib(
        7,
        TerrainBibOwnerKind::Drawable,
        transform,
        3.0,
        1.0,
        false,
        0.0,
        0.0,
        true,
        0.0,
    ));
    assert!(visual.add_faction_bib(
        7,
        TerrainBibOwnerKind::Drawable,
        transform,
        4.0,
        1.0,
        false,
        0.0,
        0.0,
        true,
        0.0,
    ));

    assert_eq!(visual.terrain_bibs().len(), 1);
    assert_eq!(visual.terrain_bibs()[0].corners[0], [-4.0, -4.0, 0.0]);

    visual.remove_bib_highlighting();
    assert!(!visual.terrain_bibs()[0].highlight);

    visual.remove_faction_bib(7, TerrainBibOwnerKind::Drawable);
    assert!(visual.terrain_bibs().is_empty());
}

#[test]
fn props_and_construction_removal_record_cpp_terrain_visual_calls() {
    let mut visual = TerrainVisualImpl::new();

    assert!(!visual.add_prop([0.0, 0.0, 0.0], 0.0, 1.0, ""));
    assert!(visual.add_prop([1.0, 1.0, 0.0], 0.25, 1.5, "TreeA"));
    assert!(visual.add_prop([5.0, 5.0, 0.0], 0.5, 0.75, "TreeB"));
    assert_eq!(visual.terrain_props().len(), 2);

    visual.remove_trees_and_props_for_construction([0.0, 0.0, 0.0], 2.0, 2.0, true, 0.0);

    assert_eq!(visual.construction_removals().len(), 1);
    assert_eq!(visual.terrain_props().len(), 1);
    assert_eq!(visual.terrain_props()[0].model_name, "TreeB");
}

#[test]
fn terrain_visual_forwards_track_detail_to_owned_track_system() {
    let terrain = FlatTrackTerrain;
    let mut visual = TerrainVisualImpl::new();
    visual.set_terrain_tracks_detail_with_config(track_config(4));
    let handle = visual
        .terrain_tracks_mut()
        .bind_track(4.0, 10.0, "tracks.tga")
        .unwrap();
    visual
        .terrain_tracks_mut()
        .add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    visual
        .terrain_tracks_mut()
        .add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);

    visual.set_terrain_tracks_detail_with_config(track_config(6));

    assert_eq!(
        visual
            .terrain_tracks()
            .track(handle)
            .unwrap()
            .active_edge_count(),
        0
    );

    for i in 0..7 {
        visual
            .terrain_tracks_mut()
            .add_edge_to_track(handle, &terrain, i as f32 * 20.0, 0.0, i);
    }

    assert_eq!(
        visual
            .terrain_tracks()
            .track(handle)
            .unwrap()
            .active_edge_count(),
        6
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
