//! Canonical W3D compressed channels (motchan.cpp).
//!
//! Payloads and packet checkpoints are immutable; independent players borrow
//! them without sharing mutable decoder state. See OWNERSHIP_AND_AUTHORITY.md
//! for the intentional corrections to the original adaptive random-seek bugs.

use glam::Quat;
use math_utilities::wwmath::{EPSILON2, WWMath};
use std::sync::LazyLock;

const FLAG: u32 = 0x8000_0000;
const PACKET_SIZE: usize = 9;

/// Source Fast_Slerp: hemisphere correction, epsilon-squared linear branch,
/// and no normalization of authored/compressed values.
pub fn fast_slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let mut cosine = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    let flip = cosine < 0.0;
    if flip { cosine = -cosine; }
    let (beta, alpha) = if 1.0 - cosine < EPSILON2 {
        (1.0 - t, t)
    } else {
        let theta = WWMath::fast_acos(cosine);
        let inverse_sine = 1.0 / WWMath::fast_sin(theta);
        (WWMath::fast_sin(theta - t * theta) * inverse_sine,
            WWMath::fast_sin(t * theta) * inverse_sine)
    };
    let alpha = if flip { -alpha } else { alpha };
    Quat::from_xyzw(beta * a.x + alpha * b.x, beta * a.y + alpha * b.y,
        beta * a.z + alpha * b.z, beta * a.w + alpha * b.w)
}

fn generate_filter_table() -> [f32; 256] {
    let mut table = [0.0; 256];
    table[..16].copy_from_slice(&[
        1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1,
        1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0,
    ]);
    for i in 0..240 {
        table[16 + i] = 1.0 - (i as f32 / 240.0 * std::f32::consts::FRAC_PI_2).sin();
    }
    table
}

// Existing immutable format table, never playback state.
static FILTER_TABLE: LazyLock<[f32; 256]> = LazyLock::new(generate_filter_table);
fn get_filter_table() -> &'static [f32; 256] {
    &FILTER_TABLE
}

fn valid_shape(kind: u16, len: usize) -> bool {
    matches!((kind, len), (0..=2, 1) | (6, 4))
}

/// Sparse timecoded motion. Movement flags belong to the destination key.
#[derive(Debug, Clone)]
pub struct TimeCodedMotionChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    vector_len: usize,
    data: Vec<u32>,
}

impl TimeCodedMotionChannelClass {
    pub fn try_new(pivot_idx: u32, channel_type: u16, num_timecodes: u32,
        vector_len: usize, data: Vec<u32>) -> Result<Self, &'static str> {
        if !valid_shape(channel_type, vector_len) || num_timecodes == 0 {
            return Err("invalid timecoded channel shape/count");
        }
        let size = vector_len + 1;
        if (num_timecodes as usize).checked_mul(size) != Some(data.len()) {
            return Err("invalid timecoded payload length");
        }
        // Source lookup assumes a motion key at frame zero and strictly
        // increasing times. Reject violations instead of hanging in its search.
        if data[0] & !FLAG != 0
            || data.chunks_exact(size).any(|p| (p[0] & !FLAG) >= (1 << 24))
            || data.chunks_exact(size).zip(data.chunks_exact(size).skip(1))
                .any(|(a, b)| (a[0] & !FLAG) >= (b[0] & !FLAG)) {
            return Err("motion keys must start at zero and strictly increase");
        }
        if data.chunks_exact(size).any(|p| p[1..].iter().any(|v| !f32::from_bits(*v).is_finite())) {
            return Err("non-finite motion value");
        }
        Ok(Self { pivot_idx, channel_type, vector_len, data })
    }

    /// Construct trusted in-memory data. File loaders use `try_new`.
    pub fn new(pivot_idx: u32, channel_type: u16, num_timecodes: u32,
        vector_len: usize, data: Vec<u32>) -> Self {
        Self::try_new(pivot_idx, channel_type, num_timecodes, vector_len, data)
            .expect("invalid timecoded channel")
    }
    pub fn get_type(&self) -> u16 { self.channel_type }
    pub fn get_pivot(&self) -> u32 { self.pivot_idx }
    pub fn last_frame(&self) -> u32 { self.data[self.data.len() - self.vector_len - 1] & !FLAG }

    fn segment(&self, frame: f32) -> (usize, usize, f32) {
        assert!(frame.is_finite() && frame >= 0.0, "invalid compressed sample frame");
        let size = self.vector_len + 1;
        let count = self.data.len() / size;
        let mut lo = 0;
        let mut hi = count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if (self.data[mid * size] & !FLAG) as f32 <= frame { lo = mid + 1; }
            else { hi = mid; }
        }
        let first = lo.saturating_sub(1) * size;
        let next = first + size;
        if next == self.data.len() || self.data[next] & FLAG != 0 {
            return (first + 1, first + 1, 0.0);
        }
        let t0 = (self.data[first] & !FLAG) as f32;
        let t1 = (self.data[next] & !FLAG) as f32;
        (first + 1, next + 1, (frame - t0) / (t1 - t0))
    }

    pub fn get_vector(&self, frame: f32, out: &mut [f32]) {
        let (a, b, t) = self.segment(frame);
        for (i, value) in out.iter_mut().take(self.vector_len).enumerate() {
            let x = f32::from_bits(self.data[a + i]);
            let y = f32::from_bits(self.data[b + i]);
            *value = if a == b { x } else { x + (y - x) * t };
        }
    }
    pub fn get_quat_vector(&self, frame: f32) -> Quat {
        assert_eq!(self.vector_len, 4);
        let (a, b, t) = self.segment(frame);
        let quat = |i| Quat::from_xyzw(f32::from_bits(self.data[i]),
            f32::from_bits(self.data[i + 1]), f32::from_bits(self.data[i + 2]),
            f32::from_bits(self.data[i + 3]));
        if a == b { quat(a) } else { fast_slerp(quat(a), quat(b), t) }
    }
}

/// Nine-byte component packets, with an immutable base per 16-delta block.
/// Checkpoints cost 4 * VectorLen * ceil(NumFrames / 16) bytes, not a dense
/// frame expansion. Construction is linear and adds deltas in source order.
#[derive(Debug, Clone)]
pub struct AdaptiveDeltaMotionChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    vector_len: usize,
    num_frames: u32,
    scale: f32,
    data: Vec<u8>,
    checkpoints: Vec<f32>,
}

impl AdaptiveDeltaMotionChannelClass {
    pub fn try_new(pivot_idx: u32, channel_type: u16, vector_len: usize,
        num_frames: u32, scale: f32, data: Vec<u8>) -> Result<Self, &'static str> {
        if !valid_shape(channel_type, vector_len) || num_frames == 0 || num_frames > (1 << 24) || !scale.is_finite() {
            return Err("invalid adaptive channel shape/count/scale");
        }
        let blocks = ((num_frames - 1) as usize).div_ceil(16);
        let required = blocks.checked_mul(PACKET_SIZE).and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_mul(vector_len)).ok_or("adaptive payload overflow")?;
        // A writer may retain a full final unused packet at an exact boundary.
        // No bytes used by sampling may be missing.
        if data.len() < required { return Err("truncated adaptive payload"); }
        let mut channel = Self { pivot_idx, channel_type, vector_len, num_frames, scale,
            data, checkpoints: Vec::with_capacity((num_frames as usize).div_ceil(16) * vector_len) };
        let mut values = [0.0; 4];
        for (i, value) in values.iter_mut().take(vector_len).enumerate() {
            *value = f32::from_le_bytes(channel.data[i * 4..i * 4 + 4].try_into().unwrap());
            if !value.is_finite() { return Err("non-finite adaptive base"); }
        }
        channel.checkpoints.extend_from_slice(&values[..vector_len]);
        for block in 0..blocks {
            let count = ((num_frames - 1) as usize - block * 16).min(16);
            for (i, value) in values.iter_mut().take(vector_len).enumerate() {
                let (filter, packet) = channel.packet(block, i);
                for delta in 0..count {
                    *value += Self::delta(packet, filter, delta);
                    if !value.is_finite() { return Err("non-finite adaptive value"); }
                }
            }
            if count == 16 { channel.checkpoints.extend_from_slice(&values[..vector_len]); }
        }
        Ok(channel)
    }
    pub fn new(pivot_idx: u32, channel_type: u16, vector_len: usize,
        num_frames: u32, scale: f32, data: Vec<u8>) -> Self {
        Self::try_new(pivot_idx, channel_type, vector_len, num_frames, scale, data)
            .expect("invalid adaptive channel")
    }
    pub fn get_type(&self) -> u16 { self.channel_type }
    pub fn get_pivot(&self) -> u32 { self.pivot_idx }
    pub fn num_frames(&self) -> u32 { self.num_frames }

    fn packet(&self, block: usize, component: usize) -> (f32, &[u8]) {
        let offset = self.vector_len * 4 + (block * self.vector_len + component) * PACKET_SIZE;
        (get_filter_table()[self.data[offset] as usize] * self.scale, &self.data[offset + 1..offset + 9])
    }

    fn delta(packet: &[u8], filter: f32, delta: usize) -> f32 {
        let byte = packet[delta / 2];
        let nibble = if delta & 1 == 0 { byte & 15 } else { byte >> 4 };
        let factor = if nibble & 8 == 0 { nibble as i32 } else { nibble as i32 - 16 };
        factor as f32 * filter
    }

    // Decode each component separately; crossing a packet never terminates
    // the other components (unlike motchan.cpp's shared `done` flag).
    fn pair(&self, frame: f32) -> ([f32; 4], [f32; 4], f32) {
        assert!(frame.is_finite() && frame >= 0.0, "invalid compressed sample frame");
        let frame = frame.min((self.num_frames - 1) as f32);
        let first = frame as u32;
        let block = first / 16;
        let mut a = [0.0; 4];
        let mut b = [0.0; 4];
        for i in 0..self.vector_len {
            let mut value = self.checkpoints[block as usize * self.vector_len + i];
            if first == self.num_frames - 1 && first % 16 == 0 {
                a[i] = value;
                b[i] = value;
                continue;
            }
            let (filter, packet) = self.packet(block as usize, i);
            let within = first as usize % 16;
            for delta in 0..within { value += Self::delta(packet, filter, delta); }
            a[i] = value;
            b[i] = if first + 1 < self.num_frames {
                value + Self::delta(packet, filter, within)
            } else { value };
        }
        (a, b, frame - first as f32)
    }
    pub fn get_vector(&self, frame: f32, out: &mut [f32]) {
        let (a, b, t) = self.pair(frame);
        for (i, value) in out.iter_mut().take(self.vector_len).enumerate() {
            *value = a[i] + (b[i] - a[i]) * t;
        }
    }
    pub fn get_quat_vector(&self, frame: f32) -> Quat {
        assert_eq!(self.vector_len, 4);
        let (a, b, t) = self.pair(frame);
        fast_slerp(Quat::from_array(a), Quat::from_array(b), t)
    }
}

/// Compressed visibility holds the first key before its time and the final
/// key afterward. Unlike raw bits, its header default is not sampled.
#[derive(Debug, Clone)]
pub struct TimeCodedBitChannelClass {
    pivot_idx: u32,
    channel_type: u16,
    bits: Vec<u32>,
}
impl TimeCodedBitChannelClass {
    pub fn try_new(pivot_idx: u32, channel_type: u16, _default_val: i32,
        num_timecodes: u32, bits: Vec<u32>) -> Result<Self, &'static str> {
        if channel_type != 0 || num_timecodes == 0 || bits.len() != num_timecodes as usize
            || bits.windows(2).any(|p| (p[0] & !FLAG) > (p[1] & !FLAG)) {
            return Err("invalid compressed bit channel");
        }
        Ok(Self { pivot_idx, channel_type, bits })
    }
    pub fn new(pivot_idx: u32, channel_type: u16, default_val: i32,
        num_timecodes: u32, bits: Vec<u32>) -> Self {
        Self::try_new(pivot_idx, channel_type, default_val, num_timecodes, bits)
            .expect("invalid compressed bit channel")
    }
    pub fn get_type(&self) -> u16 { self.channel_type }
    pub fn get_pivot(&self) -> u32 { self.pivot_idx }
    pub fn last_frame(&self) -> u32 { self.bits[self.bits.len() - 1] & !FLAG }
    pub fn get_bit(&self, frame: i32) -> i32 {
        assert!(frame >= 0, "invalid compressed sample frame");
        let index = self.bits.partition_point(|key| (key & !FLAG) <= frame as u32).saturating_sub(1);
        i32::from(self.bits[index] & FLAG != 0)
    }
}
