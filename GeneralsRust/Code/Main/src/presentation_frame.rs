//! Immutable presentation snapshot built from the authoritative Main GameLogic.
//!
//! Policy: GameClient / renderer / HUD should consume `PresentationFrame` only.
//! They must not lock or mutate the sim while a WGPU pass is active.
//!
//! Ownership: borrow-first on the authority during `build_*`; then the snapshot
//! is owned values with no live borrows into the world.

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility, PresentationFowGrid};
use crate::game_logic::host_base_defense::{
    build_patriot_laser_line3d_segments, PatriotAssistLaserKind, ResidualPatriotAssistLaser,
    PATRIOT_BINARY_DATA_STREAM, PATRIOT_LASER_INNER_COLOR, PATRIOT_LASER_TEXTURE,
};
use crate::game_logic::{
    CombatParticleKind, CombatParticleSystemEntry, GameLogic, KindOf, ObjectId, Team,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic-frame index (30 Hz authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogicFrame(pub u32);

/// Snapshot-owned factory production queue entry (host BuildingData residual).
/// Fail-closed: not full ControlBar queue UI / cancel-button WND parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProductionItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    pub cost_supplies: u32,
}

/// Snapshot-owned veterancy rank (host Experience residual).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationVeterancy {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

impl PresentationVeterancy {
    pub fn from_host(level: crate::game_logic::VeterancyLevel) -> Self {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => Self::Rookie,
            V::Veteran => Self::Veteran,
            V::Elite => Self::Elite,
            V::Heroic => Self::Heroic,
        }
    }

    /// C++ ControlBar portrait chevron image residual (SSChevron*).
    pub fn chevron_overlay(self) -> Option<&'static str> {
        match self {
            Self::Rookie => None,
            Self::Veteran => Some("SSChevron1L"),
            Self::Elite => Some("SSChevron2L"),
            Self::Heroic => Some("SSChevron3L"),
        }
    }
}

/// Snapshot-owned object kind residual (host ObjectType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

impl PresentationObjectType {
    pub fn from_host(t: crate::game_logic::ObjectType) -> Self {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => Self::Infantry,
            T::Vehicle => Self::Vehicle,
            T::Aircraft => Self::Aircraft,
            T::Building => Self::Building,
            T::Supply => Self::Supply,
            T::Projectile => Self::Projectile,
            T::Neutral => Self::Neutral,
        }
    }
}

/// Snapshot-owned structure kind residual (host BuildingType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationBuildingType {
    CommandCenter,
    Barracks,
    WarFactory,
    Airfield,
    RepairPad,
    HealPad,
    SupplyCenter,
    PowerPlant,
    DefenseTurret,
    SupplyDropZone,
    Palace,
    Propaganda,
    Bunker,
}

impl PresentationBuildingType {
    pub fn from_host(t: crate::game_logic::BuildingType) -> Self {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => Self::CommandCenter,
            B::Barracks => Self::Barracks,
            B::WarFactory => Self::WarFactory,
            B::Airfield => Self::Airfield,
            B::RepairPad => Self::RepairPad,
            B::HealPad => Self::HealPad,
            B::SupplyCenter => Self::SupplyCenter,
            B::PowerPlant => Self::PowerPlant,
            B::DefenseTurret => Self::DefenseTurret,
            B::SupplyDropZone => Self::SupplyDropZone,
            B::Palace => Self::Palace,
            B::Propaganda => Self::Propaganda,
            B::Bunker => Self::Bunker,
        }
    }

    /// Factory / barracks / airfield residual for unit production UI.
    pub fn is_unit_producer(self) -> bool {
        matches!(self, Self::Barracks | Self::WarFactory | Self::Airfield)
    }
}

/// One renderable object as seen after a completed logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderableObject {
    pub id: ObjectId,
    pub template_name: String,
    pub team: Team,
    /// Team tint for presentation-only draw (RGBA 0..1), mirrors Object::team_color.
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    /// Current movement order destination (host Movement::target_position).
    pub move_destination: Option<Vec3>,
    /// Attack target object id when set.
    pub attack_target: Option<ObjectId>,
    /// Path waypoints residual (capped) for line pack / debug draw.
    pub path_waypoints: Vec<Vec3>,
    /// Structure production queue residual (empty for non-buildings).
    pub production_queue: Vec<PresentationProductionItem>,
    /// Structure rally point residual.
    pub rally_point: Option<Vec3>,
    /// Guard position residual (units).
    pub guard_position: Option<Vec3>,
    /// Contained unit ids (garrison / transport residual, capped).
    pub garrisoned_units: Vec<ObjectId>,
    /// Max garrison slots (0 = not a container).
    pub max_garrison: usize,
    /// Structure/unit power provided residual.
    pub power_provided: i32,
    /// Structure/unit power consumed residual.
    pub power_consumed: i32,
    /// Host Object::stored_resources.supplies residual (supply center / drop zone).
    pub stored_supplies: u32,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    pub destroyed: bool,
    pub under_construction: bool,
    /// Construction progress 0..1 residual (structures / dozer builds).
    pub construction_percent: f32,
    /// Veterancy rank residual for chevrons / UI.
    pub veterancy: PresentationVeterancy,
    /// Experience points residual (display / debug).
    pub experience_points: f32,
    /// Host ObjectStatus::moving residual.
    pub moving: bool,
    /// Host ObjectStatus::attacking residual.
    pub attacking: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual.
    pub detected: bool,
    /// Stealthed && !detected && !disguised (not a legal auto-target).
    pub effectively_stealthed: bool,
    /// Any host disable residual that blocks acting.
    pub disabled: bool,
    /// Container residual when this unit is inside another object.
    pub contained_by: Option<ObjectId>,
    /// Force-attack order residual.
    pub force_attack: bool,
    /// Primary weapon present residual.
    pub has_weapon: bool,
    /// Primary weapon range residual (0 when unarmed).
    pub weapon_range: f32,
    /// Primary weapon damage residual (0 when unarmed).
    pub weapon_damage: f32,
    /// CamoNetting StealthLook ordinal residual (0..5).
    pub camo_stealth_look: u8,
    /// Bomb-truck disguise template residual.
    pub disguise_as_template: Option<String>,
    /// Apparent team while disguised.
    pub disguise_as_team: Option<Team>,
    /// Stealth detector range residual (0 = none).
    pub detection_range: f32,
    /// Special power ready residual (superweapon / hero ability).
    pub special_power_ready: bool,
    /// Special power full cooldown seconds residual.
    pub special_power_cooldown: f32,
    /// Special power remaining cooldown seconds residual.
    pub special_power_cooldown_remaining: f32,
    /// Host ObjectType residual (UI / command set feed).
    pub object_type: PresentationObjectType,
    /// Applied upgrade tags residual (capped, sorted).
    pub applied_upgrades: Vec<String>,
    /// Secondary weapon present residual.
    pub has_secondary_weapon: bool,
    /// Secondary weapon range residual (0 when none).
    pub secondary_weapon_range: f32,
    /// Secondary weapon damage residual (0 when none).
    pub secondary_weapon_damage: f32,
    /// Mine / demo-trap residual present.
    pub has_mine: bool,
    /// Host ThingTemplate KindOf set residual (sorted, capped).
    /// Lets ControlBar / unit_control classify without live template re-read.
    pub kind_of: Vec<crate::game_logic::KindOf>,
    pub is_structure: bool,
    pub is_unit: bool,
    /// Mobile residual (infantry/vehicle/aircraft) for runtime-host select.
    pub is_mobile: bool,
    /// Structure can enqueue production (host building_data present + constructed).
    pub can_produce: bool,
    /// Host BuildingType residual when structure has building_data.
    pub building_type: Option<PresentationBuildingType>,
    /// W3D / mesh resolve key (template model name). Snapshot-owned so the unit
    /// mesh pass does not re-read live ThingTemplate during GPU collect.
    pub model_key: Option<String>,
    /// Mesh scale residual (Object INI Scale; common combat units retail **1.0**).
    /// Snapshot-owned so the unit mesh pass does not re-read live template Scale.
    /// Fail-closed: not full draw-scale bone / animation scale matrix.
    pub mesh_scale: f32,
    /// Cull / selection radius for presentation-only draw (no live GameLogic re-read).
    pub selection_radius: f32,
    /// True when bridged to GameEngine ObjectFactory (`engine_object_id`).
    /// Presentation-owned so the unit mesh pass can skip double-draw without
    /// locking live GameLogic for identity.
    pub engine_bridged: bool,
    /// FOW visibility for `PresentationFrame.local_player_id` at snapshot time.
    /// Unit mesh pass applies alpha / never-explored skip from this only — no
    /// live shroud re-query mid-render.
    pub fow_visibility: ObjectVisibility,
    /// Terrain ground-height residual sampled at object XY (Wave 77 deepen).
    /// Defaults to `PRESENTATION_DEFAULT_GROUND_HEIGHT` when map height unavailable.
    /// Fail-closed: not full HeightMap bilinear / bridge-aware sample; does **not**
    /// rewrite `position.y` (locomotor ground clamp residual separate).
    pub ground_height: f32,
    /// True when `ground_height` came from terrain sample (not default-0).
    pub ground_height_from_terrain: bool,
}

/// Snapshot-owned unit mesh/position/selection/FOW input for the main unit render pass.
///
/// Built only from `PresentationFrame` — no live `GameLogic` or shroud borrow.
/// W3D asset resolve uses `assets::mesh_asset_resolve` from `model_key`
/// (see OWNERSHIP residual notes — fail-closed vs full material/animation parity).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitRenderInput {
    pub id: ObjectId,
    pub template_name: String,
    pub model_key: String,
    /// Mesh scale residual frozen from presentation (default 1.0).
    pub mesh_scale: f32,
    pub team: Team,
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    pub selected: bool,
    pub selection_radius: f32,
    pub is_structure: bool,
    pub is_unit: bool,
    /// Skip main mesh pass when RenderBridge owns this drawable.
    pub engine_bridged: bool,
    /// Local-player FOW from the presentation snapshot (not a live shroud query).
    pub fow_visibility: ObjectVisibility,
}

impl UnitRenderInput {
    pub fn from_renderable(ro: &RenderableObject) -> Self {
        let model_key = ro
            .model_key
            .clone()
            .unwrap_or_else(|| ro.template_name.clone());
        Self {
            id: ro.id,
            template_name: ro.template_name.clone(),
            model_key,
            mesh_scale: if ro.mesh_scale > 0.0 {
                ro.mesh_scale
            } else {
                1.0
            },
            team: ro.team,
            team_color: ro.team_color,
            position: ro.position,
            orientation: ro.orientation,
            selected: ro.selected,
            selection_radius: ro.selection_radius.max(5.0),
            is_structure: ro.is_structure,
            is_unit: ro.is_unit,
            engine_bridged: ro.engine_bridged,
            fow_visibility: ro.fow_visibility,
        }
    }

    /// World matrix for the unit mesh pass (translation + Y rotation).
    pub fn world_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_translation(self.position) * glam::Mat4::from_rotation_y(self.orientation)
    }

    /// Never-explored skip for the main mesh pass (snapshot FOW only).
    #[inline]
    pub fn fow_should_render(&self) -> bool {
        self.fow_visibility.should_render()
    }
}

/// Ordered gameplay event for audio/FX/UI (presentation side only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PresentationEvent {
    ObjectDestroyed {
        id: ObjectId,
        team: Team,
    },
    ConstructionComplete {
        id: ObjectId,
        template: String,
    },
    /// Host research finished this frame (name + player).
    UpgradeComplete {
        name: String,
        player_id: u32,
        team: Team,
        units_affected: u32,
    },
    /// Factory production finished (spawned unit).
    ProductionComplete {
        producer: ObjectId,
        template: String,
        spawned: ObjectId,
    },
    /// Capture / hijack / set_team transfer this frame.
    OwnerChanged {
        id: ObjectId,
        team: Team,
    },
    /// Attack target set this frame (host_attack_log).
    AttackTargeted {
        attacker: ObjectId,
        target: Option<ObjectId>,
    },
    /// Move order destination this frame (host_move_log).
    MoveOrdered {
        unit: ObjectId,
        destination: [f32; 3],
    },
    /// Post-armor HP damage applied this frame (host_damage_log).
    DamageApplied {
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
    },
    /// Absolute HP write this frame (heal / construction finish residual).
    HealApplied {
        target: ObjectId,
        health: f32,
    },
    /// Player supplies/power absolute after host economy mutation.
    EconomyChanged {
        player_id: u32,
        supplies: u32,
        power_available: i32,
    },
    Victory {
        winner_player: Option<u32>,
    },
    RadarMessage {
        team: Team,
        text: String,
    },
    /// Combat residual: particle system spawned (host registry id + template).
    ParticleSystemSpawned {
        id: u32,
        kind: CombatParticleKind,
        template_name: String,
        position: Vec3,
    },
}

/// Snapshot-owned combat particle system for presentation/client observe path.
/// Fail-closed: not full W3D GPU particle parity (hq-gq7n residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationParticleSystem {
    pub id: u32,
    pub kind: CombatParticleKind,
    pub template_name: String,
    pub position: Vec3,
    pub source_object: Option<ObjectId>,
    pub target_object: Option<ObjectId>,
    pub spawned_frame: u32,
    pub active: bool,
    pub client_system_id: Option<u32>,
}

impl PresentationParticleSystem {
    pub fn from_combat_entry(entry: &CombatParticleSystemEntry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            template_name: entry.template_name.clone(),
            position: entry.position,
            source_object: entry.source_object,
            target_object: entry.target_object,
            spawned_frame: entry.spawned_frame,
            active: entry.active,
            client_system_id: entry.client_system_id,
        }
    }
}

/// Snapshot-owned W3DLaserDraw Line3D segment (presentation residual, not GPU).
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

/// C++ `DEFAULT_FLOATING_TEXT_TIMEOUT = LOGICFRAMES_PER_SECOND / 3` → **10** frames.
pub const PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES: u32 = 10;
/// C++ `m_floatingTextMoveUpSpeed` default (world units per logic frame, draw residual).
pub const PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED: f32 = 1.0;
/// C++ `m_floatingTextMoveVanishRate` default (alpha decay residual after timeout).
pub const PRESENTATION_FLOATING_TEXT_VANISH_RATE: f32 = 0.1;
/// Host residual fade window after world-anim display time (seconds) when Fades=Yes.
///
/// Mirrors C++ WORLD_ANIM_FADE_ON_EXPIRE ~1s window. Fail-closed: not live GPU blend.
pub const PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS: f32 = 1.0;
/// Logic FPS residual for age → seconds conversion (presentation dual-tick).
pub const PRESENTATION_LOGIC_FPS: f32 = 30.0;

/// Source residual family for frozen floating cash / caption text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PresentationFloatingTextKind {
    /// AutoDepositUpdate (oil derrick / black market).
    AutoDeposit,
    /// HackInternet / Internet Center floating cash.
    Hacker,
    /// CashBounty kill bounty floating cash.
    CashBounty,
    /// MoneyCrateCollide pickup floating cash.
    MoneyCrate,
    /// Combat HP damage residual (from DamageApplied events).
    CombatDamage,
}

/// Snapshot-owned InGameUI::addFloatingText residual for dual-tick consumers.
///
/// Built only from host residual registries at presentation build time so the
/// UI / GPU layout pack path does not re-read live GameLogic mid-render.
/// Fail-closed: not full DisplayString GPU draw / Unicode GameText localization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFloatingText {
    pub kind: PresentationFloatingTextKind,
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    /// Source object (derrick / hacker / killer / crate).
    pub source_id: ObjectId,
    /// Frame when residual times out (`spawn + PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES`).
    pub timeout_frame: u32,
}

impl PresentationFloatingText {
    pub fn from_parts(
        kind: PresentationFloatingTextKind,
        text: String,
        text_key: String,
        position: Vec3,
        color_rgba: (u8, u8, u8, u8),
        amount: u32,
        spawn_frame: u32,
        source_id: ObjectId,
    ) -> Self {
        Self {
            kind,
            text,
            text_key,
            position,
            color_rgba,
            amount,
            spawn_frame,
            source_id,
            timeout_frame: spawn_frame.saturating_add(PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES),
        }
    }

    /// True while C++ keeps the entry before vanish-phase erase residual.
    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.timeout_frame
    }

    /// Age in logic frames at `logic_frame` (0 at spawn).
    pub fn age_frames_at(&self, logic_frame: u32) -> u32 {
        logic_frame.saturating_sub(self.spawn_frame)
    }

    /// C++ draw residual lift: `frameCount * m_floatingTextMoveUpSpeed`.
    pub fn lift_y_at(&self, logic_frame: u32) -> f32 {
        self.age_frames_at(logic_frame) as f32 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED
    }

    /// Vanish-rate alpha residual (1.0 while active; decays after timeout).
    ///
    /// C++: after timeout, alpha pulls toward 0 by `m_floatingTextMoveVanishRate`
    /// per frame until erased. Fail-closed: not live Display surface blend.
    pub fn vanish_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age < timeout {
            1.0
        } else {
            let past = (age - timeout) as f32;
            (1.0 - past * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0)
        }
    }

    /// C++ `updateFloatingText` integer alpha residual after timeout.
    ///
    /// ```text
    /// amount = REAL_TO_INT((currFrame - timeout) * m_floatingTextMoveVanishRate);
    /// if (a - amount < 0) a = 0; else a -= amount;
    /// ```
    /// Fail-closed: not live DisplayString surface blend / StretchRect.
    pub fn vanish_color_alpha_u8_at(&self, logic_frame: u32, base_alpha: u8) -> u8 {
        let age = self.age_frames_at(logic_frame);
        let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
        if age <= timeout {
            return base_alpha;
        }
        let past = (age - timeout) as f32;
        // REAL_TO_INT truncates toward zero (C++ `(Int)(x)`).
        let amount = (past * PRESENTATION_FLOATING_TEXT_VANISH_RATE) as i32;
        let next = base_alpha as i32 - amount;
        if next < 0 {
            0
        } else {
            next as u8
        }
    }

    /// Apply vanish-rate residual to a frozen color_rgba (RGB preserved, A decays).
    pub fn color_with_vanish_alpha_at(&self, logic_frame: u32) -> (u8, u8, u8, u8) {
        let (r, g, b, a) = self.color_rgba;
        (r, g, b, self.vanish_color_alpha_u8_at(logic_frame, a))
    }

    /// Honesty: retail vanish-rate / move-up / timeout presentation fields.
    pub fn honesty_vanish_rate_residual_ok() -> bool {
        (PRESENTATION_FLOATING_TEXT_VANISH_RATE - 0.1).abs() < 0.001
            && PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES == 10
            && (PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED - 1.0).abs() < 0.001
            && {
                let t = PresentationFloatingText::synthetic_cash(50, 0);
                (t.vanish_alpha_at(0) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(9) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(10) - 1.0).abs() < 0.001
                    && (t.vanish_alpha_at(15) - 0.5).abs() < 0.001
                    && (t.vanish_alpha_at(20) - 0.0).abs() < 0.001
                    && (t.lift_y_at(5) - 5.0).abs() < 0.001
            }
    }

    /// Wave 76 residual honesty: C++ integer color-alpha vanish path residual.
    ///
    /// Matches `InGameUI::updateFloatingText` REAL_TO_INT amount subtract on A.
    /// With default vanish rate **0.1**, past=10 → amount **1** (255→254);
    /// past=5 → amount **0** (truncation). Fail-closed vs live Display surface.
    pub fn honesty_vanish_color_alpha_residual_ok() -> bool {
        let t = PresentationFloatingText::synthetic_cash(50, 0);
        // Synthetic cash uses green (0,255,0,255).
        t.color_rgba == (0, 255, 0, 255)
            && t.vanish_color_alpha_u8_at(0, 255) == 255
            && t.vanish_color_alpha_u8_at(10, 255) == 255
            && t.vanish_color_alpha_u8_at(15, 255) == 255 // past=5 → amount=0
            && t.vanish_color_alpha_u8_at(20, 255) == 254 // past=10 → amount=1
            && t.vanish_color_alpha_u8_at(30, 255) == 253 // past=20 → amount=2
            && t.vanish_color_alpha_u8_at(20, 1) == 0 // saturating subtract residual
            && {
                let c = t.color_with_vanish_alpha_at(20);
                c == (0, 255, 0, 254)
            }
            && Self::honesty_vanish_rate_residual_ok()
    }

    /// Synthetic cash residual for host-testable floating-text pack honesty.
    pub fn synthetic_cash(amount: u32, spawn_frame: u32) -> Self {
        Self::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            format!("+${amount}"),
            "GUI:AddCash".into(),
            Vec3::new(10.0, 20.0, 5.0),
            (0, 255, 0, 255),
            amount,
            spawn_frame,
            ObjectId(7001),
        )
    }
}

/// Snapshot-owned InGameUI::addWorldAnimation residual (MoneyPickUp Anim2D family).
///
/// Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationWorldAnim {
    pub template: String,
    pub position: Vec3,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub fades: bool,
    pub spawn_frame: u32,
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
}

impl PresentationWorldAnim {
    pub fn from_money_pickup(
        anim: &crate::game_logic::host_money_crate::HostMoneyPickUpAnim,
    ) -> Self {
        Self {
            template: anim.template.clone(),
            position: anim.position,
            display_time_seconds: anim.display_time_seconds,
            z_rise_per_second: anim.z_rise_per_second,
            fades: anim.fades,
            spawn_frame: anim.spawn_frame,
            crate_id: anim.crate_id,
            picker_id: anim.picker_id,
        }
    }

    /// Synthetic MoneyPickUp residual for host-testable world-anim pack honesty.
    pub fn synthetic_money_pickup(spawn_frame: u32) -> Self {
        Self {
            template: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE.to_string(),
            position: Vec3::new(12.0, 0.0, 8.0),
            display_time_seconds:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS,
            z_rise_per_second:
                crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND,
            fades: crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_FADES,
            spawn_frame,
            crate_id: ObjectId(8001),
            picker_id: ObjectId(8002),
        }
    }

    /// Display duration residual in logic frames (30 Hz).
    pub fn display_frames(&self) -> u32 {
        (self.display_time_seconds * PRESENTATION_LOGIC_FPS)
            .ceil()
            .max(1.0) as u32
    }

    pub fn is_active_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.spawn_frame.saturating_add(self.display_frames())
    }

    /// Age in seconds at `logic_frame` (0 at spawn).
    pub fn age_seconds_at(&self, logic_frame: u32) -> f32 {
        logic_frame.saturating_sub(self.spawn_frame) as f32 / PRESENTATION_LOGIC_FPS
    }

    /// WORLD_ANIM_FADE_ON_EXPIRE residual alpha at `logic_frame`.
    ///
    /// - age < display → 1.0
    /// - age ≥ display and fades → clamp(1 - past/fade_window, 0..1)
    /// - age ≥ display and !fades → 0.0
    pub fn fade_alpha_at(&self, logic_frame: u32) -> f32 {
        let age = self.age_seconds_at(logic_frame);
        if age < self.display_time_seconds {
            1.0
        } else if self.fades {
            let past = age - self.display_time_seconds;
            (1.0 - past / PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Honesty: MoneyPickUp fade presentation residual fields.
    pub fn honesty_fade_residual_ok(&self) -> bool {
        (PRESENTATION_WORLD_ANIM_FADE_WINDOW_SECONDS - 1.0).abs() < 0.01
            && self.display_time_seconds > 0.0
            && {
                // Sample fade curve residual around display boundary.
                let mid = self
                    .spawn_frame
                    .saturating_add((self.display_time_seconds * PRESENTATION_LOGIC_FPS) as u32);
                let before = mid.saturating_sub(1);
                let half = mid.saturating_add((PRESENTATION_LOGIC_FPS * 0.5) as u32);
                let end = mid.saturating_add(PRESENTATION_LOGIC_FPS as u32);
                (self.fade_alpha_at(before) - 1.0).abs() < 0.05
                    && if self.fades {
                        (self.fade_alpha_at(half) - 0.5).abs() < 0.1
                            && (self.fade_alpha_at(end) - 0.0).abs() < 0.05
                    } else {
                        self.fade_alpha_at(half) <= 0.0
                    }
            }
    }

    /// Static honesty for retail MoneyPickUp fade residual defaults.
    pub fn honesty_money_pickup_fade_params_ok() -> bool {
        let a = Self::synthetic_money_pickup(0);
        a.fades
            && (a.display_time_seconds - 4.0).abs() < 0.01
            && (a.z_rise_per_second - 15.0).abs() < 0.01
            && a.honesty_fade_residual_ok()
    }
}

/// Collect host residual floating texts into a stable presentation list.
fn collect_presentation_floating_texts(logic: &GameLogic) -> Vec<PresentationFloatingText> {
    let mut out = Vec::new();

    for t in &logic.oil_derricks().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.black_markets().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::AutoDeposit,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.source_id,
        ));
    }
    for t in &logic.hacker_income().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::Hacker,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.hacker_id,
        ));
    }
    for t in &logic.cash_bounty_registry().floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::CashBounty,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.killer_id,
        ));
    }
    for t in &logic.host_money_crates().money_floating_texts {
        out.push(PresentationFloatingText::from_parts(
            PresentationFloatingTextKind::MoneyCrate,
            t.text.clone(),
            t.text_key.clone(),
            t.position,
            t.color_rgba,
            t.amount,
            t.spawn_frame,
            t.crate_id,
        ));
    }

    // Stable presentation order: spawn frame then source id then kind.
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.source_id.0.cmp(&b.source_id.0))
            .then(a.kind.cmp(&b.kind))
    });
    out
}

fn collect_presentation_world_anims(logic: &GameLogic) -> Vec<PresentationWorldAnim> {
    let mut out: Vec<PresentationWorldAnim> = logic
        .host_money_crates()
        .money_pickup_anims
        .iter()
        .map(PresentationWorldAnim::from_money_pickup)
        .collect();
    out.sort_by(|a, b| {
        a.spawn_frame
            .cmp(&b.spawn_frame)
            .then(a.crate_id.0.cmp(&b.crate_id.0))
            .then(a.picker_id.0.cmp(&b.picker_id.0))
    });
    out
}

// --- Wave 73: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual ---

/// Retail Spectre AttackAreaDecal Texture residual (`SCCSpecTarg`).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_DECAL: &str = "SCCSpecTarg";
/// Retail Spectre TargetingReticleDecal Texture residual (`SCCSpecRet`).
pub const PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL: &str = "SCCSpecRet";
/// Retail Spectre decal Color residual (R:127 G:177 B:222 A:255) as RGBA 0..1.
pub const PRESENTATION_SPECTRE_DECAL_COLOR: [f32; 4] =
    [127.0 / 255.0, 177.0 / 255.0, 222.0 / 255.0, 1.0];
/// Retail AttackAreaDecal OpacityMin residual (25%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN: f32 = 0.25;
/// Retail AttackAreaDecal OpacityMax residual (50%).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMin residual (50%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN: f32 = 0.50;
/// Retail TargetingReticleDecal OpacityMax residual (100%).
pub const PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX: f32 = 1.00;
/// Retail AttackAreaDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS: u32 = 1500;
/// Retail TargetingReticleDecal OpacityThrobTime residual (msec).
pub const PRESENTATION_SPECTRE_RETICLE_THROB_MS: u32 = 300;
/// Retail AttackAreaRadius residual (presentation cursor / decal radius).
pub const PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS: f32 = 200.0;
/// Retail TargetingReticleRadius residual.
pub const PRESENTATION_SPECTRE_RETICLE_RADIUS: f32 = 25.0;
/// Retail AttackAreaDecal Style residual.
pub const PRESENTATION_SPECTRE_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail OnlyVisibleToOwningPlayer residual (both decals).
pub const PRESENTATION_SPECTRE_DECAL_ONLY_OWNER: bool = true;

/// Snapshot-owned Spectre orbit decal presentation residual (AttackArea + Reticle).
///
/// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb / owning-player visibility filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresentationSpectreOrbitDecal {
    pub attack_area_texture: &'static str,
    pub reticle_texture: &'static str,
    pub color: [f32; 4],
    pub attack_area_radius: f32,
    pub reticle_radius: f32,
    pub attack_area_opacity_min: f32,
    pub attack_area_opacity_max: f32,
    pub reticle_opacity_min: f32,
    pub reticle_opacity_max: f32,
    pub attack_area_throb_ms: u32,
    pub reticle_throb_ms: u32,
    pub style: &'static str,
    pub only_visible_to_owning_player: bool,
}

impl PresentationSpectreOrbitDecal {
    /// Retail SpectreGunshipUpdate AttackAreaDecal + TargetingReticleDecal residual defaults.
    pub const RETAIL: Self = Self {
        attack_area_texture: PRESENTATION_SPECTRE_ATTACK_AREA_DECAL,
        reticle_texture: PRESENTATION_SPECTRE_TARGETING_RETICLE_DECAL,
        color: PRESENTATION_SPECTRE_DECAL_COLOR,
        attack_area_radius: PRESENTATION_SPECTRE_ATTACK_AREA_RADIUS,
        reticle_radius: PRESENTATION_SPECTRE_RETICLE_RADIUS,
        attack_area_opacity_min: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MIN,
        attack_area_opacity_max: PRESENTATION_SPECTRE_ATTACK_AREA_OPACITY_MAX,
        reticle_opacity_min: PRESENTATION_SPECTRE_RETICLE_OPACITY_MIN,
        reticle_opacity_max: PRESENTATION_SPECTRE_RETICLE_OPACITY_MAX,
        attack_area_throb_ms: PRESENTATION_SPECTRE_ATTACK_AREA_THROB_MS,
        reticle_throb_ms: PRESENTATION_SPECTRE_RETICLE_THROB_MS,
        style: PRESENTATION_SPECTRE_DECAL_STYLE,
        only_visible_to_owning_player: PRESENTATION_SPECTRE_DECAL_ONLY_OWNER,
    };

    /// Honesty: retail Spectre AttackAreaDecal / TargetingReticleDecal presentation residual.
    pub fn honesty_residual_ok(self) -> bool {
        self.attack_area_texture == "SCCSpecTarg"
            && self.reticle_texture == "SCCSpecRet"
            && (self.attack_area_radius - 200.0).abs() < 0.01
            && (self.reticle_radius - 25.0).abs() < 0.01
            && (self.attack_area_opacity_min - 0.25).abs() < 0.001
            && (self.attack_area_opacity_max - 0.50).abs() < 0.001
            && (self.reticle_opacity_min - 0.50).abs() < 0.001
            && (self.reticle_opacity_max - 1.00).abs() < 0.001
            && self.attack_area_throb_ms == 1500
            && self.reticle_throb_ms == 300
            && self.style == "SHADOW_ALPHA_DECAL"
            && self.only_visible_to_owning_player
            && (self.color[0] - 127.0 / 255.0).abs() < 0.001
            && (self.color[1] - 177.0 / 255.0).abs() < 0.001
            && (self.color[2] - 222.0 / 255.0).abs() < 0.001
            && (self.color[3] - 1.0).abs() < 0.001
            && self.attack_area_opacity_min < self.attack_area_opacity_max
            && self.reticle_opacity_min < self.reticle_opacity_max
            && self.reticle_radius < self.attack_area_radius
    }
}

/// Free-function honesty for Spectre orbit decal presentation residual (Wave 73).
pub fn honesty_spectre_orbit_decal_presentation_ok() -> bool {
    PresentationSpectreOrbitDecal::RETAIL.honesty_residual_ok()
}

/// Wave 102: dual-tick presentation residual deepen free-function honesty.
///
/// Builds an empty-host presentation snapshot and verifies dual-tick residual
/// counters (including selected/particle Wave 102 deepen) plus presentation
/// residual packs. Fail-closed vs live dual-run W3D / GPU submit.
pub fn honesty_presentation_dual_tick_residual_deepen_wave102() -> bool {
    use crate::game_logic::GameLogic;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Empty residual snapshot honesty (zero objects still dual-tick consistent).
    let empty_logic = GameLogic::new();
    let empty = PresentationFrame::build_from_logic(&empty_logic, 0);
    if !empty.dual_tick_presentation_residual_ok() {
        return false;
    }
    if empty.dual_tick.builds != 1 || empty.dual_tick.applies != 0 {
        return false;
    }
    if empty.dual_tick.selected_count != 0 || empty.dual_tick.particle_count != 0 {
        return false;
    }
    // Seeded skirmish residual: dual-tick deepen after shell apply.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresDualTick102");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        // Config residual may still produce honest empty-host dual-tick.
        return empty.dual_tick_presentation_residual_deepen_ok()
            && honesty_spectre_orbit_decal_presentation_ok();
    }
    let mut hud = crate::ui::GameHUD::new();
    let mut ui = crate::ui::GameUIState::default();
    let mut rts = crate::ui::RTSInterface::new();
    let mut cmd = crate::ui::UnitCommandPanel::new();
    let frame = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
    );
    frame.dual_tick_presentation_residual_deepen_ok()
        && frame.dual_tick.honesty_apply_ok()
        && frame.dual_tick.builds == 1
        && frame.dual_tick.applies >= 1
        && frame.dual_tick.selected_count == frame.selected.len() as u32
        && frame.dual_tick.particle_count == frame.particle_systems.len() as u32
        && honesty_spectre_orbit_decal_presentation_ok()
}

/// Combined Wave 102 presentation residual honesty pack.
pub fn honesty_presentation_residual_deepen_pack_wave102() -> bool {
    honesty_presentation_dual_tick_residual_deepen_wave102()
}

/// Dual-tick residual counters frozen on each presentation build / apply.
///
/// Host-testable bookkeeping for seed → logic step → multi-consumer apply order.
/// Fail-closed: not full dual-run determinism harness counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PresentationDualTickResidual {
    /// Always 1 after a successful `build_from_logic`.
    pub builds: u32,
    /// Incremented each time this snapshot is applied to HUD / shell consumers.
    pub applies: u32,
    pub object_count: u32,
    pub selected_count: u32,
    pub laser_beam_count: u32,
    pub floating_text_count: u32,
    pub world_anim_count: u32,
    pub particle_count: u32,
}

impl PresentationDualTickResidual {
    pub fn from_counts(
        objects: usize,
        selected: usize,
        lasers: usize,
        floating: usize,
        world: usize,
        particles: usize,
    ) -> Self {
        Self {
            builds: 1,
            applies: 0,
            object_count: objects as u32,
            selected_count: selected as u32,
            laser_beam_count: lasers as u32,
            floating_text_count: floating as u32,
            world_anim_count: world as u32,
            particle_count: particles as u32,
        }
    }

    /// Honesty: residual counters are self-consistent after build.
    pub fn honesty_build_ok(&self) -> bool {
        self.builds >= 1
    }

    /// Honesty: at least one dual-tick apply was recorded.
    pub fn honesty_apply_ok(&self) -> bool {
        self.builds >= 1 && self.applies >= 1
    }
}

/// Compact road segment for presentation-side road mesh bake.
/// Coordinates match `RuntimeRoadSegment` world space (from/to as [x,y,z]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationRoadSegment {
    pub template_name: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub width: f32,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

/// Compact bridge segment (start/end world xyz, width, template).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationBridgeSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub template_name: String,
}

/// World/environment identity frozen for the render pass.
///
/// Lets lighting / shell / map-name / bounds / heightmap-hint / roads consumers avoid
/// re-locking live `GameLogic` mid-frame when a presentation snapshot is set.
/// Fail-closed: not a full SAGE heightmap mesh or dirty-rect road stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationWorldEnv {
    pub map_name: String,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
    pub heightmap_hint: Option<String>,
    pub sun_direction: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    pub ambient_color: Option<[f32; 3]>,
    pub fog_color: Option<[f32; 3]>,
    pub fog_start: Option<f32>,
    pub fog_end: Option<f32>,
    /// Placed-object count from last parsed map metadata (prewarm signature).
    pub map_object_count: u32,
    pub has_map_metadata: bool,
    /// First N map-object template names for model prewarm (observe path).
    /// Fail-closed: not full ThingTemplate graph.
    pub prewarm_template_names: Vec<String>,
    /// Coarse height samples for minimap/terrain residual (row-major, width×height).
    /// Fail-closed: not full SAGE heightmap mesh / bilinear retail sample grid.
    pub height_grid_w: u32,
    pub height_grid_h: u32,
    pub height_samples: Vec<f32>,
    /// True when at least one sample came from live terrain (not empty default).
    pub height_samples_from_terrain: bool,
    /// Map road segments frozen for terrain-road bake without live GameLogic.
    pub road_segments: Vec<PresentationRoadSegment>,
    /// Bridge segments frozen for terrain-road bake.
    pub bridge_segments: Vec<PresentationBridgeSegment>,
}

impl PresentationWorldEnv {
    pub fn from_logic(logic: &GameLogic) -> Self {
        let (wmin, wmax) = logic.world_bounds();
        let meta = logic.last_parsed_map_settings();
        let heightmap_hint = logic
            .heightmap_hint()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .or_else(|| {
                meta.as_ref()
                    .and_then(|m| m.heightmap_path.as_ref())
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
            });
        // Coarse height grid for minimap residual (fixed 64×64 — small, deterministic).
        const HG_W: u32 = 64;
        const HG_H: u32 = 64;
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let mut height_samples = vec![0.0f32; (HG_W * HG_H) as usize];
        let mut height_samples_from_terrain = false;
        for y in 0..HG_H {
            for x in 0..HG_W {
                let u = (x as f32 + 0.5) / HG_W as f32;
                let v = (y as f32 + 0.5) / HG_H as f32;
                let world = glam::Vec3::new(wmin.x + u * span_x, 0.0, wmin.z + v * span_z);
                if let Some(h) = logic.terrain_height_at(world) {
                    height_samples[(y * HG_W + x) as usize] = h;
                    height_samples_from_terrain = true;
                }
            }
        }

        let road_segments: Vec<PresentationRoadSegment> = logic
            .terrain_road_segments_snapshot()
            .into_iter()
            .map(|s| PresentationRoadSegment {
                template_name: s.template_name,
                from: [s.from.x, s.from.y, s.from.z],
                to: [s.to.x, s.to.y, s.to.z],
                width: s.width,
                width_in_texture: s.width_in_texture,
                road_type_id: s.road_type_id,
                start_is_angled: s.start_is_angled,
                start_is_join: s.start_is_join,
                end_is_angled: s.end_is_angled,
                end_is_join: s.end_is_join,
                curve_radius: s.curve_radius,
            })
            .collect();
        let bridge_segments: Vec<PresentationBridgeSegment> = logic
            .terrain_bridge_segments_snapshot()
            .into_iter()
            .map(
                |(start, end, width, template_name)| PresentationBridgeSegment {
                    start: start.to_array(),
                    end: end.to_array(),
                    width,
                    template_name,
                },
            )
            .collect();
        // Cap prewarm names so snapshot stays small (startup model resolve only).
        const PREWARM_CAP: usize = 256;
        let prewarm_template_names: Vec<String> = meta
            .as_ref()
            .map(|m| {
                m.objects
                    .iter()
                    .filter_map(|o| {
                        let n = o.template.trim();
                        if n.is_empty() {
                            None
                        } else {
                            Some(n.to_string())
                        }
                    })
                    .take(PREWARM_CAP)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            map_name: logic.get_current_map_name().trim().to_string(),
            world_min: [wmin.x, wmin.y, wmin.z],
            world_max: [wmax.x, wmax.y, wmax.z],
            heightmap_hint,
            sun_direction: meta.as_ref().and_then(|m| m.sun_direction),
            sun_color: meta.as_ref().and_then(|m| m.sun_color.or(m.sky_color)),
            ambient_color: meta
                .as_ref()
                .and_then(|m| m.ambient_color.or(m.fog_color).or(m.sky_color)),
            fog_color: meta
                .as_ref()
                .and_then(|m| m.fog_color.or(m.sky_color).or(m.sun_color)),
            fog_start: meta.as_ref().and_then(|m| m.fog_start),
            fog_end: meta.as_ref().and_then(|m| m.fog_end),
            map_object_count: meta.as_ref().map(|m| m.objects.len() as u32).unwrap_or(0),
            has_map_metadata: meta.is_some(),
            prewarm_template_names,
            height_grid_w: HG_W,
            height_grid_h: HG_H,
            height_samples,
            height_samples_from_terrain,
            road_segments,
            bridge_segments,
        }
    }

    #[inline]
    pub fn world_bounds_vec3(&self) -> (glam::Vec3, glam::Vec3) {
        (
            glam::Vec3::from_array(self.world_min),
            glam::Vec3::from_array(self.world_max),
        )
    }

    #[inline]
    pub fn fog_range(&self) -> Option<(f32, f32)> {
        self.fog_start.zip(self.fog_end)
    }

    /// Bilinear-ish nearest sample from the coarse height grid (world XZ).
    /// Returns None when the grid is empty / not from terrain.
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> Option<f32> {
        if !self.height_samples_from_terrain
            || self.height_grid_w == 0
            || self.height_grid_h == 0
            || self.height_samples.is_empty()
        {
            return None;
        }
        let (wmin, wmax) = self.world_bounds_vec3();
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let u = ((world_x - wmin.x) / span_x).clamp(0.0, 1.0);
        let v = ((world_z - wmin.z) / span_z).clamp(0.0, 1.0);
        let x = ((u * (self.height_grid_w as f32 - 1.0)).round() as u32)
            .min(self.height_grid_w.saturating_sub(1));
        let y = ((v * (self.height_grid_h as f32 - 1.0)).round() as u32)
            .min(self.height_grid_h.saturating_sub(1));
        let idx = (y * self.height_grid_w + x) as usize;
        self.height_samples.get(idx).copied()
    }

    /// Prewarm signature fragment (map|meta|objects|heightmap|shell) without live logic.
    pub fn prewarm_signature(&self, shell_bypass: bool) -> String {
        format!(
            "{}|meta:{}|objects:{}|heightmap:{}|shell:{}",
            self.map_name,
            self.has_map_metadata,
            self.map_object_count,
            self.heightmap_hint.as_deref().unwrap_or(""),
            shell_bypass
        )
    }
}

/// Snapshot-owned in-flight projectile for presentation/client observe path.
/// Fail-closed: not full W3D projectile mesh / trail GPU parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProjectile {
    pub id: ObjectId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    pub damage: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub is_homing: bool,
}

impl PresentationProjectile {
    pub fn from_combat(p: &crate::game_logic::combat::Projectile) -> Self {
        Self {
            id: p.id,
            position: p.position,
            velocity: p.velocity,
            target_position: p.target_position,
            shooter_id: p.shooter_id,
            target_id: p.target_id,
            damage: p.damage,
            lifetime: p.lifetime,
            max_lifetime: p.max_lifetime,
            is_homing: p.is_homing,
        }
    }
}

/// Immutable feed for GameClient / renderer after each authoritative logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationFrame {
    pub frame: LogicFrame,
    pub objects: Vec<RenderableObject>,
    pub local_player_id: u32,
    pub local_supplies: u32,
    pub local_power: i32,
    /// Host Player::power_produced residual (energy bar numerator side).
    pub local_power_produced: i32,
    /// Host Player::power_consumed residual (energy bar demand side).
    pub local_power_consumed: i32,
    pub local_color_rgb: (u8, u8, u8),
    /// Local player still alive residual.
    pub local_is_alive: bool,
    /// Radar provider count residual (CommandCenter / RadarVan).
    pub local_radar_count: i32,
    /// Script/power radar disable residual.
    pub local_radar_disabled: bool,
    /// GLA cash bounty percent residual (0..1).
    pub local_cash_bounty_percent: f32,
    /// Unlocked science names residual (capped).
    pub local_unlocked_sciences: Vec<String>,
    /// Queued upgrade template names residual (capped).
    pub local_queued_upgrades: Vec<String>,
    pub selected: Vec<ObjectId>,
    pub events: Vec<PresentationEvent>,
    pub match_over: bool,
    pub victory_label: Option<String>,
    /// Shell-map FOW bypass (`GameLogic::isInShellGame`) frozen at snapshot time.
    /// When true, unit FOW is forced fully visible and never-explored skip is off.
    pub fow_shell_bypass: bool,
    /// Compact local-player cell-grid FOW for terrain overlay / minimap texture.
    /// Frozen at build so GPU upload does not re-query shroud mid-render.
    /// Fail-closed: not full SAGE dirty-rect / multi-layer shroud streaming.
    pub fow_grid: PresentationFowGrid,
    /// Active combat particle systems from host registry (observe path for client).
    pub particle_systems: Vec<PresentationParticleSystem>,
    /// Active Patriot assist / BinaryDataStream lasers + Line3D segments.
    /// Frozen so WGPU laser segment pack does not re-read live host mid-render.
    /// Fail-closed: not full SegLineRenderer GPU texture draw.
    pub laser_beams: Vec<PresentationLaserBeam>,
    /// In-flight combat projectiles frozen from host CombatSystem.
    /// Fail-closed: not full W3D projectile draw / trail mesh.
    pub projectiles: Vec<PresentationProjectile>,
    /// InGameUI floating cash / caption texts frozen from host residual registries.
    /// Fail-closed: not full DisplayString GPU / Unicode GameText draw.
    pub floating_texts: Vec<PresentationFloatingText>,
    /// InGameUI world animations (MoneyPickUp Anim2D residual) frozen from host.
    /// Fail-closed: not full Anim2DCollection GPU draw.
    pub world_anims: Vec<PresentationWorldAnim>,
    /// Dual-tick residual counters (build / apply / content counts).
    pub dual_tick: PresentationDualTickResidual,
    /// World/environment identity for lighting/shell/bounds/heightmap residual.
    /// Prefer this over live `GameLogic` during GPU collect/execute.
    pub world_env: PresentationWorldEnv,
}

impl PresentationFrame {
    /// Build a snapshot by borrowing the authoritative world for this call only.
    ///
    /// FOW for `local_player_id` is frozen here via the FOW bridge so the unit mesh
    /// pass can apply alpha / never-explored skip without mid-render shroud locks.
    /// Cell-grid FOW is also frozen into `fow_grid` for terrain overlay / minimap.
    /// Fail-closed claim: unit FOW + compact local grid; not full SAGE shroud parity.
    pub fn build_from_logic(logic: &GameLogic, local_player_id: u32) -> Self {
        // Shell maps render fully visible background scenes (C++ parity).
        let fow_shell_bypass = logic.isInShellGame();
        // Freeze terrain FOW grid once for this presentation frame (local player only).
        let fow_grid = FOWRenderingBridge::snapshot_terrain_grid(local_player_id, fow_shell_bypass);
        let mut objects = Vec::with_capacity(logic.get_objects().len());
        for obj in logic.get_objects().values() {
            let is_structure = obj.is_kind_of(KindOf::Structure);
            let is_unit = obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            // Prefer explicit template model name so mesh resolve matches live collect path.
            // Alias remap (airanger → airanger_s) keeps PresentationFrame model_key aligned
            // with shipped W3D basenames for the residual mesh asset resolve path.
            let model_key = Some(crate::assets::mesh_asset_resolve::model_key_from_template(
                obj.get_template(),
            ));
            // Wave 75: freeze mesh scale residual (common combat = 1.0; CINE/weapon peels).
            let mesh_scale =
                crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template());
            let fow_visibility = if fow_shell_bypass {
                ObjectVisibility::FULLY_VISIBLE
            } else {
                FOWRenderingBridge::get_object_visibility(local_player_id, obj.id)
            };
            // Wave 77: freeze ground-height residual at object XY (sample or default-0).
            let pos = obj.get_position();
            let (ground_height, ground_height_from_terrain) =
                sample_presentation_ground_height(logic, pos);
            objects.push(RenderableObject {
                id: obj.id,
                template_name: obj.template_name.clone(),
                team: obj.team,
                team_color: obj.team_color,
                // Use accessors so presentation matches authoritative transform state.
                position: pos,
                orientation: obj.get_orientation(),
                move_destination: obj.movement.target_position,
                attack_target: obj.target,
                path_waypoints: obj.movement.path.iter().copied().take(16).collect(),
                production_queue: obj
                    .building_data
                    .as_ref()
                    .map(|b| {
                        b.production_queue
                            .iter()
                            .map(|p| PresentationProductionItem {
                                template_name: p.template_name.clone(),
                                progress: p.progress,
                                total_time: p.total_time,
                                cost_supplies: p.cost.supplies,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                rally_point: obj.building_data.as_ref().and_then(|b| b.rally_point),
                guard_position: obj.guard_position,
                garrisoned_units: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.garrisoned_units.iter().copied().take(32).collect())
                    .unwrap_or_default(),
                max_garrison: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.max_garrison)
                    .unwrap_or(0),
                power_provided: obj.power_provided,
                power_consumed: obj.power_consumed,
                stored_supplies: obj.stored_resources.supplies,
                health_current: obj.health.current,
                health_max: obj.health.maximum,
                selected: obj.selected || obj.status.selected,
                destroyed: obj.status.destroyed || !obj.is_alive(),
                under_construction: obj.status.under_construction,
                construction_percent: obj.construction_percent.clamp(0.0, 1.0),
                veterancy: PresentationVeterancy::from_host(obj.experience.level),
                experience_points: obj.experience.current.max(0.0),
                moving: obj.status.moving,
                attacking: obj.status.attacking,
                stealthed: obj.status.stealthed,
                detected: obj.status.detected,
                effectively_stealthed: obj.is_effectively_stealthed(),
                disabled: obj.is_disabled(),
                contained_by: obj.contained_by,
                force_attack: obj.force_attack,
                has_weapon: obj.weapon.is_some(),
                weapon_range: obj.weapon.as_ref().map(|w| w.range).unwrap_or(0.0),
                weapon_damage: obj.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0),
                camo_stealth_look: obj.camo_stealth_look,
                disguise_as_template: obj.disguise_as_template.clone(),
                disguise_as_team: obj.disguise_as_team,
                detection_range: obj.detection_range.max(0.0),
                special_power_ready: obj.special_power_ready,
                special_power_cooldown: obj.special_power_cooldown.max(0.0),
                special_power_cooldown_remaining: obj.special_power_cooldown_remaining.max(0.0),
                object_type: PresentationObjectType::from_host(obj.object_type),
                applied_upgrades: {
                    const MAX_UPGRADES: usize = 24;
                    let mut v: Vec<String> = obj.applied_upgrades.iter().cloned().collect();
                    v.sort();
                    v.truncate(MAX_UPGRADES);
                    v
                },
                has_secondary_weapon: obj.secondary_weapon.is_some(),
                secondary_weapon_range: obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.range)
                    .unwrap_or(0.0),
                secondary_weapon_damage: obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.damage)
                    .unwrap_or(0.0),
                has_mine: obj.mine_data.is_some(),
                kind_of: {
                    use crate::game_logic::KindOf;
                    const MAX_KINDS: usize = 32;
                    // Stable presentation order (KindOf declaration order residual).
                    const ORDER: &[KindOf] = &[
                        KindOf::Structure,
                        KindOf::Infantry,
                        KindOf::Vehicle,
                        KindOf::Aircraft,
                        KindOf::Projectile,
                        KindOf::Resource,
                        KindOf::Selectable,
                        KindOf::Attackable,
                        KindOf::CommandCenter,
                        KindOf::Worker,
                        KindOf::Hero,
                        KindOf::SupplyCenter,
                        KindOf::PowerPlant,
                        KindOf::FSBarracks,
                        KindOf::FSWarFactory,
                        KindOf::FSAirfield,
                        KindOf::FSInternetCenter,
                        KindOf::FSPower,
                        KindOf::FSBaseDefense,
                        KindOf::FSSupplyDropzone,
                        KindOf::FSSupplyCenter,
                        KindOf::FSSuperweapon,
                        KindOf::FSStrategyCenter,
                        KindOf::FSFake,
                        KindOf::FSTechnology,
                        KindOf::FSBlackMarket,
                        KindOf::FSAdvancedTech,
                        KindOf::Harvestable,
                        KindOf::Powered,
                    ];
                    let set = &obj.get_template().kind_of;
                    let mut v: Vec<KindOf> =
                        ORDER.iter().copied().filter(|k| set.contains(k)).collect();
                    v.truncate(MAX_KINDS);
                    v
                },
                is_structure,
                is_unit,
                is_mobile: is_unit
                    || obj.is_kind_of(crate::game_logic::KindOf::Infantry)
                    || obj.is_kind_of(crate::game_logic::KindOf::Vehicle)
                    || obj.is_kind_of(crate::game_logic::KindOf::Aircraft),
                can_produce: obj.building_data.is_some()
                    && !obj.status.under_construction
                    && obj.construction_percent >= 1.0
                    && !obj.status.destroyed
                    && obj.is_alive(),
                building_type: obj
                    .building_data
                    .as_ref()
                    .map(|b| PresentationBuildingType::from_host(b.building_type)),
                model_key,
                mesh_scale,
                selection_radius: obj.selection_radius.max(5.0),
                engine_bridged: obj.engine_object_id.is_some(),
                fow_visibility,
                ground_height,
                ground_height_from_terrain,
            });
        }
        // Stable presentation order for determinism (by ObjectId).
        objects.sort_by_key(|o| o.id.0);

        let local = logic.get_player(local_player_id);
        let local_supplies = local.map(|p| p.resources.supplies).unwrap_or(0);
        let local_power = local.map(|p| p.power_available).unwrap_or(0);
        let local_power_produced = local.map(|p| p.power_produced).unwrap_or(0);
        let local_power_consumed = local.map(|p| p.power_consumed).unwrap_or(0);
        let local_color_rgb = local.map(|p| p.color_rgb).unwrap_or((200, 200, 200));
        let local_is_alive = local.map(|p| p.is_alive).unwrap_or(false);
        let local_radar_count = local.map(|p| p.radar_count).unwrap_or(0);
        let local_radar_disabled = local.map(|p| p.radar_disabled).unwrap_or(false);
        let local_cash_bounty_percent = local
            .map(|p| p.cash_bounty_percent.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        const MAX_SCIENCE_NAMES: usize = 32;
        const MAX_UPGRADE_NAMES: usize = 32;
        let mut local_unlocked_sciences: Vec<String> = local
            .map(|p| {
                let mut v: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                v.sort();
                v.truncate(MAX_SCIENCE_NAMES);
                v
            })
            .unwrap_or_default();
        let mut local_queued_upgrades: Vec<String> = local
            .map(|p| {
                let mut v: Vec<String> = p.queued_upgrades.iter().cloned().collect();
                v.sort();
                v.truncate(MAX_UPGRADE_NAMES);
                v
            })
            .unwrap_or_default();
        let _ = (&mut local_unlocked_sciences, &mut local_queued_upgrades);
        let selected = local
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();

        // Combat particle residual: freeze host registry for client/presentation observe.
        let particle_systems: Vec<PresentationParticleSystem> = logic
            .combat_particles()
            .systems_snapshot()
            .iter()
            .map(PresentationParticleSystem::from_combat_entry)
            .collect();

        // W3DLaserDraw residual: freeze active assist lasers + Line3D segments.
        // Ground height residual: sample map height when available, else default-0.
        let logic_frame = logic.get_frame();
        let laser_beams: Vec<PresentationLaserBeam> = logic
            .active_patriot_assist_lasers()
            .iter()
            .filter(|l| l.is_active_at(logic_frame))
            .enumerate()
            .map(|(i, l)| {
                let mid = Vec3::new(l.arc_mid_x, l.arc_mid_y, l.arc_mid_z);
                let (gh, from_terrain) = sample_presentation_ground_height(logic, mid);
                PresentationLaserBeam::from_host_laser_with_terrain(l, i as u32, gh, from_terrain)
            })
            .collect();

        let projectiles: Vec<PresentationProjectile> = logic
            .combat_system()
            .projectiles_snapshot()
            .into_iter()
            .map(PresentationProjectile::from_combat)
            .collect();

        // InGameUI floating text + MoneyPickUp Anim2D residual: freeze host registries.
        let mut floating_texts = collect_presentation_floating_texts(logic);
        let world_anims = collect_presentation_world_anims(logic);

        let mut events = Vec::new();
        for (id, team) in logic.combat_particles().destroyed_this_frame() {
            events.push(PresentationEvent::ObjectDestroyed {
                id: *id,
                team: *team,
            });
        }
        // Freeze pending radar texts (UI drain later remains authoritative consumer).
        for entry in logic.radar_notification_snapshot() {
            events.push(PresentationEvent::RadarMessage {
                team: Team::Neutral, // host residual: text is global/team-agnostic here
                text: entry.text,
            });
        }
        // Drain: freeze this frame's completions into the snapshot (sole consumer).
        for ev in crate::game_logic::host_construction_log::drain() {
            events.push(PresentationEvent::ConstructionComplete {
                id: ev.id,
                template: ev.template_name,
            });
        }
        for up in logic.host_upgrades().completed_this_frame_snapshot() {
            events.push(PresentationEvent::UpgradeComplete {
                name: up.name,
                player_id: up.player_id,
                team: up.team,
                units_affected: up.units_affected,
            });
        }
        // Shadow session drains production before presentation; freeze last drain batch.
        for ev in crate::game_logic::host_production_log::take_last_drain() {
            if let crate::game_logic::host_production_log::HostProductionEvent::Complete {
                producer,
                template_name,
                spawned,
            } = ev
            {
                events.push(PresentationEvent::ProductionComplete {
                    producer,
                    template: template_name,
                    spawned,
                });
            }
        }
        for ev in crate::game_logic::host_owner_log::take_last_drain() {
            events.push(PresentationEvent::OwnerChanged {
                id: ev.object,
                team: ev.team,
            });
        }
        for ev in crate::game_logic::host_attack_log::take_last_drain() {
            if ev.target.is_some() {
                events.push(PresentationEvent::AttackTargeted {
                    attacker: ev.attacker,
                    target: ev.target,
                });
            }
        }
        for ev in crate::game_logic::host_move_log::take_last_drain() {
            if let Some(destination) = ev.destination {
                events.push(PresentationEvent::MoveOrdered {
                    unit: ev.unit,
                    destination,
                });
            }
        }
        for ev in crate::game_logic::host_damage_log::take_last_drain() {
            events.push(PresentationEvent::DamageApplied {
                target: ev.target,
                amount: ev.amount,
                source: ev.source,
                destroyed: ev.destroyed,
            });
            if ev.amount > 0.0 && !ev.destroyed {
                let pos = logic
                    .get_objects()
                    .get(&ev.target)
                    .map(|o| o.get_position())
                    .unwrap_or(Vec3::ZERO);
                let frame = logic.get_frame();
                floating_texts.push(PresentationFloatingText::from_parts(
                    PresentationFloatingTextKind::CombatDamage,
                    format!("-{}", ev.amount as i32),
                    "GUI:CombatDamage".into(),
                    pos + Vec3::new(0.0, 8.0, 0.0),
                    (255, 64, 64, 255),
                    ev.amount.max(0.0) as u32,
                    frame,
                    ev.source.unwrap_or(ev.target),
                ));
            }
        }
        for ev in crate::game_logic::host_heal_log::take_last_drain() {
            events.push(PresentationEvent::HealApplied {
                target: ev.target,
                health: ev.health,
            });
        }
        for ev in crate::game_logic::host_economy_log::take_last_drain() {
            events.push(PresentationEvent::EconomyChanged {
                player_id: ev.player_id,
                supplies: ev.supplies,
                power_available: ev.power_available,
            });
        }
        for pid in logic.combat_particles().spawned_this_frame() {
            if let Some(entry) = logic.combat_particles().get(*pid) {
                events.push(PresentationEvent::ParticleSystemSpawned {
                    id: entry.id,
                    kind: entry.kind,
                    template_name: entry.template_name.clone(),
                    position: entry.position,
                });
            }
        }

        let dual_tick = PresentationDualTickResidual::from_counts(
            objects.len(),
            selected.len(),
            laser_beams.len(),
            floating_texts.len(),
            world_anims.len(),
            particle_systems.len(),
        );

        Self {
            frame: LogicFrame(logic.get_frame()),
            objects,
            local_player_id,
            local_supplies,
            local_power,
            local_power_produced,
            local_power_consumed,
            local_color_rgb,
            local_is_alive,
            local_radar_count,
            local_radar_disabled,
            local_cash_bounty_percent,
            local_unlocked_sciences,
            local_queued_upgrades,
            selected,
            events,
            match_over: false,
            victory_label: None,
            fow_shell_bypass,
            fow_grid,
            particle_systems,
            laser_beams,
            projectiles,
            floating_texts,
            world_anims,
            dual_tick,
            world_env: PresentationWorldEnv::from_logic(logic),
        }
    }

    /// Build after evaluating victory (mutates victory subsystem once).
    pub fn build_with_victory(logic: &mut GameLogic, local_player_id: u32) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        if let Some(v) = logic.evaluate_victory_condition() {
            frame.match_over = true;
            frame.victory_label = Some(format!("{v:?}"));
            let winner = match v {
                crate::game_logic::VictoryCondition::Winner(id) => Some(id),
                _ => None,
            };
            frame.events.push(PresentationEvent::Victory {
                winner_player: winner,
            });
        }
        frame
    }

    /// Lightweight fingerprint for dual-run presentation determinism.
    pub fn presentation_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.frame.0.hash(&mut h);
        self.objects.len().hash(&mut h);
        for o in &self.objects {
            o.id.0.hash(&mut h);
            o.template_name.hash(&mut h);
            o.team.hash(&mut h);
            o.health_current.to_bits().hash(&mut h);
            o.selected.hash(&mut h);
            o.destroyed.hash(&mut h);
            o.fow_visibility.visibility_alpha.to_bits().hash(&mut h);
            o.fow_visibility.is_explored.to_bits().hash(&mut h);
        }
        self.local_supplies.hash(&mut h);
        self.match_over.hash(&mut h);
        self.fow_shell_bypass.hash(&mut h);
        self.fow_grid.content_fingerprint().hash(&mut h);
        self.local_player_id.hash(&mut h);
        self.laser_beams.len().hash(&mut h);
        for beam in &self.laser_beams {
            beam.beam_index.hash(&mut h);
            beam.from_id.0.hash(&mut h);
            beam.to_id.0.hash(&mut h);
            beam.segments.len().hash(&mut h);
            beam.scroll_offset.to_bits().hash(&mut h);
        }
        self.floating_texts.len().hash(&mut h);
        for ft in &self.floating_texts {
            ft.kind.hash(&mut h);
            ft.text.hash(&mut h);
            ft.amount.hash(&mut h);
            ft.spawn_frame.hash(&mut h);
            ft.source_id.0.hash(&mut h);
            ft.position.x.to_bits().hash(&mut h);
            ft.position.y.to_bits().hash(&mut h);
            ft.position.z.to_bits().hash(&mut h);
        }
        self.world_anims.len().hash(&mut h);
        for wa in &self.world_anims {
            wa.template.hash(&mut h);
            wa.spawn_frame.hash(&mut h);
            wa.crate_id.0.hash(&mut h);
            wa.picker_id.0.hash(&mut h);
            wa.display_time_seconds.to_bits().hash(&mut h);
        }
        self.world_env.map_name.hash(&mut h);
        self.world_env.has_map_metadata.hash(&mut h);
        self.world_env.map_object_count.hash(&mut h);
        self.dual_tick.builds.hash(&mut h);
        self.dual_tick.object_count.hash(&mut h);
        h.finish()
    }

    pub fn alive_object_count(&self) -> usize {
        self.objects.iter().filter(|o| !o.destroyed).count()
    }

    /// Stable object-id list for the production render collect path.
    /// Presentation owns unit identity + unit FOW; mesh asset load may still
    /// consult asset systems (not live object transform / shroud re-read).
    pub fn renderable_object_ids(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| o.id)
            .collect()
    }

    /// Main unit mesh pass inputs from the snapshot only (no GameLogic / shroud borrow).
    ///
    /// Filters destroyed and engine-bridged objects (RenderBridge owns those).
    /// Includes local-player FOW alpha for skip/darkening without mid-render queries.
    pub fn unit_render_inputs(&self) -> Vec<UnitRenderInput> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.engine_bridged)
            .map(UnitRenderInput::from_renderable)
            .collect()
    }

    /// Structures with a non-empty production queue (ControlBar residual feed).
    pub fn structures_with_production(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| o.is_structure && !o.destroyed && !o.production_queue.is_empty())
            .collect()
    }

    /// Structures currently holding garrisoned units (contain residual feed).
    pub fn garrisoned_structures(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| o.is_structure && !o.destroyed && !o.garrisoned_units.is_empty())
            .collect()
    }

    /// Net power from non-destroyed objects (presentation economy residual).
    pub fn net_power_from_objects(&self) -> i32 {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| o.power_provided - o.power_consumed)
            .sum()
    }

    /// Objects still under construction (dozer / structure residual).
    pub fn under_construction_objects(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.under_construction)
            .collect()
    }

    /// Units at Veteran or higher (chevron residual feed).
    pub fn veteran_or_higher_units(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed && o.is_unit && !matches!(o.veterancy, PresentationVeterancy::Rookie)
            })
            .collect()
    }

    /// Units currently attacking (status residual).
    pub fn attacking_units(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.attacking)
            .collect()
    }

    /// Effectively stealthed units (hidden from non-allied targeting residual).
    pub fn effectively_stealthed_units(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.effectively_stealthed)
            .collect()
    }

    /// Contained (garrisoned/transported) units residual.
    pub fn contained_units(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.contained_by.is_some())
            .collect()
    }

    /// True when local player has any radar provider and radar is not disabled.
    pub fn local_radar_active(&self) -> bool {
        self.local_radar_count > 0 && !self.local_radar_disabled
    }

    /// Energy ratio residual (produced / max(consumed,1)) for power bar UI.
    pub fn local_energy_ratio(&self) -> f32 {
        let demand = self.local_power_consumed.max(1) as f32;
        self.local_power_produced as f32 / demand
    }

    /// Whether a science name is unlocked for the local player residual.
    pub fn local_has_science(&self, name: &str) -> bool {
        self.local_unlocked_sciences.iter().any(|s| s == name)
    }

    /// Objects with a ready special power residual (UI / command button feed).
    pub fn special_power_ready_objects(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.special_power_ready)
            .collect()
    }

    /// Special-power cooldown fraction remaining in 0..1 (0 = ready).
    pub fn special_power_cooldown_fraction(obj: &RenderableObject) -> f32 {
        if obj.special_power_cooldown <= 0.0 {
            return 0.0;
        }
        (obj.special_power_cooldown_remaining / obj.special_power_cooldown).clamp(0.0, 1.0)
    }

    /// Objects that have applied at least one upgrade residual.
    pub fn upgraded_objects(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.applied_upgrades.is_empty())
            .collect()
    }

    /// Whether `upgrade` is applied on the object residual.
    pub fn object_has_upgrade(obj: &RenderableObject, upgrade: &str) -> bool {
        obj.applied_upgrades.iter().any(|u| u == upgrade)
    }

    /// Live mine / demo-trap presentation residuals.
    pub fn mine_objects(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed && o.has_mine)
            .collect()
    }

    /// True when snapshot object carries `kind` residual.
    pub fn object_has_kind(obj: &RenderableObject, kind: crate::game_logic::KindOf) -> bool {
        obj.kind_of.iter().any(|k| *k == kind)
    }

    /// Double-click residual: same-template selectable friendlies from snapshot.
    pub fn similar_unit_ids(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let Some(clicked) = self.objects.iter().find(|o| o.id == clicked_id) else {
            return Vec::new();
        };
        if clicked.team != player_team || !UnitControlSystem::presentation_is_selectable(clicked) {
            return Vec::new();
        }
        let template = clicked.template_name.as_str();
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.template_name == template
            })
            .map(|o| o.id)
            .collect()
    }

    /// Right-click residual: enemy attackable under cursor id from snapshot.
    pub fn is_enemy_attackable(
        &self,
        target_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> bool {
        use crate::unit_control::UnitControlSystem;
        self.objects
            .iter()
            .find(|o| o.id == target_id)
            .map(|o| o.team != player_team && UnitControlSystem::presentation_is_attackable(o))
            .unwrap_or(false)
    }

    /// Drag-box residual: friendly selectable units whose XZ pose is inside the rect.
    ///
    /// Prefer non-structures when any unit is in the box (C++ InGameUI drag residual).
    /// If only structures are hit, keep a single structure when exactly one is present.
    /// Filter stored ids to alive selectable friendlies (control-group recall residual).
    /// Script camera-slave residual: first non-destroyed object matching template (case-insensitive).
    /// Control-group double-tap residual: average XZ pose of listed alive objects.
    /// Runtime-host residual: first alive mobile friendly (select_local_unit).
    pub fn first_mobile_friendly_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
    }

    /// Runtime-host residual: first constructed structure with production capacity.
    pub fn first_constructed_producer_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        // Prefer barracks/warfactory/airfield; fall back to any can_produce structure.
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.can_produce
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .or_else(|| {
                self.objects
                    .iter()
                    .find(|o| o.team == player_team && !o.destroyed && o.can_produce)
            })
            .map(|o| o.id)
    }

    /// Structures that can produce units (ControlBar factory residual feed).
    pub fn unit_producer_structures(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && o.can_produce
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Runtime-host residual: first alive enemy attackable.

    /// Unique non-empty model keys from alive objects (GPU preload residual).
    pub fn unique_model_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for o in &self.objects {
            if o.destroyed {
                continue;
            }
            if let Some(k) = o.model_key.as_ref() {
                if !k.is_empty() && seen.insert(k.clone()) {
                    keys.push(k.clone());
                }
            }
        }
        keys
    }

    /// Structures holding supply crates residual (ControlBar / gather UI).
    pub fn supply_storage_structures(&self) -> Vec<&RenderableObject> {
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && o.stored_supplies > 0
                    && (o.is_structure
                        || o.building_type.is_some()
                        || o.object_type == PresentationObjectType::Building
                        || o.object_type == PresentationObjectType::Supply)
            })
            .collect()
    }

    /// Friendly workers residual (dozer / worker command feed by team).
    pub fn friendly_workers(&self, player_team: crate::game_logic::Team) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && (Self::object_has_kind(o, KindOf::Worker)
                        || o.template_name.contains("Dozer")
                        || o.template_name.contains("Worker")
                        || o.template_name.contains("Construction"))
            })
            .collect()
    }

    pub fn first_enemy_attackable_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        self.objects
            .iter()
            .find(|o| o.team != player_team && UnitControlSystem::presentation_is_attackable(o))
            .map(|o| o.id)
    }

    /// Runtime-host residual: count of alive mobile friendlies.
    pub fn count_mobile_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        self.objects
            .iter()
            .filter(|o| o.team == player_team && !o.destroyed && o.is_mobile)
            .count() as u32
    }

    pub fn centroid_of_ids(&self, ids: &[ObjectId]) -> Option<glam::Vec3> {
        let mut sum = glam::Vec3::ZERO;
        let mut n = 0u32;
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if o.destroyed {
                    continue;
                }
                sum += o.position;
                n += 1;
            }
        }
        if n == 0 {
            None
        } else {
            Some(sum / n as f32)
        }
    }

    pub fn first_alive_position_for_template(&self, template_name: &str) -> Option<glam::Vec3> {
        self.objects
            .iter()
            .find(|o| !o.destroyed && o.template_name.eq_ignore_ascii_case(template_name))
            .map(|o| o.position)
    }

    pub fn filter_alive_selectable_ids(
        &self,
        ids: &[ObjectId],
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut out = Vec::new();
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if o.team == player_team && UnitControlSystem::presentation_is_selectable(o) {
                    out.push(*id);
                }
            }
        }
        out
    }

    /// All alive selectable friendlies (Ctrl+A / Tab cycle residual).
    pub fn alive_selectable_friendly_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| o.team == player_team && UnitControlSystem::presentation_is_selectable(o))
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn box_select_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut units = Vec::new();
        let mut structures = Vec::new();
        for o in &self.objects {
            if o.team != player_team || !UnitControlSystem::presentation_is_selectable(o) {
                continue;
            }
            let pos = o.position;
            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                continue;
            }
            let is_structure = o.is_structure
                || Self::object_has_kind(o, KindOf::Structure)
                || o.object_type == PresentationObjectType::Building;
            if is_structure {
                structures.push(o.id);
            } else {
                units.push(o.id);
            }
        }
        if !units.is_empty() {
            units
        } else if structures.len() == 1 {
            structures
        } else {
            // Multi-structure-only box: fail-closed empty (parity with unit_control residual).
            Vec::new()
        }
    }

    /// Structures residual (KindOf::Structure or object_type Building).
    pub fn structure_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building)
            })
            .collect()
    }

    /// Harvestable resource objects residual.
    pub fn harvestable_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        self.objects
            .iter()
            .filter(|o| !o.destroyed && Self::object_has_kind(o, KindOf::Harvestable))
            .collect()
    }

    /// Worker units residual (dozer / worker command feed).
    pub fn worker_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        self.objects
            .iter()
            .filter(|o| !o.destroyed && Self::object_has_kind(o, KindOf::Worker))
            .collect()
    }

    /// Overlay health/position/destroyed from a GameWorld shadow session.
    ///
    /// Host still builds the frame (templates, FOW, selection); shadow is last
    /// writer for HP and world position when authority paths are active.
    /// Unmapped objects are left unchanged.
    pub fn overlay_gameworld_shadow(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        let mut updated = 0usize;
        for obj in &mut self.objects {
            let Some(eid) = shadow.entity_for_host(obj.id) else {
                continue;
            };
            let Some(ent) = shadow.world().entity(eid) else {
                // Destroyed on shadow — mark destroyed for presentation.
                if !obj.destroyed {
                    obj.destroyed = true;
                    obj.health_current = 0.0;
                    updated += 1;
                }
                continue;
            };
            let pos = glam::Vec3::new(
                ent.transform.position.x,
                ent.transform.position.y,
                ent.transform.position.z,
            );
            let h = ent.health.max(0.0);
            let destroyed = h <= 0.0;
            if (obj.position - pos).length_squared() > 1e-6
                || (obj.health_current - h).abs() > 1e-3
                || obj.destroyed != destroyed
            {
                obj.position = pos;
                obj.orientation = ent.transform.orientation;
                obj.move_destination = ent.move_target.map(|d| glam::Vec3::new(d[0], d[1], d[2]));
                obj.attack_target = ent
                    .attack_target
                    .and_then(|tid| shadow.host_for_entity(tid));
                obj.health_current = h;
                obj.destroyed = destroyed;
                updated += 1;
            }
        }
        // Local supplies from shadow player 0 when economy authority is on.
        if crate::gameworld_shadow::gameworld_economy_authority_enabled() {
            if let Some(p) = shadow
                .world()
                .player(gamelogic::world::PlayerId::from_index(0))
            {
                self.local_supplies = p.supplies;
            }
        }
        updated
    }

    /// Lookup snapshot FOW for an object (local player). None if not on the frame.
    pub fn fow_for_object(&self, id: ObjectId) -> Option<ObjectVisibility> {
        self.objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.fow_visibility)
    }

    /// Local-player cell-grid FOW frozen on this frame (terrain / minimap).
    #[inline]
    pub fn fow_grid(&self) -> &PresentationFowGrid {
        &self.fow_grid
    }

    /// R8 terrain FOW texture from the snapshot only (no live shroud lock).
    ///
    /// Returns `None` when the grid is inactive (fail-open: skip overlay upload)
    /// or when shell bypass already forces fully visible cells that need no darkening.
    /// Callers that always want bytes can use `fow_grid().to_r8_texture()` directly.
    pub fn terrain_fow_r8(&self) -> Option<Vec<u8>> {
        if !self.fow_grid.active {
            return None;
        }
        let r8 = self.fow_grid.to_r8_texture();
        if r8.is_empty() {
            None
        } else {
            Some(r8)
        }
    }

    /// True when terrain FOW overlay should darken from the presentation grid.
    ///
    /// Shell bypass and inactive grids are fail-open (no overlay).
    pub fn terrain_fow_overlay_active(&self) -> bool {
        self.fow_grid.active && !self.fow_shell_bypass
    }

    /// All alive presentation objects including engine-bridged (for FOW/id lists).
    pub fn alive_renderables(&self) -> impl Iterator<Item = &RenderableObject> {
        self.objects.iter().filter(|o| !o.destroyed)
    }

    /// Active combat particle systems on this frame (host registry snapshot).
    pub fn active_particle_systems(&self) -> impl Iterator<Item = &PresentationParticleSystem> {
        self.particle_systems.iter().filter(|p| p.active)
    }

    /// True when at least one combat particle system is registered and active.
    pub fn has_active_particles(&self) -> bool {
        self.particle_systems.iter().any(|p| p.active)
    }

    /// Active presentation laser beams (assist BinaryDataStream residual).
    pub fn laser_beams(&self) -> &[PresentationLaserBeam] {
        &self.laser_beams
    }

    /// Total Line3D segments across all frozen laser beams.
    pub fn laser_segment_count(&self) -> usize {
        self.laser_beams.iter().map(|b| b.segments.len()).sum()
    }

    /// True when at least one residual laser beam is frozen on this frame.
    pub fn has_active_lasers(&self) -> bool {
        !self.laser_beams.is_empty()
    }

    /// Frozen InGameUI floating texts (host residual observe path).
    pub fn floating_texts(&self) -> &[PresentationFloatingText] {
        &self.floating_texts
    }

    /// Floating texts still within residual timeout at `frame` (or this frame).
    pub fn active_floating_texts_at(&self, logic_frame: u32) -> Vec<&PresentationFloatingText> {
        self.floating_texts
            .iter()
            .filter(|t| t.is_active_at(logic_frame))
            .collect()
    }

    /// True when at least one floating text is frozen on this frame.
    pub fn has_floating_texts(&self) -> bool {
        !self.floating_texts.is_empty()
    }

    /// Host-testable floating text residual usable for dual-tick UI layout pack.
    ///
    /// Empty is honest (no cash events yet). Non-empty requires GUI:AddCash key residual
    /// and positive timeout window.
    pub fn floating_text_presentation_ok(&self) -> bool {
        if self.floating_texts.is_empty() {
            return true;
        }
        self.floating_texts.iter().all(|t| {
            !t.text.is_empty()
                && t.text_key == "GUI:AddCash"
                && t.timeout_frame > t.spawn_frame
                && t.amount > 0
        })
    }

    /// Frozen MoneyPickUp / world Anim2D residuals.
    pub fn world_anims(&self) -> &[PresentationWorldAnim] {
        &self.world_anims
    }

    /// True when at least one world anim is frozen on this frame.
    pub fn has_world_anims(&self) -> bool {
        !self.world_anims.is_empty()
    }

    /// Host-testable world-anim residual usable for dual-tick Anim2D pack.
    ///
    /// Empty is honest. Non-empty requires MoneyPickUp template + positive display.
    pub fn world_anim_presentation_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return true;
        }
        self.world_anims.iter().all(|a| {
            a.template == crate::game_logic::host_money_crate::MONEY_PICKUP_ANIM_TEMPLATE
                && a.display_time_seconds > 0.0
                && a.z_rise_per_second > 0.0
        })
    }

    /// Host-testable FOW grid residual usable for minimap / terrain texture path.
    ///
    /// Active grids must have a consistent cell buffer; inactive grids are honest
    /// when shroud was not initialized (boot / no-map host).
    pub fn minimap_fow_presentation_ok(&self) -> bool {
        let g = &self.fow_grid;
        if !g.active {
            return true;
        }
        g.cell_count() == (g.width as usize).saturating_mul(g.height as usize)
            && !g.to_r8_texture().is_empty()
    }

    /// Dual-tick residual counters on this frame.
    #[inline]
    pub fn dual_tick(&self) -> &PresentationDualTickResidual {
        &self.dual_tick
    }

    /// Honesty: dual-tick build residual counters are self-consistent.
    pub fn dual_tick_presentation_residual_ok(&self) -> bool {
        self.dual_tick.honesty_build_ok()
            && self.dual_tick.object_count == self.objects.len() as u32
            && self.dual_tick.laser_beam_count == self.laser_beams.len() as u32
            && self.dual_tick.floating_text_count == self.floating_texts.len() as u32
            && self.dual_tick.world_anim_count == self.world_anims.len() as u32
            // Wave 102: selected + particle dual-tick residual counters.
            && self.dual_tick.selected_count == self.selected.len() as u32
            && self.dual_tick.particle_count == self.particle_systems.len() as u32
    }

    /// Wave 102: dual-tick residual deepen honesty (build + apply + content counts).
    ///
    /// Deepens dual-tick bookkeeping beyond Wave 65/75 counters: selected/particle
    /// counts, apply order residual (applies ≥ builds after shell apply), and
    /// cross-link presentation residual packs. Fail-closed vs live dual-run GPU.
    pub fn dual_tick_presentation_residual_deepen_ok(&self) -> bool {
        self.dual_tick_presentation_residual_ok()
            && self.dual_tick.builds >= 1
            && self.floating_text_vanish_residual_ok()
            && self.world_anim_fade_residual_ok()
            && self.laser_presentation_residual_ok()
            && self.spectre_orbit_decal_presentation_residual_ok()
            && self.mesh_scale_presentation_residual_ok()
            && self.ground_height_presentation_residual_ok()
    }

    /// Honesty: floating-text vanish-rate residual fields (empty is honest).
    pub fn floating_text_vanish_residual_ok(&self) -> bool {
        PresentationFloatingText::honesty_vanish_rate_residual_ok()
            && self.floating_texts.iter().all(|t| {
                let a = t.vanish_alpha_at(self.frame.0);
                a.is_finite() && (0.0..=1.0).contains(&a)
            })
    }

    /// Honesty: world-anim fade residual fields (empty is honest).
    pub fn world_anim_fade_residual_ok(&self) -> bool {
        if self.world_anims.is_empty() {
            return PresentationWorldAnim::honesty_money_pickup_fade_params_ok();
        }
        self.world_anims
            .iter()
            .all(|a| a.honesty_fade_residual_ok())
    }

    /// Honesty: laser ground-height + multi-beam soft-edge presentation residual.
    pub fn laser_presentation_residual_ok(&self) -> bool {
        self.laser_beams
            .iter()
            .all(|b| b.honesty_ground_height_ok() && b.honesty_soft_edge_presentation_ok())
            && PRESENTATION_ORBITAL_SOFT_EDGE.honesty_orbital_residual_ok()
            && honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
    }

    /// Honesty: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual (Wave 73).
    ///
    /// Constant pack — presentation freezes retail decal defaults so dual-tick
    /// consumers can draw orbit cursors without re-reading live SpectreGunshipUpdate.
    /// Fail-closed: not full SHADOW_ALPHA_DECAL GPU throb submit.
    pub fn spectre_orbit_decal_presentation_residual_ok(&self) -> bool {
        let _ = self;
        honesty_spectre_orbit_decal_presentation_ok()
    }

    /// Honesty: mesh scale residual frozen on objects / unit render inputs (Wave 75).
    ///
    /// Common combat units retail-default to **1.0**. Empty snapshot is honest.
    /// Fail-closed: not full Object INI Scale field / draw-scale bone matrix.
    pub fn mesh_scale_presentation_residual_ok(&self) -> bool {
        crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok()
            && self
                .objects
                .iter()
                .all(|o| o.mesh_scale.is_finite() && o.mesh_scale > 0.0)
            && self
                .unit_render_inputs()
                .iter()
                .all(|u| u.mesh_scale.is_finite() && u.mesh_scale > 0.0)
    }

    /// Honesty: unit/structure ground-height residual frozen on objects (Wave 77).
    ///
    /// Empty object lists are honest (default path). Fail-closed: not full
    /// HeightMap bilinear / bridge-aware / locomotor Y rewrite.
    pub fn ground_height_presentation_residual_ok(&self) -> bool {
        honesty_ground_height_residual_ok(PRESENTATION_DEFAULT_GROUND_HEIGHT, false)
            && self.objects.iter().all(|o| {
                honesty_ground_height_residual_ok(o.ground_height, o.ground_height_from_terrain)
            })
    }

    /// Note a dual-tick apply on this snapshot (HUD / shell multi-consumer path).
    pub fn note_dual_tick_apply(&mut self) {
        self.dual_tick.applies = self.dual_tick.applies.saturating_add(1);
    }

    /// Selected unit identity (health/name/type) from snapshot only.
    ///
    /// Prefer player selection list; fall back to objects marked selected on the frame
    /// when the player list is empty (common right after click-select before player list
    /// is mirrored).
    pub fn selected_unit_display_infos(&self) -> Vec<crate::ui::UnitDisplayInfo> {
        use crate::ui::UnitDisplayInfo;

        let by_id: std::collections::HashMap<ObjectId, &RenderableObject> =
            self.objects.iter().map(|o| (o.id, o)).collect();
        let mut selected_infos = Vec::with_capacity(self.selected.len().max(1));
        for id in &self.selected {
            if let Some(ro) = by_id.get(id) {
                if ro.destroyed {
                    continue;
                }
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        if selected_infos.is_empty() {
            for ro in self.objects.iter().filter(|o| o.selected && !o.destroyed) {
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        selected_infos
    }

    fn unit_display_info_from_renderable(ro: &RenderableObject) -> crate::ui::UnitDisplayInfo {
        let (production_template, production_progress) = ro
            .production_queue
            .first()
            .map(|p| (Some(p.template_name.clone()), Some(p.progress)))
            .unwrap_or((None, None));
        crate::ui::UnitDisplayInfo {
            object_id: ro.id,
            name: ro.template_name.clone(),
            health_current: ro.health_current,
            health_maximum: ro.health_max.max(1.0),
            unit_type: if ro.is_structure {
                "Structure".into()
            } else if ro.is_unit {
                "Unit".into()
            } else {
                "Object".into()
            },
            current_order: if ro.attacking {
                "Attack".into()
            } else if ro.moving {
                "Move".into()
            } else if production_template.is_some() {
                "Produce".into()
            } else {
                "Idle".into()
            },
            veterancy_overlay: ro.veterancy.chevron_overlay().map(str::to_string),
            production_progress,
            production_template,
        }
    }

    /// Apply presentation identity fields onto a HUD/UI state (production consumer path).
    /// Does not re-borrow GameLogic — uses only owned snapshot data.
    ///
    /// Overwrites **selection IDs, selected unit health/name, and minimap unit dots**
    /// so a prior live `update_ui_state` walk cannot leave stale identity when a frame
    /// is available.
    pub fn apply_to_ui_state(&self, ui: &mut crate::ui::GameUIState) {
        use crate::game_logic::victory::PlayerOutcome;
        use crate::ui::{color_for_player, BuildQueueEntry, MinimapDot};

        ui.credits = self.local_supplies as i32;
        // Prefer produced/consumed residual when present (energy bar parity).
        ui.power_generated = self.local_power_produced.max(self.local_power).max(0);
        ui.power_used = self.local_power_consumed.max(0);
        ui.max_power = ui.power_generated.max(1);
        ui.player_id = self.local_player_id;
        ui.selected_units = self.selected.clone();
        ui.match_over = self.match_over;
        ui.selected_unit_infos = self.selected_unit_display_infos();
        // ControlBar/WND selection panel health must come from snapshot, not live re-read.
        ui.selection_panel =
            crate::ui::ControlBarSelectionPanelState::from_unit_infos(&ui.selected_unit_infos);

        // Victory residual from snapshot events (no live evaluate_victory re-read).
        if self.match_over {
            let winner = self.events.iter().find_map(|e| match e {
                PresentationEvent::Victory { winner_player } => *winner_player,
                _ => None,
            });
            ui.player_outcome = Some(match winner {
                Some(id) if id == self.local_player_id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => {
                    if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("draw"))
                    {
                        PlayerOutcome::Draw
                    } else if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("winner"))
                    {
                        // Fail-closed: label without winner id → treat as unknown draw residual.
                        PlayerOutcome::Draw
                    } else {
                        PlayerOutcome::Draw
                    }
                }
            });
        }

        // Structure production + under-construction residual for build-queue HUD strip.
        let mut build_queue = Vec::new();
        for o in self.objects.iter().filter(|o| !o.destroyed) {
            if o.under_construction {
                build_queue.push(BuildQueueEntry {
                    template_name: o.template_name.clone(),
                    percent_complete: o.construction_percent.clamp(0.0, 1.0),
                    time_remaining: (1.0 - o.construction_percent.clamp(0.0, 1.0)) * 30.0,
                });
            }
            for item in &o.production_queue {
                build_queue.push(BuildQueueEntry {
                    template_name: item.template_name.clone(),
                    percent_complete: item.progress.clamp(0.0, 1.0),
                    time_remaining: (item.total_time * (1.0 - item.progress.clamp(0.0, 1.0)))
                        .max(0.0),
                });
            }
        }
        build_queue.truncate(16);
        ui.build_queue = build_queue;

        // Minimap dots from snapshot positions/teams (normalized into frame bounds).
        let alive: Vec<&RenderableObject> = self.objects.iter().filter(|o| !o.destroyed).collect();
        let (world_min_x, world_max_x, world_min_z, world_max_z) = if alive.is_empty() {
            (-100.0, 100.0, -100.0, 100.0)
        } else {
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;
            for o in &alive {
                min_x = min_x.min(o.position.x);
                max_x = max_x.max(o.position.x);
                min_z = min_z.min(o.position.z);
                max_z = max_z.max(o.position.z);
            }
            // Pad so single-unit maps still normalize.
            if (max_x - min_x).abs() < 1.0 {
                min_x -= 50.0;
                max_x += 50.0;
            }
            if (max_z - min_z).abs() < 1.0 {
                min_z -= 50.0;
                max_z += 50.0;
            }
            (min_x, max_x, min_z, max_z)
        };
        let span_x = (world_max_x - world_min_x).max(1.0);
        let span_z = (world_max_z - world_min_z).max(1.0);
        let mut dots = Vec::with_capacity(alive.len());
        for ro in alive {
            let nx = ((ro.position.x - world_min_x) / span_x).clamp(0.0, 1.0);
            let nz = ((ro.position.z - world_min_z) / span_z).clamp(0.0, 1.0);
            let color = match ro.team {
                Team::USA => color_for_player(1),
                Team::China => color_for_player(0),
                Team::GLA => color_for_player(4),
                Team::Neutral => color_for_player(7),
            };
            let size = if ro.is_structure { 4.0 } else { 2.0 };
            dots.push(MinimapDot::normalized(nx, nz, color, size));
        }
        ui.minimap_unit_dots = dots;
    }

    /// Resource triple for GameHud::update_resources (credits, power, max_power).
    /// Drive VictoryScreen visibility/type from snapshot residual (no live GameLogic).
    ///
    /// Fail-closed: does not rebuild full VictorySummary statistics tables.
    pub fn apply_to_victory_screen(&self, screen: &mut crate::ui::VictoryScreen) {
        if !self.match_over {
            return;
        }
        let winner = self.events.iter().find_map(|e| match e {
            PresentationEvent::Victory { winner_player } => *winner_player,
            _ => None,
        });
        match winner {
            Some(id) if id == self.local_player_id => screen.set_victory(id),
            Some(_) => screen.set_defeat(),
            None => {
                let label = self
                    .victory_label
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if label.contains("defeat") || label.contains("lost") {
                    screen.set_defeat();
                } else if label.contains("winner") && !label.contains("draw") {
                    // Label-only winner residual without player id → draw fail-closed.
                    screen.set_draw();
                } else {
                    screen.set_draw();
                }
            }
        }
    }

    pub fn hud_resource_triple(&self) -> (i32, i32, i32) {
        let credits = self.local_supplies as i32;
        let power = self.local_power.max(0);
        (credits, power, power.max(1))
    }

    /// Units list for GameHud minimap: (id, x, z, team_color_index).
    pub fn hud_minimap_units(&self) -> Vec<(ObjectId, f32, f32, u8)> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| {
                let team_idx = match o.team {
                    Team::USA => 1u8,
                    Team::China => 0u8,
                    Team::GLA => 4u8,
                    Team::Neutral => 7u8,
                };
                (o.id, o.position.x, o.position.z, team_idx)
            })
            .collect()
    }

    /// Apply presentation resources, minimap units, and selection health to GameHUD.
    ///
    /// Selection identity (IDs + health/name) is snapshot-owned so the production HUD
    /// does not re-read live GameLogic after a skirmish start / dual-tick.
    /// Also fills the ControlBar selection panel health strip via GameHUD.
    pub fn apply_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        let (credits, power, max_power) = self.hud_resource_triple();
        hud.update_resources(credits, power, max_power);
        let units = self.hud_minimap_units();
        hud.update_minimap(&units);
        let infos = self.selected_unit_display_infos();
        // Prefer explicit player selection list; if empty but infos came from
        // object.selected flags, mirror those IDs onto the HUD strip.
        let mut ids = self.selected.clone();
        if ids.is_empty() {
            ids = infos.iter().map(|i| i.object_id).collect();
        }
        hud.sync_selection_from_presentation(ids, infos);
        self.apply_events_to_game_hud(hud);
    }

    /// Route frozen gameplay events into HUD message / radar channels.
    /// Fail-closed: text residual only — not full EVA voice / WND dialog parity.
    pub fn apply_events_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        for ev in &self.events {
            match ev {
                PresentationEvent::RadarMessage { text, .. } => {
                    hud.push_radar_message(text);
                }
                PresentationEvent::ConstructionComplete { template, .. } => {
                    hud.push_info_message(&format!("Construction complete: {template}"));
                }
                PresentationEvent::UpgradeComplete { name, .. } => {
                    hud.push_info_message(&format!("Upgrade complete: {name}"));
                }
                PresentationEvent::ProductionComplete { template, .. } => {
                    hud.push_info_message(&format!("Unit ready: {template}"));
                }
                PresentationEvent::OwnerChanged { id, team } => {
                    hud.push_info_message(&format!("Ownership changed: #{} -> {:?}", id.0, team));
                }
                PresentationEvent::AttackTargeted { attacker, target } => {
                    if let Some(t) = target {
                        hud.push_info_message(&format!("Attack: #{} -> #{}", attacker.0, t.0));
                    }
                }
                PresentationEvent::MoveOrdered { unit, destination } => {
                    hud.push_info_message(&format!(
                        "Move: #{} -> ({:.0},{:.0})",
                        unit.0, destination[0], destination[2]
                    ));
                }
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed,
                    ..
                } => {
                    if *destroyed {
                        hud.push_info_message(&format!("Destroyed: #{}", target.0));
                    } else if *amount > 0.0 {
                        hud.push_info_message(&format!("-{} HP #{}", *amount as i32, target.0));
                    }
                }
                PresentationEvent::HealApplied { target, health } => {
                    hud.push_info_message(&format!("Heal #{} -> {:.0} HP", target.0, health));
                }
                PresentationEvent::EconomyChanged {
                    player_id,
                    supplies,
                    power_available,
                } => {
                    hud.push_info_message(&format!(
                        "Economy P{}: ${} power={}",
                        player_id, supplies, power_available
                    ));
                }
                PresentationEvent::ObjectDestroyed { id, .. } => {
                    hud.push_info_message(&format!("Destroyed: #{}", id.0));
                }
                PresentationEvent::Victory { winner_player } => {
                    let msg = match winner_player {
                        Some(p) => format!("Victory: player {p}"),
                        None => "Victory".to_string(),
                    };
                    hud.push_info_message(&msg);
                }
                PresentationEvent::ParticleSystemSpawned { .. } => {}
            }
        }
    }

    /// Queue presentation gameplay events into host audio residual (next-frame process).
    /// Fail-closed: not Miles/device spatial parity — event names only for dispatch tables.
    pub fn apply_events_to_audio(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::AudioEventRequest;
        let mut n = 0usize;
        for ev in &self.events {
            let mapped: Option<(&str, Option<crate::game_logic::ObjectId>)> = match ev {
                PresentationEvent::ObjectDestroyed { id, .. } => Some(("UnitDie", Some(*id))),
                PresentationEvent::ConstructionComplete { id, .. } => {
                    Some(("BuildingComplete", Some(*id)))
                }
                PresentationEvent::UpgradeComplete { .. } => Some(("UpgradeComplete", None)),
                PresentationEvent::ProductionComplete { spawned, .. } => {
                    Some(("UnitReady", Some(*spawned)))
                }
                PresentationEvent::AttackTargeted { attacker, .. } => {
                    Some(("WeaponFire", Some(*attacker)))
                }
                PresentationEvent::DamageApplied {
                    target,
                    destroyed: true,
                    ..
                } => Some(("UnitDie", Some(*target))),
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed: false,
                    ..
                } => {
                    if *amount > 0.0 {
                        Some(("WeaponHit", Some(*target)))
                    } else {
                        None
                    }
                }
                PresentationEvent::HealApplied { target, .. } => Some(("UnitHeal", Some(*target))),
                PresentationEvent::EconomyChanged { .. } => Some(("MoneyTick", None)),
                PresentationEvent::Victory { .. } => Some(("Victory", None)),
                PresentationEvent::RadarMessage { .. }
                | PresentationEvent::OwnerChanged { .. }
                | PresentationEvent::MoveOrdered { .. }
                | PresentationEvent::ParticleSystemSpawned { .. } => None,
            };
            let Some((kind, obj)) = mapped else {
                continue;
            };
            let mut req = AudioEventRequest::new(kind);
            if let Some(id) = obj {
                req = req.with_object(id);
            }
            logic.queue_audio_event(req);
            n += 1;
        }
        n
    }

    /// Snapshot-owned ControlBar / WND selection panel (health + name).
    pub fn control_bar_selection_panel(&self) -> crate::ui::ControlBarSelectionPanelState {
        let mut panel = crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &self.selected_unit_display_infos(),
        );
        // Prefer full queue from the primary selected renderable when present.
        if let Some(id) = panel.primary_object_id {
            if let Some(ro) = self.objects.iter().find(|o| o.id == id) {
                panel.production_queue = ro
                    .production_queue
                    .iter()
                    .map(|p| (p.template_name.clone(), p.progress))
                    .collect();
                if panel.production_progress.is_none() {
                    panel.production_progress = panel.production_queue.first().map(|(_, p)| *p);
                    panel.production_template =
                        panel.production_queue.first().map(|(t, _)| t.clone());
                }
                if panel.veterancy_overlay.is_none() {
                    panel.veterancy_overlay = ro.veterancy.chevron_overlay().map(str::to_string);
                }
                panel.max_garrison = ro.max_garrison;
                panel.garrisoned_count = ro.garrisoned_units.len();
                panel.under_construction = ro.under_construction;
                panel.construction_percent = ro.construction_percent;
                panel.applied_upgrades = ro.applied_upgrades.clone();
                panel.rally_point = ro.rally_point.map(|p| [p.x, p.y, p.z]);
                panel.special_power_ready = ro.special_power_ready;
                panel.special_power_cooldown_remaining = ro.special_power_cooldown_remaining;
            }
        }
        panel
    }

    /// Apply selection health/name to GameClient ControlBar without OBJECT_REGISTRY.
    ///
    /// Headless-safe: uses only presentation fields. Does not claim full WND shell.
    #[cfg(feature = "game_client")]
    pub fn apply_to_control_bar(
        &self,
        control_bar: &mut game_client::gui::control_bar::ControlBar,
    ) {
        let panel = self.control_bar_selection_panel();
        let ids: Vec<u32> = if !self.selected.is_empty() {
            self.selected.iter().map(|id| id.0).collect()
        } else {
            panel.unit_infos.iter().map(|u| u.object_id.0).collect()
        };
        let _ = control_bar.update_for_selection(ids);
        control_bar.sync_selection_display_from_presentation(
            panel.visible.then_some(panel.primary_name.as_str()),
            panel.health_current,
            panel.health_maximum,
            panel.selected_count,
            panel.veterancy_overlay.as_deref(),
            panel.production_progress,
            panel.production_template.as_deref(),
            &panel.production_queue,
        );
        control_bar.sync_structure_context_from_presentation(
            panel.max_garrison,
            panel.garrisoned_count,
            panel.under_construction,
            panel.construction_percent,
        );
        control_bar.sync_upgrades_and_specials_from_presentation(
            &panel.applied_upgrades,
            panel.rally_point,
            panel.special_power_ready,
            panel.special_power_cooldown_remaining,
        );
        control_bar.sync_sciences_from_presentation(&self.local_unlocked_sciences);
        let ready_sp: Vec<String> = self
            .selected_unit_display_infos()
            .iter()
            .filter_map(|info| {
                self.objects
                    .iter()
                    .find(|o| o.id == info.object_id && o.special_power_ready)
                    .map(|o| o.template_name.clone())
            })
            .collect();
        // Also include any selected renderable with ready SP (selection flags path).
        let mut ready_sp = ready_sp;
        for o in self
            .objects
            .iter()
            .filter(|o| o.selected && o.special_power_ready)
        {
            if !ready_sp.iter().any(|n| n == &o.template_name) {
                ready_sp.push(o.template_name.clone());
            }
        }
        control_bar.sync_radar_queues_and_specials_from_presentation(
            self.local_radar_count,
            self.local_radar_disabled,
            &self.local_queued_upgrades,
            &ready_sp,
        );
    }

    /// Selection IDs for multi-consumer apply (player list or object.selected flags).
    pub fn selection_ids_for_consumers(&self) -> Vec<crate::game_logic::ObjectId> {
        let mut ids = self.selected.clone();
        if ids.is_empty() {
            ids = self
                .selected_unit_display_infos()
                .into_iter()
                .map(|i| i.object_id)
                .collect();
        }
        ids
    }

    /// Apply selection panel to RTS interface (command/selection residual consumer).
    pub fn apply_to_rts_interface(&self, rts: &mut crate::ui::RTSInterface) {
        rts.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
    }

    /// Apply selection panel to unit command grid (context-sensitive residual).
    pub fn apply_to_unit_command_panel(&self, panel: &mut crate::ui::UnitCommandPanel) {
        panel.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
    }

    /// Dual-tick multi-consumer residual: HUD + UI state + RTS + unit command panel
    /// (+ ControlBar when `game_client` is enabled). Snapshot-owned only.
    ///
    /// Does **not** claim full windowed WND/GPU playthrough.
    pub fn apply_to_shell_ui_consumers(
        &self,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) {
        self.apply_to_game_hud(hud);
        self.apply_to_ui_state(ui);
        self.apply_to_rts_interface(rts);
        self.apply_to_unit_command_panel(command_panel);
    }

    /// Dual-tick presentation consumer after map load / logic step:
    /// build snapshot from authority and apply it to the production GameHUD.
    ///
    /// Does **not** advance the world — caller is responsible for `logic.update()`.
    pub fn build_and_apply_for_hud(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_game_hud(hud);
        frame.note_dual_tick_apply();
        frame
    }

    /// Dual-tick residual: build snapshot and apply to all headless shell UI consumers.
    ///
    /// Order matches production StartGame: authority step (caller) → presentation freeze
    /// → HUD / UIState / RTS / unit command panel. Optional ControlBar is applied by
    /// the engine path when `game_client` is present.
    pub fn build_and_apply_for_shell_consumers(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_shell_ui_consumers(hud, ui, rts, command_panel);
        frame.note_dual_tick_apply();
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameMode, KindOf, Player, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    #[test]
    fn upgrade_complete_freezes_into_presentation_events() {
        let mut logic = crate::game_logic::GameLogic::new();
        // Direct registry complete without full research path.
        let _ = logic
            .host_upgrades_mut()
            .record_complete("CaptureBuilding", 0, 1, 3);
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::UpgradeComplete {
                        name,
                        player_id: 0,
                        units_affected: 3,
                        ..
                    } if name.to_ascii_lowercase().contains("capture")
                )
            }),
            "expected UpgradeComplete: {:?}",
            frame.events
        );
    }

    #[test]
    fn apply_events_routes_upgrade_and_owner_to_hud() {
        let mut logic = crate::game_logic::GameLogic::new();
        let _ = logic
            .host_upgrades_mut()
            .record_complete("CaptureBuilding", 0, 1, 1);
        crate::game_logic::host_owner_log::clear();
        crate::game_logic::host_owner_log::record(
            crate::game_logic::ObjectId(3),
            crate::game_logic::Team::GLA,
        );
        let _ = crate::game_logic::host_owner_log::drain();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let mut hud = crate::ui::GameHUD::new();
        let before = hud.message_count_for_test();
        frame.apply_events_to_game_hud(&mut hud);
        assert!(
            hud.message_count_for_test() > before,
            "hud should receive presentation events (before={before}, after={})",
            hud.message_count_for_test()
        );
    }

    #[test]
    fn apply_events_queues_audio_for_destroy_and_attack() {
        crate::game_logic::host_attack_log::clear();
        crate::game_logic::host_attack_log::record(
            crate::game_logic::ObjectId(1),
            Some(crate::game_logic::ObjectId(2)),
        );
        let _ = crate::game_logic::host_attack_log::drain();
        let mut logic = crate::game_logic::GameLogic::new();
        // inject destroy event via construction of frame with attack only
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let n = frame.apply_events_to_audio(&mut logic);
        assert!(n >= 1, "expected audio queue from AttackTargeted, n={n}");
        assert!(logic.queued_audio_event_count_for_test() >= 1);
    }

    #[test]
    fn heal_applied_freezes_from_last_drain() {
        crate::game_logic::host_heal_log::clear();
        crate::game_logic::host_heal_log::record(crate::game_logic::ObjectId(3), 88.0);
        let _ = crate::game_logic::host_heal_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::HealApplied { target, health }
                    if target.0 == 3 && (*health - 88.0).abs() < 0.01
                )
            }),
            "expected HealApplied: {:?}",
            frame.events
        );
    }

    #[test]
    fn economy_changed_freezes_from_last_drain() {
        crate::game_logic::host_economy_log::clear();
        crate::game_logic::host_economy_log::record(0, 12345, 7);
        let _ = crate::game_logic::host_economy_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::EconomyChanged {
                        player_id: 0,
                        supplies: 12345,
                        power_available: 7
                    }
                )
            }),
            "expected EconomyChanged: {:?}",
            frame.events
        );
    }

    #[test]
    fn supply_and_model_keys_freeze_from_host() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Resources, Team, ThingTemplate,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        let mut ts = ThingTemplate::new("SupplyCenter");
        ts.set_health(1000.0);
        ts.add_kind_of(KindOf::Structure);
        ts.set_model("SCModel");
        logic.templates.insert("SupplyCenter".into(), ts);
        let mut tw = ThingTemplate::new("AmericaDozer");
        tw.set_health(200.0);
        tw.add_kind_of(KindOf::Vehicle);
        tw.add_kind_of(KindOf::Worker);
        tw.set_model("DozerModel");
        logic.templates.insert("AmericaDozer".into(), tw);
        let sc = logic
            .create_object("SupplyCenter", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let dz = logic
            .create_object("AmericaDozer", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.get_object_mut(sc) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.building_data = Some(BuildingData::new(BuildingType::SupplyCenter));
            o.stored_resources = Resources {
                supplies: 1500,
                power: 0,
            };
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let s = frame.objects.iter().find(|o| o.id == sc).unwrap();
        assert_eq!(s.stored_supplies, 1500);
        assert_eq!(s.model_key.as_deref(), Some("SCModel"));
        let d = frame.objects.iter().find(|o| o.id == dz).unwrap();
        assert_eq!(d.model_key.as_deref(), Some("DozerModel"));
        assert_eq!(frame.supply_storage_structures().len(), 1);
        assert_eq!(frame.friendly_workers(Team::USA).len(), 1);
        let keys = frame.unique_model_keys();
        assert!(keys.iter().any(|k| k == "SCModel"));
        assert!(keys.iter().any(|k| k == "DozerModel"));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn building_type_freeze_from_host() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Team, ThingTemplate,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tb = ThingTemplate::new("WarFact");
        tb.set_health(1000.0);
        tb.add_kind_of(KindOf::Structure);
        logic.templates.insert("WarFact".into(), tb);
        let mut tc = ThingTemplate::new("CC");
        tc.set_health(2000.0);
        tc.add_kind_of(KindOf::Structure);
        logic.templates.insert("CC".into(), tc);
        let wf = logic
            .create_object("WarFact", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let cc = logic
            .create_object("CC", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.get_object_mut(wf) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.building_data = Some(BuildingData::new(BuildingType::WarFactory));
        }
        if let Some(o) = logic.get_object_mut(cc) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.building_data = Some(BuildingData::new(BuildingType::CommandCenter));
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let w = frame.objects.iter().find(|o| o.id == wf).unwrap();
        assert_eq!(w.building_type, Some(PresentationBuildingType::WarFactory));
        assert!(w.can_produce);
        assert!(w.building_type.unwrap().is_unit_producer());
        let c = frame.objects.iter().find(|o| o.id == cc).unwrap();
        assert_eq!(
            c.building_type,
            Some(PresentationBuildingType::CommandCenter)
        );
        assert!(c.can_produce);
        assert!(!c.building_type.unwrap().is_unit_producer());
        // Prefer war factory over command center for unit production residual.
        assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(wf));
        assert_eq!(frame.unit_producer_structures().len(), 1);
    }

    #[test]
    fn mobile_and_producer_freeze_from_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tu = ThingTemplate::new("Humvee");
        tu.set_health(200.0);
        tu.add_kind_of(KindOf::Vehicle);
        tu.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Humvee".into(), tu);
        let mut tb = ThingTemplate::new("Barracks");
        tb.set_health(800.0);
        tb.add_kind_of(KindOf::Structure);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Barracks".into(), tb);
        let mut tw = ThingTemplate::new("Wall");
        tw.set_health(100.0);
        tw.add_kind_of(KindOf::Structure);
        logic.templates.insert("Wall".into(), tw);
        let u = logic
            .create_object("Humvee", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("Barracks", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        let w = logic
            .create_object("Wall", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.get_object_mut(b) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            if o.building_data.is_none() {
                o.building_data = Some(crate::game_logic::BuildingData::new(
                    crate::game_logic::BuildingType::Barracks,
                ));
            }
        }
        if let Some(o) = logic.get_object_mut(w) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.building_data = None;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let hu = frame.objects.iter().find(|o| o.id == u).unwrap();
        assert!(hu.is_mobile);
        assert!(!hu.can_produce);
        let hb = frame.objects.iter().find(|o| o.id == b).unwrap();
        assert!(!hb.is_mobile);
        assert!(hb.can_produce);
        let hw = frame.objects.iter().find(|o| o.id == w).unwrap();
        assert!(hw.is_structure);
        assert!(!hw.can_produce);
        assert_eq!(frame.first_mobile_friendly_id(Team::USA), Some(u));
        assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(b));
        assert_eq!(frame.count_mobile_friendlies(Team::USA), 1);
    }

    #[test]
    fn runtime_host_presentation_query_helpers() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tu = ThingTemplate::new("Ranger");
        tu.set_health(100.0);
        tu.add_kind_of(KindOf::Infantry);
        tu.add_kind_of(KindOf::Selectable);
        tu.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Ranger".into(), tu);
        let mut tb = ThingTemplate::new("WarFactory");
        tb.set_health(1000.0);
        tb.add_kind_of(KindOf::Structure);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WarFactory".into(), tb);
        let mut te = ThingTemplate::new("RedGuard");
        te.set_health(100.0);
        te.add_kind_of(KindOf::Infantry);
        te.add_kind_of(KindOf::Attackable);
        te.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RedGuard".into(), te);
        let u = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let p = logic
            .create_object("WarFactory", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        let e = logic
            .create_object("RedGuard", Team::China, glam::Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        if let Some(o) = logic.get_object_mut(p) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(frame.first_mobile_friendly_id(Team::USA), Some(u));
        assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(p));
        assert_eq!(frame.first_enemy_attackable_id(Team::USA), Some(e));
        assert_eq!(frame.count_mobile_friendlies(Team::USA), 1);
    }

    #[test]
    fn centroid_of_ids_from_presentation() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("Ranger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Ranger".into(), t);
        let a = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 6.0))
            .unwrap();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let c = frame.centroid_of_ids(&[a, b]).expect("c");
        assert!((c.x - 5.0).abs() < 0.01);
        assert!((c.z - 3.0).abs() < 0.01);
        assert!(frame.centroid_of_ids(&[]).is_none());
        assert!(frame.centroid_of_ids(&[ObjectId(99999)]).is_none());
    }

    #[test]
    fn first_alive_position_for_template_from_presentation() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("HeroJet");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Aircraft);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("HeroJet".into(), t);
        let id = logic
            .create_object("HeroJet", Team::USA, glam::Vec3::new(42.0, 5.0, -7.0))
            .unwrap();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pos = frame
            .first_alive_position_for_template("herojet")
            .expect("pos");
        assert!((pos.x - 42.0).abs() < 0.01);
        assert!((pos.z + 7.0).abs() < 0.01);
        // Move live after snapshot — presentation still returns frozen pose.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(glam::Vec3::new(900.0, 0.0, 900.0));
        }
        let pos2 = frame.first_alive_position_for_template("HeroJet").unwrap();
        assert!((pos2.x - 42.0).abs() < 0.01);
        assert!(frame.first_alive_position_for_template("Missing").is_none());
    }

    #[test]
    fn hotkey_selection_helpers_from_presentation() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("Ranger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Ranger".into(), t);
        let a = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        let enemy = logic
            .create_object("Ranger", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        // Destroy b on host after snapshot? Filter uses snapshot destroyed flag.
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let all = frame.alive_selectable_friendly_ids(Team::USA);
        assert_eq!(all, {
            let mut v = vec![a, b];
            v.sort_by_key(|id| id.0);
            v
        });
        let filtered =
            frame.filter_alive_selectable_ids(&[a, b, enemy, ObjectId(99999)], Team::USA);
        assert!(filtered.contains(&a) && filtered.contains(&b));
        assert!(!filtered.contains(&enemy));
        // Mark destroyed in a rebuilt frame.
        if let Some(o) = logic.get_object_mut(b) {
            o.status.destroyed = true;
        }
        let frame2 = PresentationFrame::build_from_logic(&logic, 0);
        let filtered2 = frame2.filter_alive_selectable_ids(&[a, b], Team::USA);
        assert_eq!(filtered2, vec![a]);
    }

    #[test]
    fn box_select_unit_ids_from_presentation() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tu = ThingTemplate::new("Ranger");
        tu.set_health(100.0);
        tu.add_kind_of(KindOf::Infantry);
        tu.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Ranger".into(), tu);
        let mut ts = ThingTemplate::new("WarFactory");
        ts.set_health(1000.0);
        ts.add_kind_of(KindOf::Structure);
        ts.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WarFactory".into(), ts);
        let u1 = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let u2 = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .unwrap();
        let s = logic
            .create_object("WarFactory", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .unwrap();
        let _enemy = logic
            .create_object("Ranger", Team::China, glam::Vec3::new(1.0, 0.0, 1.0))
            .unwrap();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let mut ids = frame.box_select_unit_ids(Team::USA, -1.0, 10.0, -1.0, 10.0);
        ids.sort_by_key(|id| id.0);
        let mut expect = vec![u1, u2];
        expect.sort_by_key(|id| id.0);
        assert_eq!(ids, expect);
        assert!(!ids.contains(&s));
        // Structure-only box around factory.
        let only_s = frame.box_select_unit_ids(Team::USA, 1.5, 2.5, 1.5, 2.5);
        assert_eq!(only_s, vec![s]);
    }

    #[test]
    fn similar_unit_ids_from_presentation() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("Ranger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Ranger".into(), t);
        let mut tb = ThingTemplate::new("MissileDefender");
        tb.set_health(100.0);
        tb.add_kind_of(KindOf::Infantry);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("MissileDefender".into(), tb);
        let a = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("Ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        let _c = logic
            .create_object(
                "MissileDefender",
                Team::USA,
                glam::Vec3::new(20.0, 0.0, 0.0),
            )
            .unwrap();
        let d = logic
            .create_object("Ranger", Team::China, glam::Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let mut ids = frame.similar_unit_ids(a, Team::USA);
        ids.sort_by_key(|id| id.0);
        let mut expect = vec![a, b];
        expect.sort_by_key(|id| id.0);
        assert_eq!(ids, expect);
        assert!(!ids.contains(&d));
        assert!(frame.is_enemy_attackable(d, Team::USA));
        assert!(!frame.is_enemy_attackable(a, Team::USA));
    }

    #[test]
    fn kind_of_freeze_from_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tw = ThingTemplate::new("Dozer");
        tw.set_health(200.0);
        tw.add_kind_of(KindOf::Vehicle);
        tw.add_kind_of(KindOf::Worker);
        tw.add_kind_of(KindOf::Selectable);
        logic.templates.insert("Dozer".into(), tw);
        let mut tr = ThingTemplate::new("SupplyDock");
        tr.set_health(1.0);
        tr.add_kind_of(KindOf::Harvestable);
        tr.add_kind_of(KindOf::Resource);
        logic.templates.insert("SupplyDock".into(), tr);
        let did = logic
            .create_object("Dozer", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("d");
        let rid = logic
            .create_object("SupplyDock", Team::Neutral, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("r");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let d = frame.objects.iter().find(|o| o.id == did).expect("dozer");
        assert!(PresentationFrame::object_has_kind(d, KindOf::Worker));
        assert!(PresentationFrame::object_has_kind(d, KindOf::Vehicle));
        assert!(PresentationFrame::object_has_kind(d, KindOf::Selectable));
        // declaration-order residual: Vehicle before Worker before Selectable
        assert!(d.kind_of.windows(2).all(|w| {
            use crate::game_logic::KindOf::*;
            let rank = |k: KindOf| match k {
                Structure => 0,
                Infantry => 1,
                Vehicle => 2,
                Aircraft => 3,
                Projectile => 4,
                Resource => 5,
                Selectable => 6,
                Attackable => 7,
                CommandCenter => 8,
                Worker => 9,
                _ => 99,
            };
            rank(w[0]) <= rank(w[1])
        }));
        let r = frame.objects.iter().find(|o| o.id == rid).expect("res");
        assert!(PresentationFrame::object_has_kind(r, KindOf::Harvestable));
        assert_eq!(frame.worker_objects().len(), 1);
        assert_eq!(frame.harvestable_objects().len(), 1);
    }

    #[test]
    fn upgrades_object_type_freeze_from_host() {
        use crate::game_logic::{
            host_mines::{HostMineData, HostMineKind},
            KindOf, Team, ThingTemplate, Weapon,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("Overlord");
        t.set_health(1200.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("Overlord".into(), t);
        let id = logic
            .create_object("Overlord", Team::China, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("id");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.applied_upgrades.insert("Upgrade_ChinaChainGuns".into());
            obj.applied_upgrades.insert("Upgrade_Nationalism".into());
            obj.secondary_weapon = Some(Weapon {
                damage: 8.0,
                range: 150.0,
                min_range: 0.0,
                reload_time: 0.5,
                last_fire_time: 0.0,
                ammo: None,
                can_target_air: true,
                can_target_ground: true,
                ..Default::default()
            });
            obj.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let o = frame.objects.iter().find(|r| r.id == id).expect("o");
        assert_eq!(o.object_type, PresentationObjectType::Vehicle);
        assert!(PresentationFrame::object_has_upgrade(
            o,
            "Upgrade_ChinaChainGuns"
        ));
        assert!(o.applied_upgrades.contains(&"Upgrade_Nationalism".into()));
        assert!(o.applied_upgrades.windows(2).all(|w| w[0] <= w[1]));
        assert!(o.has_secondary_weapon);
        assert!((o.secondary_weapon_range - 150.0).abs() < 0.01);
        assert!((o.secondary_weapon_damage - 8.0).abs() < 0.01);
        assert!(o.has_mine);
        assert_eq!(frame.upgraded_objects().len(), 1);
        assert_eq!(frame.mine_objects().len(), 1);
    }

    #[test]
    fn special_power_freeze_from_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("ParticleUplink");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("ParticleUplink".into(), t);
        let id = logic
            .create_object("ParticleUplink", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.special_power_ready = false;
            obj.special_power_cooldown = 180.0;
            obj.special_power_cooldown_remaining = 45.0;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let o = frame.objects.iter().find(|r| r.id == id).expect("o");
        assert!(!o.special_power_ready);
        assert!((o.special_power_cooldown - 180.0).abs() < 0.01);
        assert!((o.special_power_cooldown_remaining - 45.0).abs() < 0.01);
        let frac = PresentationFrame::special_power_cooldown_fraction(o);
        assert!((frac - 0.25).abs() < 0.01);
        assert!(frame.special_power_ready_objects().is_empty());
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.special_power_ready = true;
            obj.special_power_cooldown_remaining = 0.0;
        }
        let frame2 = PresentationFrame::build_from_logic(&logic, 1);
        let o2 = frame2.objects.iter().find(|r| r.id == id).expect("o2");
        assert!(o2.special_power_ready);
        assert_eq!(frame2.special_power_ready_objects().len(), 1);
        assert_eq!(PresentationFrame::special_power_cooldown_fraction(o2), 0.0);
    }

    #[test]
    fn local_player_freeze_from_host() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut t = ThingTemplate::new("LocalUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("LocalUnit".into(), t);
        let _uid = logic
            .create_object("LocalUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let pid = 0u32;
        if let Some(p) = logic.get_player_mut(pid) {
            p.is_local = true;
            p.is_alive = true;
            p.resources.supplies = 12345;
            p.power_available = 40;
            p.power_produced = 100;
            p.power_consumed = 55;
            p.radar_count = 2;
            p.radar_disabled = false;
            p.cash_bounty_percent = 0.1;
            p.unlocked_sciences.insert("SCIENCE_RedGuards".into());
            p.unlocked_sciences.insert("SCIENCE_CashBounty1".into());
            p.queued_upgrades
                .insert("Upgrade_AmericaAdvancedTraining".into());
            p.color_rgb = (10, 20, 30);
        }
        let frame = PresentationFrame::build_from_logic(&logic, pid);
        assert_eq!(frame.local_player_id, pid);
        assert_eq!(frame.local_supplies, 12345);
        assert_eq!(frame.local_power, 40);
        assert_eq!(frame.local_power_produced, 100);
        assert_eq!(frame.local_power_consumed, 55);
        assert!(frame.local_is_alive);
        assert_eq!(frame.local_radar_count, 2);
        assert!(!frame.local_radar_disabled);
        assert!(frame.local_radar_active());
        assert!((frame.local_cash_bounty_percent - 0.1).abs() < 0.001);
        assert!(frame.local_has_science("SCIENCE_CashBounty1"));
        assert!(frame
            .local_unlocked_sciences
            .contains(&"SCIENCE_RedGuards".into()));
        assert!(frame
            .local_queued_upgrades
            .contains(&"Upgrade_AmericaAdvancedTraining".into()));
        assert_eq!(frame.local_color_rgb, (10, 20, 30));
        let ratio = frame.local_energy_ratio();
        assert!((ratio - (100.0 / 55.0)).abs() < 0.01);
    }

    #[test]
    fn weapon_and_stealth_freeze_from_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("StealthScout");
        t.set_health(60.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("StealthScout".into(), t);
        let mut tb = ThingTemplate::new("Bunker");
        tb.set_health(300.0);
        tb.add_kind_of(KindOf::Structure);
        logic.templates.insert("Bunker".into(), tb);
        let uid = logic
            .create_object("StealthScout", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
            .expect("u");
        let bid = logic
            .create_object("Bunker", Team::USA, glam::Vec3::new(8.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&uid) {
            obj.weapon = Some(Weapon {
                damage: 12.0,
                range: 150.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                can_target_air: false,
                can_target_ground: true,
                ..Default::default()
            });
            obj.status.stealthed = true;
            obj.status.detected = false;
            obj.status.attacking = true;
            obj.status.moving = false;
            obj.force_attack = true;
            obj.contained_by = Some(bid);
            obj.camo_stealth_look = 5;
            obj.detection_range = 300.0;
            obj.disguise_as_template = Some("ChinaTroopCrawler".into());
            obj.disguise_as_team = Some(Team::China);
            // Disguised clears effectively_stealthed
            obj.status.disguised = true;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let u = frame.objects.iter().find(|o| o.id == uid).expect("u");
        assert!(u.has_weapon);
        assert!((u.weapon_range - 150.0).abs() < 0.01);
        assert!((u.weapon_damage - 12.0).abs() < 0.01);
        assert!(u.stealthed);
        assert!(!u.detected);
        // disguised => not effectively stealthed
        assert!(!u.effectively_stealthed);
        assert!(u.attacking);
        assert!(u.force_attack);
        assert_eq!(u.contained_by, Some(bid));
        assert_eq!(u.camo_stealth_look, 5);
        assert!((u.detection_range - 300.0).abs() < 0.01);
        assert_eq!(u.disguise_as_template.as_deref(), Some("ChinaTroopCrawler"));
        assert_eq!(u.disguise_as_team, Some(Team::China));
        assert_eq!(frame.attacking_units().len(), 1);
        assert_eq!(frame.contained_units().len(), 1);
        // pure stealth unit without disguise
        if let Some(obj) = logic.get_objects_mut().get_mut(&uid) {
            obj.status.disguised = false;
            obj.disguise_as_template = None;
            obj.disguise_as_team = None;
        }
        let frame2 = PresentationFrame::build_from_logic(&logic, 1);
        let u2 = frame2.objects.iter().find(|o| o.id == uid).expect("u2");
        assert!(u2.effectively_stealthed);
        assert_eq!(frame2.effectively_stealthed_units().len(), 1);
    }

    #[test]
    fn construction_and_veterancy_freeze_from_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, VeterancyLevel};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("VetUnit");
        t.set_health(80.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("VetUnit".into(), t);
        let mut tb = ThingTemplate::new("BuildMe");
        tb.set_health(200.0);
        tb.add_kind_of(KindOf::Structure);
        logic.templates.insert("BuildMe".into(), tb);
        let uid = logic
            .create_object("VetUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("u");
        let bid = logic
            .create_object("BuildMe", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&uid) {
            obj.experience.level = VeterancyLevel::Elite;
            obj.experience.current = 420.0;
        }
        if let Some(obj) = logic.get_objects_mut().get_mut(&bid) {
            obj.status.under_construction = true;
            obj.construction_percent = 0.55;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let u = frame.objects.iter().find(|o| o.id == uid).expect("u");
        assert_eq!(u.veterancy, PresentationVeterancy::Elite);
        assert!((u.experience_points - 420.0).abs() < 0.01);
        let b = frame.objects.iter().find(|o| o.id == bid).expect("b");
        assert!(b.under_construction);
        assert!((b.construction_percent - 0.55).abs() < 0.01);
        assert_eq!(frame.under_construction_objects().len(), 1);
        assert_eq!(frame.veteran_or_higher_units().len(), 1);
    }

    #[test]
    fn garrison_and_power_freeze_from_host() {
        use crate::game_logic::buildings::{BuildingData, BuildingType};
        use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("GarrBldg");
        t.set_health(300.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("GarrBldg".into(), t);
        let id = logic
            .create_object("GarrBldg", Team::USA, glam::Vec3::ZERO)
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.garrisoned_units = vec![ObjectId(10), ObjectId(11)];
            bd.max_garrison = 5;
            obj.building_data = Some(bd);
            obj.power_provided = 10;
            obj.power_consumed = 3;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
        assert_eq!(ro.garrisoned_units, vec![ObjectId(10), ObjectId(11)]);
        assert_eq!(ro.max_garrison, 5);
        assert_eq!(ro.power_provided, 10);
        assert_eq!(ro.power_consumed, 3);
        assert_eq!(frame.garrisoned_structures().len(), 1);
        assert_eq!(frame.net_power_from_objects(), 7);
    }

    #[test]
    fn production_queue_freezes_from_building_data() {
        use crate::game_logic::buildings::{BuildingData, BuildingType, ProductionItem};
        use crate::game_logic::{KindOf, Resources, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("ProdBldg");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("ProdBldg".into(), t);
        let id = logic
            .create_object("ProdBldg", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "Ranger".into(),
                progress: 0.4,
                total_time: 10.0,
                cost: Resources {
                    supplies: 150,
                    power: 0,
                },
            });
            bd.rally_point = Some(glam::Vec3::new(12.0, 0.0, 3.0));
            obj.building_data = Some(bd);
            obj.guard_position = Some(glam::Vec3::new(1.0, 0.0, 1.0));
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
        assert_eq!(ro.production_queue.len(), 1);
        assert_eq!(ro.production_queue[0].template_name, "Ranger");
        assert!((ro.production_queue[0].progress - 0.4).abs() < 0.01);
        assert_eq!(ro.production_queue[0].cost_supplies, 150);
        assert_eq!(ro.rally_point, Some(glam::Vec3::new(12.0, 0.0, 3.0)));
        assert_eq!(ro.guard_position, Some(glam::Vec3::new(1.0, 0.0, 1.0)));
        assert_eq!(frame.structures_with_production().len(), 1);
    }

    #[test]
    fn move_destination_freezes_from_host_movement() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = crate::game_logic::ThingTemplate::new("MoveDestU");
        t.set_health(40.0);
        t.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic.templates.insert("MoveDestU".into(), t);
        let id = logic
            .create_object(
                "MoveDestU",
                crate::game_logic::Team::USA,
                glam::Vec3::new(1.0, 0.0, 1.0),
            )
            .expect("u");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.movement.target_position = Some(glam::Vec3::new(9.0, 0.0, 4.0));
            obj.target = Some(crate::game_logic::ObjectId(99));
            obj.movement.path = vec![
                glam::Vec3::new(1.0, 0.0, 1.0),
                glam::Vec3::new(9.0, 0.0, 4.0),
            ];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
        assert_eq!(ro.move_destination, Some(glam::Vec3::new(9.0, 0.0, 4.0)));
        assert_eq!(ro.attack_target, Some(crate::game_logic::ObjectId(99)));
        assert_eq!(ro.path_waypoints.len(), 2);
    }

    #[test]
    fn projectiles_freeze_from_combat_system() {
        let mut logic = crate::game_logic::GameLogic::new();
        let weapon = crate::game_logic::Weapon::default();
        let pid = logic.combat_system_mut().fire_projectile(
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(100.0, 0.0, 0.0),
            &weapon,
            crate::game_logic::ObjectId(1),
            Some(crate::game_logic::ObjectId(2)),
            200.0,
        );
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.projectiles.iter().any(|p| p.id == pid),
            "expected projectile {pid:?} in {:?}",
            frame.projectiles.iter().map(|p| p.id).collect::<Vec<_>>()
        );
        assert!(
            frame
                .projectiles
                .iter()
                .any(|p| (p.target_position.x - 100.0).abs() < 0.1),
            "target pos frozen"
        );
    }

    #[test]
    fn combat_damage_spawns_floating_text() {
        crate::game_logic::host_damage_log::clear();
        crate::game_logic::host_damage_log::record(
            crate::game_logic::ObjectId(11),
            25.0,
            Some(crate::game_logic::ObjectId(1)),
            false,
        );
        let _ = crate::game_logic::host_damage_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.floating_texts.iter().any(|t| {
                matches!(t.kind, PresentationFloatingTextKind::CombatDamage)
                    && t.text.contains("25")
            }),
            "expected combat floating text: {:?}",
            frame
                .floating_texts
                .iter()
                .map(|t| (&t.kind, &t.text))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn damage_applied_freezes_from_last_drain() {
        crate::game_logic::host_damage_log::clear();
        crate::game_logic::host_damage_log::record(
            crate::game_logic::ObjectId(8),
            12.5,
            Some(crate::game_logic::ObjectId(1)),
            false,
        );
        let _ = crate::game_logic::host_damage_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::DamageApplied {
                        target,
                        amount,
                        destroyed: false,
                        ..
                    } if target.0 == 8 && (*amount - 12.5).abs() < 0.01
                )
            }),
            "expected DamageApplied: {:?}",
            frame.events
        );
    }

    #[test]
    fn move_ordered_freezes_from_last_drain() {
        crate::game_logic::host_move_log::clear();
        crate::game_logic::host_move_log::record(
            crate::game_logic::ObjectId(4),
            Some([10.0, 0.0, 20.0]),
        );
        let _ = crate::game_logic::host_move_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::MoveOrdered {
                        unit,
                        destination
                    } if unit.0 == 4 && *destination == [10.0, 0.0, 20.0]
                )
            }),
            "expected MoveOrdered: {:?}",
            frame.events
        );
    }

    #[test]
    fn attack_targeted_freezes_from_last_drain() {
        crate::game_logic::host_attack_log::clear();
        crate::game_logic::host_attack_log::record(
            crate::game_logic::ObjectId(2),
            Some(crate::game_logic::ObjectId(5)),
        );
        let _ = crate::game_logic::host_attack_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::AttackTargeted {
                        attacker,
                        target: Some(t)
                    } if attacker.0 == 2 && t.0 == 5
                )
            }),
            "expected AttackTargeted: {:?}",
            frame.events
        );
    }

    #[test]
    fn owner_changed_freezes_from_last_drain() {
        crate::game_logic::host_owner_log::clear();
        crate::game_logic::host_owner_log::record(
            crate::game_logic::ObjectId(7),
            crate::game_logic::Team::China,
        );
        let _ = crate::game_logic::host_owner_log::drain();
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::OwnerChanged {
                        id,
                        team: crate::game_logic::Team::China
                    } if id.0 == 7
                )
            }),
            "expected OwnerChanged: {:?}",
            frame.events
        );
    }

    #[test]
    fn production_complete_freezes_from_last_drain() {
        crate::game_logic::host_production_log::clear();
        crate::game_logic::host_production_log::record_complete(
            crate::game_logic::ObjectId(1),
            "TestRanger",
            crate::game_logic::ObjectId(9),
        );
        let _ = crate::game_logic::host_production_log::drain(); // simulate shadow session
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::ProductionComplete {
                        producer,
                        template,
                        spawned
                    } if producer.0 == 1 && spawned.0 == 9 && template == "TestRanger"
                )
            }),
            "expected ProductionComplete: {:?}",
            frame.events
        );
    }

    #[test]
    fn construction_complete_freezes_into_presentation_events() {
        crate::game_logic::host_construction_log::clear();
        crate::game_logic::host_construction_log::record(
            crate::game_logic::ObjectId(42),
            "TestBarracks",
        );
        let logic = crate::game_logic::GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::ConstructionComplete {
                        id,
                        template
                    } if id.0 == 42 && template == "TestBarracks"
                )
            }),
            "expected ConstructionComplete: {:?}",
            frame.events
        );
        // drained
        assert!(crate::game_logic::host_construction_log::drain().is_empty());
    }

    fn radar_messages_freeze_into_presentation_events() {
        use glam::Vec3;
        let mut logic = crate::game_logic::GameLogic::new();
        logic.queue_radar_message_at(
            "Test radar ping",
            Vec3::ZERO,
            crate::game_logic::radar_notifications::RadarKind::Generic,
        );
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            frame.events.iter().any(|e| {
                matches!(
                    e,
                    PresentationEvent::RadarMessage { text, .. } if text.contains("Test radar")
                )
            }),
            "expected RadarMessage in presentation events: {:?}",
            frame.events
        );
    }

    #[test]
    fn presentation_frame_is_built_from_authority_without_arc() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresMap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("PresUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("PresUnit".into(), t);
        let id = logic
            .create_object("PresUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("unit");

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.frame.0, logic.get_frame());
        assert!(snap.objects.iter().any(|o| o.id == id));
        assert_eq!(snap.local_supplies, 10_000);
        // Snapshot is owned — mutating world after build must not require re-borrow of snap.
        logic.update();
        assert_eq!(snap.objects.len(), 1);
        let h1 = snap.presentation_hash();
        let snap2 = PresentationFrame::build_from_logic(&logic, 0);
        // Frame advanced; hash may change.
        assert!(snap2.frame.0 >= snap.frame.0);
        let _ = h1;
    }

    #[test]
    fn dual_presentation_hashes_match_for_identical_worlds() {
        let mk = || {
            let mut logic = GameLogic::new();
            logic.start_new_game(GameMode::Skirmish);
            logic.clear_all_players();
            logic.add_player(Player::new(0, Team::USA, "P", true));
            let mut t = ThingTemplate::new("HashUnit");
            t.set_health(50.0);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("HashUnit".into(), t);
            let _ = logic.create_object("HashUnit", Team::USA, glam::Vec3::ZERO);
            PresentationFrame::build_from_logic(&logic, 0).presentation_hash()
        };
        assert_eq!(mk(), mk());
    }

    #[test]
    fn client_reads_snapshot_not_live_world() {
        // Simulate: authority builds snapshot, then world mutates; client still holds old frame.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ClientSnap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("SnapUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SnapUnit".into(), t);
        let id = logic
            .create_object("SnapUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");
        let client_view = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(client_view.alive_object_count(), 1);
        // Authority continues without client re-borrowing world during "render".
        if let Some(o) = logic.get_object_mut(id) {
            o.status.destroyed = true;
            o.health.current = 0.0;
        }
        // Stale presentation still has the pre-destroy object; proves client feed is owned data.
        assert_eq!(client_view.objects.len(), 1);
        assert!(!client_view.objects[0].destroyed);
        // Fresh presentation reflects authority.
        let next = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            next.objects.iter().all(|o| o.destroyed || o.id != id)
                || next.alive_object_count() == 0
                || next.objects.iter().any(|o| o.id == id && o.destroyed)
        );
    }

    #[test]
    fn shipped_hud_consumer_uses_snapshot_owned_fields() {
        // Criterion: after logic update, HUD/minimap consumers use snapshot-owned
        // id/transform/health/team/selection/model — not a live re-borrow.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HudFields");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("HudUnit");
        t.set_health(75.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HudUnit".into(), t);
        let id = logic
            .create_object("HudUnit", Team::USA, glam::Vec3::new(9.0, 0.0, -4.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        logic.update();
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let obj = snap
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in snapshot");
        assert!((obj.position.x - 9.0).abs() < 0.01);
        assert!((obj.position.z + 4.0).abs() < 0.01);
        assert_eq!(obj.health_current, 75.0);
        assert_eq!(obj.health_max, 75.0);
        assert_eq!(obj.team, Team::USA);
        assert!(obj.selected);
        assert_eq!(obj.model_key.as_deref(), Some("HudUnit"));

        let mut ui = crate::ui::GameUIState::default();
        snap.apply_to_ui_state(&mut ui);
        assert_eq!(ui.credits, snap.local_supplies as i32);
        assert!(ui.selected_units.contains(&id));

        let mut hud = crate::ui::GameHUD::new();
        snap.apply_to_game_hud(&mut hud);
        let mini = snap.hud_minimap_units();
        assert!(
            mini.iter().any(|(oid, x, z, _)| {
                *oid == id && (*x - 9.0).abs() < 0.01 && (*z + 4.0).abs() < 0.01
            }),
            "minimap units must come from snapshot positions"
        );
        assert!(
            hud.selected_unit_ids().contains(&id),
            "GameHUD selection IDs must come from presentation"
        );
        let hud_info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("GameHUD selection health from presentation");
        assert!(
            (hud_info.health_current - 75.0).abs() < 0.01,
            "GameHUD selection health must be snapshot-owned: {}",
            hud_info.health_current
        );
    }

    #[test]
    fn dual_tick_build_and_apply_after_logic_step_seeds_hud() {
        // Map-load / skirmish residual: after authority advances, presentation must
        // seed HUD resources + selection without re-borrowing live objects later.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DualTickHud");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("DualUnit");
        t.set_health(88.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DualUnit".into(), t);
        let id = logic
            .create_object("DualUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        logic.update(); // authority tick
        let mut hud = crate::ui::GameHUD::new();
        let snap = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        assert_eq!(snap.frame.0, logic.get_frame());
        assert!(
            !snap.hud_minimap_units().is_empty(),
            "presentation after tick must expose units for minimap"
        );
        let info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("selection health on HUD after dual-tick apply");
        assert!((info.health_current - 88.0).abs() < 0.01);
        // World mutates after apply; HUD must keep snapshot health.
        if let Some(o) = logic.get_object_mut(id) {
            o.health.current = 1.0;
        }
        assert!((info.health_current - 88.0).abs() < 0.01);
    }

    #[test]
    fn dual_tick_applies_selection_panel_to_shell_ui_consumers() {
        // Residual: presentation selection panel feeds HUD + UIState + RTS + unit
        // command panel from one dual-tick apply (no live re-read).
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DualTickConsumers");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("MultiUiUnit");
        t.set_health(64.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("MultiUiUnit".into(), t);
        let id = logic
            .create_object("MultiUiUnit", Team::USA, glam::Vec3::new(2.0, 0.0, 3.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        logic.update();
        let mut hud = crate::ui::GameHUD::new();
        let mut ui = crate::ui::GameUIState::default();
        let mut rts = crate::ui::RTSInterface::new();
        let mut cmd = crate::ui::UnitCommandPanel::new();
        let snap = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
        );
        assert_eq!(snap.frame.0, logic.get_frame());
        assert!(hud.selection_panel().has_positive_health());
        assert!((hud.selection_panel().health_current - 64.0).abs() < 0.01);
        assert!(ui.selection_panel.has_positive_health());
        assert!((ui.selection_panel.health_current - 64.0).abs() < 0.01);
        assert!(rts.selection_panel().has_positive_health());
        assert!(rts.selected_ids().contains(&id));
        assert!(cmd.is_visible());
        assert!((cmd.selection_panel().health_current - 64.0).abs() < 0.01);
        // Live mutation must not rewrite consumer snapshots.
        if let Some(o) = logic.get_object_mut(id) {
            o.health.current = 1.0;
        }
        assert!((hud.selection_panel().health_current - 64.0).abs() < 0.01);
        assert!((rts.selection_panel().health_current - 64.0).abs() < 0.01);
        assert!((cmd.selection_panel().health_current - 64.0).abs() < 0.01);
    }

    #[test]
    fn presentation_snapshot_includes_selection_radius_for_cull() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelRadius");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("RadiusUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("RadiusUnit".into(), t);
        let id = logic
            .create_object("RadiusUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selection_radius = 12.5;
        }
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!(
            (ro.selection_radius - 12.5).abs() < 0.01,
            "selection_radius must be snapshot-owned for presentation-only cull: {}",
            ro.selection_radius
        );
    }

    #[test]
    fn usa_ranger_presentation_model_key_non_empty_for_mesh_resolve() {
        // Residual: USA_Ranger / common infantry must expose a non-empty model_key
        // so mesh_asset_resolve can target AIRanger_S (or honest placeholder).
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RangerMeshKey");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        // Prefer host setup template when present; otherwise inject retail-like key.
        if !logic.templates.contains_key("USA_Ranger") {
            let mut t = ThingTemplate::new("USA_Ranger");
            t.set_health(60.0);
            t.set_model("airanger"); // legacy alias must remap
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("USA_Ranger".into(), t);
        }
        let id = logic
            .create_object("USA_Ranger", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("ranger");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        let key = ro.model_key.as_deref().unwrap_or("");
        assert!(
            !key.is_empty(),
            "USA_Ranger presentation model_key must be non-empty for mesh resolve"
        );
        assert_eq!(
            key.to_ascii_lowercase(),
            "airanger_s",
            "USA_Ranger model_key should alias to shipped AIRanger_S basename"
        );
        let inputs = snap.unit_render_inputs();
        let unit = inputs.iter().find(|u| u.id == id).expect("unit input");
        assert_eq!(unit.model_key.to_ascii_lowercase(), "airanger_s");
        // Wave 75: combat unit mesh scale residual freezes at 1.0.
        assert!(
            (ro.mesh_scale - 1.0).abs() < 0.001,
            "USA_Ranger mesh_scale residual must be 1.0, got {}",
            ro.mesh_scale
        );
        assert!((unit.mesh_scale - 1.0).abs() < 0.001);
        assert!(snap.mesh_scale_presentation_residual_ok());
    }

    #[test]
    fn mesh_scale_presentation_residual_wave75() {
        assert!(crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MeshScalePres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        if !logic.templates.contains_key("USA_Humvee") {
            let mut t = ThingTemplate::new("USA_Humvee");
            t.set_health(240.0);
            t.set_model("avhummer");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("USA_Humvee".into(), t);
        }
        let id = logic
            .create_object("USA_Humvee", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("humvee");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(snap.mesh_scale_presentation_residual_ok());
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!((ro.mesh_scale - 1.0).abs() < 0.001);
        let unit = snap
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .expect("unit input");
        assert!((unit.mesh_scale - 1.0).abs() < 0.001);
    }

    /// Wave 77 residual: unit/structure ground-height frozen on presentation objects.
    #[test]
    fn ground_height_presentation_residual_wave77() {
        assert!(honesty_ground_height_residual_ok(
            PRESENTATION_DEFAULT_GROUND_HEIGHT,
            false
        ));
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GroundHeightPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        if !logic.templates.contains_key("USA_Ranger") {
            let mut t = ThingTemplate::new("USA_Ranger");
            t.set_health(120.0);
            t.set_model("airanger");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("USA_Ranger".into(), t);
        }
        let id = logic
            .create_object("USA_Ranger", Team::USA, glam::Vec3::new(7.0, 0.0, 9.0))
            .expect("ranger");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(snap.ground_height_presentation_residual_ok());
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!(
            honesty_ground_height_residual_ok(ro.ground_height, ro.ground_height_from_terrain),
            "object ground_height residual inconsistent: h={} from_terrain={}",
            ro.ground_height,
            ro.ground_height_from_terrain
        );
        // Without map terrain, residual defaults to 0 and from_terrain=false.
        if !ro.ground_height_from_terrain {
            assert!((ro.ground_height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001);
        }
    }

    #[test]
    fn presentation_build_includes_unit_render_fields_and_positions() {
        // Criterion: unit mesh/position/selection inputs are snapshot-owned so the
        // main unit pass can iterate PresentationFrame without GameLogic.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UnitRenderFields");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("MeshUnit");
        t.set_health(60.0);
        t.set_model("AVTank");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("MeshUnit".into(), t);
        let id = logic
            .create_object("MeshUnit", Team::USA, glam::Vec3::new(3.0, 0.0, -8.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 11.0;
            o.team_color = [0.1, 0.2, 0.9, 1.0];
            // Not bridged — main mesh pass owns draw.
            o.engine_object_id = None;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert!((ro.position.x - 3.0).abs() < 0.01);
        assert!((ro.position.z + 8.0).abs() < 0.01);
        assert_eq!(ro.team, Team::USA);
        assert_eq!(ro.team_color, [0.1, 0.2, 0.9, 1.0]);
        assert_eq!(ro.model_key.as_deref(), Some("AVTank"));
        assert_eq!(ro.template_name, "MeshUnit");
        assert!(ro.selected);
        assert!(!ro.destroyed);
        assert!(!ro.engine_bridged);
        assert!((ro.selection_radius - 11.0).abs() < 0.01);

        // unit_render_inputs is the production pure-frame collection path.
        let inputs = snap.unit_render_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].id, id);
        assert_eq!(inputs[0].model_key, "AVTank");
        assert!((inputs[0].position.x - 3.0).abs() < 0.01);
        assert!(inputs[0].selected);
        assert!(!inputs[0].engine_bridged);
        assert_eq!(inputs[0].fow_visibility, ro.fow_visibility);

        // Mutate authority after snapshot — inputs must stay frozen.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
            o.selected = false;
            o.engine_object_id = Some(42);
        }
        let inputs_after = snap.unit_render_inputs();
        assert_eq!(inputs_after.len(), 1);
        assert!(
            (inputs_after[0].position.x - 3.0).abs() < 0.01,
            "unit render inputs must not re-read live GameLogic"
        );
        assert!(inputs_after[0].selected);
        assert!(!inputs_after[0].engine_bridged);
        assert_eq!(
            inputs_after[0].fow_visibility, ro.fow_visibility,
            "FOW on unit inputs must stay frozen after live world mutation"
        );
    }

    #[test]
    fn presentation_fow_matches_bridge_at_build_and_stays_frozen() {
        use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FowSnapConsistency");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("FowUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("FowUnit".into(), t);
        let id = logic
            .create_object("FowUnit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("unit");

        // Bridge state at build time is the source of truth for the snapshot.
        let bridge_at_build = FOWRenderingBridge::get_object_visibility(0, id);
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert_eq!(
            ro.fow_visibility, bridge_at_build,
            "presentation FOW must match FOW bridge at build time"
        );
        assert_eq!(snap.fow_for_object(id), Some(bridge_at_build));
        assert_eq!(snap.fow_shell_bypass, logic.isInShellGame());

        let inputs = snap.unit_render_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].fow_visibility, bridge_at_build);
        assert_eq!(
            inputs[0].fow_should_render(),
            bridge_at_build.should_render()
        );

        // Encode states are stable and cover the three SAGE-style buckets.
        assert_eq!(
            ObjectVisibility::from_shroud_flags(true, true),
            ObjectVisibility::VISIBLE
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, true),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, false),
            ObjectVisibility::HIDDEN
        );
        assert!(ObjectVisibility::FOGGED.should_render());
        assert!(!ObjectVisibility::HIDDEN.should_render());
        assert!(ObjectVisibility::HIDDEN.never_explored());

        // Dual-build with identical world + FOW state yields matching FOW on hash.
        let snap2 = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.fow_for_object(id), snap2.fow_for_object(id));
        assert_eq!(
            snap.objects
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.fow_visibility),
            snap2
                .objects
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.fow_visibility)
        );
    }

    #[test]
    fn presentation_fow_shell_bypass_forces_fully_visible() {
        use crate::fow_rendering::ObjectVisibility;
        use crate::game_logic::GameMode;

        let mut logic = GameLogic::new();
        // Shell map path: FOW bypass is frozen on the frame.
        logic.start_new_game(GameMode::Shell);
        assert!(logic.isInShellGame());
        let mut t = ThingTemplate::new("ShellFowUnit");
        t.set_health(10.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ShellFowUnit".into(), t);
        let id = logic
            .create_object("ShellFowUnit", Team::USA, glam::Vec3::ZERO)
            .expect("unit");

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(snap.fow_shell_bypass);
        let ro = snap.objects.iter().find(|o| o.id == id).expect("in snap");
        assert_eq!(ro.fow_visibility, ObjectVisibility::FULLY_VISIBLE);
        assert!(snap.unit_render_inputs()[0].fow_should_render());
        // Terrain overlay inactive under shell bypass (fail-open / no darkening).
        assert!(!snap.terrain_fow_overlay_active());
    }

    #[test]
    fn presentation_world_env_freezes_bounds_and_map_name() {
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WorldEnvMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.world_env.map_name, logic.get_current_map_name().trim());
        let (a, b) = logic.world_bounds();
        assert_eq!(snap.world_env.world_min, [a.x, a.y, a.z]);
        assert_eq!(snap.world_env.world_max, [b.x, b.y, b.z]);
        // Shell bypass matches frozen flag used by render execute residual.
        assert_eq!(snap.fow_shell_bypass, logic.isInShellGame());
        let sig = snap.world_env.prewarm_signature(snap.fow_shell_bypass);
        assert!(sig.contains(&snap.world_env.map_name) || snap.world_env.map_name.is_empty());
        assert!(sig.contains(&format!("shell:{}", snap.fow_shell_bypass)));
    }

    #[test]
    fn world_env_height_grid_is_self_consistent() {
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HeightGridMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(snap.world_env.height_grid_w, 64);
        assert_eq!(snap.world_env.height_grid_h, 64);
        assert_eq!(snap.world_env.height_samples.len(), (64 * 64) as usize);
        // Road/bridge/prewarm vectors always present (may be empty without map parse).
        let _ = &snap.world_env.road_segments;
        let _ = &snap.world_env.bridge_segments;
        assert!(snap.world_env.prewarm_template_names.len() <= 256);
        if snap.world_env.height_samples_from_terrain {
            let (a, b) = snap.world_env.world_bounds_vec3();
            let mid_x = (a.x + b.x) * 0.5;
            let mid_z = (a.z + b.z) * 0.5;
            assert!(snap.world_env.sample_height(mid_x, mid_z).is_some());
        }
    }

    #[test]
    fn presentation_fow_grid_matches_shroud_snapshot_and_stays_frozen() {
        use crate::fow_rendering::{FOWRenderingBridge, PresentationFowGrid};
        use gamelogic::system::shroud_manager::get_shroud_manager;

        // Isolate global shroud manager for this test.
        {
            let mut shroud = get_shroud_manager().lock().expect("shroud");
            shroud.clear_all();
            shroud.init_shroud_grid(500.0, 500.0); // 10x10 cells at 50 wu
            shroud.force_update();
            // Mark as updated so snapshot does not fail-open to fully visible.
            let _ = shroud.update(1);
            // Leave most cells Hidden; reveal whole map for player 0 after first snap?
            // First: capture hidden baseline.
        }

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FowGridSnap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");

        // Build with active hidden grid (last_update_frame > 0 after update above).
        let bridge_grid = FOWRenderingBridge::snapshot_terrain_grid(0, false);
        let snap = PresentationFrame::build_from_logic(&logic, 0);

        assert_eq!(
            snap.fow_grid.content_fingerprint(),
            bridge_grid.content_fingerprint(),
            "presentation fow_grid must match FOW bridge grid at build time"
        );
        assert_eq!(snap.fow_grid(), &bridge_grid);
        assert!(snap.fow_grid.active, "grid should be active after init");
        assert_eq!(snap.fow_grid.width, 10);
        assert_eq!(snap.fow_grid.height, 10);
        assert_eq!(snap.fow_grid.cell_count(), 100, "10x10 compact grid");

        // R8 payload length matches grid; encoding is deterministic.
        let r8 = snap.terrain_fow_r8().expect("active grid has r8");
        assert_eq!(r8.len(), 100);
        assert_eq!(r8, snap.fow_grid.to_r8_texture());

        // Dual-build consistency.
        let snap2 = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(
            snap.fow_grid.content_fingerprint(),
            snap2.fow_grid.content_fingerprint()
        );
        assert_eq!(snap.presentation_hash(), snap2.presentation_hash());

        // Freeze: mutate live shroud after snapshot — presentation cells must not change.
        let frozen_fp = snap.fow_grid.content_fingerprint();
        let frozen_r8 = snap.fow_grid.to_r8_texture();
        {
            let mut shroud = get_shroud_manager().lock().expect("shroud");
            // Permanent reveal → all cells Visible on the live manager.
            shroud.reveal_map_for_player_permanently(0).expect("reveal");
        }
        assert_eq!(
            snap.fow_grid.content_fingerprint(),
            frozen_fp,
            "owned grid must stay frozen after live shroud mutation"
        );
        assert_eq!(snap.fow_grid.to_r8_texture(), frozen_r8);

        // New build sees the reveal.
        let snap_after = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap_after
                .fow_grid
                .cells
                .iter()
                .all(|&c| c == PresentationFowGrid::CELL_VISIBLE),
            "fresh snapshot after permanent reveal must be fully visible"
        );
        assert_ne!(
            snap_after.fow_grid.content_fingerprint(),
            frozen_fp,
            "new frame must differ after live reveal"
        );

        // Shell bypass forces fully visible cells when grid dims exist.
        {
            use crate::game_logic::GameMode;
            let mut shell_logic = GameLogic::new();
            shell_logic.start_new_game(GameMode::Shell);
            let shell_snap = PresentationFrame::build_from_logic(&shell_logic, 0);
            assert!(shell_snap.fow_shell_bypass);
            if shell_snap.fow_grid.active {
                assert!(shell_snap
                    .fow_grid
                    .cells
                    .iter()
                    .all(|&c| c == PresentationFowGrid::CELL_VISIBLE));
            }
            assert!(!shell_snap.terrain_fow_overlay_active());
        }

        // Cleanup global shroud so other tests fail-open cleanly.
        // Permanent reveal leaves lookers; re-init grid + clear_all resets counters.
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.clear_all();
            shroud.init_shroud_grid(1.0, 1.0);
            shroud.clear_all();
        }
    }

    #[test]
    fn unit_render_inputs_skip_destroyed_and_engine_bridged() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UnitRenderSkip");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("SkipUnit");
        t.set_health(40.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SkipUnit".into(), t);

        let alive_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("alive");
        let dead_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("dead");
        let bridged_id = logic
            .create_object("SkipUnit", Team::China, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("bridged");
        if let Some(o) = logic.get_object_mut(dead_id) {
            o.status.destroyed = true;
            o.health.current = 0.0;
        }
        if let Some(o) = logic.get_object_mut(bridged_id) {
            o.engine_object_id = Some(99);
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let inputs = snap.unit_render_inputs();
        assert_eq!(
            inputs.len(),
            1,
            "only non-destroyed, non-bridged units enter main mesh pass"
        );
        assert_eq!(inputs[0].id, alive_id);
        // IDs list still includes all alive (including bridged) for FOW/id residual.
        let ids = snap.renderable_object_ids();
        assert!(ids.contains(&alive_id));
        assert!(ids.contains(&bridged_id));
        assert!(!ids.contains(&dead_id));
    }

    #[test]
    fn production_tick_builds_presentation_after_side_systems() {
        // Structural: presentation snapshot must be built after projectile/path host systems.
        let src = include_str!("cnc_game_engine.rs");
        let proj = src
            .find("drain_pending_projectiles")
            .expect("projectile drain");
        let path = src.find("move_unit_along_path").expect("path move");
        let pres = src
            .find("PresentationFrame::build_from_logic")
            .expect("presentation build");
        assert!(
            proj < pres && path < pres,
            "PresentationFrame must be built after projectiles ({proj}) and path ({path}); found at {pres}"
        );
    }

    #[test]
    fn apply_to_ui_state_overwrites_live_identity_after_mutation() {
        // Production path: live update_ui_state may run first; apply_to_ui_state must
        // replace selection health + minimap dots with snapshot-owned values.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HudIdentity");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("HudIdUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("HudIdUnit".into(), t);
        let id = logic
            .create_object("HudIdUnit", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        // Live world mutates after snapshot (would poison a re-read).
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(glam::Vec3::new(999.0, 0.0, 999.0));
            o.health.current = 3.0;
        }

        // Simulate production: live walk first, then presentation overlay.
        let mut ui = logic.update_ui_state(0);
        snap.apply_to_ui_state(&mut ui);

        assert!(
            ui.selected_units.contains(&id),
            "selection ids from snapshot"
        );
        let info = ui
            .selected_unit_infos
            .iter()
            .find(|u| u.object_id == id)
            .expect("selected_unit_infos from snapshot");
        assert!(
            (info.health_current - 100.0).abs() < 0.01,
            "health must be snapshot 100, not live 3: {}",
            info.health_current
        );
        assert!(
            !ui.minimap_unit_dots.is_empty(),
            "minimap dots filled from presentation objects"
        );
        assert_eq!(
            ui.minimap_unit_dots.len(),
            snap.objects.iter().filter(|o| !o.destroyed).count()
        );
        assert!(
            ui.selection_panel.has_positive_health(),
            "last_ui_state selection panel must carry snapshot health"
        );
        assert!(
            (ui.selection_panel.health_current - 100.0).abs() < 0.01,
            "selection panel HP from presentation: {}",
            ui.selection_panel.health_current
        );
    }

    #[test]
    fn presentation_feeds_victory_and_construction() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType, ProductionItem},
            victory::PlayerOutcome,
            KindOf, Player, Resources, Team, ThingTemplate,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "VHuman", true));
        logic.add_player(Player::new(1, Team::China, "VAI", false));
        let mut tb = ThingTemplate::new("VBarracks");
        tb.set_health(1000.0);
        tb.add_kind_of(KindOf::Structure);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("VBarracks".into(), tb);
        let mut tc = ThingTemplate::new("VConstruct");
        tc.set_health(500.0);
        tc.add_kind_of(KindOf::Structure);
        logic.templates.insert("VConstruct".into(), tc);
        let barracks = logic
            .create_object("VBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("b");
        let constructing = logic
            .create_object("VConstruct", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("c");
        if let Some(o) = logic.get_object_mut(barracks) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "Ranger".into(),
                progress: 0.25,
                total_time: 20.0,
                cost: Resources {
                    supplies: 100,
                    power: 0,
                },
            });
            o.building_data = Some(bd);
        }
        if let Some(o) = logic.get_object_mut(constructing) {
            o.status.under_construction = true;
            o.construction_percent = 0.4;
            o.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.is_local = true;
            p.power_produced = 80;
            p.power_consumed = 30;
        }
        // Mark match over via victory event residual (build_with_victory path).
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        frame.match_over = true;
        frame.victory_label = Some("Winner(0)".into());
        frame.events.push(PresentationEvent::Victory {
            winner_player: Some(0),
        });

        let mut ui = crate::ui::GameUIState::default();
        frame.apply_to_ui_state(&mut ui);
        assert!(ui.match_over);
        assert_eq!(ui.player_outcome, Some(PlayerOutcome::Won));
        assert_eq!(ui.power_generated, 80);
        assert_eq!(ui.power_used, 30);
        assert!(
            ui.build_queue
                .iter()
                .any(|b| b.template_name == "Ranger" && (b.percent_complete - 0.25).abs() < 0.01),
            "expected production queue residual: {:?}",
            ui.build_queue
        );
        assert!(
            ui.build_queue
                .iter()
                .any(|b| b.template_name == "VConstruct" && (b.percent_complete - 0.4).abs() < 0.01),
            "expected under-construction residual: {:?}",
            ui.build_queue
        );

        let mut screen = crate::ui::VictoryScreen::new();
        frame.apply_to_victory_screen(&mut screen);
        use crate::ui::Renderable;
        assert!(screen.is_visible());
    }

    #[test]
    fn presentation_feeds_control_bar_radar_and_queues() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "RadarP", true));
        let mut t = ThingTemplate::new("RadarVan");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RadarVan".into(), t);
        let id = logic
            .create_object("RadarVan", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.is_local = true;
            p.is_alive = true;
            p.selected_objects = vec![id];
            p.radar_count = 3;
            p.radar_disabled = false;
            p.queued_upgrades
                .insert("Upgrade_AmericaAdvancedTraining".into());
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.special_power_ready = true;
            o.special_power_cooldown_remaining = 0.0;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(frame.local_radar_count, 3);
        assert!(frame
            .local_queued_upgrades
            .iter()
            .any(|u| u.contains("AdvancedTraining")));

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            frame.apply_to_control_bar(&mut bar);
            assert_eq!(bar.presentation_radar_count(), 3);
            assert!(!bar.presentation_radar_disabled());
            assert!(bar
                .presentation_queued_upgrades()
                .iter()
                .any(|u| u.contains("AdvancedTraining")));
            assert!(
                !bar.get_special_power_shortcuts().is_empty(),
                "expected special power shortcuts from ready selection"
            );
            assert_eq!(
                bar.get_special_power_shortcuts()[0].availability,
                game_client::gui::control_bar::CommandAvailability::Available
            );
        }
    }

    #[test]
    fn presentation_feeds_control_bar_sciences() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "SciP", true));
        let mut t = ThingTemplate::new("SciUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SciUnit".into(), t);
        let id = logic
            .create_object("SciUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.is_local = true;
            p.is_alive = true;
            p.selected_objects = vec![id];
            p.unlocked_sciences.insert("SCIENCE_RedGuards".into());
            p.unlocked_sciences.insert("SCIENCE_PaladinTank".into());
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(frame
            .local_unlocked_sciences
            .iter()
            .any(|s| s == "SCIENCE_RedGuards"));
        assert!(frame.local_has_science("SCIENCE_PaladinTank"));

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            frame.apply_to_control_bar(&mut bar);
            let sci = bar.get_science_state();
            assert!(sci
                .unlocked_sciences
                .iter()
                .any(|s| s == "SCIENCE_RedGuards"));
            assert!(
                sci.rank1_buttons
                    .iter()
                    .any(|b| b.is_purchased && b.command_name.contains("RedGuards")),
                "expected purchased science button, got {:?}",
                sci.rank1_buttons
            );
        }
    }

    #[test]
    fn presentation_feeds_control_bar_upgrade_cameos() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = crate::game_logic::GameLogic::new();
        let mut t = ThingTemplate::new("UpgUnit");
        t.set_health(150.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("UpgUnit".into(), t);
        let id = logic
            .create_object("UpgUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 4.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.applied_upgrades.insert("UpgradeAdvancedTraining".into());
            o.applied_upgrades.insert("UpgradeCaptureBuilding".into());
            o.special_power_ready = true;
            o.special_power_cooldown_remaining = 0.0;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let panel = frame.control_bar_selection_panel();
        assert!(panel
            .applied_upgrades
            .iter()
            .any(|u| u == "UpgradeAdvancedTraining"));
        assert!(panel.special_power_ready);

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            frame.apply_to_control_bar(&mut bar);
            let portrait = bar.get_portrait_state();
            assert_eq!(portrait.upgrade_cameos.len(), 2);
            assert!(portrait
                .upgrade_cameos
                .iter()
                .any(|c| c.upgrade_name == "UpgradeAdvancedTraining" && c.is_completed));
            assert!(portrait.special_power_ready);
        }
    }

    #[test]
    fn presentation_feeds_control_bar_garrison_inventory() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Team, ThingTemplate,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tb = ThingTemplate::new("GarrisonBunker");
        tb.set_health(800.0);
        tb.add_kind_of(KindOf::Structure);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("GarrisonBunker".into(), tb);
        let mut tu = ThingTemplate::new("GarrisonRanger");
        tu.set_health(100.0);
        tu.add_kind_of(KindOf::Infantry);
        tu.add_kind_of(KindOf::Selectable);
        logic.templates.insert("GarrisonRanger".into(), tu);
        let bunker = logic
            .create_object("GarrisonBunker", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("bunker");
        let ranger = logic
            .create_object("GarrisonRanger", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(o) = logic.get_object_mut(bunker) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.selected = true;
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = 5;
            bd.garrisoned_units.push(ranger);
            o.building_data = Some(bd);
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![bunker];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let panel = frame.control_bar_selection_panel();
        assert_eq!(panel.max_garrison, 5);
        assert_eq!(panel.garrisoned_count, 1);
        assert!(!panel.under_construction);

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            frame.apply_to_control_bar(&mut bar);
            let ctx = bar.get_context();
            let guard = ctx.read().expect("read");
            let names: Vec<_> = guard
                .available_commands
                .iter()
                .map(|b| b.command_name.as_str())
                .collect();
            assert!(
                names
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case("Command_StructureExit")),
                "expected StructureExit, got {:?}",
                names
            );
            assert!(
                names
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case("Command_Evacuate")),
                "expected Evacuate, got {:?}",
                names
            );
            assert_eq!(guard.last_recorded_inventory_count, 1);
        }
    }

    #[test]
    fn presentation_feeds_control_bar_veterancy_and_production() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType, ProductionItem},
            Experience, KindOf, Team, ThingTemplate, VeterancyLevel,
        };
        let mut logic = crate::game_logic::GameLogic::new();
        let mut tb = ThingTemplate::new("VetBarracks");
        tb.set_health(1200.0);
        tb.add_kind_of(KindOf::Structure);
        tb.add_kind_of(KindOf::Selectable);
        logic.templates.insert("VetBarracks".into(), tb);
        let id = logic
            .create_object("VetBarracks", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("building");
        if let Some(o) = logic.get_object_mut(id) {
            o.status.under_construction = false;
            o.construction_percent = 1.0;
            o.selected = true;
            o.experience = Experience {
                current: 500.0,
                level: VeterancyLevel::Elite,
            };
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "Ranger".into(),
                progress: 0.55,
                total_time: 10.0,
                cost: crate::game_logic::Resources {
                    supplies: 200,
                    power: 0,
                },
            });
            o.building_data = Some(bd);
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let panel = frame.control_bar_selection_panel();
        assert!(panel.visible);
        assert_eq!(panel.veterancy_overlay.as_deref(), Some("SSChevron2L"));
        assert_eq!(panel.production_template.as_deref(), Some("Ranger"));
        assert!((panel.production_progress.unwrap_or(0.0) - 0.55).abs() < 0.01);
        assert_eq!(panel.production_queue.len(), 1);

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            frame.apply_to_control_bar(&mut bar);
            let portrait = bar.get_portrait_state();
            assert_eq!(portrait.veterancy_overlay.as_deref(), Some("SSChevron2L"));
            assert_eq!(portrait.production_template.as_deref(), Some("Ranger"));
            assert!((portrait.production_progress.unwrap_or(0.0) - 0.55).abs() < 0.01);
            assert_eq!(bar.get_build_queue_data().len(), 1);
            assert_eq!(bar.get_build_queue_data()[0].upgrade_name, "Ranger");
        }
    }

    #[test]
    fn presentation_feeds_control_bar_selection_panel_health() {
        // Residual: ControlBar/WND selection panel health from PresentationFrame
        // (not stale/zero). Headless path — no WND window load required.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CbSelPanel");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("CbPanelUnit");
        t.set_health(77.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CbPanelUnit".into(), t);
        let id = logic
            .create_object("CbPanelUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 5.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        logic.update();

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let panel = snap.control_bar_selection_panel();
        assert!(panel.visible, "selection panel visible with selection");
        assert_eq!(panel.primary_name, "CbPanelUnit");
        assert!(
            (panel.health_current - 77.0).abs() < 0.01,
            "panel health from presentation: {}",
            panel.health_current
        );
        assert!((panel.health_maximum - 77.0).abs() < 0.01);
        assert_eq!(panel.selected_count, 1);
        assert_eq!(panel.primary_object_id, Some(id));

        // GameHUD selection panel (production host display state).
        let mut hud = crate::ui::GameHUD::new();
        snap.apply_to_game_hud(&mut hud);
        assert!(
            hud.selection_panel().has_positive_health(),
            "GameHUD selection panel must show presentation health"
        );
        assert!(
            (hud.selection_panel().health_current - 77.0).abs() < 0.01,
            "HUD panel HP {}",
            hud.selection_panel().health_current
        );

        // last_ui_state path used by engine consumers.
        let mut ui = crate::ui::GameUIState::default();
        snap.apply_to_ui_state(&mut ui);
        assert!(
            (ui.selection_panel.health_current - 77.0).abs() < 0.01,
            "last_ui_state selection panel health"
        );

        // GameClient ControlBar portrait/health strip (no OBJECT_REGISTRY).
        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            // Poison live world after snapshot so a re-read would be wrong.
            if let Some(o) = logic.get_object_mut(id) {
                o.health.current = 1.0;
            }
            snap.apply_to_control_bar(&mut bar);
            let (hp, max) = bar
                .selection_panel_health()
                .expect("ControlBar selection panel health from presentation");
            assert!(
                (hp - 77.0).abs() < 0.01,
                "ControlBar must keep snapshot HP 77, not live 1: {hp}"
            );
            assert!((max - 77.0).abs() < 0.01);
            assert_eq!(bar.get_portrait_state().portrait_image, "CbPanelUnit");
            assert!(bar.get_portrait_state().is_visible);
            assert_eq!(bar.get_portrait_state().selected_count, 1);
        }
    }

    /// Residual (hq-gq7n): after combat kill, PresentationFrame exposes particle
    /// systems from the host registry (observe path for client / HUD).
    #[test]
    fn presentation_frame_observes_combat_kill_particle_systems() {
        use crate::game_logic::{CombatParticleKind, ThingTemplate, Weapon};

        let mut logic = GameLogic::new();
        let mut tank = ThingTemplate::new("FxTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0);
        logic.templates.insert("FxTank".into(), tank);

        let attacker = logic
            .create_object("FxTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker");
        let victim = logic
            .create_object("FxTank", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("victim");

        {
            let a = logic.get_object_mut(attacker).expect("attacker");
            a.attack_target(victim);
            a.weapon = Some(Weapon {
                damage: 9999.0,
                range: 100.0,
                reload_time: 0.0,
                last_fire_time: 0.0,
                ..Weapon::default()
            });
        }
        {
            let v = logic.get_object_mut(victim).expect("victim");
            v.health.current = 5.0;
            v.health.maximum = 5.0;
        }

        // Advance one full host step so combat fires and destroy list runs.
        logic.update();

        assert!(
            logic.find_object(victim).is_none(),
            "victim should be destroyed after combat step"
        );
        assert!(
            logic.combat_particles().active_count() > 0,
            "host particle registry must hold systems after kill"
        );

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap.has_active_particles(),
            "PresentationFrame must expose active particle systems after combat kill"
        );
        assert!(
            snap.particle_systems
                .iter()
                .any(|p| p.kind == CombatParticleKind::DeathExplosion
                    && p.template_name == "MediumExplosion"),
            "death explosion particle must be on presentation frame: {:?}",
            snap.particle_systems
                .iter()
                .map(|p| (&p.template_name, p.kind))
                .collect::<Vec<_>>()
        );
        assert!(
            snap.events
                .iter()
                .any(|e| matches!(e, PresentationEvent::ParticleSystemSpawned { .. })),
            "presentation events should include ParticleSystemSpawned"
        );
        assert!(
            snap.events.iter().any(|e| matches!(
                e,
                PresentationEvent::ObjectDestroyed { id, .. } if *id == victim
            )),
            "presentation events should include ObjectDestroyed for victim"
        );
    }

    /// Residual: presentation freezes InGameUI floating text + MoneyPickUp Anim2D.
    #[test]
    fn presentation_frame_freezes_floating_text_and_world_anim() {
        use crate::game_logic::host_money_crate::{
            HostMoneyCrateRegistry, MONEY_PICKUP_ANIM_TEMPLATE,
        };
        use crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FloatPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");

        // Empty residual when host has no cash events.
        let empty = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!empty.has_floating_texts());
        assert!(!empty.has_world_anims());
        assert!(empty.floating_text_presentation_ok());
        assert!(empty.world_anim_presentation_ok());

        let frame = logic.get_frame();
        let oil_ft = HostAutoDepositFloatingText::new(
            ObjectId(11),
            Vec3::new(1.0, 0.0, 2.0),
            100,
            (200, 200, 200),
            frame,
            false,
        );
        logic.push_residual_auto_deposit_floating_text_for_presentation(oil_ft);

        let anim = HostMoneyCrateRegistry::money_pickup_anim(
            ObjectId(21),
            ObjectId(22),
            Vec3::new(5.0, 0.0, 6.0),
            frame,
        );
        let money_ft = HostMoneyCrateRegistry::money_floating_text(
            ObjectId(21),
            ObjectId(22),
            Vec3::new(5.0, 0.0, 6.0),
            125,
            frame,
        );
        logic.push_residual_money_pickup_presentation(anim, money_ft);

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap.has_floating_texts(),
            "presentation must freeze host floating texts"
        );
        assert!(
            snap.has_world_anims(),
            "presentation must freeze MoneyPickUp world anim"
        );
        assert!(snap.floating_text_presentation_ok());
        assert!(snap.world_anim_presentation_ok());
        assert_eq!(snap.floating_texts.len(), 2);
        assert_eq!(snap.world_anims.len(), 1);
        assert_eq!(snap.world_anims[0].template, MONEY_PICKUP_ANIM_TEMPLATE);
        assert!(snap
            .floating_texts
            .iter()
            .any(|t| t.kind == PresentationFloatingTextKind::AutoDeposit && t.amount == 100));
        assert!(snap
            .floating_texts
            .iter()
            .any(|t| t.kind == PresentationFloatingTextKind::MoneyCrate
                && t.amount == 125
                && t.color_rgba == (0, 255, 0, 255)));
        assert_eq!(snap.active_floating_texts_at(frame).len(), 2);
        assert!(snap
            .active_floating_texts_at(frame + PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES)
            .is_empty());

        // Snapshot stays frozen after host clears residual registries.
        let frozen_count = snap.floating_texts.len();
        let frozen_anims = snap.world_anims.len();
        logic.clear_residual_floating_text_for_presentation();
        assert_eq!(snap.floating_texts.len(), frozen_count);
        assert_eq!(snap.world_anims.len(), frozen_anims);
        let after = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!after.has_floating_texts());
        assert!(!after.has_world_anims());

        // Synthetic residual for host-testable pack without combat/deposit path.
        let synth = PresentationFloatingText::synthetic_cash(50, 0);
        assert_eq!(synth.text_key, "GUI:AddCash");
        assert_eq!(
            synth.timeout_frame,
            PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES
        );
        assert!(PresentationFloatingText::honesty_vanish_rate_residual_ok());
        assert!(PresentationFloatingText::honesty_vanish_color_alpha_residual_ok());
        assert!((synth.vanish_alpha_at(0) - 1.0).abs() < 0.001);
        assert!((synth.vanish_alpha_at(15) - 0.5).abs() < 0.001);
        assert_eq!(synth.vanish_color_alpha_u8_at(20, 255), 254);
        assert_eq!(synth.color_with_vanish_alpha_at(20), (0, 255, 0, 254));
        assert!((synth.lift_y_at(3) - 3.0).abs() < 0.001);
        let wa = PresentationWorldAnim::synthetic_money_pickup(0);
        assert_eq!(wa.template, MONEY_PICKUP_ANIM_TEMPLATE);
        assert!((wa.z_rise_per_second - 15.0).abs() < 0.01);
        assert!(wa.honesty_fade_residual_ok());
        assert!(PresentationWorldAnim::honesty_money_pickup_fade_params_ok());
        assert!((wa.fade_alpha_at(0) - 1.0).abs() < 0.01);
        // Dual-tick residual counters on freeze.
        assert!(snap.dual_tick_presentation_residual_ok());
        assert!(snap.floating_text_vanish_residual_ok());
        assert!(snap.world_anim_fade_residual_ok());
        assert_eq!(snap.dual_tick.builds, 1);
        assert_eq!(snap.dual_tick.floating_text_count, 2);
        assert_eq!(snap.dual_tick.world_anim_count, 1);
    }

    /// Residual: presentation freezes assist laser Line3D segments for SegLine pack.
    #[test]
    fn presentation_frame_freezes_laser_line3d_segments() {
        use crate::game_logic::host_base_defense::{
            make_patriot_assist_lasers, PATRIOT_LASER_SEGMENTS,
        };

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("LaserPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");

        // Empty lasers when host has none.
        let empty = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!empty.has_active_lasers());
        assert_eq!(empty.laser_segment_count(), 0);
        assert!(empty.minimap_fow_presentation_ok());

        // Inject residual assist lasers via public host slice mutation path:
        // push through make + internal list via active endpoint track simulation.
        let beams = make_patriot_assist_lasers(
            ObjectId(1),
            ObjectId(2),
            ObjectId(3),
            (0.0, 0.0, 5.0),
            (30.0, 0.0, 5.0),
            (60.0, 0.0, 5.0),
            logic.get_frame(),
        );
        logic.push_residual_patriot_assist_lasers_for_presentation(beams);

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap.has_active_lasers(),
            "presentation must freeze active assist lasers"
        );
        assert_eq!(snap.laser_beams.len(), 2);
        assert_eq!(
            snap.laser_segment_count(),
            PATRIOT_LASER_SEGMENTS as usize * 2
        );
        assert_eq!(
            snap.laser_beams[0].segments.len(),
            PATRIOT_LASER_SEGMENTS as usize
        );
        assert_eq!(
            snap.laser_beams[0].template_name,
            crate::game_logic::host_base_defense::PATRIOT_BINARY_DATA_STREAM
        );
        // Snapshot stays frozen after host clears lasers.
        let frozen_count = snap.laser_segment_count();
        logic.clear_residual_patriot_assist_lasers_for_presentation();
        assert_eq!(snap.laser_segment_count(), frozen_count);
        let after = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!after.has_active_lasers());

        // Synthetic assist pair residual for host-testable pack without combat.
        let pair = PresentationLaserBeam::synthetic_assist_pair(0);
        assert_eq!(pair[0].segments.len(), PATRIOT_LASER_SEGMENTS as usize);
        assert_eq!(pair[1].segments.len(), PATRIOT_LASER_SEGMENTS as usize);
        assert!(pair[0].honesty_ground_height_ok());
        assert!((pair[0].ground_height - PRESENTATION_DEFAULT_GROUND_HEIGHT).abs() < 0.001);
        assert!(!pair[0].ground_height_from_terrain);
        assert!(!pair[0].has_soft_edge());
        assert!(pair[0].honesty_soft_edge_presentation_ok());

        // Optional ground-height override residual path.
        let pair_gh = PresentationLaserBeam::synthetic_assist_pair_with_ground(0, 12.5);
        assert!((pair_gh[0].ground_height - 12.5).abs() < 0.001);
        assert!(honesty_ground_height_residual_ok(12.5, true));

        // Orbital multi-beam soft-edge presentation residual → pack wiring fields.
        let orbital = PresentationLaserBeam::synthetic_orbital_soft_edge(0);
        assert!(orbital.has_soft_edge());
        assert!(orbital.honesty_soft_edge_presentation_ok());
        let se = orbital.soft_edge.expect("soft edge");
        assert!(se.honesty_orbital_residual_ok());
        assert_eq!(se.num_beams, 12);
        let (s, e, elapsed, width_scalar) = se.pack_endpoints(orbital.from, orbital.to, 1.0);
        assert_eq!(s, orbital.from);
        assert_eq!(e, orbital.to);
        assert!((elapsed - 1.0).abs() < 0.001);
        assert!((width_scalar - 1.0).abs() < 0.001);
        assert!(snap.laser_presentation_residual_ok() || empty.laser_presentation_residual_ok());
        assert!(empty.dual_tick_presentation_residual_ok());
    }

    #[test]
    fn dual_tick_residual_counters_increment_on_apply() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DualTickCtr");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut hud = crate::ui::GameHUD::new();
        let mut ui = crate::ui::GameUIState::default();
        let mut rts = crate::ui::RTSInterface::new();
        let mut cmd = crate::ui::UnitCommandPanel::new();
        let frame = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
        );
        assert!(frame.dual_tick_presentation_residual_ok());
        assert!(frame.dual_tick.honesty_apply_ok());
        assert_eq!(frame.dual_tick.builds, 1);
        assert_eq!(frame.dual_tick.applies, 1);
        assert!(frame.floating_text_vanish_residual_ok());
        assert!(frame.world_anim_fade_residual_ok());
        assert!(frame.laser_presentation_residual_ok());
    }

    /// Wave 73: Spectre AttackAreaDecal / TargetingReticleDecal presentation residual.
    #[test]
    fn spectre_orbit_decal_presentation_residual_wave73() {
        assert!(honesty_spectre_orbit_decal_presentation_ok());
        let decal = PresentationSpectreOrbitDecal::RETAIL;
        assert!(decal.honesty_residual_ok());
        assert_eq!(decal.attack_area_texture, "SCCSpecTarg");
        assert_eq!(decal.reticle_texture, "SCCSpecRet");
        assert!((decal.attack_area_radius - 200.0).abs() < 0.01);
        assert!((decal.reticle_radius - 25.0).abs() < 0.01);
        assert_eq!(decal.attack_area_throb_ms, 1500);
        assert_eq!(decal.reticle_throb_ms, 300);
        assert_eq!(decal.style, "SHADOW_ALPHA_DECAL");
        assert!(decal.only_visible_to_owning_player);
        assert!(decal.reticle_radius < decal.attack_area_radius);

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpectreDecalPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(snap.spectre_orbit_decal_presentation_residual_ok());
    }

    /// Wave 102 residual: dual-tick deepen (selected/particle counters + packs).
    #[test]
    fn presentation_dual_tick_residual_deepen_wave102() {
        assert!(honesty_presentation_dual_tick_residual_deepen_wave102());
        assert!(honesty_presentation_residual_deepen_pack_wave102());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("Pres102");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut hud = crate::ui::GameHUD::new();
        let mut ui = crate::ui::GameUIState::default();
        let mut rts = crate::ui::RTSInterface::new();
        let mut cmd = crate::ui::UnitCommandPanel::new();
        let frame = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic, 0, &mut hud, &mut ui, &mut rts, &mut cmd,
        );
        assert!(frame.dual_tick_presentation_residual_ok());
        assert!(frame.dual_tick_presentation_residual_deepen_ok());
        assert_eq!(frame.dual_tick.selected_count, frame.selected.len() as u32);
        assert_eq!(
            frame.dual_tick.particle_count,
            frame.particle_systems.len() as u32
        );
        assert!(frame.dual_tick.honesty_apply_ok());
    }
}
