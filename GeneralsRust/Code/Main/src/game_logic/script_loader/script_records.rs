// Select the map script root and adapt the authored lists for world admission.

const PLAYER_SCRIPTS_LABEL: &str = "PlayerScriptsList";

fn load_sides_list_fallback(
    map_path: &Path,
    body: &[u8],
    toc: &HashMap<u32, String>,
    names: &mut NameKeyNamespace,
    scripts: ScriptListReadInfo,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let Some((sides_version, sides_payload)) = find_chunk_by_label(body, toc, SIDES_LIST_LABEL)?
    else {
        return Ok(None);
    };

    let script_lists =
        parse_script_lists_from_sides_chunk(sides_payload, toc, sides_version, names, scripts)?;
    let total = count_scripts(&script_lists);
    info!(
        "Decoded {} script lists ({} scripts) from '{}' via SidesList fallback",
        script_lists.len(),
        total,
        map_path.display()
    );
    Ok(Some(MapScriptLoadResult {
        source_path: map_path.to_path_buf(),
        script_lists,
        total_scripts: total,
    }))
}

/// Attempt to locate and decode scripts for the provided map.
pub fn load_map_scripts(map_name: &str) -> LoaderResult<Option<MapScriptLoadResult>> {
    let templates = current_script_templates();
    load_map_scripts_with_templates(map_name, &templates)
}

fn current_script_templates() -> ScriptTemplateLookup {
    gamelogic::scripting::engine::with_script_engine_ref(|engine| {
        engine.script_template_lookup()
    })
    .unwrap_or_default()
}

pub(super) fn load_map_scripts_with_templates(
    map_name: &str,
    templates: &ScriptTemplateLookup,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    NameKeyGenerator::with_namespace(|names| {
        load_map_scripts_with_catalog(
            map_name, templates.clone(), names, gamelogic::scripting::new_script_group,
        )
    })
}

pub(super) fn load_map_scripts_with_catalog(
    map_name: &str,
    templates: ScriptTemplateLookup,
    names: &mut NameKeyNamespace,
    new_group: impl Fn() -> ScriptGroup + 'static,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let Some(_) = locate_map_file(map_name) else {
        warn!(
            "No .map file could be found for '{}'; mission scripts unavailable",
            map_name
        );
        return Ok(None);
    };
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    load_map_scripts_from_chunky_with_catalog(&chunky, templates, names, new_group)
}

/// Decode mission scripts from an already-decompressed chunky map.
/// C++ `TerrainLogic::loadMap` (`TerrainLogic.cpp:1248-1262`) opens the
/// `.map` once via `CachedFileInputStream` and parses chunks from that stream.
pub fn load_map_scripts_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let templates = current_script_templates();
    load_map_scripts_from_chunky_with_templates(chunky, &templates)
}

/// Decode against the driving world's frozen template definitions.
/// No callback or leaf parser consults mutable engine storage.
pub(super) fn load_map_scripts_from_chunky_with_templates(
    chunky: &ChunkyMap,
    templates: &ScriptTemplateLookup,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    NameKeyGenerator::with_namespace(|names| {
        load_map_scripts_from_chunky_with_catalog(
            chunky, templates.clone(), names, gamelogic::scripting::new_script_group,
        )
    })
}

pub(super) fn load_map_scripts_from_chunky_with_catalog(
    chunky: &ChunkyMap,
    templates: ScriptTemplateLookup,
    names: &mut NameKeyNamespace,
    new_group: impl Fn() -> ScriptGroup + 'static,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let map_path = chunky.source.clone();
    if chunky.body_offset >= chunky.bytes.len() {
        return Err(GameLogicError::Configuration(format!(
            "Map '{}' chunk table extends past file",
            map_path.display()
        )));
    }

    let body = &chunky.bytes[chunky.body_offset..];
    let player_scripts_chunk = find_chunk_by_label(body, &chunky.toc, PLAYER_SCRIPTS_LABEL)?;
    let mut scripts = ScriptListReadInfo::with_templates(templates, new_group);
    let Some((version, payload)) = player_scripts_chunk else {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc, names, scripts)? {
            return Ok(Some(result));
        } else {
            debug!(
                "Map '{}' does not contain a '{}' chunk; no mission scripts available",
                map_path.display(),
                PLAYER_SCRIPTS_LABEL
            );
            return Ok(Some(MapScriptLoadResult {
                source_path: map_path,
                script_lists: Vec::new(),
                total_scripts: 0,
            }));
        }
    };

    if version == 0 {
        warn!("PlayerScriptsList chunk reported version 0; continuing");
    }
    scripts.decode(payload, &chunky.toc, &mut |name| names.name_to_key(name))?;
    let script_lists: Vec<_> = scripts.lists.drain(..).map(|list| *list).collect();
    let total = count_scripts(&script_lists);

    if total == 0 {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc, names, scripts)? {
            info!(
                "PlayerScriptsList in '{}' decoded empty; using SidesList fallback instead",
                map_path.display()
            );
            return Ok(Some(result));
        }
    }

    info!(
        "Decoded {} script lists ({} scripts) from '{}'",
        script_lists.len(),
        total,
        map_path.display()
    );

    Ok(Some(MapScriptLoadResult {
        source_path: map_path,
        script_lists,
        total_scripts: total,
    }))
}

// -------------------------------------------------------------------------------------------------
// Script parsing
// -------------------------------------------------------------------------------------------------

fn synthesize_chunk_stream(
    toc: &HashMap<u32, String>,
    root_label: &str,
    version: u16,
    payload: &[u8],
) -> LoaderResult<Vec<u8>> {
    let Some((&root_id, _)) = toc.iter().find(|(_, name)| name.as_str() == root_label) else {
        return Err(configuration_error(format!(
            "Chunk table does not contain '{}'",
            root_label
        )));
    };

    let mut mappings: Vec<(&u32, &String)> = toc.iter().collect();
    mappings.sort_by_key(|(id, _)| **id);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(CHUNK_MAGIC);
    bytes.extend_from_slice(&(mappings.len() as i32).to_le_bytes());
    for (id, name) in mappings {
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u8::MAX as usize {
            return Err(configuration_error(format!(
                "Chunk label '{}' is too long for synthetic chunk stream",
                name
            )));
        }
        bytes.push(name_bytes.len() as u8);
        bytes.extend_from_slice(name_bytes);
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes.extend_from_slice(&root_id.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn parse_sides_chunk_for_scripts_only(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(scripts) = user_data.downcast_mut::<ScriptListReadInfo>() else {
        return false;
    };

    let count = input.read_int().max(0) as usize;
    for _side_index in 0..count {
        let _dict = input.read_dict();
        let build_count = input.read_int().max(0) as usize;
        for _ in 0..build_count {
            let _building_name = input.read_ascii_string();
            let _template_name = input.read_ascii_string();
            let _x = input.read_real();
            let _y = input.read_real();
            let _z = input.read_real();
            let _angle = input.read_real();
            let _initially_built = input.read_byte();
            let _num_rebuilds = input.read_int();
            if info.version >= 3 {
                let _script_name = input.read_ascii_string();
                let _health = input.read_int();
                let _whiner = input.read_byte();
                let _unsellable = input.read_byte();
                let _repairable = input.read_byte();
            }
        }
    }

    if info.version >= 2 {
        let team_count = input.read_int().max(0) as usize;
        for _ in 0..team_count {
            let _dict = input.read_dict();
        }
    }

    input.register_parser(
        PLAYER_SCRIPTS_LABEL,
        &info.label,
        parse_player_scripts_list_chunk,
    );
    if !input.parse(scripts) {
        return false;
    }

    input.at_end_of_chunk()
}

fn parse_script_lists_from_sides_chunk(
    payload: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
    names: &mut NameKeyNamespace,
    mut scripts: ScriptListReadInfo,
) -> LoaderResult<Vec<ScriptList>> {
    let chunk_stream = synthesize_chunk_stream(toc, SIDES_LIST_LABEL, version, payload)?;
    let mut input = DataChunkInput::with_namespace(chunk_stream, names);
    if !input.is_valid_file_type() {
        return Err(configuration_error(
            "Synthetic SidesList chunk stream is not valid",
        ));
    }

    input.register_parser(SIDES_LIST_LABEL, "", parse_sides_chunk_for_scripts_only);
    if !input.parse(&mut scripts) {
        return Err(configuration_error(
            "Failed to parse PlayerScriptsList from SidesList chunk",
        ));
    }

    let lists: Vec<ScriptList> = scripts.lists
        .into_iter()
        .map(|list| *list)
        .collect();
    if lists.is_empty() {
        warn!(
            "SidesList fallback decoded without any ScriptList children (payload={} bytes, version={})",
            payload.len(),
            version
        );
    }
    Ok(lists)
}

fn count_scripts(lists: &[ScriptList]) -> usize {
    fn count_chain(mut script: Option<&Box<Script>>) -> usize {
        let mut total = 0;
        while let Some(node) = script {
            total += 1;
            script = node.next_script.as_ref();
        }
        total
    }

    let mut total = 0;
    for list in lists {
        total += count_chain(list.first_script.as_ref());
        let mut group = list.first_group.as_ref();
        while let Some(node) = group {
            total += count_chain(node.first_script.as_ref());
            group = node.next_group.as_ref();
        }
    }
    total
}

fn configuration_error(message: impl Into<String>) -> GameLogicError {
    GameLogicError::Configuration(message.into())
}
