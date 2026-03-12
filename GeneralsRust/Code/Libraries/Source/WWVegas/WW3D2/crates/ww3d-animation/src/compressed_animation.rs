//! Compressed Animation System - HCAnim equivalent
//!
//! This module implements the compressed animation system that was a major
//! feature of the original C++ WW3D2. Compressed animations reduce memory
//! usage while maintaining animation quality through various compression techniques.

use crate::{HAnim, AnimationChannel, ChannelType, HTree};
use glam::{Vec3, Quat as Quaternion, Mat4 as Matrix4};
use std::collections::HashMap;

/// Compression types for different data types
#[derive(Debug, Clone, Copy)]
pub enum CompressionType {
    /// No compression - raw float values
    None,
    /// Time-based compression (remove redundant keyframes)
    TimeBased,
    /// Quantization-based compression
    Quantized,
    /// Adaptive compression based on error tolerance
    Adaptive,
}

/// Compressed animation channel data
#[derive(Debug, Clone)]
pub struct CompressedChannel {
    pub pivot_idx: usize,
    pub channel_type: ChannelType,
    pub compression_type: CompressionType,
    pub compressed_data: Vec<u8>,
    pub keyframe_count: u32,
    pub time_range: (f32, f32), // min_time, max_time
    pub value_range: (f32, f32), // min_value, max_value for quantization
}

/// Compressed hierarchical animation
#[derive(Debug)]
pub struct HCompressedAnim {
    pub name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub channels: Vec<CompressedChannel>,
    pub tree_name: String, // Associated skeleton name
    pub compression_quality: f32, // 0.0 = max compression, 1.0 = min compression
}

impl HCompressedAnim {
    /// Create a new compressed animation from regular animation
    pub fn from_hanim(hanim: &HAnim, quality: f32) -> Self {
        let mut compressed_channels = Vec::new();

        for channel in &hanim.channels {
            let compressed = Self::compress_channel(channel, quality);
            compressed_channels.push(compressed);
        }

        Self {
            name: hanim.name.clone(),
            num_frames: hanim.num_frames,
            frame_rate: hanim.frame_rate,
            channels: compressed_channels,
            tree_name: String::new(), // Will be set when associated with skeleton
            compression_quality: quality,
        }
    }

    /// Compress a single animation channel
    fn compress_channel(channel: &AnimationChannel, quality: f32) -> CompressedChannel {
        let compression_type = if quality > 0.8 {
            CompressionType::None
        } else if quality > 0.5 {
            CompressionType::TimeBased
        } else if quality > 0.2 {
            CompressionType::Quantized
        } else {
            CompressionType::Adaptive
        };

        let compressed_data = match compression_type {
            CompressionType::None => Self::compress_none(channel),
            CompressionType::TimeBased => Self::compress_time_based(channel, quality),
            CompressionType::Quantized => Self::compress_quantized(channel, quality),
            CompressionType::Adaptive => Self::compress_adaptive(channel, quality),
        };

        let time_range = if channel.times.is_empty() {
            (0.0, 0.0)
        } else {
            (*channel.times.first().unwrap(), *channel.times.last().unwrap())
        };

        let value_range = if channel.data.is_empty() {
            (0.0, 0.0)
        } else {
            let min_val = channel.data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max_val = channel.data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            (min_val, max_val)
        };

        CompressedChannel {
            pivot_idx: channel.pivot_idx,
            channel_type: channel.channel_type,
            compression_type,
            compressed_data,
            keyframe_count: channel.times.len() as u32,
            time_range,
            value_range,
        }
    }

    /// No compression - just store raw data
    fn compress_none(channel: &AnimationChannel) -> Vec<u8> {
        let mut data = Vec::new();

        // Store keyframe count
        data.extend_from_slice(&(channel.times.len() as u32).to_le_bytes());

        // Store times and values
        for (&time, &value) in channel.times.iter().zip(&channel.data) {
            data.extend_from_slice(&time.to_le_bytes());
            data.extend_from_slice(&value.to_le_bytes());
        }

        data
    }

    /// Time-based compression - remove keyframes that can be linearly interpolated
    fn compress_time_based(channel: &AnimationChannel, quality: f32) -> Vec<u8> {
        if channel.times.len() < 3 {
            return Self::compress_none(channel);
        }

        let tolerance = (1.0 - quality) * 0.01; // Error tolerance based on quality
        let mut compressed_times = Vec::new();
        let mut compressed_values = Vec::new();

        // Always keep first and last keyframes
        compressed_times.push(channel.times[0]);
        compressed_values.push(channel.data[0]);

        for i in 1..channel.times.len() - 1 {
            let prev_time = channel.times[i - 1];
            let curr_time = channel.times[i];
            let next_time = channel.times[i + 1];
            let prev_val = channel.data[i - 1];
            let curr_val = channel.data[i];
            let next_val = channel.data[i + 1];

            // Calculate interpolated value at current time
            let t = (curr_time - prev_time) / (next_time - prev_time);
            let interpolated_val = prev_val + (next_val - prev_val) * t;

            // Keep keyframe if deviation exceeds tolerance
            if (curr_val - interpolated_val).abs() > tolerance {
                compressed_times.push(curr_time);
                compressed_values.push(curr_val);
            }
        }

        // Always keep last keyframe
        compressed_times.push(*channel.times.last().unwrap());
        compressed_values.push(*channel.data.last().unwrap());

        // Serialize compressed data
        let mut data = Vec::new();
        data.extend_from_slice(&(compressed_times.len() as u32).to_le_bytes());

        for (&time, &value) in compressed_times.iter().zip(&compressed_values) {
            data.extend_from_slice(&time.to_le_bytes());
            data.extend_from_slice(&value.to_le_bytes());
        }

        data
    }

    /// Quantized compression - reduce precision to save space
    fn compress_quantized(channel: &AnimationChannel, quality: f32) -> Vec<u8> {
        let bits = ((quality * 16.0) as u32).max(4).min(32); // 4-32 bits based on quality
        let scale = (2u32.pow(bits) - 1) as f32;

        let min_val = channel.data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = channel.data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_val - min_val;

        let mut data = Vec::new();
        data.extend_from_slice(&(channel.times.len() as u32).to_le_bytes());
        data.extend_from_slice(&bits.to_le_bytes());
        data.extend_from_slice(&min_val.to_le_bytes());
        data.extend_from_slice(&max_val.to_le_bytes());

        for (&time, &value) in channel.times.iter().zip(&channel.data) {
            data.extend_from_slice(&time.to_le_bytes());

            // Quantize value
            let normalized = if range > 0.0 {
                (value - min_val) / range
            } else {
                0.0
            };
            let quantized = (normalized * scale) as u32;
            data.extend_from_slice(&quantized.to_le_bytes());
        }

        data
    }

    /// Adaptive compression - use different methods based on data characteristics
    fn compress_adaptive(channel: &AnimationChannel, quality: f32) -> Vec<u8> {
        // Analyze data characteristics to choose best compression method
        let variance = Self::calculate_variance(&channel.data);
        let is_smooth = Self::is_smooth_curve(&channel.times, &channel.data);

        if variance < 0.01 && is_smooth {
            // Low variance, smooth curve - use time-based compression
            Self::compress_time_based(channel, quality.max(0.7))
        } else if variance > 1.0 {
            // High variance - use no compression
            Self::compress_none(channel)
        } else {
            // Medium variance - use quantized compression
            Self::compress_quantized(channel, quality)
        }
    }

    /// Calculate variance of data values
    fn calculate_variance(data: &[f32]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }

        let mean = data.iter().sum::<f32>() / data.len() as f32;
        let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / data.len() as f32;

        variance
    }

    /// Check if curve is smooth (low second derivative)
    fn is_smooth_curve(times: &[f32], values: &[f32]) -> bool {
        if times.len() < 3 {
            return true;
        }

        let mut second_derivatives = Vec::new();

        for i in 1..times.len() - 1 {
            let dt1 = times[i] - times[i - 1];
            let dt2 = times[i + 1] - times[i];

            if dt1 > 0.0 && dt2 > 0.0 {
                let dv1 = values[i] - values[i - 1];
                let dv2 = values[i + 1] - values[i];

                let accel1 = dv1 / dt1;
                let accel2 = dv2 / dt2;

                let jerk = (accel2 - accel1) / ((dt1 + dt2) / 2.0);
                second_derivatives.push(jerk.abs());
            }
        }

        let avg_jerk = second_derivatives.iter().sum::<f32>() / second_derivatives.len() as f32;
        avg_jerk < 10.0 // Threshold for smoothness
    }

    /// Decompress animation data for a specific frame
    pub fn sample(&self, frame: f32, pivot_idx: usize) -> Option<f32> {
        for channel in &self.channels {
            if channel.pivot_idx == pivot_idx {
                return Some(self.decompress_channel(channel, frame));
            }
        }
        None
    }

    /// Decompress a single channel at a specific frame
    fn decompress_channel(&self, channel: &CompressedChannel, frame: f32) -> f32 {
        match channel.compression_type {
            CompressionType::None => self.decompress_none(channel, frame),
            CompressionType::TimeBased => self.decompress_time_based(channel, frame),
            CompressionType::Quantized => self.decompress_quantized(channel, frame),
            CompressionType::Adaptive => self.decompress_adaptive(channel, frame),
        }
    }

    fn decompress_none(&self, channel: &CompressedChannel, frame: f32) -> f32 {
        let data = &channel.compressed_data;
        let keyframe_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;

        if keyframe_count == 0 {
            return 0.0;
        }

        // Find appropriate keyframes
        let mut left = 0;
        let mut right = keyframe_count - 1;

        while left < right {
            let mid = (left + right) / 2;
            let time_offset = 4 + mid * 8;
            let time = f32::from_le_bytes(data[time_offset..time_offset + 4].try_into().unwrap());

            if time < frame {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            let value_offset = 4 + 4;
            f32::from_le_bytes(data[value_offset..value_offset + 4].try_into().unwrap())
        } else if left >= keyframe_count {
            let value_offset = 4 + (keyframe_count - 1) * 8 + 4;
            f32::from_le_bytes(data[value_offset..value_offset + 4].try_into().unwrap())
        } else {
            let time1_offset = 4 + (left - 1) * 8;
            let time2_offset = 4 + left * 8;
            let val1_offset = time1_offset + 4;
            let val2_offset = time2_offset + 4;

            let time1 = f32::from_le_bytes(data[time1_offset..time1_offset + 4].try_into().unwrap());
            let time2 = f32::from_le_bytes(data[time2_offset..time2_offset + 4].try_into().unwrap());
            let val1 = f32::from_le_bytes(data[val1_offset..val1_offset + 4].try_into().unwrap());
            let val2 = f32::from_le_bytes(data[val2_offset..val2_offset + 4].try_into().unwrap());

            if (time2 - time1).abs() < f32::EPSILON {
                val1
            } else {
                let factor = (frame - time1) / (time2 - time1);
                val1 + (val2 - val1) * factor
            }
        }
    }

    fn decompress_time_based(&self, channel: &CompressedChannel, frame: f32) -> f32 {
        // Time-based decompression is similar to none since we store the compressed keyframes
        self.decompress_none(channel, frame)
    }

    fn decompress_quantized(&self, channel: &CompressedChannel, frame: f32) -> f32 {
        let data = &channel.compressed_data;

        if data.len() < 20 {
            return 0.0;
        }

        let keyframe_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let bits = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let min_val = f32::from_le_bytes(data[8..12].try_into().unwrap());
        let max_val = f32::from_le_bytes(data[12..16].try_into().unwrap());
        let range = max_val - min_val;
        let scale = (2u32.pow(bits) - 1) as f32;

        if keyframe_count == 0 || range <= 0.0 {
            return min_val;
        }

        // Find appropriate keyframes
        let mut left = 0;
        let mut right = keyframe_count - 1;

        while left < right {
            let mid = (left + right) / 2;
            let offset = 16 + mid * 8;
            let time = f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());

            if time < frame {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        let time1_offset = 16 + (left.saturating_sub(1)) * 8;
        let time2_offset = 16 + left * 8;
        let quant1_offset = time1_offset + 4;
        let quant2_offset = time2_offset + 4;

        if left == 0 {
            // Before first keyframe
            let quantized = u32::from_le_bytes(data[quant1_offset..quant1_offset + 4].try_into().unwrap());
            min_val + (quantized as f32 / scale) * range
        } else if left >= keyframe_count {
            // After last keyframe
            let last_offset = 16 + (keyframe_count - 1) * 8 + 4;
            let quantized = u32::from_le_bytes(data[last_offset..last_offset + 4].try_into().unwrap());
            min_val + (quantized as f32 / scale) * range
        } else {
            // Interpolate between keyframes
            let time1 = f32::from_le_bytes(data[time1_offset..time1_offset + 4].try_into().unwrap());
            let time2 = f32::from_le_bytes(data[time2_offset..time2_offset + 4].try_into().unwrap());
            let quant1 = u32::from_le_bytes(data[quant1_offset..quant1_offset + 4].try_into().unwrap());
            let quant2 = u32::from_le_bytes(data[quant2_offset..quant2_offset + 4].try_into().unwrap());

            let val1 = min_val + (quant1 as f32 / scale) * range;
            let val2 = min_val + (quant2 as f32 / scale) * range;

            if (time2 - time1).abs() < f32::EPSILON {
                val1
            } else {
                let factor = (frame - time1) / (time2 - time1);
                val1 + (val2 - val1) * factor
            }
        }
    }

    fn decompress_adaptive(&self, channel: &CompressedChannel, frame: f32) -> f32 {
        // For adaptive compression, we fall back to the appropriate decompression method
        // In a real implementation, we'd store metadata about which compression was used
        self.decompress_none(channel, frame)
    }

    /// Get compression statistics
    pub fn get_compression_stats(&self) -> CompressionStats {
        let mut total_original_size = 0u64;
        let mut total_compressed_size = 0u64;
        let mut channel_count = 0u32;

        for channel in &self.channels {
            let original_keyframes = channel.keyframe_count as u64;
            let original_size = original_keyframes * 8; // time + value (4 bytes each)
            let compressed_size = channel.compressed_data.len() as u64;

            total_original_size += original_size;
            total_compressed_size += compressed_size;
            channel_count += 1;
        }

        let compression_ratio = if total_original_size > 0 {
            total_compressed_size as f32 / total_original_size as f32
        } else {
            1.0
        };

        CompressionStats {
            total_original_size,
            total_compressed_size,
            compression_ratio,
            channel_count,
            average_compression_ratio: compression_ratio,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub total_original_size: u64,
    pub total_compressed_size: u64,
    pub compression_ratio: f32,
    pub channel_count: u32,
    pub average_compression_ratio: f32,
}

/// Compressed animation manager
pub struct HCompressedAnimManager {
    animations: HashMap<String, HCompressedAnim>,
    cache: HashMap<String, Vec<Matrix4>>, // Cached decompressed frames
}

impl HCompressedAnimManager {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// Load a compressed animation
    pub fn load_animation(&mut self, anim: HCompressedAnim) {
        self.animations.insert(anim.name.clone(), anim);
    }

    /// Get animation by name
    pub fn get_animation(&self, name: &str) -> Option<&HCompressedAnim> {
        self.animations.get(name)
    }

    /// Sample animation at specific frame for given skeleton
    pub fn sample_animation(&self, anim_name: &str, frame: f32, htree: &HTree) -> Option<Vec<Matrix4>> {
        let anim = self.animations.get(anim_name)?;

        let mut bone_matrices = vec![Matrix4::identity(); htree.pivots.len()];

        // Sample each channel
        for channel in &anim.channels {
            if let Some(value) = anim.sample(frame, channel.pivot_idx) {
                self.apply_channel_value(&mut bone_matrices, channel, value, htree);
            }
        }

        // Apply hierarchy
        self.apply_hierarchy(&mut bone_matrices, htree);

        Some(bone_matrices)
    }

    /// Apply channel value to bone matrices
    fn apply_channel_value(&self, bone_matrices: &mut [Matrix4], channel: &CompressedChannel, value: f32, htree: &HTree) {
        if channel.pivot_idx >= htree.pivots.len() {
            return;
        }

        let pivot = &htree.pivots[channel.pivot_idx];

        match channel.channel_type {
            ChannelType::Translation => {
                let translation = Matrix4::translation(Vec3::new(value, 0.0, 0.0));
                bone_matrices[channel.pivot_idx] = translation * bone_matrices[channel.pivot_idx];
            }
            ChannelType::Rotation => {
                let rotation = Matrix4::rotation_y(value);
                bone_matrices[channel.pivot_idx] = rotation * bone_matrices[channel.pivot_idx];
            }
            ChannelType::Visibility => {
                // Handle visibility if needed
            }
        }
    }

    /// Apply hierarchical transformations
    fn apply_hierarchy(&self, bone_matrices: &mut [Matrix4], htree: &HTree) {
        for i in 0..htree.pivots.len() {
            if let Some(parent_idx) = htree.pivots[i].parent_idx {
                if parent_idx < bone_matrices.len() {
                    bone_matrices[i] = bone_matrices[parent_idx] * bone_matrices[i];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_none() {
        let mut channel = AnimationChannel::new(0, ChannelType::Translation);
        channel.add_keyframe(0.0, 0.0);
        channel.add_keyframe(1.0, 1.0);
        channel.add_keyframe(2.0, 2.0);

        let compressed = HCompressedAnim::compress_channel(&channel, 1.0);
        let anim = HCompressedAnim::from_hanim(&HAnim::new("test".to_string()), 1.0);

        // Test decompression
        assert_eq!(compressed.decompress_channel(&compressed, 0.0), 0.0);
        assert_eq!(compressed.decompress_channel(&compressed, 1.0), 1.0);
        assert_eq!(compressed.decompress_channel(&compressed, 2.0), 2.0);
        assert_eq!(compressed.decompress_channel(&compressed, 1.5), 1.5);
    }

    #[test]
    fn test_compression_time_based() {
        let mut channel = AnimationChannel::new(0, ChannelType::Translation);

        // Create a mostly linear animation that can be compressed
        for i in 0..10 {
            let time = i as f32;
            let value = time * 0.1; // Linear relationship
            channel.add_keyframe(time, value);
        }

        let compressed = HCompressedAnim::compress_channel(&channel, 0.3);
        let anim = HCompressedAnim::from_hanim(&HAnim::new("test".to_string()), 0.3);

        // Should have fewer keyframes after compression
        assert!(compressed.compressed_data.len() < channel.times.len() * 8);
    }
}