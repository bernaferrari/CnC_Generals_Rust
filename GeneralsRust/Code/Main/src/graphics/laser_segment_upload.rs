//! W3DLaserDraw / SegLineRenderer residual: pack presentation laser Line3D segments
//! into a CPU vertex buffer ready for WGPU upload.
//!
//! Host residual closed here (fail-closed vs full retail SegLineRenderer GPU draw):
//! - Interleaved vertex bytes from `PresentationFrame.laser_beams` / Line3D segments
//! - Honesty counters for beams / segments / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//!
//! Still residual:
//! - Actual `wgpu::Queue::write_buffer` against a live device/pipeline
//! - EXBinaryStream32.tga texture bind / UV scroll GPU sample
//! - Full multi-beam soft edge / outer color strip

use crate::game_logic::host_base_defense::{
    HostLaserLine3DSegment, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
};
use crate::presentation_frame::{PresentationFrame, PresentationLaserBeam};

/// Bytes per packed laser segment vertex (pos.xyz + uv.xy + color.rgba = 9 × f32).
pub const LASER_VERTEX_FLOATS: usize = 9;
/// Two vertices (start + end) per Line3D segment.
pub const LASER_VERTS_PER_SEGMENT: usize = 2;
/// Bytes per Line3D segment in the residual upload buffer.
pub const LASER_BYTES_PER_SEGMENT: usize =
    LASER_VERTEX_FLOATS * LASER_VERTS_PER_SEGMENT * std::mem::size_of::<f32>();

/// One interleaved residual laser vertex (CPU-side SegLine point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaserSegmentVertex {
    pub position: [f32; 3],
    /// U = tile factor along beam, V = scroll residual.
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl LaserSegmentVertex {
    pub fn to_floats(self) -> [f32; LASER_VERTEX_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.uv[0],
            self.uv[1],
            self.color[0],
            self.color[1],
            self.color[2],
            self.color[3],
        ]
    }
}

/// Honesty bookkeeping for the residual laser segment upload path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaserSegmentUploadHonesty {
    pub beams_packed: u32,
    pub segments_packed: u32,
    pub vertices_packed: u32,
    pub bytes_packed: u32,
    /// True when pack completed without panic (empty is honest success).
    pub cpu_pack_ok: bool,
    /// True when at least one segment was packed (non-empty residual exercise).
    pub has_geometry: bool,
    /// Texture name residual for presentation consumers (not GPU-bound here).
    pub texture_name: String,
    /// True after a host called `mark_gpu_upload_ready` (still not a live queue write).
    pub gpu_upload_ready: bool,
}

impl LaserSegmentUploadHonesty {
    pub fn honesty_cpu_pack_ok(&self) -> bool {
        self.cpu_pack_ok
    }

    pub fn honesty_geometry_ok(&self) -> bool {
        self.cpu_pack_ok && self.has_geometry && self.segments_packed > 0
    }

    pub fn honesty_upload_ready_ok(&self) -> bool {
        self.gpu_upload_ready && self.cpu_pack_ok
    }
}

/// Packed laser segment payload ready for WGPU buffer write.
#[derive(Debug, Clone, PartialEq)]
pub struct LaserSegmentUpload {
    /// Interleaved f32 vertex payload (see `LaserSegmentVertex`).
    pub vertex_bytes: Vec<u8>,
    pub honesty: LaserSegmentUploadHonesty,
}

impl LaserSegmentUpload {
    /// Empty pack — honest residual when no beams are active.
    pub fn empty() -> Self {
        Self {
            vertex_bytes: Vec::new(),
            honesty: LaserSegmentUploadHonesty {
                cpu_pack_ok: true,
                texture_name: PATRIOT_LASER_TEXTURE.to_string(),
                ..Default::default()
            },
        }
    }

    pub fn segment_count(&self) -> u32 {
        self.honesty.segments_packed
    }

    pub fn vertex_count(&self) -> u32 {
        self.honesty.vertices_packed
    }

    /// Mark residual as ready for a live queue write (host-testable flag only).
    pub fn mark_gpu_upload_ready(&mut self) {
        self.honesty.gpu_upload_ready = self.honesty.cpu_pack_ok;
    }

    /// Pack Line3D segment descriptors into interleaved vertex bytes.
    pub fn pack_line3d_segments(segments: &[HostLaserLine3DSegment]) -> Self {
        Self::pack_line3d_segments_with_color(segments, PATRIOT_LASER_INNER_COLOR)
    }

    pub fn pack_line3d_segments_with_color(
        segments: &[HostLaserLine3DSegment],
        color: (f32, f32, f32, f32),
    ) -> Self {
        let mut floats = Vec::with_capacity(segments.len() * LASER_VERTS_PER_SEGMENT * LASER_VERTEX_FLOATS);
        for (i, seg) in segments.iter().enumerate() {
            let verts = segment_to_vertices(seg, color, i as f32);
            for v in verts {
                floats.extend_from_slice(&v.to_floats());
            }
        }
        let vertex_bytes = f32_slice_to_bytes(&floats);
        let segments_packed = segments.len() as u32;
        let vertices_packed = segments_packed.saturating_mul(LASER_VERTS_PER_SEGMENT as u32);
        let bytes_packed = vertex_bytes.len() as u32;
        Self {
            vertex_bytes,
            honesty: LaserSegmentUploadHonesty {
                beams_packed: if segments_packed > 0 { 1 } else { 0 },
                segments_packed,
                vertices_packed,
                bytes_packed,
                cpu_pack_ok: true,
                has_geometry: segments_packed > 0,
                texture_name: PATRIOT_LASER_TEXTURE.to_string(),
                gpu_upload_ready: false,
            },
        }
    }

    /// Pack all beams from a presentation snapshot (preferred production path).
    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_beams(&frame.laser_beams)
    }

    /// Pack presentation laser beams into one interleaved buffer.
    pub fn pack_beams(beams: &[PresentationLaserBeam]) -> Self {
        if beams.is_empty() {
            return Self::empty();
        }
        let mut floats = Vec::new();
        let mut segments_packed = 0u32;
        for beam in beams {
            for (i, seg) in beam.segments.iter().enumerate() {
                let host = HostLaserLine3DSegment {
                    start: seg.start,
                    end: seg.end,
                    width: seg.width,
                    tile_factor: seg.tile_factor,
                    scroll_offset: seg.scroll_offset,
                };
                let color = beam.inner_color;
                let verts = segment_to_vertices(&host, color, i as f32);
                for v in verts {
                    floats.extend_from_slice(&v.to_floats());
                }
                segments_packed = segments_packed.saturating_add(1);
            }
        }
        let vertex_bytes = f32_slice_to_bytes(&floats);
        let vertices_packed = segments_packed.saturating_mul(LASER_VERTS_PER_SEGMENT as u32);
        let bytes_packed = vertex_bytes.len() as u32;
        Self {
            vertex_bytes,
            honesty: LaserSegmentUploadHonesty {
                beams_packed: beams.len() as u32,
                segments_packed,
                vertices_packed,
                bytes_packed,
                cpu_pack_ok: true,
                has_geometry: segments_packed > 0,
                texture_name: beams
                    .first()
                    .map(|b| b.texture_name.clone())
                    .unwrap_or_else(|| PATRIOT_LASER_TEXTURE.to_string()),
                gpu_upload_ready: false,
            },
        }
    }
}

fn segment_to_vertices(
    seg: &HostLaserLine3DSegment,
    color: (f32, f32, f32, f32),
    segment_index: f32,
) -> [LaserSegmentVertex; 2] {
    // V carries scroll residual; U carries tile residual along the beam.
    // Start U=0, end U=tile_factor (SegLine tile factor residual).
    let c = [color.0, color.1, color.2, color.3];
    let scroll = seg.scroll_offset + segment_index * 0.0;
    [
        LaserSegmentVertex {
            position: [seg.start.0, seg.start.1, seg.start.2],
            uv: [0.0, scroll],
            color: c,
        },
        LaserSegmentVertex {
            position: [seg.end.0, seg.end.1, seg.end.2],
            uv: [seg.tile_factor.max(0.0), scroll],
            color: c,
        },
    ]
}

fn f32_slice_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Host-testable residual: pack + mark upload-ready without a live WGPU device.
pub fn pack_and_mark_upload_ready(frame: &PresentationFrame) -> LaserSegmentUpload {
    let mut pack = LaserSegmentUpload::pack_from_presentation(frame);
    pack.mark_gpu_upload_ready();
    pack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_base_defense::{
        build_patriot_laser_line3d_segments, make_patriot_assist_lasers, PATRIOT_LASER_ARC_HEIGHT,
        PATRIOT_LASER_SEGMENTS,
    };
    use crate::game_logic::ObjectId;
    use crate::presentation_frame::PresentationLaserBeam;

    #[test]
    fn empty_pack_is_honest_cpu_success() {
        let pack = LaserSegmentUpload::empty();
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.honesty_geometry_ok());
        assert!(pack.vertex_bytes.is_empty());
        assert_eq!(pack.honesty.texture_name, PATRIOT_LASER_TEXTURE);
    }

    #[test]
    fn packs_line3d_segments_with_expected_byte_layout() {
        let segs = build_patriot_laser_line3d_segments(
            (0.0, 0.0, 0.0),
            (100.0, 0.0, 0.0),
            PATRIOT_LASER_ARC_HEIGHT,
            -0.25,
            0.0,
        );
        assert_eq!(segs.len(), PATRIOT_LASER_SEGMENTS as usize);
        let pack = LaserSegmentUpload::pack_line3d_segments(&segs);
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert_eq!(pack.honesty.segments_packed, PATRIOT_LASER_SEGMENTS);
        assert_eq!(
            pack.honesty.vertices_packed,
            PATRIOT_LASER_SEGMENTS * LASER_VERTS_PER_SEGMENT as u32
        );
        assert_eq!(
            pack.vertex_bytes.len(),
            (PATRIOT_LASER_SEGMENTS as usize) * LASER_BYTES_PER_SEGMENT
        );
        // First vertex position should be near start (ground skim may raise Z).
        let x0 = f32::from_le_bytes(pack.vertex_bytes[0..4].try_into().unwrap());
        assert!(x0.abs() < 1.0, "start X residual: {x0}");
    }

    #[test]
    fn packs_presentation_beams_from_assist_lasers() {
        let beams_host = make_patriot_assist_lasers(
            ObjectId(1),
            ObjectId(2),
            ObjectId(3),
            (0.0, 0.0, 5.0),
            (50.0, 0.0, 5.0),
            (100.0, 0.0, 5.0),
            0,
        );
        let beams: Vec<PresentationLaserBeam> = beams_host
            .iter()
            .enumerate()
            .map(|(i, l)| PresentationLaserBeam::from_host_laser(l, i as u32, 0.0))
            .collect();
        assert_eq!(beams.len(), 2);
        assert_eq!(
            beams[0].segments.len(),
            PATRIOT_LASER_SEGMENTS as usize
        );
        let pack = LaserSegmentUpload::pack_beams(&beams);
        assert!(pack.honesty.honesty_geometry_ok());
        assert_eq!(pack.honesty.beams_packed, 2);
        assert_eq!(
            pack.honesty.segments_packed,
            PATRIOT_LASER_SEGMENTS * 2
        );
        let mut marked = pack;
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
    }

    #[test]
    fn pack_from_empty_presentation_is_honest() {
        // No GameLogic: construct empty frame-like via pack_beams([]).
        let pack = LaserSegmentUpload::pack_beams(&[]);
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.has_geometry);
    }
}
