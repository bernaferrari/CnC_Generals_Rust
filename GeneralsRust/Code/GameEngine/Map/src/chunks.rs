use crate::{BinaryReader, DecodeError, DecodeResult};
use std::collections::HashMap;

pub const CHUNK_HEADER_SIZE: usize = 10;
pub const CHUNK_MAGIC: &[u8; 4] = b"CkMp";

pub struct ChunkHeader {
    pub label: String,
    pub version: u16,
    pub size: usize,
}

pub fn parse_chunk_toc(bytes: &[u8]) -> DecodeResult<(HashMap<u32, String>, usize)> {
    if bytes.len() < CHUNK_MAGIC.len() {
        return Err(DecodeError::new("Chunky file too small to contain header"));
    }
    if &bytes[..4] != CHUNK_MAGIC {
        return Err(DecodeError::new("Missing chunky magic header"));
    }

    let mut reader = BinaryReader::new(bytes);
    reader.read_bytes(4)?; // consume magic
    let count = reader.read_i32()?;
    if count < 0 || count as usize > reader.remaining() / 5 {
        return Err(DecodeError::new("Invalid chunky table entry count"));
    }
    let count = count as usize;
    let mut toc = HashMap::with_capacity(count);
    for _ in 0..count {
        let name_len = reader.read_u8()? as usize;
        let name_bytes = reader.read_bytes(name_len)?;
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let id = reader.read_u32()?;
        toc.insert(id, name);
    }

    Ok((toc, reader.position()))
}

pub fn read_chunk_header(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> DecodeResult<Option<ChunkHeader>> {
    if reader.remaining() < CHUNK_HEADER_SIZE {
        return Ok(None);
    }

    let id = reader.read_u32()?;
    let Some(label) = toc.get(&id).cloned() else {
        return Err(DecodeError::new(format!(
            "Chunk id 0x{id:08X} missing from table of contents"
        )));
    };
    let version = reader.read_u16()?;
    let size = reader.read_i32()?;
    if size < 0 {
        return Err(DecodeError::new(format!(
            "Chunk '{}' reported negative payload size",
            label
        )));
    }
    let size = size as usize;
    if reader.remaining() < size {
        return Err(DecodeError::new(format!(
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

pub fn parse_chunk_sequence<F>(
    data: &[u8],
    toc: &HashMap<u32, String>,
    mut handler: F,
) -> DecodeResult<()>
where
    F: FnMut(&str, u16, &[u8]) -> DecodeResult<()>,
{
    let mut reader = BinaryReader::new(data);
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?;
        handler(&header.label, header.version, payload)?;
    }
    Ok(())
}

pub fn find_chunk_by_label<'a>(
    data: &'a [u8],
    toc: &HashMap<u32, String>,
    target: &str,
) -> DecodeResult<Option<(u16, &'a [u8])>> {
    let mut reader = BinaryReader::new(data);
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?;
        if header.label == target {
            return Ok(Some((header.version, payload)));
        }
    }
    Ok(None)
}
