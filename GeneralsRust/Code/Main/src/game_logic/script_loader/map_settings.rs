// C++ ownership: WorldHeightMap::ParseLightingDataChunk and GameLogic::loadMapINI map settings.

/// Parse high-level settings like world bounds and lighting colors.
pub fn parse_map_settings(map_name: &str) -> LoaderResult<MapMetadata> {
    let mut meta = MapMetadata::default();
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(meta);
    };
    parse_map_settings_from_loaded_chunky(map_name, &chunky, meta)
}

/// C++ `WorldHeightMap::ParseLightingDataChunk` (WorldHeightMap.cpp:758-829).
/// Payload is: i32 timeOfDay; then 4 TOD rows of terrain[0]+objects[0] (9 floats
/// each); v2 adds two extra object lights; v3 adds two extra terrain lights.
/// This chunk never carries sky/fog. Scene fog stays disabled like C++.
fn parse_lighting_payload_for_settings(
    version: u16,
    payload: &[u8],
    meta: &mut MapMetadata,
) -> LoaderResult<()> {
    let mut reader = BinaryReader::new(payload);
    if reader.remaining() < 4 {
        return Ok(());
    }
    let time_of_day = reader.read_i32()?;

    // C++ writes both arrays for Morning..Night (WorldHeightMap.cpp:772-820).
    let mut terrain_lights = [[[0.0f32; 9]; 3]; 4];
    let mut objects_lights = [[[0.0f32; 9]; 3]; 4];
    for tod in 0..4 {
        if reader.remaining() < 9 * 4 * 2 {
            break;
        }
        terrain_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        objects_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        if version >= 2 {
            for extra in 1..3 {
                if reader.remaining() < 9 * 4 {
                    break;
                }
                objects_lights[tod][extra] = read_global_lighting_row(&mut reader)?;
            }
        }
        if version >= 3 {
            for extra in 1..3 {
                if reader.remaining() < 9 * 4 {
                    break;
                }
                terrain_lights[tod][extra] = read_global_lighting_row(&mut reader)?;
            }
        }
    }

    // C++ TimeOfDay: Invalid=0, Morning=1, Afternoon=2, Evening=3, Night=4.
    let row_index = match time_of_day {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        _ => 1,
    };
    let terrain = terrain_lights[row_index][0];
    let objects = objects_lights[row_index][0];
    meta.ambient_color = Some([terrain[0], terrain[1], terrain[2]]);
    meta.sun_color = Some([terrain[3], terrain[4], terrain[5]]);
    meta.sun_direction = Some([terrain[6], terrain[7], terrain[8]]);
    // Units/shadows use the objects row (C++ W3DDisplay.cpp:2128-2147).
    meta.objects_ambient_color = Some([objects[0], objects[1], objects[2]]);
    meta.objects_sun_color = Some([objects[3], objects[4], objects[5]]);
    meta.objects_sun_direction = Some([objects[6], objects[7], objects[8]]);
    if version >= 2 {
        meta.objects_extra_lights =
            vec![objects_lights[row_index][1], objects_lights[row_index][2]];
    }
    if version >= 3 {
        meta.terrain_extra_lights =
            vec![terrain_lights[row_index][1], terrain_lights[row_index][2]];
    }
    apply_map_lighting_to_global_data(time_of_day, version, &terrain_lights, &objects_lights);
    // Never invent fog/sky from this chunk — C++ FogEnabled defaults false
    // and GlobalLighting has no fog fields.
    meta.fog_color = None;
    meta.fog_start = None;
    meta.fog_end = None;
    meta.sky_color = None;
    Ok(())
}

fn read_global_lighting_row(reader: &mut BinaryReader<'_>) -> LoaderResult<[f32; 9]> {
    Ok([
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
    ])
}

fn lighting_row_to_authored(
    row: [f32; 9],
) -> game_engine::common::ini::ini_game_data::TerrainLighting {
    use game_engine::common::ini::ini_game_data::{Coord3D, RGBColor, TerrainLighting};
    TerrainLighting {
        ambient: RGBColor::new(row[0], row[1], row[2]),
        diffuse: RGBColor::new(row[3], row[4], row[5]),
        light_pos: Coord3D::new(row[6], row[7], row[8]),
    }
}

/// C++ `WorldHeightMap::ParseLightingDataChunk` writes both lighting arrays
/// into `TheWritableGlobalData` for every TOD slot.
fn apply_map_lighting_to_global_data(
    time_of_day: i32,
    version: u16,
    terrain_lights: &[[[f32; 9]; 3]; 4],
    objects_lights: &[[[f32; 9]; 3]; 4],
) {
    use game_engine::common::ini::ini_game_data::{
        MAX_GLOBAL_LIGHTS, TIME_OF_DAY_FIRST, TimeOfDay, ensure_global_data,
    };
    let handle = ensure_global_data();
    let mut data = handle.write();
    data.time_of_day = match time_of_day {
        1 => TimeOfDay::Morning,
        2 => TimeOfDay::Afternoon,
        3 => TimeOfDay::Evening,
        4 => TimeOfDay::Night,
        _ => data.time_of_day,
    };
    for tod in 0..4 {
        let dest_tod = tod + TIME_OF_DAY_FIRST;
        if dest_tod >= data.terrain_lighting.len() {
            continue;
        }
        data.terrain_lighting[dest_tod][0] = lighting_row_to_authored(terrain_lights[tod][0]);
        data.terrain_objects_lighting[dest_tod][0] =
            lighting_row_to_authored(objects_lights[tod][0]);
        if version >= 2 {
            for light in 1..MAX_GLOBAL_LIGHTS {
                data.terrain_objects_lighting[dest_tod][light] =
                    lighting_row_to_authored(objects_lights[tod][light]);
            }
        }
        if version >= 3 {
            for light in 1..MAX_GLOBAL_LIGHTS {
                data.terrain_lighting[dest_tod][light] =
                    lighting_row_to_authored(terrain_lights[tod][light]);
            }
        }
    }
}

/// C++ GameLogic::loadMapINI — pull `ParticleSystem` blocks out of mixed map.ini.
pub fn extract_map_ini_particle_system_blocks(contents: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for raw in contents.lines() {
        let line = raw.split(';').next().unwrap_or("").trim_end();
        let trimmed = line.trim();
        if !in_block {
            if trimmed
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("ParticleSystem"))
            {
                in_block = true;
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if trimmed.eq_ignore_ascii_case("End") {
            in_block = false;
        }
    }
    out
}

/// Overlay map.ini ParticleSystem blocks onto the live GameClient manager
/// (C++ INIParticleSys.cpp via INI::load CREATE_OVERRIDES).
pub fn overlay_map_ini_particle_systems(contents: &str) -> usize {
    let extracted = extract_map_ini_particle_system_blocks(contents);
    if extracted.is_empty() {
        return 0;
    }
    let count = extracted
        .lines()
        .filter(|line| {
            line.trim()
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("ParticleSystem"))
        })
        .count();
    // Common INI dispatch hits the live overlay hook when GameClient registered it.
    let mut ini = INI::new();
    if let Err(err) = ini.with_inline_source(&extracted, |ini| ini.parse_current_file()) {
        warn!("map.ini ParticleSystem Common dispatch failed: {err}");
    }
    #[cfg(feature = "game_client")]
    {
        use game_client::effects::{
            ParticleSystemINIParser, ParticleSystemManager, get_particle_system_manager_mut,
        };
        if let Ok(mut guard) = get_particle_system_manager_mut() {
            let manager = guard.get_or_insert_with(ParticleSystemManager::new);
            let parser = ParticleSystemINIParser::default();
            if let Err(err) = parser.overlay_mixed_source(&extracted, manager) {
                warn!("map.ini ParticleSystem live overlay failed: {err}");
            }
        }
    }
    count
}

/// C++ GameLogic::loadMapINI — pull `Weather` blocks out of mixed map.ini.
pub fn extract_map_ini_weather_blocks(contents: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for raw in contents.lines() {
        let line = raw.split(';').next().unwrap_or("").trim_end();
        let trimmed = line.trim();
        if !in_block {
            if trimmed
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("Weather"))
            {
                in_block = true;
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if trimmed.eq_ignore_ascii_case("End") {
            in_block = false;
        }
    }
    out
}

/// C++ `GameLogic::loadMapINI` — dispatch the full INI block table with
/// `INI_LOAD_CREATE_OVERRIDES` so map-authored CommandSet/CommandButton/Upgrade
/// (and the rest of the table) actually apply.
pub fn overlay_map_ini_create_overrides(contents: &str) -> usize {
    match gamelogic::system::load_map_ini_ui_overrides_from_contents(contents) {
        Ok(applied) => applied,
        Err(err) => {
            warn!("map.ini CREATE_OVERRIDES dispatch failed: {err}");
            0
        }
    }
}

/// Overlay map.ini `Weather` onto `TheWeatherSetting` via CREATE_OVERRIDES
/// (C++ GameLogic.cpp:2407-2408 `ini.load(..., INI_LOAD_CREATE_OVERRIDES)`).
/// Returns whether a Weather block was applied.
pub fn overlay_map_ini_weather(contents: &str) -> bool {
    game_engine::common::ini::ini_weather::clear_weather_setting_overrides();
    let extracted = extract_map_ini_weather_blocks(contents);
    if extracted.trim().is_empty() {
        #[cfg(feature = "game_client")]
        sync_live_snow_manager_from_common();
        return false;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "map_weather_override_{}_{}.ini",
        std::process::id(),
        nanos
    ));
    if std::fs::write(&path, &extracted).is_err() {
        warn!("map.ini Weather overlay could not stage a temp INI");
        return false;
    }
    let mut ini = INI::new();
    let result = ini.with_file_source(&path, INILoadType::CreateOverrides, |ini| {
        ini.parse_current_file()
    });
    let _ = std::fs::remove_file(&path);
    if let Err(err) = result {
        warn!("map.ini Weather CREATE_OVERRIDES parse failed: {err}");
        return false;
    }
    #[cfg(feature = "game_client")]
    sync_live_snow_manager_from_common();
    true
}

#[cfg(feature = "game_client")]
fn sync_live_snow_manager_from_common() {
    // Common INI dispatch writes TheWeatherSetting; SnowManager still needs
    // the GameClient copy + flake spacing (C++ Snow.cpp parseWeatherDefinition).
    let _ = game_client::snow::get_weather_setting();
    let manager = game_client::snow::get_snow_manager()
        .unwrap_or_else(game_client::snow::initialize_snow_manager);
    if let Ok(mut guard) = manager.lock() {
        guard.update_ini_settings();
    }
}

/// Parse settings from an already-decompressed chunky map.
pub fn parse_map_settings_from_chunky(chunky: &ChunkyMap) -> LoaderResult<MapMetadata> {
    let map_name = chunky.source.to_string_lossy();
    parse_map_settings_from_loaded_chunky(map_name.as_ref(), chunky, MapMetadata::default())
}

fn parse_map_settings_from_loaded_chunky(
    map_name: &str,
    chunky: &ChunkyMap,
    mut meta: MapMetadata,
) -> LoaderResult<MapMetadata> {
    let body = &chunky.bytes[chunky.body_offset..];
    if let Some((ver, payload)) = find_chunk_by_label(body, &chunky.toc, "GlobalLighting")? {
        parse_lighting_payload_for_settings(ver, payload, &mut meta)?;
    }

    if let Some((min, max)) = parse_world_bounds_from_chunky(chunky).ok().flatten() {
        meta.world_min = Some(min);
        meta.world_max = Some(max);
    }

    match parse_object_placements_from_chunky(chunky) {
        Ok(objects) => {
            meta.objects = objects;
        }
        Err(err) => {
            warn!(
                "Failed to parse map object placements for '{}': {}",
                map_name, err
            );
        }
    }

    match parse_side_build_list_from_chunky(chunky) {
        Ok(builds) => {
            meta.side_builds = builds;
        }
        Err(err) => {
            warn!(
                "Failed to parse SidesList build entries for '{}': {}",
                map_name, err
            );
        }
    }

    match parse_player_start_waypoints_from_chunky(chunky) {
        Ok(starts) => {
            let mut wps = Vec::new();
            for (idx, pos, rally) in starts {
                wps.push((format!("Player_{}_Start", idx + 1), pos));
                if let Some(rally) = rally {
                    wps.push((format!("Player_{}_Rally", idx + 1), rally));
                }
            }
            meta.start_waypoints = wps;
        }
        Err(err) => {
            warn!(
                "Failed to parse player start waypoints for '{}': {}",
                map_name, err
            );
        }
    }

    meta.initial_camera_position = parse_initial_camera_position_from_chunky(chunky)
        .ok()
        .flatten();

    // Heightmap hint: look for common heightmap filenames next to the .map.
    if let Some(map_path) = locate_map_file(map_name) {
        if let Some(dir) = map_path.parent() {
            if let Some((_, contents)) =
                first_readable_map_ini_companion(dir, &["Map.ini", "map.ini"])
            {
                // C++ GameLogic.cpp:2404-2408 loadMapINI — full block table
                // via INI_LOAD_CREATE_OVERRIDES (CommandSet/CommandButton/Upgrade).
                let _ = overlay_map_ini_create_overrides(&contents);
                // ParticleSystem still needs the live GameClient manager hook.
                let _ = overlay_map_ini_particle_systems(&contents);
                // Weather CREATE_OVERRIDES + SnowManager flake spacing.
                let _ = overlay_map_ini_weather(&contents);

                let mut skybox_textures: [Option<String>; 5] = [None, None, None, None, None];
                for raw_line in contents.lines() {
                    let line = raw_line.split(';').next().unwrap_or("").trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Some((key, value)) = line.split_once('=') else {
                        continue;
                    };
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');
                    if value.is_empty() {
                        continue;
                    }
                    match key.to_ascii_lowercase().as_str() {
                        "skyboxtexturen" => skybox_textures[0] = Some(value.to_string()),
                        "skyboxtexturee" => skybox_textures[1] = Some(value.to_string()),
                        "skyboxtextures" => skybox_textures[2] = Some(value.to_string()),
                        "skyboxtexturew" => skybox_textures[3] = Some(value.to_string()),
                        "skyboxtexturet" => skybox_textures[4] = Some(value.to_string()),
                        _ => {}
                    }
                }
                if skybox_textures.iter().all(|texture| texture.is_some()) {
                    meta.skybox_textures = Some([
                        skybox_textures[0].clone().unwrap(),
                        skybox_textures[1].clone().unwrap(),
                        skybox_textures[2].clone().unwrap(),
                        skybox_textures[3].clone().unwrap(),
                        skybox_textures[4].clone().unwrap(),
                    ]);
                }
            }

            // C++ loadMapINI also loads companion solo.ini CREATE_OVERRIDES.
            if let Some((_, contents)) =
                first_readable_map_ini_companion(dir, &["Solo.ini", "solo.ini"])
            {
                let _ = overlay_map_ini_create_overrides(&contents);
            }

            // C++ parity: only treat dedicated heightmap companions as terrain sources.
            // Generic *.tga beside a map is commonly preview/sky art, not elevation data.
            for ext in ["hmp", "raw"] {
                let stem = map_path.file_stem().and_then(|stem| stem.to_str());
                let Some(stem) = stem else {
                    continue;
                };

                let mut candidate = dir.join(stem);
                candidate.set_extension(ext);
                if let Some(heightmap_path) = resolve_runtime_path(&candidate) {
                    meta.heightmap_path = Some(heightmap_path);
                    break;
                }
            }

            // Skybox hints: look for common texture names in the map folder.
            let faces = ["front", "back", "left", "right", "top"];
            let mut textures: [Option<String>; 5] = [None, None, None, None, None];
            for (i, face) in faces.iter().enumerate() {
                let mut candidate = dir.to_path_buf();
                candidate.push(format!("Sky{}.tga", face));
                if path_is_accessible(&candidate) {
                    textures[i] = Some(candidate.to_string_lossy().to_string());
                    continue;
                }
                let mut alt = dir.to_path_buf();
                alt.push(format!(
                    "{}{}.tga",
                    map_path.file_stem().unwrap_or_default().to_string_lossy(),
                    face
                ));
                if path_is_accessible(&alt) {
                    textures[i] = Some(alt.to_string_lossy().to_string());
                }
            }
            if meta.skybox_textures.is_none() && textures.iter().all(|t| t.is_some()) {
                meta.skybox_textures = Some([
                    textures[0].clone().unwrap(),
                    textures[1].clone().unwrap(),
                    textures[2].clone().unwrap(),
                    textures[3].clone().unwrap(),
                    textures[4].clone().unwrap(),
                ]);
            }
        }
    }

    Ok(meta)
}
