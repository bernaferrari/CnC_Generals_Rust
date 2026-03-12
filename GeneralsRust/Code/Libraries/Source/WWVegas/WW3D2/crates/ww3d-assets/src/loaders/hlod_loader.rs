//! W3D HLOD Loader
//!
//! Implements loading of W3D_CHUNK_HLOD chunks into HLOD prototypes.
//! This is a faithful port of HLodDefClass::Load_W3D in hlod.cpp.
//!
//! # C++ Reference
//! - File: `/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp`
//! - Functions: `HLodDefClass::Load_W3D`, `HLodDefClass::read_header`,
//!   `HLodDefClass::SubObjectArrayClass::Load_W3D`, `HLodDefClass::read_proxy_array`

use crate::chunk_reader::{ChunkError, ChunkReader, ChunkResult};
use crate::prototypes::{
    HlodAggregateEntry, HlodLodEntry, HlodPrototype, HlodProxyEntry, HlodSubObject,
};
use std::io::{Read, Seek};
use ww3d_core::W3DChunkType;

#[derive(Debug, Clone)]
struct HlodHeader {
    version: u32,
    lod_count: u32,
    name: String,
    hierarchy_name: String,
}

pub struct HlodLoader;

impl HlodLoader {
    /// Load a HLOD definition from a ChunkReader positioned at HLOD chunk.
    pub fn load_hlod<R: Read + Seek>(reader: &mut ChunkReader<R>) -> ChunkResult<HlodPrototype> {
        let header = Self::read_header(reader)?;

        let mut lods = Vec::with_capacity(header.lod_count as usize);
        for _ in 0..header.lod_count {
            if !reader.open_chunk()? {
                return Err(ChunkError::InvalidHeader);
            }

            if reader.current_chunk_id()? != W3DChunkType::HlodLodArray.as_u32() {
                return Err(ChunkError::InvalidHeader);
            }

            let lod_entry = Self::read_subobject_array(reader)?;
            lods.push(lod_entry);

            reader.close_chunk()?;
        }

        let mut aggregates = Vec::new();
        let mut proxies = Vec::new();

        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::HlodAggregateArray) => {
                    if let Some(entry) = Self::read_aggregate_array(reader)? {
                        aggregates.push(entry);
                    }
                }
                Some(W3DChunkType::HlodProxyArray) => {
                    proxies = Self::read_proxy_array(reader)?;
                }
                _ => {
                    // Skip unknown chunks
                }
            }

            reader.close_chunk()?;
        }

        Ok(HlodPrototype {
            name: header.name,
            hierarchy_name: header.hierarchy_name,
            version: header.version,
            lods,
            aggregates,
            proxy_entries: proxies,
        })
    }

    fn read_header<R: Read + Seek>(reader: &mut ChunkReader<R>) -> ChunkResult<HlodHeader> {
        if !reader.open_chunk()? {
            return Err(ChunkError::InvalidHeader);
        }

        if reader.current_chunk_id()? != W3DChunkType::HlodHeader.as_u32() {
            return Err(ChunkError::InvalidHeader);
        }

        let version = reader.read_u32()?;
        let lod_count = reader.read_u32()?;
        let name = reader.read_fixed_string(16)?;
        let hierarchy_name = reader.read_fixed_string(16)?;

        reader.close_chunk()?;

        Ok(HlodHeader {
            version,
            lod_count,
            name,
            hierarchy_name,
        })
    }

    fn read_subobject_array<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<HlodLodEntry> {
        if !reader.open_chunk()? {
            return Err(ChunkError::InvalidHeader);
        }

        if reader.current_chunk_id()? != W3DChunkType::HlodSubObjectArrayHeader.as_u32() {
            return Err(ChunkError::InvalidHeader);
        }

        let (model_count, max_screen_size) = Self::read_subobject_array_header(reader)?;
        reader.close_chunk()?;

        let models = Self::read_subobjects(reader, model_count)?;

        Ok(HlodLodEntry {
            max_screen_size,
            models,
        })
    }

    fn read_aggregate_array<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<Option<HlodAggregateEntry>> {
        if !reader.open_chunk()? {
            return Ok(None);
        }

        let chunk_id = reader.current_chunk_id()?;
        let entry = match W3DChunkType::from_u32(chunk_id) {
            Some(W3DChunkType::HlodLodArray) => {
                let lod_entry = Self::read_subobject_array(reader)?;
                reader.close_chunk()?;
                Some(HlodAggregateEntry {
                    max_screen_size: lod_entry.max_screen_size,
                    models: lod_entry.models,
                })
            }
            Some(W3DChunkType::HlodSubObjectArrayHeader) => {
                let (model_count, max_screen_size) = Self::read_subobject_array_header(reader)?;
                reader.close_chunk()?;
                let models = Self::read_subobjects(reader, model_count)?;
                Some(HlodAggregateEntry {
                    max_screen_size,
                    models,
                })
            }
            _ => {
                reader.close_chunk()?;
                None
            }
        };

        Ok(entry)
    }

    fn read_proxy_array<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<Vec<HlodProxyEntry>> {
        if !reader.open_chunk()? {
            return Ok(Vec::new());
        }

        let chunk_id = reader.current_chunk_id()?;
        let proxies = match W3DChunkType::from_u32(chunk_id) {
            Some(W3DChunkType::HlodLodArray) => {
                let lod_entry = Self::read_subobject_array(reader)?;
                reader.close_chunk()?;
                lod_entry
                    .models
                    .into_iter()
                    .map(|model| HlodProxyEntry {
                        name: model.name,
                        bone_index: model.bone_index,
                    })
                    .collect()
            }
            Some(W3DChunkType::HlodSubObjectArrayHeader) => {
                let (model_count, _) = Self::read_subobject_array_header(reader)?;
                reader.close_chunk()?;
                let models = Self::read_subobjects(reader, model_count)?;
                models
                    .into_iter()
                    .map(|model| HlodProxyEntry {
                        name: model.name,
                        bone_index: model.bone_index,
                    })
                    .collect()
            }
            _ => {
                reader.close_chunk()?;
                Vec::new()
            }
        };

        Ok(proxies)
    }

    fn read_subobject_array_header<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<(u32, f32)> {
        let model_count = reader.read_u32()?;
        let max_screen_size = reader.read_f32()?;
        Ok((model_count, max_screen_size))
    }

    fn read_subobjects<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        model_count: u32,
    ) -> ChunkResult<Vec<HlodSubObject>> {
        let mut models = Vec::with_capacity(model_count as usize);

        for _ in 0..model_count {
            if !reader.open_chunk()? {
                return Err(ChunkError::InvalidHeader);
            }

            if reader.current_chunk_id()? != W3DChunkType::HlodSubObject.as_u32() {
                return Err(ChunkError::InvalidHeader);
            }

            let bone_index = reader.read_u32()?;
            let name = reader.read_fixed_string(32)?;

            reader.close_chunk()?;

            models.push(HlodSubObject { name, bone_index });
        }

        Ok(models)
    }
}
