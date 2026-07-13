//! W3DLaserDraw / SegLineRenderer residual: pack presentation laser Line3D segments
//! into a CPU vertex buffer ready for WGPU upload.
//!
//! Host residual closed here (fail-closed vs full retail SegLineRenderer GPU draw):
//! - Interleaved vertex bytes from `PresentationFrame.laser_beams` / Line3D segments
//! - Honesty counters for beams / segments / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//! - Multi-beam soft-edge CPU residual (OrbitalLaser NumBeams width/color lerp)
//! - ScrollRate UV + TilingScalar tile factor residual on multi-beam layers
//!
//! Still residual:
//! - Actual `wgpu::Queue::write_buffer` against a live device/pipeline
//! - EXNoise02.tga / EXBinaryStream32.tga texture atlas GPU sample bind
//! - Full soft-edge additive shader blend on live W3D scene graph

use crate::game_logic::host_base_defense::{
    HostLaserLine3DSegment, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
};
use crate::presentation_frame::{PresentationFrame, PresentationLaserBeam};

// ---------------------------------------------------------------------------
// ParticleUplinkCannon_OrbitalLaser multi-beam soft-edge residual (W3DLaserDraw)
// ---------------------------------------------------------------------------

/// Retail OrbitalLaser NumBeams (overlapping cylinders).
pub const ORBITAL_LASER_NUM_BEAMS: u32 = 12;
/// Retail InnerBeamWidth.
pub const ORBITAL_LASER_INNER_BEAM_WIDTH: f32 = 0.6;
/// Retail OuterBeamWidth.
pub const ORBITAL_LASER_OUTER_BEAM_WIDTH: f32 = 26.0;
/// Retail ScrollRate (toward muzzle negative).
pub const ORBITAL_LASER_SCROLL_RATE: f32 = -1.75;
/// Retail TilingScalar.
pub const ORBITAL_LASER_TILING_SCALAR: f32 = 0.15;
/// Retail texture residual.
pub const ORBITAL_LASER_TEXTURE: &str = "EXNoise02.tga";
/// Retail InnerColor R:255 G:255 B:255 A:250 → normalized.
pub const ORBITAL_LASER_INNER_COLOR: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 250.0 / 255.0);
/// Retail OuterColor R:0 G:0 B:255 A:150 → normalized.
pub const ORBITAL_LASER_OUTER_COLOR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 150.0 / 255.0);

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
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// Multi-beam soft-edge residual: overlapping cylinder layers packed.
    pub multi_beam_layers: u32,
    /// Peak layer width residual (OuterBeamWidth × width_scalar at outer edge).
    pub multi_beam_peak_width: f32,
    /// Inner layer width residual.
    pub multi_beam_inner_width: f32,
    /// ScrollRate UV residual sampled for soft-edge pack.
    pub multi_beam_scroll_uv: f32,
    /// TilingScalar residual honesty.
    pub multi_beam_tiling_scalar: f32,
    /// True when multi-beam soft-edge residual was exercised honestly.
    pub multi_beam_soft_edge_ok: bool,
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

    pub fn honesty_multi_beam_soft_edge_ok(&self) -> bool {
        self.multi_beam_soft_edge_ok
            && self.multi_beam_layers == ORBITAL_LASER_NUM_BEAMS
            && (self.multi_beam_peak_width - ORBITAL_LASER_OUTER_BEAM_WIDTH).abs() < 0.01
            && (self.multi_beam_inner_width - ORBITAL_LASER_INNER_BEAM_WIDTH).abs() < 0.01
            && (self.multi_beam_tiling_scalar - ORBITAL_LASER_TILING_SCALAR).abs() < 0.001
    }
}

/// One multi-beam soft-edge layer residual (width + color lerp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiBeamLayerResidual {
    pub layer_index: u32,
    pub scale: f32,
    pub width: f32,
    pub color: (f32, f32, f32, f32),
    pub tile_factor: f32,
    pub scroll_uv: f32,
}

/// Retail W3DLaserDraw multi-beam soft-edge width/color lerp residual.
///
/// C++: `scale = i / (numBeams - 1)`;
/// `width = inner + scale * (outer - inner)`;
/// color/alpha lerp similarly. Fail-closed vs full additive GPU cylinders.
pub fn multi_beam_layer_residuals(
    num_beams: u32,
    inner_width: f32,
    outer_width: f32,
    inner_color: (f32, f32, f32, f32),
    outer_color: (f32, f32, f32, f32),
    segment_length: f32,
    tiling_scalar: f32,
    scroll_uv: f32,
    width_scalar: f32,
) -> Vec<MultiBeamLayerResidual> {
    let n = num_beams.max(1);
    let mut layers = Vec::with_capacity(n as usize);
    for i in 0..n {
        let (scale, width, color) = if n == 1 {
            (0.0, inner_width * width_scalar, inner_color)
        } else {
            let scale = i as f32 / (n as f32 - 1.0);
            let width =
                (inner_width + scale * (outer_width - inner_width)) * width_scalar;
            let color = (
                inner_color.0 + scale * (outer_color.0 - inner_color.0),
                inner_color.1 + scale * (outer_color.1 - inner_color.1),
                inner_color.2 + scale * (outer_color.2 - inner_color.2),
                inner_color.3 + scale * (outer_color.3 - inner_color.3),
            );
            (scale, width, color)
        };
        // C++ tileFactor = length/width * textureAspect * tilingScalar (aspect=1 residual).
        let tile_factor = if width > 1e-6 {
            (segment_length / width) * 1.0 * tiling_scalar
        } else {
            0.0
        };
        layers.push(MultiBeamLayerResidual {
            layer_index: i,
            scale,
            width,
            color,
            tile_factor,
            scroll_uv,
        });
    }
    layers
}

/// Honesty: OrbitalLaser multi-beam soft-edge residual params.
pub fn honesty_orbital_multi_beam_layers(layers: &[MultiBeamLayerResidual]) -> bool {
    if layers.len() != ORBITAL_LASER_NUM_BEAMS as usize {
        return false;
    }
    let inner = layers.first().unwrap();
    let outer = layers.last().unwrap();
    (inner.width - ORBITAL_LASER_INNER_BEAM_WIDTH).abs() < 0.01
        && (outer.width - ORBITAL_LASER_OUTER_BEAM_WIDTH).abs() < 0.01
        && (inner.scale - 0.0).abs() < 0.001
        && (outer.scale - 1.0).abs() < 0.001
        && (inner.color.0 - ORBITAL_LASER_INNER_COLOR.0).abs() < 0.01
        && (outer.color.2 - ORBITAL_LASER_OUTER_COLOR.2).abs() < 0.01
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
                multi_beam_soft_edge_ok: true, // empty is honest (no multi-beam claim)
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

    /// Pack OrbitalLaser multi-beam soft-edge residual for a single vertical segment.
    ///
    /// Emits `NumBeams` overlapping Line3D cylinders with retail width/color lerp.
    /// Fail-closed: not live WGPU texture atlas / additive soft-edge shader submit.
    pub fn pack_orbital_multi_beam_soft_edge(
        start: (f32, f32, f32),
        end: (f32, f32, f32),
        elapsed_seconds: f32,
        width_scalar: f32,
    ) -> Self {
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let dz = end.2 - start.2;
        let length = (dx * dx + dy * dy + dz * dz).sqrt();
        let scroll_uv = ORBITAL_LASER_SCROLL_RATE * elapsed_seconds;
        let layers = multi_beam_layer_residuals(
            ORBITAL_LASER_NUM_BEAMS,
            ORBITAL_LASER_INNER_BEAM_WIDTH,
            ORBITAL_LASER_OUTER_BEAM_WIDTH,
            ORBITAL_LASER_INNER_COLOR,
            ORBITAL_LASER_OUTER_COLOR,
            length,
            ORBITAL_LASER_TILING_SCALAR,
            scroll_uv,
            width_scalar,
        );
        let soft_ok = honesty_orbital_multi_beam_layers(&layers) && width_scalar > 0.0;

        let mut floats =
            Vec::with_capacity(layers.len() * LASER_VERTS_PER_SEGMENT * LASER_VERTEX_FLOATS);
        for layer in &layers {
            let host = HostLaserLine3DSegment {
                start,
                end,
                width: layer.width,
                tile_factor: layer.tile_factor,
                scroll_offset: layer.scroll_uv,
            };
            let verts = segment_to_vertices(&host, layer.color, layer.layer_index as f32);
            for v in verts {
                floats.extend_from_slice(&v.to_floats());
            }
        }
        let vertex_bytes = f32_slice_to_bytes(&floats);
        let bytes_packed = vertex_bytes.len() as u32;
        let segments_packed = layers.len() as u32;
        let vertices_packed = segments_packed.saturating_mul(LASER_VERTS_PER_SEGMENT as u32);
        let peak_width = layers.last().map(|l| l.width).unwrap_or(0.0);
        let inner_width = layers.first().map(|l| l.width).unwrap_or(0.0);
        Self {
            vertex_bytes,
            honesty: LaserSegmentUploadHonesty {
                beams_packed: 1,
                segments_packed,
                vertices_packed,
                bytes_packed,
                cpu_pack_ok: true,
                has_geometry: segments_packed > 0,
                texture_name: ORBITAL_LASER_TEXTURE.to_string(),
                gpu_upload_ready: false,
                multi_beam_layers: ORBITAL_LASER_NUM_BEAMS,
                multi_beam_peak_width: peak_width,
                multi_beam_inner_width: inner_width,
                multi_beam_scroll_uv: scroll_uv,
                multi_beam_tiling_scalar: ORBITAL_LASER_TILING_SCALAR,
                multi_beam_soft_edge_ok: soft_ok,
            },
        }
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
                ..Default::default()
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
                ..Default::default()
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

/// Pack OrbitalLaser multi-beam soft-edge residual using spawn/current frame residual.
///
/// Bridges special_power_strikes WidthGrow scalar + ScrollRate UV into the
/// presentation multi-beam soft-edge pack path.
pub fn pack_orbital_soft_edge_from_frames(
    start: (f32, f32, f32),
    end: (f32, f32, f32),
    spawn_frame: u32,
    current_frame: u32,
) -> LaserSegmentUpload {
    use crate::game_logic::special_power_strikes::{
        particle_orbital_laser_scroll_uv, particle_width_scalar,
    };
    let elapsed_sec = if current_frame <= spawn_frame {
        0.0
    } else {
        (current_frame - spawn_frame) as f32 / 30.0
    };
    let width_scalar = particle_width_scalar(spawn_frame, current_frame);
    let _scroll = particle_orbital_laser_scroll_uv(spawn_frame, current_frame);
    LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(start, end, elapsed_sec, width_scalar)
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

    #[test]
    fn orbital_multi_beam_soft_edge_residual_honesty() {
        let pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 200.0),
            1.0, // 1 second → ScrollRate UV = -1.75
            1.0,
        );
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert!(
            pack.honesty.honesty_multi_beam_soft_edge_ok(),
            "layers={} peak={} inner={} scroll={} tile={}",
            pack.honesty.multi_beam_layers,
            pack.honesty.multi_beam_peak_width,
            pack.honesty.multi_beam_inner_width,
            pack.honesty.multi_beam_scroll_uv,
            pack.honesty.multi_beam_tiling_scalar
        );
        assert_eq!(pack.honesty.segments_packed, ORBITAL_LASER_NUM_BEAMS);
        assert_eq!(pack.honesty.texture_name, ORBITAL_LASER_TEXTURE);
        assert!((pack.honesty.multi_beam_scroll_uv - ORBITAL_LASER_SCROLL_RATE).abs() < 0.001);
        // Outer layer tile factor residual: length/outer * tiling
        let expected_tile = (200.0 / ORBITAL_LASER_OUTER_BEAM_WIDTH) * ORBITAL_LASER_TILING_SCALAR;
        let layers = multi_beam_layer_residuals(
            ORBITAL_LASER_NUM_BEAMS,
            ORBITAL_LASER_INNER_BEAM_WIDTH,
            ORBITAL_LASER_OUTER_BEAM_WIDTH,
            ORBITAL_LASER_INNER_COLOR,
            ORBITAL_LASER_OUTER_COLOR,
            200.0,
            ORBITAL_LASER_TILING_SCALAR,
            ORBITAL_LASER_SCROLL_RATE,
            1.0,
        );
        assert!((layers.last().unwrap().tile_factor - expected_tile).abs() < 0.01);
        assert!(honesty_orbital_multi_beam_layers(&layers));
    }
}
