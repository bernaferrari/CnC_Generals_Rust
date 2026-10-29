use game_client_rust::display::display::DisplayLightingState;
use game_client_rust::fx_list::{DisplayLightPulse, DisplayLightScene};
use game_client_rust::system::SubsystemInterface;
use game_client_rust::terrain::HeightMap;
use game_client_rust::terrain::chunk::TerrainStaticLighting;
use game_client_rust::terrain::scorch_mesh::{TerrainScorchBuffer, scorch_diffuse_bgra};
use game_client_rust::terrain::terrain_visual::TerrainVisualImpl;
use game_engine::common::ini::ini_game_data::{
    LightingContentRules, RGBColor, TerrainTrackRules, TimeOfDay,
};
use game_engine::common::ini::ini_terrain::TerrainTypes;
use glam::{Vec3, camera::rh::view::look_at_mat4};
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn missing_skybox_face_retries_after_receiving_visual_reset() {
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::local_file_system::LocalFileSystem;

    let root = tempfile::tempdir().unwrap();
    let face_path = root.path().join("OwnerSkyboxAppearsAfterReset.png");
    let face_name = face_path.to_string_lossy().into_owned();
    {
        let file_system = get_file_system();
        let mut file_system = file_system.lock().unwrap();
        let local: &mut LocalFileSystem = file_system.ensure_backend(LocalFileSystem::new);
        local.add_search_path(root.path());
        file_system.clear_cache();
    }

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("skybox retry regression requires a WGPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("receiving skybox retry test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .expect("skybox retry regression requires a WGPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let mut visual = TerrainVisualImpl::new();
    visual
        .init_gpu_resources(Arc::clone(&device), Arc::clone(&queue))
        .unwrap();

    let empty = ["", "", "", "", ""];
    let missing_face = [face_name.as_str(), "", "", "", ""];
    visual
        .replace_skybox_textures(&empty, &missing_face)
        .unwrap();
    assert_ne!(visual.last_skybox_face_bind(), Some(face_name.as_str()));

    let encoded = {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([220, 40, 30, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    };
    std::fs::write(&face_path, encoded).unwrap();

    // A peer's independent miss cache must not inherit the first visual's
    // failure for the same name after the asset becomes available.
    let mut peer = TerrainVisualImpl::new();
    peer.init_gpu_resources(device, queue).unwrap();
    peer.replace_skybox_textures(&empty, &missing_face).unwrap();
    assert_eq!(peer.last_skybox_face_bind(), Some(face_name.as_str()));

    // A miss is retained by this visual for its current asset context. Reset
    // clears that cache; the same mounted file then loads into the actual GPU
    // face and is selected for the background bind.
    visual
        .replace_skybox_textures(&empty, &missing_face)
        .unwrap();
    assert_ne!(visual.last_skybox_face_bind(), Some(face_name.as_str()));
    visual.reset().unwrap();
    visual
        .replace_skybox_textures(&empty, &missing_face)
        .unwrap();
    assert_eq!(visual.last_skybox_face_bind(), Some(face_name.as_str()));

    drop(peer);
    drop(visual);
    let file_system = get_file_system();
    let mut file_system = file_system.lock().unwrap();
    if let Some(local) = file_system.get_backend_mut::<LocalFileSystem>() {
        local.remove_search_path(root.path());
    }
    file_system.clear_cache();
}

fn lighting(ambient: [f32; 3]) -> TerrainStaticLighting {
    TerrainStaticLighting {
        lights: vec![(Vec3::new(0.0, -1.0, 0.0), [0.05; 3])],
        ambient,
        scorch_diffuse: [0.2; 3],
    }
}

#[test]
fn receiving_lighting_changes_baked_scorch_vertex_color() {
    let mut scorch = TerrainScorchBuffer::new();
    assert!(scorch.add_scorch([12.0, 8.0, 0.0], 18.0, 2));
    let height = HeightMap::new(8, 8, 0.0, 10.0);

    let receiving = TerrainStaticLighting {
        lights: Vec::new(),
        ambient: [0.1, 0.2, 0.3],
        scorch_diffuse: [0.2, 0.4, 0.6],
    };
    let first = scorch.update_scorches(
        &height,
        scorch_diffuse_bgra(receiving.ambient, receiving.scorch_diffuse),
    );
    assert_eq!(first.vertices[0].diffuse, 0xFF33_6699);

    let changed = TerrainStaticLighting {
        ambient: [0.2, 0.1, 0.0],
        scorch_diffuse: [0.4, 0.8, 0.6],
        ..receiving
    };
    scorch.scorches_in_buffer = 0;
    let second = scorch.update_scorches(
        &height,
        scorch_diffuse_bgra(changed.ambient, changed.scorch_diffuse),
    );
    assert_eq!(second.vertices[0].diffuse, 0xFF66_7F4C);
    assert_ne!(second.vertices[0].diffuse, first.vertices[0].diffuse);
}

fn visual(light: TerrainStaticLighting) -> TerrainVisualImpl {
    let mut visual = TerrainVisualImpl::new();
    visual.admit_terrain_definitions(Arc::new(TerrainTypes::new()));
    visual.admit_static_lighting(light);
    visual
        .load_heightmap_from_data(HeightMap::new(8, 8, 1.0, 10.0), None, None)
        .unwrap();
    visual.update().unwrap();
    visual
}

fn first_color(visual: &TerrainVisualImpl) -> [f32; 4] {
    visual
        .chunk_manager()
        .iter_chunks()
        .find_map(|chunk| chunk.base_colors.first().copied())
        .expect("terrain update must bake a chunk")
}

fn first_render_color(visual: &TerrainVisualImpl) -> [f32; 4] {
    visual
        .chunk_manager()
        .iter_chunks()
        .find_map(|chunk| chunk.vertices.first().map(|vertex| vertex.color))
        .expect("terrain update must bake a chunk")
}

#[test]
fn interleaved_terrain_owners_keep_their_own_baked_lighting() {
    let mut first = visual(lighting([0.8, 0.4, 0.4]));
    let first_color_before = first_color(&first);
    let mut peer = visual(lighting([0.4, 0.8, 0.4]));
    let peer_color_before = first_color(&peer);
    assert!(first_color_before[0] > first_color_before[1]);
    assert!(peer_color_before[1] > peer_color_before[0]);

    assert!(peer.admit_static_lighting(lighting([0.4, 0.4, 0.8])));
    peer.update().unwrap();
    assert!(first_color(&peer)[2] > first_color(&peer)[0]);
    first.update().unwrap();
    assert_eq!(first_color(&first), first_color_before);

    peer.reset().unwrap();
    drop(peer);
    first.update().unwrap();
    assert_eq!(first_color(&first), first_color_before);
}

#[test]
fn client_display_lighting_and_skybox_use_the_receiving_owner() {
    let mut first_rules = LightingContentRules::default();
    let mut peer_rules = LightingContentRules::default();
    first_rules.time_of_day = TimeOfDay::Night;
    peer_rules.time_of_day = TimeOfDay::Night;
    first_rules.objects[TimeOfDay::Night as usize][0].ambient = RGBColor::new(0.8, 0.1, 0.1);
    peer_rules.objects[TimeOfDay::Night as usize][0].ambient = RGBColor::new(0.1, 0.8, 0.1);
    let first = DisplayLightingState::from_lighting_rules(&first_rules, TimeOfDay::Invalid, true);
    let peer = DisplayLightingState::from_lighting_rules(&peer_rules, TimeOfDay::Invalid, true);
    assert_eq!(first.scene_ambient, [0.8, 0.1, 0.1]);
    assert_eq!(peer.scene_ambient, [0.1, 0.8, 0.1]);

    let mut first_terrain = TerrainVisualImpl::new();
    let mut peer_terrain = TerrainVisualImpl::new();
    first_terrain.set_skybox_enabled(true);
    assert!(first_terrain.skybox_enabled());
    assert!(!peer_terrain.skybox_enabled());
    peer_terrain.set_skybox_enabled(true);
    first_terrain.reset().unwrap();
    assert!(!first_terrain.skybox_enabled());
    assert!(peer_terrain.skybox_enabled());
}

#[test]
fn dynamic_light_and_expiry_are_isolated_between_terrain_owners() {
    use game_client_rust::terrain::w3d_overlay_mesh::{
        bake_straight_road_segment, bake_water_patch_world_with_lights, light_road_seg_vertices,
    };
    let mut first = visual(lighting([0.4; 3]));
    let mut peer = visual(lighting([0.4; 3]));
    let peer_before = first_render_color(&peer);
    let mut scene = DisplayLightScene::default();
    assert!(scene.create_pulse(DisplayLightPulse {
        pos: [0.0, 0.0, 10.0],
        color: [0.5, 0.0, 0.0],
        inner_radius: 10.0,
        outer_radius: 100.0,
        increase_frames: 0,
        decay_frames: 0,
    }));
    first.admit_dynamic_lights(scene.lights());
    first.update().unwrap();
    peer.update().unwrap();
    assert!(first_render_color(&first)[0] > peer_before[0]);
    assert_eq!(first_render_color(&peer), peer_before);

    let (lit_water, _, _) =
        bake_water_patch_world_with_lights(0.0, 0.0, 20.0, 20.0, 0.0, 0xff33_3333, scene.lights());
    let (peer_water, _, _) =
        bake_water_patch_world_with_lights(0.0, 0.0, 20.0, 20.0, 0.0, 0xff33_3333, &[]);
    assert!(lit_water[0].c > peer_water[0].c);
    let (mut road, _) =
        bake_straight_road_segment([0.0, 0.0], [30.0, 0.0], 10.0, 1.0, 0.0, 0.0, 1.0, |_, _| {
            0.0
        })
        .unwrap();
    for vertex in &mut road {
        vertex.diffuse = 0xff33_3333;
    }
    light_road_seg_vertices(&mut road, 0xff33_3333, scene.lights());
    assert!(road[0].diffuse > 0xff33_3333);

    scene.clear_lights();
    first.admit_dynamic_lights(scene.lights());
    first.update().unwrap();
    assert_eq!(first_render_color(&first), peer_before);
    assert_eq!(first_render_color(&peer), peer_before);
}

#[test]
fn terrain_track_pools_use_each_worlds_ini_settings() {
    let defaults = LightingContentRules::default();
    let first_rules = defaults
        .overlay_ini_text("GameData\n MaxTerrainTracks = 1\n MakeTrackMarks = No\nEnd\n")
        .unwrap();
    let peer_rules = defaults
        .overlay_ini_text("GameData\n MaxTerrainTracks = 2\nEnd\n")
        .unwrap();
    let mut first = TerrainVisualImpl::new_with_track_rules(&first_rules.terrain_tracks);
    let mut peer = TerrainVisualImpl::new_with_track_rules(&peer_rules.terrain_tracks);
    assert_eq!(first.terrain_tracks_mut().config().max_terrain_tracks, 1);
    assert!(!first.terrain_tracks_mut().config().make_track_marks);
    assert_eq!(peer.terrain_tracks_mut().config().max_terrain_tracks, 2);
    assert!(peer.terrain_tracks_mut().config().make_track_marks);

    assert!(
        first
            .terrain_tracks_mut()
            .bind_track(1.0, 1.0, "first")
            .is_some()
    );
    assert!(
        first
            .terrain_tracks_mut()
            .bind_track(1.0, 1.0, "extra")
            .is_none()
    );
    assert!(
        peer.terrain_tracks_mut()
            .bind_track(1.0, 1.0, "peer-one")
            .is_some()
    );
    assert!(
        peer.terrain_tracks_mut()
            .bind_track(1.0, 1.0, "peer-two")
            .is_some()
    );
    first.reset().unwrap();
    assert!(
        peer.terrain_tracks_mut()
            .bind_track(1.0, 1.0, "extra")
            .is_none()
    );
    assert!(
        first
            .terrain_tracks_mut()
            .bind_track(1.0, 1.0, "after-reset")
            .is_some()
    );
}

#[test]
fn repeated_custom_track_detail_resets_only_the_receiving_visual() {
    use game_client_rust::terrain::{TerrainTrackHeightProvider, TerrainTracksConfig};
    struct Flat;
    impl TerrainTrackHeightProvider for Flat {
        fn ground_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
            (0.0, Vec3::Y)
        }
    }

    let first_rules = TerrainTrackRules {
        max_terrain_tracks: 2,
        max_tank_track_edges: 6,
        max_tank_track_opaque_edges: 2,
        max_tank_track_fade_delay: 90,
        make_track_marks: true,
    };
    let peer_rules = TerrainTrackRules {
        max_terrain_tracks: 2,
        max_tank_track_edges: 3,
        max_tank_track_opaque_edges: 1,
        max_tank_track_fade_delay: 30,
        make_track_marks: false,
    };
    let mut first = TerrainVisualImpl::new_with_track_rules(&first_rules);
    let mut peer = TerrainVisualImpl::new_with_track_rules(&peer_rules);
    let ground = Flat;
    let first_track = first
        .terrain_tracks_mut()
        .bind_track(2.0, 6.0, "first")
        .unwrap();
    let peer_track = peer
        .terrain_tracks_mut()
        .bind_track(2.0, 6.0, "peer")
        .unwrap();
    first
        .terrain_tracks_mut()
        .add_edge_to_track(first_track, &ground, 0.0, 0.0, 0);
    first
        .terrain_tracks_mut()
        .add_edge_to_track(first_track, &ground, 20.0, 0.0, 1);
    peer.terrain_tracks_mut()
        .add_edge_to_track(peer_track, &ground, 0.0, 0.0, 0);
    peer.terrain_tracks_mut()
        .add_edge_to_track(peer_track, &ground, 20.0, 0.0, 1);
    assert_eq!(
        first
            .terrain_tracks()
            .track(first_track)
            .unwrap()
            .active_edge_count(),
        1
    );
    assert_eq!(
        peer.terrain_tracks()
            .track(peer_track)
            .unwrap()
            .active_edge_count(),
        1
    );

    // C++ non-Custom levels are suppressed when already current; Custom is
    // reapplied and clears tracks on each accepted Options event.
    let custom = TerrainTracksConfig::from_rules(&first_rules);
    first.set_terrain_tracks_detail_with_config(custom);
    assert_eq!(
        first
            .terrain_tracks()
            .track(first_track)
            .unwrap()
            .active_edge_count(),
        0
    );
    assert_eq!(
        peer.terrain_tracks()
            .track(peer_track)
            .unwrap()
            .active_edge_count(),
        1
    );
    first
        .terrain_tracks_mut()
        .add_edge_to_track(first_track, &ground, 30.0, 0.0, 2);
    first.set_terrain_tracks_detail_with_config(custom);
    assert_eq!(
        first
            .terrain_tracks()
            .track(first_track)
            .unwrap()
            .active_edge_count(),
        0
    );
    assert_eq!(
        peer.terrain_tracks()
            .track(peer_track)
            .unwrap()
            .active_edge_count(),
        1
    );
    assert_eq!(first.terrain_tracks().config().max_tank_track_edges, 6);
    assert_eq!(peer.terrain_tracks().config().max_tank_track_edges, 3);
}

#[test]
fn terrain_water_plane_and_settings_follow_the_receiving_world() {
    let defaults = LightingContentRules::default();
    let first_rules = defaults
        .overlay_ini_text(
            "GameData\n WaterPositionX = 10\n WaterPositionY = 20\n WaterPositionZ = 3\n WaterExtentX = 80\n WaterExtentY = 40\n WaterType = 2\n ShowSoftWaterEdge = No\n FeatherWater = 2\nEnd\n",
        )
        .unwrap()
        .overlay_ini_text("GameData\n WaterPositionZ = 5\nEnd\n")
        .unwrap();
    let peer_rules = defaults
        .overlay_ini_text(
            "GameData\n WaterPositionX = -8\n WaterPositionY = 6\n WaterPositionZ = 9\n WaterExtentX = 12\n WaterExtentY = 30\n WaterType = 2\n ShowSoftWaterEdge = Yes\nEnd\n",
        )
        .unwrap();
    let mut first = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    first.admit_water_rules(first_rules.terrain_water_rules());
    peer.admit_water_rules(peer_rules.terrain_water_rules());
    assert_eq!(
        first.water_rules().sea_plane_bounds(),
        Some(([10.0, 20.0], [90.0, 60.0], 5.0))
    );
    assert_eq!(
        peer.water_rules().sea_plane_bounds(),
        Some(([-8.0, 6.0], [4.0, 36.0], 9.0))
    );
    assert!(!first.water_rules().show_soft_water_edge);
    assert!(peer.water_rules().show_soft_water_edge);
    assert_eq!(first.water_rules().feather_water, 2);

    let mut changed = peer.water_rules();
    changed.water_type = 0;
    peer.admit_water_rules(changed);
    assert_eq!(peer.water_rules().sea_plane_bounds(), None);
    peer.reset().unwrap();
    drop(peer);
    assert_eq!(
        first.water_rules().sea_plane_bounds(),
        Some(([10.0, 20.0], [90.0, 60.0], 5.0))
    );
}

#[test]
fn water_transparency_and_skybox_follow_the_receiving_visual() {
    use game_engine::common::ini::INILoadType;
    use game_engine::common::ini::ini_water::WaterDefinitions;

    let base = WaterDefinitions::default()
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 3\nSkyboxTextureN = BaseN.tga\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let first_rules = base
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 7\nSkyboxTextureN = FirstN.tga\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let peer_rules = base
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 11\nSkyboxTextureN = PeerN.tga\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let mut first = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    first
        .admit_water_presentation(&first_rules.presentation())
        .unwrap();
    peer.admit_water_presentation(&peer_rules.presentation())
        .unwrap();
    assert_eq!(first.water_presentation().transparent_water_depth, 7.0);
    assert_eq!(peer.water_presentation().transparent_water_depth, 11.0);
    assert_eq!(
        first.current_skybox_texture_names()[0].as_deref(),
        Some("FirstN.tga")
    );
    assert_eq!(
        peer.current_skybox_texture_names()[0].as_deref(),
        Some("PeerN.tga")
    );
    peer.reset().unwrap();
    drop(peer);
    assert_eq!(
        first.current_skybox_texture_names()[0].as_deref(),
        Some("FirstN.tga")
    );
    assert_eq!(first.water_presentation().transparent_water_depth, 7.0);
}

#[test]
fn terrain_skybox_uses_the_receiving_view() {
    let mut first = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    let north = look_at_mat4(Vec3::ZERO, Vec3::Z, Vec3::Y);
    let east = look_at_mat4(Vec3::ZERO, Vec3::X, Vec3::Y);
    first.set_skybox_view(&north);
    peer.set_skybox_view(&east);
    assert_eq!(first.skybox_view_face(), 0);
    assert_eq!(peer.skybox_view_face(), 1);
    let upward = look_at_mat4(Vec3::ZERO, Vec3::new(0.0, 2.0, 1.0), Vec3::Y);
    first.set_skybox_view(&upward);
    assert_eq!(first.skybox_view_face(), 4);
    assert_eq!(peer.skybox_view_face(), 1);
    let vertical = look_at_mat4(Vec3::ZERO, Vec3::Y, Vec3::Z);
    first.set_skybox_view(&vertical);
    assert_eq!(first.skybox_view_face(), 4);
    first.set_skybox_view(&east);
    first.reset().unwrap();
    assert_eq!(first.skybox_view_face(), 0);
    assert_eq!(peer.skybox_view_face(), 1);
}
