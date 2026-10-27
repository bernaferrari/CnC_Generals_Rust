// C++ ownership: WorldHeightMap::ParseLightingDataChunk and GameLogic::loadMapINI map settings.

/// CPP loadMapINI loads Map.ini then Solo.ini from the pristine map source.
/// Reuse the map settings reader; retain the concrete source for diagnostics.
pub(crate) fn read_weapon_override_companions(source: &str) -> Vec<(PathBuf, String)> {
    let Some(map_path) = resolve_map_companion_source(source) else {
        return Vec::new();
    };
    let Some(directory) = map_path.parent() else {
        return Vec::new();
    };
    [["Map.ini", "map.ini"], ["Solo.ini", "solo.ini"]]
        .into_iter()
        .filter_map(|names| first_readable_map_ini_companion(directory, &names))
        .collect()
}

/// Native receiving definitions distinguish missing optional companions from
/// selected-source read or decoding failures. Selection is Map then Solo.
pub(crate) fn read_receiving_definition_companions(
    source: &str,
) -> Result<Vec<(PathBuf, String)>, String> {
    let Some(path) = resolve_map_companion_source(source) else {
        return Ok(Vec::new());
    };
    let Some(directory) = path.parent() else {
        return Ok(Vec::new());
    };
    let mut sources = Vec::new();
    for names in [["Map.ini", "map.ini"], ["Solo.ini", "solo.ini"]] {
        for name in names {
            let path = directory.join(name);
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    sources.push((path, contents));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!(
                        "Cannot read definition '{}': {error}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(sources)
}

fn selected_map_definition_companion(
    directory: &Path,
    names: &[&str],
    admitted_sources: Option<&[(PathBuf, String)]>,
) -> Option<(PathBuf, String)> {
    if let Some(sources) = admitted_sources {
        sources
            .iter()
            .find(|(path, _)| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        names
                            .iter()
                            .any(|candidate| name.eq_ignore_ascii_case(candidate))
                    })
            })
            .cloned()
    } else {
        first_readable_map_ini_companion(directory, names)
    }
}

#[cfg(test)]
mod weapon_override_companion_tests {
    use super::*;
    #[test]
    fn actual_companion_reader_preserves_map_then_solo_paths_without_geometry() {
        let directory = tempfile::tempdir().unwrap();
        let map = directory.path().join("SavedPristine.map");
        let map_ini = directory.path().join("Map.ini");
        let solo_ini = directory.path().join("Solo.ini");
        fs::write(&map_ini, "Weapon SameRule\nPrimaryDamage = 40\nEnd\n").unwrap();
        fs::write(&solo_ini, "Weapon SameRule\nPrimaryDamage = 55\nEnd\n").unwrap();
        assert!(
            !map.exists(),
            "saved geometry need not remain beside companions"
        );
        let texts = read_weapon_override_companions(map.to_str().unwrap());
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].0, map_ini);
        assert_eq!(texts[1].0, solo_ini);
        assert!(texts[0].1.contains("PrimaryDamage = 40"));
        assert!(texts[1].1.contains("PrimaryDamage = 55"));
        fs::remove_file(&map_ini).unwrap();
        fs::remove_file(&solo_ini).unwrap();
        assert!(
            texts[0].1.contains("PrimaryDamage = 40"),
            "owned text outlives source files"
        );
        assert!(read_weapon_override_companions(map.to_str().unwrap()).is_empty());
    }
}
/// Parse high-level settings like world bounds and lighting colors.
pub fn parse_map_settings(map_name: &str) -> LoaderResult<MapMetadata> {
    let mut meta = MapMetadata::default();
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(meta);
    };
    parse_map_settings_from_loaded_chunky(map_name, &chunky, meta, None, None, None)
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
    let time_of_day = reader.read_i32()?;

    // C++ writes both arrays for Morning..Night (WorldHeightMap.cpp:772-820).
    let empty = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0];
    let mut terrain_lights = [[empty; 3]; 4];
    let mut objects_lights = [[empty; 3]; 4];
    for tod in 0..4 {
        terrain_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        objects_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        if version >= 2 {
            for extra in 1..3 {
                objects_lights[tod][extra] = read_global_lighting_row(&mut reader)?;
            }
        }
        if version >= 3 {
            for extra in 1..3 {
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
    use game_engine::common::ini::ini_game_data::TimeOfDay;
    meta.lighting = Some(crate::game_logic::lighting_rules::MapLighting {
        time_of_day: match time_of_day {
            1 => TimeOfDay::Morning,
            2 => TimeOfDay::Afternoon,
            3 => TimeOfDay::Evening,
            4 => TimeOfDay::Night,
            _ => TimeOfDay::Invalid,
        },
        terrain: terrain_lights,
        objects: objects_lights,
    });
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
pub fn collect_map_ini_particle_systems(contents: &str, receiving: &mut Vec<String>) -> usize {
    let extracted = extract_map_ini_particle_system_blocks(contents);
    let count = extracted
        .lines()
        .filter(|line| {
            line.trim()
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("ParticleSystem"))
        })
        .count();
    if count != 0 {
        receiving.push(extracted);
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

/// Parse into a detached map definition using the ordinary INI tokenizer and
/// C++ base-copy override chain. No live renderer or weather manager is touched.
fn overlay_owned_map_weather(
    contents: &str,
    definition: &mut Option<game_engine::common::ini::ini_weather::WeatherSetting>,
    updates: &mut Vec<game_engine::common::ini::ini_weather::WeatherSetting>,
) -> LoaderResult<()> {
    let extracted = extract_map_ini_weather_blocks(contents);
    INI::new()
        .with_inline_source_and_load_type(&extracted, INILoadType::CreateOverrides, |ini| {
            ini.parse_current_file_into_weather(definition, updates)
        })
        .map_err(|error| GameLogicError::Configuration(format!("map Weather: {error}")))
}

/// Overlay map.ini `Weather` onto `TheWeatherSetting` via CREATE_OVERRIDES
/// (C++ GameLogic.cpp:2407-2408 `ini.load(..., INI_LOAD_CREATE_OVERRIDES)`).
/// Returns whether a Weather block was applied.
pub fn overlay_map_ini_weather(contents: &str) -> bool {
    game_engine::common::ini::ini_weather::clear_weather_setting_overrides();
    let extracted = extract_map_ini_weather_blocks(contents);
    if extracted.trim().is_empty() {
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
    true
}

/// Parse settings from an already-decompressed chunky map.
pub fn parse_map_settings_from_chunky(chunky: &ChunkyMap) -> LoaderResult<MapMetadata> {
    let map_name = chunky.source.to_string_lossy();
    parse_map_settings_from_loaded_chunky(
        map_name.as_ref(),
        chunky,
        MapMetadata::default(),
        None,
        None,
        None,
    )
}

/// Production map and Solo overrides parse only into the driving draft.
pub(crate) fn parse_map_settings_from_chunky_with_ai_data(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
) -> LoaderResult<MapMetadata> {
    let map_name = chunky.source.to_string_lossy();
    parse_map_settings_from_chunky_with_ai_data_from_source(chunky, draft, map_name.as_ref(), None)
}

/// Decode the supplied CKMP body, while resolving companion files explicitly.
pub(crate) fn parse_map_settings_from_chunky_with_ai_data_from_source(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_chunky_with_content_rules(
        chunky,
        draft,
        definition_source,
        production_energy,
        None,
    )
}

pub(crate) fn parse_map_settings_from_chunky_with_content_rules(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
    physics_rules: Option<game_engine::common::ini::ini_game_data::PhysicsContentRules>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_chunky_with_camera_rules(
        chunky,
        draft,
        definition_source,
        production_energy,
        physics_rules,
        None,
    )
}

pub(crate) fn parse_map_settings_from_chunky_with_camera_rules(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
    physics_rules: Option<game_engine::common::ini::ini_game_data::PhysicsContentRules>,
    camera_rules: Option<game_engine::common::ini::ini_game_data::CameraContentRules>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_chunky_with_camera_and_clip_rules(
        chunky,
        draft,
        definition_source,
        production_energy,
        physics_rules,
        camera_rules,
        None,
        None,
    )
}

pub(crate) fn parse_map_settings_from_chunky_with_camera_and_clip_rules(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
    physics_rules: Option<game_engine::common::ini::ini_game_data::PhysicsContentRules>,
    camera_rules: Option<game_engine::common::ini::ini_game_data::CameraContentRules>,
    camera_clip_rules: Option<game_engine::common::ini::ini_game_data::CameraClipRules>,
    camera_shake_rules: Option<game_engine::common::ini::ini_game_data::CameraShakeRules>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_chunky_with_owned_terrain_definitions(
        chunky,
        draft,
        definition_source,
        production_energy,
        physics_rules,
        camera_rules,
        camera_clip_rules,
        camera_shake_rules,
        std::sync::Arc::new(game_engine::common::ini::ini_terrain::TerrainTypes::new()),
    )
}

pub(crate) fn parse_map_settings_from_chunky_with_owned_terrain_definitions(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
    physics_rules: Option<game_engine::common::ini::ini_game_data::PhysicsContentRules>,
    camera_rules: Option<game_engine::common::ini::ini_game_data::CameraContentRules>,
    camera_clip_rules: Option<game_engine::common::ini::ini_game_data::CameraClipRules>,
    camera_shake_rules: Option<game_engine::common::ini::ini_game_data::CameraShakeRules>,
    terrain_definitions: std::sync::Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_loaded_chunky(
        definition_source,
        chunky,
        MapMetadata {
            terrain_definitions: Some(terrain_definitions),
            camera_clip_rules,
            camera_shake_rules,
            camera_rules,
            production_energy,
            physics_rules,
            ..MapMetadata::default()
        },
        Some(draft),
        None,
        None,
    )
}

pub(crate) fn parse_map_settings_from_chunky_with_owned_definition_catalogs(
    chunky: &ChunkyMap,
    draft: &mut game_engine::common::ini::ini_ai_data::AIDataStore,
    definition_source: &str,
    production_energy: Option<game_engine::common::ini::ini_game_data::ProductionEnergyRules>,
    physics_rules: Option<game_engine::common::ini::ini_game_data::PhysicsContentRules>,
    camera_rules: Option<game_engine::common::ini::ini_game_data::CameraContentRules>,
    camera_clip_rules: Option<game_engine::common::ini::ini_game_data::CameraClipRules>,
    camera_shake_rules: Option<game_engine::common::ini::ini_game_data::CameraShakeRules>,
    terrain_definitions: std::sync::Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
    road_definitions: game_engine::common::ini::ini_road::TerrainRoadCollection,
    armor_definitions: Option<std::sync::Arc<gamelogic::object::armor::ArmorDefinitions>>,
    admitted_object_sources: Option<&[(PathBuf, String)]>,
    upgrade_definitions: gamelogic::upgrade::center::UpgradeCenter,
    science_facts: std::sync::Arc<game_engine::common::rts::science::ScienceNameFacts>,
) -> LoaderResult<MapMetadata> {
    parse_map_settings_from_loaded_chunky(
        definition_source,
        chunky,
        MapMetadata {
            upgrade_definitions: Some(upgrade_definitions),
            terrain_definitions: Some(terrain_definitions),
            road_definitions: Some(road_definitions),
            armor_definitions,
            camera_clip_rules,
            camera_shake_rules,
            camera_rules,
            production_energy,
            physics_rules,
            ..MapMetadata::default()
        },
        Some(draft),
        Some(science_facts),
        admitted_object_sources,
    )
}

fn parse_map_settings_from_loaded_chunky(
    map_name: &str,
    chunky: &ChunkyMap,
    mut meta: MapMetadata,
    mut ai_data_target: Option<&mut game_engine::common::ini::ini_ai_data::AIDataStore>,
    science_facts: Option<std::sync::Arc<game_engine::common::rts::science::ScienceNameFacts>>,
    admitted_object_sources: Option<&[(PathBuf, String)]>,
) -> LoaderResult<MapMetadata> {
    let mut terrain_definitions = meta
        .terrain_definitions
        .as_deref()
        .cloned()
        .unwrap_or_else(game_engine::common::ini::ini_terrain::TerrainTypes::new);
    let mut road_definitions = meta.road_definitions.take().unwrap_or_default();
    let mut upgrade_definitions = meta.upgrade_definitions.take().unwrap_or_else(|| {
        let mut definitions = game_engine::common::system::upgrade::UpgradeCenter::new();
        definitions.init();
        definitions
    });
    let mut armor_definitions = meta
        .armor_definitions
        .as_deref()
        .cloned()
        .unwrap_or_default();
    // Capture pristine immutable content before any other map's overrides.
    if meta.weather_definition.is_none() {
        meta.weather_definition = game_engine::common::ini::ini_weather::get_base_weather_setting();
    }
    let body = &chunky.bytes[chunky.body_offset..];
    if let Some((ver, payload)) = find_chunk_by_label(body, &chunky.toc, "GlobalLighting")? {
        parse_lighting_payload_for_settings(ver, payload, &mut meta)?;
    }

    if let Some((min, max)) = parse_world_bounds_from_chunky(chunky).ok().flatten() {
        meta.world_min = Some(min);
        meta.world_max = Some(max);
    }

    match parse_object_placements_from_chunky(chunky, &road_definitions) {
        Ok(objects) => {
            meta.authored_scorches = objects
                .iter()
                .filter_map(PlacedObject::terrain_scorch)
                .collect();
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
    if let Some(map_path) = resolve_map_companion_source(map_name)
        .or_else(|| admitted_object_sources.map(|_| PathBuf::from(map_name)))
    {
        if let Some(dir) = map_path.parent() {
            if let Some((_, contents)) = selected_map_definition_companion(
                dir,
                &["Map.ini", "map.ini"],
                admitted_object_sources,
            ) {
                if let Some(rules) = meta.camera_shake_rules {
                    meta.camera_shake_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.camera_clip_rules {
                    meta.camera_clip_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.camera_rules {
                    meta.camera_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.physics_rules {
                    meta.physics_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.production_energy {
                    meta.production_energy = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                meta.default_starting_cash =
                    parse_map_default_starting_cash(&contents)?.or(meta.default_starting_cash);
                // C++ GameLogic.cpp:2404-2408 loadMapINI — full block table
                // via INI_LOAD_CREATE_OVERRIDES (CommandSet/CommandButton/Upgrade).
                let (_, updated, roads, armor, upgrades) =
                    gamelogic::system::game_initialization::load_map_ini_ui_overrides_with_owned_upgrade_definitions(
                        &contents,
                        ai_data_target.as_deref_mut(),
                        science_facts.clone(),
                        terrain_definitions,
                        road_definitions,
                        armor_definitions.to_parsed(),
                        admitted_object_sources.is_some(),
                        upgrade_definitions,
                    )
                    .map_err(GameLogicError::Configuration)?;
                upgrade_definitions = upgrades;
                terrain_definitions = updated;
                road_definitions = roads;
                armor_definitions
                    .overlay_parsed(&armor)
                    .map_err(|error| GameLogicError::Configuration(error.to_string()))?;
                collect_map_ini_particle_systems(&contents, &mut meta.particle_definition_overlays);
                // Weather CREATE_OVERRIDES + SnowManager flake spacing.
                overlay_owned_map_weather(
                    &contents,
                    &mut meta.weather_definition,
                    &mut meta.weather_definition_updates,
                )?;

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
            if let Some((_, contents)) = selected_map_definition_companion(
                dir,
                &["Solo.ini", "solo.ini"],
                admitted_object_sources,
            ) {
                if let Some(rules) = meta.camera_shake_rules {
                    meta.camera_shake_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.camera_clip_rules {
                    meta.camera_clip_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.camera_rules {
                    meta.camera_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.physics_rules {
                    meta.physics_rules = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                if let Some(rules) = meta.production_energy {
                    meta.production_energy = Some(
                        rules
                            .overlay_ini_text(&contents)
                            .map_err(GameLogicError::Configuration)?,
                    );
                }
                meta.default_starting_cash =
                    parse_map_default_starting_cash(&contents)?.or(meta.default_starting_cash);
                let (_, updated, roads, armor, upgrades) =
                    gamelogic::system::game_initialization::load_map_ini_ui_overrides_with_owned_upgrade_definitions(
                        &contents,
                        ai_data_target.as_deref_mut(),
                        science_facts.clone(),
                        terrain_definitions,
                        road_definitions,
                        armor_definitions.to_parsed(),
                        admitted_object_sources.is_some(),
                        upgrade_definitions,
                    )
                    .map_err(GameLogicError::Configuration)?;
                upgrade_definitions = upgrades;
                terrain_definitions = updated;
                road_definitions = roads;
                armor_definitions
                    .overlay_parsed(&armor)
                    .map_err(|error| GameLogicError::Configuration(error.to_string()))?;
                collect_map_ini_particle_systems(&contents, &mut meta.particle_definition_overlays);
                overlay_owned_map_weather(
                    &contents,
                    &mut meta.weather_definition,
                    &mut meta.weather_definition_updates,
                )?;
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

    // Map.ini and Solo.ini can add road names after the original object read.
    meta.objects
        .retain(|object| road_definitions.find_road(&object.template).is_none());
    meta.authored_scorches = meta
        .objects
        .iter()
        .filter_map(PlacedObject::terrain_scorch)
        .collect();
    meta.terrain_definitions = Some(std::sync::Arc::new(terrain_definitions));
    meta.road_definitions = Some(road_definitions);
    // None is the explicit mapless residual profile. A map without Armor
    // blocks does not silently change it into an authored empty catalogue.
    // Native startup always supplies Some, including an intentionally empty set.
    meta.upgrade_definitions = Some(upgrade_definitions);
    if meta.armor_definitions.is_some() || !armor_definitions.is_empty() {
        meta.armor_definitions = Some(std::sync::Arc::new(armor_definitions));
    }
    Ok(meta)
}

/// Read the non-nested GameData block through the owned Common INI cursor.
/// This preserves optional `=`, whitespace and comments without registry dispatch.
fn parse_map_default_starting_cash(contents: &str) -> LoaderResult<Option<u32>> {
    let mut ini = INI::new();
    ini.with_inline_source(contents, |ini| {
        let mut game_data = false;
        let mut cash = None;
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                break;
            }
            let Some(key) = ini.get_next_token_or_null() else {
                continue;
            };
            if key.eq_ignore_ascii_case("GameData") {
                game_data = true;
            } else if key.eq_ignore_ascii_case("End") {
                game_data = false;
            } else if game_data && key.eq_ignore_ascii_case("DefaultStartingCash") {
                cash = Some(INI::parse_unsigned_int(&ini.get_next_token()?)?);
            }
        }
        Ok(cash)
    })
    .map_err(|err| GameLogicError::Configuration(format!("DefaultStartingCash: {err}")))
}

#[cfg(test)]
mod cash_tests {
    use super::*;
    #[test]
    fn owned_cash_parser_accepts_cpp_delimiters_comments_and_last_override() {
        let source = "PlayerTemplate Test\nStartMoney 7\nEnd\nGameData\nDefaultStartingCash 12500 ; comment\nEnd\nGameData\nDefaultStartingCash=0\nEnd";
        assert_eq!(parse_map_default_starting_cash(source).unwrap(), Some(0));
        assert_eq!(
            parse_map_default_starting_cash("GameData\nDefaultStartingCash=4294967295\nEnd")
                .unwrap(),
            Some(u32::MAX)
        );
        assert_eq!(
            parse_map_default_starting_cash("PlayerTemplate Test\nDefaultStartingCash = 999\nEnd")
                .unwrap(),
            None
        );
    }
    #[test]
    fn owned_cash_parser_rejects_missing_or_malformed_cash() {
        for token in ["", "bad"] {
            assert!(
                parse_map_default_starting_cash(&format!(
                    "GameData\nDefaultStartingCash = {token}\nEnd"
                ))
                .is_err()
            );
        }
    }
}

#[cfg(test)]
mod owned_weather_tests {
    use super::*;
    #[test]
    fn absent_base_first_block_becomes_base_for_later_overrides() {
        let mut definition = None;
        let mut updates = Vec::new();
        overlay_owned_map_weather(
            "Weather\n SnowVelocity = 17\nEnd\n",
            &mut definition,
            &mut updates,
        )
        .unwrap();
        overlay_owned_map_weather(
            "Weather\n SnowAmplitude = 23\nEnd\n",
            &mut definition,
            &mut updates,
        )
        .unwrap();
        assert_eq!(
            definition.as_ref().unwrap().final_override().snow_velocity,
            17.
        );
        assert_eq!(
            definition.as_ref().unwrap().final_override().snow_amplitude,
            23.
        );
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].snow_velocity, 17.);
        assert_eq!(updates[1].snow_amplitude, 23.);
    }
    #[test]
    fn map_and_solo_weather_use_detached_cpp_base_copy_overrides() {
        let mut owner = Some(game_engine::common::ini::ini_weather::WeatherSetting::default());
        let mut peer = owner.clone();
        let mut updates = Vec::new();
        let mut peer_updates = Vec::new();
        overlay_owned_map_weather(
            "Weather\n SnowVelocity = 17\n SnowEnabled = Yes\nEnd\n",
            &mut owner,
            &mut updates,
        )
        .unwrap();
        overlay_owned_map_weather(
            "Weather\n SnowAmplitude = 23\nEnd\n",
            &mut owner,
            &mut updates,
        )
        .unwrap();
        let final_definition = owner.as_ref().unwrap().final_override();
        assert_eq!(final_definition.snow_amplitude, 23.);
        assert_eq!(
            final_definition.snow_velocity, 4.,
            "CPP copies pristine base, not previous override"
        );
        assert!(!final_definition.snow_enabled);
        overlay_owned_map_weather(
            "Weather\n SnowVelocity = 31\nEnd\n",
            &mut peer,
            &mut peer_updates,
        )
        .unwrap();
        assert_eq!(peer.as_ref().unwrap().final_override().snow_velocity, 31.);
        assert_eq!(owner.as_ref().unwrap().final_override().snow_velocity, 4.);
        assert!(
            overlay_owned_map_weather(
                "Weather\n SnowVelocity = broken\nEnd\n",
                &mut peer,
                &mut peer_updates
            )
            .is_err()
        );
        assert_eq!(peer.as_ref().unwrap().final_override().snow_velocity, 31.);
        assert_eq!(updates.len(), 2);
        assert_eq!(
            peer_updates.len(),
            1,
            "failed block emits no admitted update"
        );
        assert_eq!(updates[0].snow_velocity, 17.);
        assert_eq!(updates[1].snow_velocity, 4.);
    }
}

#[cfg(test)]
#[path = "receiving_definition_companion_tests.rs"]
mod receiving_definition_companion_tests;
