// C++ ownership: WorldHeightMap HeightMapData/BlendTileData/WorldInfo chunks and PolygonTrigger data.

/// Parse the `HeightMapData` chunk into raw 8-bit height samples.
pub fn parse_heightmap_data(map_name: &str) -> LoaderResult<Option<HeightMapData>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };

    parse_heightmap_data_from_chunky(&chunky)
}

pub fn parse_heightmap_data_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<HeightMapData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "HeightMapData")? else {
        return Ok(None);
    };

    let mut reader = BinaryReader::new(payload);
    let width = reader.read_i32()?;
    let height = reader.read_i32()?;
    let border_size = if version >= 3 { reader.read_i32()? } else { 0 };

    let boundaries = if version >= 4 {
        let count = reader.read_i32()?.max(0) as usize;
        let mut boundaries = Vec::with_capacity(count.max(1));
        for _ in 0..count {
            boundaries.push((reader.read_i32()?, reader.read_i32()?));
        }
        boundaries
    } else {
        vec![(width - 2 * border_size, height - 2 * border_size)]
    };

    let data_size = reader.read_i32()?;
    let expected = width.saturating_mul(height);
    if data_size <= 0 || data_size != expected {
        return Err(configuration_error(format!(
            "HeightMapData has invalid dataSize={}, expected {}",
            data_size, expected
        )));
    }

    let mut data = reader.read_bytes(data_size as usize)?.to_vec();

    if version == 1 {
        let new_width = (width + 1) / 2;
        let new_height = (height + 1) / 2;
        let mut resized = vec![0u8; (new_width * new_height).max(0) as usize];
        for i in 0..new_height.max(0) {
            for j in 0..new_width.max(0) {
                let src = (2 * i * width + 2 * j).max(0) as usize;
                let dst = (i * new_width + j).max(0) as usize;
                if src < data.len() && dst < resized.len() {
                    resized[dst] = data[src];
                }
            }
        }
        data = resized;
        return Ok(Some(HeightMapData {
            width: new_width,
            height: new_height,
            border_size,
            boundaries,
            data,
        }));
    }

    Ok(Some(HeightMapData {
        width,
        height,
        border_size,
        boundaries,
        data,
    }))
}

pub fn parse_blend_tile_data_from_chunky(
    chunky: &ChunkyMap,
    heightmap: &HeightMapData,
) -> LoaderResult<Option<BlendTileData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "BlendTileData")? else {
        return Ok(None);
    };

    let mut reader = BinaryReader::new(payload);
    let data_size = reader.read_i32()?;
    let expected = heightmap.width.saturating_mul(heightmap.height);
    if data_size <= 0 || data_size != expected {
        return Err(configuration_error(format!(
            "BlendTileData has invalid dataSize={}, expected {}",
            data_size, expected
        )));
    }
    let data_size = data_size as usize;

    let mut tile_ndxes = reader.read_i16_vec(data_size)?;
    let mut blend_tile_ndxes = reader.read_i16_vec(data_size)?;

    let mut extra_blend_tile_ndxes = if version >= 6 {
        reader.read_i16_vec(data_size)?
    } else {
        vec![0i16; data_size]
    };
    if version >= 5 {
        let _cliff_info_ndxes = reader.read_i16_vec(data_size)?;
    }
    if version >= 7 {
        let byte_width = if version == 7 {
            (heightmap.width + 1) / 8
        } else {
            (heightmap.width + 7) / 8
        }
        .max(0) as usize;
        let byte_count = byte_width.saturating_mul(heightmap.height.max(0) as usize);
        reader.read_bytes(byte_count)?;
    }

    let _num_bitmap_tiles = reader.read_i32()?;
    let num_blended_tiles = reader.read_i32()?.max(0) as usize;
    if version >= 5 {
        let _num_cliff_info = reader.read_i32()?;
    }

    let texture_class_count = reader.read_i32()?.max(0) as usize;
    let mut texture_classes = Vec::with_capacity(texture_class_count);
    for _ in 0..texture_class_count {
        let first_tile = reader.read_i32()?;
        let num_tiles = reader.read_i32()?;
        let width = reader.read_i32()?;
        let _legacy_gdf = reader.read_i32()?;
        let name = reader.read_ascii_string()?;
        texture_classes.push(BlendTileTextureClass {
            first_tile,
            num_tiles,
            width,
            name,
        });
    }

    // C++ WorldHeightMap.cpp:1124-1167 — edge classes (v4+) then TBlendTileInfo.
    // Older synthetic/truncated payloads stop after texture classes; keep
    // tile_ndxes and extra-blend indexes rather than failing the whole parse.
    let mut edge_texture_classes = Vec::new();
    if version >= 4 && reader.remaining() >= 8 {
        let _num_edge_tiles = reader.read_i32()?;
        let edge_class_count = reader.read_i32()?.max(0) as usize;
        edge_texture_classes.reserve(edge_class_count);
        for _ in 0..edge_class_count {
            if reader.remaining() < 12 {
                break;
            }
            let first_tile = reader.read_i32()?;
            let num_tiles = reader.read_i32()?;
            let width = reader.read_i32()?;
            let name = reader.read_ascii_string()?;
            edge_texture_classes.push(BlendTileTextureClass {
                first_tile,
                num_tiles,
                width,
                name,
            });
        }
    }

    let mut blended_tiles = Vec::new();
    if num_blended_tiles > 1 && reader.remaining() > 0 {
        blended_tiles.reserve(num_blended_tiles.saturating_sub(1));
        for _ in 1..num_blended_tiles {
            let blend_ndx = reader.read_i32()?;
            let horiz = reader.read_u8()?;
            let vert = reader.read_u8()?;
            let right_diagonal = reader.read_u8()?;
            let left_diagonal = reader.read_u8()?;
            let inverted = reader.read_u8()?;
            let long_diagonal = if version >= 3 { reader.read_u8()? } else { 0 };
            let custom_blend_edge_class = if version >= 4 { reader.read_i32()? } else { -1 };
            let _flag = reader.read_i32()?;
            blended_tiles.push(BlendTileInfo {
                blend_ndx,
                horiz,
                vert,
                right_diagonal,
                left_diagonal,
                inverted,
                long_diagonal,
                custom_blend_edge_class,
            });
        }
    }

    if version == 1 {
        let new_width = (heightmap.width + 1) / 2;
        let new_height = (heightmap.height + 1) / 2;
        let mut resized_tiles = vec![0i16; (new_width * new_height).max(0) as usize];
        let mut resized_blends = vec![0i16; resized_tiles.len()];
        for i in 0..new_height.max(0) {
            for j in 0..new_width.max(0) {
                let src = (2 * i * heightmap.width + 2 * j).max(0) as usize;
                let dst = (i * new_width + j).max(0) as usize;
                if src < tile_ndxes.len() && dst < resized_tiles.len() {
                    resized_tiles[dst] = tile_ndxes[src];
                    resized_blends[dst] = 0;
                }
            }
        }
        tile_ndxes = resized_tiles;
        blend_tile_ndxes = resized_blends;
        extra_blend_tile_ndxes = vec![0i16; tile_ndxes.len()];
        blended_tiles.clear();
    }

    Ok(Some(BlendTileData {
        tile_ndxes,
        blend_tile_ndxes,
        extra_blend_tile_ndxes,
        texture_classes,
        edge_texture_classes,
        blended_tiles,
    }))
}

/// Parse `WorldInfo` water-table height when the map dict carries one.
///
/// C++ `WorldHeightMap::ParseWorldDictDataChunk` (WorldHeightMap.cpp:739) stores
/// the world dict; crate `MapLoader::extract_water_height` reads `waterHeight`.
/// Missing or unreadable WorldInfo is fail-soft (`None`).
pub fn parse_runtime_water_height_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<f32>> {
    let Some(dict) = parse_runtime_world_info_dict(chunky)? else {
        return Ok(None);
    };
    Ok(dict_lookup_ci(&dict, &["waterHeight", "WaterHeight"]).and_then(|value| value.parse().ok()))
}

/// Parse `WorldInfo` weather enum (`TheKey_weather`).
///
/// C++ `WorldHeightMap::ParseWorldDictDataChunk` (WorldHeightMap.cpp:743-746)
/// writes `TheWritableGlobalData->m_weather` (`WEATHER_NORMAL=0` /
/// `WEATHER_SNOWY=1`) when the key exists. Missing WorldInfo or key is
/// fail-soft (`None`) so GameData.ini `WEATHER` remains the default.
pub fn parse_runtime_weather_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<i32>> {
    let Some(dict) = parse_runtime_world_info_dict(chunky)? else {
        return Ok(None);
    };
    Ok(dict_lookup_ci(&dict, &["weather", "Weather"])
        .and_then(|value| parse_world_weather_value(&value)))
}

/// C++ `Weather` int / WorldBuilder name → `WEATHER_NORMAL=0` / `WEATHER_SNOWY=1`.
pub fn parse_world_weather_value(raw: &str) -> Option<i32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        return Some(value);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" => Some(0),
        "SNOWY" | "SNOW" => Some(1),
        _ => None,
    }
}

/// C++ `TheKey_objectWeather` (`Object.cpp:3595-3605`): 0 follow, 1 force
/// normal, 2 force snow.
pub fn parse_object_weather_value(raw: &str) -> Option<i32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        return Some(value);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" | "FOLLOW" => Some(0),
        "FORCEDNORMAL" | "FORCE_NORMAL" => Some(1),
        "SNOWY" | "SNOW" | "FORCEDSNOW" | "FORCE_SNOW" => Some(2),
        _ => None,
    }
}

/// Resolve SNOW after global weather stamp, then per-object override.
pub fn resolve_object_weather_snow(object_weather: i32, world_is_snow: bool) -> bool {
    match object_weather {
        1 => false,
        2 => true,
        _ => world_is_snow,
    }
}

fn parse_runtime_world_info_dict(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<HashMap<String, String>>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((_version, payload)) = find_chunk_by_label(body, &chunky.toc, "WorldInfo")? else {
        return Ok(None);
    };
    let mut reader = BinaryReader::new(payload);
    if reader.remaining() == 0 {
        return Ok(None);
    }
    match parse_chunk_dict(&mut reader, &chunky.toc) {
        Ok(dict) => Ok(Some(dict)),
        Err(err) => {
            warn!("WorldInfo dict parse failed; water/weather unavailable: {err}");
            Ok(None)
        }
    }
}

/// Parse `PolygonTriggers`, including water-area flags and point Z heights.
///
/// C++ `PolygonTrigger::ParsePolygonTriggersDataChunk` (PolygonTrigger.cpp).
/// Missing or truncated chunks fail-soft to an empty list.
pub fn parse_runtime_polygon_triggers_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<PolygonTrigger>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "PolygonTriggers")?
    else {
        return Ok(Vec::new());
    };

    let mut reader = BinaryReader::new(payload);
    if reader.remaining() < 4 {
        return Ok(Vec::new());
    }
    let count = reader.read_i32()?.max(0);
    let mut triggers = Vec::new();
    let mut max_trigger_id = 0i32;
    for _ in 0..count {
        if reader.remaining() < 2 {
            break;
        }
        let trigger_name = match reader.read_ascii_string() {
            Ok(name) => name,
            Err(_) => break,
        };
        let layer_name = if version >= 4 {
            match reader.read_ascii_string() {
                Ok(name) => name,
                Err(_) => break,
            }
        } else {
            String::new()
        };
        let Ok(trigger_id) = reader.read_i32() else {
            break;
        };
        let is_water = if version >= 2 {
            match reader.read_u8() {
                Ok(byte) => byte != 0,
                Err(_) => break,
            }
        } else {
            false
        };
        let (is_river, river_start) = if version >= 3 {
            let river = match reader.read_u8() {
                Ok(byte) => byte != 0,
                Err(_) => break,
            };
            let start = match reader.read_i32() {
                Ok(value) => value,
                Err(_) => break,
            };
            (river, start)
        } else {
            (false, 0)
        };
        let Ok(num_points) = reader.read_i32() else {
            break;
        };
        let mut points = Vec::new();
        let mut points_ok = true;
        for _ in 0..num_points.max(0) {
            let (Ok(x), Ok(y), Ok(z)) = (reader.read_i32(), reader.read_i32(), reader.read_i32())
            else {
                points_ok = false;
                break;
            };
            points.push(ICoord3D::new(x, y, z));
        }
        if !points_ok {
            break;
        }
        if trigger_id > max_trigger_id {
            max_trigger_id = trigger_id;
        }
        if points.len() < 2 {
            continue;
        }
        let mut trigger =
            PolygonTrigger::new(trigger_id, AsciiString::from(trigger_name.as_str()), points);
        trigger.set_layer_name(AsciiString::from(layer_name.as_str()));
        trigger.set_water_area(is_water);
        trigger.set_river(is_river);
        trigger.set_river_start(river_start);
        triggers.push(trigger);
    }

    // C++ PolygonTrigger.cpp version-1 maps auto-add a full-extent water table.
    if version == 1 {
        // C++ `pTrig->m_triggerID = maxTriggerId++` reuses the current max.
        let water_id = max_trigger_id;
        let mut trigger = PolygonTrigger::new(
            water_id,
            AsciiString::from("AutoAddedWaterAreaTrigger"),
            Vec::new(),
        );
        trigger.set_water_area(true);
        let (water_extent_x, water_extent_y) = game_engine::common::ini::get_global_data()
            .map(|data| {
                let data = data.read();
                (data.water_extent_x, data.water_extent_y)
            })
            .unwrap_or((0.0, 0.0));
        let border = 30.0 * MAP_XY_FACTOR;
        trigger.add_point(ICoord3D::new((-border) as i32, (-border) as i32, 7));
        trigger.add_point(ICoord3D::new(
            (border + water_extent_x) as i32,
            (-border) as i32,
            7,
        ));
        trigger.add_point(ICoord3D::new(
            (border + water_extent_x) as i32,
            (border + water_extent_y) as i32,
            7,
        ));
        trigger.add_point(ICoord3D::new(
            (-border) as i32,
            (border + water_extent_y) as i32,
            7,
        ));
        triggers.push(trigger);
    }

    Ok(triggers)
}

/// Register parsed map polygons on leftover `TerrainLogic` by name.
///
/// C++ `PolygonTrigger::ParsePolygonTriggersDataChunk` leaves the live list
/// as the geometry source for `pointInTrigger`. Skip names already present
/// so `load_map_data` and this installer do not double-add.
pub fn install_runtime_polygon_triggers(triggers: &[PolygonTrigger]) {
    let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() else {
        return;
    };
    for trigger in triggers {
        let name = trigger.get_trigger_name().as_str();
        if terrain.get_trigger_area_by_name(name).is_some() {
            continue;
        }
        terrain.add_trigger_area(trigger.clone());
    }
}

/// Parse world bounds from the map's GlobalLighting or Waypoints chunk.
pub fn parse_world_bounds(map_name: &str) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    parse_world_bounds_from_chunky(&chunky)
}

pub fn parse_world_bounds_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    let body = &chunky.bytes[chunky.body_offset..];

    let mut waypoint_bounds = None;
    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "WaypointsList")? {
        let mut reader = BinaryReader::new(payload);
        if reader.remaining() >= 4 {
            let count = reader.read_i32()? as usize;
            let mut min = Coord3D::new(f32::MAX, f32::MAX, f32::MAX);
            let mut max = Coord3D::new(f32::MIN, f32::MIN, f32::MIN);
            for _ in 0..count {
                if reader.remaining() < 12 {
                    break;
                }
                let x = reader.read_f32()?;
                let y = reader.read_f32()?;
                let z = reader.read_f32()?;
                min.x = min.x.min(x);
                min.y = min.y.min(y);
                min.z = min.z.min(z);
                max.x = max.x.max(x);
                max.y = max.y.max(y);
                max.z = max.z.max(z);
            }
            if min.x < f32::MAX / 2.0 && max.x > f32::MIN / 2.0 {
                waypoint_bounds = Some((min, max));
            }
        }
    }

    if let Some((min, max)) = waypoint_bounds {
        let extent_x = (max.x - min.x).abs();
        let extent_z = (max.z - min.z).abs();
        if extent_x >= 1.0 && extent_z >= 1.0 {
            return Ok(Some((min, max)));
        }
    }

    if let Some(heightmap) = parse_heightmap_data_from_chunky(chunky)? {
        let playable_w = (heightmap.width - 2 * heightmap.border_size).max(1) as f32;
        let playable_h = (heightmap.height - 2 * heightmap.border_size).max(1) as f32;
        let max = Coord3D::new(playable_w * MAP_XY_FACTOR, 0.0, playable_h * MAP_XY_FACTOR);
        return Ok(Some((Coord3D::new(0.0, 0.0, 0.0), max)));
    }

    Ok(None)
}
