// C++ ownership: ChunkFile/DataChunk token decoding, Dict record conversion, RefPack decompression.

const CHUNK_HEADER_SIZE: usize = 10; // u32 id + u16 version + i32 size

const CHUNK_MAGIC: &[u8; 4] = b"CkMp";

fn decompress_map_bytes(raw_bytes: &[u8]) -> LoaderResult<Vec<u8>> {
    MAP_DECOMPRESS_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    // Real Generals assets commonly use the legacy EA wrapper header:
    //   - 4 byte signature (EAR\0)
    //   - 4 byte uncompressed size (little-endian)
    // followed by a RefPack stream (starting with 0x10FB/0x11FB/...).
    //
    // The repo also contains a newer synthetic header handled by `generals_compression`;
    // keep a fallback path for that format.
    if raw_bytes.len() >= 8 && &raw_bytes[..4] == b"EAR\0" {
        let expected_size =
            u32::from_le_bytes(raw_bytes[4..8].try_into().unwrap_or([0; 4])) as usize;
        return decompress_refpack_stream(&raw_bytes[8..], expected_size).map_err(|err| {
            configuration_error(format!("Failed to decompress RefPack payload: {err}"))
        });
    }

    generals_compression::decompress(raw_bytes)
        .map_err(|err| configuration_error(format!("Fallback decompression failed: {err}")))
}

fn decompress_refpack_stream(data: &[u8], expected_size: usize) -> Result<Vec<u8>, String> {
    // Ported from `GeneralsMD/Code/Libraries/Source/Compression/EAC/refdecode.cpp` (REF_decode).
    if data.len() < 2 {
        return Err("RefPack stream too small".to_string());
    }

    let mut pos: usize = 0;
    let type_word: u16 = ((data[pos] as u16) << 8) | data[pos + 1] as u16;
    pos += 2;

    let ulen: usize;
    if (type_word & 0x8000) != 0 {
        // 4 byte size field
        if (type_word & 0x0100) != 0 {
            // skip ulen
            if data.len() < pos + 4 {
                return Err("RefPack header truncated (skip ulen)".to_string());
            }
            pos += 4;
        }
        if data.len() < pos + 4 {
            return Err("RefPack header truncated (ulen32)".to_string());
        }
        ulen = ((data[pos] as usize) << 24)
            | ((data[pos + 1] as usize) << 16)
            | ((data[pos + 2] as usize) << 8)
            | (data[pos + 3] as usize);
        pos += 4;
    } else {
        // 3 byte size field
        if (type_word & 0x0100) != 0 {
            if data.len() < pos + 3 {
                return Err("RefPack header truncated (skip ulen)".to_string());
            }
            pos += 3;
        }
        if data.len() < pos + 3 {
            return Err("RefPack header truncated (ulen24)".to_string());
        }
        ulen =
            ((data[pos] as usize) << 16) | ((data[pos + 1] as usize) << 8) | data[pos + 2] as usize;
        pos += 3;
    }

    if expected_size != 0 && ulen != expected_size {
        // Keep going (the inner size is authoritative for this stream), but surface the mismatch.
        trace!(
            "RefPack size mismatch: outer={}, inner={}",
            expected_size, ulen
        );
    }

    let mut out: Vec<u8> = Vec::with_capacity(ulen);
    loop {
        if pos >= data.len() {
            return Err("RefPack stream ended before EOF marker".to_string());
        }
        let first = data[pos];
        pos += 1;

        if (first & 0x80) == 0 {
            // short form
            if pos >= data.len() {
                return Err("RefPack short form truncated".to_string());
            }
            let second = data[pos];
            pos += 1;
            let literal_count = (first & 3) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = (((first & 0x60) as usize) << 3) + second as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (short)".to_string())?;

            let mut run = (((first & 0x1c) >> 2) as usize) + 3;
            while run > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (short)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                run -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        if (first & 0x40) == 0 {
            // int form
            if data.len() < pos + 2 {
                return Err("RefPack int form truncated".to_string());
            }
            let second = data[pos];
            let third = data[pos + 1];
            pos += 2;

            let literal_count = (second >> 6) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = (((second & 0x3f) as usize) << 8) + third as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (int)".to_string())?;

            let mut run = ((first & 0x3f) as usize) + 4;
            while run > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (int)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                run -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        if (first & 0x20) == 0 {
            // very int form
            if data.len() < pos + 3 {
                return Err("RefPack very-int form truncated".to_string());
            }
            let second = data[pos];
            let third = data[pos + 1];
            let forth = data[pos + 2];
            pos += 3;

            let literal_count = (first & 3) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = ((((first & 0x10) as usize) >> 4) << 16)
                + ((second as usize) << 8)
                + third as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (very-int)".to_string())?;

            let run = ((((first & 0x0c) as usize) >> 2) << 8) + forth as usize + 5;
            let mut remaining = run;
            while remaining > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (very-int)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                remaining -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        let literal_run = (((first & 0x1f) as usize) << 2) + 4;
        if literal_run <= 112 {
            if data.len() < pos + literal_run {
                return Err("RefPack literal run truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_run]);
            pos += literal_run;
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        // EOF (+0..3 literal)
        let tail = (first & 3) as usize;
        if data.len() < pos + tail {
            return Err("RefPack EOF tail truncated".to_string());
        }
        out.extend_from_slice(&data[pos..pos + tail]);
        let _pos = pos + tail;
        break;
    }

    if out.len() != ulen {
        return Err(format!("Size mismatch: expected {ulen}, got {}", out.len()));
    }
    Ok(out)
}

/// Raw chunky map data for further decoding (terrain, objects, etc.).
#[derive(Clone)]
pub struct ChunkyMap {
    pub source: PathBuf,
    pub toc: HashMap<u32, String>,
    pub body_offset: usize,
    pub bytes: Vec<u8>,
}

thread_local! {
    static MAP_DECOMPRESS_COUNT: Cell<u32> = const { Cell::new(0) };
}
static LAST_LOADED_CHUNKY: Mutex<Option<ChunkyMap>> = Mutex::new(None);

/// How many times this thread RefPack-decoded a `.map` file.
pub fn map_decompress_count() -> u32 {
    MAP_DECOMPRESS_COUNT.with(Cell::get)
}

/// Test helper: isolate decompress-reuse assertions from other tests.
pub fn reset_map_decompress_count() {
    MAP_DECOMPRESS_COUNT.with(|count| count.set(0));
}

fn cached_chunky_for(path: &Path) -> Option<ChunkyMap> {
    LAST_LOADED_CHUNKY
        .lock()
        .ok()?
        .as_ref()
        .filter(|chunky| chunky.source == path)
        .cloned()
}

fn remember_loaded_chunky(chunky: &ChunkyMap) {
    if let Ok(mut guard) = LAST_LOADED_CHUNKY.lock() {
        *guard = Some(chunky.clone());
    }
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

// -------------------------------------------------------------------------------------------------
// Chunk parsing helpers
// -------------------------------------------------------------------------------------------------

struct ChunkHeader {
    label: String,
    version: u16,
    size: usize,
}

struct BinaryReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, len: usize) -> LoaderResult<&'a [u8]> {
        if self.remaining() < len {
            return Err(configuration_error("Unexpected end of chunk data"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_u32(&mut self) -> LoaderResult<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_i32(&mut self) -> LoaderResult<i32> {
        Ok(self.read_u32()? as i32)
    }

    fn read_u16(&mut self) -> LoaderResult<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_i16_vec(&mut self, count: usize) -> LoaderResult<Vec<i16>> {
        let bytes = self.read_bytes(count.saturating_mul(2))?;
        Ok(bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes(chunk.try_into().unwrap()))
            .collect())
    }

    fn read_u8(&mut self) -> LoaderResult<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_f32(&mut self) -> LoaderResult<f32> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_ascii_string(&mut self) -> LoaderResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        let text = String::from_utf8_lossy(bytes).to_string();
        Ok(text)
    }

    fn read_unicode_string(&mut self) -> LoaderResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len.saturating_mul(2))?;
        let mut utf16 = Vec::with_capacity(len);
        for chunk in bytes.chunks_exact(2) {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(String::from_utf16_lossy(&utf16))
    }

    fn take_remaining(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }
}

fn parse_chunk_toc(bytes: &[u8]) -> LoaderResult<(HashMap<u32, String>, usize)> {
    if bytes.len() < CHUNK_MAGIC.len() {
        return Err(configuration_error(
            "Chunky file too small to contain header",
        ));
    }
    if &bytes[..4] != CHUNK_MAGIC {
        return Err(configuration_error("Missing chunky magic header"));
    }

    let mut reader = BinaryReader::new(bytes);
    reader.read_bytes(4)?; // consume magic
    let count = reader.read_i32()? as usize;
    let mut toc = HashMap::with_capacity(count);
    for _ in 0..count {
        let name_len = reader.read_u8()? as usize;
        let name_bytes = reader.read_bytes(name_len)?;
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let id = reader.read_u32()?;
        toc.insert(id, name);
    }

    Ok((toc, reader.pos))
}

fn read_chunk_header(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<ChunkHeader>> {
    if reader.remaining() < CHUNK_HEADER_SIZE {
        return Ok(None);
    }

    let id = reader.read_u32()?;
    let Some(label) = toc.get(&id).cloned() else {
        return Err(configuration_error(format!(
            "Chunk id 0x{id:08X} missing from table of contents"
        )));
    };
    let version = reader.read_u16()?;
    let size = reader.read_i32()?;
    if size < 0 {
        return Err(configuration_error(format!(
            "Chunk '{}' reported negative payload size",
            label
        )));
    }
    let size = size as usize;
    if reader.remaining() < size {
        return Err(configuration_error(format!(
            "Chunk '{}' extends past parent data region",
            label
        )));
    }

    Ok(Some(ChunkHeader {
        label,
        version,
        size,
    }))
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
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?.to_vec();
        handler(&header.label, header.version, &payload)?;
    }
    Ok(())
}

fn find_chunk_by_label<'a>(
    data: &'a [u8],
    toc: &HashMap<u32, String>,
    target: &str,
) -> LoaderResult<Option<(u16, &'a [u8])>> {
    let mut reader = BinaryReader::new(data);
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?;
        if header.label == target {
            return Ok(Some((header.version, payload)));
        }
    }
    Ok(None)
}
