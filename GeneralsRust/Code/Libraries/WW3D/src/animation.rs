// Animation System
// Ported from hanim.h, hrawanim.h

use crate::math::*;
use crate::hierarchy::HTree;
use crate::w3d_file::*;
use crate::{Result};
use std::io::Read;

// Animation channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationChannelType {
    TranslationX,
    TranslationY,
    TranslationZ,
    Quaternion,
    Visibility,
}

// Animation channel data
pub struct AnimationChannel {
    pub channel_type: AnimationChannelType,
    pub pivot: u16,
    pub first_frame: u32,
    pub last_frame: u32,
    pub data: Vec<f32>,
    pub time_coded: bool,
}

impl AnimationChannel {
    pub fn new(channel_type: AnimationChannelType, pivot: u16) -> Self {
        Self {
            channel_type,
            pivot,
            first_frame: 0,
            last_frame: 0,
            data: Vec::new(),
            time_coded: false,
        }
    }

    pub fn get_value(&self, frame: f32) -> f32 {
        if self.data.is_empty() {
            return 0.0;
        }

        let frame = frame.clamp(self.first_frame as f32, self.last_frame as f32);
        let local_frame = frame - self.first_frame as f32;

        if self.time_coded {
            // Time-coded animation - data is stored as [time, value] pairs
            self.interpolate_time_coded(local_frame)
        } else {
            // Linear animation - one value per frame
            let index = local_frame as usize;
            if index >= self.data.len() {
                self.data[self.data.len() - 1]
            } else if index == local_frame as usize {
                self.data[index]
            } else {
                // Linear interpolation
                let t = local_frame - local_frame.floor();
                let v0 = self.data[index];
                let v1 = if index + 1 < self.data.len() {
                    self.data[index + 1]
                } else {
                    v0
                };
                v0 + (v1 - v0) * t
            }
        }
    }

    fn interpolate_time_coded(&self, frame: f32) -> f32 {
        // Find the two keyframes that bracket the current frame
        let mut i = 0;
        while i < self.data.len() / 2 - 1 {
            let t1 = self.data[i * 2];
            if frame < t1 {
                break;
            }
            i += 1;
        }

        if i == 0 {
            return self.data[1];
        }

        let t0 = self.data[(i - 1) * 2];
        let t1 = self.data[i * 2];
        let v0 = self.data[(i - 1) * 2 + 1];
        let v1 = self.data[i * 2 + 1];

        let t = (frame - t0) / (t1 - t0);
        v0 + (v1 - v0) * t
    }
}

// Base trait for all animations
pub trait HAnimation: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_hierarchy_name(&self) -> &str;
    fn get_num_frames(&self) -> u32;
    fn get_frame_rate(&self) -> f32;
    fn get_total_time(&self) -> f32 {
        self.get_num_frames() as f32 / self.get_frame_rate()
    }

    fn get_translation(&self, pivot_idx: usize, frame: f32) -> Vec3;
    fn get_orientation(&self, pivot_idx: usize, frame: f32) -> UnitQuat;
    fn get_visibility(&self, pivot_idx: usize, frame: f32) -> bool;

    fn get_num_pivots(&self) -> usize;
    fn is_node_motion_present(&self, pivot_idx: usize) -> bool;
}

// Raw hierarchical animation
pub struct HRawAnimation {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: u32,
    pub channels: Vec<AnimationChannel>,
}

impl HRawAnimation {
    pub fn new(name: String, hierarchy_name: String) -> Self {
        Self {
            name,
            hierarchy_name,
            num_frames: 0,
            frame_rate: 30,
            channels: Vec::new(),
        }
    }

    pub fn load_w3d<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        loop {
            let header = match W3DChunkHeader::read(reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            match W3DChunkType::from_u32(header.chunk_type) {
                Some(W3DChunkType::AnimationHeader) => self.read_header(reader)?,
                Some(W3DChunkType::AnimationChannel) => self.read_channel(reader, header.chunk_size)?,
                _ => {
                    let mut buf = vec![0u8; header.chunk_size as usize];
                    reader.read_exact(&mut buf)?;
                }
            }
        }

        Ok(())
    }

    fn read_header<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        let mut buf = [0u8; std::mem::size_of::<W3DAnimationHeader>()];
        reader.read_exact(&mut buf)?;

        let name_bytes = &buf[4..20];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        self.name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let hierarchy_bytes = &buf[20..36];
        let hierarchy_end = hierarchy_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        self.hierarchy_name = String::from_utf8_lossy(&hierarchy_bytes[..hierarchy_end]).to_string();

        self.num_frames = u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
        self.frame_rate = u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]);

        Ok(())
    }

    fn read_channel<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let mut buf = [0u8; std::mem::size_of::<W3DAnimationChannelHeader>()];
        reader.read_exact(&mut buf)?;

        let first_frame = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let last_frame = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let vector_len = u16::from_le_bytes([buf[8], buf[9]]);
        let flags = u16::from_le_bytes([buf[10], buf[11]]);
        let pivot = u16::from_le_bytes([buf[12], buf[13]]);

        let channel_type = match flags & 0x7FFF {
            W3D_ANIMATION_CHANNEL_X => AnimationChannelType::TranslationX,
            W3D_ANIMATION_CHANNEL_Y => AnimationChannelType::TranslationY,
            W3D_ANIMATION_CHANNEL_Z => AnimationChannelType::TranslationZ,
            W3D_ANIMATION_CHANNEL_Q => AnimationChannelType::Quaternion,
            _ => AnimationChannelType::TranslationX,
        };

        let time_coded = (flags & W3D_ANIMATION_CHANNEL_TIMECODED) != 0;

        let data_size = size - std::mem::size_of::<W3DAnimationChannelHeader>() as u32;
        let num_values = data_size as usize / 4;
        let mut data = Vec::with_capacity(num_values);

        for _ in 0..num_values {
            let mut value_buf = [0u8; 4];
            reader.read_exact(&mut value_buf)?;
            data.push(f32::from_le_bytes(value_buf));
        }

        let mut channel = AnimationChannel::new(channel_type, pivot);
        channel.first_frame = first_frame;
        channel.last_frame = last_frame;
        channel.data = data;
        channel.time_coded = time_coded;

        self.channels.push(channel);

        Ok(())
    }

    fn find_channel(&self, pivot: usize, channel_type: AnimationChannelType) -> Option<&AnimationChannel> {
        self.channels.iter().find(|c| c.pivot as usize == pivot && c.channel_type == channel_type)
    }
}

impl HAnimation for HRawAnimation {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hierarchy_name(&self) -> &str {
        &self.hierarchy_name
    }

    fn get_num_frames(&self) -> u32 {
        self.num_frames
    }

    fn get_frame_rate(&self) -> f32 {
        self.frame_rate as f32
    }

    fn get_translation(&self, pivot_idx: usize, frame: f32) -> Vec3 {
        let x = self.find_channel(pivot_idx, AnimationChannelType::TranslationX)
            .map(|c| c.get_value(frame))
            .unwrap_or(0.0);
        let y = self.find_channel(pivot_idx, AnimationChannelType::TranslationY)
            .map(|c| c.get_value(frame))
            .unwrap_or(0.0);
        let z = self.find_channel(pivot_idx, AnimationChannelType::TranslationZ)
            .map(|c| c.get_value(frame))
            .unwrap_or(0.0);

        Vec3::new(x, y, z)
    }

    fn get_orientation(&self, pivot_idx: usize, frame: f32) -> UnitQuat {
        if let Some(channel) = self.find_channel(pivot_idx, AnimationChannelType::Quaternion) {
            let frame_int = frame.floor() as usize;
            if frame_int * 4 + 3 < channel.data.len() {
                let x = channel.data[frame_int * 4];
                let y = channel.data[frame_int * 4 + 1];
                let z = channel.data[frame_int * 4 + 2];
                let w = channel.data[frame_int * 4 + 3];
                return UnitQuat::from_quaternion(Quat::new(w, x, y, z));
            }
        }

        UnitQuat::identity()
    }

    fn get_visibility(&self, pivot_idx: usize, frame: f32) -> bool {
        self.find_channel(pivot_idx, AnimationChannelType::Visibility)
            .map(|c| c.get_value(frame) > 0.5)
            .unwrap_or(true)
    }

    fn get_num_pivots(&self) -> usize {
        self.channels.iter().map(|c| c.pivot as usize).max().unwrap_or(0) + 1
    }

    fn is_node_motion_present(&self, pivot_idx: usize) -> bool {
        self.channels.iter().any(|c| c.pivot as usize == pivot_idx)
    }
}

// Apply animation to hierarchy tree
pub fn apply_animation(htree: &mut HTree, animation: &dyn HAnimation, frame: f32, root: &Mat4) {
    if htree.pivots.is_empty() {
        return;
    }

    // Update root
    let translation = if animation.is_node_motion_present(0) {
        animation.get_translation(0, frame)
    } else {
        htree.pivots[0].translation
    };

    let rotation = if animation.is_node_motion_present(0) {
        animation.get_orientation(0, frame)
    } else {
        htree.pivots[0].rotation
    };

    htree.pivots[0].transform = *root * matrix_from_rotation_translation(&rotation, &translation);
    htree.pivots[0].is_visible = animation.get_visibility(0, frame);

    // Update children
    for i in 1..htree.num_pivots {
        if htree.pivots[i].is_captured {
            // Use captured transform
            if let Some(control_transform) = htree.pivots[i].control_transform {
                let parent_idx = htree.pivots[i].parent_idx as usize;
                htree.pivots[i].transform = htree.pivots[parent_idx].transform * control_transform;
            }
        } else {
            // Use animation transform
            let translation = if animation.is_node_motion_present(i) {
                animation.get_translation(i, frame)
            } else {
                htree.pivots[i].translation
            };

            let rotation = if animation.is_node_motion_present(i) {
                animation.get_orientation(i, frame)
            } else {
                htree.pivots[i].rotation
            };

            let parent_idx = htree.pivots[i].parent_idx as usize;
            let local_transform = matrix_from_rotation_translation(&rotation, &translation);
            htree.pivots[i].transform = htree.pivots[parent_idx].transform * local_transform;
        }

        htree.pivots[i].is_visible = animation.get_visibility(i, frame);
    }
}
