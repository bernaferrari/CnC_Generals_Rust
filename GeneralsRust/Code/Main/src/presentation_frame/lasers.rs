use super::*;

#[cfg(all(test, feature = "game_client"))]
#[path = "owned_laser_scene_tests.rs"]
mod owned_laser_scene_tests;

/// Frozen W3DLaserDraw / Tracer / Rope line from GameClient RenderBridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSceneLine {
    pub start: (f32, f32, f32),
    pub end: (f32, f32, f32),
    pub width: f32,
    pub color: (f32, f32, f32, f32),
    pub texture_name: String,
    pub tile_factor: f32,
    #[serde(default)]
    pub scroll_rate: f32,
}

impl PresentationSceneLine {
    /// Freeze a Core W3D descriptor in Main's Y-up renderer coordinates.
    /// Core FX endpoints and terrain are C++ Z-up; this is the one output
    /// conversion after the canonical client module has produced its geometry.
    pub fn from_cpp_desc(line: game_engine::common::system::SceneLineDesc) -> Option<Self> {
        line.visible.then(|| Self {
            start: (line.start.x, line.start.z, line.start.y),
            end: (line.end.x, line.end.z, line.end.y),
            width: line.width,
            color: (line.color_r, line.color_g, line.color_b, line.opacity),
            texture_name: line.texture_name.unwrap_or_default(),
            tile_factor: line.tile_factor,
            scroll_rate: line.scroll_rate,
        })
    }
}

#[cfg(feature = "game_client")]
impl From<game_client::render_bridge::FrozenSceneLine> for PresentationSceneLine {
    fn from(line: game_client::render_bridge::FrozenSceneLine) -> Self {
        // The bridge's remaining producers are W3DLaserDraw, W3DTracerDraw
        // and W3DRopeDraw. Their descriptors all use original C++ Z-up.
        Self {
            start: (line.start[0], line.start[2], line.start[1]),
            end: (line.end[0], line.end[2], line.end[1]),
            width: line.width,
            color: (line.color[0], line.color[1], line.color[2], line.color[3]),
            texture_name: line.texture_name,
            tile_factor: line.tile_factor,
            scroll_rate: line.scroll_rate,
        }
    }
}

/// Freeze actual installed LaserUpdate/W3DLaserDraw geometry from the
/// receiving client. Texture definitions and terrain are supplied by their
/// owners; geometry never discovers a global drawable or advances simulation.
#[cfg(feature = "game_client")]
pub(crate) fn freeze_owned_laser_scene_lines(
    client: &mut game_client::core::game_client::GameClient,
    texture_aspect: &dyn Fn(&str) -> Result<f32, String>,
    terrain_height: &mut dyn FnMut(f32, f32) -> Option<f32>,
) -> Result<Vec<PresentationSceneLine>, String> {
    client
        .owned_ray_geometry(texture_aspect, terrain_height)
        .map(|lines| {
            lines
                .into_iter()
                .filter_map(PresentationSceneLine::from_cpp_desc)
                .collect()
        })
        .map_err(|error| error.to_string())
}

/// Frozen C++ `W3DStatusCircle` camera fade (ADD/SUBTRACT/SATURATE/MULTIPLY).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationCameraFade {
    /// `TFade` discriminant: 0 none, 1 subtract, 2 add, 3 saturate, 4 multiply.
    pub fade: u8,
    pub intensity: f32,
    pub diffuse: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserSegment {
    pub start: (f32, f32, f32),
    pub end: (f32, f32, f32),
    pub width: f32,
    pub tile_factor: f32,
    pub scroll_offset: f32,
}

/// Default Line3D ground-skim residual when map height is unavailable.
///
/// C++ samples terrain; host residual defaults to **0** and optionally overrides
/// when `GameLogic::terrain_height_at` returns a sample.
pub const PRESENTATION_DEFAULT_GROUND_HEIGHT: f32 = 0.0;

/// Sample residual ground height for laser Line3D skim.
///
/// Prefer map terrain height when available; else default-0 (honest residual).
/// Fail-closed: not full HeightMap bilinear / bridge-aware sample.
pub fn sample_presentation_ground_height(logic: &GameLogic, world_pos: Vec3) -> (f32, bool) {
    match logic.terrain_height_at(world_pos) {
        Some(h) if h.is_finite() => (h, true),
        _ => (PRESENTATION_DEFAULT_GROUND_HEIGHT, false),
    }
}

/// Optional multi-beam fields in serialized presentation snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserSoftEdge {
    pub num_beams: u32,
    pub inner_width: f32,
    pub outer_width: f32,
    pub outer_color: (f32, f32, f32, f32),
    pub tiling_scalar: f32,
    pub scroll_rate: f32,
}

/// Snapshot-owned PatriotBinaryDataStream / assist laser beam for client draw.
///
/// Built only from host residual lasers at presentation build time so the
/// SegLine pack path does not re-read live GameLogic mid-render.
/// Fail-closed: not full W3DLaserDraw WGPU texture sample / multi-beam soft edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserBeam {
    /// Stable presentation index (order among active beams this frame).
    pub beam_index: u32,
    pub kind: PresentationLaserKind,
    pub from_id: ObjectId,
    pub to_id: ObjectId,
    pub from: (f32, f32, f32),
    pub to: (f32, f32, f32),
    pub arc_mid: (f32, f32, f32),
    pub scroll_offset: f32,
    pub expires_frame: u32,
    pub template_name: String,
    pub texture_name: String,
    /// C++ Weapon.ini LaserBoneName residual (empty for Patriot assist beams).
    #[serde(default)]
    pub laser_bone_name: String,
    pub inner_color: (f32, f32, f32, f32),
    pub segments: Vec<PresentationLaserSegment>,
    /// Line3D ground-skim residual used when segments were built.
    pub ground_height: f32,
    /// True when `ground_height` came from terrain sample (not default-0).
    pub ground_height_from_terrain: bool,
    /// Optional multi-beam soft-edge presentation residual (OrbitalLaser family).
    /// None for single-beam Patriot BinaryDataStream residual.
    pub soft_edge: Option<PresentationLaserSoftEdge>,
}

/// Assist laser kind frozen for presentation (mirrors host residual enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationLaserKind {
    FromAssisted,
    ToTarget,
    /// Weapon.ini LaserName combat residual (not Patriot assist pair).
    WeaponLaser,
}

impl PresentationLaserKind {
    pub fn from_host(kind: PatriotAssistLaserKind) -> Self {
        match kind {
            PatriotAssistLaserKind::FromAssisted => Self::FromAssisted,
            PatriotAssistLaserKind::ToTarget => Self::ToTarget,
        }
    }
}

impl PresentationLaserBeam {
    /// Build from host residual laser + ground height (Line3D skim residual).
    pub fn from_host_laser(
        laser: &ResidualPatriotAssistLaser,
        beam_index: u32,
        ground_height: f32,
    ) -> Self {
        Self::from_host_laser_with_terrain(laser, beam_index, ground_height, false)
    }

    /// Build from host residual laser with terrain-sample honesty flag.
    /// Build from Weapon.ini LaserName residual beam.
    pub fn from_weapon_laser(
        laser: &crate::game_logic::host_weapon_laser::ResidualWeaponLaser,
        beam_index: u32,
        ground_height: f32,
        ground_height_from_terrain: bool,
    ) -> Self {
        use crate::game_logic::host_base_defense::build_patriot_laser_line3d_segments;
        let host_segs = build_patriot_laser_line3d_segments(
            laser.from_pos(),
            laser.to_pos(),
            0.0, // combat lasers are straight residual (no Patriot arc)
            laser.scroll_offset,
            ground_height,
        );
        let segments = host_segs
            .into_iter()
            .map(|s| PresentationLaserSegment {
                start: s.start,
                end: s.end,
                width: s.width,
                tile_factor: s.tile_factor,
                scroll_offset: s.scroll_offset,
            })
            .collect();
        let mid = (
            (laser.from_x + laser.to_x) * 0.5,
            (laser.from_y + laser.to_y) * 0.5,
            (laser.from_z + laser.to_z) * 0.5,
        );
        Self {
            beam_index,
            kind: PresentationLaserKind::WeaponLaser,
            from_id: laser.from_id,
            to_id: laser.to_id.unwrap_or(ObjectId(0)),
            from: laser.from_pos(),
            to: laser.to_pos(),
            arc_mid: mid,
            scroll_offset: laser.scroll_offset,
            expires_frame: laser.expires_frame,
            template_name: laser.laser_name.clone(),
            texture_name: laser.laser_name.clone(),
            laser_bone_name: laser.laser_bone_name.clone(),
            inner_color: (1.0, 0.2, 0.2, 1.0),
            segments,
            ground_height,
            ground_height_from_terrain,
            soft_edge: None,
        }
    }

    pub fn from_host_laser_with_terrain(
        laser: &ResidualPatriotAssistLaser,
        beam_index: u32,
        ground_height: f32,
        ground_height_from_terrain: bool,
    ) -> Self {
        let host_segs = build_patriot_laser_line3d_segments(
            (laser.from_x, laser.from_y, laser.from_z),
            (laser.to_x, laser.to_y, laser.to_z),
            laser.arc_height(),
            laser.scroll_offset,
            ground_height,
        );
        let segments = host_segs
            .into_iter()
            .map(|s| PresentationLaserSegment {
                start: s.start,
                end: s.end,
                width: s.width,
                tile_factor: s.tile_factor,
                scroll_offset: s.scroll_offset,
            })
            .collect();
        Self {
            beam_index,
            kind: PresentationLaserKind::from_host(laser.kind),
            from_id: laser.from_id,
            to_id: laser.to_id,
            from: (laser.from_x, laser.from_y, laser.from_z),
            to: (laser.to_x, laser.to_y, laser.to_z),
            arc_mid: (laser.arc_mid_x, laser.arc_mid_y, laser.arc_mid_z),
            scroll_offset: laser.scroll_offset,
            expires_frame: laser.expires_frame,
            template_name: PATRIOT_BINARY_DATA_STREAM.to_string(),
            texture_name: PATRIOT_LASER_TEXTURE.to_string(),
            laser_bone_name: String::new(),
            inner_color: PATRIOT_LASER_INNER_COLOR,
            segments,
            ground_height,
            ground_height_from_terrain,
            soft_edge: None,
        }
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
}
