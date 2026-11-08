//! Canonical W3D motion and bit channels (`motchan.h`, `motchan.cpp`).
//!
//! Raw per-frame channels, compressed timecoded/adaptive-delta channels and
//! both bit-channel forms live here and nowhere else. Payloads and packet
//! checkpoints are immutable; independent players borrow them without
//! sharing mutable decoder state. See OWNERSHIP_AND_AUTHORITY.md for the
//! intentional corrections to the original adaptive random-seek bugs.

use glam::Quat;
use math_utilities::wwmath::{EPSILON2, WWMath};
use std::sync::LazyLock;

const FLAG: u32 = 0x8000_0000;
const PACKET_SIZE: usize = 9;

/// Motion channel kinds (`ANIM_CHANNEL_*`, `w3d_file.h:1373-1392`).
///
/// Raw animations store these values in `W3dAnimChannelStruct::Flags`;
/// compressed channels store them in the channel `Flags` byte. Values 7..=15
/// (timecoded/adaptive aliases and visibility) are never installed by the
/// source animation classes, so they have no variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimChannelKind {
    X,
    Y,
    Z,
    XRotation,
    YRotation,
    ZRotation,
    Quaternion,
}

impl AnimChannelKind {
    /// Decode the source `Flags` value; `None` for kinds no animation installs.
    pub fn from_flags(flags: u16) -> Option<Self> {
        Some(match flags {
            0 => Self::X,
            1 => Self::Y,
            2 => Self::Z,
            3 => Self::XRotation,
            4 => Self::YRotation,
            5 => Self::ZRotation,
            6 => Self::Quaternion,
            _ => return None,
        })
    }

    /// Components per frame. A different authored `VectorLen` would make the
    /// source write past its three-float translation or read past its
    /// quaternion, so loaders reject it.
    pub fn vector_len(self) -> usize {
        match self {
            Self::Quaternion => 4,
            _ => 1,
        }
    }

    /// Translation axis index for X/Y/Z channels.
    pub fn translation_axis(self) -> Option<usize> {
        match self {
            Self::X => Some(0),
            Self::Y => Some(1),
            Self::Z => Some(2),
            _ => None,
        }
    }
}

/// Bit channel kinds (`BIT_CHANNEL_*`, `w3d_file.h:1418-1422`). Only
/// visibility is installed by `HRawAnimClass` and `HCompressedAnimClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitChannelKind {
    Visibility,
}

impl BitChannelKind {
    pub fn from_flags(flags: u16) -> Option<Self> {
        (flags == 0).then_some(Self::Visibility)
    }
}

/// Source Fast_Slerp: hemisphere correction, epsilon-squared linear branch,
/// and no normalization of authored/compressed values.
pub fn fast_slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let mut cosine = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    let flip = cosine < 0.0;
    if flip {
        cosine = -cosine;
    }
    let (beta, alpha) = if 1.0 - cosine < EPSILON2 {
        (1.0 - t, t)
    } else {
        let theta = WWMath::fast_acos(cosine);
        let inverse_sine = 1.0 / WWMath::fast_sin(theta);
        (
            WWMath::fast_sin(theta - t * theta) * inverse_sine,
            WWMath::fast_sin(t * theta) * inverse_sine,
        )
    };
    let alpha = if flip { -alpha } else { alpha };
    Quat::from_xyzw(
        beta * a.x + alpha * b.x,
        beta * a.y + alpha * b.y,
        beta * a.z + alpha * b.z,
        beta * a.w + alpha * b.w,
    )
}

/// `AdaptiveDeltaMotionChannelClass` constructor table (`motchan.cpp:15-33`,
/// `824-830`): sixteen authored powers of ten, then
/// `1 - Sin(DEG_TO_RAD(90 * i / 240))`. `DEG_TO_RAD` widens to double before
/// `WWMath::Sin` narrows its argument back to float.
fn generate_filter_table() -> [f32; 256] {
    const WWMATH_PI: f32 = 3.141_592_654;
    let mut table = [0.0; 256];
    table[..16].copy_from_slice(&[
        1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0, 10000.0,
        100000.0, 1000000.0, 10000000.0,
    ]);
    for (index, slot) in (0u8..240).zip(table[16..].iter_mut()) {
        let ratio = f32::from(index) / 240.0;
        let radians = f64::from(90.0 * ratio) * f64::from(WWMATH_PI) / 180.0;
        // Float narrowing at the original `Sin(float)` call boundary.
        #[allow(clippy::cast_possible_truncation)]
        let radians = radians as f32;
        *slot = 1.0 - radians.sin();
    }
    table
}

// Existing immutable format table, never playback state.
static FILTER_TABLE: LazyLock<[f32; 256]> = LazyLock::new(generate_filter_table);

/// Classic per-frame motion (`MotionChannelClass`, `motchan.h:24-112`).
///
/// Frames outside `[FirstFrame, LastFrame]` produce the channel identity:
/// zero for scalar kinds, `(0, 0, 0, 1)` for quaternions. Authored
/// quaternions are returned exactly, never normalized.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionChannelClass {
    pivot_idx: u32,
    kind: AnimChannelKind,
    first_frame: u16,
    last_frame: u16,
    data: Box<[f32]>,
}

impl MotionChannelClass {
    /// `data` must hold exactly `(last - first + 1) * kind.vector_len()`
    /// values; loaders discard the exporter's trailing surplus first.
    pub fn try_new(
        pivot_idx: u32,
        kind: AnimChannelKind,
        first_frame: u16,
        last_frame: u16,
        data: Vec<f32>,
    ) -> Result<Self, &'static str> {
        let frames = Self::frame_count(first_frame, last_frame)?;
        if frames.checked_mul(kind.vector_len()) != Some(data.len()) {
            return Err("raw motion payload does not match its frame range");
        }
        Ok(Self { pivot_idx, kind, first_frame, last_frame, data: data.into_boxed_slice() })
    }

    /// Number of authored frames, or an error for an inverted range (the
    /// source computes an unsigned wrap and fails its read).
    pub fn frame_count(first_frame: u16, last_frame: u16) -> Result<usize, &'static str> {
        last_frame
            .checked_sub(first_frame)
            .map(|span| usize::from(span) + 1)
            .ok_or("inverted raw channel frame range")
    }

    pub fn get_type(&self) -> AnimChannelKind {
        self.kind
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    fn authored(&self, frame: i32) -> Option<&[f32]> {
        if frame < i32::from(self.first_frame) || frame > i32::from(self.last_frame) {
            return None;
        }
        let len = self.kind.vector_len();
        let start = usize::try_from(frame - i32::from(self.first_frame)).ok()? * len;
        self.data.get(start..start + len)
    }

    /// `MotionChannelClass::Get_Vector` for a scalar channel.
    pub fn get_scalar(&self, frame: i32) -> f32 {
        self.authored(frame).map_or(0.0, |values| values[0])
    }

    /// `MotionChannelClass::Get_Vector_As_Quat`.
    pub fn get_vector_as_quat(&self, frame: i32) -> Quat {
        match self.authored(frame) {
            Some(&[x, y, z, w]) => Quat::from_xyzw(x, y, z, w),
            _ => Quat::IDENTITY,
        }
    }
}

/// Classic per-frame bits (`BitChannelClass`, `motchan.h:116-167`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitChannelClass {
    pivot_idx: u32,
    default_val: u8,
    first_frame: u16,
    last_frame: u16,
    bits: Box<[u8]>,
}

impl BitChannelClass {
    /// `bits` must hold exactly `ceil((last - first + 1) / 8)` bytes.
    pub fn try_new(
        pivot_idx: u32,
        default_val: u8,
        first_frame: u16,
        last_frame: u16,
        bits: Vec<u8>,
    ) -> Result<Self, &'static str> {
        let frames = MotionChannelClass::frame_count(first_frame, last_frame)?;
        if frames.div_ceil(8) != bits.len() {
            return Err("raw bit payload does not match its frame range");
        }
        Ok(Self { pivot_idx, default_val, first_frame, last_frame, bits: bits.into_boxed_slice() })
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    /// `BitChannelClass::Get_Bit`: the raw `DefaultVal` byte outside the
    /// authored range (callers compare with one, so a default of two hides),
    /// otherwise the LSB-first packed bit.
    pub fn get_bit(&self, frame: i32) -> u8 {
        if frame < i32::from(self.first_frame) || frame > i32::from(self.last_frame) {
            return self.default_val;
        }
        let Ok(bit) = usize::try_from(frame - i32::from(self.first_frame)) else {
            return self.default_val;
        };
        u8::from(self.bits[bit / 8] & (1 << (bit % 8)) != 0)
    }
}

/// Sparse timecoded motion. Movement flags belong to the destination key.
#[derive(Debug, Clone)]
pub struct TimeCodedMotionChannelClass {
    pivot_idx: u32,
    kind: AnimChannelKind,
    vector_len: usize,
    data: Vec<u32>,
}

impl TimeCodedMotionChannelClass {
    pub fn try_new(pivot_idx: u32, kind: AnimChannelKind, num_timecodes: u32,
        vector_len: usize, data: Vec<u32>) -> Result<Self, &'static str> {
        if kind.vector_len() != vector_len || num_timecodes == 0 {
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
        Ok(Self { pivot_idx, kind, vector_len, data })
    }

    /// Construct trusted in-memory data. File loaders use `try_new`.
    pub fn new(pivot_idx: u32, kind: AnimChannelKind, num_timecodes: u32,
        vector_len: usize, data: Vec<u32>) -> Self {
        Self::try_new(pivot_idx, kind, num_timecodes, vector_len, data)
            .expect("invalid timecoded channel")
    }
    pub fn get_type(&self) -> AnimChannelKind { self.kind }
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
    kind: AnimChannelKind,
    vector_len: usize,
    num_frames: u32,
    scale: f32,
    data: Vec<u8>,
    checkpoints: Vec<f32>,
}

impl AdaptiveDeltaMotionChannelClass {
    pub fn try_new(pivot_idx: u32, kind: AnimChannelKind, vector_len: usize,
        num_frames: u32, scale: f32, data: Vec<u8>) -> Result<Self, &'static str> {
        if kind.vector_len() != vector_len || num_frames == 0 || num_frames > (1 << 24) || !scale.is_finite() {
            return Err("invalid adaptive channel shape/count/scale");
        }
        let blocks = ((num_frames - 1) as usize).div_ceil(16);
        let required = blocks.checked_mul(PACKET_SIZE).and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_mul(vector_len)).ok_or("adaptive payload overflow")?;
        // A writer may retain a full final unused packet at an exact boundary.
        // No bytes used by sampling may be missing.
        if data.len() < required { return Err("truncated adaptive payload"); }
        let mut channel = Self { pivot_idx, kind, vector_len, num_frames, scale,
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
    pub fn new(pivot_idx: u32, kind: AnimChannelKind, vector_len: usize,
        num_frames: u32, scale: f32, data: Vec<u8>) -> Self {
        Self::try_new(pivot_idx, kind, vector_len, num_frames, scale, data)
            .expect("invalid adaptive channel")
    }
    pub fn get_type(&self) -> AnimChannelKind { self.kind }
    pub fn get_pivot(&self) -> u32 { self.pivot_idx }
    pub fn num_frames(&self) -> u32 { self.num_frames }

    fn packet(&self, block: usize, component: usize) -> (f32, &[u8]) {
        let offset = self.vector_len * 4 + (block * self.vector_len + component) * PACKET_SIZE;
        (FILTER_TABLE[usize::from(self.data[offset])] * self.scale, &self.data[offset + 1..offset + 9])
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

/// Compressed visibility (`TimeCodedBitChannelClass`) holds the first key
/// before its time and the final key afterward. Unlike raw bits, its header
/// default is not sampled. Only `BIT_CHANNEL_VIS` channels are constructed;
/// loaders skip other kinds exactly as `add_bit_channel` ignores them.
#[derive(Debug, Clone)]
pub struct TimeCodedBitChannelClass {
    pivot_idx: u32,
    bits: Vec<u32>,
}

impl TimeCodedBitChannelClass {
    pub fn try_new(pivot_idx: u32, num_timecodes: u32, bits: Vec<u32>) -> Result<Self, &'static str> {
        if num_timecodes == 0
            || usize::try_from(num_timecodes) != Ok(bits.len())
            || bits.windows(2).any(|p| (p[0] & !FLAG) > (p[1] & !FLAG))
        {
            return Err("invalid compressed bit channel");
        }
        Ok(Self { pivot_idx, bits })
    }

    pub fn new(pivot_idx: u32, num_timecodes: u32, bits: Vec<u32>) -> Self {
        Self::try_new(pivot_idx, num_timecodes, bits).expect("invalid compressed bit channel")
    }

    pub fn get_pivot(&self) -> u32 {
        self.pivot_idx
    }

    pub fn last_frame(&self) -> u32 {
        self.bits[self.bits.len() - 1] & !FLAG
    }

    pub fn get_bit(&self, frame: i32) -> i32 {
        let frame = u32::try_from(frame).expect("invalid compressed sample frame");
        let index = self.bits.partition_point(|key| (key & !FLAG) <= frame).saturating_sub(1);
        i32::from(self.bits[index] & FLAG != 0)
    }
}
