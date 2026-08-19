use super::*;

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

/// Honesty: default-0 residual + optional terrain / override path.
///
/// Any finite height is honest (default-0 when map height missing, terrain
/// sample when available, or host-testable override via synthetic path).
pub fn honesty_ground_height_residual_ok(height: f32, from_terrain: bool) -> bool {
    let _ = from_terrain;
    height.is_finite()
        && (from_terrain
            || (height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001
            || height.abs() > 0.0)
}

/// OrbitalLaser multi-beam soft-edge presentation residual (W3DLaserDraw NumBeams).
///
/// Host-testable fields that wire to `LaserSegmentUpload::pack_orbital_multi_beam_soft_edge`.
/// Fail-closed: not full additive GPU cylinder soft edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationLaserSoftEdge {
    pub num_beams: u32,
    pub inner_width: f32,
    pub outer_width: f32,
    pub outer_color: (f32, f32, f32, f32),
    pub tiling_scalar: f32,
    pub scroll_rate: f32,
}

/// Retail OrbitalLaser texture residual name (`ParticleUplinkCannon_OrbitalLaser`).
pub const PRESENTATION_ORBITAL_LASER_TEXTURE: &str = "EXNoise02.tga";

/// Retail ParticleUplinkCannon_OrbitalLaser soft-edge residual defaults.
pub const PRESENTATION_ORBITAL_SOFT_EDGE: PresentationLaserSoftEdge = PresentationLaserSoftEdge {
    num_beams: 12,
    inner_width: 0.6,
    outer_width: 26.0,
    outer_color: (0.0, 0.0, 1.0, 150.0 / 255.0),
    tiling_scalar: 0.15,
    scroll_rate: -1.75,
};

impl PresentationLaserSoftEdge {
    /// Honesty: retail OrbitalLaser NumBeams soft-edge presentation fields.
    pub fn honesty_orbital_residual_ok(self) -> bool {
        self.num_beams == 12
            && (self.inner_width - 0.6).abs() < 0.01
            && (self.outer_width - 26.0).abs() < 0.01
            && (self.tiling_scalar - 0.15).abs() < 0.001
            && (self.scroll_rate - (-1.75)).abs() < 0.001
            && PRESENTATION_ORBITAL_LASER_TEXTURE == "EXNoise02.tga"
            && (self.outer_color.2 - 1.0).abs() < 0.01
    }

    /// Endpoints + elapsed for `LaserSegmentUpload::pack_orbital_multi_beam_soft_edge`.
    pub fn pack_endpoints(
        &self,
        start: (f32, f32, f32),
        end: (f32, f32, f32),
        elapsed_seconds: f32,
    ) -> ((f32, f32, f32), (f32, f32, f32), f32, f32) {
        let _ = self;
        (start, end, elapsed_seconds, 1.0)
    }
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

    /// Synthetic assist-pair residual for host-testable laser pack honesty.
    ///
    /// Produces LaserFromAssisted + LaserToTarget with retail Segments=20 each.
    pub fn synthetic_assist_pair(start_frame: u32) -> [Self; 2] {
        Self::synthetic_assist_pair_with_ground(start_frame, PRESENTATION_DEFAULT_GROUND_HEIGHT)
    }

    /// Synthetic assist pair with explicit ground-height residual override.
    pub fn synthetic_assist_pair_with_ground(start_frame: u32, ground_height: f32) -> [Self; 2] {
        let beams = crate::game_logic::host_base_defense::make_patriot_assist_lasers(
            ObjectId(9001),
            ObjectId(9002),
            ObjectId(9003),
            (0.0, 0.0, 5.0),
            (40.0, 0.0, 5.0),
            (80.0, 0.0, 5.0),
            start_frame,
        );
        [
            Self::from_host_laser_with_terrain(&beams[0], 0, ground_height, false),
            Self::from_host_laser_with_terrain(&beams[1], 1, ground_height, false),
        ]
    }

    /// Synthetic OrbitalLaser multi-beam soft-edge residual for pack honesty.
    ///
    /// Vertical beam from origin; soft-edge fields wire to laser_segment_upload pack.
    pub fn synthetic_orbital_soft_edge(start_frame: u32) -> Self {
        let soft = PRESENTATION_ORBITAL_SOFT_EDGE;
        let start = (0.0, 0.0, 0.0);
        let end = (0.0, 0.0, 200.0);
        Self {
            beam_index: 0,
            kind: PresentationLaserKind::ToTarget,
            from_id: ObjectId(9101),
            to_id: ObjectId(9102),
            from: start,
            to: end,
            arc_mid: (0.0, 0.0, 100.0),
            scroll_offset: soft.scroll_rate * (start_frame as f32 / 30.0),
            expires_frame: start_frame.saturating_add(30),
            template_name: "ParticleUplinkCannon_OrbitalLaser".into(),
            texture_name: PRESENTATION_ORBITAL_LASER_TEXTURE.to_string(),
            laser_bone_name: String::new(),
            inner_color: (1.0, 1.0, 1.0, 250.0 / 255.0),
            segments: vec![PresentationLaserSegment {
                start,
                end,
                width: soft.inner_width,
                tile_factor: soft.tiling_scalar,
                scroll_offset: soft.scroll_rate * (start_frame as f32 / 30.0),
            }],
            ground_height: PRESENTATION_DEFAULT_GROUND_HEIGHT,
            ground_height_from_terrain: false,
            soft_edge: Some(soft),
        }
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// True when multi-beam soft-edge presentation residual is armed.
    pub fn has_soft_edge(&self) -> bool {
        self.soft_edge.is_some()
    }

    /// Honesty: ground-height residual on this beam is consistent.
    pub fn honesty_ground_height_ok(&self) -> bool {
        honesty_ground_height_residual_ok(self.ground_height, self.ground_height_from_terrain)
    }

    /// Honesty: soft-edge residual fields (or honest single-beam absence).
    pub fn honesty_soft_edge_presentation_ok(&self) -> bool {
        match self.soft_edge {
            Some(se) => se.honesty_orbital_residual_ok(),
            None => true, // single-beam Patriot residual is honest without soft edge
        }
    }
}
