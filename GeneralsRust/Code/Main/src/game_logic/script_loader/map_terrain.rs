// C++ ownership: WorldHeightMap HeightMapData/BlendTileData/WorldInfo chunks and PolygonTrigger data.

use game_engine::common::system::game_type::Weather;

/// Parse the `HeightMapData` chunk into raw 8-bit height samples.
pub fn parse_heightmap_data(map_name: &str) -> LoaderResult<Option<HeightMapData>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };

    parse_heightmap_data_from_chunky(&chunky)
}


/// Read the map's authored width, height and border without copying samples.
pub fn parse_heightmap_dimensions_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<(i32, i32, i32)>> {
    Ok(generals_map::read_heightmap_dimensions(chunky)?)
}

pub fn parse_heightmap_data_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<HeightMapData>> {
    Ok(generals_map::decode_heightmap(chunky)?)
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
    let mut cliff_info_ndxes = if version >= 5 {
        reader.read_i16_vec(data_size)?
    } else {
        vec![0; data_size]
    };
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
    let num_cliff_info = if version >= 5 {
        reader.read_i32()?.max(1) as usize
    } else {
        1
    };

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

    // WorldHeightMap.cpp:1168-1181 stores records after all blend records.
    let mut cliff_info = Vec::with_capacity(num_cliff_info.saturating_sub(1));
    for _ in 1..num_cliff_info {
        let tile_index = reader.read_i32()?;
        let mut u = [0.0; 4];
        let mut v = [0.0; 4];
        for i in 0..4 {
            u[i] = reader.read_f32()?;
            v[i] = reader.read_f32()?;
        }
        cliff_info.push(CliffInfo {
            tile_index,
            u,
            v,
            flip: reader.read_u8()? != 0,
            mutant: reader.read_u8()? != 0,
        });
    }
    // C++ constructor sanitizes indices against the loaded table.
    for index in &mut cliff_info_ndxes {
        if *index < 0 || *index as usize >= num_cliff_info {
            *index = 0;
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
        cliff_info_ndxes = vec![0; tile_ndxes.len()];
        cliff_info.clear();
        blended_tiles.clear();
    }

    Ok(Some(BlendTileData {
        tile_ndxes,
        blend_tile_ndxes,
        extra_blend_tile_ndxes,
        cliff_info_ndxes,
        cliff_info,
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
    let weather = match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" => Weather::Normal,
        "SNOWY" | "SNOW" => Weather::Snowy,
        _ => return None,
    };
    Some(weather.value() as i32)
}

/// A map object's SNOW override (`Object::updateObjValuesFromMapProperties`).
#[repr(i32)]
enum ObjectWeatherOverride {
    FollowWorld = 0,
    ForceNormal = 1,
    ForceSnow = 2,
}

impl ObjectWeatherOverride {
    fn from_value(value: i32) -> Self {
        match value {
            1 => Self::ForceNormal,
            2 => Self::ForceSnow,
            _ => Self::FollowWorld,
        }
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
    let weather = match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" | "FOLLOW" => ObjectWeatherOverride::FollowWorld,
        "FORCEDNORMAL" | "FORCE_NORMAL" => ObjectWeatherOverride::ForceNormal,
        "SNOWY" | "SNOW" | "FORCEDSNOW" | "FORCE_SNOW" => ObjectWeatherOverride::ForceSnow,
        _ => return None,
    };
    Some(weather as i32)
}

/// Resolve SNOW after global weather stamp, then per-object override.
pub fn resolve_object_weather_snow(object_weather: i32, world_is_snow: bool) -> bool {
    match ObjectWeatherOverride::from_value(object_weather) {
        ObjectWeatherOverride::FollowWorld => world_is_snow,
        ObjectWeatherOverride::ForceNormal => false,
        ObjectWeatherOverride::ForceSnow => true,
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
    water_extent: [f32; 2],
) -> LoaderResult<Vec<PolygonTrigger>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "PolygonTriggers")?
    else {
        return Ok(Vec::new());
    };
    Ok(game_engine::polygon_trigger::decode_polygon_triggers(
        version,
        payload,
        water_extent,
    ))
}

/// Admit the parsed polygon table on its explicit receiving logical owner.
/// The source parser replaces its list and preserves every row in file order,
/// including duplicate names/IDs; first-match lookup remains the owner policy.
/// Shared with canonical map admission so early load-screen callbacks observe
/// the same complete table without a global lookup or contention-based skip.
pub fn install_runtime_polygon_triggers(
    terrain: &mut gamelogic::terrain::TerrainLogic,
    triggers: &[PolygonTrigger],
) {
    terrain.replace_receiving_trigger_areas(triggers.iter().cloned());
}

/// Parse the map's logical world bounds.
///
/// C++ `W3DTerrainLogic::getExtent` is zero-origin and spans the heightmap's
/// playable area. `WaypointsList` holds waypoint links, never world bounds.
pub fn parse_world_bounds(map_name: &str) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    parse_world_bounds_from_chunky(&chunky)
}

pub fn parse_world_bounds_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    if let Some((width, height, border_size)) = parse_heightmap_dimensions_from_chunky(chunky)? {
        let playable_w = (width - 2 * border_size).max(1) as f32;
        let playable_h = (height - 2 * border_size).max(1) as f32;
        let max = Coord3D::new(playable_w * MAP_XY_FACTOR, 0.0, playable_h * MAP_XY_FACTOR);
        return Ok(Some((Coord3D::new(0.0, 0.0, 0.0), max)));
    }

    Ok(None)
}
