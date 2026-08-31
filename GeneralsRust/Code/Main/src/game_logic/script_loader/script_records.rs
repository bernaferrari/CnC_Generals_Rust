// C++ ownership: ScriptEngine ScriptList/Script/OrCondition/Condition/ScriptAction decoding and linking.

const PLAYER_SCRIPTS_LABEL: &str = "PlayerScriptsList";
const SCRIPT_LIST_LABEL: &str = "ScriptList";
const SCRIPT_LABEL: &str = "Script";
const SCRIPT_GROUP_LABEL: &str = "ScriptGroup";
const OR_CONDITION_LABEL: &str = "OrCondition";
const CONDITION_LABEL: &str = "Condition";
const SCRIPT_ACTION_LABEL: &str = "ScriptAction";
const SCRIPT_ACTION_FALSE_LABEL: &str = "ScriptActionFalse";

#[derive(Default)]
struct SidesScriptContext {
    scripts: ScriptListReadInfo,
}

fn load_sides_list_fallback(
    map_path: &Path,
    body: &[u8],
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let Some((sides_version, sides_payload)) = find_chunk_by_label(body, toc, SIDES_LIST_LABEL)?
    else {
        return Ok(None);
    };

    let script_lists = parse_script_lists_from_sides_chunk(sides_payload, toc, sides_version)?;
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
    let Some(map_path) = locate_map_file(map_name) else {
        warn!(
            "No .map file could be found for '{}'; mission scripts unavailable",
            map_name
        );
        return Ok(None);
    };
    if let Some(chunky) = cached_chunky_for(&map_path) {
        return load_map_scripts_from_chunky(&chunky);
    }
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    load_map_scripts_from_chunky(&chunky)
}

/// Decode mission scripts from an already-decompressed chunky map.
/// C++ `TerrainLogic::loadMap` (`TerrainLogic.cpp:1248-1262`) opens the
/// `.map` once via `CachedFileInputStream` and parses chunks from that stream.
pub fn load_map_scripts_from_chunky(
    chunky: &ChunkyMap,
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
    let Some((version, payload)) = player_scripts_chunk else {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc)? {
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

    let script_lists = parse_script_lists(payload, &chunky.toc, version)?;
    let total = count_scripts(&script_lists);

    if total == 0 {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc)? {
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
    let Some(context) = user_data.downcast_mut::<SidesScriptContext>() else {
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
    if !input.parse(&mut context.scripts) {
        return false;
    }

    input.at_end_of_chunk()
}

fn parse_script_lists_from_sides_chunk(
    payload: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<Vec<ScriptList>> {
    let chunk_stream = synthesize_chunk_stream(toc, SIDES_LIST_LABEL, version, payload)?;
    let mut input = DataChunkInput::new(chunk_stream);
    if !input.is_valid_file_type() {
        return Err(configuration_error(
            "Synthetic SidesList chunk stream is not valid",
        ));
    }

    let mut context = SidesScriptContext::default();
    input.register_parser(SIDES_LIST_LABEL, "", parse_sides_chunk_for_scripts_only);
    if !input.parse(&mut context) {
        return Err(configuration_error(
            "Failed to parse PlayerScriptsList from SidesList chunk",
        ));
    }

    let lists: Vec<ScriptList> = context
        .scripts
        .lists
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

fn parse_script_lists(
    data: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<Vec<ScriptList>> {
    if version == 0 {
        warn!("PlayerScriptsList chunk reported version 0; continuing");
    }
    let mut lists = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        if label == SCRIPT_LIST_LABEL {
            lists.push(parse_script_list(payload, toc, chunk_version)?);
        } else {
            debug!(
                "Skipping unexpected chunk '{}' under PlayerScriptsList",
                label
            );
        }
        Ok(())
    })?;
    if lists.is_empty() {
        warn!(
            "PlayerScriptsList chunk decoded without any ScriptList children (payload={} bytes, version={})",
            data.len(),
            version
        );
    }
    Ok(lists)
}

fn parse_script_list(
    data: &[u8],
    toc: &HashMap<u32, String>,
    _version: u16,
) -> LoaderResult<ScriptList> {
    let mut top_scripts = Vec::new();
    let mut groups = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        match label {
            SCRIPT_LABEL => top_scripts.push(parse_script(payload, toc, chunk_version)?),
            SCRIPT_GROUP_LABEL => groups.push(parse_script_group(payload, toc, chunk_version)?),
            _ => debug!("Unknown chunk '{}' inside ScriptList", label),
        }
        Ok(())
    })?;

    let mut list = ScriptList::new();
    list.first_script = link_scripts(top_scripts);
    list.first_group = link_script_groups(groups);
    Ok(list)
}

fn parse_script(data: &[u8], toc: &HashMap<u32, String>, version: u16) -> LoaderResult<Script> {
    let mut reader = BinaryReader::new(data);
    let mut script = Script::new();
    script.script_name = reader.read_ascii_string()?;
    script.comment = reader.read_ascii_string()?;
    script.condition_comment = reader.read_ascii_string()?;
    script.action_comment = reader.read_ascii_string()?;
    script.is_active = reader.read_u8()? != 0;
    script.is_one_shot = reader.read_u8()? != 0;
    script.easy = reader.read_u8()? != 0;
    script.normal = reader.read_u8()? != 0;
    script.hard = reader.read_u8()? != 0;
    script.is_subroutine = reader.read_u8()? != 0;
    if version >= 2 {
        script.delay_evaluation_seconds = reader.read_i32()?;
    }

    let nested = reader.take_remaining();
    let mut or_nodes = Vec::new();
    let mut actions = Vec::new();
    let mut false_actions = Vec::new();
    parse_chunk_sequence(nested, toc, |label, chunk_version, payload| {
        match label {
            OR_CONDITION_LABEL => or_nodes.push(parse_or_condition(payload, toc, chunk_version)?),
            SCRIPT_ACTION_LABEL => actions.push(parse_script_action(payload, chunk_version)?),
            SCRIPT_ACTION_FALSE_LABEL => {
                false_actions.push(parse_script_action(payload, chunk_version)?)
            }
            _ => debug!("Unhandled chunk '{}' inside Script", label),
        }
        Ok(())
    })?;

    script.condition = link_or_conditions(or_nodes);
    script.action = link_actions(actions);
    script.action_false = link_actions(false_actions);
    Ok(script)
}

fn parse_script_group(
    data: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<ScriptGroup> {
    let mut reader = BinaryReader::new(data);
    let mut group = ScriptGroup::new();
    group.group_name = reader.read_ascii_string()?;
    group.is_group_active = reader.read_u8()? != 0;
    group.is_group_subroutine = if version >= 2 {
        reader.read_u8()? != 0
    } else {
        false
    };

    let nested = reader.take_remaining();
    let mut scripts = Vec::new();
    parse_chunk_sequence(nested, toc, |label, chunk_version, payload| {
        if label == SCRIPT_LABEL {
            scripts.push(parse_script(payload, toc, chunk_version)?);
        } else {
            debug!("Skipping '{}' inside ScriptGroup", label);
        }
        Ok(())
    })?;
    group.first_script = link_scripts(scripts);
    Ok(group)
}

fn parse_or_condition(
    data: &[u8],
    toc: &HashMap<u32, String>,
    _version: u16,
) -> LoaderResult<OrCondition> {
    let mut or_node = OrCondition::new();
    let mut conditions = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        if label == CONDITION_LABEL {
            conditions.push(parse_condition(payload, chunk_version)?);
        } else {
            debug!("Unknown chunk '{}' inside OrCondition", label);
        }
        Ok(())
    })?;
    or_node.first_and = link_conditions(conditions);
    Ok(or_node)
}

fn parse_condition(data: &[u8], version: u16) -> LoaderResult<Condition> {
    let mut reader = BinaryReader::new(data);
    let cond_value = reader.read_i32()? as u32;
    let mut cond_type = convert_condition_type(cond_value)?;
    let mut condition = Condition::new(cond_type);
    if version >= 4 {
        let name_key = reader.read_u32()?;
        let mut matched = false;
        if let Ok(engine_guard) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_condition_template(cond_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(resolved) = engine.find_condition_type_by_name_key(name_key) {
                        cond_type = resolved;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            cond_type = ConditionType::ConditionFalse;
        }
        condition.condition_type = cond_type;
    }
    let param_count = reader.read_i32()? as usize;
    for _ in 0..param_count {
        let param = parse_parameter(&mut reader)?;
        append_parameter(&mut condition.parameters, &mut condition.num_parms, param)?;
    }
    Ok(condition)
}

fn parse_script_action(data: &[u8], version: u16) -> LoaderResult<ScriptAction> {
    let mut reader = BinaryReader::new(data);
    let mut action_type = convert_action_type(reader.read_i32()? as u32)?;
    let mut action = ScriptAction::new(action_type);
    if version >= 2 {
        let name_key = reader.read_u32()?;
        let mut matched = false;
        if let Ok(engine_guard) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_action_template(action_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(resolved) = engine.find_action_type_by_name_key(name_key) {
                        action_type = resolved;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            action_type = ScriptActionType::NoOp;
        }
        action.action_type = action_type;
    }
    let param_count = reader.read_i32()? as usize;
    for _ in 0..param_count {
        let param = parse_parameter(&mut reader)?;
        append_parameter(&mut action.parameters, &mut action.num_parms, param)?;
    }
    Ok(action)
}

fn parse_parameter(reader: &mut BinaryReader<'_>) -> LoaderResult<Parameter> {
    let kind = convert_parameter_type(reader.read_i32()? as u32)?;
    let mut param = Parameter::new(kind);
    param.initialized = true;
    if kind == ParameterType::Coord3D {
        let coord = Coord3D::new(reader.read_f32()?, reader.read_f32()?, reader.read_f32()?);
        param.coord_value = coord;
    } else {
        param.int_value = reader.read_i32()?;
        param.real_value = reader.read_f32()?;
        param.string_value = reader.read_ascii_string()?;
    }
    Ok(param)
}

fn append_parameter(
    slots: &mut [Option<Parameter>],
    count: &mut usize,
    parameter: Parameter,
) -> LoaderResult<()> {
    if *count >= slots.len() {
        return Err(configuration_error(
            "Script parameter count exceeded maximum capacity",
        ));
    }
    slots[*count] = Some(parameter);
    *count += 1;
    Ok(())
}

fn link_scripts(mut scripts: Vec<Script>) -> Option<Box<Script>> {
    let mut next = None;
    while let Some(mut script) = scripts.pop() {
        script.next_script = next;
        next = Some(Box::new(script));
    }
    next
}

fn link_script_groups(mut groups: Vec<ScriptGroup>) -> Option<Box<ScriptGroup>> {
    let mut next = None;
    while let Some(mut group) = groups.pop() {
        group.next_group = next;
        next = Some(Box::new(group));
    }
    next
}

fn link_or_conditions(mut nodes: Vec<OrCondition>) -> Option<Box<OrCondition>> {
    let mut next = None;
    while let Some(mut node) = nodes.pop() {
        node.next_or = next;
        next = Some(Box::new(node));
    }
    next
}

fn link_conditions(mut conditions: Vec<Condition>) -> Option<Box<Condition>> {
    let mut next = None;
    while let Some(mut cond) = conditions.pop() {
        cond.next_and_condition = next;
        next = Some(Box::new(cond));
    }
    next
}

fn link_actions(mut actions: Vec<ScriptAction>) -> Option<Box<ScriptAction>> {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(Box::new(action));
    }
    next
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

// -------------------------------------------------------------------------------------------------
// Enum conversion helpers
// -------------------------------------------------------------------------------------------------

fn convert_parameter_type(value: u32) -> LoaderResult<ParameterType> {
    ParameterType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ParameterType value {}", value)))
}

fn convert_condition_type(value: u32) -> LoaderResult<ConditionType> {
    ConditionType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ConditionType value {}", value)))
}

fn convert_action_type(value: u32) -> LoaderResult<ScriptActionType> {
    ScriptActionType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ScriptActionType value {}", value)))
}

fn configuration_error(message: impl Into<String>) -> GameLogicError {
    GameLogicError::Configuration(message.into())
}
