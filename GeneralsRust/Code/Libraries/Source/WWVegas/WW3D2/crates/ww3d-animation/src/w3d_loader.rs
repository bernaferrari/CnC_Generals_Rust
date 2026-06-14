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
    W3DChunkType, W3dAnimChannelStruct, W3dAnimationStruct, W3dBitChannelStruct, W3dChunkHeader,
    W3dCompressedAnimHeaderStruct, W3dHModelHeaderStruct, W3dHModelNodeStruct, W3dHierarchyStruct,
    W3dPivotStruct,
};

use crate::hanim::{BitChannel, HAnimClass, MotionChannel, MotionChannelType};
use crate::hcompressed_anim::{
    HCompressedAnimClass, ANIM_FLAVOR_ADAPTIVE_DELTA, ANIM_FLAVOR_TIMECODED,
};
use crate::htree::HTreeClass;
use crate::motion_channels::{
    AdaptiveDeltaMotionChannelClass, TimeCodedBitChannelClass, TimeCodedMotionChannelClass,
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
    pub channels: Vec<W3DAnimationChannel>,
    pub bit_channels: Vec<W3DAnimationBitChannel>,
    pub compressed_anim: Option<HCompressedAnimClass>, // NEW: Holds compressed animation if present
    pub morph_anim: Option<W3DMorphAnimationData>,
}

/// Animation channel with keyframe data
#[derive(Debug, Clone)]
pub struct W3DAnimationChannel {
    pub pivot_index: u16,
    pub channel_type: MotionChannelType,
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: usize,
    pub data: Vec<f32>,
}

/// Packed visibility channel data.
#[derive(Debug, Clone)]
pub struct W3DAnimationBitChannel {
    pub pivot_index: u16,
    pub channel_type: MotionChannelType,
    pub first_frame: u16,
    pub last_frame: u16,
    pub default_value: u8,
    pub data: Vec<u8>,
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
pub fn load_w3d_animation_from_file(path: &str) -> Result<W3DAnimationData, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3d_animation(&mut file)
}

/// Load a W3D animation from a reader
pub fn load_w3d_animation<R: Read + Seek>(
    reader: &mut R,
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
                return parse_compressed_animation_chunk(reader, header.actual_size());
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

/// Parse an uncompressed animation chunk
fn parse_animation_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut header: Option<W3dAnimationStruct> = None;
    let mut channels: Vec<W3DAnimationChannel> = Vec::new();
    let mut bit_channels: Vec<W3DAnimationBitChannel> = Vec::new();

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::AnimationHeader) => {
                header = Some(reader.read_le()?);
            }
            Some(W3DChunkType::AnimationChannel) => {
                let channel = parse_animation_channel_chunk(reader, sub_header.actual_size())?;
                channels.push(channel);
            }
            Some(W3DChunkType::BitChannel) => {
                let channel = parse_bit_channel_chunk(reader, sub_header.actual_size())?;
                bit_channels.push(channel);
            }
            Some(W3DChunkType::TimeCodedAnimChannel) => {
                let channel = parse_timecoded_channel_chunk(reader, sub_header.actual_size())?;
                channels.push(channel);
            }
            _ => {
                // Skip unknown sub-chunks
                reader.seek(SeekFrom::Start(sub_end))?;
            }
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header =
        header.ok_or_else(|| W3DAnimationError::MissingChunk("Animation header".to_string()))?;

    Ok(W3DAnimationData {
        name: header.name_str(),
        hierarchy_name: header.hiera_name_str(),
        num_frames: header.num_frames,
        frame_rate: header.frame_rate as f32,
        compression_flavor: None, // Uncompressed animation
        channels,
        bit_channels,
        compressed_anim: None,
        morph_anim: None,
    })
}

/// Parse an animation channel chunk
fn parse_animation_channel_chunk<R: Read + Seek>(
    reader: &mut R,
    _chunk_size: u32,
) -> Result<W3DAnimationChannel, W3DAnimationError> {
    // Read the channel header
    let channel_struct: W3dAnimChannelStruct = reader.read_le()?;

    // Determine the channel type
    let channel_type = MotionChannelType::from_flags(channel_struct.flags);

    // Calculate the number of frames
    let num_frames = (channel_struct.last_frame - channel_struct.first_frame + 1) as usize;
    let vector_len = channel_struct.vector_len.max(1) as usize;
    let data_count = num_frames * vector_len;

    // Read the animation data
    let mut data = Vec::with_capacity(data_count);
    for _ in 0..data_count {
        let value: f32 = reader.read_le()?;
        data.push(value);
    }

    Ok(W3DAnimationChannel {
        pivot_index: channel_struct.pivot,
        channel_type,
        first_frame: channel_struct.first_frame,
        last_frame: channel_struct.last_frame,
        vector_len,
        data,
    })
}

/// Parse a bit channel chunk (visibility channel)
fn parse_bit_channel_chunk<R: Read + Seek>(
    reader: &mut R,
    _chunk_size: u32,
) -> Result<W3DAnimationBitChannel, W3DAnimationError> {
    // Read the channel header
    let channel_struct: W3dBitChannelStruct = reader.read_le()?;

    let num_frames = (channel_struct.last_frame - channel_struct.first_frame + 1) as usize;
    let num_bytes = ((num_frames + 7) / 8).max(1);
    let mut data = Vec::with_capacity(num_bytes);
    data.push(channel_struct.data[0]);
    if num_bytes > 1 {
        data.resize(num_bytes, 0);
        reader.read_exact(&mut data[1..])?;
    }

    Ok(W3DAnimationBitChannel {
        pivot_index: channel_struct.pivot,
        channel_type: MotionChannelType::Visibility,
        first_frame: channel_struct.first_frame,
        last_frame: channel_struct.last_frame,
        default_value: channel_struct.default_val,
        data,
    })
}

/// Parse a time-coded animation channel chunk
fn parse_timecoded_channel_chunk<R: Read + Seek>(
    reader: &mut R,
    _chunk_size: u32,
) -> Result<W3DAnimationChannel, W3DAnimationError> {
    // Read the channel header
    let channel_struct: W3dAnimChannelStruct = reader.read_le()?;

    let channel_type = MotionChannelType::from_flags(channel_struct.flags);
    let vector_len = channel_struct.vector_len.max(1) as usize;

    // Time-coded channels store keyframe count first
    let keyframe_count: u32 = reader.read_le()?;

    let mut data = Vec::new();

    // Read time-coded keyframes (time, value pairs)
    for _ in 0..keyframe_count {
        let _time: f32 = reader.read_le()?; // Time value (could be used for interpolation)

        // Read vector data
        for _ in 0..vector_len {
            let value: f32 = reader.read_le()?;
            data.push(value);
        }
    }

    Ok(W3DAnimationChannel {
        pivot_index: channel_struct.pivot,
        channel_type,
        first_frame: channel_struct.first_frame,
        last_frame: channel_struct.last_frame,
        vector_len,
        data,
    })
}

/// Parse a compressed animation chunk with full HCompressedAnimClass integration
/// Reference: w3d_file.cpp compressed animation loading
fn parse_compressed_animation_chunk<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<W3DAnimationData, W3DAnimationError> {
    let chunk_end = reader.stream_position()? + chunk_size as u64;

    let mut header: Option<W3dCompressedAnimHeaderStruct> = None;

    // First pass: read header to create HCompressedAnimClass
    let start_pos = reader.stream_position()?;

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        if let Some(W3DChunkType::CompressedAnimationHeader) = sub_header.chunk_type() {
            header = Some(reader.read_le()?);
            break;
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    let header = header.ok_or_else(|| {
        W3DAnimationError::MissingChunk("Compressed animation header".to_string())
    })?;

    // First pass: find the maximum pivot index to determine num_nodes
    // This allows us to calculate the correct number of bones from the channels themselves
    let mut max_pivot_index = 0u32;
    reader.seek(SeekFrom::Start(start_pos))?;

    while reader.stream_position()? < chunk_end {
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;
        let chunk_size = sub_header.actual_size();

        match sub_header.chunk_type() {
            Some(W3DChunkType::CompressedAnimationChannel) => {
                if chunk_size >= 6 {
                    let _count: u32 = reader.read_le()?;
                    let pivot_index: u16 = reader.read_le()?;
                    max_pivot_index = max_pivot_index.max(pivot_index as u32);
                }
            }
            Some(W3DChunkType::CompressedBitChannel) => {
                if chunk_size >= 6 {
                    let _num_timecodes: u32 = reader.read_le()?;
                    let pivot_index: u16 = reader.read_le()?;
                    max_pivot_index = max_pivot_index.max(pivot_index as u32);
                }
            }
            _ => {}
        }

        reader.seek(SeekFrom::Start(sub_end))?;
    }

    // Calculate num_nodes from maximum pivot index (add 1 to account for 0-based indexing)
    let num_nodes = (max_pivot_index + 1).max(1) as usize;

    // Create HCompressedAnimClass with calculated num_nodes
    let flavor = u32::from(header.flavor);
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
        let sub_header: W3dChunkHeader = reader.read_le()?;
        let sub_start = reader.stream_position()?;
        let sub_end = sub_start + sub_header.actual_size() as u64;

        match sub_header.chunk_type() {
            Some(W3DChunkType::CompressedAnimationChannel) => {
                match flavor {
                    ANIM_FLAVOR_TIMECODED => {
                        if let Ok(channel) =
                            parse_timecoded_motion_channel(reader, sub_header.actual_size())
                        {
                            compressed_anim.add_timecoded_channel(channel);
                        }
                    }
                    ANIM_FLAVOR_ADAPTIVE_DELTA => {
                        if let Ok(channel) = parse_adaptive_delta_motion_channel(
                            reader,
                            sub_header.actual_size(),
                            header.num_frames,
                        ) {
                            compressed_anim.add_adaptive_delta_channel(channel);
                        }
                    }
                    _ => {
                        // Skip unsupported flavors
                    }
                }
            }
            Some(W3DChunkType::CompressedBitChannel) => {
                if let Ok(channel) = parse_timecoded_bit_channel(reader, sub_header.actual_size()) {
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
        channels: Vec::new(), // Compressed animations use HCompressedAnimClass instead
        bit_channels: Vec::new(),
        compressed_anim: Some(compressed_anim),
        morph_anim: None,
    })
}

/// Parse a timecoded motion channel for compressed animations
/// Reference: motchan.cpp:386-413
fn parse_timecoded_motion_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<TimeCodedMotionChannelClass, W3DAnimationError> {
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
    let vector_len = usize::from(vector_len_raw.max(1));
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;
    let data_u32_count = remaining_bytes / 4;
    let mut data = Vec::with_capacity(data_u32_count);
    for _ in 0..data_u32_count {
        let value: u32 = reader.read_le()?;
        data.push(value);
    }

    Ok(TimeCodedMotionChannelClass::new(
        pivot as u32,
        flags as u16,
        num_timecodes,
        vector_len,
        data,
    ))
}

/// Parse an adaptive delta motion channel for compressed animations
/// Reference: motchan.cpp:940-967
fn parse_adaptive_delta_motion_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
    num_frames: u32,
) -> Result<AdaptiveDeltaMotionChannelClass, W3DAnimationError> {
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
    let vector_len = usize::from(vector_len_raw.max(1));
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;

    let data_u32_count = remaining_bytes / 4;
    let mut data = Vec::with_capacity(data_u32_count);
    for _ in 0..data_u32_count {
        let value: u32 = reader.read_le()?;
        data.push(value);
    }

    Ok(AdaptiveDeltaMotionChannelClass::new(
        pivot as u32,
        flags as u16,
        vector_len,
        if channel_num_frames == 0 {
            num_frames
        } else {
            channel_num_frames
        },
        scale,
        data,
    ))
}

/// Parse a timecoded bit channel for visibility
/// Reference: motchan.cpp:754-788
fn parse_timecoded_bit_channel<R: Read + Seek>(
    reader: &mut R,
    chunk_size: u32,
) -> Result<TimeCodedBitChannelClass, W3DAnimationError> {
    const HEADER_SIZE: u32 = 8;
    if chunk_size < HEADER_SIZE {
        return Err(W3DAnimationError::InvalidData(format!(
            "timecoded bit channel too small: {chunk_size}"
        )));
    }

    let num_timecodes: u32 = reader.read_le()?;
    let pivot: u16 = reader.read_le()?;
    let flags: u8 = reader.read_le()?;
    let default_val: u8 = reader.read_le()?;
    let remaining_bytes = (chunk_size - HEADER_SIZE) as usize;

    let num_words = remaining_bytes / 4;
    let mut bits = Vec::with_capacity(num_timecodes as usize);
    for _ in 0..num_words {
        let value: u32 = reader.read_le()?;
        bits.push(value);
    }

    Ok(TimeCodedBitChannelClass::new(
        pivot as u32,
        flags as u16,
        default_val as i32,
        num_timecodes,
        bits,
    ))
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
        channels: Vec::new(), // Morph animations don't use standard channels
        bit_channels: Vec::new(),
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

/// Convert W3D animation data to HAnimClass
pub fn w3d_animation_to_hanim(anim_data: W3DAnimationData) -> HAnimClass {
    let channels: Vec<MotionChannel> = anim_data
        .channels
        .into_iter()
        .map(|channel| {
            MotionChannel::new(
                channel.channel_type,
                channel.pivot_index as usize,
                channel.first_frame,
                channel.last_frame,
                channel.vector_len,
                channel.data,
            )
        })
        .collect();
    let bit_channels: Vec<BitChannel> = anim_data
        .bit_channels
        .into_iter()
        .map(|channel| BitChannel {
            pivot_idx: channel.pivot_index as usize,
            channel_type: channel.channel_type,
            data: channel.data,
            first_frame: channel.first_frame,
            last_frame: channel.last_frame,
            default_value: channel.default_value,
        })
        .collect();

    HAnimClass::with_channels(
        &anim_data.name,
        &anim_data.hierarchy_name,
        anim_data.num_frames,
        anim_data.frame_rate,
        channels,
        bit_channels,
    )
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
fn parse_hierarchy_chunk<R: Read + Seek>(
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
pub fn load_w3c_from_file(path: &str) -> Result<W3CContainerData, W3DAnimationError> {
    let mut file = std::fs::File::open(path)?;
    load_w3c_from_reader(&mut file)
}

/// Load W3C container from reader
/// W3C files contain multiple W3D chunks (meshes, animations, hierarchies)
pub fn load_w3c_from_reader<R: Read + Seek>(
    reader: &mut R,
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
                if let Ok(anim) = parse_compressed_animation_chunk(reader, header.actual_size()) {
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
        push_u32(&mut header, 0x0001);
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
        bit_channel.push(1);
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

    #[test]
    fn test_channel_type_detection() {
        // Test that we can detect different channel types from flags
        let flags_translation = 0x0000; // Translation X
        let flags_rotation = 0x0006; // Rotation (quaternion)
        let flags_visibility = 15; // Visibility

        let ct_translation = MotionChannelType::from_flags(flags_translation);
        let ct_rotation = MotionChannelType::from_flags(flags_rotation);
        let ct_visibility = MotionChannelType::from_flags(flags_visibility);

        assert!(matches!(ct_translation, MotionChannelType::Translation(_)));
        assert!(matches!(ct_rotation, MotionChannelType::Quaternion));
        assert!(matches!(ct_visibility, MotionChannelType::Visibility));
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
            channels: vec![],
            bit_channels: vec![],
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
            channels: vec![],
            bit_channels: vec![],
            compressed_anim: Some(compressed_anim),
            morph_anim: None,
        };

        assert_eq!(compressed.name, "CompressedAnim");
        assert_eq!(compressed.compression_flavor, Some(ANIM_FLAVOR_TIMECODED));
        assert!(compressed.compressed_anim.is_some());
    }

    #[test]
    fn test_w3d_animation_to_hanim_preserves_quaternion_vector_len() {
        let q0 = Quat::from_rotation_z(0.25);
        let q1 = Quat::from_rotation_z(0.75);
        let anim_data = W3DAnimationData {
            name: "Turn".to_string(),
            hierarchy_name: "Unit".to_string(),
            num_frames: 2,
            frame_rate: 30.0,
            compression_flavor: None,
            channels: vec![W3DAnimationChannel {
                pivot_index: 1,
                channel_type: MotionChannelType::Quaternion,
                first_frame: 0,
                last_frame: 1,
                vector_len: 4,
                data: vec![q0.x, q0.y, q0.z, q0.w, q1.x, q1.y, q1.z, q1.w],
            }],
            bit_channels: vec![],
            compressed_anim: None,
            morph_anim: None,
        };

        let hanim = w3d_animation_to_hanim(anim_data);

        assert!(hanim.has_quaternion_rotation(1));
        assert!(hanim.get_orientation(1, 0.0).abs_diff_eq(q0, 0.00001));
        assert!(hanim.get_orientation(1, 1.0).abs_diff_eq(q1, 0.00001));
    }

    #[test]
    fn test_w3d_animation_to_hanim_installs_packed_visibility_bits() {
        let anim_data = W3DAnimationData {
            name: "Blink".to_string(),
            hierarchy_name: "Unit".to_string(),
            num_frames: 6,
            frame_rate: 30.0,
            compression_flavor: None,
            channels: vec![],
            bit_channels: vec![W3DAnimationBitChannel {
                pivot_index: 2,
                channel_type: MotionChannelType::Visibility,
                first_frame: 1,
                last_frame: 4,
                default_value: 1,
                data: vec![0b0000_1010],
            }],
            compressed_anim: None,
            morph_anim: None,
        };

        let hanim = w3d_animation_to_hanim(anim_data);

        assert!(hanim.has_visibility(2));
        assert!(hanim.get_visibility(2, 0.0));
        assert!(!hanim.get_visibility(2, 1.0));
        assert!(hanim.get_visibility(2, 2.0));
        assert!(!hanim.get_visibility(2, 3.0));
        assert!(hanim.get_visibility(2, 4.0));
        assert!(hanim.get_visibility(2, 5.0));
    }

    #[test]
    fn load_w3d_animation_reads_classic_bit_channel_layout() {
        let bytes = raw_animation_with_bit_channel_fixture();
        let mut cursor = Cursor::new(bytes);

        let anim_data = load_w3d_animation(&mut cursor).expect("load raw animation");

        assert_eq!(anim_data.bit_channels.len(), 1);
        assert_eq!(anim_data.bit_channels[0].pivot_index, 2);
        assert_eq!(anim_data.bit_channels[0].default_value, 1);
        assert_eq!(anim_data.bit_channels[0].data, vec![0b0000_1010]);

        let hanim = w3d_animation_to_hanim(anim_data);
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

        let anim_data = load_w3d_animation(&mut cursor).expect("load compressed animation");

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
        assert_eq!(compressed.get_num_pivots(), 4);
        assert!(compressed.has_x_translation(2));
        assert!(compressed.has_visibility(3));
        assert_eq!(compressed.get_translation(2, 0.0).x, 1.5);
        assert!(!compressed.get_visibility(3, 0.0));
        assert!(compressed.get_visibility(3, 4.0));
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
        let anim = load_w3d_animation(&mut cursor).expect("morph animation should parse");
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
        let container = load_w3c_from_reader(&mut cursor).expect("container should parse");

        assert_eq!(container.animations.len(), 0);
        assert_eq!(container.morph_animations.len(), 1);
        assert_eq!(container.morph_animations[0].name, "Talk");
        assert_eq!(container.morph_animations[0].channels.len(), 2);
        assert_eq!(container.morph_animations[0].pivot_channels, vec![0, 1, 1]);
    }
}
