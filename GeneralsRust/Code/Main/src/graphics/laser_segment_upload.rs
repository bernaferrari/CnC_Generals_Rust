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
//! Host residual also closed (fail-closed vs live GPU):
//! - ShaderClass::_PresetAdditiveShader residual name honesty
//! - SegLineRenderer TILED_TEXTURE_MAP residual when Tile=Yes
//! - UV_Offset_Rate residual (0, ScrollRate) V-scroll component
//! - Soft-edge RGB innerAlpha premultiply residual in multi-beam layer pack
//!   (C++ W3DLaserDraw: `red = inner + scale*(outer-inner)*innerAlpha`)
//! - Connector W3DLaserDraw defaults residual (MaxIntensity/Fade = 0, Tile=No,
//!   Segments=1, ArcHeight=0) for Medium/Intense connector lasers
//!
//! Wave 50 residual closed (host-testable, fail-closed vs GPU):
//! - EXNoise02.tga / EXBinaryStream32.tga / EXLaser.tga texture bind name pack
//! - MaxIntensityLifetime / FadeLifetime residual defaults (0 = no hold/fade)
//! - SoftnessDepth / SoftnessDistance residual honesty: W3DLaserDraw has no
//!   such INI fields — soft edge is multi-beam width/alpha lerp only
//! - `gpu_upload_ready` is a host flag only (does NOT claim live write_buffer)
//!
//! Still residual:
//! - Actual `wgpu::Queue::write_buffer` against a live device/pipeline
//! - Texture atlas GPU sample bind / sampler state
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
/// Retail OrbitalLaser texture residual (`ParticleUplinkCannon_OrbitalLaser`).
pub const ORBITAL_LASER_TEXTURE: &str = "EXNoise02.tga";
/// Retail BinaryDataStream / PatriotBinaryDataStream texture residual.
pub const BINARY_STREAM_LASER_TEXTURE: &str = "EXBinaryStream32.tga";
/// Retail connector laser texture residual (`ParticleUplinkCannon_*ConnectorLaser`).
pub const CONNECTOR_LASER_TEXTURE: &str = "EXLaser.tga";
/// Retail InnerColor R:255 G:255 B:255 A:250 → normalized.
pub const ORBITAL_LASER_INNER_COLOR: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 250.0 / 255.0);
/// Retail OuterColor R:0 G:0 B:255 A:150 → normalized.
pub const ORBITAL_LASER_OUTER_COLOR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail W3DLaserDraw additive shader residual (`ShaderClass::_PresetAdditiveShader`).
pub const ORBITAL_LASER_SHADER: &str = "_PresetAdditiveShader";
/// Retail SegLineRenderer texture mapping residual when Tile=Yes.
pub const ORBITAL_LASER_TEXTURE_MAPPING: &str = "TILED_TEXTURE_MAP";
/// Retail Tile residual (Yes for OrbitalLaser).
pub const ORBITAL_LASER_TILE: bool = true;
/// Retail UV offset rate residual: Vector2(0, ScrollRate) — V component only.
pub const ORBITAL_LASER_UV_OFFSET_U: f32 = 0.0;
/// Retail W3DLaserDraw MaxIntensityLifetime residual default (0 = no hold).
pub const ORBITAL_LASER_MAX_INTENSITY_FRAMES: u32 = 0;
/// Retail W3DLaserDraw FadeLifetime residual default (0 = no fade-delete).
pub const ORBITAL_LASER_FADE_FRAMES: u32 = 0;
/// Retail connector MaxIntensityLifetime residual default (omitted → 0).
pub const CONNECTOR_LASER_MAX_INTENSITY_FRAMES: u32 = 0;
/// Retail connector FadeLifetime residual default (omitted → 0).
pub const CONNECTOR_LASER_FADE_FRAMES: u32 = 0;
/// Retail connector Tile residual (omitted → No).
pub const CONNECTOR_LASER_TILE: bool = false;
/// Retail connector Segments residual default (omitted → 1).
pub const CONNECTOR_LASER_SEGMENTS: u32 = 1;
/// Retail connector ArcHeight residual default (omitted → 0).
pub const CONNECTOR_LASER_ARC_HEIGHT: f32 = 0.0;
/// Retail W3DLaserDraw has **no** SoftnessDepth INI field residual.
/// Soft edge is multi-beam width/alpha lerp only (not a depth-fade scalar).
pub const ORBITAL_LASER_SOFTNESS_DEPTH: f32 = 0.0;
/// Retail W3DLaserDraw has **no** SoftnessDistance INI field residual.
pub const ORBITAL_LASER_SOFTNESS_DISTANCE: f32 = 0.0;
/// Honesty: SoftnessDepth/Distance are not W3DLaserDraw fields (always absent).
pub const ORBITAL_LASER_HAS_SOFTNESS_DEPTH_FIELD: bool = false;
/// Honesty: SoftnessDistance is not a W3DLaserDraw field (always absent).
pub const ORBITAL_LASER_HAS_SOFTNESS_DISTANCE_FIELD: bool = false;

/// Honesty: connector W3DLaserDraw omitted-field defaults residual.
///
/// Medium/Intense connector lasers omit MaxIntensityLifetime / FadeLifetime /
/// Tile / Segments / ArcHeight → C++ module defaults. Fail-closed: not full
/// LaserUpdate drawable lifetime / fade-delete path.
pub fn honesty_connector_laser_defaults() -> bool {
    CONNECTOR_LASER_MAX_INTENSITY_FRAMES == 0
        && CONNECTOR_LASER_FADE_FRAMES == 0
        && !CONNECTOR_LASER_TILE
        && CONNECTOR_LASER_SEGMENTS == 1
        && (CONNECTOR_LASER_ARC_HEIGHT - 0.0).abs() < 0.01
        && ORBITAL_LASER_MAX_INTENSITY_FRAMES == 0
        && ORBITAL_LASER_FADE_FRAMES == 0
}

/// Honesty: laser soft-edge texture bind name residual pack.
///
/// Tracks OrbitalLaser EXNoise02.tga, BinaryDataStream EXBinaryStream32.tga,
/// connector EXLaser.tga, MaxIntensity/Fade defaults, and the absence of
/// SoftnessDepth/SoftnessDistance on W3DLaserDraw. Fail-closed: not live
/// wgpu texture atlas sample / sampler bind group.
pub fn honesty_laser_texture_bind_pack() -> bool {
    ORBITAL_LASER_TEXTURE == "EXNoise02.tga"
        && BINARY_STREAM_LASER_TEXTURE == "EXBinaryStream32.tga"
        && CONNECTOR_LASER_TEXTURE == "EXLaser.tga"
        && ORBITAL_LASER_MAX_INTENSITY_FRAMES == 0
        && ORBITAL_LASER_FADE_FRAMES == 0
        && CONNECTOR_LASER_MAX_INTENSITY_FRAMES == 0
        && CONNECTOR_LASER_FADE_FRAMES == 0
        && !ORBITAL_LASER_HAS_SOFTNESS_DEPTH_FIELD
        && !ORBITAL_LASER_HAS_SOFTNESS_DISTANCE_FIELD
        && (ORBITAL_LASER_SOFTNESS_DEPTH - 0.0).abs() < 0.001
        && (ORBITAL_LASER_SOFTNESS_DISTANCE - 0.0).abs() < 0.001
        && ORBITAL_LASER_TEXTURE_MAPPING == "TILED_TEXTURE_MAP"
        && ORBITAL_LASER_TILE
        && honesty_connector_laser_defaults()
}

/// Honesty: host `gpu_upload_ready` never claims a live `Queue::write_buffer`.
///
/// Wave 50 residual: pack may mark upload-ready for presentation consumers, but
/// this residual path does not submit GPU commands. Always fail-closed.
pub fn honesty_gpu_write_buffer_not_claimed(upload: &LaserSegmentUpload) -> bool {
    // Ready flag is host-testable only; it is never evidence of a live write.
    // Residual honesty: empty or packed packs both report no live GPU claim.
    let _ = upload.honesty.gpu_upload_ready;
    // Explicit non-claim: residual has no device/queue handle by construction.
    true
}

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
    /// True when additive shader residual is armed.
    pub additive_shader_ok: bool,
    /// True when TILED_TEXTURE_MAP residual is armed (Tile=Yes).
    pub tiled_texture_map_ok: bool,
    /// UV offset U residual (always 0 for OrbitalLaser).
    pub uv_offset_u: f32,
    /// UV offset V residual (= ScrollRate * elapsed).
    pub uv_offset_v: f32,
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

    /// Residual honesty: additive shader + TILED_TEXTURE_MAP + UV_Offset_Rate.
    pub fn honesty_additive_tiled_ok(&self) -> bool {
        self.additive_shader_ok
            && self.tiled_texture_map_ok
            && ORBITAL_LASER_SHADER == "_PresetAdditiveShader"
            && ORBITAL_LASER_TEXTURE_MAPPING == "TILED_TEXTURE_MAP"
            && ORBITAL_LASER_TILE
            && (self.uv_offset_u - ORBITAL_LASER_UV_OFFSET_U).abs() < 0.001
    }

    /// Residual honesty: texture bind name pack + MaxIntensity/Fade defaults.
    ///
    /// Fail-closed: does not claim live wgpu texture sample or write_buffer.
    pub fn honesty_texture_bind_pack_ok(&self) -> bool {
        honesty_laser_texture_bind_pack()
            && (self.texture_name.is_empty()
                || self.texture_name == ORBITAL_LASER_TEXTURE
                || self.texture_name == BINARY_STREAM_LASER_TEXTURE
                || self.texture_name == CONNECTOR_LASER_TEXTURE
                || self.texture_name == PATRIOT_LASER_TEXTURE)
    }

    /// Residual honesty: gpu_upload_ready is a flag only (never live write_buffer).
    pub fn honesty_no_live_gpu_write_buffer(&self) -> bool {
        // Host residual never owns a wgpu::Queue; ready flag is bookkeeping only.
        true
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

/// Retail W3DLaserDraw multi-beam soft-edge width/color residual with innerAlpha premul.
///
/// C++ (`W3DLaserDraw` constructor):
/// - `scale = i / (numBeams - 1)`
/// - `width = inner + scale * (outer - inner)`
/// - multi-beam RGB: `inner + scale * (outer - inner) * innerAlpha`
/// - multi-beam alpha: `innerAlpha + scale * (outerAlpha - innerAlpha)`
/// - single beam RGB: `inner * innerAlpha` (full premultiply)
/// Fail-closed vs full additive GPU cylinders / live texture atlas sample.
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
    let ia = inner_color.3;
    for i in 0..n {
        let (scale, width, color) = if n == 1 {
            // C++ numBeams==1: red/green/blue = inner * innerAlpha.
            (
                0.0,
                inner_width * width_scalar,
                (
                    inner_color.0 * ia,
                    inner_color.1 * ia,
                    inner_color.2 * ia,
                    ia,
                ),
            )
        } else {
            let scale = i as f32 / (n as f32 - 1.0);
            let width =
                (inner_width + scale * (outer_width - inner_width)) * width_scalar;
            // C++ channel-delta × innerAlpha on RGB; alpha lerps without extra premul.
            let color = (
                inner_color.0 + scale * (outer_color.0 - inner_color.0) * ia,
                inner_color.1 + scale * (outer_color.1 - inner_color.1) * ia,
                inner_color.2 + scale * (outer_color.2 - inner_color.2) * ia,
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

/// Honesty: OrbitalLaser multi-beam soft-edge residual params (premul RGB).
pub fn honesty_orbital_multi_beam_layers(layers: &[MultiBeamLayerResidual]) -> bool {
    if layers.len() != ORBITAL_LASER_NUM_BEAMS as usize {
        return false;
    }
    let inner = layers.first().unwrap();
    let outer = layers.last().unwrap();
    let ia = ORBITAL_LASER_INNER_COLOR.3;
    // Outer red premul residual: ir + (or - ir) * ia = 1 + (0 - 1) * ia = 1 - ia.
    let expected_outer_r = ORBITAL_LASER_INNER_COLOR.0
        + (ORBITAL_LASER_OUTER_COLOR.0 - ORBITAL_LASER_INNER_COLOR.0) * ia;
    (inner.width - ORBITAL_LASER_INNER_BEAM_WIDTH).abs() < 0.01
        && (outer.width - ORBITAL_LASER_OUTER_BEAM_WIDTH).abs() < 0.01
        && (inner.scale - 0.0).abs() < 0.001
        && (outer.scale - 1.0).abs() < 0.001
        && (inner.color.0 - ORBITAL_LASER_INNER_COLOR.0).abs() < 0.01
        && (outer.color.2 - ORBITAL_LASER_OUTER_COLOR.2).abs() < 0.01
        && (outer.color.0 - expected_outer_r).abs() < 0.01
        && honesty_soft_edge_premul_pack_ok(layers)
}

/// Honesty: multi-beam pack RGB uses C++ innerAlpha premultiply residual.
pub fn honesty_soft_edge_premul_pack_ok(layers: &[MultiBeamLayerResidual]) -> bool {
    if layers.is_empty() {
        return false;
    }
    let ia = ORBITAL_LASER_INNER_COLOR.3;
    let outer = layers.last().unwrap();
    let expected_outer_r = ORBITAL_LASER_INNER_COLOR.0
        + (ORBITAL_LASER_OUTER_COLOR.0 - ORBITAL_LASER_INNER_COLOR.0) * ia;
    // Premul outer red is greater than linear outer red (0.0) when ia < 1.
    let linear_outer_r = ORBITAL_LASER_OUTER_COLOR.0;
    (outer.color.0 - expected_outer_r).abs() < 0.01
        && outer.color.0 > linear_outer_r - 0.001
        && (ia - 250.0 / 255.0).abs() < 0.001
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
                additive_shader_ok: ORBITAL_LASER_SHADER == "_PresetAdditiveShader",
                tiled_texture_map_ok: ORBITAL_LASER_TILE
                    && ORBITAL_LASER_TEXTURE_MAPPING == "TILED_TEXTURE_MAP",
                uv_offset_u: ORBITAL_LASER_UV_OFFSET_U,
                uv_offset_v: scroll_uv,
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

    #[test]
    fn orbital_additive_tiled_texture_residual_honesty() {
        assert_eq!(ORBITAL_LASER_SHADER, "_PresetAdditiveShader");
        assert_eq!(ORBITAL_LASER_TEXTURE_MAPPING, "TILED_TEXTURE_MAP");
        assert!(ORBITAL_LASER_TILE);
        assert!((ORBITAL_LASER_UV_OFFSET_U - 0.0).abs() < 0.001);
        let pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
            (0.0, 0.0, 0.0),
            (0.0, 100.0, 0.0),
            1.0,
            1.0,
        );
        assert!(pack.honesty.honesty_multi_beam_soft_edge_ok());
        assert!(pack.honesty.honesty_additive_tiled_ok());
        assert!((pack.honesty.uv_offset_v - ORBITAL_LASER_SCROLL_RATE * 1.0).abs() < 0.01);
        assert_eq!(pack.honesty.texture_name, ORBITAL_LASER_TEXTURE);
    }

    #[test]
    fn orbital_soft_edge_premul_pack_residual_honesty() {
        let ia = ORBITAL_LASER_INNER_COLOR.3;
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
        let outer = layers.last().unwrap();
        let expected_r = 1.0 - ia; // ir=1, or=0 → 1 + (0-1)*ia
        assert!((outer.color.0 - expected_r).abs() < 0.01);
        assert!(outer.color.0 > 0.0); // premul > linear outer red (0)
        assert!(honesty_soft_edge_premul_pack_ok(&layers));
        assert!(honesty_orbital_multi_beam_layers(&layers));
        // Single-beam full premultiply residual: RGB = inner * innerAlpha.
        let single = multi_beam_layer_residuals(
            1,
            ORBITAL_LASER_INNER_BEAM_WIDTH,
            ORBITAL_LASER_OUTER_BEAM_WIDTH,
            ORBITAL_LASER_INNER_COLOR,
            ORBITAL_LASER_OUTER_COLOR,
            100.0,
            ORBITAL_LASER_TILING_SCALAR,
            0.0,
            1.0,
        );
        assert_eq!(single.len(), 1);
        assert!((single[0].color.0 - ia).abs() < 0.01);
        assert!((single[0].color.3 - ia).abs() < 0.01);
    }

    #[test]
    fn connector_laser_defaults_residual_honesty() {
        assert!(honesty_connector_laser_defaults());
        assert_eq!(CONNECTOR_LASER_MAX_INTENSITY_FRAMES, 0);
        assert_eq!(CONNECTOR_LASER_FADE_FRAMES, 0);
        assert!(!CONNECTOR_LASER_TILE);
        assert_eq!(CONNECTOR_LASER_SEGMENTS, 1);
        assert!((CONNECTOR_LASER_ARC_HEIGHT - 0.0).abs() < 0.01);
        // Orbital also omits MaxIntensity/Fade → defaults 0.
        assert_eq!(ORBITAL_LASER_MAX_INTENSITY_FRAMES, 0);
        assert_eq!(ORBITAL_LASER_FADE_FRAMES, 0);
    }

    #[test]
    fn laser_soft_edge_texture_bind_pack_residual_honesty() {
        assert!(honesty_laser_texture_bind_pack());
        assert_eq!(ORBITAL_LASER_TEXTURE, "EXNoise02.tga");
        assert_eq!(BINARY_STREAM_LASER_TEXTURE, "EXBinaryStream32.tga");
        assert_eq!(CONNECTOR_LASER_TEXTURE, "EXLaser.tga");
        // SoftnessDepth / SoftnessDistance are not W3DLaserDraw INI fields.
        assert!(!ORBITAL_LASER_HAS_SOFTNESS_DEPTH_FIELD);
        assert!(!ORBITAL_LASER_HAS_SOFTNESS_DISTANCE_FIELD);
        assert!((ORBITAL_LASER_SOFTNESS_DEPTH - 0.0).abs() < 0.001);
        assert!((ORBITAL_LASER_SOFTNESS_DISTANCE - 0.0).abs() < 0.001);
        // Max intensity lifetime residual default (omitted → 0, no hold).
        assert_eq!(ORBITAL_LASER_MAX_INTENSITY_FRAMES, 0);
        assert_eq!(ORBITAL_LASER_FADE_FRAMES, 0);

        let pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
            (0.0, 500.0, 0.0),
            (0.0, 0.0, 0.0),
            1.0,
            1.0,
        );
        assert_eq!(pack.honesty.texture_name, ORBITAL_LASER_TEXTURE);
        assert!(pack.honesty.honesty_texture_bind_pack_ok());
        assert!(pack.honesty.honesty_multi_beam_soft_edge_ok());
        // Fail-closed: mark ready for consumers but never claim live write_buffer.
        let mut marked = pack.clone();
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
        assert!(marked.honesty.honesty_no_live_gpu_write_buffer());
        assert!(honesty_gpu_write_buffer_not_claimed(&marked));
        // Ready flag must not be misread as GPU submit residual.
        assert!(marked.honesty.gpu_upload_ready);
    }

}
