use game_client_rust::{
    system::SubsystemInterface,
    terrain::{
        TILE_BYTES_PER_PIXEL, TREE_TILE_DATA_LEN, TerrainTrackHeightProvider, TerrainTracksConfig,
        TreeModuleData, TreeRegion2D, TreeSphere, TreeTgaHeader, TreeTileImageSpec, TreeTypeMesh,
        add_terrain_scorch, blit_tree_tile_into_atlas, do_lighting, do_tree_atlas_mip,
        generate_box_mip_chain,
        height_map::HeightMap,
        terrain_scorch_count,
        terrain_visual::{TerrainBibOwnerKind, TerrainSourceTileClass, TerrainVisualImpl},
        textures::{BlendTileInfo, FLIPPED_MASK, TileData},
    },
};
use glam::{Mat4, Vec2, Vec3};
use image::{Rgba, RgbaImage};

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
    visual.debug_clear_dirty_chunks();

    assert_eq!(visual.get_raw_map_height(1, 2), 100);
    assert_eq!(visual.debug_dirty_chunk_count(), 0);
    assert!(!visual.debug_roads_need_terrain_normal_reprojection());

    visual.set_raw_map_height(1, 2, 130);
    assert_eq!(visual.get_raw_map_height(1, 2), 100);
    assert_eq!(visual.debug_dirty_chunk_count(), 0);
    assert!(!visual.debug_roads_need_terrain_normal_reprojection());

    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 80);
    assert!(visual.debug_dirty_chunk_count() > 0);
    assert!(visual.debug_roads_need_terrain_normal_reprojection());

    assert_eq!(visual.get_raw_map_height(2, 2), 120);
}

#[test]
fn raw_map_height_returns_zero_without_loaded_logic_map() {
    let mut visual = TerrainVisualImpl::new();

    assert_eq!(visual.get_raw_map_height(1, 2), 0);
    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 0);
}

#[test]
fn terrain_tile_query_uses_logic_heightmap_packed_tile_indices() {
    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.tile_ndxes[(3 * 6 + 2) as usize] = 52;

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");

    assert_eq!(visual.get_terrain_tile(1.25, 2.75).unwrap(), 13);
}

#[test]
fn terrain_color_query_uses_logic_heightmap_source_tile_mip_like_cpp() {
    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.tile_ndxes[(3 * 6 + 2) as usize] = 52;

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");

    let mut tile = TileData::new();
    for pixel in tile.data.chunks_exact_mut(4) {
        pixel[0] = 64;
        pixel[1] = 128;
        pixel[2] = 192;
        pixel[3] = 255;
    }
    tile.update_mips();
    visual.debug_set_source_tile(13, tile);

    let color = visual.get_terrain_color_at(1.25, 2.75).unwrap();
    assert_eq!(color, [192.0 / 255.0, 128.0 / 255.0, 64.0 / 255.0]);
}

#[test]
fn leftover_radar_paint_source_samples_terrain_visual_tile_color() {
    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.tile_ndxes[(3 * 6 + 2) as usize] = 52;

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");

    let mut tile = TileData::new();
    for pixel in tile.data.chunks_exact_mut(4) {
        pixel[0] = 64;
        pixel[1] = 128;
        pixel[2] = 192;
        pixel[3] = 255;
    }
    tile.update_mips();
    visual.debug_set_source_tile(13, tile);

    {
        let mut guard = game_client_rust::terrain::terrain_visual::get_terrain_visual()
            .expect("terrain visual lock");
        *guard = Some(visual);
    }

    game_client_rust::terrain::ensure_radar_terrain_paint_source_registered();
    let color = game_client_rust::terrain::leftover_radar_terrain_color_at(1.25, 2.75)
        .expect("paint source must sample TheTerrainVisual");
    assert_eq!(color, [192.0 / 255.0, 128.0 / 255.0, 64.0 / 255.0]);

    let none = game_client_rust::terrain::leftover_radar_bridge_at(
        &game_engine::common::system::radar::Coord3D::new(1.25, 2.75, 0.0),
    );
    assert!(
        none.is_none(),
        "no intact leftover bridge object must not paint a span"
    );
}

#[test]
fn tile_blend_alpha_selection_matches_cpp_nonzero_inverted_flag() {
    let mut heightmap = HeightMap::new(1, 1, 255.0, 1.0);
    heightmap.tile_ndxes[0] = 0;
    heightmap.blend_tile_ndxes[0] = 1;

    let source_tiles: Box<
        [Option<TileData>; game_client_rust::terrain::height_map::NUM_SOURCE_TILES],
    > = vec![None; game_client_rust::terrain::height_map::NUM_SOURCE_TILES]
        .into_boxed_slice()
        .try_into()
        .expect("source tile array size");
    let mut blend_tiles: Box<
        [BlendTileInfo; game_client_rust::terrain::height_map::NUM_BLEND_TILES],
    > = vec![BlendTileInfo::new(); game_client_rust::terrain::height_map::NUM_BLEND_TILES]
        .into_boxed_slice()
        .try_into()
        .expect("blend tile array size");
    blend_tiles[1].blend_ndx = 4;
    blend_tiles[1].horiz = 1;
    blend_tiles[1].inverted = FLIPPED_MASK;

    let alpha_tiles: [Option<Vec<u8>>; 12] = std::array::from_fn(|index| {
        let mut pixel = vec![0, 0, 0, 0];
        if index == 6 {
            pixel[3] = 255;
        }
        Some(pixel)
    });

    let get_raw_tile_data = |tile_ndx: i16, _width: i32, buffer: &mut [u8]| match tile_ndx {
        0 => {
            buffer[..4].copy_from_slice(&[10, 20, 30, 255]);
            true
        }
        4 => {
            buffer[..4].copy_from_slice(&[110, 120, 130, 255]);
            true
        }
        _ => false,
    };

    let blended = heightmap
        .get_pointer_to_tile_data(
            0,
            0,
            1,
            &source_tiles,
            &blend_tiles,
            &alpha_tiles,
            &get_raw_tile_data,
        )
        .expect("tile data should blend");

    assert_eq!(&blended[..4], &[110, 120, 130, 255]);
}

#[test]
fn terrain_source_tile_classes_load_tga_tiles_for_color_queries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let tile_path = temp.path().join("terrain_tile.tga");
    let image = RgbaImage::from_pixel(64, 64, Rgba([192, 128, 64, 255]));
    image.save(&tile_path).expect("write source tile image");

    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.tile_ndxes[(3 * 6 + 2) as usize] = 52;

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");

    let loaded = visual
        .load_source_tiles_from_texture_classes(&[TerrainSourceTileClass {
            first_tile: 13,
            num_tiles: 1,
            width: 1,
            name: tile_path.to_string_lossy().to_string(),
        }])
        .expect("source tile should load");

    assert_eq!(loaded, 1);
    let color = visual.get_terrain_color_at(1.25, 2.75).unwrap();
    assert_eq!(color, [192.0 / 255.0, 128.0 / 255.0, 64.0 / 255.0]);
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
fn skybox_replace_does_not_abort_when_default_tga_faces_are_missing() {
    // C++ W3DTerrainVisual::replaceSkyboxTextures (W3DTerrainVisual.cpp:1135)
    // applies each face independently via WaterRenderObjClass::replaceSkyboxTexture.
    // Rust used to `?` the whole set, aborting bind-group refresh when TSMorning*.tga
    // is absent (retail ships DDS).
    let mut visual = TerrainVisualImpl::new();
    let old = ["", "", "", "", ""];
    let missing = [
        "TSMorningN.tga",
        "TSMorningE.tga",
        "TSMorningS.tga",
        "TSMorningW.tga",
        "TSMorningT.tga",
    ];
    visual
        .replace_skybox_textures(&old, &missing)
        .expect("missing faces must not abort replace_skybox_textures");
    assert_eq!(
        option_names(visual.current_skybox_texture_names()),
        [
            Some("TSMorningN.tga"),
            Some("TSMorningE.tga"),
            Some("TSMorningS.tga"),
            Some("TSMorningW.tga"),
            Some("TSMorningT.tga")
        ]
    );
}

#[test]
fn skybox_candidates_swap_tga_to_dds_and_search_art_and_map_dir() {
    // C++ DDSFileClass (ddsfile.cpp:33-37) rewrites .tga → .dds;
    // W3DFileSystem.cpp:197-201 looks in TGA_DIR_PATH ("Art/Textures/").
    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(
            HeightMap::new(4, 4, 255.0, 1.0),
            Some(std::path::Path::new("maps/Alpine/Alpine.map")),
            None,
        )
        .expect("heightmap load sets map directory");
    let candidates = visual.skybox_texture_search_candidates("TSMorningN.tga");
    let as_str: Vec<String> = candidates
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(
        as_str
            .iter()
            .any(|c| c.ends_with("Art/Textures/TSMorningN.tga")
                || c == "Art/Textures/TSMorningN.tga"),
        "missing Art/Textures candidate: {as_str:?}"
    );
    assert!(
        as_str
            .iter()
            .any(|c| c.contains("art/textures/TSMorningN.tga")),
        "missing art/textures candidate: {as_str:?}"
    );
    assert!(
        as_str
            .iter()
            .any(|c| c.ends_with("Art/Textures/TSMorningN.dds")
                || c == "Art/Textures/TSMorningN.dds"),
        "missing tga→dds swap candidate: {as_str:?}"
    );
    assert!(
        as_str
            .iter()
            .any(|c| c.contains("maps/Alpine") && c.ends_with("TSMorningN.tga")),
        "missing map-directory candidate: {as_str:?}"
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
fn water_tracks_flush_is_called_from_live_water_record() {
    // C++ WaterTracksRenderSystem::flush is invoked from WaterRenderObjClass
    // (W3DWater.cpp / W3DWaterTracks.cpp). Live TerrainVisual must call it.
    use game_client_rust::terrain::WaterTrackType;
    let mut visual = TerrainVisualImpl::new();
    let handle = visual
        .water_tracks_mut()
        .bind_track(WaterTrackType::Pond)
        .expect("bind pond wake");
    visual.water_tracks_mut().track_mut(handle).unwrap().init(
        18.0,
        28.0,
        glam::Vec2::new(10.0, 20.0),
        glam::Vec2::new(10.0, 21.0),
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
fn water_grid_resolution_matches_cpp_y_only_reallocation_bug() {
    let mut visual = TerrainVisualImpl::new();

    visual.set_water_grid_resolution(4.0, 5.0, 10.0);
    visual.set_water_attenuation_factors(1.0, 0.0, 0.0, 10.0);
    visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
    visual.enable_water_grid(true);
    assert!(visual.change_water_height(10.0, 10.0, 2.0));
    visual.add_water_velocity(10.0, 10.0, 1.0, 3.0);
    assert_eq!(visual.water_grid_resolution(), (4.0, 5.0, 10.0));
    assert!(!visual.water_grid_state().height_deltas.is_empty());
    assert!(!visual.water_grid_state().velocity_events.is_empty());

    visual.set_water_grid_resolution(4.0, 9.0, 12.0);

    assert_eq!(visual.water_grid_resolution(), (4.0, 5.0, 12.0));
    assert!(!visual.water_grid_state().height_deltas.is_empty());
    assert!(!visual.water_grid_state().velocity_events.is_empty());

    visual.set_water_grid_resolution(6.0, 9.0, 12.0);

    assert_eq!(visual.water_grid_resolution(), (6.0, 9.0, 12.0));
    assert!(visual.water_grid_state().height_deltas.is_empty());
    assert!(visual.water_grid_state().velocity_events.is_empty());
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

#[test]
fn tree_vb_upload_uses_do_lighting_on_every_draw() {
    let mut visual = TerrainVisualImpl::new();
    visual.set_lighting(
        Some([0.0, -1.0, 0.0]),
        Some([0.8, 0.4, 0.0]),
        Some([0.2, 0.1, 0.0]),
        None,
        None,
    );

    let type_idx = visual
        .tree_buffer_mut()
        .add_tree_type(
            TreeModuleData {
                model_name: "Oak".into(),
                texture_name: "OakT".into(),
                ..TreeModuleData::default()
            },
            TreeSphere {
                center: Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();
    {
        let info = visual.tree_buffer_mut().tree_type_mut(type_idx).unwrap();
        info.tile_width = 1;
    }
    visual.tree_buffer_mut().set_tree_type_mesh(
        type_idx,
        TreeTypeMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: Some(vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: Some(vec![0xFFFF_FFFF, 0xFF80_0000, 0xFFFF_FFFF]),
            polygons: vec![[0, 1, 2]],
            emissive: [0.0, 0.0, 0.0],
        },
    );
    visual
        .tree_buffer_mut()
        .set_bounds(TreeRegion2D::new(Vec2::ZERO, Vec2::new(100.0, 100.0)));
    visual
        .tree_buffer_mut()
        .add_tree(
            1,
            Vec3::new(10.0, 20.0, 3.0),
            2.0,
            0.0,
            0.0,
            TreeModuleData {
                model_name: "Oak".into(),
                texture_name: "OakT".into(),
                ..TreeModuleData::default()
            },
            TreeSphere {
                center: Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();

    visual.update_tree_meshes();

    let lights = [game_client_rust::terrain::TreeObjectLight {
        ambient: [0.2, 0.1, 0.0],
        diffuse: [0.8, 0.4, 0.0],
        light_pos: [0.0, 0.0, -1.0],
    }];
    let up = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFFFF_FFFF, 1.0);
    let tinted = do_lighting([0.0, 0.0, 1.0], &lights, [0.0, 0.0, 0.0], 0xFF80_0000, 1.0);
    let side = do_lighting([1.0, 0.0, 0.0], &lights, [0.0, 0.0, 0.0], 0xFFFF_FFFF, 1.0);

    let gpu = visual.last_tree_gpu_vertices();
    assert_eq!(gpu.len(), 3);
    assert_eq!(gpu[0].diffuse, up);
    assert_eq!(gpu[1].diffuse, tinted);
    assert_eq!(gpu[2].diffuse, side);
    assert_eq!(gpu[0].position, [10.0, 3.0, 20.0]);

    // Second draw still ships the doLighting VB (C++ dirty skip keeps last fill).
    visual.update_tree_meshes();
    assert_eq!(visual.last_tree_gpu_vertices()[0].diffuse, up);
    assert_eq!(visual.last_tree_gpu_vertices()[2].diffuse, side);
}

#[test]
fn tree_atlas_mips_reach_terrain_visual_upload_path() {
    let mut visual = TerrainVisualImpl::new();
    visual
        .tree_buffer_mut()
        .add_tree_type(
            TreeModuleData {
                model_name: "Oak".into(),
                texture_name: "Oak.tga".into(),
                ..TreeModuleData::default()
            },
            TreeSphere::default(),
        )
        .unwrap();
    visual
        .tree_buffer_mut()
        .update_texture(&[TreeTileImageSpec {
            texture_name: "Oak.tga".into(),
            header: TreeTgaHeader::truecolor(64, 64),
        }]);
    let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
    tile[0..4].copy_from_slice(&[11, 22, 33, 44]);
    assert!(visual.tree_buffer_mut().set_source_tile_bgra(0, &tile));
    assert_eq!(visual.tree_buffer_mut().update_tree_texture_class(1), 512);
    visual.update_tree_meshes();
    let expected_len = visual.tree_buffer_mut().atlas_mips().len() - 1;
    let uploaded = visual.last_tree_atlas_mips();
    assert!(!uploaded.is_empty());
    assert_eq!(uploaded.len(), expected_len);
    assert_eq!(uploaded[0].len(), 256 * 256 * 4);
}

#[test]
fn tree_atlas_live_draw_matches_cpp_blit_mip_and_lod() {
    let mut visual = TerrainVisualImpl::new();
    visual
        .tree_buffer_mut()
        .add_tree_type(
            TreeModuleData {
                model_name: "Oak".into(),
                texture_name: "Oak.tga".into(),
                ..TreeModuleData::default()
            },
            TreeSphere::default(),
        )
        .unwrap();
    let mut tile = vec![0u8; TREE_TILE_DATA_LEN];
    tile[0..4].copy_from_slice(&[11, 22, 33, 44]);
    assert!(visual.tree_buffer_mut().set_source_tile_bgra(0, &tile));
    visual
        .tree_buffer_mut()
        .update_texture(&[TreeTileImageSpec {
            texture_name: "Oak.tga".into(),
            header: TreeTgaHeader::truecolor(64, 64),
        }]);
    visual.tree_buffer_mut().set_texture_lod(1);
    visual.update_tree_meshes();

    let (width, height) = visual.tree_buffer_mut().texture_size();
    let loc = visual
        .tree_buffer_mut()
        .tile_location_in_texture(0)
        .expect("packed tile");
    let mut level0 = vec![0u8; (width as usize) * (height as usize) * TILE_BYTES_PER_PIXEL];
    blit_tree_tile_into_atlas(&mut level0, width, &tile, loc.0, loc.1);
    let expected = generate_box_mip_chain(&level0, width, height);
    let uploaded = visual.last_tree_atlas_mips();
    assert!(!uploaded.is_empty());
    assert_eq!(uploaded[0].len(), expected[1].len());
    assert_eq!(uploaded[0], expected[1]);
    assert_eq!(do_tree_atlas_mip(&expected[0], width), expected[1]);
    let dest = (63 * 512 + 0) * 4;
    assert_eq!(&expected[0][dest..dest + 4], &[11, 22, 33, 44]);
}

/// C++ BaseHeightMap.cpp:618 `reset` → `clearAllScorches`.
#[test]
fn terrain_visual_reset_clears_all_scorches() {
    add_terrain_scorch([12.0, 8.0, 0.0], 18.0, 2);
    assert!(
        terrain_scorch_count() > 0,
        "pre-reset scorches must exist so the clear is observable"
    );
    let mut visual = TerrainVisualImpl::new();
    visual.reset().expect("TerrainVisual::reset");
    assert_eq!(
        terrain_scorch_count(),
        0,
        "hq-y8cc2: reset must call clear_terrain_scorches"
    );
}
