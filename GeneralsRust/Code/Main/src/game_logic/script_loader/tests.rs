// Characterization and validation coverage for the map script loader fragments.

#[cfg(test)]
mod tests {
    use super::*;
    fn push_f32s(buf: &mut Vec<u8>, values: [f32; 9]) {
        for v in values {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }

    #[test]
    fn global_lighting_keeps_objects_row_for_units_and_shadows() {
        // C++ WorldHeightMap.cpp:772-804 — terrain[0] then objects[0] per TOD.
        use game_engine::common::ini::ini_game_data::ensure_global_data;
        let handle = ensure_global_data();
        let previous = handle.read().clone();
        let mut payload = Vec::new();
        payload.extend_from_slice(&4i32.to_le_bytes()); // Night
        for tod in 0..4 {
            let t = tod as f32;
            push_f32s(
                &mut payload,
                [0.1 + t, 0.2, 0.3, 0.4, 0.5, 0.6, 1.0, 2.0, 3.0],
            );
            push_f32s(
                &mut payload,
                [0.9, 0.8, 0.7, 0.15, 0.25, 0.35, 9.0, 8.0, 7.0],
            );
        }
        let mut meta = MapMetadata::default();
        parse_lighting_payload_for_settings(1, &payload, &mut meta).expect("parse lighting");
        // Night = index 3 of the four TOD rows.
        assert_eq!(meta.ambient_color, Some([3.1, 0.2, 0.3]));
        assert_eq!(meta.sun_color, Some([0.4, 0.5, 0.6]));
        assert_eq!(meta.sun_direction, Some([1.0, 2.0, 3.0]));
        assert_eq!(meta.objects_ambient_color, Some([0.9, 0.8, 0.7]));
        assert_eq!(meta.objects_sun_color, Some([0.15, 0.25, 0.35]));
        assert_eq!(meta.objects_sun_direction, Some([9.0, 8.0, 7.0]));
        let objects = handle.read().terrain_objects_lighting[4][0].clone();
        assert!((objects.ambient.r - 0.9).abs() < 1e-5);
        assert!((objects.light_pos.x - 9.0).abs() < 1e-5);
        *handle.write() = previous;
    }

    #[test]
    fn set_weather_visible_reaches_snow_manager() {
        // C++ ScriptActions.cpp:3804 TheSnowManager->setVisible
        #[cfg(feature = "game_client")]
        {
            let snow = game_client::snow::initialize_snow_manager();
            snow.lock().expect("snow lock").set_visible(true);
            let mut logic = crate::game_logic::GameLogic::new();
            logic.set_weather_visible(false);
            assert!(!logic.weather_state().visible);
            assert!(
                !snow.lock().expect("snow lock").is_visible(),
                "script Weather must hide SnowManager flakes"
            );
            logic.set_weather_visible(true);
            assert!(snow.lock().expect("snow lock").is_visible());
        }
    }

    #[test]
    fn retail_shell_map_terrain_chunks_decode_when_present() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files_v2/MapsZH/Maps/ShellMapMD/ShellMapMD.map",
        );
        let Ok(raw) = std::fs::read(&path) else {
            return;
        };
        let bytes = decompress_map_bytes(&raw).expect("retail shell map decompression");
        let (toc, body_offset) = parse_chunk_toc(&bytes).expect("retail shell map TOC");
        let chunky = ChunkyMap {
            source: path,
            toc,
            body_offset,
            bytes,
        };
        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .expect("retail shell map heightmap parse")
            .expect("retail shell map embeds HeightMapData");
        assert_eq!((heightmap.width, heightmap.height), (315, 315));
        assert_eq!(heightmap.border_size, 70);
        assert_eq!(heightmap.data.len(), 315 * 315);
    }

    #[test]
    fn map_ini_extracts_only_particle_system_blocks() {
        // C++ GameLogic.cpp:2404-2408 loadMapINI dispatches ParticleSystem
        // blocks; mixed Object/Weather content must not be treated as particles.
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
ParticleSystem MapSmoke\n\
  Priority = NONE\n\
End\n\
\n\
Weather\n\
  Snow = Yes\n\
End\n";
        let extracted = extract_map_ini_particle_system_blocks(mixed);
        assert!(extracted.contains("ParticleSystem MapSmoke"));
        assert!(extracted.contains("Priority = NONE"));
        assert!(!extracted.contains("Object SomeUnit"));
        assert!(!extracted.contains("Weather"));
        assert_eq!(overlay_map_ini_particle_systems(mixed), 1);
    }

    #[test]
    fn map_ini_weather_overlay_sets_snow_enabled() {
        // C++ GameLogic.cpp:2407-2408 loadMapINI CREATE_OVERRIDES Weather.
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
ParticleSystem MapSmoke\n\
  Priority = NONE\n\
End\n\
\n\
Weather\n\
  SnowEnabled = Yes\n\
End\n";
        let extracted = extract_map_ini_weather_blocks(mixed);
        assert!(extracted.contains("Weather"));
        assert!(extracted.contains("SnowEnabled = Yes"));
        assert!(!extracted.contains("ParticleSystem"));
        assert!(!extracted.contains("Object SomeUnit"));
        assert!(overlay_map_ini_weather(mixed));
        assert!(
            game_engine::common::ini::ini_weather::is_snow_enabled(),
            "map.ini Weather SnowEnabled must reach TheWeatherSetting"
        );
    }

    #[test]
    fn map_ini_create_overrides_applies_command_set() {
        // C++ loadMapINI full table: CommandSet CREATE_OVERRIDES must apply
        // even when mixed Object/Weather content is present.
        use game_engine::common::ini::ini_command_set::{
            get_command_set_manager, initialize_command_set_manager,
        };
        initialize_command_set_manager();
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
CommandSet MapIniDozerCommandSet\n\
  1 = Command_ConstructAmericaPowerPlant\n\
  2 = Command_ConstructAmericaBarracks\n\
End\n\
\n\
Upgrade MapIniRangerCapture\n\
  DisplayName = MapOverrideCapture\n\
End\n\
\n\
Weather\n\
  SnowEnabled = Yes\n\
End\n";
        let applied = overlay_map_ini_create_overrides(mixed);
        assert!(
            applied >= 1,
            "map.ini CommandSet/Upgrade CREATE_OVERRIDES must apply"
        );
        let manager = get_command_set_manager().expect("CommandSet manager");
        let set = manager
            .find_command_set_resolved("MapIniDozerCommandSet")
            .expect("map.ini CommandSet override must reach TheControlBar table");
        assert_eq!(
            set.get_button_at_position(0).map(String::as_str),
            Some("Command_ConstructAmericaPowerPlant")
        );
        assert_eq!(
            set.get_button_at_position(1).map(String::as_str),
            Some("Command_ConstructAmericaBarracks")
        );
    }

    #[test]
    fn world_weather_and_object_weather_values_match_cpp() {
        assert_eq!(parse_world_weather_value("1"), Some(1));
        assert_eq!(parse_world_weather_value("SNOWY"), Some(1));
        assert_eq!(parse_world_weather_value("0"), Some(0));
        assert_eq!(parse_object_weather_value("2"), Some(2));
        assert_eq!(parse_object_weather_value("1"), Some(1));
        assert!(resolve_object_weather_snow(0, true));
        assert!(!resolve_object_weather_snow(1, true));
        assert!(resolve_object_weather_snow(2, false));
    }

    #[test]
    fn world_info_weather_int_parses_snowy() {
        // C++ WorldHeightMap.cpp:743-746 TheKey_weather → WEATHER_SNOWY=1.
        let mut toc = HashMap::new();
        toc.insert(1, "WorldInfo".to_string());
        toc.insert(6, "weather".to_string());
        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticSnow.map"),
            toc,
            body_offset: 0,
            bytes: chunk(1, 1, &dict_int(6, 1)),
        };
        assert_eq!(
            parse_runtime_weather_from_chunky(&chunky).expect("weather parse"),
            Some(1)
        );
        assert_eq!(
            parse_runtime_water_height_from_chunky(&chunky).expect("water parse"),
            None
        );
    }

    #[test]
    fn lone_eagle_fast_chunky_parses_in_under_five_seconds() {
        // Smoke hangs inside sync_legacy_runtime_from_fast_chunky after
        // "Fast legacy runtime sync started". Isolate CPU parse from
        // THE_TERRAIN_LOGIC / SidesList / PlayerList / TeamFactory / FileSystem
        // locks. Contended-lock fail-open is covered separately.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        let Ok(raw) = std::fs::read(&path) else {
            return;
        };
        let started = std::time::Instant::now();
        let bytes = decompress_map_bytes(&raw).expect("Lone Eagle decompress");
        let (toc, body_offset) = parse_chunk_toc(&bytes).expect("Lone Eagle TOC");
        let chunky = ChunkyMap {
            source: path,
            toc,
            body_offset,
            bytes,
        };
        let heightmap = parse_heightmap_data_from_chunky(&chunky).expect("heightmap");
        let _ = parse_runtime_waypoints_from_chunky(&chunky).expect("waypoints");
        let _ = parse_runtime_bridges_from_chunky(&chunky).expect("bridges");
        let _ = parse_runtime_water_height_from_chunky(&chunky).expect("water");
        let _ = parse_runtime_polygon_triggers_from_chunky(&chunky).expect("polygons");
        let _ = parse_runtime_roads_from_chunky(&chunky).expect("roads");
        let _ = parse_runtime_sides_from_chunky(&chunky).expect("sides");
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs_f32() < 5.0,
            "Lone Eagle CPU parse took {:?}; hang is not parse",
            elapsed
        );
        assert!(heightmap.is_some(), "Lone Eagle must embed HeightMapData");
    }

    #[test]
    fn lone_eagle_live_path_reuses_one_chunky_decode() {
        // C++ TerrainLogic::loadMap (TerrainLogic.cpp:1248-1262) opens the
        // .map once via CachedFileInputStream. The live Rust load_map used
        // to inspect + load_chunky_map + parse_player_start_waypoints +
        // load_map_scripts, each RefPack-decoding Lone Eagle again.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        if !path.is_file() {
            return;
        }
        reset_map_decompress_count();
        let map_name = path.to_string_lossy();
        let chunky = load_chunky_map(map_name.as_ref())
            .expect("load Lone Eagle")
            .expect("Lone Eagle present");
        let _ = inspect_map_chunks_from_chunky(&chunky);
        let meta = parse_map_settings_from_chunky(&chunky).expect("settings from chunky");
        let _ = parse_player_start_waypoints(map_name.as_ref()).expect("starts via cache");
        let _ = load_map_scripts(map_name.as_ref()).expect("scripts via cache");
        assert_eq!(
            map_decompress_count(),
            1,
            "live load_map helpers must reuse the first ChunkyMap decode"
        );
        assert!(
            !meta.objects.is_empty() || !meta.start_waypoints.is_empty(),
            "Lone Eagle settings must carry placements or starts"
        );
    }

    fn chunk(id: u32, version: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn ascii(value: &str, out: &mut Vec<u8>) {
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    #[test]
    fn blend_tile_data_parser_recovers_packed_tile_indices_and_texture_classes() {
        let mut toc = HashMap::new();
        toc.insert(1, "HeightMapData".to_string());
        toc.insert(2, "BlendTileData".to_string());

        let mut height_payload = Vec::new();
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&0i32.to_le_bytes());
        height_payload.extend_from_slice(&1i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&4i32.to_le_bytes());
        height_payload.extend_from_slice(&[1, 2, 3, 4]);

        let mut blend_payload = Vec::new();
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        for value in [0i16, 4, 8, 12] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [1i16, 2, 3, 4] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 2]);
        blend_payload.extend_from_slice(&16i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&2i32.to_le_bytes());
        blend_payload.extend_from_slice(&0i32.to_le_bytes());
        ascii("Grass", &mut blend_payload);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 4, &height_payload));
        bytes.extend_from_slice(&chunk(2, 8, &blend_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("Synthetic.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .unwrap()
            .expect("heightmap should parse");
        let blend = parse_blend_tile_data_from_chunky(&chunky, &heightmap)
            .unwrap()
            .expect("blend data should parse");

        assert_eq!(blend.tile_ndxes, vec![0, 4, 8, 12]);
        assert_eq!(blend.blend_tile_ndxes, vec![1, 2, 3, 4]);
        assert_eq!(blend.extra_blend_tile_ndxes, vec![0, 0, 0, 0]);
        assert_eq!(blend.texture_classes.len(), 1);
        assert_eq!(blend.texture_classes[0].first_tile, 4);
        assert_eq!(blend.texture_classes[0].num_tiles, 4);
        assert_eq!(blend.texture_classes[0].width, 2);
        assert_eq!(blend.texture_classes[0].name, "Grass");
    }

    #[test]
    fn extra_blend_tile_ndxes_are_stored_on_parse_and_assign() {
        let mut toc = HashMap::new();
        toc.insert(1, "HeightMapData".to_string());
        toc.insert(2, "BlendTileData".to_string());

        let mut height_payload = Vec::new();
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&0i32.to_le_bytes());
        height_payload.extend_from_slice(&1i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&4i32.to_le_bytes());
        height_payload.extend_from_slice(&[1, 2, 3, 4]);

        let mut blend_payload = Vec::new();
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        for value in [0i16, 4, 8, 12] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [1i16, 2, 3, 4] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [5i16, 6, 7, 8] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 2]);
        blend_payload.extend_from_slice(&16i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&2i32.to_le_bytes());
        blend_payload.extend_from_slice(&0i32.to_le_bytes());
        ascii("Grass", &mut blend_payload);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 4, &height_payload));
        bytes.extend_from_slice(&chunk(2, 8, &blend_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticExtraBlend.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .unwrap()
            .expect("heightmap should parse");
        let blend = parse_blend_tile_data_from_chunky(&chunky, &heightmap)
            .unwrap()
            .expect("blend data should parse");

        assert_eq!(blend.extra_blend_tile_ndxes, vec![5, 6, 7, 8]);

        #[cfg(feature = "game_client")]
        {
            let mut hm = game_client::terrain::height_map::HeightMap::new(2, 2, 100.0, 1.0);
            hm.assign_extra_blend_tile_ndxes(blend.extra_blend_tile_ndxes.clone());
            assert_eq!(hm.extra_blend_tile_ndxes, vec![5, 6, 7, 8]);
            assert_eq!(hm.get_extra_blend_tile_index(0, 0), 5);
            assert_eq!(hm.get_extra_blend_tile_index(1, 1), 8);
        }
    }

    fn dict_int(toc_id: u32, value: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let key_and_type = (toc_id << 8) | 1;
        bytes.extend_from_slice(&(key_and_type as i32).to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn dict_real(toc_id: u32, value: f32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let key_and_type = (toc_id << 8) | 2;
        bytes.extend_from_slice(&(key_and_type as i32).to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn icoord(x: i32, y: i32, z: i32, out: &mut Vec<u8>) {
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
    }

    fn object_endpoint(x: f32, y: f32, z: f32, flags: i32, name: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        payload.extend_from_slice(&0f32.to_le_bytes());
        payload.extend_from_slice(&flags.to_le_bytes());
        ascii(name, &mut payload);
        payload
    }

    #[test]
    fn live_fast_path_parses_water_polygons_and_bridge_endpoints() {
        // C++ PolygonTrigger::ParsePolygonTriggersDataChunk + WorldInfo waterHeight
        // + W3DBridgeBuffer::addBridge → TerrainLogic::addBridgeToLogic.
        // Pre-fix live MapData hard-coded water_height: None / polygon_triggers: Vec::new()
        // and never called add_bridge_to_logic.
        let mut toc = HashMap::new();
        toc.insert(1, "WorldInfo".to_string());
        toc.insert(2, "PolygonTriggers".to_string());
        toc.insert(3, "ObjectsList".to_string());
        toc.insert(4, "Object".to_string());
        toc.insert(5, "waterHeight".to_string());

        let mut polygon_payload = Vec::new();
        polygon_payload.extend_from_slice(&1i32.to_le_bytes());
        ascii("Lake", &mut polygon_payload);
        ascii("water", &mut polygon_payload);
        polygon_payload.extend_from_slice(&3i32.to_le_bytes());
        polygon_payload.push(1);
        polygon_payload.push(0);
        polygon_payload.extend_from_slice(&0i32.to_le_bytes());
        polygon_payload.extend_from_slice(&4i32.to_le_bytes());
        icoord(0, 0, 12, &mut polygon_payload);
        icoord(40, 0, 12, &mut polygon_payload);
        icoord(40, 40, 12, &mut polygon_payload);
        icoord(0, 40, 12, &mut polygon_payload);

        let mut objects_payload = Vec::new();
        objects_payload.extend_from_slice(&chunk(
            4,
            3,
            &object_endpoint(0.0, 0.0, 20.0, FLAG_BRIDGE_POINT1, "TestBridge"),
        ));
        objects_payload.extend_from_slice(&chunk(
            4,
            3,
            &object_endpoint(40.0, 0.0, 20.0, FLAG_BRIDGE_POINT2, "TestBridge"),
        ));

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 1, &dict_real(5, 7.5)));
        bytes.extend_from_slice(&chunk(2, 4, &polygon_payload));
        bytes.extend_from_slice(&chunk(3, 3, &objects_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticWaterBridge.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let water_height = parse_runtime_water_height_from_chunky(&chunky)
            .expect("WorldInfo parse should fail-soft, not error")
            .expect("waterHeight must be recovered from WorldInfo");
        assert_eq!(water_height, 7.5);

        let triggers = parse_runtime_polygon_triggers_from_chunky(&chunky)
            .expect("PolygonTriggers parse should fail-soft, not error");
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].is_water_area());
        assert_eq!(triggers[0].get_trigger_name().as_str(), "Lake");
        assert_eq!(triggers[0].get_point(0).map(|p| p.z), Some(12));

        let bridges =
            parse_runtime_bridges_from_chunky(&chunky).expect("bridge endpoints should pair");
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].template_name, "TestBridge");
        assert_eq!(bridges[0].from.z, 20.0);
        assert_eq!(bridges[0].to.z, 20.0);

        let missing = ChunkyMap {
            source: PathBuf::from("Empty.map"),
            toc: HashMap::new(),
            body_offset: 0,
            bytes: Vec::new(),
        };
        assert_eq!(
            parse_runtime_water_height_from_chunky(&missing).unwrap(),
            None
        );
        assert!(
            parse_runtime_polygon_triggers_from_chunky(&missing)
                .unwrap()
                .is_empty()
        );

        let mut map_data = gamelogic::system::map_loader::MapData::new();
        map_data.water_height = Some(water_height);
        map_data.polygon_triggers = triggers;
        map_data.bridges = bridges;
        let mut terrain = gamelogic::terrain::TerrainLogic::new();
        terrain.load_map_data(map_data);
        assert_eq!(
            terrain
                .get_water_handle(10.0, 10.0)
                .map(|handle| handle.get_current_height()),
            Some(12.0)
        );
        assert_eq!(
            terrain
                .get_first_bridge()
                .map(|bridge| bridge.get_bridge_template_name().as_str().to_string())
                .as_deref(),
            Some("TestBridge")
        );
    }

    #[test]
    fn live_fast_path_source_wires_water_and_add_bridge() {
        let src = concat!(
            include_str!("../world_save.rs"),
            include_str!("../world_save/world_subsystems.rs"),
            include_str!("../world_save/world_paths.rs"),
            include_str!("../world_save/world_runtime.rs"),
            include_str!("../world_save/world_players.rs"),
            include_str!("../world_save/world_load.rs"),
        );
        assert!(
            src.contains("parse_runtime_water_height_from_chunky")
                && src.contains("parse_runtime_polygon_triggers_from_chunky")
                && src.contains("map_data.water_height = water_height")
                && src.contains("map_data.polygon_triggers = polygon_triggers")
                && src.contains("map_data.bridges = bridges"),
            "live fast-path MapData must keep water polygons/height and parsed bridges"
        );
        let terrain_src = concat!(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/map_height.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/bridge.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/bridges.rs"
            )),
        );
        let prod = terrain_src
            .split("#[cfg(test)]")
            .next()
            .expect("production");
        assert!(
            prod.contains("add_bridge_to_logic") && prod.contains("bridge_info_from_map_data"),
            "TerrainLogic::load_map_data must call add_bridge_to_logic for map bridges"
        );
    }
}

#[cfg(test)]
mod locate_map_file_workspace_residual_tests {
    #[test]
    fn locate_map_file_searches_parent_workspace_roots() {
        let src = include_str!("../script_loader.rs");
        assert!(
            src.contains("Workspace-relative residual")
                && src.contains("search_roots")
                && src.contains("CARGO_MANIFEST_DIR")
                && src.contains("extracted_big_files/MapsZH"),
            "locate_map_file must search parent workspace roots for windows_game extracts"
        );
    }

    #[test]
    fn find_map_file_resolves_shellmapmd_from_generals_ini_name() {
        let found = crate::game_logic::find_map_file(r"Maps\ShellMapMD\ShellMapMD.map")
            .or_else(|| crate::game_logic::find_map_file("ShellMapMD"));
        if found.is_none() {
            return;
        }
        let path = found.unwrap();
        assert!(
            path.is_file(),
            "resolved ShellMapMD must be a real file: {}",
            path.display()
        );
        assert!(
            path.to_string_lossy()
                .to_ascii_lowercase()
                .contains("shellmapmd"),
            "unexpected resolve {}",
            path.display()
        );
    }
}
