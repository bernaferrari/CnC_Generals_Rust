//! W3D Animation File Chunk Loader
//!
//! This module provides binary chunk parsing for W3D animation files (.w3d).
//! It uses binrw for efficient binary I/O and properly extracts keyframe data
//! that can be used by the animation system.
//!
//! C++ Reference: w3d_file.h, w3d_file.cpp, chunks.cpp

use binrw::BinReaderExt;
use std::io::{Read, Seek, SeekFrom};
use thiserror::Error;
use ww3d_core::{
    W3DChunkType, W3dChunkHeader,
    W3dCompressedAnimHeaderStruct, W3dHModelHeaderStruct, W3dHModelNodeStruct, W3dHierarchyStruct,
    W3dPivotStruct,
};

use crate::hanim::HAnimClass;
use crate::hcompressed_anim::{
    ANIM_FLAVOR_ADAPTIVE_DELTA, ANIM_FLAVOR_TIMECODED, HCompressedAnimClass,
};
use crate::hraw_anim::HRawAnimClass;
use crate::htree::HTreeClass;
use crate::motion_channels::{
    AdaptiveDeltaMotionChannelClass, AnimChannelKind, BitChannelKind, TimeCodedBitChannelClass,
    TimeCodedMotionChannelClass,
};

#[derive(Debug, Error)]
pub enum W3DAnimationError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Binary read error: {0}")]
    BinReadError(String),

    #[error("Invalid chunk type: expected {expected}, found {found}")]
    InvalidChunkType { expected: String, found: String },

    #[error("Missing required chunk: {0}")]
    MissingChunk(String),

    #[error("Invalid animation data: {0}")]
    InvalidData(String),

    #[error("Unsupported animation flavor: {0}")]
    UnsupportedFlavor(u32),
}

impl From<binrw::Error> for W3DAnimationError {
    fn from(err: binrw::Error) -> Self {
        W3DAnimationError::BinReadError(format!("{:?}", err))
    }
}

/// Parsed W3D animation data - supports both uncompressed and compressed animations
#[derive(Debug, Clone)]
pub struct W3DAnimationData {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub compression_flavor: Option<u32>, // None = uncompressed, Some(0) = timecoded, Some(1) = adaptive delta
    /// Canonical classic channels for `W3D_CHUNK_ANIMATION`.
    pub raw_anim: Option<HRawAnimClass>,
    /// Canonical compressed channels for `W3D_CHUNK_COMPRESSED_ANIMATION`.
    pub compressed_anim: Option<HCompressedAnimClass>,
    pub morph_anim: Option<W3DMorphAnimationData>,
}

/// Morph animation data
#[derive(Debug, Clone)]
pub struct W3DMorphAnimationData {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub channels: Vec<W3DMorphChannel>,
    pub pivot_channels: Vec<u32>,
}

/// Morph animation channel
#[derive(Debug, Clone)]
pub struct W3DMorphChannel {
    pub pose_name: String,
    pub keyframes: Vec<W3DMorphKey>,
}

/// Time-coded morph key mapping a morph frame to a pose-animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W3DMorphKey {
    pub morph_frame: u32,
    pub pose_frame: u32,
}

/// HModel data (Hierarchical Model blueprint)
#[derive(Debug, Clone)]
pub struct W3DHModelData {
    pub name: String,
    pub hierarchy_name: String,
    pub nodes: Vec<W3DHModelNode>,
}

/// HModel node connection
#[derive(Debug, Clone)]
pub struct W3DHModelNode {
    pub render_obj_name: String,
    pub pivot_idx: u32,
}

/// Load a W3D animation from a file
pub fn load_w3d_animation_from_file(
    path: &str,
    hierarchy_pivots: &dyn Fn(&str) -> Option<usize>,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3d_animation(&mut file, hierarchy_pivots)
}

/// Load a W3D animation from a reader
pub fn load_w3d_animation<R: Read + Seek>(
    reader: &mut R,
    hierarchy_pivots: &dyn Fn(&str) -> Option<usize>,
) -> Result<W3DAnimationData, W3DAnimationError> {
    // Read the file and find the animation chunk
    let file_len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    while reader.stream_position()? + 8 <= file_len {
        let header: W3dChunkHeader = reader.read_le()?;
        let chunk_start = reader.stream_position()?;
        let chunk_end = chunk_start + header.actual_size() as u64;

        match header.chunk_type() {
            Some(W3DChunkType::Animation) => {
                return parse_animation_chunk(reader, header.actual_size());
            }
            Some(W3DChunkType::CompressedAnimation) => {
                // NEW: Parse compressed animation with full HCompressedAnimClass support
                return parse_compressed_animation_chunk(reader, header.actual_size(), hierarchy_pivots);
            }
            Some(W3DChunkType::MorphAnimation) => {
                return parse_morph_animation_chunk(reader, header.actual_size());
            }
            Some(W3DChunkType::Hmodel) => {
                // HModel chunks are hierarchical models, not animations
                // Skip for now, but could be parsed separately
                reader.seek(SeekFrom::Start(chunk_end))?;
            }
            _ => {
                // Skip this chunk
                reader.seek(SeekFrom::Start(chunk_end))?;
            }
        }
    }

    Err(W3DAnimationError::MissingChunk(
        "Animation chunk not found in file".to_string(),
    ))
}

/// Parse an uncompressed animation chunk with the canonical raw owner.
pub(crate) fn parse_animation_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let mut payload = vec![0; usize::try_from(chunk_size).map_err(|_| {
        W3DAnimationError::InvalidData("animation chunk exceeds address space".into())
    })?];
    reader.read_exact(&mut payload)?;
    let raw = HRawAnimClass::load_w3d(&payload)?;
    #[allow(clippy::cast_precision_loss)]
    let frame_rate = raw.get_frame_rate() as f32;
    Ok(W3DAnimationData {
        name: raw.get_name().to_string(),
        hierarchy_name: raw.get_hierarchy_name().to_string(),
        num_frames: raw.get_num_frames(),
        frame_rate,
        compression_flavor: None,
        raw_anim: Some(raw),
        compressed_anim: None,
        morph_anim: None,
    })
}

/// Parse a compressed animation chunk with full HCompressedAnimClass integration
/// Reference: w3d_file.cpp compressed animation loading
pub fn parse_compressed_animation_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
    hierarchy_pivots: &dyn Fn(&str) -> Option<usize>,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;
    let payload_start = reader.stream_position()?;
    if chunk_end > reader.seek(SeekFrom::End(0))? {
        return Err(W3DAnimationError::InvalidData("truncated compressed container".into()));
    }
    reader.seek(SeekFrom::Start(payload_start))?;

    let mut header: Option<W3dCompressedAnimHeaderStruct> = None;

    // First pass: read header to create HCompressedAnimClass
    let start_pos = reader.stream_position()?;

    while reader.stream_position()? < chunk_end {
        if chunk_end - reader.stream_position()? < 8 {
            return Err(W3DAnimationError::InvalidData("truncated compressed subchunk header".into()));
        }
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;
        if sub_end > chunk_end {
            return Err(W3DAnimationError::InvalidData("compressed subchunk exceeds container".into()));
        }

        if let Some(W3DChunkType::CompressedAnimationHeader) = sub_header.chunk_type() {
            if sub_header.actual_size() < 44 {
                return Err(W3DAnimationError::InvalidData("truncated compressed animation header".into()));
            }
            header = Some(reader.read_le()?);
            break;
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header = header.ok_or_else(|| {
        W3DAnimationError::MissingChunk("Compressed animation header".to_string())
    })?;

    let hierarchy_name = ww3d_core::w3d_string_from_bytes(&header.hierarchy_name);
    let num_nodes = hierarchy_pivots(&hierarchy_name).ok_or_else(|| {
        W3DAnimationError::MissingChunk(format!("Hierarchy {hierarchy_name}"))
    })?;

    // C++ allocates one motion slot per pivot in the named hierarchy.
    let flavor = u32::from(header.flavor);
    if !matches!(flavor, ANIM_FLAVOR_TIMECODED | ANIM_FLAVOR_ADAPTIVE_DELTA) {
        return Err(W3DAnimationError::UnsupportedFlavor(flavor));
    }
    if header.num_frames == 0 || header.num_frames > (1 << 24) || header.frame_rate == 0 {
        return Err(W3DAnimationError::InvalidData("invalid compressed frame domain".into()));
    }
    let mut compressed_anim = HCompressedAnimClass::new(
        ww3d_core::w3d_string_from_bytes(&header.name),
        ww3d_core::w3d_string_from_bytes(&header.hierarchy_name),
        header.num_frames,
        num_nodes,
        flavor,
        header.frame_rate as f32,
    );

    // Second pass: parse all channels and add to compressed_anim
    reader.seek(SeekFrom::Start(start_pos))?;

    while reader.stream_position()? < chunk_end {
        if chunk_end - reader.stream_position()? < 8 {
            return Err(W3DAnimationError::InvalidData("truncated compressed subchunk header".into()));
        }
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;
        if sub_end > chunk_end {
            return Err(W3DAnimationError::InvalidData("compressed subchunk exceeds container".into()));
        }

        match sub_header.chunk_type() {
            Some(W3DChunkType::CompressedAnimationChannel) => {
                match flavor {
                    // `hcanim.cpp:257-289`: a missing bone is reported and
                    // discarded; an unused channel kind is never installed.
                    ANIM_FLAVOR_TIMECODED => {
                        let channel = parse_timecoded_motion_channel(reader, sub_header.actual_size())?;
                        if channel.as_ref().is_some_and(|c| c.last_frame() >= header.num_frames) {
                            return Err(W3DAnimationError::InvalidData("timecoded channel outside animation domain".into()));
                        }
                        if let Some(channel) = channel.filter(|c| pivot_in(c.get_pivot(), num_nodes)) {
                            compressed_anim.add_timecoded_channel(channel);
                        }
                    }
                    ANIM_FLAVOR_ADAPTIVE_DELTA => {
                        let channel = parse_adaptive_delta_motion_channel(reader, sub_header.actual_size(), header.num_frames)?;
                        if let Some(channel) = channel.filter(|c| pivot_in(c.get_pivot(), num_nodes)) {
                            compressed_anim.add_adaptive_delta_channel(channel);
                        }
                    }
                    _ => unreachable!("flavor validated above"),
                }
            }
            Some(W3DChunkType::CompressedBitChannel) => {
                let channel = parse_timecoded_bit_channel(reader, sub_header.actual_size())?;
                if channel.as_ref().is_some_and(|c| c.last_frame() >= header.num_frames) {
                    return Err(W3DAnimationError::InvalidData("bit channel outside animation domain".into()));
                }
                if let Some(channel) = channel.filter(|c| pivot_in(c.get_pivot(), num_nodes)) {
                    compressed_anim.add_bit_channel(channel);
                }
            }
            _ => {
                // Skip unknown sub-chunks
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    Ok(W3DAnimationData {
        name: ww3d_core::w3d_string_from_bytes(&header.name),
        hierarchy_name: ww3d_core::w3d_string_from_bytes(&header.hierarchy_name),
        num_frames: header.num_frames,
        frame_rate: header.frame_rate as f32,
        compression_flavor: Some(flavor),
        raw_anim: None,
        compressed_anim: Some(compressed_anim),
        morph_anim: None,
    })
}

fn pivot_in(pivot: u32, num_nodes: usize) -> bool {
    usize::try_from(pivot).is_ok_and(|pivot| pivot < num_nodes)
}

/// Compressed containers install only X/Y/Z/Q (`hcanim.cpp:368-415`).
fn installed_kind(flags: u8) -> Option<AnimChannelKind> {
    AnimChannelKind::from_flags(u16::from(flags)).filter(|kind| {
        matches!(kind, AnimChannelKind::X | AnimChannelKind::Y | AnimChannelKind::Z | AnimChannelKind::Quaternion)
    })
}

/// Parse a timecoded motion channel for compressed animations
/// Reference: motchan.cpp:386-413
fn parse_timecoded_motion_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<Option<TimeCodedMotionChannelClass>, W3DAnimationError> {
    const HEADER_SIZE: u32 = 8;
    if chunk_size < HEADER_SIZE {
        return Err(W3DAnimationError::InvalidData(format!(
            "timecoded channel too small: {chunk_size}"
        )));
    }

    let num_timecodes: u32 = reader.read_le()?;
    let pivot: u16 = reader.read_le()?;
    let vector_len_raw: u8 = reader.read_le()?;
    let flags: u8 = reader.read_le()?;
    let vector_len = usize::from(vector_len_raw);
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;
    if remaining_bytes % 4 != 0 {
        return Err(W3DAnimationError::InvalidData("unaligned timecoded payload".into()));
    }
    let data_u32_count = remaining_bytes / 4;
    let mut data = Vec::with_capacity(data_u32_count);
    for _ in 0..data_u32_count {
        let value: u32 = reader.read_le()?;
        data.push(value);
    }

    let Some(kind) = installed_kind(flags) else {
        return Ok(None);
    };
    TimeCodedMotionChannelClass::try_new(u32::from(pivot), kind, num_timecodes, vector_len, data)
        .map(Some)
        .map_err(|error| W3DAnimationError::InvalidData(error.into()))
}

/// Parse an adaptive delta motion channel for compressed animations
/// Reference: motchan.cpp:940-967
fn parse_adaptive_delta_motion_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
    num_frames: u32,
) -> Result<Option<AdaptiveDeltaMotionChannelClass>, W3DAnimationError> {
    const HEADER_SIZE: u32 = 12;
    if chunk_size < HEADER_SIZE {
        return Err(W3DAnimationError::InvalidData(format!(
            "adaptive delta channel too small: {chunk_size}"
        )));
    }

    let channel_num_frames: u32 = reader.read_le()?;
    let pivot: u16 = reader.read_le()?;
    let vector_len_raw: u8 = reader.read_le()?;
    let flags: u8 = reader.read_le()?;
    let scale: f32 = reader.read_le()?;
    let vector_len = usize::from(vector_len_raw);
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;

    // Packets are nine bytes each and need not end on a u32 boundary.
    // Keep the exact byte length so truncated packets cannot acquire padding.
    let mut data = vec![0; remaining_bytes];
    reader.read_exact(&mut data)?;

    if channel_num_frames != num_frames {
        return Err(W3DAnimationError::InvalidData("adaptive frame count differs from animation".into()));
    }
    let Some(kind) = installed_kind(flags) else {
        return Ok(None);
    };
    AdaptiveDeltaMotionChannelClass::try_new(u32::from(pivot), kind, vector_len, channel_num_frames, scale, data)
        .map(Some)
        .map_err(|error| W3DAnimationError::InvalidData(error.into()))
}

/// Parse a timecoded bit channel for visibility
/// Reference: motchan.cpp:754-788
fn parse_timecoded_bit_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<Option<TimeCodedBitChannelClass>, W3DAnimationError> {
    const HEADER_SIZE: u32 = 8;
    if chunk_size < HEADER_SIZE {
        return Err(W3DAnimationError::InvalidData(format!(
            "timecoded bit channel too small: {chunk_size}"
        )));
    }

    let num_timecodes: u32 = reader.read_le()?;
    let pivot: u16 = reader.read_le()?;
    let flags: u8 = reader.read_le()?;
    // Compressed visibility holds its keys; the header default is unused.
    let _default_val: u8 = reader.read_le()?;
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;
    if remaining_bytes % 4 != 0 {
        return Err(W3DAnimationError::InvalidData("unaligned compressed bit payload".into()));
    }

    let num_words = remaining_bytes / 4;
    let mut bits = Vec::with_capacity(num_words);
    for _ in 0..num_words {
        let value: u32 = reader.read_le()?;
        bits.push(value);
    }

    if BitChannelKind::from_flags(u16::from(flags)).is_none() {
        return Ok(None);
    }
    TimeCodedBitChannelClass::try_new(u32::from(pivot), num_timecodes, bits)
        .map(Some)
        .map_err(|error| W3DAnimationError::InvalidData(error.into()))
}

/// Parse a morph animation chunk
/// Reference: w3d_file.cpp morph animation loading (similar to compressed animation loading)
fn parse_morph_animation_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let morph_data = parse_morph_animation_data(reader, chunk_size)?;

    Ok(W3DAnimationData {
        name: morph_data.name.clone(),
        hierarchy_name: morph_data.hierarchy_name.clone(),
        num_frames: morph_data.num_frames,
        frame_rate: morph_data.frame_rate,
        compression_flavor: None,
        raw_anim: None,
        compressed_anim: None,
        morph_anim: Some(morph_data),
    })
}

/// Parse a morph animation channel
/// Reference: w3d_file.cpp morph channel loading
fn parse_morph_channel_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DMorphChannel, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut pose_name = String::new();
    let mut keyframes: Vec<W3DMorphKey> = Vec::new();

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::MorphanimPosename) => {
                // Read pose name
                let mut name_bytes = vec![0u8; sub_header.actual_size() as usize];
                reader.read_exact(&mut name_bytes)?;
                pose_name = ww3d_core::w3d_string_from_bytes(&name_bytes);
            }
            Some(W3DChunkType::MorphanimKeydata) => {
                let num_keyframes = sub_header.actual_size() as usize
                    / std::mem::size_of::<ww3d_core::W3dMorphAnimKeyStruct>();
                for _ in 0..num_keyframes {
                    let key: ww3d_core::W3dMorphAnimKeyStruct = reader.read_le()?;
                    keyframes.push(W3DMorphKey {
                        morph_frame: key.morph_frame,
                        pose_frame: key.pose_frame,
                    });
                }
            }
            _ => {
                reader.seek(SeekFrom::Start(sub_end))?;
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    Ok(W3DMorphChannel {
        pose_name,
        keyframes,
    })
}

/// Parse an HModel chunk (Hierarchical Model)
/// Reference: w3d_file.cpp HModel loading
pub fn load_w3d_hmodel_from_file(path: &str) -> Result<W3DHModelData, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3d_hmodel(&mut file)
}

/// Load HModel from reader
pub fn load_w3d_hmodel<R: Read + Seek>(reader: &mut R) -> Result<W3DHModelData, W3DAnimationError> {
    let file_len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    while reader.stream_position()? + 8 <= file_len {
        let header: W3dChunkHeader = reader.read_le()?;
        let chunk_start = reader.stream_position()?;
        let chunk_end = chunk_start + header.actual_size() as u64;

        match header.chunk_type() {
            Some(W3DChunkType::Hmodel) => {
                return parse_hmodel_chunk(reader, header.actual_size());
            }
            _ => {
                reader.seek(SeekFrom::Start(chunk_end))?;
            }
        }
    }

    Err(W3DAnimationError::MissingChunk(
        "HModel chunk not found in file".to_string(),
    ))
}

/// Parse an HModel chunk
fn parse_hmodel_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DHModelData, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut header: Option<W3dHModelHeaderStruct> = None;
    let mut nodes: Vec<W3DHModelNode> = Vec::new();

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::HmodelHeader) => {
                header = Some(reader.read_le()?);
            }
            Some(W3DChunkType::Node)
            | Some(W3DChunkType::CollisionNode)
            | Some(W3DChunkType::SkinNode) => {
                let node: W3dHModelNodeStruct = reader.read_le()?;
                nodes.push(W3DHModelNode {
                    render_obj_name: ww3d_core::w3d_string_from_bytes(&node.render_obj_name),
                    pivot_idx: node.pivot_idx,
                });
            }
            _ => {
                reader.seek(SeekFrom::Start(sub_end))?;
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header =
        header.ok_or_else(|| W3DAnimationError::MissingChunk("HModel header".to_string()))?;

    Ok(W3DHModelData {
        name: ww3d_core::w3d_string_from_bytes(&header.name),
        hierarchy_name: ww3d_core::w3d_string_from_bytes(&header.hierarchy_name),
        nodes,
    })
}

/// Convert raw animation data using its named authored hierarchy.
pub fn w3d_animation_to_hanim(
    anim_data: W3DAnimationData,
    hierarchy: &HTreeClass,
) -> Result<HAnimClass, W3DAnimationError> {
    if !anim_data.hierarchy_name.eq_ignore_ascii_case(&hierarchy.name) {
        return Err(W3DAnimationError::MissingChunk(format!(
            "Hierarchy {}", anim_data.hierarchy_name
        )));
    }
    if anim_data.compression_flavor.is_some() || anim_data.morph_anim.is_some() {
        return Err(W3DAnimationError::InvalidData("Expected raw animation".into()));
    }
    let raw = anim_data
        .raw_anim
        .ok_or_else(|| W3DAnimationError::InvalidData("Expected raw animation".into()))?;
    Ok(HAnimClass::from_raw(raw.bind_to_hierarchy(hierarchy.num_pivots())))
}

/// Load hierarchy from W3D file
pub fn load_w3d_hierarchy_from_file(path: &str) -> Result<HTreeClass, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3d_hierarchy(&mut file)
}

/// Load hierarchy from reader
pub fn load_w3d_hierarchy<R: Read + Seek>(reader: &mut R) -> Result<HTreeClass, W3DAnimationError> {
    let file_len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    while reader.stream_position()? + 8 <= file_len {
        let header: W3dChunkHeader = reader.read_le()?;
        let chunk_start = reader.stream_position()?;
        let chunk_end = chunk_start + header.actual_size() as u64;

        match header.chunk_type() {
            Some(W3DChunkType::Hierarchy) => {
                return parse_hierarchy_chunk(reader, header.actual_size());
            }
            _ => {
                reader.seek(SeekFrom::Start(chunk_end))?;
            }
        }
    }

    Err(W3DAnimationError::MissingChunk(
        "Hierarchy chunk not found in file".to_string(),
    ))
}

/// Parse a hierarchy chunk
pub(crate) fn parse_hierarchy_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<HTreeClass, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut header: Option<W3dHierarchyStruct> = None;
    let mut pivots: Vec<W3dPivotStruct> = Vec::new();

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::HierarchyHeader) => {
                header = Some(reader.read_le()?);
            }
            Some(W3DChunkType::Pivots) => {
                if let Some(ref hdr) = header {
                    for _ in 0..hdr.num_pivots {
                        let pivot: W3dPivotStruct = reader.read_le()?;
                        pivots.push(pivot);
                    }
                }
            }
            _ => {
                reader.seek(SeekFrom::Start(sub_end))?;
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header =
        header.ok_or_else(|| W3DAnimationError::MissingChunk("Hierarchy header".to_string()))?;

    // Build HTreeClass
    let mut tree = HTreeClass::new();
    tree.name = header.name_str();

    for pivot in pivots {
        let base_transform = pivot.base_transform();
        tree.add_pivot_from_base(&pivot.name_str(), pivot.parent_idx, base_transform);
    }

    if !tree.pivots.is_empty() {
        tree.base_update(glam::Mat4::IDENTITY);
    }

    Ok(tree)
}

/// Container for multiple W3D assets loaded from a .w3c file
#[derive(Debug, Clone)]
pub struct W3CContainerData {
    pub animations: Vec<W3DAnimationData>,
    pub hierarchies: Vec<HTreeClass>,
    pub hmodels: Vec<W3DHModelData>,
    pub morph_animations: Vec<W3DMorphAnimationData>,
}

/// Load a W3C container file (can contain multiple W3D assets)
/// Reference: w3d_file.cpp W3C loading
pub fn load_w3c_from_file(
    path: &str,
    hierarchy_pivots: &dyn Fn(&str) -> Option<usize>,
) -> Result<W3CContainerData, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3c_from_reader(&mut file, hierarchy_pivots)
}

/// Load W3C container from reader
/// W3C files contain multiple W3D chunks (meshes, animations, hierarchies)
pub fn load_w3c_from_reader<R: Read + Seek>(
    reader: &mut R,
    hierarchy_pivots: &dyn Fn(&str) -> Option<usize>,
) -> Result<W3CContainerData, W3DAnimationError> {
    let file_len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    let mut animations = Vec::new();
    let mut hierarchies = Vec::new();
    let mut hmodels = Vec::new();
    let mut morph_animations = Vec::new();

    // W3C files are just concatenated W3D chunks
    while reader.stream_position()? + 8 <= file_len {
        let header: W3dChunkHeader = reader.read_le()?;
        let chunk_start = reader.stream_position()?;
        let chunk_end = chunk_start + header.actual_size() as u64;

        match header.chunk_type() {
            Some(W3DChunkType::Animation) => {
                if let Ok(anim) = parse_animation_chunk(reader, header.actual_size()) {
                    animations.push(anim);
                }
            }
            Some(W3DChunkType::CompressedAnimation) => {
                if let Ok(anim) = parse_compressed_animation_chunk(reader, header.actual_size(), &|name| {
                    hierarchies.iter().find(|tree: &&HTreeClass| tree.name.eq_ignore_ascii_case(name))
                        .map(|tree| tree.num_pivots()).or_else(|| hierarchy_pivots(name))
                }) {
                    animations.push(anim);
                }
            }
            Some(W3DChunkType::MorphAnimation) => {
                if let Ok(anim) = parse_morph_animation_chunk(reader, header.actual_size()) {
                    if let Some(morph) = anim.morph_anim.clone() {
                        morph_animations.push(morph);
                    }
                }
            }
            Some(W3DChunkType::Hierarchy) => {
                if let Ok(hierarchy) = parse_hierarchy_chunk(reader, header.actual_size()) {
                    hierarchies.push(hierarchy);
                }
            }
            Some(W3DChunkType::Hmodel) => {
                if let Ok(hmodel) = parse_hmodel_chunk(reader, header.actual_size()) {
                    hmodels.push(hmodel);
                }
            }
            _ => {
                // Skip other chunk types (meshes, textures, etc.)
            }
        }

        reader.seek(SeekFrom::Start(chunk_end))?;
    }

    Ok(W3CContainerData {
        animations,
        hierarchies,
        hmodels,
        morph_animations,
    })
}

/// Parse morph animation data directly (helper for W3C loading)
fn parse_morph_animation_data<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DMorphAnimationData, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut header: Option<ww3d_core::W3dMorphAnimHeaderStruct> = None;
    let mut channels: Vec<W3DMorphChannel> = Vec::new();
    let mut pivot_channels: Vec<u32> = Vec::new();

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::MorphanimHeader) => {
                header = Some(reader.read_le()?);
            }
            Some(W3DChunkType::MorphanimChannel) => {
                if let Ok(channel) = parse_morph_channel_chunk(reader, sub_header.actual_size()) {
                    channels.push(channel);
                }
            }
            Some(W3DChunkType::MorphanimPivotchanneldata) => {
                let count = sub_header.actual_size() as usize / std::mem::size_of::<u32>();
                for _ in 0..count {
                    pivot_channels.push(reader.read_le()?);
                }
            }
            _ => {
                reader.seek(SeekFrom::Start(sub_end))?;
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header = header
        .ok_or_else(|| W3DAnimationError::MissingChunk("Morph animation header".to_string()))?;

    Ok(W3DMorphAnimationData {
        name: ww3d_core::w3d_string_from_bytes(&header.name),
        hierarchy_name: ww3d_core::w3d_string_from_bytes(&header.hierarchy_name),
        num_frames: header.frame_count,
        frame_rate: header.frame_rate,
        channels,
        pivot_channels,
    })
}

/// Load a W3X hierarchy file (simplified hierarchy format)
/// W3X files are essentially the same as W3D files but specifically for hierarchies
pub fn load_w3x_from_file(path: &str) -> Result<HTreeClass, W3DAnimationError> {
    // W3X files use the same format as W3D hierarchy chunks
    load_w3d_hierarchy_from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;
    use std::io::Cursor;

    fn unit_tree(count: usize) -> HTreeClass {
        let mut tree = HTreeClass::with_name("Unit");
        for i in 0..count {
            tree.add_pivot(&format!("bone{i}"), if i == 0 { -1 } else { 0 }, glam::Vec3::ZERO, Quat::IDENTITY);
        }
        tree
    }

    fn fixed_name(name: &str) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        let input = name.as_bytes();
        let len = input.len().min(bytes.len().saturating_sub(1));
        bytes[..len].copy_from_slice(&input[..len]);
        bytes
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(bytes: &mut Vec<u8>, value: f32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_chunk(bytes: &mut Vec<u8>, chunk_type: W3DChunkType, payload: &[u8]) {
        push_u32(bytes, chunk_type as u32);
        push_u32(bytes, payload.len() as u32);
        bytes.extend_from_slice(payload);
    }

    fn morph_animation_fixture() -> Vec<u8> {
        let mut header = Vec::new();
        push_u32(&mut header, 0x0001);
        header.extend_from_slice(&fixed_name("Talk"));
        header.extend_from_slice(&fixed_name("Face"));
        push_u32(&mut header, 42);
        push_f32(&mut header, 24.0);
        push_u32(&mut header, 2);

        let mut channel_a = Vec::new();
        push_chunk(
            &mut channel_a,
            W3DChunkType::MorphanimPosename,
            b"Face.Phoneme\0",
        );
        let mut keys_a = Vec::new();
        push_u32(&mut keys_a, 0);
        push_u32(&mut keys_a, 3);
        push_u32(&mut keys_a, 10);
        push_u32(&mut keys_a, 8);
        push_chunk(&mut channel_a, W3DChunkType::MorphanimKeydata, &keys_a);

        let mut channel_b = Vec::new();
        push_chunk(
            &mut channel_b,
            W3DChunkType::MorphanimPosename,
            b"Face.Expression\0",
        );
        let mut keys_b = Vec::new();
        push_u32(&mut keys_b, 4);
        push_u32(&mut keys_b, 1);
        push_chunk(&mut channel_b, W3DChunkType::MorphanimKeydata, &keys_b);

        let mut pivot_channels = Vec::new();
        push_u32(&mut pivot_channels, 0);
        push_u32(&mut pivot_channels, 1);
        push_u32(&mut pivot_channels, 1);

        let mut morph_payload = Vec::new();
        push_chunk(&mut morph_payload, W3DChunkType::MorphanimHeader, &header);
        push_chunk(
            &mut morph_payload,
            W3DChunkType::MorphanimChannel,
            &channel_a,
        );
        push_chunk(
            &mut morph_payload,
            W3DChunkType::MorphanimChannel,
            &channel_b,
        );
        push_chunk(
            &mut morph_payload,
            W3DChunkType::MorphanimPivotchanneldata,
            &pivot_channels,
        );

        let mut file = Vec::new();
        push_chunk(&mut file, W3DChunkType::MorphAnimation, &morph_payload);
        file
    }

    fn raw_animation_with_bit_channel_fixture() -> Vec<u8> {
        let mut header = Vec::new();
        push_u32(&mut header, 0x0004_0001);
        header.extend_from_slice(&fixed_name("Blink"));
        header.extend_from_slice(&fixed_name("Unit"));
        push_u32(&mut header, 6);
        push_u32(&mut header, 30);

        let mut bit_channel = Vec::new();
        push_u16(&mut bit_channel, 1);
        push_u16(&mut bit_channel, 4);
        push_u16(&mut bit_channel, 0);
        push_u16(&mut bit_channel, 2);
        bit_channel.push(1);
        bit_channel.push(0b0000_1010);

        let mut anim_payload = Vec::new();
        push_chunk(&mut anim_payload, W3DChunkType::AnimationHeader, &header);
        push_chunk(&mut anim_payload, W3DChunkType::BitChannel, &bit_channel);

        let mut file = Vec::new();
        push_chunk(&mut file, W3DChunkType::Animation, &anim_payload);
        file
    }

    fn compressed_animation_with_timecoded_channels_fixture() -> Vec<u8> {
        let mut header = Vec::new();
        push_u32(&mut header, 0x0001);
        header.extend_from_slice(&fixed_name("Move"));
        header.extend_from_slice(&fixed_name("Unit"));
        push_u32(&mut header, 5);
        push_u16(&mut header, 30);
        push_u16(&mut header, ANIM_FLAVOR_TIMECODED as u16);

        let mut x_channel = Vec::new();
        push_u32(&mut x_channel, 2);
        push_u16(&mut x_channel, 2);
        x_channel.push(1);
        x_channel.push(0);
        push_u32(&mut x_channel, 0);
        push_u32(&mut x_channel, 1.5f32.to_bits());
        push_u32(&mut x_channel, 4);
        push_u32(&mut x_channel, 3.0f32.to_bits());

        let mut bit_channel = Vec::new();
        push_u32(&mut bit_channel, 2);
        push_u16(&mut bit_channel, 3);
        bit_channel.push(0); // BIT_CHANNEL_VIS
        bit_channel.push(1);
        push_u32(&mut bit_channel, 0);
        push_u32(&mut bit_channel, 4 | 0x8000_0000);

        let mut anim_payload = Vec::new();
        push_chunk(
            &mut anim_payload,
            W3DChunkType::CompressedAnimationHeader,
            &header,
        );
        push_chunk(
            &mut anim_payload,
            W3DChunkType::CompressedAnimationChannel,
            &x_channel,
        );
        push_chunk(
            &mut anim_payload,
            W3DChunkType::CompressedBitChannel,
            &bit_channel,
        );

        let mut file = Vec::new();
        push_chunk(&mut file, W3DChunkType::CompressedAnimation, &anim_payload);
        file
    }

    fn compressed_animation_with_adaptive_packet_fixture(packet: &[u8]) -> Vec<u8> {
        let mut header = Vec::new();
        push_u32(&mut header, 0x0001);
        header.extend_from_slice(&fixed_name("Move"));
        header.extend_from_slice(&fixed_name("Unit"));
        push_u32(&mut header, 17);
        push_u16(&mut header, 30);
        push_u16(&mut header, ANIM_FLAVOR_ADAPTIVE_DELTA as u16);

        let mut channel = Vec::new();
        push_u32(&mut channel, 17);
        push_u16(&mut channel, 0);
        channel.push(1);
        channel.push(0);
        push_f32(&mut channel, 1.0);
        push_f32(&mut channel, 2.0);
        channel.extend_from_slice(packet);

        let mut payload = Vec::new();
        push_chunk(&mut payload, W3DChunkType::CompressedAnimationHeader, &header);
        push_chunk(&mut payload, W3DChunkType::CompressedAnimationChannel, &channel);
        let mut file = Vec::new();
        push_chunk(&mut file, W3DChunkType::CompressedAnimation, &payload);
        file
    }

    #[test]
    fn adaptive_w3d_loader_preserves_unaligned_final_packet_byte() {
        let bytes = compressed_animation_with_adaptive_packet_fixture(&[8, 0, 0, 0, 0, 0, 0, 0, 0x21]);
        let mut animation = load_w3d_animation(&mut Cursor::new(bytes), &|_| Some(1))
            .unwrap().compressed_anim.unwrap();
        assert_eq!(animation.get_translation(0, 0.0).x, 2.0);
        assert_eq!(animation.get_translation(0, 15.0).x, 3.0);
        assert_eq!(animation.get_translation(0, 16.0).x, 5.0);
    }

    #[test]
    fn adaptive_w3d_loader_does_not_pad_truncated_packet() {
        let bytes = compressed_animation_with_adaptive_packet_fixture(&[8, 0, 0, 0, 0, 0, 0, 0]);
        assert!(load_w3d_animation(&mut Cursor::new(bytes), &|_| Some(1)).is_err());
    }

    #[test]
    fn test_w3d_animation_data_structure() {
        // Test that W3DAnimationData can hold both compressed and uncompressed data
        let uncompressed = W3DAnimationData {
            name: "TestAnim".to_string(),
            hierarchy_name: "TestSkeleton".to_string(),
            num_frames: 30,
            frame_rate: 30.0,
            compression_flavor: None,
            raw_anim: None,
            compressed_anim: None,
            morph_anim: None,
        };

        assert_eq!(uncompressed.name, "TestAnim");
        assert!(uncompressed.compression_flavor.is_none());
        assert!(uncompressed.compressed_anim.is_none());

        // Test compressed animation structure
        let compressed_anim = HCompressedAnimClass::new(
            "CompressedAnim".to_string(),
            "TestSkeleton".to_string(),
            60,
            10,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );

        let compressed = W3DAnimationData {
            name: "CompressedAnim".to_string(),
            hierarchy_name: "TestSkeleton".to_string(),
            num_frames: 60,
            frame_rate: 30.0,
            compression_flavor: Some(ANIM_FLAVOR_TIMECODED),
            raw_anim: None,
            compressed_anim: Some(compressed_anim),
            morph_anim: None,
        };

        assert_eq!(compressed.name, "CompressedAnim");
        assert_eq!(compressed.compression_flavor, Some(ANIM_FLAVOR_TIMECODED));
        assert!(compressed.compressed_anim.is_some());
    }

    #[test]
    fn w3d_animation_to_hanim_samples_canonical_raw_quaternions() {
        let q0 = Quat::from_rotation_z(0.25);
        let q1 = Quat::from_rotation_z(0.75);
        let mut raw = HRawAnimClass::new("Turn", "Unit", 2, 30);
        raw.add_channel(
            crate::motion_channels::MotionChannelClass::try_new(
                1,
                AnimChannelKind::Quaternion,
                0,
                1,
                vec![q0.x, q0.y, q0.z, q0.w, q1.x, q1.y, q1.z, q1.w],
            )
            .unwrap(),
        );
        let anim_data = W3DAnimationData {
            name: "Turn".to_string(),
            hierarchy_name: "Unit".to_string(),
            num_frames: 2,
            frame_rate: 30.0,
            compression_flavor: None,
            raw_anim: Some(raw),
            compressed_anim: None,
            morph_anim: None,
        };

        let hanim = w3d_animation_to_hanim(anim_data, &unit_tree(4)).unwrap();

        assert!(hanim.has_rotation(1));
        assert_eq!(hanim.get_orientation(1, 0.0), q0);
        assert_eq!(hanim.get_orientation(1, 1.0), q1);
    }

    #[test]
    fn load_w3d_animation_reads_classic_bit_channel_layout() {
        let bytes = raw_animation_with_bit_channel_fixture();
        let mut cursor = Cursor::new(bytes);

        let anim_data = load_w3d_animation(&mut cursor, &|_| None).expect("load raw animation");

        let raw = anim_data.raw_anim.as_ref().expect("raw channels");
        assert!(raw.has_visibility(2));
        assert_eq!(raw.get_num_pivots(), 3);

        let hanim = w3d_animation_to_hanim(anim_data, &unit_tree(4)).unwrap();
        assert!(hanim.get_visibility(2, 0.0));
        assert!(!hanim.get_visibility(2, 1.0));
        assert!(hanim.get_visibility(2, 2.0));
        assert!(!hanim.get_visibility(2, 3.0));
        assert!(hanim.get_visibility(2, 4.0));
        assert!(hanim.get_visibility(2, 5.0));
    }

    #[test]
    fn load_w3d_animation_reads_compressed_header_and_channel_layouts() {
        let bytes = compressed_animation_with_timecoded_channels_fixture();
        let mut cursor = Cursor::new(bytes);

        let anim_data = load_w3d_animation(&mut cursor, &|name| (name == "Unit").then_some(6)).expect("load compressed animation");

        assert_eq!(anim_data.name, "Move");
        assert_eq!(anim_data.hierarchy_name, "Unit");
        assert_eq!(anim_data.num_frames, 5);
        assert_eq!(anim_data.frame_rate, 30.0);
        assert_eq!(anim_data.compression_flavor, Some(ANIM_FLAVOR_TIMECODED));

        let mut compressed = anim_data.compressed_anim.expect("compressed animation");
        assert_eq!(compressed.get_name(), "Move");
        assert_eq!(compressed.get_hname(), "Unit");
        assert_eq!(compressed.get_num_frames(), 5);
        assert_eq!(compressed.get_frame_rate(), 30.0);
        assert_eq!(compressed.get_flavor(), ANIM_FLAVOR_TIMECODED);
        assert_eq!(compressed.get_num_pivots(), 6);
        assert!(compressed.has_x_translation(2));
        assert!(compressed.has_visibility(3));
        assert_eq!(compressed.get_translation(2, 0.0).x, 1.5);
        assert!(!compressed.get_visibility(3, 0.0));
        assert!(compressed.get_visibility(3, 4.0));
    }

    #[test]
    fn named_hierarchy_controls_raw_and_compressed_admission_and_bounds() {
        let raw = raw_animation_with_bit_channel_fixture();
        let data = load_w3d_animation(&mut Cursor::new(&raw), &|_| None).unwrap();
        let wrong_tree = HTreeClass::with_name("Other");
        assert!(w3d_animation_to_hanim(data.clone(), &wrong_tree).is_err());
        let small = w3d_animation_to_hanim(data.clone(), &unit_tree(2)).unwrap();
        assert_eq!(small.num_pivots(), 2);
        assert!(!small.has_visibility(2));
        let large = w3d_animation_to_hanim(data, &unit_tree(5)).unwrap();
        assert_eq!(large.num_pivots(), 5);
        assert!(large.has_visibility(2));
        assert!(large.get_visibility(4, 0.0));
        let mut tree = unit_tree(6);
        tree.pivots[4].is_visible = false;
        tree.pivots[5].is_visible = false;
        let mut object = crate::Animatable3DObjClass::new(tree);
        object.set_animation(Some(std::sync::Arc::new(large)), 1.0, crate::AnimationMode::Manual, 0.0);
        object.update_sub_object_transforms();
        assert!(!object.htree.get_visibility(2));
        assert!(object.htree.get_visibility(4));
        assert!(!object.htree.get_visibility(5));

        let compressed = compressed_animation_with_timecoded_channels_fixture();
        assert!(load_w3d_animation(&mut Cursor::new(&compressed), &|_| None).is_err());
        let small = load_w3d_animation(&mut Cursor::new(&compressed), &|_| Some(3))
            .unwrap().compressed_anim.unwrap();
        assert_eq!(small.get_num_pivots(), 3);
        assert!(!small.has_visibility(3));
    }

    #[test]
    fn model_loader_resolves_animation_hierarchy_from_preloaded_owner() {
        let raw = raw_animation_with_bit_channel_fixture();
        let tree = unit_tree(5);
        let model = crate::W3DModel::load_from_reader(
            &mut Cursor::new(&raw),
            "clip",
            &|name| name.eq_ignore_ascii_case("Unit").then_some(&tree),
        ).unwrap();
        let anim = model.animations.get("Blink").unwrap();
        assert_eq!(anim.num_pivots(), 5);
        assert!(!anim.get_visibility(2, 1.0));

        let missing = crate::W3DModel::load_from_reader(
            &mut Cursor::new(&raw), "clip", &|_| None,
        );
        assert!(matches!(&missing, Err(W3DAnimationError::MissingChunk(name)) if name == "Hierarchy Unit"));

        let wrong = HTreeClass::with_name("Other");
        let mismatch = crate::W3DModel::load_from_reader(
            &mut Cursor::new(&raw), "clip", &|_| Some(&wrong),
        );
        assert!(matches!(&mismatch, Err(W3DAnimationError::MissingChunk(name)) if name == "Hierarchy Unit"));
    }

    #[test]
    fn model_loader_resolves_hierarchy_that_follows_animation_in_file() {
        let mut bytes = raw_animation_with_bit_channel_fixture();
        let mut header = Vec::new();
        push_u32(&mut header, 0x0003_0000);
        header.extend_from_slice(&fixed_name("Unit"));
        push_u32(&mut header, 5);
        for _ in 0..3 {
            push_f32(&mut header, 0.0);
        }
        let mut pivots = Vec::new();
        for index in 0..5 {
            pivots.extend_from_slice(&fixed_name(&format!("bone{index}")));
            pivots.extend_from_slice(&(if index == 0 { -1_i32 } else { 0 }).to_le_bytes());
            for _ in 0..6 {
                push_f32(&mut pivots, 0.0);
            }
        }
        let mut hierarchy = Vec::new();
        push_chunk(&mut hierarchy, W3DChunkType::HierarchyHeader, &header);
        push_chunk(&mut hierarchy, W3DChunkType::Pivots, &pivots);
        push_chunk(&mut bytes, W3DChunkType::Hierarchy, &hierarchy);

        let model = crate::W3DModel::load_from_reader(
            &mut Cursor::new(&bytes), "clip", &|_| None,
        ).unwrap();
        let anim = model.animations.get("Blink").unwrap();
        assert_eq!(anim.num_pivots(), 5);
        assert!(!anim.get_visibility(2, 1.0));
        assert!(anim.get_visibility(4, 1.0));
    }

    #[test]
    fn test_hmodel_data_structure() {
        let hmodel = W3DHModelData {
            name: "TestModel".to_string(),
            hierarchy_name: "TestSkeleton".to_string(),
            nodes: vec![
                W3DHModelNode {
                    render_obj_name: "Mesh01".to_string(),
                    pivot_idx: 0,
                },
                W3DHModelNode {
                    render_obj_name: "Mesh02".to_string(),
                    pivot_idx: 1,
                },
            ],
        };

        assert_eq!(hmodel.name, "TestModel");
        assert_eq!(hmodel.nodes.len(), 2);
        assert_eq!(hmodel.nodes[0].render_obj_name, "Mesh01");
        assert_eq!(hmodel.nodes[1].pivot_idx, 1);
    }

    #[test]
    fn test_w3c_container_structure() {
        let container = W3CContainerData {
            animations: vec![],
            hierarchies: vec![],
            hmodels: vec![],
            morph_animations: vec![],
        };

        assert_eq!(container.animations.len(), 0);
        assert_eq!(container.hierarchies.len(), 0);
        assert_eq!(container.hmodels.len(), 0);
        assert_eq!(container.morph_animations.len(), 0);
    }

    #[test]
    fn test_morph_animation_structure() {
        let morph_anim = W3DMorphAnimationData {
            name: "FacialAnim".to_string(),
            hierarchy_name: "HeadSkeleton".to_string(),
            num_frames: 30,
            frame_rate: 30.0,
            channels: vec![W3DMorphChannel {
                pose_name: "Smile".to_string(),
                keyframes: vec![
                    W3DMorphKey {
                        morph_frame: 0,
                        pose_frame: 0,
                    },
                    W3DMorphKey {
                        morph_frame: 15,
                        pose_frame: 4,
                    },
                ],
            }],
            pivot_channels: vec![0, 0, 1],
        };

        assert_eq!(morph_anim.name, "FacialAnim");
        assert_eq!(morph_anim.channels.len(), 1);
        assert_eq!(morph_anim.channels[0].pose_name, "Smile");
        assert_eq!(morph_anim.channels[0].keyframes[1].pose_frame, 4);
        assert_eq!(morph_anim.pivot_channels, vec![0, 0, 1]);
    }

    #[test]
    fn load_w3d_animation_preserves_morph_channels_keys_and_pivot_map() {
        let fixture = morph_animation_fixture();
        let mut cursor = Cursor::new(fixture);
        let anim = load_w3d_animation(&mut cursor, &|_| None).expect("morph animation should parse");
        let morph = anim.morph_anim.expect("morph payload should be retained");

        assert_eq!(anim.name, "Talk");
        assert_eq!(anim.hierarchy_name, "Face");
        assert_eq!(anim.num_frames, 42);
        assert_eq!(anim.frame_rate, 24.0);
        assert_eq!(morph.name, "Talk");
        assert_eq!(morph.hierarchy_name, "Face");
        assert_eq!(morph.frame_rate, 24.0);
        assert_eq!(morph.channels.len(), 2);
        assert_eq!(morph.channels[0].pose_name, "Face.Phoneme");
        assert_eq!(
            morph.channels[0].keyframes,
            vec![
                W3DMorphKey {
                    morph_frame: 0,
                    pose_frame: 3,
                },
                W3DMorphKey {
                    morph_frame: 10,
                    pose_frame: 8,
                },
            ]
        );
        assert_eq!(morph.channels[1].pose_name, "Face.Expression");
        assert_eq!(
            morph.channels[1].keyframes,
            vec![W3DMorphKey {
                morph_frame: 4,
                pose_frame: 1,
            }]
        );
        assert_eq!(morph.pivot_channels, vec![0, 1, 1]);
    }

    #[test]
    fn load_w3c_stores_morph_animations_without_reparse_loss() {
        let fixture = morph_animation_fixture();
        let mut cursor = Cursor::new(fixture);
        let container = load_w3c_from_reader(&mut cursor, &|_| None).expect("container should parse");

        assert_eq!(container.animations.len(), 0);
        assert_eq!(container.morph_animations.len(), 1);
        assert_eq!(container.morph_animations[0].name, "Talk");
        assert_eq!(container.morph_animations[0].channels.len(), 2);
        assert_eq!(container.morph_animations[0].pivot_channels, vec![0, 1, 1]);
    }
}
