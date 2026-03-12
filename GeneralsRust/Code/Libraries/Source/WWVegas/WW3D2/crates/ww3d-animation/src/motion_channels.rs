//! Motion Channel Classes - Direct C++ Port
//!
//! This module implements the exact motion channel compression algorithms from the C++
//! codebase. These are critical for loading and playing game animations.
//!
//! Referenced C++ files:
//! - motchan.h (lines 217-333)
//! - motchan.cpp (lines 313-1328)
//!
//! ## TimeCodedMotionChannelClass
//! Uses binary search for keyframe lookup with time-coded packets.
//! Each packet contains: [timecode, value1, value2, ..., valueN]
//!
//! ## AdaptiveDeltaMotionChannelClass
//! Uses adaptive delta compression with a filter table and nibble-packed deltas.
//! Data format: [header floats][packets of filter_index + 16 nibbles]

use glam::Quat;
use std::sync::OnceLock;

// From motchan.cpp:57-79 - Filter table constants
const FILTER_TABLE_SIZE: usize = 256;
const FILTER_TABLE_GEN_START: usize = 16;
const FILTER_TABLE_GEN_SIZE: usize = FILTER_TABLE_SIZE - FILTER_TABLE_GEN_START;

// From motchan.cpp:983 - Packet size for adaptive delta
const PACKET_SIZE: usize = 9; // 1 filter byte + 8 data bytes

// W3D flags from w3d_file.h
const W3D_TIMECODED_BINARY_MOVEMENT_FLAG: u32 = 0x80000000;
const W3D_TIMECODED_BIT_MASK: u32 = 0x80000000;

// Channel types from w3d_file.h
#[allow(dead_code)]
const ANIM_CHANNEL_X: u16 = 0;
#[allow(dead_code)]
const ANIM_CHANNEL_Y: u16 = 1;
#[allow(dead_code)]
const ANIM_CHANNEL_Z: u16 = 2;
#[allow(dead_code)]
const ANIM_CHANNEL_Q: u16 = 6;

// Bit channel type
#[allow(dead_code)]
const BIT_CHANNEL_VIS: u16 = 0;

/// Fast quaternion slerp with lerp approximation for small angles
fn fast_slerp(q0: Quat, q1: Quat, t: f32) -> Quat {
    let dot = q0.dot(q1);
    let q1_adjusted = if dot < 0.0 { -q1 } else { q1 };
    let dot_abs = dot.abs();

    if dot_abs > 0.9995 {
        q0.lerp(q1_adjusted, t).normalize()
    } else {
        q0.slerp(q1_adjusted, t)
    }
}

/// Generate the filter table used by adaptive delta decompression
/// Reference: motchan.cpp:865-879
fn generate_filter_table() -> [f32; FILTER_TABLE_SIZE] {
    let mut table = [0.0f32; FILTER_TABLE_SIZE];

    // First 16 hardcoded values from motchan.cpp:61-77
    let hardcoded = [
        0.00000001f32,
        0.0000001,
        0.000001,
        0.00001,
        0.0001,
        0.001,
        0.01,
        0.1,
        1.0,
        10.0,
        100.0,
        1000.0,
        10000.0,
        100000.0,
        1000000.0,
        10000000.0,
    ];
    table[0..16].copy_from_slice(&hardcoded);

    // Generate the rest using sine curve (motchan.cpp:868-875)
    for i in 0..FILTER_TABLE_GEN_SIZE {
        let ratio = (i + 1) as f32 / FILTER_TABLE_GEN_SIZE as f32;
        table[i + FILTER_TABLE_GEN_START] = 1.0 - (ratio * std::f32::consts::FRAC_PI_2).sin();
    }

    table
}

/// Global filter table for adaptive delta decompression
static FILTER_TABLE: OnceLock<[f32; FILTER_TABLE_SIZE]> = OnceLock::new();

fn get_filter_table() -> &'static [f32; FILTER_TABLE_SIZE] {
    FILTER_TABLE.get_or_init(generate_filter_table)
}

/// Time-coded motion channel with binary search lookup
/// Reference: motchan.h:217-253, motchan.cpp:313-679
#[derive(Debug, Clone)]
pub struct TimeCodedMotionChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    vector_len: usize,
    packet_size: usize,
    num_timecodes: u32,
    last_timecode_idx: usize,
    cached_idx: usize,
    /// Packed data: alternating [timecode, value1, value2, ..., valueN] packets
    data: Vec<u32>,
}

impl TimeCodedMotionChannelClass {
    /// Create from W3D chunk data
    /// Reference: motchan.cpp:386-413
    pub fn new(
        pivot_idx: u32,
        channel_type: u16,
        num_timecodes: u32,
        vector_len: usize,
        data: Vec<u32>,
    ) -> Self {
        let packet_size = vector_len + 1; // timecode + values
        let last_timecode_idx = ((num_timecodes - 1) as usize) * packet_size;

        Self {
            pivot_idx,
            channel_type,
            vector_len,
            packet_size,
            num_timecodes,
            last_timecode_idx,
            cached_idx: 0,
            data,
        }
    }

    pub fn get_type(&self) -> u16 {
        self.channel_type
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    /// Get a scalar vector value at the specified frame
    /// Reference: motchan.cpp:428-479
    pub fn get_vector(&mut self, frame: f32, setvec: &mut [f32]) {
        let tc0 = frame as u32;
        let pidx = self.get_index(tc0);

        // Check if we're at the last packet (motchan.cpp:438-450)
        if pidx == (self.num_timecodes - 1) as usize * self.packet_size {
            // Last packet - no interpolation
            let frm_offset = pidx + 1;
            for i in 0..self.vector_len.min(setvec.len()) {
                setvec[i] = f32::from_bits(self.data[frm_offset + i]);
            }
            return;
        }

        let p2idx = pidx + self.packet_size;
        let time = self.data[p2idx];

        // Check for binary movement flag (motchan.cpp:457-463)
        if (time & W3D_TIMECODED_BINARY_MOVEMENT_FLAG) != 0 {
            let frm_offset = pidx + 1;
            for i in 0..self.vector_len.min(setvec.len()) {
                setvec[i] = f32::from_bits(self.data[frm_offset + i]);
            }
            return;
        }

        // Interpolate between two keyframes (motchan.cpp:465-477)
        let time1 = (self.data[pidx] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let time2 = (time & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let ratio = (frame - time1) / (time2 - time1);

        let frame1_offset = pidx + 1;
        let frame2_offset = p2idx + 1;

        for i in 0..self.vector_len.min(setvec.len()) {
            let val1 = f32::from_bits(self.data[frame1_offset + i]);
            let val2 = f32::from_bits(self.data[frame2_offset + i]);
            setvec[i] = val1 + (val2 - val1) * ratio; // WWMath::Lerp
        }
    }

    /// Get a quaternion vector at the specified frame with slerp interpolation
    /// Reference: motchan.cpp:482-538
    pub fn get_quat_vector(&mut self, frame: f32) -> Quat {
        debug_assert_eq!(
            self.vector_len, 4,
            "Quaternion channels must have vector_len=4"
        );

        let tc0 = frame as u32;
        let pidx = self.get_index(tc0);

        // Check if we're at the last packet
        if pidx == (self.num_timecodes - 1) as usize * self.packet_size {
            let vec_offset = pidx + 1;
            let x = f32::from_bits(self.data[vec_offset]);
            let y = f32::from_bits(self.data[vec_offset + 1]);
            let z = f32::from_bits(self.data[vec_offset + 2]);
            let w = f32::from_bits(self.data[vec_offset + 3]);
            return Quat::from_xyzw(x, y, z, w);
        }

        let p2idx = pidx + self.packet_size;
        let time = self.data[p2idx];

        // Check for binary movement flag
        if (time & W3D_TIMECODED_BINARY_MOVEMENT_FLAG) != 0 {
            let vec_offset = pidx + 1;
            let x = f32::from_bits(self.data[vec_offset]);
            let y = f32::from_bits(self.data[vec_offset + 1]);
            let z = f32::from_bits(self.data[vec_offset + 2]);
            let w = f32::from_bits(self.data[vec_offset + 3]);
            return Quat::from_xyzw(x, y, z, w);
        }

        // Interpolate using slerp (motchan.cpp:520-536)
        let time1 = (self.data[pidx] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let time2 = (time & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let ratio = (frame - time1) / (time2 - time1);

        let frame1_offset = pidx + 1;
        let frame2_offset = p2idx + 1;

        let q1 = Quat::from_xyzw(
            f32::from_bits(self.data[frame1_offset]),
            f32::from_bits(self.data[frame1_offset + 1]),
            f32::from_bits(self.data[frame1_offset + 2]),
            f32::from_bits(self.data[frame1_offset + 3]),
        );

        let q2 = Quat::from_xyzw(
            f32::from_bits(self.data[frame2_offset]),
            f32::from_bits(self.data[frame2_offset + 1]),
            f32::from_bits(self.data[frame2_offset + 2]),
            f32::from_bits(self.data[frame2_offset + 3]),
        );

        fast_slerp(q1, q2, ratio)
    }

    /// Binary search to find the packet index for a given timecode
    /// Reference: motchan.cpp:555-609
    fn binary_search_index(&self, timecode: u32) -> usize {
        let mut left_idx = 0;
        let mut right_idx = self.num_timecodes as isize - 2;

        // Special case: check last packet first (motchan.cpp:563-570)
        let idx = self.last_timecode_idx;
        let time = self.data[idx] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
        if timecode >= time {
            return idx;
        }

        loop {
            let dx = (right_idx - left_idx) >> 1; // divide by 2
            let dx = dx + left_idx;
            let idx = (dx as usize) * self.packet_size;

            let time = self.data[idx] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time {
                right_idx = dx;
                continue;
            }

            let time_next = self.data[idx + self.packet_size] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time_next {
                return idx;
            }

            if left_idx != dx {
                left_idx = dx;
                continue;
            }

            // If leftIdx == dx prior to assignment, then leftIdx is stuck
            left_idx += 1;
        }
    }

    /// Get the packet index for a timecode with caching
    /// Reference: motchan.cpp:624-651
    fn get_index(&mut self, timecode: u32) -> usize {
        debug_assert!(self.cached_idx <= self.last_timecode_idx);

        let time = self.data[self.cached_idx] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;

        if timecode >= time {
            // Possibly in the current packet

            // Special case for end packets
            if self.cached_idx == self.last_timecode_idx {
                return self.cached_idx;
            }

            let time_next =
                self.data[self.cached_idx + self.packet_size] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time_next {
                return self.cached_idx;
            }

            // Do one time look-ahead before reverting to a search (motchan.cpp:640-644)
            self.cached_idx += self.packet_size;
            if self.cached_idx == self.last_timecode_idx {
                return self.cached_idx;
            }

            let time_next =
                self.data[self.cached_idx + self.packet_size] & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time_next {
                return self.cached_idx;
            }
        }

        // Fall back to binary search
        self.cached_idx = self.binary_search_index(timecode);
        self.cached_idx
    }
}

/// Adaptive delta motion channel with nibble-packed delta compression
/// Reference: motchan.h:255-293, motchan.cpp:840-1272
#[derive(Debug, Clone)]
pub struct AdaptiveDeltaMotionChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    vector_len: usize,
    num_frames: u32,
    scale: f32,
    cache_frame: u32,
    cache_data: Vec<f32>, // CacheFrame and CacheFrame+1, by VectorLen
    /// Packed data: [header floats][compressed packets]
    data: Vec<u32>,
}

impl AdaptiveDeltaMotionChannelClass {
    /// Create from W3D chunk data
    /// Reference: motchan.cpp:940-967
    pub fn new(
        pivot_idx: u32,
        channel_type: u16,
        vector_len: usize,
        num_frames: u32,
        scale: f32,
        data: Vec<u32>,
    ) -> Self {
        // Initialize filter table on first use
        let _ = get_filter_table();

        Self {
            pivot_idx,
            channel_type,
            vector_len,
            num_frames,
            scale,
            cache_frame: 0x7FFFFFFF, // a big number, so we know it's not valid
            cache_data: vec![0.0; vector_len * 2], // cached frame & cached frame+1
            data,
        }
    }

    pub fn get_type(&self) -> u16 {
        self.channel_type
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    /// Decompress from the beginning up to frame_idx
    /// Reference: motchan.cpp:984-1054
    fn decompress(&self, frame_idx: u32, outdata: &mut [f32]) {
        // Start over from the beginning
        let base_ptr = self.data.as_ptr() as *const f32;
        let base = unsafe { std::slice::from_raw_parts(base_ptr, self.vector_len) };

        for vi in 0..self.vector_len {
            // Decompress all vector indices
            let mut p_packet = unsafe {
                (self.data.as_ptr() as *const u8)
                    .add(std::mem::size_of::<f32>() * self.vector_len) // skip header
                    .add(PACKET_SIZE * vi) // skip to appropriate packet start
            };

            let mut last_value = base[vi];
            let mut frame: u32 = 1;

            while frame <= frame_idx {
                // Frame loop
                let filter_index = unsafe { *p_packet } as usize;
                p_packet = unsafe { p_packet.add(1) }; // skip to nibble compressed data

                let filter = get_filter_table()[filter_index] * self.scale;

                // Data is grouped in sets of 16 nibbles (motchan.cpp:1008-1042)
                for fi in 0..16 {
                    let pi = fi >> 1; // packet index

                    // Extract nibble (4 bits)
                    let mut factor = unsafe { *p_packet.add(pi) } as i32;
                    if (fi & 1) != 0 {
                        factor >>= 4;
                    }

                    // Sign extend (motchan.cpp:1023-1026)
                    factor &= 0xF;
                    if (factor & 0x8) != 0 {
                        factor |= 0xFFFFFFF0u32 as i32;
                    }

                    let ffactor = factor as f32;
                    let delta = ffactor * filter;
                    last_value += delta;

                    if frame == frame_idx {
                        break;
                    }
                    frame += 1;
                }

                if frame == frame_idx {
                    break;
                }

                // Skip to next packet (motchan.cpp:1046)
                p_packet = unsafe { p_packet.add((PACKET_SIZE * self.vector_len) - 1) };
            }

            outdata[vi] = last_value;
        }
    }

    /// Decompress from src_idx to frame_idx (continuation)
    /// Reference: motchan.cpp:1056-1135
    fn decompress_continuation(
        &self,
        src_idx: u32,
        srcdata: &[f32],
        frame_idx: u32,
        outdata: &mut [f32],
    ) {
        debug_assert!(src_idx < frame_idx);
        let src_idx = src_idx + 1;

        let base_ptr = unsafe {
            (self.data.as_ptr() as *const u8).add(std::mem::size_of::<f32>() * self.vector_len)
        };

        for vi in 0..self.vector_len {
            let mut p_packet = unsafe {
                base_ptr
                    .add(PACKET_SIZE * vi)
                    .add((PACKET_SIZE * self.vector_len) * (((src_idx - 1) >> 4) as usize))
            };

            // Initial filter index (motchan.cpp:1075)
            let mut fi = ((src_idx - 1) & 0xF) as usize;

            let mut last_value = srcdata[vi];
            let mut frame = src_idx;

            while frame <= frame_idx {
                let filter_index = unsafe { *p_packet } as usize;
                p_packet = unsafe { p_packet.add(1) };

                let filter = get_filter_table()[filter_index] * self.scale;

                while fi < 16 {
                    let pi = fi >> 1;
                    let mut factor = unsafe { *p_packet.add(pi) } as i32;

                    if (fi & 1) != 0 {
                        factor >>= 4;
                    }

                    factor &= 0xF;
                    if (factor & 0x8) != 0 {
                        factor |= 0xFFFFFFF0u32 as i32;
                    }

                    let ffactor = factor as f32;
                    let delta = ffactor * filter;
                    last_value += delta;

                    if frame == frame_idx {
                        break;
                    }
                    frame += 1;
                    fi += 1;
                }

                fi = 0;

                if frame == frame_idx {
                    break;
                }

                p_packet = unsafe { p_packet.add((PACKET_SIZE * self.vector_len) - 1) };
            }

            outdata[vi] = last_value;
        }
    }

    /// Get decompressed data for a specific frame and vector index
    /// Reference: motchan.cpp:1150-1212
    fn getframe(&mut self, frame_idx: u32, vector_idx: usize) -> f32 {
        // Make sure frame_idx is valid
        let frame_idx = frame_idx.min(self.num_frames - 1);

        // Check if data is already in cache
        if self.cache_frame == frame_idx {
            return self.cache_data[vector_idx];
        }

        if self.cache_frame + 1 == frame_idx {
            return self.cache_data[vector_idx + self.vector_len];
        }

        if frame_idx < self.cache_frame {
            // Decompress from beginning (motchan.cpp:1167-1178)
            // Use unsafe to work around borrow checker limitations - this is safe because
            // decompress/decompress_continuation only read from self.data and other immutable fields,
            // and we're only mutating cache_data
            unsafe {
                let self_ptr = self as *const Self;
                let cache_ptr = self.cache_data.as_mut_ptr();
                let vector_len = self.vector_len;

                (*self_ptr).decompress(
                    frame_idx,
                    std::slice::from_raw_parts_mut(cache_ptr, vector_len),
                );

                if frame_idx != self.num_frames - 1 {
                    (*self_ptr).decompress_continuation(
                        frame_idx,
                        std::slice::from_raw_parts(cache_ptr, vector_len),
                        frame_idx + 1,
                        std::slice::from_raw_parts_mut(cache_ptr.add(vector_len), vector_len),
                    );
                }
            }

            self.cache_frame = frame_idx;
            return self.cache_data[vector_idx];
        }

        // Sliding window optimization (motchan.cpp:1183-1193)
        if frame_idx == self.cache_frame + 2 {
            self.cache_data.copy_within(self.vector_len.., 0);
            self.cache_frame += 1;

            unsafe {
                let self_ptr = self as *const Self;
                let cache_ptr = self.cache_data.as_mut_ptr();
                let vector_len = self.vector_len;
                let cache_frame = self.cache_frame;

                (*self_ptr).decompress_continuation(
                    cache_frame,
                    std::slice::from_raw_parts(cache_ptr, vector_len),
                    frame_idx,
                    std::slice::from_raw_parts_mut(cache_ptr.add(vector_len), vector_len),
                );
            }

            return self.cache_data[self.vector_len + vector_idx];
        }

        // Use last known frame to decompress forwards (motchan.cpp:1195-1208)
        debug_assert!(self.vector_len <= 4);
        let mut temp = [0.0f32; 4];
        temp[..self.vector_len]
            .copy_from_slice(&self.cache_data[self.vector_len..self.vector_len * 2]);

        unsafe {
            let self_ptr = self as *const Self;
            let cache_ptr = self.cache_data.as_mut_ptr();
            let vector_len = self.vector_len;
            let cache_frame_plus_one = self.cache_frame + 1;

            (*self_ptr).decompress_continuation(
                cache_frame_plus_one,
                &temp[..vector_len],
                frame_idx,
                std::slice::from_raw_parts_mut(cache_ptr, vector_len),
            );
        }
        self.cache_frame = frame_idx;

        if frame_idx != self.num_frames - 1 {
            unsafe {
                let self_ptr = self as *const Self;
                let cache_ptr = self.cache_data.as_mut_ptr();
                let vector_len = self.vector_len;
                let cache_frame = self.cache_frame;

                (*self_ptr).decompress_continuation(
                    cache_frame,
                    std::slice::from_raw_parts(cache_ptr, vector_len),
                    frame_idx + 1,
                    std::slice::from_raw_parts_mut(cache_ptr.add(vector_len), vector_len),
                );
            }
        }

        self.cache_data[vector_idx]
    }

    /// Get a scalar vector value at the specified frame
    /// Reference: motchan.cpp:1226-1239
    pub fn get_vector(&mut self, frame: f32, setvec: &mut [f32]) {
        let frame1 = frame as u32;
        let ratio = frame - frame1 as f32;

        let value1 = self.getframe(frame1, 0);
        let value2 = self.getframe(frame1 + 1, 0);

        setvec[0] = value1 + (value2 - value1) * ratio; // WWMath::Lerp
    }

    /// Get a quaternion vector at the specified frame with slerp
    /// Reference: motchan.cpp:1245-1272
    pub fn get_quat_vector(&mut self, frame: f32) -> Quat {
        let frame1 = frame as u32;
        let frame2 = frame1 + 1;
        let ratio = frame - frame1 as f32;

        let q1 = Quat::from_xyzw(
            self.getframe(frame1, 0),
            self.getframe(frame1, 1),
            self.getframe(frame1, 2),
            self.getframe(frame1, 3),
        );

        let q2 = Quat::from_xyzw(
            self.getframe(frame2, 0),
            self.getframe(frame2, 1),
            self.getframe(frame2, 2),
            self.getframe(frame2, 3),
        );

        fast_slerp(q1, q2, ratio)
    }
}

/// Time-coded bit channel for visibility data
/// Reference: motchan.h:304-332, motchan.cpp:693-837
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TimeCodedBitChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    default_val: i32,
    num_timecodes: u32,
    cached_idx: u32,
    /// Each u32 contains: [31 bits: timecode][1 bit: value (MSB)]
    bits: Vec<u32>,
}

impl TimeCodedBitChannelClass {
    /// Create from W3D chunk data
    /// Reference: motchan.cpp:754-788
    pub fn new(
        pivot_idx: u32,
        channel_type: u16,
        default_val: i32,
        num_timecodes: u32,
        bits: Vec<u32>,
    ) -> Self {
        Self {
            pivot_idx,
            channel_type,
            default_val,
            num_timecodes,
            cached_idx: 0,
            bits,
        }
    }

    pub fn get_type(&self) -> u16 {
        self.channel_type
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    /// Lookup a bit in the bit channel at the specified frame
    /// Reference: motchan.cpp:803-837
    pub fn get_bit(&mut self, frame: i32) -> i32 {
        debug_assert!(frame >= 0);
        debug_assert!(self.cached_idx < self.num_timecodes);

        let mut time = (self.bits[self.cached_idx as usize] & !W3D_TIMECODED_BIT_MASK) as i32;
        let mut idx = if frame >= time {
            // Start from cached position
            self.cached_idx as usize + 1
        } else {
            0
        };

        // Linear search from idx to find the right time bucket (motchan.cpp:821-827)
        while idx < self.num_timecodes as usize {
            time = (self.bits[idx] & !W3D_TIMECODED_BIT_MASK) as i32;
            if frame < time {
                break;
            }
            idx += 1;
        }

        idx = idx.saturating_sub(1);
        self.cached_idx = idx as u32;

        // Return the bit value (motchan.cpp:835)
        if (self.bits[idx] & W3D_TIMECODED_BIT_MASK) == W3D_TIMECODED_BIT_MASK {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_table_generation() {
        let table = get_filter_table();

        // Check first hardcoded values
        assert_eq!(table[0], 0.00000001);
        assert_eq!(table[8], 1.0);
        assert_eq!(table[15], 10000000.0);

        // Check generated values are decreasing from 1.0
        assert!(table[16] < 1.0);
        assert!(table[16] > table[255]);
    }

    #[test]
    fn test_timecoded_channel_basic() {
        // Create simple time-coded channel with 2 keyframes
        // Packet format: [timecode, value]
        let data = vec![
            0,                 // timecode 0
            0.0f32.to_bits(),  // value 0.0
            10,                // timecode 10
            10.0f32.to_bits(), // value 10.0
        ];

        let mut channel = TimeCodedMotionChannelClass::new(
            0, // pivot_idx
            ANIM_CHANNEL_X,
            2, // num_timecodes
            1, // vector_len
            data,
        );

        let mut result = [0.0f32];

        // Test at keyframe
        channel.get_vector(0.0, &mut result);
        assert_eq!(result[0], 0.0);

        // Test interpolation
        channel.get_vector(5.0, &mut result);
        assert!((result[0] - 5.0).abs() < 0.001);

        // Test at second keyframe
        channel.get_vector(10.0, &mut result);
        assert_eq!(result[0], 10.0);
    }

    #[test]
    fn test_adaptive_delta_simple() {
        // This is a minimal test - real data would be complex
        // Just verify structure is correct
        let mut channel = AdaptiveDeltaMotionChannelClass::new(
            0, // pivot
            ANIM_CHANNEL_X,
            1,                      // vector_len
            10,                     // num_frames
            1.0,                    // scale
            vec![0.0f32.to_bits()], // minimal data
        );

        assert_eq!(channel.get_pivot(), 0);
        assert_eq!(channel.get_type(), ANIM_CHANNEL_X);
    }
}
