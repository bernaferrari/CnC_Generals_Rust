// C++ ownership: ChunkFile/DataChunk token decoding, Dict record conversion, RefPack decompression.

pub use generals_map::ChunkyMap;
use generals_map::{BinaryReader, CHUNK_MAGIC};

fn decompress_map_bytes(raw_bytes: &[u8]) -> LoaderResult<Vec<u8>> {
    // Real Generals assets commonly use the legacy EA wrapper header:
    //   - 4 byte signature (EAR\0)
    //   - 4 byte uncompressed size (little-endian)
    // followed by a RefPack stream (starting with 0x10FB/0x11FB/...).
    //
    // The repo also contains a newer synthetic header handled by `generals_compression`;
    // keep a fallback path for that format.
    if raw_bytes.len() >= 8 && &raw_bytes[..4] == b"EAR\0" {
        return game_engine::common::system::compression::decompress_data(raw_bytes).map_err(
            |err| configuration_error(format!("Failed to decompress RefPack payload: {err}")),
        );
    }
    // A bare RefPack stream has no EAR wrapper. generals_compression treats
    // its RefPack type as LZ4, so recognize the retail type id and reuse
    // REF_decode through the EAR entry point.
    if raw_bytes.len() >= 2 {
        let type_id = ((raw_bytes[0] as u16) << 8) | raw_bytes[1] as u16;
        if matches!(type_id, 0x10FB | 0x11FB | 0x90FB | 0x91FB) {
            let mut wrapped = Vec::with_capacity(8 + raw_bytes.len());
            wrapped.extend_from_slice(b"EAR\0");
            wrapped.extend_from_slice(&[0, 0, 0, 0]);
            wrapped.extend_from_slice(raw_bytes);
            return game_engine::common::system::compression::decompress_data(&wrapped).map_err(
                |err| configuration_error(format!("Failed to decompress RefPack payload: {err}")),
            );
        }
    }

    generals_compression::decompress(raw_bytes)
        .map_err(|err| configuration_error(format!("Fallback decompression failed: {err}")))
}

fn parse_chunk_dict(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<HashMap<String, String>> {
    let pair_count = reader.read_u16()? as usize;
    let mut dict = HashMap::with_capacity(pair_count);
    for _ in 0..pair_count {
        let key_and_type = reader.read_i32()? as u32;
        let data_type = (key_and_type & 0xFF) as u8;
        let name_id = key_and_type >> 8;
        let key_name = toc.get(&name_id).cloned().unwrap_or_default();
        let value = match data_type {
            0 => (reader.read_u8()? != 0).to_string(),
            1 => reader.read_i32()?.to_string(),
            2 => reader.read_f32()?.to_string(),
            3 => reader.read_ascii_string()?,
            4 => reader.read_unicode_string()?,
            _ => {
                return Err(configuration_error(format!(
                    "Unknown map dict value type {}",
                    data_type
                )));
            }
        };
        if !key_name.is_empty() {
            dict.insert(key_name, value);
        }
    }
    Ok(dict)
}

fn parse_chunk_dict_typed(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Dict> {
    let pair_count = reader.read_u16()? as usize;
    let mut dict = Dict::new();
    for _ in 0..pair_count {
        let key_and_type = reader.read_i32()? as u32;
        let data_type = (key_and_type & 0xFF) as u8;
        let name_id = key_and_type >> 8;
        let key_name = toc.get(&name_id).cloned().unwrap_or_default();
        if key_name.is_empty() {
            match data_type {
                0 => {
                    let _ = reader.read_u8()?;
                }
                1 => {
                    let _ = reader.read_i32()?;
                }
                2 => {
                    let _ = reader.read_f32()?;
                }
                3 => {
                    let _ = reader.read_ascii_string()?;
                }
                4 => {
                    let _ = reader.read_unicode_string()?;
                }
                _ => {
                    return Err(configuration_error(format!(
                        "Unknown map dict value type {}",
                        data_type
                    )));
                }
            }
            continue;
        }
        let key = NameKeyGenerator::name_to_key(&key_name);
        match data_type {
            0 => dict.set_bool(key, reader.read_u8()? != 0),
            1 => dict.set_int(key, reader.read_i32()?),
            2 => dict.set_real(key, reader.read_f32()?),
            3 => dict.set_ascii_string(key, reader.read_ascii_string()?),
            4 => dict.set_unicode_string(key, reader.read_unicode_string()?),
            _ => {
                return Err(configuration_error(format!(
                    "Unknown map dict value type {}",
                    data_type
                )));
            }
        }
    }
    Ok(dict)
}

fn dict_to_string_map(dict: &Dict) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(dict.get_pair_count());
    for i in 0..dict.get_pair_count() {
        let Some(key) = dict.get_nth_key(i) else {
            continue;
        };
        let Some(name) = NameKeyGenerator::key_to_name(key) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let value = match dict.get_type(key) {
            Some(DictType::Bool) => dict.get_bool(key).to_string(),
            Some(DictType::Int) => dict.get_int(key).to_string(),
            Some(DictType::Real) => dict.get_real(key).to_string(),
            Some(DictType::AsciiString) => dict.get_ascii_string(key),
            Some(DictType::UnicodeString) => dict.get_unicode_string(key),
            None => continue,
        };
        out.insert(name, value);
    }
    out
}

fn dict_lookup_ci(dict: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = dict.get(*key) {
            return Some(value.trim().to_string());
        }
        if let Some((_, value)) = dict
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn parse_ini_boolish(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn dict_contains_key(dict: &HashMap<String, String>, key: &str) -> bool {
    dict.contains_key(key)
        || dict
            .keys()
            .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

// Engine error/callback adapters; all byte traversal lives in generals_map.
fn parse_chunk_toc(bytes: &[u8]) -> LoaderResult<(HashMap<u32, String>, usize)> {
    Ok(generals_map::parse_chunk_toc(bytes)?)
}
fn parse_chunk_sequence<F>(
    data: &[u8],
    toc: &HashMap<u32, String>,
    mut handler: F,
) -> LoaderResult<()>
where
    F: FnMut(&str, u16, &[u8]) -> LoaderResult<()>,
{
    let mut reader = BinaryReader::new(data);
    while let Some(header) = generals_map::read_chunk_header(&mut reader, toc)? {
        // Borrow the parent bytes rather than copying each payload.
        let payload = reader.read_bytes(header.size)?;
        handler(&header.label, header.version, payload)?;
    }
    Ok(())
}
fn find_chunk_by_label<'a>(
    data: &'a [u8],
    toc: &HashMap<u32, String>,
    target: &str,
) -> LoaderResult<Option<(u16, &'a [u8])>> {
    Ok(generals_map::find_chunk_by_label(data, toc, target)?)
}
