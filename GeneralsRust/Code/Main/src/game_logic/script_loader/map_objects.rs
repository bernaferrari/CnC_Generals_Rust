// C++ ownership: MapObject/SidesList objects, teams/players, waypoints, bridges, and roads.

const OBJECTS_LIST_LABEL: &str = "ObjectsList";
const OBJECT_CREATION_LIST_LABEL: &str = "ObjectCreationList";
const OBJECT_LABEL: &str = "Object";
const SIDES_LIST_LABEL: &str = "SidesList";
const FLAG_ROAD_POINT1: i32 = 0x00000002;
const FLAG_ROAD_POINT2: i32 = 0x00000004;
const FLAG_ROAD_CORNER_ANGLED: i32 = 0x00000008;
const FLAG_BRIDGE_POINT1: i32 = 0x00000010;
const FLAG_BRIDGE_POINT2: i32 = 0x00000020;
const FLAG_ROAD_CORNER_TIGHT: i32 = 0x00000040;
const FLAG_ROAD_JOIN: i32 = 0x00000080;
const DEFAULT_RUNTIME_ROAD_WIDTH: f32 = 8.0;
const DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE: f32 = 1.0;
const DEFAULT_RUNTIME_ROAD_UNIQUE_ID: u32 = 1;
const CORNER_RADIUS: f32 = 1.5;
const TIGHT_CORNER_RADIUS: f32 = 0.5;

pub fn parse_runtime_waypoints_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<(Vec<RuntimeWaypoint>, Vec<(u32, u32)>)> {
    let body = &chunky.bytes[chunky.body_offset..];
    let mut waypoints = Vec::new();
    let mut links = Vec::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(waypoint) =
                parse_waypoint_object_chunk(data, child_version.max(version), &chunky.toc)?
            {
                waypoints.push(waypoint);
            }
            Ok(())
        })?;
    }

    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "WaypointsList")? {
        let mut reader = BinaryReader::new(payload);
        let count = reader.read_i32()?.max(0) as usize;
        for _ in 0..count {
            if reader.remaining() < 8 {
                break;
            }
            let id1 = reader.read_i32()? as u32;
            let id2 = reader.read_i32()? as u32;
            links.push((id1, id2));
        }
    }

    Ok((waypoints, links))
}

pub fn parse_runtime_bridges_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<BridgeData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let mut bridges = Vec::new();
    let mut pending: HashMap<String, Vec<PendingRuntimeBridge>> = HashMap::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(endpoint) =
                parse_bridge_endpoint_object_chunk(data, child_version.max(version))?
            {
                add_runtime_bridge_point(&mut bridges, &mut pending, endpoint);
            }
            Ok(())
        })?;
    }

    Ok(bridges)
}

/// Parse runtime terrain-road segments from map objects.
///
/// This mirrors C++ `W3DRoadBuffer::addMapObjects` pairing semantics:
/// only `ROAD_POINT1` objects whose immediate next object is `ROAD_POINT2`
/// produce a segment.
pub fn parse_runtime_roads_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<RuntimeRoadSegment>> {
    ensure_terrain_roads_loaded();

    let body = &chunky.bytes[chunky.body_offset..];
    let mut objects = Vec::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(map_object) =
                parse_runtime_map_object_stub_chunk(data, child_version.max(version))?
            {
                objects.push(map_object);
            }
            Ok(())
        })?;
    }

    let mut roads = Vec::new();
    let mut index = 0usize;
    while index < objects.len() {
        let current = &objects[index];
        if (current.flags & FLAG_ROAD_POINT1) == 0 {
            index += 1;
            continue;
        }

        let Some(next) = objects.get(index + 1) else {
            break;
        };
        if (next.flags & FLAG_ROAD_POINT2) == 0 {
            index += 1;
            continue;
        }

        roads.push(build_runtime_road_data(
            current.template_name.as_str(),
            current.location,
            next.location,
            current.flags,
            next.flags,
        ));
        index += 2;
    }

    Ok(roads)
}

pub fn parse_runtime_sides_from_chunky(chunky: &ChunkyMap) -> LoaderResult<RuntimeSidesData> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, SIDES_LIST_LABEL)? else {
        return Ok(RuntimeSidesData::default());
    };

    let mut reader = BinaryReader::new(payload);
    let mut side_dicts = Vec::new();
    let mut team_dicts = Vec::new();

    let mut side_builds = Vec::new();
    let side_count = reader.read_i32()?.max(0) as usize;
    for side_index in 0..side_count {
        side_dicts.push(parse_chunk_dict_typed(&mut reader, &chunky.toc)?);
        let build_count = reader.read_i32()?.max(0) as usize;
        for _ in 0..build_count {
            side_builds.push(parse_side_build_entry(
                &mut reader,
                version,
                side_index as u32,
            )?);
        }
    }

    if version >= 2 {
        let team_count = reader.read_i32()?.max(0) as usize;
        for _ in 0..team_count {
            team_dicts.push(parse_chunk_dict_typed(&mut reader, &chunky.toc)?);
        }
    }

    Ok(RuntimeSidesData {
        side_dicts,
        team_dicts,
        side_builds,
    })
}

/// Parse placed objects from a chunky map. Currently supports a minimal subset
/// of the ObjectCreationList chunk (template, position, rotation, team).

/// Wave 831: parse SidesList build-list entries for skirmish faction bases.

/// Wave 831: Player_N_Start / Player_N_Rally waypoints (1-based N).
pub fn parse_player_start_waypoints(
    map_name: &str,
) -> LoaderResult<Vec<(u32, Coord3D, Option<Coord3D>)>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_player_start_waypoints_from_chunky(&chunky)
}

pub fn parse_player_start_waypoints_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<(u32, Coord3D, Option<Coord3D>)>> {
    let (waypoints, _links) = parse_runtime_waypoints_from_chunky(chunky)?;
    let mut starts: std::collections::HashMap<u32, Coord3D> = std::collections::HashMap::new();
    let mut rallies: std::collections::HashMap<u32, Coord3D> = std::collections::HashMap::new();
    for wp in waypoints {
        let name = wp.name.trim();
        let lower = name.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("player_") {
            if let Some((num, kind)) = rest.split_once('_') {
                if let Ok(idx1) = num.parse::<u32>() {
                    if idx1 >= 1 {
                        let idx0 = idx1 - 1;
                        if kind.starts_with("start") {
                            starts.insert(idx0, wp.location);
                        } else if kind.starts_with("rally") {
                            rallies.insert(idx0, wp.location);
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut keys: Vec<u32> = starts.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        out.push((k, starts[&k], rallies.get(&k).copied()));
    }
    Ok(out)
}

pub fn parse_side_build_list(map_name: &str) -> LoaderResult<Vec<SideBuildEntry>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_side_build_list_from_chunky(&chunky)
}

pub fn parse_side_build_list_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<SideBuildEntry>> {
    let sides = parse_runtime_sides_from_chunky(chunky)?;
    Ok(sides.side_builds)
}

pub fn parse_object_placements(map_name: &str) -> LoaderResult<Vec<PlacedObject>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_object_placements_from_chunky(&chunky)
}

pub fn parse_object_placements_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<PlacedObject>> {
    ensure_terrain_roads_loaded();
    let body = &chunky.bytes[chunky.body_offset..];
    let map_name = chunky.source.display().to_string();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        let mut objects = Vec::new();
        let mut labels_seen: HashMap<String, usize> = HashMap::new();
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            *labels_seen.entry(label.to_string()).or_insert(0) += 1;
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(obj) = parse_map_object_chunk(data, version, &chunky.toc)? {
                objects.push(obj);
            }
            Ok(())
        })?;
        if objects.is_empty() {
            debug!(
                "Map '{}' ObjectsList parsed with no placements; subchunk histogram: {:?}",
                map_name, labels_seen
            );
        }
        return Ok(objects);
    }

    if let Some((version, payload)) =
        find_chunk_by_label(body, &chunky.toc, OBJECT_CREATION_LIST_LABEL)?
    {
        let mut objects = Vec::new();
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(obj) = parse_object_creation_chunk(data, version)? {
                objects.push(obj);
            }
            Ok(())
        })?;
        return Ok(objects);
    }

    warn!(
        "Map '{}' has neither '{}' nor '{}' chunks; skipping object placements",
        map_name, OBJECTS_LIST_LABEL, OBJECT_CREATION_LIST_LABEL
    );
    Ok(Vec::new())
}

fn parse_initial_camera_position(map_name: &str) -> LoaderResult<Option<Coord3D>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    parse_initial_camera_position_from_chunky(&chunky)
}

fn parse_initial_camera_position_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<Coord3D>> {
    let body = &chunky.bytes[chunky.body_offset..];

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        let mut result = None;
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if result.is_some() || label != OBJECT_LABEL {
                return Ok(());
            }
            result = parse_camera_waypoint_chunk(data, version, &chunky.toc)?;
            Ok(())
        })?;
        if result.is_some() {
            return Ok(result);
        }
    }

    if let Some((version, payload)) =
        find_chunk_by_label(body, &chunky.toc, OBJECT_CREATION_LIST_LABEL)?
    {
        let mut result = None;
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if result.is_some() || label != OBJECT_LABEL {
                return Ok(());
            }
            result = parse_camera_waypoint_chunk(data, version, &chunky.toc)?;
            Ok(())
        })?;
        if result.is_some() {
            return Ok(result);
        }
    }

    Ok(None)
}

fn parse_camera_waypoint_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<Coord3D>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;

    if version < 2 || reader.remaining() == 0 {
        if template_name.eq_ignore_ascii_case("InitialCameraPosition") {
            return Ok(Some(Coord3D::new(x, y, z)));
        }
        return Ok(None);
    }

    let dict = parse_chunk_dict(&mut reader, toc)?;
    if !dict_contains_key(&dict, "waypointID") {
        return Ok(None);
    }

    let waypoint_name = dict_lookup_ci(&dict, &["waypointName"])
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(template_name);
    if waypoint_name.eq_ignore_ascii_case("InitialCameraPosition") {
        return Ok(Some(Coord3D::new(x, y, z)));
    }

    Ok(None)
}

fn parse_map_object_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<PlacedObject>> {
    // Mirrors C++ ParseObjectDataChunk: x/y/z, angle, flags, template name, dict (v2+).
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template = reader.read_ascii_string()?;
    if template.is_empty() {
        return Ok(None);
    }
    if is_terrain_road_name(&template) {
        return Ok(None);
    }

    let mut name = None;
    let mut team_name = None;
    let mut player_id = None;
    let mut upgrade = None;
    let mut unsellable = None;
    let mut enabled = None;
    let mut powered = None;
    let mut indestructible = None;

    let mut object_weather = None;
    let mut properties = Dict::new();

    if version >= 2 && reader.remaining() > 0 {
        properties = parse_chunk_dict_typed(&mut reader, toc)?;
        let dict = dict_to_string_map(&properties);

        // Waypoints are map metadata nodes, not spawnable world objects.
        if dict_contains_key(&dict, "waypointID") {
            return Ok(None);
        }

        team_name = dict_lookup_ci(
            &dict,
            &["teamName", "team", "originalOwner", "owner", "playerName"],
        )
        .filter(|value| !value.trim().is_empty());

        name = dict_lookup_ci(
            &dict,
            &["objectName", "scriptName", "unitName", "thingName", "name"],
        )
        .filter(|value| !value.trim().is_empty());

        player_id = dict_lookup_ci(
            &dict,
            &[
                "player",
                "playerId",
                "playerID",
                "ownerPlayer",
                "multiplayerStartIndex",
                "originalOwner",
            ],
        )
        .and_then(|value| parse_player_id(&value));

        upgrade = dict_lookup_ci(
            &dict,
            &[
                "upgrade",
                "upgrades",
                "upgradeList",
                "startupUpgrade",
                "startupUpgrades",
            ],
        )
        .filter(|value| !value.trim().is_empty());

        unsellable = dict_lookup_ci(
            &dict,
            &["objectUnsellable", "unsellable", "object_unsellable"],
        )
        .map(|value| parse_ini_boolish(&value));

        enabled = dict_lookup_ci(&dict, &["objectEnabled", "enabled", "object_enabled"])
            .map(|value| parse_ini_boolish(&value));

        powered = dict_lookup_ci(&dict, &["objectPowered", "powered", "object_powered"])
            .map(|value| parse_ini_boolish(&value));

        indestructible = dict_lookup_ci(
            &dict,
            &[
                "objectIndestructible",
                "indestructible",
                "object_indestructible",
            ],
        )
        .map(|value| parse_ini_boolish(&value));

        object_weather = dict_lookup_ci(&dict, &["objectWeather", "object_weather"])
            .and_then(|value| parse_object_weather_value(&value));
    }

    Ok(Some(PlacedObject {
        template,
        name,
        position: Coord3D::new(x, y, z),
        rotation: Some(angle),
        team_name,
        player_id,
        upgrade,
        unsellable,
        enabled,
        powered,
        indestructible,

        object_weather,
        properties,
    }))
}

fn parse_waypoint_object_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<RuntimeWaypoint>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;
    if version < 2 || reader.remaining() == 0 {
        return Ok(None);
    }

    let dict = parse_chunk_dict(&mut reader, toc)?;
    if !dict_contains_key(&dict, "waypointID") {
        return Ok(None);
    }

    let waypoint_id = dict_lookup_ci(&dict, &["waypointID"])
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0)
        .max(0) as u32;
    let waypoint_name = dict_lookup_ci(&dict, &["waypointName"]).unwrap_or_default();
    let resolved_name = if waypoint_name.trim().is_empty() {
        template_name
    } else {
        waypoint_name
    };

    Ok(Some(RuntimeWaypoint {
        id: waypoint_id,
        name: resolved_name,
        location: Coord3D::new(x, y, z),
        path_label1: dict_lookup_ci(&dict, &["waypointPathLabel1"]).unwrap_or_default(),
        path_label2: dict_lookup_ci(&dict, &["waypointPathLabel2"]).unwrap_or_default(),
        path_label3: dict_lookup_ci(&dict, &["waypointPathLabel3"]).unwrap_or_default(),
        bi_directional: dict_lookup_ci(&dict, &["waypointPathBiDirectional"])
            .map(|value| {
                let trimmed = value.trim();
                trimmed.eq_ignore_ascii_case("true")
                    || trimmed.eq_ignore_ascii_case("yes")
                    || trimmed == "1"
            })
            .unwrap_or(false),
    }))
}

fn parse_bridge_endpoint_object_chunk(
    data: &[u8],
    version: u16,
) -> LoaderResult<Option<RuntimeBridgeEndpoint>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;
    if template_name.trim().is_empty() {
        return Ok(None);
    }

    if (flags & (FLAG_BRIDGE_POINT1 | FLAG_BRIDGE_POINT2)) == 0 {
        return Ok(None);
    }

    let point = Coord3D::new(x, y, z);
    if (flags & FLAG_BRIDGE_POINT1) != 0 {
        return Ok(Some(RuntimeBridgeEndpoint {
            template_name,
            location: point,
            is_point1: true,
        }));
    }

    Ok(Some(RuntimeBridgeEndpoint {
        template_name,
        location: point,
        is_point1: false,
    }))
}

fn parse_runtime_map_object_stub_chunk(
    data: &[u8],
    version: u16,
) -> LoaderResult<Option<RuntimeMapObjectStub>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;

    Ok(Some(RuntimeMapObjectStub {
        template_name,
        location: Coord3D::new(x, y, z),
        flags,
    }))
}

fn add_runtime_bridge_point(
    bridges: &mut Vec<BridgeData>,
    pending: &mut HashMap<String, Vec<PendingRuntimeBridge>>,
    endpoint: RuntimeBridgeEndpoint,
) {
    let entry = pending.entry(endpoint.template_name.clone()).or_default();

    if endpoint.is_point1 {
        for index in 0..entry.len() {
            if entry[index].from.is_none() && entry[index].to.is_some() {
                let to = entry[index].to.take().unwrap_or(endpoint.location);
                let from = endpoint.location;
                entry.swap_remove(index);
                bridges.push(build_runtime_bridge_data(
                    endpoint.template_name.as_str(),
                    from,
                    to,
                ));
                return;
            }
        }
        entry.push(PendingRuntimeBridge {
            from: Some(endpoint.location),
            to: None,
        });
    } else {
        for index in 0..entry.len() {
            if entry[index].to.is_none() && entry[index].from.is_some() {
                let from = entry[index].from.take().unwrap_or(endpoint.location);
                let to = endpoint.location;
                entry.swap_remove(index);
                bridges.push(build_runtime_bridge_data(
                    endpoint.template_name.as_str(),
                    from,
                    to,
                ));
                return;
            }
        }
        entry.push(PendingRuntimeBridge {
            from: None,
            to: Some(endpoint.location),
        });
    }
}

fn build_runtime_bridge_data(template_name: &str, from: Coord3D, to: Coord3D) -> BridgeData {
    let width = runtime_bridge_width_from_template(template_name).unwrap_or(MAP_XY_FACTOR * 2.0);
    BridgeData::new(
        SystemCoord3D::new(from.x, from.y, from.z),
        SystemCoord3D::new(to.x, to.y, to.z),
        width,
        template_name.to_string(),
    )
}

fn runtime_bridge_width_from_template(template_name: &str) -> Option<f32> {
    // Do not call TheThingFactory::find_template here: that helper lazy-inits
    // the entire Object INI database (14s+ on Lone Eagle). C++ already has
    // TheThingFactory from GameEngine::init; if the host factory is empty or
    // still held by the abandoned boot worker, use the default bridge width.
    let factory_guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = factory_guard.as_ref()?;
    let template = factory.find_template(template_name, false)?;
    let geometry = template.get_template_geometry_info();
    let width = (geometry.minor_radius() * 2.0).max(0.0);
    if width > 0.0 { Some(width) } else { None }
}

fn build_runtime_road_data(
    template_name: &str,
    from: Coord3D,
    mut to: Coord3D,
    from_flags: i32,
    to_flags: i32,
) -> RuntimeRoadSegment {
    if (from.x - to.x).abs() <= f32::EPSILON && (from.y - to.y).abs() <= f32::EPSILON {
        to.x += 0.25;
    }

    let (width, width_in_texture, road_type_id) = runtime_road_style_for_template(template_name);
    RuntimeRoadSegment {
        template_name: template_name.to_string(),
        from,
        to,
        width,
        width_in_texture,
        road_type_id,
        start_is_angled: (from_flags & FLAG_ROAD_CORNER_ANGLED) != 0,
        start_is_join: (from_flags & FLAG_ROAD_JOIN) != 0,
        end_is_angled: (to_flags & FLAG_ROAD_CORNER_ANGLED) != 0,
        end_is_join: (to_flags & FLAG_ROAD_JOIN) != 0,
        curve_radius: if (from_flags & FLAG_ROAD_CORNER_TIGHT) != 0 {
            TIGHT_CORNER_RADIUS
        } else {
            CORNER_RADIUS
        },
    }
}

fn runtime_road_style_for_template(template_name: &str) -> (f32, f32, u32) {
    if let Some(roads) = try_get_terrain_roads() {
        if let Some(road) = roads.find_road(template_name) {
            let width = if road.road_width > 0.0 {
                road.road_width
            } else {
                DEFAULT_RUNTIME_ROAD_WIDTH
            };
            let width_in_texture = if road.road_width_in_texture > 0.0 {
                road.road_width_in_texture
            } else {
                DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE
            };
            return (width, width_in_texture, road.id);
        }
    }

    (
        DEFAULT_RUNTIME_ROAD_WIDTH,
        DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE,
        DEFAULT_RUNTIME_ROAD_UNIQUE_ID,
    )
}

fn parse_object_creation_chunk(data: &[u8], _version: u16) -> LoaderResult<Option<PlacedObject>> {
    // This is a partial parser: many fields omitted, only template/team/position are read.
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 24 {
        return Ok(None);
    }

    // Template name (null-terminated string length-prefixed by u8)
    let name_len = reader.read_u8()? as usize;
    let name_bytes = reader.read_bytes(name_len)?;
    let template = String::from_utf8_lossy(name_bytes).to_string();
    if template.is_empty() || is_terrain_road_name(&template) {
        return Ok(None);
    }

    // Position (f32 x3)
    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let z = reader.read_f32()?;
    let position = Coord3D::new(x, y, z);

    // Rotation (yaw). Some maps store it as a single f32 after position.
    let rotation = if reader.remaining() >= 4 {
        Some(reader.read_f32()?)
    } else {
        None
    };

    // Team name (length-prefixed u8 string)
    let team_name = if reader.remaining() >= 1 {
        let len = reader.read_u8()? as usize;
        if len > 0 && reader.remaining() >= len {
            let bytes = reader.read_bytes(len)?;
            Some(String::from_utf8_lossy(bytes).to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Player ID (optional). Some builds store it as u8 after team.
    let player_id = if reader.remaining() >= 1 {
        Some(reader.read_u8()? as u32)
    } else {
        None
    };

    // Optional upgrade/facing string (length-prefixed u8). Treat as upgrade tag for now.
    let upgrade = if reader.remaining() >= 1 {
        let len = reader.read_u8()? as usize;
        if len > 0 && reader.remaining() >= len {
            let bytes = reader.read_bytes(len)?;
            Some(String::from_utf8_lossy(bytes).to_string())
        } else {
            None
        }
    } else {
        None
    };

    Ok(Some(PlacedObject {
        template,
        name: None,
        position,
        rotation,
        team_name,
        player_id,
        upgrade,
        unsellable: None,
        enabled: None,
        powered: None,
        indestructible: None,

        object_weather: None,
        properties: Dict::new(),
    }))
}

fn parse_side_build_entry(
    reader: &mut BinaryReader<'_>,
    version: u16,
    side_index: u32,
) -> LoaderResult<SideBuildEntry> {
    let building_name = reader.read_ascii_string()?;
    let template = reader.read_ascii_string()?;
    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let z = reader.read_f32()?;
    let angle = reader.read_f32()?;
    let initially_built = reader.read_u8()? != 0;
    let num_rebuilds = reader.read_i32()?;

    let mut script_name = None;
    let mut health = None;
    let mut whiner = None;
    let mut unsellable = None;
    let mut repairable = None;
    if version >= 3 {
        let s = reader.read_ascii_string()?;
        if !s.is_empty() {
            script_name = Some(s);
        }
        health = Some(reader.read_i32()?);
        whiner = Some(reader.read_u8()? != 0);
        unsellable = Some(reader.read_u8()? != 0);
        repairable = Some(reader.read_u8()? != 0);
    }

    Ok(SideBuildEntry {
        building_name,
        template,
        position: Coord3D::new(x, y, z),
        angle,
        initially_built,
        num_rebuilds,
        side_index,
        script_name,
        health,
        whiner,
        unsellable,
        repairable,
    })
}

fn skip_side_build_entry(reader: &mut BinaryReader<'_>, version: u16) -> LoaderResult<()> {
    let _ = parse_side_build_entry(reader, version, 0)?;
    Ok(())
}

fn parse_player_id(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(raw) = trimmed.parse::<u32>() {
        return Some(raw);
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("player_") {
        if let Ok(raw) = rest.parse::<u32>() {
            return raw.checked_sub(1).or(Some(0));
        }
    }
    if let Some(rest) = lower.strip_prefix("player") {
        if let Ok(raw) = rest.parse::<u32>() {
            return raw.checked_sub(1).or(Some(0));
        }
    }

    None
}
