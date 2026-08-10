// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn ensure_meta_map_loaded() {
    META_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("CommandMap", parse_meta_map_definition);
    });

    if get_meta_map()
        .read()
        .map(|guard| !guard.records.is_empty())
        .unwrap_or(false)
    {
        return;
    }

    load_meta_map_files();
}

fn load_meta_map_files() {
    let mut ini = INI::new();
    let paths = discover_command_map_files();
    for (index, path) in paths.into_iter().enumerate() {
        let load_type = if index == 0 {
            INILoadType::Overwrite
        } else {
            INILoadType::MultiFile
        };
        let _ = ini.load(path, load_type);
    }
}

fn discover_command_map_files() -> Vec<PathBuf> {
    let mut roots = Vec::<PathBuf>::new();
    let mut seen_roots = HashSet::<PathBuf>::new();

    let mut push_root = |path: PathBuf| {
        if seen_roots.insert(path.clone()) {
            roots.push(path);
        }
    };

    push_root(PathBuf::from("."));
    if let Ok(current) = std::env::current_dir() {
        push_root(current.clone());
        for ancestor in current.ancestors() {
            push_root(ancestor.to_path_buf());
        }
    }

    if let Some(global) = get_global_data() {
        let mod_dir = global.read().mod_dir.clone();
        if !mod_dir.trim().is_empty() {
            push_root(PathBuf::from(mod_dir.trim()));
        }
    }

    let mut files = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        push_command_map_file(&mut files, &mut seen, root.join("Data/INI/CommandMap.ini"));
        push_command_map_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/CommandMapDebug.ini"),
        );
        push_command_map_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/CommandMapDemo.ini"),
        );

        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMap.ini"),
            );
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMapDebug.ini"),
            );
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMapDemo.ini"),
            );
        }

        for localized in [
            root.join("windows_game/extracted_big_files/EnglishZH"),
            root.join("windows_game/extracted_big_files/W3DEnglishZH"),
            root.join("windows_game/extracted_big_files_v2/EnglishZH"),
            root.join("windows_game/extracted_big_files_v2/W3DEnglishZH"),
        ] {
            push_command_map_file(
                &mut files,
                &mut seen,
                localized.join("Data/English/CommandMap.ini"),
            );
        }
    }

    files
}

fn push_command_map_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_file() {
        let key = std::fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

pub fn get_command_map_entries() -> Vec<CommandMapEntry> {
    ensure_meta_map_loaded();
    let guard = get_meta_map().read().unwrap_or_else(|e| e.into_inner());
    guard
        .iter()
        .map(|record| CommandMapEntry {
            key: record.key,
            mod_state: record.mod_state,
            category: record.category.clone(),
            description: record.description.clone(),
            display_name: record.display_name.clone(),
        })
        .collect()
}

pub fn update_command_map_entry(
    category: &str,
    display_name: &str,
    key: u32,
    mod_state: u32,
) -> bool {
    ensure_meta_map_loaded();
    let Ok(mut guard) = get_meta_map().write() else {
        return false;
    };

    let Some(record) = guard.records.iter_mut().find(|record| {
        record.category.eq_ignore_ascii_case(category)
            && record.display_name.eq_ignore_ascii_case(display_name)
    }) else {
        return false;
    };

    record.key = key;
    record.mod_state = mod_state;
    true
}

pub fn reset_command_map_entries() {
    META_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("CommandMap", parse_meta_map_definition);
    });
    if let Ok(mut guard) = get_meta_map().write() {
        guard.records.clear();
    }
    load_meta_map_files();
}

fn parse_meta_map_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    if !is_supported_command_map_name(&name) {
        return Err(INIError::InvalidData);
    }

    let meta = lookup_meta_message_type(&name);
    let mut record = MetaMapRec {
        name: name.clone(),
        meta,
        key: 0,
        transition: Transition::Down,
        mod_state: 0,
        usable_in: COMMANDUSABLE_NONE,
        category: String::new(),
        description: String::new(),
        display_name: String::new(),
    };

    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }

        match key.to_ascii_uppercase().as_str() {
            "KEY" => {
                if let Some(value) = values.first() {
                    if let Some(code) = lookup_key_code(value) {
                        record.key = code;
                    }
                }
            }
            "TRANSITION" => {
                if let Some(value) = values.first() {
                    record.transition = parse_transition(value);
                }
            }
            "MODIFIERS" => {
                if let Some(value) = values.first() {
                    record.mod_state = parse_mod_state(value);
                }
            }
            "USEABLEIN" => {
                record.usable_in = parse_usable_in(&values);
            }
            "CATEGORY" => {
                if let Some(value) = values.first() {
                    record.category = value.to_string();
                }
            }
            "DESCRIPTION" => {
                if let Some(value) = values.first() {
                    record.description = value.to_string();
                }
            }
            "DISPLAYNAME" => {
                if let Some(value) = values.first() {
                    record.display_name = value.to_string();
                }
            }
            _ => {}
        }
    }

    get_meta_map()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .add_record(record);
    Ok(())
}

fn is_dispatch_handled_cpp_command_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "CHEAT_ADD_CASH"
        | "CHEAT_DESHROUD"
        | "CHEAT_GIVE_ALL_SCIENCES"
        | "CHEAT_GIVE_SCIENCEPURCHASEPOINTS"
        | "CHEAT_INSTANT_BUILD"
        | "CHEAT_KILL_SELECTION"
        | "CHEAT_SHOW_HEALTH"
        | "CHEAT_SWITCH_TEAMS"
        | "CHEAT_TOGGLE_HAND_OF_GOD_MODE"
        | "CHEAT_TOGGLE_MESSAGE_TEXT"
        | "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS"
        | "DEMO_ADDCASH"
        | "DEMO_BEGIN_ADJUST_FOV"
        | "DEMO_BEGIN_ADJUST_PITCH"
        | "DEMO_BATTLE_CRY"
        | "DEBUG_DUMP_ALL_PLAYER_OBJECTS"
        | "DEBUG_DUMP_PLAYER_OBJECTS"
        | "DEBUG_DRAWABLE_ID_PERFORMANCE"
        | "DEBUG_OBJECT_ID_PERFORMANCE"
        | "DEBUG_SLEEPY_UPDATE_PERFORMANCE"
        | "DEMO_CYCLE_EXTENT_TYPE"
        | "DEMO_CYCLE_LOD_LEVEL"
        | "DEMO_DECR_ANIM_SKATE_SPEED"
        | "DEMO_DECR_EXTENT_HEIGHT"
        | "DEMO_DECR_EXTENT_HEIGHT_LARGE"
        | "DEMO_DECR_EXTENT_MAJOR"
        | "DEMO_DECR_EXTENT_MAJOR_LARGE"
        | "DEMO_DECR_EXTENT_MINOR"
        | "DEMO_DECR_EXTENT_MINOR_LARGE"
        | "DEMO_DESHROUD"
        | "DEMO_DUMP_ASSETS"
        | "DEMO_ENSHROUD"
        | "DEMO_END_ADJUST_FOV"
        | "DEMO_END_ADJUST_PITCH"
        | "DEMO_FREE_BUILD"
        | "DEMO_GIVE_ALL_SCIENCES"
        | "DEMO_GIVE_RANKLEVEL"
        | "DEMO_GIVE_SCIENCEPURCHASEPOINTS"
        | "DEMO_GIVE_VETERANCY"
        | "DEMO_INSTANT_BUILD"
        | "DEMO_INCR_EXTENT_HEIGHT"
        | "DEMO_INCR_EXTENT_HEIGHT_LARGE"
        | "DEMO_INCR_EXTENT_MAJOR"
        | "DEMO_INCR_EXTENT_MAJOR_LARGE"
        | "DEMO_INCR_EXTENT_MINOR"
        | "DEMO_INCR_EXTENT_MINOR_LARGE"
        | "DEMO_INCR_ANIM_SKATE_SPEED"
        | "DEMO_KILL_ALL_ENEMIES"
        | "DEMO_KILL_SELECTION"
        | "DEMO_LOCK_CAMERA_TO_PLANES"
        | "DEMO_LOCK_CAMERA_TO_SELECTION"
        | "DEMO_LOD_DECREASE"
        | "DEMO_LOD_INCREASE"
        | "DEMO_MUSIC_NEXT_TRACK"
        | "DEMO_MUSIC_PREV_TRACK"
        | "DEMO_NEXT_OBJECTIVE_MOVIE"
        | "DEMO_PERFORM_STATISTICAL_DUMP"
        | "DEMO_PLAY_CAMEO_MOVIE"
        | "DEMO_REMOVE_PREREQ"
        | "DEMO_SHOW_AUDIO_LOCATIONS"
        | "DEMO_SHOW_EXTENTS"
        | "DEMO_SHOW_HEALTH"
        | "DEMO_SWITCH_TEAMS"
        | "DEMO_SWITCH_TEAMS_CHINA_USA"
        | "DEMO_SWITCH_TEAMS_BETWEEN_CHINA_USA"
        | "DEMO_TAKE_RANKLEVEL"
        | "DEMO_TAKE_VETERANCY"
        | "DEMO_TIME_OF_DAY"
        | "DEMO_DEBUG_SELECTION"
        | "DEMO_TOGGLE_AI_DEBUG"
        | "DEMO_TOGGLE_AUDIODEBUG"
        | "DEMO_TOGGLE_AVI"
        | "DEMO_TOGGLE_BEHIND_BUILDINGS"
        | "DEMO_TOGGLE_CASHMAPDEBUG"
        | "DEMO_TOGGLE_CAMERA_DEBUG"
        | "DEMO_TOGGLE_DEBUG_STATS"
        | "DEMO_TOGGLE_FEATHER_WATER"
        | "DEMO_TOGGLE_FOGOFWAR"
        | "DEMO_TOGGLE_GRAPHICALFRAMERATEBAR"
        | "DEMO_TOGGLE_GREEN_VIEW"
        | "DEMO_TOGGLE_MESSAGE_TEXT"
        | "DEMO_TOGGLE_METRICS"
        | "DEMO_TOGGLE_MILITARY_SUBTITLES"
        | "DEMO_TOGGLE_MOTION_BLUR_ZOOM"
        | "DEMO_TOGGLE_MUSIC"
        | "DEMO_TOGGLE_NETWORK"
        | "DEMO_TOGGLE_NO_DRAW"
        | "DEMO_TOGGLE_PARTICLEDEBUG"
        | "DEMO_TOGGLE_PROJECTILEDEBUG"
        | "DEMO_TOGGLE_BW_VIEW"
        | "DEMO_TOGGLE_RED_VIEW"
        | "DEMO_TOGGLE_RENDER"
        | "DEMO_TOGGLE_LETTERBOX"
        | "DEMO_TOGGLE_SHADOW_VOLUMES"
        | "DEMO_TOGGLE_SOUND"
        | "DEMO_TOGGLE_SPECIAL_POWER_DELAYS"
        | "DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT"
        | "DEMO_TOGGLE_HAND_OF_GOD_MODE"
        | "DEMO_TOGGLE_HURT_ME_MODE"
        | "DEMO_TEST_SURRENDER"
        | "DEMO_TOGGLE_THREATDEBUG"
        | "DEMO_TOGGLE_TRACKMARKS"
        | "DEMO_TOGGLE_VISIONDEBUG"
        | "DEMO_TOGGLE_WATERPLANE"
        | "DEMO_TOGGLE_ZOOM_LOCK"
        | "DEMO_VTUNE_OFF"
        | "DEMO_VTUNE_ON"
        | "HELP"
        | "DEMO_WIN" => true,
        _ => {
            parse_runscript_alias(&upper).is_some() || parse_objective_movie_alias(&upper).is_some()
        }
    }
}

fn is_unimplemented_cpp_command_name(name: &str) -> bool {
    // C++ MetaEvent.cpp table entries that exist in CommandMap files but are not
    // represented as typed Rust messages yet. Keep these accepted/consumed so keybind
    // behavior stays aligned while the full message pipeline is still being ported.
    if is_dispatch_handled_cpp_command_name(name) {
        return false;
    }

    false
}

fn is_runtime_command_map_alias(name: &str) -> bool {
    name.eq_ignore_ascii_case("PLACE_BEACON")
        || name.eq_ignore_ascii_case("DELETE_BEACON")
        || name.eq_ignore_ascii_case("TOGGLE_LOWER_DETAILS")
}

fn is_supported_command_map_name(name: &str) -> bool {
    lookup_meta_message_type(name).is_some()
        || is_runtime_command_map_alias(name)
        || is_dispatch_handled_cpp_command_name(name)
        || is_unimplemented_cpp_command_name(name)
}

fn with_local_player_mut<F>(f: F) -> bool
where
    F: FnOnce(&mut gamelogic::player::Player),
{
    let Some(local_player) = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
    else {
        return false;
    };

    let Ok(mut local_guard) = local_player.write() else {
        return false;
    };
    f(&mut local_guard);
    true
}
