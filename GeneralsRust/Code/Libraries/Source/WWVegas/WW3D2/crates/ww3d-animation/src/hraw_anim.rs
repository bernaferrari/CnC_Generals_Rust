//! Canonical classic (uncompressed) W3D animation (`hrawanim.cpp`).
//!
//! `HRawAnimClass` owns the immutable per-pivot channel slots of one
//! `W3D_CHUNK_ANIMATION`. It provides both source sampling paths:
//! the generic interpolated `HRawAnimClass::Get_*` methods used by
//! `HTreeClass::Anim_Update(HAnimClass*)`, and the Generals specialized
//! integer-frame `HTreeClass::Anim_Update(HRawAnimClass*)` used for HLOD
//! poses. Playback position is always caller owned.

use crate::motion_channels::{
    AnimChannelKind, BitChannelClass, BitChannelKind, MotionChannelClass, fast_slerp,
};
use crate::w3d_loader::W3DAnimationError;
use glam::{Quat, Vec3};

const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x0000_0201;
const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x0000_0202;
const W3D_CHUNK_BIT_CHANNEL: u32 = 0x0000_0203;
/// `sizeof(W3dAnimHeaderStruct)`.
const ANIM_HEADER_SIZE: usize = 44;
/// `sizeof(W3dAnimChannelStruct)` without its trailing `Data[1]`.
const ANIM_CHANNEL_HEADER_SIZE: usize = 12;
/// `sizeof(W3dBitChannelStruct)` without its trailing `Data[1]`.
const BIT_CHANNEL_HEADER_SIZE: usize = 9;
const W3D_NAME_LEN: usize = 16;
/// `W3D_MAKE_VERSION(3, 0)`: earlier animations predate the HTree root pivot.
const W3D_HTREE_ROOT_VERSION: u32 = 3 << 16;

/// One pivot's installed channels (`NodeMotionStruct`). The source also keeps
/// XR/YR/ZR pointers, but no Generals sampling path reads them (orientation
/// comes only from `Q` under `SPECIAL_GETVEC_AS_QUAT`), so they are dropped
/// after their payload is validated.
#[derive(Debug, Clone, Default, PartialEq)]
struct NodeMotion {
    translation: [Option<MotionChannelClass>; 3],
    rotation: Option<MotionChannelClass>,
    visibility: Option<BitChannelClass>,
}

/// One pivot sampled by the specialized raw `Anim_Update`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPivotPose {
    /// Authored X/Y/Z, zero where a channel is absent or out of range.
    pub translation: Vec3,
    /// `None` when the pivot has no `Q` channel; the source then skips its
    /// `postMul` instead of multiplying by an identity matrix.
    pub rotation: Option<Quat>,
    /// `Get_Bit(frame) == 1`, or visible when no visibility channel exists.
    pub visible: bool,
}

/// Classic hierarchical animation.
#[derive(Debug, Clone, PartialEq)]
pub struct HRawAnimClass {
    name: String,
    hierarchy_name: String,
    num_frames: u32,
    frame_rate: u32,
    node_motion: Vec<NodeMotion>,
}

impl HRawAnimClass {
    /// An animation without channels; populate it with
    /// [`add_channel`](Self::add_channel) and
    /// [`add_bit_channel`](Self::add_bit_channel).
    pub fn new(name: &str, hierarchy_name: &str, num_frames: u32, frame_rate: u32) -> Self {
        Self {
            name: name.to_string(),
            hierarchy_name: hierarchy_name.to_string(),
            num_frames,
            frame_rate,
            node_motion: Vec::new(),
        }
    }

    /// `HRawAnimClass::Load_W3D` over the payload of one
    /// `W3D_CHUNK_ANIMATION`. The header must be the first sub-chunk, a
    /// short or inverted channel fails the whole animation as in the source,
    /// and pre-3.0 pivots shift past the inserted HTree root.
    ///
    /// The caller's hierarchy may not be loaded yet, so slots extend to the
    /// largest authored pivot; [`bind_to_hierarchy`](Self::bind_to_hierarchy)
    /// applies the source's `NumNodes` discard when it is known.
    pub fn load_w3d(payload: &[u8]) -> Result<Self, W3DAnimationError> {
        let mut chunks = SubChunks { data: payload };
        let (header_id, header) = chunks.next().ok_or_else(missing_header)??;
        if header_id != W3D_CHUNK_ANIMATION_HEADER {
            return Err(missing_header());
        }
        let header = header
            .get(..ANIM_HEADER_SIZE)
            .ok_or_else(|| invalid("truncated animation header"))?;
        let version = read_u32(header, 0);
        let pre30 = version < W3D_HTREE_ROOT_VERSION;
        let mut anim = Self::new(
            &fixed_name(&header[4..4 + W3D_NAME_LEN]),
            &fixed_name(&header[20..20 + W3D_NAME_LEN]),
            read_u32(header, 36),
            read_u32(header, 40),
        );
        let pivot = |authored: u16| u32::from(authored) + u32::from(pre30);

        for chunk in chunks {
            let (id, data) = chunk?;
            match id {
                W3D_CHUNK_ANIMATION_CHANNEL => {
                    if let Some(channel) = parse_motion_channel(data, pivot)? {
                        anim.add_channel(channel);
                    }
                }
                W3D_CHUNK_BIT_CHANNEL => {
                    if let Some(channel) = parse_bit_channel(data, pivot)? {
                        anim.add_bit_channel(channel);
                    }
                }
                _ => {}
            }
        }
        Ok(anim)
    }

    /// Size the slots to the named hierarchy (`NodeMotion[NumNodes]`),
    /// discarding channels for missing bones as `Load_W3D` does.
    pub fn bind_to_hierarchy(mut self, num_pivots: usize) -> Self {
        self.node_motion.resize_with(num_pivots, NodeMotion::default);
        self
    }

    /// `HRawAnimClass::add_channel`: a later channel of the same kind
    /// replaces the earlier one for its pivot.
    pub fn add_channel(&mut self, channel: MotionChannelClass) {
        let Some(node) = self.node_mut(channel.get_pivot()) else {
            return;
        };
        let kind = channel.get_type();
        if let Some(axis) = kind.translation_axis() {
            node.translation[axis] = Some(channel);
        } else if kind == AnimChannelKind::Quaternion {
            node.rotation = Some(channel);
        }
    }

    /// `HRawAnimClass::add_bit_channel` for `BIT_CHANNEL_VIS`.
    pub fn add_bit_channel(&mut self, channel: BitChannelClass) {
        if let Some(node) = self.node_mut(channel.get_pivot()) {
            node.visibility = Some(channel);
        }
    }

    fn node_mut(&mut self, pivot: u32) -> Option<&mut NodeMotion> {
        let pivot = usize::try_from(pivot).ok()?;
        if pivot >= self.node_motion.len() {
            self.node_motion.resize_with(pivot + 1, NodeMotion::default);
        }
        self.node_motion.get_mut(pivot)
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_hierarchy_name(&self) -> &str {
        &self.hierarchy_name
    }

    pub fn get_num_frames(&self) -> u32 {
        self.num_frames
    }

    pub fn get_frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Pivot slots (`NumNodes` once bound to a hierarchy).
    pub fn get_num_pivots(&self) -> usize {
        self.node_motion.len()
    }

    pub fn has_x_translation(&self, pivot: usize) -> bool {
        self.node(pivot).is_some_and(|node| node.translation[0].is_some())
    }

    pub fn has_y_translation(&self, pivot: usize) -> bool {
        self.node(pivot).is_some_and(|node| node.translation[1].is_some())
    }

    pub fn has_z_translation(&self, pivot: usize) -> bool {
        self.node(pivot).is_some_and(|node| node.translation[2].is_some())
    }

    pub fn has_rotation(&self, pivot: usize) -> bool {
        self.node(pivot).is_some_and(|node| node.rotation.is_some())
    }

    pub fn has_visibility(&self, pivot: usize) -> bool {
        self.node(pivot).is_some_and(|node| node.visibility.is_some())
    }

    fn node(&self, pivot: usize) -> Option<&NodeMotion> {
        self.node_motion.get(pivot)
    }

    /// The integer frame chosen by `HTreeClass::Anim_Update(HRawAnimClass*)`:
    /// `Float_To_Long` (x87 round-to-nearest-even), then a frame at or past
    /// `NumFrames` wraps to zero. Negative frames stay negative so channels
    /// yield their identity/default values.
    pub fn raw_frame_index(&self, frame: f32) -> i32 {
        let frame = float_to_long(frame);
        if i64::from(frame) >= i64::from(self.num_frames) { 0 } else { frame }
    }

    /// One pivot of the specialized raw `Anim_Update` at an integer frame.
    pub fn raw_pivot_pose(&self, pivot: usize, frame: i32) -> RawPivotPose {
        let Some(node) = self.node(pivot) else {
            return RawPivotPose { translation: Vec3::ZERO, rotation: None, visible: true };
        };
        RawPivotPose {
            translation: node_translation(node, frame),
            rotation: node.rotation.as_ref().map(|q| q.get_vector_as_quat(frame)),
            visible: node.visibility.as_ref().is_none_or(|vis| vis.get_bit(frame) == 1),
        }
    }

    /// `Float_To_Long(frame - 0.499999)` and its successor (wrapping to zero
    /// at `NumFrames`), with the source's unclamped ratio.
    fn interpolation_frames(&self, frame: f32) -> (i32, i32, f32) {
        let frame0 = float_to_long(frame - 0.499_999);
        let frame1 = frame0.wrapping_add(1);
        let frame1 = if i64::from(frame1) >= i64::from(self.num_frames) { 0 } else { frame1 };
        // The source subtracts `(float)frame0`; i32 to f32 rounding included.
        #[allow(clippy::cast_precision_loss)]
        let ratio = frame - frame0 as f32;
        (frame0, frame1, ratio)
    }

    /// `HRawAnimClass::Get_Translation`.
    pub fn get_translation(&self, pivot: usize, frame: f32) -> Vec3 {
        let Some(node) = self.node(pivot).filter(|node| node.translation.iter().any(Option::is_some))
        else {
            return Vec3::ZERO;
        };
        let (frame0, frame1, ratio) = self.interpolation_frames(frame);
        let trans0 = node_translation(node, frame0);
        if ratio == 0.0 {
            return trans0;
        }
        let trans1 = node_translation(node, frame1);
        trans0 + (trans1 - trans0) * ratio
    }

    /// `HRawAnimClass::Get_Orientation` (`SPECIAL_GETVEC_AS_QUAT`).
    pub fn get_orientation(&self, pivot: usize, frame: f32) -> Quat {
        let (frame0, frame1, ratio) = self.interpolation_frames(frame);
        let (q0, q1) = match self.node(pivot).and_then(|node| node.rotation.as_ref()) {
            Some(q) => (q.get_vector_as_quat(frame0), q.get_vector_as_quat(frame1)),
            None => (Quat::IDENTITY, Quat::IDENTITY),
        };
        if ratio == 0.0 {
            q0
        } else if ratio == 1.0 {
            q1
        } else {
            fast_slerp(q0, q1, ratio)
        }
    }

    /// `HRawAnimClass::Get_Visibility`: `(int)frame` truncation.
    pub fn get_visibility(&self, pivot: usize, frame: f32) -> bool {
        self.node(pivot)
            .and_then(|node| node.visibility.as_ref())
            .is_none_or(|vis| vis.get_bit(truncate_frame(frame)) == 1)
    }
}

fn node_translation(node: &NodeMotion, frame: i32) -> Vec3 {
    let axis = |slot: &Option<MotionChannelClass>| slot.as_ref().map_or(0.0, |c| c.get_scalar(frame));
    Vec3::new(axis(&node.translation[0]), axis(&node.translation[1]), axis(&node.translation[2]))
}

/// `WWMath::Float_To_Long` via x87 `fistp` under the game's
/// round-to-nearest mode; unrepresentable input stores the x87 integer
/// indefinite value.
pub fn float_to_long(frame: f32) -> i32 {
    let rounded = frame.round_ties_even();
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&rounded) {
        return i32::MIN;
    }
    // The range check above makes this integral conversion exact.
    #[allow(clippy::cast_possible_truncation)]
    let frame = rounded as i32;
    frame
}

/// C `(int)frame` truncation (saturating where the source is undefined).
fn truncate_frame(frame: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let frame = frame as i32;
    frame
}

/// `MotionChannelClass::Load_W3D`. Unused kinds are validated and dropped,
/// as `add_channel` ignores them; installed kinds must have the vector length
/// the source sampler writes.
fn parse_motion_channel(
    data: &[u8],
    pivot: impl Fn(u16) -> u32,
) -> Result<Option<MotionChannelClass>, W3DAnimationError> {
    let header = data
        .get(..ANIM_CHANNEL_HEADER_SIZE)
        .ok_or_else(|| invalid("truncated animation channel"))?;
    let (first, last) = (read_u16(header, 0), read_u16(header, 2));
    let vector_len = usize::from(read_u16(header, 4));
    let flags = read_u16(header, 6);
    let frames = MotionChannelClass::frame_count(first, last).map_err(invalid)?;
    let floats = frames.checked_mul(vector_len).filter(|count| *count > 0);
    // The exporter sometimes saved surplus floats; the source reads only
    // the declared range and seeks past the rest.
    let payload = floats
        .and_then(|count| data.get(ANIM_CHANNEL_HEADER_SIZE..ANIM_CHANNEL_HEADER_SIZE + count * 4))
        .ok_or_else(|| invalid("truncated animation channel payload"))?;
    let Some(kind) = AnimChannelKind::from_flags(flags) else {
        return Ok(None);
    };
    if kind.vector_len() != vector_len {
        return Err(invalid("animation channel vector length does not match its kind"));
    }
    let values = payload
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    MotionChannelClass::try_new(pivot(read_u16(header, 8)), kind, first, last, values)
        .map(Some)
        .map_err(invalid)
}

/// `BitChannelClass::Load_W3D`; non-visibility kinds are dropped.
fn parse_bit_channel(
    data: &[u8],
    pivot: impl Fn(u16) -> u32,
) -> Result<Option<BitChannelClass>, W3DAnimationError> {
    let header = data
        .get(..BIT_CHANNEL_HEADER_SIZE)
        .ok_or_else(|| invalid("truncated bit channel"))?;
    let (first, last) = (read_u16(header, 0), read_u16(header, 2));
    let bytes = MotionChannelClass::frame_count(first, last).map_err(invalid)?.div_ceil(8);
    let bits = data
        .get(BIT_CHANNEL_HEADER_SIZE..BIT_CHANNEL_HEADER_SIZE + bytes)
        .ok_or_else(|| invalid("truncated bit channel payload"))?;
    if BitChannelKind::from_flags(read_u16(header, 4)).is_none() {
        return Ok(None);
    }
    BitChannelClass::try_new(pivot(read_u16(header, 6)), header[8], first, last, bits.to_vec())
        .map(Some)
        .map_err(invalid)
}

/// Iterator over `(id, payload)` sub-chunks of a container payload.
struct SubChunks<'a> {
    data: &'a [u8],
}

impl<'a> Iterator for SubChunks<'a> {
    type Item = Result<(u32, &'a [u8]), W3DAnimationError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        let Some(header) = self.data.get(..8) else {
            self.data = &[];
            return Some(Err(invalid("truncated animation sub-chunk header")));
        };
        let id = read_u32(header, 0);
        let size = usize::try_from(read_u32(header, 4) & 0x7FFF_FFFF).ok();
        let Some(payload) = size.and_then(|size| self.data.get(8..8 + size)) else {
            self.data = &[];
            return Some(Err(invalid("animation sub-chunk exceeds its container")));
        };
        self.data = &self.data[8 + payload.len()..];
        Some(Ok((id, payload)))
    }
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn fixed_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn invalid(message: &str) -> W3DAnimationError {
    W3DAnimationError::InvalidData(message.to_string())
}

fn missing_header() -> W3DAnimationError {
    W3DAnimationError::MissingChunk("animation header".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = id.to_le_bytes().to_vec();
        out.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn header(version: u32, frames: u32) -> Vec<u8> {
        let mut out = version.to_le_bytes().to_vec();
        out.extend_from_slice(b"Walk\0\0\0\0\0\0\0\0\0\0\0\0");
        out.extend_from_slice(b"Skel\0\0\0\0\0\0\0\0\0\0\0\0");
        out.extend_from_slice(&frames.to_le_bytes());
        out.extend_from_slice(&15u32.to_le_bytes());
        out
    }

    fn motion(first: u16, last: u16, vector_len: u16, flags: u16, pivot: u16, values: &[f32]) -> Vec<u8> {
        let mut out = Vec::new();
        for field in [first, last, vector_len, flags, pivot, 0] {
            out.extend_from_slice(&field.to_le_bytes());
        }
        values.iter().for_each(|value| out.extend_from_slice(&value.to_le_bytes()));
        chunk(W3D_CHUNK_ANIMATION_CHANNEL, &out)
    }

    fn bits(first: u16, last: u16, flags: u16, pivot: u16, default: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for field in [first, last, flags, pivot] {
            out.extend_from_slice(&field.to_le_bytes());
        }
        out.push(default);
        out.extend_from_slice(payload);
        chunk(W3D_CHUNK_BIT_CHANNEL, &out)
    }

    fn load(version: u32, frames: u32, channels: &[Vec<u8>]) -> Result<HRawAnimClass, W3DAnimationError> {
        let mut payload = chunk(W3D_CHUNK_ANIMATION_HEADER, &header(version, frames));
        channels.iter().for_each(|channel| payload.extend_from_slice(channel));
        HRawAnimClass::load_w3d(&payload)
    }

    const V4: u32 = 4 << 16;

    #[test]
    fn raw_channels_return_identity_outside_range_and_exact_authored_quaternions() {
        let anim = load(V4, 4, &[
            motion(1, 2, 1, 0, 1, &[5.0, 6.0, 99.0]),
            motion(1, 1, 4, 6, 1, &[0.0, 0.0, 2.0, 2.0]),
        ])
        .unwrap();
        assert_eq!(anim.raw_pivot_pose(1, 0).translation, Vec3::ZERO);
        assert_eq!(anim.raw_pivot_pose(1, 0).rotation, Some(Quat::IDENTITY));
        assert_eq!(anim.raw_pivot_pose(1, 2).translation, Vec3::new(6.0, 0.0, 0.0));
        // Not normalized.
        assert_eq!(anim.raw_pivot_pose(1, 1).rotation, Some(Quat::from_xyzw(0.0, 0.0, 2.0, 2.0)));
        assert_eq!(anim.raw_pivot_pose(1, 1).rotation.map(|q| q.length()), Some(8f32.sqrt()));
    }

    #[test]
    fn later_channels_replace_earlier_slots_and_unused_kinds_are_ignored() {
        let anim = load(V4, 2, &[
            motion(0, 0, 1, 0, 1, &[1.0]),
            motion(0, 0, 1, 3, 1, &[9.0]),
            motion(0, 0, 1, 11, 1, &[9.0]),
            motion(0, 0, 1, 0, 1, &[2.0]),
        ])
        .unwrap();
        assert_eq!(anim.raw_pivot_pose(1, 0).translation.x, 2.0);
        assert!(!anim.has_rotation(1));
    }

    #[test]
    fn pre30_pivots_shift_past_the_root_without_wrapping() {
        let anim = load(2 << 16, 1, &[motion(0, 0, 1, 1, u16::MAX, &[4.0]), motion(0, 0, 1, 1, 0, &[3.0])])
            .unwrap();
        assert_eq!(anim.raw_pivot_pose(1, 0).translation.y, 3.0);
        assert_eq!(anim.get_num_pivots(), usize::from(u16::MAX) + 2);
        assert_eq!(anim.raw_pivot_pose(0, 0).translation, Vec3::ZERO);
        assert_eq!(anim.bind_to_hierarchy(2).get_num_pivots(), 2);
    }

    #[test]
    fn malformed_channels_fail_the_animation_like_the_source_read() {
        assert!(load(V4, 2, &[motion(2, 1, 1, 0, 1, &[1.0])]).is_err());
        assert!(load(V4, 2, &[motion(0, 1, 1, 0, 1, &[1.0])]).is_err());
        assert!(load(V4, 2, &[motion(0, 0, 0, 3, 1, &[])]).is_err());
        assert!(load(V4, 2, &[motion(0, 0, 4, 0, 1, &[1.0; 4])]).is_err());
        assert!(load(V4, 2, &[bits(0, 9, 0, 1, 1, &[0xFF])]).is_err());
        assert!(load(V4, 2, &[bits(3, 2, 0, 1, 1, &[0xFF])]).is_err());
        let mut no_header = motion(0, 0, 1, 0, 1, &[1.0]);
        no_header.extend_from_slice(&chunk(W3D_CHUNK_ANIMATION_HEADER, &header(V4, 1)));
        assert!(HRawAnimClass::load_w3d(&no_header).is_err());
        // Exporter surplus after the declared range is legal.
        assert!(load(V4, 2, &[motion(0, 0, 1, 0, 1, &[1.0, 7.0])]).is_ok());
    }

    #[test]
    fn visibility_uses_lsb_bits_and_the_raw_default_byte() {
        let anim = load(V4, 12, &[
            bits(2, 10, 0, 1, 1, &[0b1111_1011, 0b0000_0001]),
            bits(0, 0, 0, 2, 2, &[0]),
            bits(0, 0, 1, 3, 0, &[0]),
        ])
        .unwrap();
        let visible = |pivot, frame| anim.raw_pivot_pose(pivot, frame).visible;
        assert!(visible(1, 0) && visible(1, 11) && visible(1, 2));
        assert!(!visible(1, 4));
        assert!(visible(1, 10));
        // A default of two is not `== 1`.
        assert!(!visible(2, 5));
        // Non-visibility bit kinds are not installed.
        assert!(visible(3, 0) && !anim.has_visibility(3));
        assert!(!anim.get_visibility(1, 4.9));
        assert!(anim.get_visibility(1, 5.0));
    }

    #[test]
    fn specialized_frame_rounds_to_nearest_even_and_wraps() {
        let anim = HRawAnimClass::new("a", "h", 4, 30);
        assert_eq!(anim.raw_frame_index(1.5), 2);
        assert_eq!(anim.raw_frame_index(2.5), 2);
        assert_eq!(anim.raw_frame_index(3.6), 0);
        assert_eq!(anim.raw_frame_index(-1.2), -1);
        assert_eq!(anim.raw_frame_index(f32::NAN), i32::MIN);
    }

    #[test]
    fn generic_sampling_interpolates_wraps_to_frame_zero_and_fast_slerps() {
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut q = Vec::new();
        q.extend_from_slice(&Quat::IDENTITY.to_array());
        q.extend_from_slice(&quarter.to_array());
        let anim = load(V4, 2, &[motion(0, 1, 1, 2, 1, &[4.0, 8.0]), motion(0, 1, 4, 6, 1, &q)]).unwrap();
        assert_eq!(anim.get_translation(1, 0.0).z, 4.0);
        assert_eq!(anim.get_translation(1, 0.25).z, 5.0);
        // Frame one interpolates toward wrapped frame zero.
        assert_eq!(anim.get_translation(1, 1.5).z, 6.0);
        assert_eq!(anim.get_orientation(1, 1.0), quarter);
        assert_eq!(anim.get_orientation(1, 0.5), fast_slerp(Quat::IDENTITY, quarter, 0.5));
        assert_eq!(anim.get_orientation(0, 0.5), Quat::IDENTITY);
        assert_eq!(anim.get_translation(0, 0.5), Vec3::ZERO);
    }
}
