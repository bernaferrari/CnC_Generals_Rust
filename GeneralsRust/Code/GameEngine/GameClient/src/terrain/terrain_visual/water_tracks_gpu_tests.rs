use super::*;
use crate::terrain::water_tracks::{
    WaterTrackDrawRange, WaterTrackType, WaterTrackVertex, WaterTracksFlush,
    water_track_strip_indices,
};

#[test]
#[ignore = "requires a serialized native GPU"]
fn unchanged_water_and_shoreline_keep_their_gpu_buffers() {
    use game_engine::common::common_services::{CommonServices, with_common_services};

    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));
    let points = [[0.0, 0.0, 1.0], [4.0, 0.0, 1.0], [4.0, 4.0, 1.0], [0.0, 4.0, 1.0]];
    visual.overlay.shoreline_tiles.push(crate::terrain::ShoreLineTile {
        packed_xy: 0,
        verts: points,
        t: [0.0, 0.25, 0.5, 0.75],
    });
    visual.set_map_water_areas(vec![TerrainWaterArea {
        points: points.to_vec(),
        is_river: false,
        river_start: 0,
    }]);
    visual.rebuild_all_overlays();
    let buffers = |visual: &TerrainVisualImpl| {
        let water = &visual.polygon_water_meshes[0];
        let shore = &visual.shoreline_meshes[0];
        (water.vertex_buffer.clone(), shore.vertex_buffer.clone())
    };
    let (first_water, first_shore) = buffers(&visual);
    // Client-frame refresh and a separate dirty-overlay flush can run together.
    visual.sync_polygon_water_meshes(&device).unwrap();
    visual.rebuild_all_overlays();
    assert_eq!(buffers(&visual), (first_water.clone(), first_shore.clone()));

    visual.overlay.river_v_origin += 0.002;
    visual.sync_polygon_water_meshes(&device).unwrap();
    let (advanced_water, unchanged_shore) = buffers(&visual);
    assert_ne!(advanced_water, first_water);
    assert_eq!(unchanged_shore, first_shore);

    let elevated = points.map(|[x, y, z]| [x, y, z + 1.0]);
    visual.set_map_water_areas(vec![TerrainWaterArea {
        points: elevated.to_vec(),
        is_river: false,
        river_start: 0,
    }]);
    let (replaced_water, _) = buffers(&visual);
    assert_ne!(replaced_water, advanced_water);
    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || visual.init_gpu_resources(device, queue)).unwrap();
    visual.rebuild_all_overlays();
    let (recreated_water, recreated_shore) = buffers(&visual);
    assert_ne!(recreated_water, replaced_water);
    assert_ne!(recreated_shore, first_shore);

    visual.set_map_water_areas(Vec::new());
    assert!(visual.polygon_water_meshes.is_empty());
}

#[test]
#[ignore = "requires native GPU and the mounted retail map and textures"]
fn lone_eagle_polygon_water_reuses_indices_across_visible_animation_phases() {
    use game_engine::common::ini::ini::INILoadType;
    use game_engine::common::ini::ini_game_data::LightingContentRules;
    use game_engine::common::ini::ini_water::WaterDefinitions;
    use game_engine::common::system::big_file_system::{BigArchiveBackend, BigFileSystem};
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::subsystem_interface::SubsystemInterface;
    use std::io::Read;

    let retail = std::env::var_os("GENERALS_ASSETS_DIR").expect("retail Zero Hour assets required");
    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail Generals textures required");
    let mut maps = BigFileSystem::new();
    maps.open_archive_file(Path::new(&retail).join("Data/MapsZH.big"))
        .unwrap();
    let mut bytes = Vec::new();
    maps.open_file("Maps/Lone Eagle/Lone Eagle.map", 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut ini_archive = BigFileSystem::new();
    ini_archive
        .open_archive_file(Path::new(&retail).join("Data/INIZH.big"))
        .unwrap();
    let mut water_ini = String::new();
    ini_archive
        .open_file("Data/INI/Water.ini", 0)
        .unwrap()
        .read_to_string(&mut water_ini)
        .unwrap();
    let mut game_data_ini = String::new();
    ini_archive
        .open_file("Data/INI/GameData.ini", 0)
        .unwrap()
        .read_to_string(&mut game_data_ini)
        .unwrap();
    let source = tempfile::tempdir().unwrap();
    std::fs::hard_link(
        Path::new(&base).join("Data/Textures.big"),
        source.path().join("Textures.big"),
    )
    .unwrap();
    let world = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&world, || {
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(source.path());
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }

        let areas = mounted_water_areas(&bytes);
        assert_eq!(areas.len(), 3);
        assert_eq!(areas.iter().filter(|area| area.is_river).count(), 2);
        let (min, max) = water_test_bounds(&areas);

        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        let water = WaterDefinitions::default()
            .overlay_ini_text(&water_ini, INILoadType::Overwrite)
            .unwrap()
            .presentation();
        assert!(water.setting_at(visual.water_rules().time_of_day).surface_color.3 > 0.0);
        visual.admit_water_presentation(&water).unwrap();
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        assert!(
            visual.river_gpu.river_tex.as_ref().unwrap().width() > 1,
            "the native polygon shader must bind the mounted TWWater01 art",
        );
        assert_eq!(visual.river_gpu.river_tex, visual.water_texture);
        let edge = visual.river_gpu.detail_views.as_ref().unwrap().edge.texture();
        assert_eq!(
            (edge.width(), edge.height()),
            (256, 256),
            "the mounted river uses the stock TWAlphaEdge texture"
        );
        set_water_test_camera(&mut visual, &device, min, max);
        visual.set_map_water_areas(areas.clone());
        assert!(
            !visual.sync_river_params(),
            "new buffers contain the current phase"
        );
        let phase_0 = read_water_pixels(&visual, &device, &queue, true);
        assert!(
            phase_0.chunks_exact(4).any(|pixel| pixel[..3] != [0; 3]),
            "the native polygon pass must paint the installed map geometry",
        );
        let read_shaded = |visual: &mut TerrainVisualImpl| {
            let cell_size = (max - min).max_element() / 4.0;
            visual.set_shroud_overlay_r8(5, 5, cell_size, min.to_array(), &[160; 25]);
            visual.sync_shroud_dest_texture();
            let pixels = read_water_pixels(visual, &device, &queue, true);
            visual.clear_shroud_overlay();
            pixels
        };
        let shaded_phase_0 = read_shaded(&mut visual);
        assert_ne!(shaded_phase_0, phase_0);
        let vertex_buffer_count = |meshes: &[GpuWaterPlane]| {
            meshes
                .iter()
                .map(|mesh| &mesh.vertex_buffer)
                .collect::<std::collections::HashSet<_>>()
                .len()
        };
        let retained = std::mem::take(&mut visual.polygon_water_meshes);
        assert!(!retained.is_empty());
        let mut fingerprints = vec![crc32fast::hash(&phase_0)];
        let mut vertex_buffers_per_phase = vec![vertex_buffer_count(&retained)];
        let mut index_reuse_per_phase = Vec::new();
        let mut repeated_uploads = (0..4).filter(|_| visual.sync_river_params()).count();
        for phase in [0.002, 0.004, 0.2, 0.474] {
            visual.overlay.river_v_origin = phase;
            assert!(visual.sync_river_params());
            repeated_uploads += (0..4).filter(|_| visual.sync_river_params()).count();
            visual.sync_polygon_water_meshes(&device).unwrap();
            assert_eq!(visual.polygon_water_meshes.len(), retained.len());
            vertex_buffers_per_phase.push(vertex_buffer_count(&visual.polygon_water_meshes));
            let mut same_indices = 0;
            for (mesh, initial) in visual.polygon_water_meshes.iter().zip(&retained) {
                assert_ne!(mesh.vertex_buffer, initial.vertex_buffer);
                assert_eq!(
                    (mesh.index_count, mesh.river),
                    (initial.index_count, initial.river)
                );
                same_indices += usize::from(mesh.index_buffer == initial.index_buffer);
            }
            index_reuse_per_phase.push(same_indices);
            fingerprints.push(crc32fast::hash(&read_water_pixels(
                &visual, &device, &queue, true,
            )));
        }
        assert!(
            fingerprints.windows(2).any(|a| a[0] != a[1]),
            "native pixels witness active wave phase changes"
        );
        // An earlier recorded mesh can remain alive while later animation advances.
        let latest = std::mem::replace(&mut visual.polygon_water_meshes, retained);
        visual.overlay.river_v_origin = 0.0;
        assert!(visual.sync_river_params());
        assert_eq!(read_water_pixels(&visual, &device, &queue, true), phase_0);
        assert_eq!(read_shaded(&mut visual), shaded_phase_0);
        let retained = std::mem::replace(&mut visual.polygon_water_meshes, latest);

        visual.set_map_water_areas(Vec::new());
        assert!(visual.polygon_water_meshes.is_empty());
        let source_count = areas.len();
        visual.set_map_water_areas(areas);
        assert_eq!(visual.polygon_water_meshes.len(), retained.len());
        assert!(
            visual
                .polygon_water_meshes
                .iter()
                .zip(&retained)
                .all(|(mesh, initial)| mesh.index_buffer != initial.index_buffer)
        );
        let index_bytes: u64 = retained.iter().map(|mesh| mesh.index_buffer.size()).sum();
        eprintln!(
            "Lone Eagle {source_count} water areas and {} GPU meshes ({index_bytes} index bytes), retained indices {index_reuse_per_phase:?}, phase CRC32 {fingerprints:08x?}",
            retained.len()
        );
        assert_eq!(index_reuse_per_phase, vec![retained.len(); 4]);

        let original_groups = [
            visual.river_gpu.bind_group.clone(),
            visual.river_gpu.trapezoid_bind_group.clone(),
        ];
        let original_params = [
            visual.river_gpu.params.clone(),
            visual.river_gpu.trapezoid_params.clone(),
        ];
        visual.ensure_named_water_bind_group(&device, "Wave256.tga");
        let named_wake = visual.water_named_bind_groups["Wave256.tga"].bind_group.clone();
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        set_water_test_camera(&mut visual, &device, min, max);
        visual.rebuild_all_overlays();
        assert!(
            !visual.sync_river_params(),
            "recreated buffers contain the current phase"
        );
        assert_eq!(
            visual.water_named_bind_groups["Wave256.tga"].bind_group,
            named_wake
        );
        let rebound_groups = [
            visual.river_gpu.bind_group.as_ref() != original_groups[0].as_ref(),
            visual.river_gpu.trapezoid_bind_group.as_ref() != original_groups[1].as_ref(),
        ];
        assert_ne!(visual.river_gpu.params, original_params[0]);
        assert_ne!(visual.river_gpu.trapezoid_params, original_params[1]);
        let reinitialized_pixels = read_water_pixels(&visual, &device, &queue, true);
        eprintln!(
            "Lone Eagle river parameter updates on 20 unchanged presents: {repeated_uploads}, groups rebound to recreated buffers: {rebound_groups:?}, reinitialized phase CRC32 {:08x}",
            crc32fast::hash(&reinitialized_pixels),
        );
        assert_eq!((repeated_uploads, rebound_groups), (0, [true; 2]));
        assert_eq!(reinitialized_pixels, phase_0);

        let (replacement_device, replacement_queue) = native_water_test_device_with_limits(
            wgpu::Limits {
                max_buffer_size: 10 * 1024,
                ..Default::default()
            },
        );
        visual
            .init_gpu_resources(replacement_device.clone(), replacement_queue.clone())
            .unwrap();
        assert!(visual.water_named_bind_groups.is_empty());
        visual.ensure_named_water_bind_group(&replacement_device, "Wave256.tga");
        assert!(visual.water_named_bind_groups.contains_key("Wave256.tga"));
        set_water_test_camera(&mut visual, &replacement_device, min, max);
        visual.rebuild_all_overlays();
        assert_eq!(vertex_buffer_count(&visual.polygon_water_meshes), 2);
        assert!(!visual.sync_river_params());
        assert_eq!(
            read_water_pixels(&visual, &replacement_device, &replacement_queue, true),
            phase_0
        );
        eprintln!(
            "Lone Eagle polygon vertex buffers per phase {vertex_buffers_per_phase:?}, retained shroud CRC32 {:08x}",
            crc32fast::hash(&shaded_phase_0),
        );
        assert_eq!(vertex_buffers_per_phase, vec![1; 5]);

        // Stock afternoon GameData applied to the mounted geometry, using
        // the original GlobalData.cpp default count for the receiving inputs.
        let mut authored = LightingContentRules::default();
        authored.num_global_lights = 3;
        let authored = authored.overlay_ini_text(&game_data_ini).unwrap();
        assert_eq!(authored.time_of_day as u8, visual.water_rules().time_of_day);
        assert_eq!(authored.num_global_lights, 3);
        let lights = &authored.terrain[authored.time_of_day as usize];
        let ambient = &lights[0].ambient;
        visual.admit_static_lighting(crate::terrain::chunk::TerrainStaticLighting {
            lights: lights
                .iter()
                .map(|light| {
                    let pos = &light.light_pos;
                    let color = &light.diffuse;
                    (Vec3::new(pos.x, pos.z, pos.y), [color.r, color.g, color.b])
                })
                .collect(),
            ambient: [ambient.r, ambient.g, ambient.b],
            scorch_diffuse: [1.0; 3],
        });
        let diffuse = visual.standing_water_diffuse_packed();
        visual.rebuild_all_overlays();
        let pixels = read_water_pixels(&visual, &replacement_device, &replacement_queue, true);
        eprintln!(
            "Retail GameData/Water.ini afternoon diffuse {diffuse:08x}, mounted water CRC32 {:08x}",
            crc32fast::hash(&pixels)
        );
        assert_eq!(diffuse, 0xff83_8078);
    });
}

#[test]
#[ignore = "requires native GPU and the mounted retail campaign map and textures"]
fn usa05_polygon_water_uses_the_mounted_map_standing_texture() {
    use game_engine::common::ini::ini::INILoadType;
    use game_engine::common::ini::ini_water::WaterDefinitions;
    use game_engine::common::system::big_file_system::{BigArchiveBackend, BigFileSystem};
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::subsystem_interface::SubsystemInterface;
    use gamelogic::system::engine_stores::{new_world_services, with_world_services};
    use std::io::Read;

    let retail = std::env::var_os("GENERALS_ASSETS_DIR").expect("retail Zero Hour assets required");
    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail Generals textures required");
    let retail_data = Path::new(&retail).join("Data");
    let mut maps = BigFileSystem::new();
    maps.open_archive_file(retail_data.join("MapsZH.big"))
        .unwrap();
    let mut bytes = Vec::new();
    maps.open_file("Maps/MD_USA05/MD_USA05.map", 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut map_ini = String::new();
    maps.open_file("Maps/MD_USA05/map.ini", 0)
        .unwrap()
        .read_to_string(&mut map_ini)
        .unwrap();
    let mut ini_archive = BigFileSystem::new();
    ini_archive
        .open_archive_file(retail_data.join("INIZH.big"))
        .unwrap();
    let mut water_ini = String::new();
    ini_archive
        .open_file("Data/INI/Water.ini", 0)
        .unwrap()
        .read_to_string(&mut water_ini)
        .unwrap();
    let source = tempfile::tempdir().unwrap();
    for (data, name) in [
        (retail_data, "TexturesZH.big"),
        (Path::new(&base).join("Data"), "Textures.big"),
    ] {
        std::fs::hard_link(data.join(name), source.path().join(name)).unwrap();
    }
    let world = new_world_services();
    with_world_services(&world, || {
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(source.path());
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }
        let stock = WaterDefinitions::default()
            .overlay_ini_text(&water_ini, INILoadType::Overwrite)
            .unwrap();
        let water = stock
            .overlay_ini_text(&map_ini, INILoadType::CreateOverrides)
            .unwrap()
            .presentation();
        assert!(
            water
                .standing_water_texture
                .eq_ignore_ascii_case("TSWater03.tga")
        );
        let mut default_art = water.clone();
        default_art.standing_water_texture = stock.presentation().standing_water_texture;
        assert!(
            default_art
                .standing_water_texture
                .eq_ignore_ascii_case("TWWater01.tga")
        );

        let areas = mounted_water_areas(&bytes);
        assert_eq!(areas.len(), 4);
        assert!(
            areas
                .iter()
                .all(|area| !area.is_river && area.points.len() == 4)
        );
        let (min, max) = water_test_bounds(&areas);
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        visual.admit_water_presentation(&water).unwrap();
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        visual.set_map_water_areas(areas.clone());
        let read = |visual: &mut TerrainVisualImpl| {
            set_water_test_camera(visual, &device, min, max);
            read_water_pixels(visual, &device, &queue, true)
        };
        let authored = read(&mut visual);
        assert!(authored.chunks_exact(4).any(|pixel| pixel[..3] != [0; 3]));
        let initially_shared = visual.river_gpu.river_tex == visual.water_texture;
        assert_eq!(
            Path::new(&visual.bound_standing_water_texture)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("tswater03"),
        );
        let authored_material = visual.river_gpu.trapezoid_bind_group.clone();
        let detail_views = |visual: &TerrainVisualImpl| {
            let detail = visual.river_gpu.detail_views.as_ref().unwrap();
            [detail.sparkle.clone(), detail.noise.clone(), detail.edge.clone()]
        };
        let original_details = detail_views(&visual);
        let refresh = |visual: &mut TerrainVisualImpl| {
            TerrainVisual::render(visual, &Mat4::IDENTITY, &Mat4::IDENTITY).unwrap();
        };
        refresh(&mut visual);
        let unchanged_kept_group = visual.river_gpu.trapezoid_bind_group == authored_material;
        assert_eq!(read(&mut visual), authored);

        visual.admit_water_presentation(&default_art).unwrap();
        refresh(&mut visual);
        let default_pixels = read(&mut visual);
        let changed_group = visual.river_gpu.trapezoid_bind_group != authored_material;
        let default_shared = visual.river_gpu.river_tex == visual.water_texture;

        let mut peer = TerrainVisualImpl::new();
        peer.admit_water_presentation(&default_art).unwrap();
        peer.init_gpu_resources(device.clone(), queue.clone()).unwrap();
        peer.set_map_water_areas(areas);
        visual.admit_water_presentation(&water).unwrap();
        refresh(&mut visual);
        assert_eq!(read(&mut visual), authored);
        assert_eq!(read(&mut peer), default_pixels);
        assert_eq!(detail_views(&visual), original_details);
        let restored_shared = visual.river_gpu.river_tex == visual.water_texture;
        eprintln!(
            "USA05 installed-water shared {initially_shared}/{default_shared}/{restored_shared}, same retained {unchanged_kept_group}, changed group {changed_group}, authored CRC32 {:08x}, default CRC32 {:08x}",
            crc32fast::hash(&authored),
            crc32fast::hash(&default_pixels),
        );
        assert_eq!(
            (
                initially_shared,
                unchanged_kept_group,
                changed_group,
                default_shared,
                restored_shared
            ),
            (true, true, true, true, true)
        );
        assert_ne!(authored, default_pixels);
    });
}

fn mounted_water_areas(bytes: &[u8]) -> Vec<TerrainWaterArea> {
    use gamelogic::system::map_loader::MapLoader;

    let mut loader = MapLoader::new();
    loader.load_runtime_support_from_bytes(bytes).unwrap();
    loader
        .to_map_data()
        .polygon_triggers
        .iter()
        .filter(|trigger| trigger.is_water_area() && trigger.get_num_points() > 2)
        .map(|trigger| TerrainWaterArea {
            points: (0..trigger.get_num_points())
                .map(|index| {
                    let p = trigger.get_point(index).unwrap();
                    // The native receiving terrain uses Y-up, like the Main delivery.
                    [p.x as f32, p.z as f32, p.y as f32]
                })
                .collect(),
            is_river: trigger.is_river(),
            river_start: trigger.get_river_start(),
        })
        .collect()
}

fn water_test_bounds(areas: &[TerrainWaterArea]) -> (Vec2, Vec2) {
    let (min, max) = areas.iter().flat_map(|area| &area.points).fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(min, max), p| {
            let v = Vec2::new(p[0], p[2]);
            (min.min(v), max.max(v))
        },
    );
    (min - Vec2::splat(20.0), max + Vec2::splat(20.0))
}

#[test]
#[ignore = "requires a serialized native GPU"]
fn water_shroud_preserves_the_trapezoid_background_before_alpha_blend() {
    use game_engine::common::common_services::{CommonServices, with_common_services};

    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || {
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        let mut water = visual.water_presentation().clone();
        for setting in &mut water.settings {
            setting.surface_color = (255.0, 255.0, 255.0, 128.0);
        }
        water.standing_water_color = (1.0, 0.7, 0.6);
        let mut rules = visual.water_rules();
        rules.feather_water = 0;
        visual.admit_water_rules(rules);
        visual.admit_water_presentation(&water).unwrap();
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        // Keep the source below attachment saturation so the two blend orders
        // can be compared independently of the machine's mounted artwork.
        let solid = |rgba| {
            TerrainVisualImpl::create_solid_rgba_texture(&device, &queue, rgba, "water shroud control")
        };
        visual.water_texture = Some(solid([96, 128, 160, 255]));
        let details = visual.river_gpu.detail_views.as_mut().unwrap();
        details.sparkle = solid([0, 0, 0, 255]).create_view(&Default::default());
        details.edge = solid([0, 0, 0, 255]).create_view(&Default::default());
        visual.ensure_water_materials(&device);
        set_water_test_camera(&mut visual, &device, Vec2::splat(-1.0), Vec2::splat(9.0));

        let read = |visual: &TerrainVisualImpl, background: wgpu::Color| {
            let pixels = read_water_pixels_with_background(visual, &device, &queue, true, background);
            let center = (8 * 32 + 16) * 4;
            <[u8; 3]>::try_from(&pixels[center..center + 3]).unwrap()
        };
        let background = wgpu::Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        };
        let sample = |visual: &mut TerrainVisualImpl, is_river| {
            visual.set_map_water_areas(vec![TerrainWaterArea {
                points: vec![
                    [0.0, 0.0, 0.0],
                    [8.0, 0.0, 0.0],
                    [8.0, 0.0, 8.0],
                    [0.0, 0.0, 8.0],
                ],
                is_river,
                river_start: 0,
            }]);
            let unshaded = [read(visual, wgpu::Color::BLACK), read(visual, background)];
            visual.set_shroud_overlay_r8(2, 2, 4.0, [0.0, 0.0], &[128; 4]);
            visual.sync_shroud_dest_texture();
            let shaded = [read(visual, wgpu::Color::BLACK), read(visual, background)];
            visual.clear_shroud_overlay();
            assert_eq!(read(visual, background), unshaded[1]);
            (unshaded, shaded)
        };
        let (river, shaded_river) = sample(&mut visual, true);
        let (pool, shaded_pool) = sample(&mut visual, false);
        let tint = [192, 128, 64];
        visual.set_projected_shroud_overlay_r8(2, 2, 4.0, [0.0, 0.0], tint, &[128; 4]);
        visual.sync_shroud_dest_texture();
        let tinted_pool = [
            read(&visual, wgpu::Color::BLACK),
            read(&visual, background),
        ];
        let background_delta = |pixels: [[u8; 3]; 2]| {
            std::array::from_fn::<_, 3, _>(|channel| {
                i16::from(pixels[1][channel]) - i16::from(pixels[0][channel])
            })
        };
        let river_delta = background_delta(river);
        let river_shaded_delta = background_delta(shaded_river);
        let pool_delta = background_delta(pool);
        let pool_shaded_delta = background_delta(shaded_pool);
        let pool_tinted_delta = background_delta(tinted_pool);
        eprintln!(
            "Water background contributions: separate river {river_delta:?} -> {river_shaded_delta:?}, trapezoid {pool_delta:?} -> {pool_shaded_delta:?}. Trapezoid water contribution {:?} -> {:?}",
            pool[0], shaded_pool[0],
        );
        for channel in 0..3 {
            let shroud = 128.0 / 255.0;
            let post_blend = (f32::from(river_delta[channel]) * shroud).round() as i16;
            assert!(river_delta[channel] > 16);
            assert!(river_shaded_delta[channel].abs_diff(post_blend) <= 1);
            let water_only = (f32::from(pool[0][channel]) * shroud).round() as u8;
            assert!(pool[0][channel] > 8);
            assert!(shaded_pool[0][channel].abs_diff(water_only) <= 1);
            assert!(pool_delta[channel] > 16);
            assert!(
                pool_shaded_delta[channel].abs_diff(pool_delta[channel]) <= 1,
                "the trapezoid shroud must leave the background contribution intact",
            );
            // The readback is BGRA, while the tint is RGB.
            let colored_water =
                (f32::from(pool[0][channel]) * shroud * f32::from(tint[2 - channel]) / 255.0)
                    .round() as u8;
            assert!(tinted_pool[0][channel].abs_diff(colored_water) <= 1);
            assert!(pool_tinted_delta[channel].abs_diff(pool_delta[channel]) <= 1);
        }
    });
}

#[test]
#[ignore = "requires a serialized native GPU"]
fn river_uses_the_authored_edge_coordinate_and_alpha() {
    use game_engine::common::common_services::{CommonServices, with_common_services};

    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || {
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        let mut water = visual.water_presentation().clone();
        water.standing_water_color = (1.0, 0.7, 0.6);
        for setting in &mut water.settings {
            setting.surface_color = (255.0, 255.0, 255.0, 128.0);
        }
        visual.admit_water_presentation(&water).unwrap();
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        let solid = |rgba| {
            TerrainVisualImpl::create_solid_rgba_texture(
                &device,
                &queue,
                rgba,
                "river edge control",
            )
        };
        visual.water_texture = Some(solid([32, 64, 96, 255]));
        let details = visual.river_gpu.detail_views.as_mut().unwrap();
        details.sparkle = solid([0, 0, 0, 255]).create_view(&Default::default());
        let edge = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("River edge UV and alpha control"),
            size: wgpu::Extent3d {
                width: 8,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut texels = [[0, 0, 0, 64_u8]; 8];
        texels[3..5].fill([200, 0, 0, 128]);
        queue.write_texture(
            edge.as_image_copy(),
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32),
                rows_per_image: Some(1),
            },
            edge.size(),
        );
        details.edge = edge.create_view(&Default::default());
        visual.ensure_water_materials(&device);
        set_water_test_camera(&mut visual, &device, Vec2::splat(-1.0), Vec2::splat(9.0));
        visual.set_map_water_areas(vec![TerrainWaterArea {
            points: vec![
                [0.0, 0.0, 0.0],
                [8.0, 0.0, 0.0],
                [8.0, 0.0, 8.0],
                [0.0, 0.0, 8.0],
            ],
            is_river: true,
            river_start: 0,
        }]);
        // The covered center pixel has u1≈0.26 and u2≈0.52. Linear sampling
        // selects only columns 1–2 for u1, and columns 3–4 for the edge's u2.
        // At u2, the original multiplies the mesh alpha by 128, not red 200.
        let background = wgpu::Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        };
        let dark = read_water_pixels(&visual, &device, &queue, true);
        let lit = read_water_pixels_with_background(&visual, &device, &queue, true, background);
        let center = (8 * 32 + 16) * 4;
        let delta: [i16; 3] = std::array::from_fn(|channel| {
            i16::from(lit[center + channel]) - i16::from(dark[center + channel])
        });
        eprintln!(
            "River edge artwork leaves a background contribution of {delta:?}, original expected ~96"
        );
        assert!(delta.iter().all(|value| value.abs_diff(96) <= 1));
    });
}

fn water_diffuse_test_lighting(count: usize) -> crate::terrain::chunk::TerrainStaticLighting {
    let lights = [
        (Vec3::NEG_Y, [0.125, 0.25, 0.0]),
        (Vec3::NEG_Y * 0.5, [0.25, 0.0, 0.5]),
        (Vec3::Y * 0.5, [1.0; 3]),
    ];
    crate::terrain::chunk::TerrainStaticLighting {
        lights: lights[..count].to_vec(),
        ambient: [0.125; 3],
        scorch_diffuse: [0.0; 3],
    }
}

fn admit_water_diffuse_test_presentation(visual: &mut TerrainVisualImpl) {
    let mut rules = visual.water_rules();
    rules.feather_water = 0;
    visual.admit_water_rules(rules);
    let mut water = visual.water_presentation().clone();
    water.standing_water_color = (1.0, 1.0, 1.0);
    for setting in &mut water.settings {
        setting.surface_color = (255.0, 255.0, 255.0, 128.0);
    }
    visual.admit_water_presentation(&water).unwrap();
}

#[test]
fn water_diffuse_uses_every_admitted_terrain_light_and_cpp_truncation() {
    let mut visual = TerrainVisualImpl::new();
    admit_water_diffuse_test_presentation(&mut visual);
    visual.set_lighting(
        Some(Vec3::NEG_Y.to_array()),
        Some([0.125, 0.25, 0.0]),
        Some([0.125; 3]),
        None,
        None,
    );
    let standalone = visual.standing_water_diffuse_packed();
    let default_lighting = visual.chunk_manager.static_lighting().clone();
    assert!(!visual.admit_static_lighting(default_lighting));
    let admitted_default = visual.standing_water_diffuse_packed();
    let colors: [_; 4] = std::array::from_fn(|count| {
        visual.admit_static_lighting(water_diffuse_test_lighting(count));
        visual.standing_water_diffuse_packed()
    });
    visual.set_lighting(
        Some(Vec3::Y.to_array()),
        Some([0.0; 3]),
        Some([0.0; 3]),
        None,
        None,
    );
    let complete_after_standalone_setter = visual.standing_water_diffuse_packed();
    let mut water = visual.water_presentation().clone();
    water.standing_water_color = (0.6, 0.7, 1.0);
    visual.admit_water_presentation(&water).unwrap();
    let override_color = visual.standing_water_diffuse_packed();
    eprintln!(
        "Water diffuse for zero through three admitted lights {colors:08x?}, authored override {override_color:08x}"
    );
    // Original W3DWater.cpp sum/pack with BaseType.h REAL_TO_INT.
    assert_eq!(colors, [0x801f_1f1f, 0x803f_5f1f, 0x805f_5f5f, 0x805f_5f5f]);
    assert_eq!(standalone, 0x803f_5f1f);
    assert_ne!(admitted_default, standalone);
    assert_eq!(complete_after_standalone_setter, colors[3]);
    assert_eq!(override_color, 0x8099_b2ff);
}

#[test]
#[ignore = "requires a serialized native GPU"]
fn polygon_water_uses_changed_terrain_lights_in_the_same_phase() {
    use gamelogic::system::engine_stores::{new_world_services, with_world_services};

    let world = new_world_services();
    with_world_services(&world, || {
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        admit_water_diffuse_test_presentation(&mut visual);
        visual.admit_static_lighting(water_diffuse_test_lighting(1));
        visual
            .init_gpu_resources(device.clone(), queue.clone())
            .unwrap();
        let solid = |rgba| {
            TerrainVisualImpl::create_solid_rgba_texture(
                &device,
                &queue,
                rgba,
                "water diffuse control",
            )
        };
        visual.water_texture = Some(solid([255; 4]));
        visual.river_gpu.detail_views.as_mut().unwrap().sparkle =
            solid([0, 0, 0, 255]).create_view(&Default::default());
        visual.ensure_water_materials(&device);
        visual.set_map_water_areas(vec![TerrainWaterArea {
            points: vec![
                [0.0, 0.0, 0.0],
                [8.0, 0.0, 0.0],
                [8.0, 0.0, 8.0],
                [0.0, 0.0, 8.0],
            ],
            is_river: false,
            river_start: 0,
        }]);
        let capture = |visual: &mut TerrainVisualImpl| {
            TerrainVisual::render(visual, &Mat4::IDENTITY, &Mat4::IDENTITY).unwrap();
            set_water_test_camera(visual, &device, Vec2::splat(-1.0), Vec2::splat(9.0));
            let pixels = read_water_pixels(visual, &device, &queue, true);
            let center = (8 * 32 + 16) * 4;
            <[u8; 3]>::try_from(&pixels[center..center + 3]).unwrap()
        };
        let first = capture(&mut visual);
        let phase = visual.overlay.river_v_origin.to_bits();
        assert!(visual.admit_static_lighting(water_diffuse_test_lighting(2)));
        let second = capture(&mut visual);
        assert_eq!(visual.overlay.river_v_origin.to_bits(), phase);
        assert!(!visual.admit_static_lighting(water_diffuse_test_lighting(2)));
        assert_eq!(capture(&mut visual), second);
        eprintln!(
            "Water at the same phase with one versus two admitted lights: {first:?} -> {second:?}"
        );
        for (actual, expected) in first
            .into_iter()
            .zip([16, 48, 32])
            .chain(second.into_iter().zip([48; 3]))
        {
            assert!(actual.abs_diff(expected) <= 1);
        }
    });
}

#[test]
#[ignore = "requires a serialized native GPU"]
fn adjacent_water_tracks_preserve_ordered_color_and_shroud_pixels() {
    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_water_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));

    set_water_test_camera(&mut visual, &device, Vec2::ZERO, Vec2::new(4.0, 2.0));
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    for (name, rgba) in [
        ("wave256.tga", [255, 255, 255, 255]),
        ("blue-wave.tga", [0, 0, 255, 255]),
    ] {
        let texture = TerrainVisualImpl::create_solid_rgba_texture(&device, &queue, rgba, name);
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: visual.water_texture_bind_group_layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        if name == "wave256.tga" {
            visual.water_texture_bind_group = Some(bind_group.clone());
        }
        visual.water_named_bind_groups.insert(
            name.to_owned(),
            NamedWaterBind {
                _texture: texture,
                bind_group,
            },
        );
    }
    visual.set_shroud_overlay_r8(
        4,
        2,
        1.0,
        [0.0, 0.0],
        &[255, 217, 191, 159, 255, 217, 191, 159],
    );
    visual.sync_shroud_dest_texture();

    let mut flush = WaterTracksFlush::default();
    let strip = water_track_strip_indices(2, 2);
    for (handle, (name, diffuse, left, right)) in [
        ("wave256.tga", 0x80ff_0000, 0.2, 2.7),
        ("", 0x8000_ff00, 0.8, 3.2),
        ("blue-wave.tga", 0x80ff_ffff, 1.0, 3.7),
        ("wave256.tga", 0x80ff_ff00, 1.5, 3.8),
    ]
    .into_iter()
    .enumerate()
    {
        let first_vertex = flush.vertices.len();
        let first_index = flush.indices.len();
        flush.ranges.push(WaterTrackDrawRange {
            handle,
            first_vertex,
            vertex_count: 4,
            first_index,
            index_count: strip.len(),
            texture_name: name.to_owned(),
            wave_type: WaterTrackType::Pond,
        });
        flush.vertices.extend(
            [(left, 0.25), (right, 0.25), (left, 1.75), (right, 1.75)].map(|(x, y)| {
                WaterTrackVertex {
                    x,
                    y,
                    z: 1.5,
                    diffuse,
                    u1: 0.5,
                    v1: 0.5,
                }
            }),
        );
        flush
            .indices
            .extend(strip.iter().map(|&index| index + first_vertex as u16));
    }

    let mut separate_meshes = Vec::new();
    for range in &flush.ranges {
        let single = WaterTracksFlush {
            vertices: flush.vertices.clone(),
            indices: flush.indices.clone(),
            ranges: vec![range.clone()],
        };
        visual.upload_water_track_meshes(&single);
        assert_eq!(visual.water_track_meshes.len(), 1);
        separate_meshes.append(&mut visual.water_track_meshes);
    }
    visual.water_track_meshes = separate_meshes;
    let separate_color = read_water_pixels(&visual, &device, &queue, false);
    let separate_shroud = read_water_pixels(&visual, &device, &queue, true);
    assert_ne!(
        separate_color, separate_shroud,
        "the shroud must affect the readback"
    );
    visual.water_track_meshes.swap(0, 1);
    assert_ne!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color,
        "the overlapping alpha colors must detect a change in primitive order",
    );

    visual.upload_water_track_meshes(&flush);
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, true),
        separate_shroud
    );
    let meshes: Vec<_> = visual
        .water_track_meshes
        .iter()
        .map(|mesh| (mesh.texture_name.as_str(), mesh.index_count))
        .collect();
    assert_eq!(
        meshes,
        [
            ("wave256.tga", 12),
            ("blue-wave.tga", 6),
            ("wave256.tga", 6)
        ]
    );

    let first_index_buffer = &visual.water_track_meshes[0].index_buffer;
    assert!(
        visual.water_track_meshes[1..]
            .iter()
            .all(|mesh| &mesh.index_buffer == first_index_buffer)
    );

    let original = std::mem::take(&mut visual.water_track_meshes);
    let mut changed = flush.clone();
    changed.indices[..4].copy_from_slice(&[0, 3, 1, 2]);
    visual.upload_water_track_meshes(&changed);
    assert_eq!(
        visual.water_track_meshes[0].index_count,
        original[0].index_count
    );
    assert_ne!(
        visual.water_track_meshes[0].index_buffer,
        original[0].index_buffer
    );
    assert_ne!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );

    // A previously recorded frame can retain its old immutable topology.
    visual.water_track_meshes = original;
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, true),
        separate_shroud
    );
    check_projected_shroud_uploads(&mut visual, &device, &queue);
}

fn check_projected_shroud_uploads(
    visual: &mut TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let (width, height) = (67, 41);
    let origin = [-17.0, -13.0];
    let mut cells: Vec<_> = (0..width * height)
        .map(|index| (20 + (index * 17 + index / width * 9) % 235) as u8)
        .collect();
    visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells);
    let mask_bytes = visual.sync_shroud_dest_texture();
    let r8_pixels = read_water_pixels(visual, device, queue, true);
    let unshrouded = read_water_pixels(visual, device, queue, false);
    assert_ne!(r8_pixels, unshrouded);
    let texture = visual.shroud_gpu.dest_texture.as_ref().unwrap();
    assert_eq!((texture.width(), texture.height()), (67, 41));
    assert_eq!(texture.format(), wgpu::TextureFormat::R8Unorm);
    assert_eq!(mask_bytes, cells.len());

    // Compare all pixels with the old replicated RGBA format through the same
    // sampler, projection parameters, mesh, and native terrain shroud shader.
    let rgba = legacy_shroud_binding(visual, device, queue, &cells, 67, 41);
    let r8 = visual.shroud_gpu.bind_group.replace(rgba).unwrap();
    assert_eq!(read_water_pixels(visual, device, queue, true), r8_pixels);
    visual.shroud_gpu.bind_group = Some(r8);
    for _ in 0..90 {
        visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells);
        assert_eq!(visual.sync_shroud_dest_texture(), 0);
    }
    assert_eq!(read_water_pixels(visual, device, queue, true), r8_pixels);
    visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells[..cells.len() - 1]);
    assert_eq!(visual.sync_shroud_dest_texture(), 0);
    eprintln!(
        "shroud native frame CRC32 {:08x}, projected mask {mask_bytes} bytes, 90 identical updates 0 bytes",
        crc32fast::hash(&r8_pixels),
    );

    visual.set_shroud_level(18, 13, 255);
    cells[13 * width as usize + 18] = 255;
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    let after_cell = read_water_pixels(visual, device, queue, true);
    assert_ne!(after_cell, r8_pixels);
    visual.set_shroud_level(18, 13, 255);
    assert_eq!(visual.sync_shroud_dest_texture(), 0);

    visual.set_shroud_overlay_r8(width, height, 1.0, [-18.0, -13.0], &cells);
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    let after_origin = read_water_pixels(visual, device, queue, true);
    assert_ne!(after_origin, after_cell);
    visual.create_shroud_overlay_pipelines(device).unwrap();
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    assert_eq!(read_water_pixels(visual, device, queue, true), after_origin);

    visual.clear_shroud_overlay();
    assert_eq!(visual.sync_shroud_dest_texture(), 1);
    assert_eq!(read_water_pixels(visual, device, queue, true), unshrouded);
    visual.clear_shroud_overlay();
    assert_eq!(visual.sync_shroud_dest_texture(), 0);
}

fn legacy_shroud_binding(
    visual: &TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    cells: &[u8],
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("prior RGBA terrain shroud test control"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let pixels: Vec<_> = cells
        .iter()
        .flat_map(|&cell| [cell, cell, cell, 255])
        .collect();
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("prior RGBA terrain shroud test control"),
        layout: visual.shroud_gpu.bind_layout.as_ref().unwrap(),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(
                    visual.shroud_gpu.sampler.as_ref().unwrap(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: visual
                    .shroud_gpu
                    .params
                    .as_ref()
                    .unwrap()
                    .as_entire_binding(),
            },
        ],
    })
}

#[test]
#[ignore = "requires native GPU and the retail Zero Hour MapsZH.big"]
fn tournament_b_water_tracks_upload_one_mesh() {
    use game_engine::common::system::big_file_system::BigFileSystem;
    use std::io::Read;

    let assets_dir =
        std::env::var_os("GENERALS_ASSETS_DIR").expect("retail Zero Hour assets required");
    let mut files = BigFileSystem::new();
    files
        .open_archive_file(Path::new(&assets_dir).join("Data").join("MapsZH.big"))
        .unwrap();
    let mut bytes = Vec::new();
    files
        .open_file("Maps/TournamentB/TournamentB.wak", 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let records = crate::terrain::water_tracks::decode_wak_records(&bytes).unwrap();
    let mut visual = TerrainVisualImpl::new();
    assert_eq!(visual.water_tracks_mut().load_records(&records).len(), 921);
    let (device, queue) = native_water_test_device();
    visual.device = Some(device);
    visual.queue = Some(queue);
    visual.flush_water_tracks();

    assert_eq!(visual.last_water_tracks_flush().ranges.len(), 921);
    assert_eq!(visual.water_track_meshes.len(), 1);
    assert_eq!(visual.water_track_meshes[0].index_count, 921 * 6);
    assert_eq!(visual.water_track_meshes[0].texture_name, "wave256.tga");

    let indices = visual.water_track_meshes[0].index_buffer.clone();
    let vertices = visual.water_track_meshes[0].vertex_buffer.clone();
    let initial_vertices = visual.last_water_tracks_flush().vertices.clone();
    for _ in 0..30 {
        visual.flush_water_tracks();
        assert_eq!(visual.water_track_meshes.len(), 1);
        assert_eq!(visual.water_track_meshes[0].index_buffer, indices);
        assert_ne!(visual.water_track_meshes[0].vertex_buffer, vertices);
    }
    assert_ne!(visual.last_water_tracks_flush().vertices, initial_vertices);
}

#[test]
#[ignore = "requires native GPU and the retail Generals Textures.big"]
fn stock_wake_texture_keeps_the_original_seven_dds_levels() {
    use game_engine::common::common_services::{CommonServices, with_common_services};
    use game_engine::common::system::big_file_system::BigArchiveBackend;
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::subsystem_interface::SubsystemInterface;

    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail base Generals assets required");
    let archive = Path::new(&base).join("Data").join("Textures.big");
    let source = tempfile::tempdir().unwrap();
    std::fs::hard_link(archive, source.path().join("Textures.big")).unwrap();
    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || {
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(source.path());
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        visual.create_terrain_pipeline(&device).unwrap();
        visual.create_water_pipeline(&device).unwrap();
        visual.device = Some(Arc::clone(&device));
        visual.queue = Some(queue);
        visual.ensure_named_water_bind_group(&device, "wave256.tga");
        let texture = &visual.water_named_bind_groups["wave256.tga"]._texture;
        assert_eq!((texture.width(), texture.height()), (256, 128));
        assert_eq!(texture.mip_level_count(), 7);
    });
}

#[test]
#[ignore = "requires native GPU and the retail Generals Textures.big"]
fn mounted_wake_dds_is_bound_before_first_native_water_draw() {
    use game_engine::common::system::big_file_system::BigFileSystem;
    use std::io::Read;

    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail base Generals assets required");
    let archive = Path::new(&base).join("Data").join("Textures.big");
    let mut files = BigFileSystem::new();
    files.open_archive_file(archive).unwrap();
    let mut dds = Vec::new();
    files
        .open_file("Art/Textures/wave256.dds", 0)
        .unwrap()
        .read_to_end(&mut dds)
        .unwrap();

    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_water_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));
    let wake = visual.water_tracks_mut().add_wave_from_info(
        WaterTrackType::Pond,
        Vec2::new(100.0, 100.0),
        Vec2::new(100.0, 128.0),
        false,
    )[0];
    visual
        .water_tracks_mut()
        .track_mut(wake)
        .unwrap()
        .update(1000);

    let mut requests = Vec::new();
    visual.hydrate_mounted_water_track_textures(|filename| {
        requests.push(filename.to_owned());
        (filename == "wave256.dds").then(|| dds.clone())
    });
    assert_eq!(requests, ["wave256.dds"]);
    assert!(visual.last_water_tracks_flush().ranges.is_empty());
    let authored = &visual.water_named_bind_groups["wave256.tga"];
    assert_eq!(
        (authored._texture.width(), authored._texture.height()),
        (256, 128)
    );
    assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 9);
    // DDSFileClass drops the two lowest levels after applying reduction.
    assert_eq!(authored._texture.mip_level_count(), 7);
    visual.water_texture_bind_group = Some(authored.bind_group.clone());
    visual.hydrate_mounted_water_track_textures(|_| panic!("resident wake art reopened"));
    visual.flush_water_tracks();
    assert_eq!(visual.water_track_meshes.len(), 1);

    let bounds = visual.last_water_tracks_flush().vertices.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(min, max), vertex| {
            let point = Vec2::new(vertex.x, vertex.y);
            (min.min(point), max.max(point))
        },
    );
    set_water_test_camera(&mut visual, &device, bounds.0, bounds.1);
    let pixels_with_mips = read_water_pixels(&visual, &device, &queue, false);
    assert!(
        pixels_with_mips
            .chunks_exact(4)
            .any(|p| p[0..3] != [0, 0, 0])
    );

    // Use the same BC decoder and uploader for level zero in both captures.
    // The sampler's available mip levels are the only difference.
    let dds = ww3d_assets::image::load_dds_from_memory(&dds).unwrap();
    let single_level = ww3d_renderer_3d::texture_system::upload_dds_levels(
        &device,
        &queue,
        "single-level wake control",
        dds.width,
        dds.height,
        1,
        dds.compression.unwrap(),
        |level| dds.get_level_data(level),
    )
    .unwrap();
    let layout = Arc::clone(visual.water_texture_bind_group_layout.as_ref().unwrap());
    visual.bind_water_track_texture(&device, &layout, "wave256.tga", single_level);
    let pixels_without_mips = read_water_pixels(&visual, &device, &queue, false);
    assert_ne!(pixels_with_mips, pixels_without_mips);

    // The actual mounted wake's material still samples the tinted shroud once
    // its R8 texture is available to both native and Draw consumers.
    let cell_size = (bounds.1 - bounds.0).max_element() + 1.0;
    let origin = (bounds.0 - Vec2::splat(cell_size)).to_array();
    let tint = [128, 255, 0];
    let cells: Arc<[u8]> = Arc::from([160; 4]);
    visual.set_projected_shroud_overlay_r8(2, 2, cell_size, origin, tint, &cells);
    assert_eq!(visual.sync_shroud_dest_texture(), cells.len());
    let borrowed_frame = read_water_pixels(&visual, &device, &queue, true);
    visual.set_shared_projected_shroud_overlay_r8(2, 2, cell_size, origin, tint, &cells);
    assert!(
        std::ptr::eq(visual.overlay.shroud_cells.as_ref(), cells.as_ref()),
        "the native receiver must retain the presentation's frozen shroud allocation"
    );
    assert_eq!(visual.sync_shroud_dest_texture(), 0);
    let (view, sampler) = visual
        .matching_shroud_sampling_resources(&device, (2, 2), &cells)
        .unwrap();
    assert!(Arc::ptr_eq(
        view,
        visual.shroud_gpu.dest_view.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        sampler,
        visual.shroud_gpu.sampler.as_ref().unwrap()
    ));
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &[159, 160, 160, 160])
            .is_none()
    );
    let (foreign, _) = native_water_test_device();
    assert!(
        visual
            .matching_shroud_sampling_resources(&foreign, (2, 2), &cells)
            .is_none()
    );

    let tinted = read_water_pixels(&visual, &device, &queue, true);
    assert_eq!(tinted, borrowed_frame);
    eprintln!(
        "mounted shared and borrowed R8 frame CRC32 {:08x}",
        crc32fast::hash(&tinted)
    );
    assert_ne!(tinted, pixels_without_mips);
    for (base, shaded) in pixels_without_mips
        .chunks_exact(4)
        .zip(tinted.chunks_exact(4))
    {
        for (channel, tint) in [0.0, 1.0, 128.0 / 255.0].into_iter().enumerate() {
            let expected = (f32::from(base[channel]) * (160.0 / 255.0) * tint).round() as u8;
            assert!(
                shaded[channel].abs_diff(expected) <= 1,
                "mounted colored wake pixel {base:?} -> {shaded:?}"
            );
        }
    }

    // A standalone mutable caller separates storage only when a cell changes.
    visual.set_shroud_level(0, 0, 160);
    assert!(Arc::ptr_eq(&visual.overlay.shroud_cells, &cells));
    visual.set_shroud_level(0, 0, 159);
    assert_eq!(&*cells, &[160; 4]);
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &cells)
            .is_none()
    );
    assert_eq!(visual.sync_shroud_dest_texture(), cells.len());
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &cells)
            .is_none()
    );
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &[159, 160, 160, 160])
            .is_some()
    );
    visual.set_shared_projected_shroud_overlay_r8(2, 2, cell_size, origin, tint, &cells);
    assert!(Arc::ptr_eq(&visual.overlay.shroud_cells, &cells));
    assert_eq!(visual.sync_shroud_dest_texture(), cells.len());
    assert_eq!(read_water_pixels(&visual, &device, &queue, true), tinted);
}

fn set_water_test_camera(
    visual: &mut TerrainVisualImpl,
    device: &wgpu::Device,
    min: Vec2,
    max: Vec2,
) {
    let scale = Vec2::splat(2.0) / (max - min);
    let projection = Mat4::from_cols_array_2d(&[
        [scale.x, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [0.0, scale.y, 0.0, 0.0],
        [-1.0 - min.x * scale.x, -1.0 - min.y * scale.y, 0.5, 1.0],
    ]);
    let uniforms = TerrainUniforms {
        view_proj: projection.to_cols_array_2d(),
        view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        projection_matrix: projection.to_cols_array_2d(),
        camera_position: [0.0; 4],
        time: 0.0,
        _time_padding: [0.0; 3],
        fog_color: [0.0; 4],
        fog_range: [0.0; 4],
    };
    let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("water track test camera"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    visual.terrain_camera_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera.as_entire_binding(),
        }],
    }));
}

fn native_water_test_device() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    native_water_test_device_with_limits(wgpu::Limits::default())
}

fn native_water_test_device_with_limits(
    required_limits: wgpu::Limits,
) -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits,
        ..Default::default()
    }))
    .unwrap();
    (Arc::new(device), Arc::new(queue))
}

fn read_water_pixels(
    visual: &TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shroud: bool,
) -> Vec<u8> {
    read_water_pixels_with_background(visual, device, queue, shroud, wgpu::Color::BLACK)
}

fn read_water_pixels_with_background(
    visual: &TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shroud: bool,
    background: wgpu::Color,
) -> Vec<u8> {
    let size = wgpu::Extent3d {
        width: 32,
        height: 16,
        depth_or_array_layers: 1,
    };
    let attachment = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = attachment(
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = attachment(
        TERRAIN_PIPELINES_DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ordered water track readback"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        if shroud {
            visual.record_water_scene_draws(&mut pass);
        } else {
            visual.record_water_draws(&mut pass);
        }
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    let submission = queue.submit([encoder.finish()]);
    let (done, received) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            done.send(result).unwrap()
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(20)),
        })
        .unwrap();
    received
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap();
    let pixels = readback
        .slice(..)
        .get_mapped_range()
        .unwrap()
        .chunks_exact(256)
        .flat_map(|row| row[..size.width as usize * 4].iter().copied())
        .collect();
    readback.unmap();
    pixels
}
