//! Host mine / demo-trap / demo-charge residual.
//!
//! Residual slice (playability):
//! - Place land mines (ChinaStandardMine / ClusterMines special power residual)
//! - Place GLA demo traps (proximity detonation when enemies enter range)
//!   - Standard: `DemoTrapDetonationWeapon` Primary **600**/r**25** + Secondary **400**/r**50**
//!   - Chem Beta: `Chem_DemoTrapDetonationWeaponBeta` Primary **250**/r**25** +
//!     Secondary **100**/r**50** + MediumPoisonFieldUpgraded residual
//!   - Chem Gamma: `Chem_DemoTrapDetonationWeaponGamma` same rings + gamma poison field
//!   - Demo: `Demo_DemoTrapDetonationWeapon` Primary **700**/r**25** + Secondary **500**/r**50**
//! - Place timed demo charges (Burton / Tank Hunter sticky residual)
//! - Place remote demo charges (Burton SPECIAL_REMOTE_CHARGES sticky residual)
//! - Detonate remote demo charges on command (no auto-timer)
//! - Enemy/neutral proximity trigger → area damage + destroy mine/trap
//! - Timed charges detonate at absolute frame
//! - Dozer / Worker mine-clear: approach enemy/neutral mine → disarm without detonation
//!
//! - **DemoTrapUpdate weapon-slot mode residual**:
//!   - DefaultProximityMode **Yes** → starts in Proximity (SECONDARY residual)
//!   - Manual mode (TERTIARY residual) disables proximity scan
//!   - Detonation slot (PRIMARY residual) → manual detonate residual
//!   - TriggerDetonationRange **40**, DetonateWhenKilled **Yes**,
//!     AutoDetonationWithFriendsInvolved **Yes**, DestructionDelay **1000**ms → **30**f
//!
//! Wave 51 residual pack (retail INI honesty):
//! - DozerMineDisarmingWeapon: AttackRange **5**, PreAttackDelay **1200**ms → **36**f,
//!   ClipReloadTime **4000**ms → **120**f, ContinueAttackRange **100**
//! - WorkerMineDisarmingWeapon: AttackRange **5**, PreAttackDelay **1000**ms → **30**f,
//!   DelayBetweenShots **1000**ms → **30**f, ContinueAttackRange **100**
//! - Burton MaxSpecialObjects: RemoteC4 **8**, TimedC4 **10**, Unique targets **Yes**,
//!   UnpackTime **5500**ms → **165**f, FleeRangeAfterCompletion **100**
//! - SUPERWEAPON_ClusterMines OCL: DropVariance **X:20 Y:20 Z:0**, DeliveryDistance **140**,
//!   DeliveryDecalRadius / RadiusCursorRadius **100**, DistanceAroundObject **80**,
//!   NumVirtualMines **8**, payload ClusterMinesBomb → ChinaClusterMine
//!
//! - C++ MinefieldBehavior virtual-mine persist / immunity slots / worker skip
//! - C++ StickyBombUpdate plant-XY freeze + UnitBombPing
//! - C++ GenerateMinefieldBehavior SmartBorder / EMP swap
//! - RemoteC4Charge / TimedC4Charge SpecialObject names + MaxSpecialObjects residual closed
//! - Not full StickyBombUpdate attach bones / geometry-based splash / live max-charge list UI
//! - Not full OCL ClusterMinesBomb aircraft path

use super::ObjectId;
use crate::game_logic::host_toxin_tractor::{AnthraxResidualTier, is_chem_general_template};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const MINE_LOGIC_FPS: f32 = 30.0;

/// C++ MinefieldBehavior RepeatDetonateMoveThresh default.
pub const REPEAT_DETONATE_MOVE_THRESH: f32 = 1.0;
/// C++ MinefieldBehavior MAX_IMMUNITY.
pub const MINE_MAX_IMMUNITY: usize = 3;
/// C++ immunity expires when now > collideTime + 2.
pub const MINE_IMMUNITY_HOLD_FRAMES: u32 = 2;
/// C++ detonateOnce never drains health below this.
pub const MINE_MIN_HEALTH: f32 = 0.1;
/// C++ CreatorDeathCheckRate default (LOGICFRAMES_PER_SECOND).
pub const MINE_CREATOR_DEATH_CHECK_FRAMES: u32 = 30;
/// Retail ChinaStandardMine DegenPercentPerSecondAfterCreatorDies **100%**.
pub const MINE_DEGEN_PERCENT_PER_SECOND: f32 = 1.0;
/// China mine AutoHeal HealingAmount residual (StartsActive Yes).
pub const MINE_AUTO_HEAL_AMOUNT: f32 = 2.0;
/// China mine AutoHeal HealingDelay 1000ms → 30f.
pub const MINE_AUTO_HEAL_DELAY_FRAMES: u32 = 30;

/// Retail ChinaStandardMine / ChinaEMPMine NumVirtualMines residual.
pub const STANDARD_MINE_NUM_VIRTUAL: u32 = 8;
/// ChinaStandardMine / ChinaEMPMine / ChinaClusterMine GeometryMajorRadius residual.
pub const LAND_MINE_GEOMETRY_RADIUS: f32 = 30.0;
/// Retail ChinaStandardMine / ChinaClusterMine / ChinaEMPMine ScootFromStartingPointTime **1000**ms.
pub const MINE_SCOOT_FROM_STARTING_POINT_MS: u32 = 1_000;
/// 1000ms → 30f @ 30 FPS (`INI::parseDurationUnsignedInt`).
pub const MINE_SCOOT_FROM_STARTING_POINT_FRAMES: u32 = 30;
/// Leftover `MinefieldBehavior::set_scoot_parms` / C++ `GlobalData` default gravity.
pub const MINE_SCOOT_GRAVITY: f32 = -1.0;

/// NeutronMineWeapon PrimaryDamage residual (ChinaEMPMine detonation).
pub const NEUTRON_MINE_WEAPON_DAMAGE: f32 = 1.0;
/// NeutronBlastBehavior BlastRadius residual (ChinaEMPMine NeutronBlastObject).
pub const NEUTRON_MINE_BLAST_RADIUS: f32 = 20.0;
/// NeutronBlastObject AffectAirborne residual.
pub const NEUTRON_MINE_AFFECT_AIRBORNE: bool = false;
/// NeutronBlastObject AffectAllies residual.
pub const NEUTRON_MINE_AFFECT_ALLIES: bool = false;
/// C++ GenerateMinefieldBehavior SkipIfThisMuchUnderStructure default.
pub const SKIP_IF_THIS_MUCH_UNDER_STRUCTURE: f32 = 0.33;
/// C++ GlobalData m_standardMinefieldDistance.
pub const STANDARD_MINEFIELD_DISTANCE: f32 = 40.0;
/// C++ StickyBombUpdate per-unit create sound.
pub const STICKY_BOMB_CREATED_AUDIO: &str = "StickyBombCreated";
/// C++ StickyBombUpdate per-unit UnitBombPing.
pub const UNIT_BOMB_PING_AUDIO: &str = "UnitBombPing";
/// C++ LOGICFRAMES_PER_SECOND ping interval.
pub const UNIT_BOMB_PING_FRAMES: u32 = 30;
/// Retail StickyBombUpdate OffsetZ residual (ride on vehicle roof).
pub const STICKY_OFFSET_Z: f32 = 8.0;

/// DemoTrapUpdate weapon-slot mode residual.
///
/// Retail slots:
/// - `DetonationWeaponSlot = PRIMARY` → detonate now
/// - `ProximityModeWeaponSlot = SECONDARY` → proximity scan on
/// - `ManualModeWeaponSlot = TERTIARY` → proximity scan off
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DemoTrapMode {
    /// ProximityModeWeaponSlot residual (DefaultProximityMode Yes).
    Proximity,
    /// ManualModeWeaponSlot residual — wait for manual detonate / detonation slot.
    Manual,
    /// DetonationWeaponSlot residual — command to detonate immediately.
    Detonate,
}

impl DemoTrapMode {
    pub fn proximity_enabled(self) -> bool {
        matches!(self, DemoTrapMode::Proximity)
    }

    pub fn is_detonate_command(self) -> bool {
        matches!(self, DemoTrapMode::Detonate)
    }
}

/// Host residual mine/trap kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostMineKind {
    /// Collision / proximity land mine (ChinaStandardMine / ChinaClusterMine).
    LandMine,
    /// GLA demo trap (DemoTrapUpdate proximity residual).
    DemoTrap,
    /// Timed demo charge (TNTStickyBomb / Burton timed residual).
    TimedDemoCharge,
    /// Remote demo charge (Burton SPECIAL_REMOTE_CHARGES sticky residual).
    /// No auto-timer — detonates only via DetonateRemoteDemoCharges command.
    RemoteDemoCharge,
}

impl HostMineKind {
    pub fn label(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "LandMine",
            HostMineKind::DemoTrap => "DemoTrap",
            HostMineKind::TimedDemoCharge => "TimedDemoCharge",
            HostMineKind::RemoteDemoCharge => "RemoteDemoCharge",
        }
    }

    /// Retail-inspired residual defaults (Weapon.ini / Object INI).
    pub fn default_trigger_range(self) -> f32 {
        match self {
            // ChinaStandardMine GeometryMajorRadius residual for onCollide trip.
            HostMineKind::LandMine => LAND_MINE_GEOMETRY_RADIUS,
            // GLADemoTrap DemoTrapUpdate TriggerDetonationRange = 40.
            HostMineKind::DemoTrap => 40.0,
            // Timed / remote charges do not proximity-trigger by default.
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge => 0.0,
        }
    }

    pub fn default_damage(self) -> f32 {
        match self {
            // StructureMineWeapon PrimaryDamage residual.
            HostMineKind::LandMine => 100.0,
            // DemoTrapDetonationWeapon PrimaryDamage residual.
            HostMineKind::DemoTrap => 600.0,
            // TNTDetonationWeapon PrimaryDamage residual (timed + remote).
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge => 500.0,
        }
    }

    pub fn default_damage_radius(self) -> f32 {
        match self {
            // StructureMineWeapon secondary radius residual.
            HostMineKind::LandMine => 5.0,
            // DemoTrapDetonationWeapon primary radius residual.
            HostMineKind::DemoTrap => 25.0,
            // TNTDetonationWeapon secondary radius residual (observable splash).
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge => 50.0,
        }
    }

    /// Default timed lifetime frames (only TimedDemoCharge uses this).
    pub fn default_lifetime_frames(self) -> Option<u32> {
        match self {
            // TNTStickyBomb LifetimeUpdate Min/MaxLifetime = 10000 ms @ 30 FPS.
            HostMineKind::TimedDemoCharge => Some(300),
            HostMineKind::LandMine | HostMineKind::DemoTrap | HostMineKind::RemoteDemoCharge => {
                None
            }
        }
    }

    pub fn defaults_to_proximity(self) -> bool {
        match self {
            HostMineKind::LandMine | HostMineKind::DemoTrap => true,
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge => false,
        }
    }

    pub fn place_audio(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "MineFieldPlaced",
            HostMineKind::DemoTrap => "DemoTrapPlaced",
            HostMineKind::TimedDemoCharge => "ColonelBurtonSetDemoCharge",
            HostMineKind::RemoteDemoCharge => "ColonelBurtonSetRemoteCharge",
        }
    }

    pub fn detonate_audio(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "ExplosionClusterMine",
            HostMineKind::DemoTrap => "DemoTrapExplosion",
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge => {
                "RemoteDemoChargeExplosion"
            }
        }
    }
}

/// Host residual Demo Trap detonation profile (Weapon.ini variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DemoTrapProfile {
    /// Standard DemoTrapDetonationWeapon residual.
    #[default]
    Standard,
    /// Chem_DemoTrapDetonationWeaponBeta residual.
    ChemBeta,
    /// Chem_DemoTrapDetonationWeaponGamma residual.
    ChemGamma,
    /// Demo_DemoTrapDetonationWeapon residual.
    Demo,
}

impl DemoTrapProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::ChemBeta => "ChemBeta",
            Self::ChemGamma => "ChemGamma",
            Self::Demo => "Demo",
        }
    }

    pub fn primary_damage(self) -> f32 {
        match self {
            Self::Standard => 600.0,
            Self::ChemBeta | Self::ChemGamma => 250.0,
            Self::Demo => 700.0,
        }
    }

    pub fn primary_radius(self) -> f32 {
        25.0
    }

    pub fn secondary_damage(self) -> f32 {
        match self {
            Self::Standard => 400.0,
            Self::ChemBeta | Self::ChemGamma => 100.0,
            Self::Demo => 500.0,
        }
    }

    pub fn secondary_radius(self) -> f32 {
        50.0
    }

    /// Weapon.ini Primary+Secondary step rings (Standard / Chem / Demo).
    pub fn uses_dual_ring(self) -> bool {
        true
    }

    pub fn spawns_poison(self) -> bool {
        matches!(self, Self::ChemBeta | Self::ChemGamma)
    }

    pub fn poison_anthrax_tier(self) -> AnthraxResidualTier {
        match self {
            Self::ChemGamma => AnthraxResidualTier::Gamma,
            Self::ChemBeta => AnthraxResidualTier::Beta,
            Self::Standard | Self::Demo => AnthraxResidualTier::None,
        }
    }
}

/// Whether template is Demo General residual (Demo_ prefix).
///
/// Fail-closed: does **not** match bare `TestDemoTrap` (standard residual test name).
pub fn is_demo_general_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("demo_")
        || n == "testdemogeneraldemotrap"
        || n == "testdemo_demotrap"
        || n.contains("demo_glademotrap")
}

/// Resolve demo trap detonation profile from template + anthrax flags.
pub fn demo_trap_profile(template_name: &str, has_gamma: bool, has_beta: bool) -> DemoTrapProfile {
    if is_demo_general_template(template_name) {
        return DemoTrapProfile::Demo;
    }
    let n = template_name.to_ascii_lowercase();
    if is_chem_general_template(template_name)
        || n.contains("chem_demotrap")
        || n.contains("chemdemotrap")
        || n == "testchemdemotrap"
        || has_gamma
        || has_beta
    {
        if has_gamma {
            DemoTrapProfile::ChemGamma
        } else {
            DemoTrapProfile::ChemBeta
        }
    } else {
        DemoTrapProfile::Standard
    }
}

/// Dual-ring residual damage for Standard/Chem/Demo demo traps.
pub fn demo_trap_damage_at(profile: DemoTrapProfile, distance: f32) -> f32 {
    if distance <= profile.primary_radius() {
        profile.primary_damage()
    } else if distance <= profile.secondary_radius() {
        profile.secondary_damage()
    } else {
        0.0
    }
}

/// C++ MinefieldBehavior DetonatorInfo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HostMineDetonator {
    pub id: ObjectId,
    pub x: f32,
    pub z: f32,
}

/// C++ MinefieldBehavior ImmuneInfo (MAX_IMMUNITY 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMineImmune {
    pub id: ObjectId,
    pub collide_frame: u32,
}

/// Per-object host residual mine/trap state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMineData {
    pub kind: HostMineKind,
    /// Proximity scan radius (0 = disabled).
    pub trigger_range: f32,
    pub detonation_damage: f32,
    pub detonation_radius: f32,
    /// Optional secondary ring residual (Chem/Demo demo trap variants).
    #[serde(default)]
    pub secondary_damage: f32,
    #[serde(default)]
    pub secondary_radius: f32,
    /// Demo trap death-weapon profile residual (Standard default).
    #[serde(default)]
    pub demo_trap_profile: DemoTrapProfile,
    /// When true, enemies in trigger_range detonate (DemoTrap / LandMine).
    pub proximity_enabled: bool,
    /// DemoTrapUpdate weapon-slot mode residual (Proximity / Manual).
    /// Land mines ignore this (always proximity when enabled).
    #[serde(default = "default_demo_trap_mode")]
    pub demo_trap_mode: DemoTrapMode,
    pub detonated: bool,
    /// Absolute logic frame for timed detonation (TimedDemoCharge).
    pub detonate_at_frame: Option<u32>,
    /// Optional sticky target (fail-closed residual bookkeeping).
    pub attached_to: Option<ObjectId>,
    /// Source that placed this residual (producer).
    pub producer_id: Option<ObjectId>,
    /// C++ MinefieldBehavior m_numVirtualMines.
    #[serde(default = "default_virtual_mines_one")]
    pub num_virtual_mines: u32,
    /// C++ MinefieldBehavior m_virtualMinesRemaining.
    #[serde(default = "default_virtual_mines_one")]
    pub virtual_mines_remaining: u32,
    /// C++ MinefieldBehaviorModuleData m_regenerates.
    #[serde(default)]
    pub regenerates: bool,
    /// C++ WorkersDetonate (module default false).
    #[serde(default)]
    pub workers_detonate: bool,
    /// C++ RepeatDetonateMoveThresh (default 1).
    #[serde(default = "default_repeat_detonate_thresh")]
    pub repeat_detonate_move_thresh: f32,
    /// C++ m_detonators — last trip XZ per victim.
    #[serde(default)]
    pub detonators: Vec<HostMineDetonator>,
    /// C++ MAX_IMMUNITY 3 collide slots.
    #[serde(default)]
    pub immunes: Vec<HostMineImmune>,
    /// C++ StickyBombUpdate m_nextPingFrame.
    #[serde(default)]
    pub next_ping_frame: Option<u32>,
    /// C++ MinefieldBehavior m_draining (producer died).
    #[serde(default)]
    pub draining: bool,
    /// C++ MinefieldBehavior m_ignoreDamage (health clamp must not retrigger).
    #[serde(default)]
    pub ignore_damage: bool,
    /// C++ StopsRegenAfterCreatorDies (module default true).
    #[serde(default = "default_stops_regen_after_creator_dies")]
    pub stops_regen_after_creator_dies: bool,
    /// C++ AutoHealBehavior::stopHealing after producer death.
    #[serde(default)]
    pub auto_heal_stopped: bool,
    /// Next AutoHeal pulse frame for regenerating pads.
    #[serde(default)]
    pub auto_heal_wake_frame: u32,
    /// C++ m_nextDeathCheckFrame.
    #[serde(default)]
    pub next_death_check_frame: u32,
    /// Last health fed to onDamage virtual sync (None = never synced).
    #[serde(default)]
    pub last_synced_health: Option<f32>,
    /// Dozer/worker currently winding PreAttackDelay against this pad.
    #[serde(default)]
    pub clear_pre_attack_clearer: Option<ObjectId>,
    /// Frame when that clearer may DISARM.
    #[serde(default)]
    pub clear_pre_attack_ready_frame: u32,
    /// ChinaEMPMine NeutronBlastObject residual (infantry die / vehicles unmanned).
    #[serde(default)]
    pub neutron_blast: bool,
    /// DemoTrap SlowDeath warning fuse (absolute frame when splash may fire).
    #[serde(default)]
    pub warning_detonate_at_frame: Option<u32>,
    /// C++ MinefieldBehaviorModuleData m_scootFromStartingPointTime (frames).
    #[serde(default)]
    pub scoot_from_starting_point_time: u32,
    /// C++ MinefieldBehavior m_scootFramesLeft.
    #[serde(default)]
    pub scoot_frames_left: u32,
    /// C++ MinefieldBehavior m_scootVel (host Y-up).
    #[serde(default)]
    pub scoot_vel: [f32; 3],
    /// C++ MinefieldBehavior m_scootAccel (host Y-up).
    #[serde(default)]
    pub scoot_accel: [f32; 3],
}

fn default_demo_trap_mode() -> DemoTrapMode {
    DemoTrapMode::Proximity
}

fn default_virtual_mines_one() -> u32 {
    1
}

fn default_repeat_detonate_thresh() -> f32 {
    REPEAT_DETONATE_MOVE_THRESH
}

fn default_stops_regen_after_creator_dies() -> bool {
    true
}

impl Default for HostMineData {
    fn default() -> Self {
        Self::new(HostMineKind::LandMine)
    }
}

impl HostMineData {
    pub fn new(kind: HostMineKind) -> Self {
        Self {
            kind,
            trigger_range: kind.default_trigger_range(),
            detonation_damage: kind.default_damage(),
            detonation_radius: kind.default_damage_radius(),
            secondary_damage: 0.0,
            secondary_radius: 0.0,
            demo_trap_profile: DemoTrapProfile::Standard,
            proximity_enabled: kind.defaults_to_proximity(),
            // DefaultProximityMode Yes residual for DemoTrap; other kinds ignore.
            demo_trap_mode: if matches!(kind, HostMineKind::DemoTrap) {
                DemoTrapMode::Proximity
            } else {
                DemoTrapMode::Manual
            },
            detonated: false,
            detonate_at_frame: None,
            attached_to: None,
            producer_id: None,
            num_virtual_mines: 1,
            virtual_mines_remaining: 1,
            regenerates: false,
            workers_detonate: false,
            repeat_detonate_move_thresh: REPEAT_DETONATE_MOVE_THRESH,
            detonators: Vec::new(),
            immunes: Vec::new(),
            draining: false,
            ignore_damage: false,
            stops_regen_after_creator_dies: true,
            auto_heal_stopped: false,
            auto_heal_wake_frame: 0,
            next_death_check_frame: 0,
            last_synced_health: None,
            clear_pre_attack_clearer: None,
            clear_pre_attack_ready_frame: 0,
            next_ping_frame: None,
            neutron_blast: false,
            warning_detonate_at_frame: None,
            scoot_from_starting_point_time: 0,
            scoot_frames_left: 0,
            scoot_vel: [0.0; 3],
            scoot_accel: [0.0; 3],
        }
    }

    /// Apply DemoTrapUpdate weapon-slot mode residual.
    ///
    /// Returns true if mode changed (or detonate was selected). Detonate does not
    /// persist as mode — caller should fire manual detonate residual.
    pub fn set_demo_trap_mode(&mut self, mode: DemoTrapMode) -> bool {
        if !matches!(self.kind, HostMineKind::DemoTrap) || self.detonated {
            return false;
        }
        if mode.is_detonate_command() {
            // Detonation slot residual: leave mode as Manual (proximity off) and
            // signal caller to detonate.
            self.demo_trap_mode = DemoTrapMode::Manual;
            self.proximity_enabled = false;
            return true;
        }
        self.demo_trap_mode = mode;
        self.proximity_enabled = mode.proximity_enabled();
        true
    }

    pub fn land_mine() -> Self {
        Self::new(HostMineKind::LandMine)
    }

    /// Bind virtual-mine / regen residual from the mine template name.
    pub fn land_mine_for_template(template_name: &str) -> Self {
        let mut data = Self::land_mine();
        let n = template_name.to_ascii_lowercase();
        if n.contains("cluster") {
            data.num_virtual_mines = CLUSTER_MINE_NUM_VIRTUAL;
            data.virtual_mines_remaining = CLUSTER_MINE_NUM_VIRTUAL;
            data.regenerates = false;
            data.scoot_from_starting_point_time = MINE_SCOOT_FROM_STARTING_POINT_FRAMES;
        } else if n.contains("empmine") {
            data.num_virtual_mines = STANDARD_MINE_NUM_VIRTUAL;
            data.virtual_mines_remaining = STANDARD_MINE_NUM_VIRTUAL;
            data.regenerates = true;
            data.neutron_blast = true;
            data.detonation_damage = NEUTRON_MINE_WEAPON_DAMAGE;
            data.detonation_radius = NEUTRON_MINE_BLAST_RADIUS;
            data.scoot_from_starting_point_time = MINE_SCOOT_FROM_STARTING_POINT_FRAMES;
        } else if n.contains("standardmine") {
            data.num_virtual_mines = STANDARD_MINE_NUM_VIRTUAL;
            data.virtual_mines_remaining = STANDARD_MINE_NUM_VIRTUAL;
            data.regenerates = true;
            data.scoot_from_starting_point_time = MINE_SCOOT_FROM_STARTING_POINT_FRAMES;
        }

        data
    }

    pub fn demo_trap() -> Self {
        Self::demo_trap_with_profile(DemoTrapProfile::Standard)
    }

    /// Demo trap residual with Chem/Demo/Standard detonation profile.
    pub fn demo_trap_with_profile(profile: DemoTrapProfile) -> Self {
        let mut data = Self::new(HostMineKind::DemoTrap);
        data.demo_trap_profile = profile;
        data.detonation_damage = profile.primary_damage();
        data.detonation_radius = if profile.uses_dual_ring() {
            profile.secondary_radius()
        } else {
            profile.primary_radius()
        };
        if profile.uses_dual_ring() {
            data.secondary_damage = profile.secondary_damage();
            data.secondary_radius = profile.secondary_radius();
        }
        data
    }

    pub fn timed_demo_charge(current_frame: u32) -> Self {
        let mut data = Self::new(HostMineKind::TimedDemoCharge);
        let delay = HostMineKind::TimedDemoCharge
            .default_lifetime_frames()
            .unwrap_or(300);
        data.detonate_at_frame = Some(current_frame.saturating_add(delay));
        data.next_ping_frame = Some(sticky_next_ping_frame(
            current_frame,
            data.detonate_at_frame,
        ));
        data
    }

    /// Remote demo charge: sticky until DetonateRemoteDemoCharges (no auto-timer).
    pub fn remote_demo_charge() -> Self {
        let mut data = Self::new(HostMineKind::RemoteDemoCharge);
        data.next_ping_frame = Some(UNIT_BOMB_PING_FRAMES);
        data
    }

    pub fn with_producer(mut self, producer: ObjectId) -> Self {
        self.producer_id = Some(producer);
        self
    }

    pub fn with_attach(mut self, target: ObjectId) -> Self {
        self.attached_to = Some(target);
        self
    }

    pub fn with_lifetime_frames(mut self, current_frame: u32, delay_frames: u32) -> Self {
        self.detonate_at_frame = Some(current_frame.saturating_add(delay_frames));
        self.next_ping_frame = Some(sticky_next_ping_frame(
            current_frame,
            self.detonate_at_frame,
        ));
        self
    }

    pub fn is_active(&self) -> bool {
        !self.detonated && self.has_virtual_charge()
    }

    /// C++ MinefieldBehavior::onCollide :360-362 — inert while sliding.
    pub fn is_scooting(&self) -> bool {
        matches!(self.kind, HostMineKind::LandMine) && self.scoot_frames_left > 0
    }

    /// C++ MinefieldBehavior::setScootParms (host Y-up leftover port).
    /// Returns the spawn pose: producer/start while scooting, dest otherwise.
    pub fn set_scoot_parms(&mut self, start: Vec3, end: Vec3, ground_y: f32) -> Vec3 {
        if !matches!(self.kind, HostMineKind::LandMine) {
            return end;
        }
        let mut end_on_ground = end;
        end_on_ground.y = ground_y;
        let mut scoot_time = self.scoot_from_starting_point_time;
        if start.y > end_on_ground.y {
            let fall = start.y - end_on_ground.y;
            let gravity = MINE_SCOOT_GRAVITY.abs().max(1e-6);
            let falling_time = (2.0 * fall / gravity).sqrt().ceil() as u32;
            scoot_time = scoot_time.max(falling_time);
        }
        if scoot_time == 0 {
            self.clear_scoot();
            return end_on_ground;
        }
        let dx = end_on_ground.x - start.x;
        let dz = end_on_ground.z - start.z;
        let dy = end_on_ground.y - start.y;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 0.1 && dy.abs() <= 0.1 {
            self.clear_scoot();
            return end_on_ground;
        }
        let t = scoot_time as f32;
        let speed = dist / t;
        let accel_mag = (2.0 * (dist - speed * t) / (t * t)).abs();
        let dx_norm = if dist <= 0.1 { 0.0 } else { dx / dist };
        let dz_norm = if dist <= 0.1 { 0.0 } else { dz / dist };
        self.scoot_vel = [dx_norm * speed, 0.0, dz_norm * speed];
        self.scoot_accel = [
            -dx_norm * accel_mag,
            MINE_SCOOT_GRAVITY,
            -dz_norm * accel_mag,
        ];
        self.scoot_frames_left = scoot_time;
        start
    }

    fn clear_scoot(&mut self) {
        self.scoot_frames_left = 0;
        self.scoot_vel = [0.0; 3];
        self.scoot_accel = [0.0; 3];
    }

    /// C++ MinefieldBehavior::update scoot integrate. `None` when not scooting.
    pub fn tick_scoot(&mut self, pos: Vec3, ground_y: f32) -> Option<Vec3> {
        if !self.is_scooting() {
            return None;
        }
        let mut vel = Vec3::from_array(self.scoot_vel);
        let accel = Vec3::from_array(self.scoot_accel);
        vel += accel;
        let mut next = pos + vel;
        if next.y < ground_y || self.scoot_frames_left <= 1 {
            next.y = ground_y;
        }
        self.scoot_vel = vel.to_array();
        self.scoot_frames_left = self.scoot_frames_left.saturating_sub(1);
        if self.scoot_frames_left == 0 {
            self.scoot_vel = [0.0; 3];
            self.scoot_accel = [0.0; 3];
        }
        Some(next)
    }

    /// C++ onCollide early-out when m_virtualMinesRemaining == 0.
    pub fn has_virtual_charge(&self) -> bool {
        if !matches!(self.kind, HostMineKind::LandMine) {
            return true;
        }
        self.virtual_mines_remaining > 0
    }

    /// C++ MinefieldBehavior::onDamage runs after HP is applied and before
    /// the body is destroyed. A 0-HP pad with remaining virtual mines must
    /// still chain-detonate (not silently vanish).
    pub fn defers_lethal_body_destroy(&self) -> bool {
        matches!(self.kind, HostMineKind::LandMine) && !self.detonated && self.has_virtual_charge()
    }

    /// C++ MinefieldBehavior::detonateOnce decrement + destroy gate.
    /// Returns true when the object itself should be destroyed.
    pub fn consume_virtual_mine(&mut self) -> bool {
        if matches!(self.kind, HostMineKind::LandMine) && self.virtual_mines_remaining > 0 {
            self.virtual_mines_remaining -= 1;
        }
        if matches!(self.kind, HostMineKind::LandMine) {
            self.virtual_mines_remaining == 0 && !self.regenerates
        } else {
            true
        }
    }

    /// Health fraction matching remaining / numVirtualMines (C++ detonateOnce).
    pub fn virtual_health_fraction(&self) -> f32 {
        let num = self.num_virtual_mines.max(1) as f32;
        (self.virtual_mines_remaining as f32 / num).clamp(0.0, 1.0)
    }

    pub fn expire_immunes(&mut self, now: u32) {
        self.immunes
            .retain(|im| now <= im.collide_frame.saturating_add(MINE_IMMUNITY_HOLD_FRAMES));
    }

    /// C++ immunity list first: refresh collideTime and skip detonate.
    pub fn refresh_immune(&mut self, id: ObjectId, now: u32) -> bool {
        if let Some(slot) = self.immunes.iter_mut().find(|im| im.id == id) {
            slot.collide_frame = now;
            return true;
        }
        false
    }

    /// C++ grant immunity while isClearingMines && goal object.
    pub fn grant_immunity(&mut self, id: ObjectId, now: u32) {
        if let Some(slot) = self.immunes.iter_mut().find(|im| im.id == id) {
            slot.collide_frame = now;
            return;
        }
        if self.immunes.len() < MINE_MAX_IMMUNITY {
            self.immunes.push(HostMineImmune {
                id,
                collide_frame: now,
            });
        }
    }

    /// C++ RepeatDetonateMoveThresh: same unit must move before another trip.
    pub fn allow_repeat_detonate(&mut self, id: ObjectId, pos: Vec3) -> bool {
        let thresh = self.repeat_detonate_move_thresh.max(0.0);
        let thresh_sqr = thresh * thresh;
        if let Some(det) = self.detonators.iter_mut().find(|d| d.id == id) {
            let dx = pos.x - det.x;
            let dz = pos.z - det.z;
            if dx * dx + dz * dz <= thresh_sqr {
                return false;
            }
            det.x = pos.x;
            det.z = pos.z;
            return true;
        }
        self.detonators.push(HostMineDetonator {
            id,
            x: pos.x,
            z: pos.z,
        });
        true
    }

    /// C++ DemoTrapUpdate.cpp:124-130: dead + DetonateWhenKilled → detonate().
    pub fn detonate_when_killed(&self) -> bool {
        matches!(self.kind, HostMineKind::DemoTrap) && !self.detonated
    }

    /// C++ DemoTrap SlowDeath DestructionDelay: arm 1s warning before splash.
    pub fn arm_demo_trap_warning(&mut self, now: u32) -> bool {
        if !matches!(self.kind, HostMineKind::DemoTrap) || self.detonated {
            return false;
        }
        if self.warning_detonate_at_frame.is_some() {
            return false;
        }
        self.warning_detonate_at_frame =
            Some(now.saturating_add(DEMO_TRAP_DESTRUCTION_DELAY_FRAMES));
        self.proximity_enabled = false;
        true
    }

    pub fn demo_trap_warning_armed(&self) -> bool {
        matches!(self.kind, HostMineKind::DemoTrap)
            && !self.detonated
            && self.warning_detonate_at_frame.is_some()
    }

    pub fn demo_trap_warning_ready(&self, now: u32) -> bool {
        self.demo_trap_warning_armed() && self.warning_detonate_at_frame.is_some_and(|at| now >= at)
    }

    /// C++ MinefieldBehavior::disarm for Regenerates pads: keep the object.
    /// Drain to MIN_HEALTH, virtual=0. Caller applies rubble/MASKED + health.
    pub fn disarm_regenerating_pad(&mut self) -> bool {
        if !matches!(self.kind, HostMineKind::LandMine) || !self.regenerates {
            return false;
        }
        self.virtual_mines_remaining = 0;
        self.draining = false;
        self.auto_heal_stopped = false;
        self.auto_heal_wake_frame = 0;
        self.last_synced_health = Some(MINE_MIN_HEALTH);
        self.clear_pre_attack_clearer = None;
        self.clear_pre_attack_ready_frame = 0;
        true
    }

    /// C++ onDamage virtual-mine expected count (ceil damage / floor healing).
    pub fn virtual_mines_expected_from_health(
        &self,
        health: f32,
        max_health: f32,
        healing: bool,
    ) -> u32 {
        let max_h = max_health.max(1e-6);
        let expected_f = self.num_virtual_mines as f32 * health.max(0.0) / max_h;
        let expected = if healing {
            expected_f.floor() as u32
        } else {
            expected_f.ceil() as u32
        };
        expected.min(self.num_virtual_mines)
    }
}

/// One step of C++ MinefieldBehavior::onDamage virtual-sync loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MineOnDamageStep {
    /// Health band matches remaining virtual mines.
    Done,
    /// Quiet drain decrement; caller should re-read health and continue.
    Continue,
    /// Fire detonateOnce (splash + consume_virtual_mine) then continue.
    Detonate,
}

impl HostMineData {
    /// One C++ onDamage loop step. `Detonate` means caller must fire detonateOnce.
    pub fn apply_on_damage_step(
        &mut self,
        health: f32,
        max_health: f32,
        healing: bool,
        self_unresistable_drain: bool,
    ) -> MineOnDamageStep {
        if self.ignore_damage || !matches!(self.kind, HostMineKind::LandMine) {
            return MineOnDamageStep::Done;
        }
        let expected = self.virtual_mines_expected_from_health(health, max_health, healing);
        if self.virtual_mines_remaining < expected {
            self.virtual_mines_remaining = expected;
            self.last_synced_health = Some(health);
            MineOnDamageStep::Done
        } else if self.virtual_mines_remaining > expected {
            if self.draining && self_unresistable_drain {
                self.virtual_mines_remaining = self.virtual_mines_remaining.saturating_sub(1);
                self.last_synced_health = Some(health);
                MineOnDamageStep::Continue
            } else {
                MineOnDamageStep::Detonate
            }
        } else {
            self.last_synced_health = Some(health);
            MineOnDamageStep::Done
        }
    }

    /// Clamp empty regen pads to MIN_HEALTH (C++ onDamage after the loop).
    pub fn clamp_empty_regen_health(&self, health: f32) -> f32 {
        if self.virtual_mines_remaining == 0 && self.regenerates && health < MINE_MIN_HEALTH {
            MINE_MIN_HEALTH
        } else {
            health
        }
    }

    /// C++ AutoHeal pulse for regenerating China pads.
    pub fn tick_regen_auto_heal(&mut self, now: u32, health: f32, max_health: f32) -> f32 {
        if !self.regenerates || self.auto_heal_stopped || self.draining || self.detonated {
            return health;
        }
        if health + 1e-4 >= max_health {
            return health;
        }
        if self.auto_heal_wake_frame == 0 {
            self.auto_heal_wake_frame = now.saturating_add(MINE_AUTO_HEAL_DELAY_FRAMES);
            return health;
        }
        if now < self.auto_heal_wake_frame {
            return health;
        }
        self.auto_heal_wake_frame = now.saturating_add(MINE_AUTO_HEAL_DELAY_FRAMES);
        (health + MINE_AUTO_HEAL_AMOUNT).min(max_health)
    }

    /// C++ StopsRegenAfterCreatorDies: flip regen off and start drain.
    pub fn note_producer_dead(&mut self, now: u32) {
        if !self.regenerates || !self.stops_regen_after_creator_dies {
            return;
        }
        self.regenerates = false;
        self.draining = true;
        self.auto_heal_stopped = true;
        self.next_death_check_frame = now.saturating_add(MINE_CREATOR_DEATH_CHECK_FRAMES);
    }

    /// C++ DegenPercentPerSecondAfterCreatorDies / LOGICFRAMES_PER_SECOND.
    pub fn drain_health_amount(&self, max_health: f32) -> f32 {
        if !self.draining {
            return 0.0;
        }
        (max_health * MINE_DEGEN_PERCENT_PER_SECOND) / MINE_LOGIC_FPS
    }

    /// PreAttackDelay wind-up. Returns true when DISARM may fire.
    pub fn begin_or_ready_clear_pre_attack(
        &mut self,
        clearer: ObjectId,
        now: u32,
        delay_frames: u32,
    ) -> bool {
        if self.clear_pre_attack_clearer != Some(clearer) || self.clear_pre_attack_ready_frame == 0
        {
            self.clear_pre_attack_clearer = Some(clearer);
            self.clear_pre_attack_ready_frame = now.saturating_add(delay_frames.max(1));
            return false;
        }
        now >= self.clear_pre_attack_ready_frame
    }

    pub fn reset_clear_pre_attack(&mut self) {
        self.clear_pre_attack_clearer = None;
        self.clear_pre_attack_ready_frame = 0;
    }
}

/// Damage plan for one victim under a residual detonation.
#[derive(Debug, Clone, Copy)]
pub struct HostMineDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
}

/// Result of resolving one residual detonation.
#[derive(Debug, Clone)]
pub struct HostMineDetonationPlan {
    pub mine_id: ObjectId,
    pub kind: HostMineKind,
    pub position: Vec3,
    pub owner_team: super::Team,
    pub producer_id: Option<ObjectId>,
    pub hits: Vec<HostMineDamageHit>,
    pub reason: HostMineDetonateReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMineDetonateReason {
    Proximity,
    Timed,
    Manual,
    /// C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
    Killed,
}

/// Cluster-mine ring residual (not full OCL scatter density).
/// Host placement ring uses a residual subset of retail NumVirtualMines / DistanceAroundObject.
pub const CLUSTER_MINE_COUNT: usize = 6;
pub const CLUSTER_MINE_RING_RADIUS: f32 = 40.0;

/// Retail ChinaClusterMine MinefieldBehavior NumVirtualMines residual.
pub const CLUSTER_MINE_NUM_VIRTUAL: u32 = 8;

/// C++ MinefieldBehavior ctor DetonatedBy = ENEMIES | NEUTRAL.
pub fn land_mine_proximity_trips(is_enemies: bool, is_neutral: bool) -> bool {
    is_enemies || is_neutral
}

/// Retail DozerMineDisarmingWeapon vs WorkerMineDisarmingWeapon PreAttackDelay.
pub fn mine_clear_pre_attack_frames(is_worker: bool, template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("dozer") {
        DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES
    } else if is_worker || n.contains("worker") {
        WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES
    } else {
        DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES
    }
}

/// Deterministic DropVariance unit samples in [0, 1] (not a fixed 0.5).
pub fn cluster_mines_drop_unit_samples(seed: u32) -> (f32, f32) {
    let ux = crate::game_logic::host_rng_residual::pure_logic_random_real(seed, 0, 0.0, 1.0);
    let uy = crate::game_logic::host_rng_residual::pure_logic_random_real(seed, 1, 0.0, 1.0);
    (ux, uy)
}

/// Retail ClusterMinesBomb GenerateMinefieldBehavior DistanceAroundObject residual.
pub const CLUSTER_MINES_DISTANCE_AROUND_OBJECT: f32 = 80.0;

/// Retail SUPERWEAPON_ClusterMines DeliverPayload DropVariance residual (X/Y/Z).
pub const CLUSTER_MINES_DROP_VARIANCE: (f32, f32, f32) = (20.0, 20.0, 0.0);

/// Retail SUPERWEAPON_ClusterMines DeliveryDistance residual.
pub const CLUSTER_MINES_DELIVERY_DISTANCE: f32 = 140.0;

/// Retail SUPERWEAPON_ClusterMines DeliveryDecalRadius residual.
pub const CLUSTER_MINES_DELIVERY_DECAL_RADIUS: f32 = 100.0;

/// Retail SuperweaponClusterMines RadiusCursorRadius residual.
pub const CLUSTER_MINES_RADIUS_CURSOR: f32 = 100.0;

/// Retail SUPERWEAPON_ClusterMines Transport residual.
pub const CLUSTER_MINES_OCL_TRANSPORT: &str = "ChinaJetCargoPlane";

/// Retail payload bomb / mine template residual names.
pub const CLUSTER_MINES_BOMB_TEMPLATE: &str = "ClusterMinesBomb";
pub const CLUSTER_MINES_MINE_TEMPLATE: &str = "ChinaClusterMine";

/// Retail SuperweaponClusterMines ReloadTime residual (msec).
pub const SUPERWEAPON_CLUSTER_MINES_RELOAD_MS: u32 = 240_000;
/// ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const SUPERWEAPON_CLUSTER_MINES_RELOAD_FRAMES: u32 = 7_200;

// --- Wave 78: ClusterMines DeliveryDecal / ViewObject / science residual deepen ---
/// Retail SuperweaponClusterMines special-power template residual name.
pub const SUPERWEAPON_CLUSTER_MINES: &str = "SuperweaponClusterMines";
/// Retail SUPERWEAPON_ClusterMines OCL residual name.
pub const CLUSTER_MINES_OCL: &str = "SUPERWEAPON_ClusterMines";
/// Retail SCIENCE_ClusterMines residual.
pub const SCIENCE_CLUSTER_MINES: &str = "SCIENCE_ClusterMines";
/// Retail SciencePurchasePointCost residual.
pub const CLUSTER_MINES_SCIENCE_POINT_COST: u32 = 1;
/// Retail SCIENCE_ClusterMines PrerequisiteSciences residual tokens.
pub const CLUSTER_MINES_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_CHINA", "SCIENCE_Rank3"];
/// Retail SuperweaponClusterMines ViewObjectDuration residual (msec).
pub const CLUSTER_MINES_VIEW_OBJECT_DURATION_MS: u32 = 30_000;
/// ViewObjectDuration 30000ms → 900 frames @ 30 FPS.
pub const CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES: u32 = 900;
/// Retail SuperweaponClusterMines ViewObjectRange residual.
pub const CLUSTER_MINES_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail DeliverPayload DropOffset residual (X/Y/Z).
pub const CLUSTER_MINES_DROP_OFFSET: (f32, f32, f32) = (0.0, 0.0, -2.0);
/// Retail DeliverPayload MaxAttempts residual.
pub const CLUSTER_MINES_MAX_ATTEMPTS: u32 = 4;
/// Retail DeliveryDecal Texture residual.
pub const CLUSTER_MINES_DECAL_TEXTURE: &str = "SCCClusterMines_China";
/// Retail DeliveryDecal Style residual.
pub const CLUSTER_MINES_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail DeliveryDecal OpacityMin residual (percent).
pub const CLUSTER_MINES_DECAL_OPACITY_MIN_PCT: u32 = 25;
/// Retail DeliveryDecal OpacityMax residual (percent).
pub const CLUSTER_MINES_DECAL_OPACITY_MAX_PCT: u32 = 50;
/// Retail DeliveryDecal OpacityThrobTime residual (msec).
pub const CLUSTER_MINES_DECAL_THROB_MS: u32 = 500;
/// Retail DeliveryDecal Color residual (R:255 G:156 B:0 A:255).
pub const CLUSTER_MINES_DECAL_COLOR: (u8, u8, u8, u8) = (255, 156, 0, 255);
/// Retail Payload count residual (`Payload = ClusterMinesBomb 1`).
pub const CLUSTER_MINES_PAYLOAD_COUNT: u32 = 1;

/// DozerMineDisarmingWeapon / WorkerMineDisarmingWeapon AttackRange residual.
pub const DOZER_MINE_CLEAR_RANGE: f32 = 5.0;

/// ContinueAttackRange residual: after/while clearing, look for mines this far.
/// Also used as idle dozer auto-acquire scan radius residual (not full BoredRange).
pub const DOZER_MINE_CLEAR_SCAN_RANGE: f32 = 100.0;

/// Retail DozerMineDisarmingWeapon PreAttackDelay residual (msec).
pub const DOZER_MINE_CLEAR_PRE_ATTACK_MS: u32 = 1_200;
/// PreAttackDelay 1200ms → 36 frames @ 30 FPS.
pub const DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES: u32 = 36;

/// Retail DozerMineDisarmingWeapon ClipReloadTime residual (msec).
pub const DOZER_MINE_CLEAR_CLIP_RELOAD_MS: u32 = 4_000;
/// ClipReloadTime 4000ms → 120 frames @ 30 FPS.
pub const DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES: u32 = 120;

/// Retail WorkerMineDisarmingWeapon PreAttackDelay residual (msec).
pub const WORKER_MINE_CLEAR_PRE_ATTACK_MS: u32 = 1_000;
/// PreAttackDelay 1000ms → 30 frames @ 30 FPS.
pub const WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES: u32 = 30;

/// Retail WorkerMineDisarmingWeapon DelayBetweenShots residual (msec).
pub const WORKER_MINE_CLEAR_DELAY_MS: u32 = 1_000;
/// DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const WORKER_MINE_CLEAR_DELAY_FRAMES: u32 = 30;

/// Audio residual when a dozer/worker safely disarms a mine (FXList MineClearedByDozer).
pub const MINE_CLEARED_AUDIO: &str = "MineClearedByDozer";

// --- DemoTrapUpdate residual (FactionBuilding GLADemoTrap) ---

/// Retail DemoTrapUpdate DefaultProximityMode residual.
pub const DEMO_TRAP_DEFAULT_PROXIMITY_MODE: bool = true;
/// Retail DemoTrapUpdate TriggerDetonationRange residual.
pub const DEMO_TRAP_TRIGGER_RANGE: f32 = 40.0;
/// Retail SlowDeathBehavior DestructionDelay residual (msec) — warning fuse.
pub const DEMO_TRAP_DESTRUCTION_DELAY_MS: u32 = 1_000;
/// DestructionDelay 1000ms → 30 frames @ 30 FPS.
pub const DEMO_TRAP_DESTRUCTION_DELAY_FRAMES: u32 = 30;
/// Retail SlowDeath INITIAL FX_GLADemoTrapWarning residual.
pub const DEMO_TRAP_WARNING_AUDIO: &str = "GLADemoTrapWarning";
/// Retail DemoTrapUpdate DetonateWhenKilled residual.
pub const DEMO_TRAP_DETONATE_WHEN_KILLED: bool = true;
/// Retail DemoTrapUpdate AutoDetonationWithFriendsInvolved residual.
pub const DEMO_TRAP_AUTO_DETONATE_WITH_FRIENDS: bool = true;

// --- Burton SpecialAbilityUpdate MaxSpecialObjects residual ---

/// Retail ColonelBurton RemoteC4Charge MaxSpecialObjects residual.
pub const BURTON_MAX_REMOTE_CHARGES: u32 = 8;
/// Retail ColonelBurton TimedC4Charge MaxSpecialObjects residual.
pub const BURTON_MAX_TIMED_CHARGES: u32 = 10;
/// Retail UniqueSpecialObjectTargets residual (one charge per target).
pub const BURTON_UNIQUE_CHARGE_TARGETS: bool = true;
/// Retail UnpackTime residual for plant charge (msec).
pub const BURTON_CHARGE_UNPACK_TIME_MS: u32 = 5_500;
/// UnpackTime 5500ms → 165 frames @ 30 FPS.
pub const BURTON_CHARGE_UNPACK_TIME_FRAMES: u32 = 165;
/// Retail FleeRangeAfterCompletion residual.
pub const BURTON_FLEE_RANGE_AFTER_CHARGE: f32 = 100.0;
/// Retail SpecialObject remote / timed residual names.
pub const BURTON_REMOTE_CHARGE_OBJECT: &str = "RemoteC4Charge";
pub const BURTON_TIMED_CHARGE_OBJECT: &str = "TimedC4Charge";
/// Retail Tank Hunter SpecialObject.
pub const TANK_HUNTER_TNT_OBJECT: &str = "TNTStickyBomb";
/// Retail TimedC4Charge LifetimeUpdate Min/MaxLifetime = 20000 ms → 600f @ 30 FPS.
pub const TIMED_C4_LIFETIME_MS: u32 = 20_000;
pub const TIMED_C4_LIFETIME_FRAMES: u32 = (TIMED_C4_LIFETIME_MS * 30 + 999) / 1000;
/// Retail TNTStickyBomb LifetimeUpdate Min/MaxLifetime = 10000 ms → 300f @ 30 FPS.
pub const TNT_STICKY_LIFETIME_MS: u32 = 10_000;
pub const TNT_STICKY_LIFETIME_FRAMES: u32 = (TNT_STICKY_LIFETIME_MS * 30 + 999) / 1000;

/// Retail timed SpecialObject name for producer template residual.
pub fn retail_timed_charge_template(producer_template: Option<&str>) -> &'static str {
    if producer_template
        .map(|n| {
            let l = n.to_ascii_lowercase();
            l.contains("tankhunter") || l.contains("tank_hunter")
        })
        .unwrap_or(false)
    {
        TANK_HUNTER_TNT_OBJECT
    } else {
        // Burton TimedC4 default (also generic timed demo residual).
        BURTON_TIMED_CHARGE_OBJECT
    }
}

/// Retail default lifetime frames for timed charge template.
pub fn retail_timed_charge_lifetime_frames(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("timedc4") {
        TIMED_C4_LIFETIME_FRAMES
    } else {
        // TNTStickyBomb / TestTimedDemoCharge residual default 10s.
        TNT_STICKY_LIFETIME_FRAMES
    }
}

/// Whether residual unit can clear mines (C++ KINDOF_DOZER / Worker + DISARM weapon residual).
/// Fail-closed: not full weapon-set / AntiMine bit matrix.
pub fn is_mine_clearer(is_worker: bool, template_name: &str) -> bool {
    if is_worker {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    n.contains("dozer") || n.contains("worker")
}

/// C++ DemoTrapUpdate.cpp:181-191: skip KINDOF_DOZER only when current weapon
/// is DAMAGE_DISARM **and** OBJECT_STATUS_IS_ATTACKING. Idle dozers still trigger.
pub fn demo_trap_skips_dozer_disarm_while_attacking(
    is_dozer: bool,
    current_weapon_is_disarm: bool,
    is_attacking: bool,
) -> bool {
    is_dozer && current_weapon_is_disarm && is_attacking
}

/// C++ DemoTrapUpdate.cpp:196 — proximity trigger requires ENEMIES.
/// Neutral / Allies never start the fuse (FriendlyDetonation only continues scan).
pub fn demo_trap_proximity_requires_enemies(is_enemies: bool) -> bool {
    is_enemies
}

/// C++ GameLogicDispatch.cpp:594 MSG_SET_MINE_CLEARING_DETAIL.
/// Idle dozers must not auto-seek/clear unless the player armed the detail set.
pub fn mine_clear_allowed_for_order(mine_clearing_detail: bool) -> bool {
    mine_clearing_detail
}

/// C++ MinefieldBehavior.cpp:345-350: infantry+dozer workers skip unless WorkersDetonate.
pub fn minefield_skips_worker(workers_detonate: bool, is_infantry: bool, is_dozer: bool) -> bool {
    !workers_detonate && is_infantry && is_dozer
}

/// C++ isClearingMines() && getGoalObject() != NULL.
/// Host: mine-clearing detail armed and a goal mine is assigned (approach or attack).
pub fn is_clearing_mines_with_goal(mine_clearing_detail: bool, has_goal_mine: bool) -> bool {
    mine_clearing_detail && has_goal_mine
}

/// C++ StickyBombUpdate initStickyBomb / update ping schedule.
pub fn sticky_next_ping_frame(now: u32, detonate_at: Option<u32>) -> u32 {
    if let Some(die) = detonate_at {
        if die > now {
            let pings = (die - now) / UNIT_BOMB_PING_FRAMES;
            return die.saturating_sub(pings * UNIT_BOMB_PING_FRAMES);
        }
    }
    now.saturating_add(UNIT_BOMB_PING_FRAMES)
}

/// C++ StickyBombUpdate immobile follow: keep plant XY, snap Z/Y to ground.
pub fn sticky_immobile_follow_pos(charge_pos: Vec3, ground_y: f32) -> Vec3 {
    Vec3::new(charge_pos.x, ground_y, charge_pos.z)
}

/// C++ StickyBombUpdate vehicle follow: target XY + OffsetZ.
pub fn sticky_vehicle_follow_pos(target_pos: Vec3, offset_z: f32) -> Vec3 {
    Vec3::new(target_pos.x, target_pos.y + offset_z, target_pos.z)
}

/// C++ GeometryInfo::clipPointToFootprint (circle residual).
pub fn clip_point_to_mine_footprint(mine_pos: Vec3, victim_pos: Vec3, radius: f32) -> Vec3 {
    let dx = victim_pos.x - mine_pos.x;
    let dz = victim_pos.z - mine_pos.z;
    let dist = (dx * dx + dz * dz).sqrt();
    if radius <= 0.0 || dist <= radius || dist <= f32::EPSILON {
        return Vec3::new(victim_pos.x, mine_pos.y, victim_pos.z);
    }
    let s = radius / dist;
    Vec3::new(mine_pos.x + dx * s, mine_pos.y, mine_pos.z + dz * s)
}

/// Residual kinds that can be disarmed (DAMAGE_DISARM → destroy without detonation).
///
/// C++ Weapon.cpp DISARM estimate is nonzero for KINDOF_MINE | BOOBY_TRAP | DEMOTRAP.
/// No LandMineInterface → destroyObject (safe douse, no explosion).
pub fn can_clear_mine_kind(kind: HostMineKind) -> bool {
    match kind {
        HostMineKind::LandMine
        | HostMineKind::TimedDemoCharge
        | HostMineKind::RemoteDemoCharge
        | HostMineKind::DemoTrap => true,
    }
}

/// C++ MinefieldBehavior::onCollide geometry contact (circle residual).
///
/// Partition collide fires when the victim footprint overlaps the mine disk
/// (China mine GeometryMajorRadius 30 + victim geometry), not when centers
/// are within 30.
pub fn land_mine_geometry_contacts(
    mine_pos: Vec3,
    mine_radius: f32,
    victim_pos: Vec3,
    victim_radius: f32,
) -> bool {
    let dx = victim_pos.x - mine_pos.x;
    let dz = victim_pos.z - mine_pos.z;
    let r = mine_radius.max(0.0) + victim_radius.max(0.0);
    dx * dx + dz * dz <= r * r
}

/// Victim collide radius for MinefieldBehavior::onCollide residual.
/// Authored geometry uses the bounding circle; otherwise selection radius.
pub fn victim_mine_collide_radius(
    authored: bool,
    bounding_circle: f32,
    selection_radius: f32,
) -> f32 {
    if authored && bounding_circle > 0.0 {
        bounding_circle
    } else {
        selection_radius.max(0.0)
    }
}

/// C++ Thing::isAboveTerrain — height-above-terrain > 0 (host Y-up).
pub const fn is_above_terrain(height_y: f32, ground_y: f32) -> bool {
    height_y > ground_y
}

/// NeutronBlastBehavior::neutronBlastToObject residual outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronMineVictimEffect {
    None,
    KillInfantry,
    UnmanVehicle,
    KillCliffJumper,
}

/// Neutron blast victim filter + effect (radius 20, no airborne, no allies).
pub fn neutron_mine_victim_effect(
    distance: f32,
    is_ally: bool,
    is_airborne: bool,
    is_infantry: bool,
    is_vehicle: bool,
    is_drone: bool,
    is_cliff_jumper: bool,
) -> NeutronMineVictimEffect {
    if distance > NEUTRON_MINE_BLAST_RADIUS {
        return NeutronMineVictimEffect::None;
    }
    if !NEUTRON_MINE_AFFECT_ALLIES && is_ally {
        return NeutronMineVictimEffect::None;
    }
    if !NEUTRON_MINE_AFFECT_AIRBORNE && is_airborne {
        return NeutronMineVictimEffect::None;
    }
    if is_infantry {
        return NeutronMineVictimEffect::KillInfantry;
    }
    if is_vehicle && !is_drone {
        if is_cliff_jumper {
            return NeutronMineVictimEffect::KillCliffJumper;
        }
        return NeutronMineVictimEffect::UnmanVehicle;
    }
    NeutronMineVictimEffect::None
}

/// Template names recognized as residual land mines.
pub fn is_land_mine_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("standardmine")
        || n.contains("clustermine")
        || n.contains("empmine")
        || n == "testlandmine"
        || (n.contains("mine") && !n.contains("minefield") && !n.contains("miner"))
}

/// Template names recognized as residual demo traps.
pub fn is_demo_trap_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("demotrap") || n == "testdemotrap"
}

/// Template names recognized as residual timed demo charges.
pub fn is_timed_demo_charge_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Prefer remote match first via is_remote_demo_charge_template.
    if is_remote_demo_charge_template(name) {
        return false;
    }
    n.contains("stickybomb")
        || n.contains("democharge")
        || n.contains("tntsticky")
        || n.contains("timedc4")
        || n == "testtimeddemocharge"
        || n == "timedc4charge"
}

/// Template names recognized as residual remote demo charges.
pub fn is_remote_demo_charge_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("remotedemocharge")
        || n.contains("remotecharge")
        || n.contains("remotec4")
        || n == "testremotedemocharge"
        || n == "remotec4charge"
}

/// Infer residual mine kind from template name, if any.
pub fn infer_mine_kind(template_name: &str) -> Option<HostMineKind> {
    if is_demo_trap_template(template_name) {
        Some(HostMineKind::DemoTrap)
    } else if is_remote_demo_charge_template(template_name) {
        Some(HostMineKind::RemoteDemoCharge)
    } else if is_timed_demo_charge_template(template_name) {
        Some(HostMineKind::TimedDemoCharge)
    } else if is_land_mine_template(template_name) {
        Some(HostMineKind::LandMine)
    } else {
        None
    }
}

/// C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
pub fn should_detonate_when_killed(
    kind: HostMineKind,
    detonate_when_killed: bool,
    effectively_dead: bool,
    detonated: bool,
    under_construction_or_sold: bool,
) -> bool {
    detonate_when_killed
        && effectively_dead
        && !detonated
        && !under_construction_or_sold
        && matches!(kind, HostMineKind::DemoTrap)
}

/// Build residual mine data for a newly created host object (if template matches).
///
/// `has_gamma` applies Chem Anthrax Gamma puddle (`OCL_PoisonFieldGammaMedium`)
/// when the controlling player has researched the Palace upgrade.
pub fn residual_data_for_template(
    template_name: &str,
    current_frame: u32,
    has_gamma: bool,
) -> Option<HostMineData> {
    match infer_mine_kind(template_name)? {
        HostMineKind::LandMine => Some(HostMineData::land_mine_for_template(template_name)),
        HostMineKind::DemoTrap => {
            let profile = demo_trap_profile(template_name, has_gamma, false);
            Some(HostMineData::demo_trap_with_profile(profile))
        }
        HostMineKind::TimedDemoCharge => Some(HostMineData::timed_demo_charge(current_frame)),
        HostMineKind::RemoteDemoCharge => {
            let mut data = HostMineData::remote_demo_charge();
            data.next_ping_frame = Some(current_frame.saturating_add(UNIT_BOMB_PING_FRAMES));
            Some(data)
        }
    }
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn mine_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / MINE_LOGIC_FPS)).round() as u32
}

/// Whether a producer may place another remote charge (MaxSpecialObjects residual).
pub fn can_place_remote_charge(active_remote_count: u32) -> bool {
    active_remote_count < BURTON_MAX_REMOTE_CHARGES
}

/// Whether a producer may place another timed charge (MaxSpecialObjects residual).
pub fn can_place_timed_charge(active_timed_count: u32) -> bool {
    active_timed_count < BURTON_MAX_TIMED_CHARGES
}

/// Apply SUPERWEAPON_ClusterMines DropVariance residual to a delivery center.
///
/// Unit-sample residual (not full GameLogicRandom stream): sample in [-var, +var]
/// via a deterministic host unit sample in [0, 1].
pub fn apply_cluster_mines_drop_variance(center: Vec3, unit_x: f32, unit_y: f32) -> Vec3 {
    let (vx, vy, vz) = CLUSTER_MINES_DROP_VARIANCE;
    let ux = unit_x.clamp(0.0, 1.0);
    let uy = unit_y.clamp(0.0, 1.0);
    Vec3::new(
        center.x + (ux * 2.0 - 1.0) * vx,
        center.y + 0.0 * vz, // Z variance residual is 0 in retail OCL
        center.z + (uy * 2.0 - 1.0) * vy,
    )
}

/// Positions for a residual cluster-mine ring around `center`.
pub fn cluster_mine_positions(center: Vec3, count: usize, ring_radius: f32) -> Vec<Vec3> {
    if count == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let angle = (i as f32) * std::f32::consts::TAU / (count as f32);
        out.push(Vec3::new(
            center.x + ring_radius * angle.cos(),
            center.y,
            center.z + ring_radius * angle.sin(),
        ));
    }
    out
}

/// Producer / bomb / building footprint used by GenerateMinefieldBehavior residual.
#[derive(Debug, Clone, Copy)]
pub struct MinePlacementGeom {
    pub center: Vec3,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub is_box: bool,
}

impl MinePlacementGeom {
    pub fn circle(center: Vec3, radius: f32) -> Self {
        Self {
            center,
            major_radius: radius.max(0.0),
            minor_radius: radius.max(0.0),
            is_box: false,
        }
    }

    pub fn bounding_circle_radius(self) -> f32 {
        if self.is_box {
            (self.major_radius * self.major_radius + self.minor_radius * self.minor_radius).sqrt()
        } else {
            self.major_radius.max(self.minor_radius)
        }
    }

    pub fn expand(mut self, amount: f32) -> Self {
        self.major_radius = (self.major_radius + amount).max(0.0);
        self.minor_radius = (self.minor_radius + amount).max(0.0);
        self
    }
}

fn circle_mine_count(radius: f32, mine_radius: f32) -> usize {
    let diameter = (mine_radius * 2.0).max(0.001);
    let circum = std::f32::consts::TAU * radius.max(0.0);
    (circum / diameter).ceil().max(1.0) as usize
}

fn line_mine_count(len: f32, mine_radius: f32) -> usize {
    let diameter = (mine_radius * 2.0).max(0.001);
    (len / diameter).ceil().max(1.0) as usize
}

/// C++ placeMinesAroundCircle (no jitter residual).
pub fn mines_around_circle(center: Vec3, radius: f32, mine_radius: f32) -> Vec<Vec3> {
    let count = circle_mine_count(radius, mine_radius);
    cluster_mine_positions(center, count, radius)
}

/// C++ placeMinesAroundRect + placeMinesAlongLine (skip first on each edge).
pub fn mines_around_rect(center: Vec3, major: f32, minor: f32, mine_radius: f32) -> Vec<Vec3> {
    let corners = [
        Vec3::new(center.x + major, center.y, center.z + minor),
        Vec3::new(center.x - major, center.y, center.z + minor),
        Vec3::new(center.x - major, center.y, center.z - minor),
        Vec3::new(center.x + major, center.y, center.z - minor),
    ];
    let mut out = Vec::new();
    for i in 0..4 {
        let start = corners[i];
        let end = corners[(i + 1) % 4];
        let dx = end.x - start.x;
        let dz = end.z - start.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len <= f32::EPSILON {
            continue;
        }
        let n = line_mine_count(len, mine_radius);
        let inc = len / n as f32;
        let mut place = inc; // skipOneAtStart
        while place <= len + 0.001 {
            let t = place / len;
            out.push(Vec3::new(start.x + dx * t, center.y, start.z + dz * t));
            place += inc;
        }
    }
    out
}

/// C++ GenerateMinefieldBehavior::placeMines SmartBorder path.
pub fn smart_border_mine_positions(
    geom: MinePlacementGeom,
    distance_around: f32,
    mine_radius: f32,
    always_circular: bool,
    skip_interior: bool,
) -> Vec<Vec3> {
    let mut out = Vec::new();
    let mut ring = if skip_interior {
        geom
    } else {
        out.push(geom.center);
        MinePlacementGeom::circle(geom.center, mine_radius)
    };
    if always_circular {
        let r = ring.bounding_circle_radius();
        ring = MinePlacementGeom::circle(ring.center, r);
    }
    ring = ring.expand(mine_radius);
    let mine_diameter = mine_radius * 2.0;
    // C++ placeMines SmartBorder: do { place; expand(diameter); }
    // while (expandedRadius < DistanceAroundObject). Check AFTER expand so
    // the last placed ring is the one still inside the budget (outer 73, not 89).
    loop {
        if ring.is_box && !always_circular {
            out.extend(mines_around_rect(
                ring.center,
                ring.major_radius,
                ring.minor_radius,
                mine_radius,
            ));
        } else {
            out.extend(mines_around_circle(
                ring.center,
                ring.major_radius,
                mine_radius,
            ));
        }
        ring = ring.expand(mine_diameter);
        if ring.bounding_circle_radius() >= distance_around {
            break;
        }
    }
    out
}

/// C++ GenerateMinefieldBehavior BorderOnly path (structure China mines).
pub fn structure_border_mine_positions(
    geom: MinePlacementGeom,
    distance_around: f32,
    mine_radius: f32,
    always_circular: bool,
) -> Vec<Vec3> {
    let expanded = geom.expand(distance_around);
    if expanded.is_box && !always_circular {
        mines_around_rect(
            expanded.center,
            expanded.major_radius,
            expanded.minor_radius,
            mine_radius,
        )
    } else {
        mines_around_circle(expanded.center, expanded.major_radius, mine_radius)
    }
}

/// ClusterMinesBomb SmartBorder residual around a drop center.
pub fn cluster_smart_border_positions(center: Vec3) -> Vec<Vec3> {
    smart_border_mine_positions(
        MinePlacementGeom::circle(center, 1.0),
        CLUSTER_MINES_DISTANCE_AROUND_OBJECT,
        LAND_MINE_GEOMETRY_RADIUS,
        false,
        true,
    )
}

/// C++ placeMineAt skip: mine mostly under a structure.
pub fn mine_spot_under_structure(
    spot: Vec3,
    structure_pos: Vec3,
    structure_radius: f32,
    mine_radius: f32,
) -> bool {
    let shrink = mine_radius * (1.0 - SKIP_IF_THIS_MUCH_UNDER_STRUCTURE);
    let dx = spot.x - structure_pos.x;
    let dz = spot.z - structure_pos.z;
    let reach = (structure_radius + shrink.max(0.0)).max(0.0);
    dx * dx + dz * dz <= reach * reach
}

/// Wave 51 residual honesty: DemoTrap DefaultProximity / trigger / death fuse constants.
pub fn honesty_demo_trap_mode_residual_ok() -> bool {
    DEMO_TRAP_DEFAULT_PROXIMITY_MODE
        && (DEMO_TRAP_TRIGGER_RANGE - 40.0).abs() < 0.01
        && DEMO_TRAP_DESTRUCTION_DELAY_MS == 1_000
        && DEMO_TRAP_DESTRUCTION_DELAY_FRAMES == mine_ms_to_frames(DEMO_TRAP_DESTRUCTION_DELAY_MS)
        && DEMO_TRAP_DETONATE_WHEN_KILLED
        && DEMO_TRAP_AUTO_DETONATE_WITH_FRIENDS
        && HostMineData::demo_trap().demo_trap_mode == DemoTrapMode::Proximity
        && HostMineData::demo_trap().proximity_enabled
        && (HostMineKind::DemoTrap.default_trigger_range() - DEMO_TRAP_TRIGGER_RANGE).abs() < 0.01
}

/// Wave 51 residual honesty: dozer/worker disarm distance + pre-attack/reload frames.
pub fn honesty_mine_clear_residual_ok() -> bool {
    (DOZER_MINE_CLEAR_RANGE - 5.0).abs() < 0.01
        && (DOZER_MINE_CLEAR_SCAN_RANGE - 100.0).abs() < 0.01
        && DOZER_MINE_CLEAR_PRE_ATTACK_MS == 1_200
        && DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES == mine_ms_to_frames(DOZER_MINE_CLEAR_PRE_ATTACK_MS)
        && DOZER_MINE_CLEAR_CLIP_RELOAD_MS == 4_000
        && DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES == mine_ms_to_frames(DOZER_MINE_CLEAR_CLIP_RELOAD_MS)
        && WORKER_MINE_CLEAR_PRE_ATTACK_MS == 1_000
        && WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES == mine_ms_to_frames(WORKER_MINE_CLEAR_PRE_ATTACK_MS)
        && WORKER_MINE_CLEAR_DELAY_MS == 1_000
        && WORKER_MINE_CLEAR_DELAY_FRAMES == mine_ms_to_frames(WORKER_MINE_CLEAR_DELAY_MS)
        && !MINE_CLEARED_AUDIO.is_empty()
}

/// Wave 51 residual honesty: Chem/Demo dual-ring secondary weapon constants.
pub fn honesty_demo_trap_weapon_rings_residual_ok() -> bool {
    let std = DemoTrapProfile::Standard;
    let chem = DemoTrapProfile::ChemBeta;
    let gamma = DemoTrapProfile::ChemGamma;
    let demo = DemoTrapProfile::Demo;
    (std.primary_damage() - 600.0).abs() < 0.01
        && (std.primary_radius() - 25.0).abs() < 0.01
        && (std.secondary_damage() - 400.0).abs() < 0.01
        && (std.secondary_radius() - 50.0).abs() < 0.01
        && (chem.primary_damage() - 250.0).abs() < 0.01
        && (chem.secondary_damage() - 100.0).abs() < 0.01
        && (gamma.primary_damage() - 250.0).abs() < 0.01
        && (gamma.secondary_damage() - 100.0).abs() < 0.01
        && gamma.spawns_poison()
        && chem.spawns_poison()
        && (demo.primary_damage() - 700.0).abs() < 0.01
        && (demo.secondary_damage() - 500.0).abs() < 0.01
        && demo.uses_dual_ring()
        && chem.uses_dual_ring()
        && std.uses_dual_ring()
        && (demo_trap_damage_at(DemoTrapProfile::Standard, 30.0) - 400.0).abs() < 0.01
        && (demo_trap_damage_at(DemoTrapProfile::Demo, 0.0) - 700.0).abs() < 0.01
        && (demo_trap_damage_at(DemoTrapProfile::Demo, 30.0) - 500.0).abs() < 0.01
        && (demo_trap_damage_at(DemoTrapProfile::ChemBeta, 30.0) - 100.0).abs() < 0.01
}

/// Wave 51 residual honesty: Burton sticky max-count residual.
pub fn honesty_burton_charge_max_residual_ok() -> bool {
    BURTON_MAX_REMOTE_CHARGES == 8
        && BURTON_MAX_TIMED_CHARGES == 10
        && BURTON_UNIQUE_CHARGE_TARGETS
        && BURTON_CHARGE_UNPACK_TIME_MS == 5_500
        && BURTON_CHARGE_UNPACK_TIME_FRAMES == mine_ms_to_frames(BURTON_CHARGE_UNPACK_TIME_MS)
        && (BURTON_FLEE_RANGE_AFTER_CHARGE - 100.0).abs() < 0.01
        && BURTON_REMOTE_CHARGE_OBJECT == "RemoteC4Charge"
        && BURTON_TIMED_CHARGE_OBJECT == "TimedC4Charge"
        && TIMED_C4_LIFETIME_FRAMES == 600
        && TNT_STICKY_LIFETIME_FRAMES == 300
        && TANK_HUNTER_TNT_OBJECT == "TNTStickyBomb"
        && can_place_remote_charge(0)
        && can_place_remote_charge(7)
        && !can_place_remote_charge(8)
        && can_place_timed_charge(9)
        && !can_place_timed_charge(10)
}

/// Wave 51 residual honesty: ClusterMines DropVariance + OCL residual constants.
pub fn honesty_cluster_mines_ocl_residual_ok() -> bool {
    CLUSTER_MINES_DROP_VARIANCE == (20.0, 20.0, 0.0)
        && (CLUSTER_MINES_DELIVERY_DISTANCE - 140.0).abs() < 0.01
        && (CLUSTER_MINES_DELIVERY_DECAL_RADIUS - 100.0).abs() < 0.01
        && (CLUSTER_MINES_RADIUS_CURSOR - 100.0).abs() < 0.01
        && (CLUSTER_MINES_DISTANCE_AROUND_OBJECT - 80.0).abs() < 0.01
        && CLUSTER_MINE_NUM_VIRTUAL == 8
        && CLUSTER_MINE_COUNT > 0
        && CLUSTER_MINE_COUNT as u32 <= CLUSTER_MINE_NUM_VIRTUAL
        && CLUSTER_MINES_OCL_TRANSPORT == "ChinaJetCargoPlane"
        && CLUSTER_MINES_BOMB_TEMPLATE == "ClusterMinesBomb"
        && CLUSTER_MINES_MINE_TEMPLATE == "ChinaClusterMine"
        && SUPERWEAPON_CLUSTER_MINES_RELOAD_MS == 240_000
        && SUPERWEAPON_CLUSTER_MINES_RELOAD_FRAMES
            == mine_ms_to_frames(SUPERWEAPON_CLUSTER_MINES_RELOAD_MS)
}

/// Combined Wave 51 mines residual honesty pack.
pub fn honesty_mines_residual_pack_ok() -> bool {
    honesty_demo_trap_mode_residual_ok()
        && honesty_mine_clear_residual_ok()
        && honesty_demo_trap_weapon_rings_residual_ok()
        && honesty_burton_charge_max_residual_ok()
        && honesty_cluster_mines_ocl_residual_ok()
        && MINE_SCOOT_FROM_STARTING_POINT_MS == 1_000
        && MINE_SCOOT_FROM_STARTING_POINT_FRAMES
            == mine_ms_to_frames(MINE_SCOOT_FROM_STARTING_POINT_MS)
        && HostMineData::land_mine_for_template("ChinaStandardMine").scoot_from_starting_point_time
            == MINE_SCOOT_FROM_STARTING_POINT_FRAMES
        && HostMineData::land_mine().scoot_from_starting_point_time == 0
}

/// Wave 78 residual honesty: ClusterMines DeliveryDecal / ViewObject / science residual deepen.
///
/// Fail-closed: not full OCL ClusterMinesBomb aircraft path / GenerateMinefieldBehavior SmartBorder.
pub fn honesty_cluster_mines_residual_pack_wave78() -> bool {
    SUPERWEAPON_CLUSTER_MINES == "SuperweaponClusterMines"
        && CLUSTER_MINES_OCL == "SUPERWEAPON_ClusterMines"
        && SCIENCE_CLUSTER_MINES == "SCIENCE_ClusterMines"
        && CLUSTER_MINES_SCIENCE_POINT_COST == 1
        && CLUSTER_MINES_PREREQ_SCIENCES == ["SCIENCE_CHINA", "SCIENCE_Rank3"]
        && CLUSTER_MINES_VIEW_OBJECT_DURATION_MS == 30_000
        && CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES == 900
        && mine_ms_to_frames(CLUSTER_MINES_VIEW_OBJECT_DURATION_MS)
            == CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES
        && (CLUSTER_MINES_VIEW_OBJECT_RANGE - 250.0).abs() < 0.01
        && CLUSTER_MINES_DROP_OFFSET == (0.0, 0.0, -2.0)
        && CLUSTER_MINES_MAX_ATTEMPTS == 4
        && CLUSTER_MINES_DECAL_TEXTURE == "SCCClusterMines_China"
        && CLUSTER_MINES_DECAL_STYLE == "SHADOW_ALPHA_DECAL"
        && CLUSTER_MINES_DECAL_OPACITY_MIN_PCT == 25
        && CLUSTER_MINES_DECAL_OPACITY_MAX_PCT == 50
        && CLUSTER_MINES_DECAL_THROB_MS == 500
        && CLUSTER_MINES_DECAL_COLOR == (255, 156, 0, 255)
        && CLUSTER_MINES_PAYLOAD_COUNT == 1
        && (CLUSTER_MINES_DELIVERY_DECAL_RADIUS - CLUSTER_MINES_RADIUS_CURSOR).abs() < 0.01
        && honesty_cluster_mines_ocl_residual_ok()
}

/// Simple distance falloff: full damage inside half-radius, linear to edge.
pub fn damage_at_distance(base_damage: f32, radius: f32, distance: f32) -> f32 {
    if radius <= 0.0 || distance > radius {
        return 0.0;
    }
    let half = radius * 0.5;
    if distance <= half {
        base_damage
    } else {
        let t = (distance - half) / (radius - half).max(0.001);
        base_damage * (1.0 - t).max(0.0)
    }
}

/// C++ GenerateMinefieldBehavior::upgradeImplementation residual.
/// China mines upgrade places mines around the structure footprint.
pub const CHINA_STRUCTURE_MINE_RING_COUNT: u32 = 8;
pub const CHINA_STRUCTURE_MINE_RING_RADIUS: f32 = 40.0;
pub const CHINA_STANDARD_MINE_TEMPLATE: &str = "ChinaStandardMine";
pub const CHINA_EMP_MINE_TEMPLATE: &str = "ChinaEMPMine";
pub const UPGRADE_CHINA_MINES: &str = "Upgrade_ChinaMines";
pub const UPGRADE_CHINA_EMP_MINES: &str = "Upgrade_ChinaEMPMines";

pub fn is_china_emp_mines_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    (n.contains("chinaempmines") || n.contains("china_emp_mines") || n.contains("empmines"))
        && n.contains("emp")
        && !n.contains("cluster")
}

pub fn is_china_standard_mines_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    (n.contains("chinamines") || n.contains("china_mines"))
        && !n.contains("emp")
        && !n.contains("cluster")
}

pub fn is_china_mines_upgrade(upgrade: &str) -> bool {
    is_china_standard_mines_upgrade(upgrade) || is_china_emp_mines_upgrade(upgrade)
}

/// C++ MineName vs MineNameUpgraded residual.
pub fn china_mine_template_for_upgrade(upgrade: &str) -> &'static str {
    if is_china_emp_mines_upgrade(upgrade) {
        CHINA_EMP_MINE_TEMPLATE
    } else {
        CHINA_STANDARD_MINE_TEMPLATE
    }
}

/// World positions for structure mine ring residual (evenly spaced on circle).
pub fn structure_minefield_positions(
    center: glam::Vec3,
    count: u32,
    radius: f32,
) -> Vec<glam::Vec3> {
    if count == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let a = (i as f32) * std::f32::consts::TAU / (count as f32);
        out.push(glam::Vec3::new(
            center.x + radius * a.cos(),
            center.y,
            center.z + radius * a.sin(),
        ));
    }
    out
}

/// Structure BorderOnly residual from building footprint (not a fixed 8@40 ring).
pub fn structure_minefield_positions_for_geom(
    center: glam::Vec3,
    major_radius: f32,
    minor_radius: f32,
    is_box: bool,
) -> Vec<glam::Vec3> {
    structure_border_mine_positions(
        MinePlacementGeom {
            center,
            major_radius: major_radius.max(1.0),
            minor_radius: minor_radius.max(1.0),
            is_box,
        },
        STANDARD_MINEFIELD_DISTANCE,
        LAND_MINE_GEOMETRY_RADIUS,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn land_mine_defaults_proximity() {
        let d = HostMineData::land_mine();
        assert!(d.proximity_enabled);
        assert!(d.trigger_range > 0.0);
        assert!(d.detonation_damage > 0.0);
        assert!(d.detonate_at_frame.is_none());
    }

    #[test]
    fn demo_trap_weapon_slot_mode_residual() {
        // DefaultProximityMode Yes residual.
        let mut d = HostMineData::demo_trap();
        assert_eq!(d.demo_trap_mode, DemoTrapMode::Proximity);
        assert!(d.proximity_enabled);
        assert!(DemoTrapMode::Proximity.proximity_enabled());
        assert!(!DemoTrapMode::Manual.proximity_enabled());
        assert!(DemoTrapMode::Detonate.is_detonate_command());

        // ManualModeWeaponSlot residual.
        assert!(d.set_demo_trap_mode(DemoTrapMode::Manual));
        assert_eq!(d.demo_trap_mode, DemoTrapMode::Manual);
        assert!(!d.proximity_enabled);

        // Back to ProximityModeWeaponSlot residual.
        assert!(d.set_demo_trap_mode(DemoTrapMode::Proximity));
        assert_eq!(d.demo_trap_mode, DemoTrapMode::Proximity);
        assert!(d.proximity_enabled);

        // DetonationWeaponSlot residual: signals detonate + proximity off.
        assert!(d.set_demo_trap_mode(DemoTrapMode::Detonate));
        assert!(!d.proximity_enabled);
        assert_eq!(d.demo_trap_mode, DemoTrapMode::Manual);

        // Land mines reject demo-trap mode residual.
        let mut land = HostMineData::land_mine();
        assert!(!land.set_demo_trap_mode(DemoTrapMode::Manual));
        assert!(land.proximity_enabled);
    }

    #[test]
    fn timed_charge_schedules_frame() {
        let d = HostMineData::timed_demo_charge(10);
        assert!(!d.proximity_enabled);
        assert_eq!(d.detonate_at_frame, Some(310));
    }

    #[test]
    fn remote_charge_has_no_auto_timer() {
        let d = HostMineData::remote_demo_charge();
        assert!(!d.proximity_enabled);
        assert!(d.detonate_at_frame.is_none());
        assert_eq!(d.kind, HostMineKind::RemoteDemoCharge);
        assert!(d.detonation_damage > 0.0);
    }

    #[test]
    fn cluster_ring_count() {
        let pts = cluster_mine_positions(Vec3::ZERO, CLUSTER_MINE_COUNT, CLUSTER_MINE_RING_RADIUS);
        assert_eq!(pts.len(), CLUSTER_MINE_COUNT);
        for p in &pts {
            let dist = (p.x * p.x + p.z * p.z).sqrt();
            assert!((dist - CLUSTER_MINE_RING_RADIUS).abs() < 0.01);
        }
    }

    #[test]
    fn demo_trap_profile_matrix() {
        assert_eq!(
            demo_trap_profile("GLADemoTrap", false, false),
            DemoTrapProfile::Standard
        );
        assert_eq!(
            demo_trap_profile("TestDemoTrap", false, false),
            DemoTrapProfile::Standard
        );
        assert_eq!(
            demo_trap_profile("Chem_GLADemoTrap", false, false),
            DemoTrapProfile::ChemBeta
        );
        assert_eq!(
            demo_trap_profile("Chem_GLADemoTrap", true, false),
            DemoTrapProfile::ChemGamma
        );
        assert_eq!(
            demo_trap_profile("Demo_GLADemoTrap", false, false),
            DemoTrapProfile::Demo
        );
        let chem = HostMineData::demo_trap_with_profile(DemoTrapProfile::ChemGamma);
        assert!((chem.detonation_damage - 250.0).abs() < 0.01);
        assert!((chem.secondary_damage - 100.0).abs() < 0.01);
        assert!(chem.demo_trap_profile.spawns_poison());
        let demo = HostMineData::demo_trap_with_profile(DemoTrapProfile::Demo);
        assert!((demo.detonation_damage - 700.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::Demo, 0.0) - 700.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::Demo, 30.0) - 500.0).abs() < 0.01);
        assert_eq!(
            residual_data_for_template("Chem_GLADemoTrap", 0, false)
                .unwrap()
                .demo_trap_profile,
            DemoTrapProfile::ChemBeta
        );
        assert_eq!(
            residual_data_for_template("Chem_GLADemoTrap", 0, true)
                .unwrap()
                .demo_trap_profile,
            DemoTrapProfile::ChemGamma
        );
    }

    #[test]
    fn infer_templates() {
        assert_eq!(
            infer_mine_kind("ChinaStandardMine"),
            Some(HostMineKind::LandMine)
        );
        assert_eq!(
            infer_mine_kind("ChinaClusterMine"),
            Some(HostMineKind::LandMine)
        );
        assert_eq!(infer_mine_kind("GLADemoTrap"), Some(HostMineKind::DemoTrap));
        assert_eq!(
            infer_mine_kind("TNTStickyBomb"),
            Some(HostMineKind::TimedDemoCharge)
        );
        assert_eq!(
            infer_mine_kind("TestRemoteDemoCharge"),
            Some(HostMineKind::RemoteDemoCharge)
        );
        assert_eq!(infer_mine_kind("AmericaRanger"), None);
    }

    #[test]
    fn land_mine_trips_on_geometry_contact_not_center_only() {
        let mine = Vec3::new(0.0, 0.0, 0.0);
        let mine_r = LAND_MINE_GEOMETRY_RADIUS;
        assert!((mine_r - 30.0).abs() < f32::EPSILON);
        // Center 20 wu away, victim r=8: overlap (30+8=38).
        assert!(land_mine_geometry_contacts(
            mine,
            mine_r,
            Vec3::new(20.0, 0.0, 0.0),
            8.0
        ));
        // Center 40 wu away, victim r=8: no overlap (38).
        assert!(!land_mine_geometry_contacts(
            mine,
            mine_r,
            Vec3::new(40.0, 0.0, 0.0),
            8.0
        ));
        // Zero-radius victim still needs center inside the mine disk.
        assert!(land_mine_geometry_contacts(
            mine,
            mine_r,
            Vec3::new(30.0, 0.0, 0.0),
            0.0
        ));
        assert!(!land_mine_geometry_contacts(
            mine,
            mine_r,
            Vec3::new(30.01, 0.0, 0.0),
            0.0
        ));
        assert!((victim_mine_collide_radius(true, 12.0, 8.0) - 12.0).abs() < f32::EPSILON);
        assert!((victim_mine_collide_radius(false, 1.0, 8.0) - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn can_clear_includes_remote_charge() {
        assert!(can_clear_mine_kind(HostMineKind::RemoteDemoCharge));
        assert!(can_clear_mine_kind(HostMineKind::TimedDemoCharge));
    }

    #[test]
    fn damage_falloff_full_then_zero() {
        assert!((damage_at_distance(100.0, 10.0, 0.0) - 100.0).abs() < 0.01);
        assert!((damage_at_distance(100.0, 10.0, 4.0) - 100.0).abs() < 0.01);
        assert_eq!(damage_at_distance(100.0, 10.0, 11.0), 0.0);
    }

    #[test]
    fn mine_clearer_helpers() {
        assert!(is_mine_clearer(true, "TestInfantry"));
        assert!(is_mine_clearer(false, "USA_Dozer"));
        assert!(is_mine_clearer(false, "GLA_Worker"));
        assert!(!is_mine_clearer(false, "USA_Ranger"));
        assert!(can_clear_mine_kind(HostMineKind::LandMine));
        assert!(can_clear_mine_kind(HostMineKind::DemoTrap));
        assert!(DOZER_MINE_CLEAR_RANGE > 0.0);
        assert!(DOZER_MINE_CLEAR_SCAN_RANGE > DOZER_MINE_CLEAR_RANGE);
        // C++ DemoTrapUpdate.cpp:181-191 — idle dozer is not skipped.
        assert!(!demo_trap_skips_dozer_disarm_while_attacking(
            true, true, false
        ));
        assert!(!demo_trap_skips_dozer_disarm_while_attacking(
            true, false, true
        ));
        assert!(demo_trap_skips_dozer_disarm_while_attacking(
            true, true, true
        ));
        assert!(!demo_trap_skips_dozer_disarm_while_attacking(
            false, true, true
        ));
        // C++ DemoTrapUpdate.cpp:196 — ENEMIES only.
        assert!(demo_trap_proximity_requires_enemies(true));
        assert!(!demo_trap_proximity_requires_enemies(false));
        // C++ GameLogicDispatch.cpp:594 — no idle auto-clear.
        assert!(!mine_clear_allowed_for_order(false));
        assert!(mine_clear_allowed_for_order(true));
    }

    #[test]
    fn demo_trap_mode_residual_honesty() {
        assert!(honesty_demo_trap_mode_residual_ok());
        assert_eq!(
            DEMO_TRAP_DESTRUCTION_DELAY_FRAMES,
            mine_ms_to_frames(DEMO_TRAP_DESTRUCTION_DELAY_MS)
        );
        let mut d = HostMineData::demo_trap();
        assert_eq!(d.demo_trap_mode, DemoTrapMode::Proximity);
        assert!(d.set_demo_trap_mode(DemoTrapMode::Manual));
        assert!(!d.proximity_enabled);
        assert!(d.set_demo_trap_mode(DemoTrapMode::Proximity));
        assert!(d.proximity_enabled);
    }

    #[test]
    fn mine_clear_disarm_distance_time_residual_honesty() {
        assert!(honesty_mine_clear_residual_ok());
        assert_eq!(DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES, 36);
        assert_eq!(DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES, 120);
        assert_eq!(WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES, 30);
        assert_eq!(WORKER_MINE_CLEAR_DELAY_FRAMES, 30);
        assert!((DOZER_MINE_CLEAR_RANGE - 5.0).abs() < 0.01);
        assert!((DOZER_MINE_CLEAR_SCAN_RANGE - 100.0).abs() < 0.01);
    }

    #[test]
    fn demo_trap_weapon_secondary_rings_residual_honesty() {
        assert!(honesty_demo_trap_weapon_rings_residual_ok());
        // Chem / Demo dual-ring residual.
        assert!((demo_trap_damage_at(DemoTrapProfile::ChemGamma, 10.0) - 250.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::ChemGamma, 40.0) - 100.0).abs() < 0.01);
        assert_eq!(demo_trap_damage_at(DemoTrapProfile::ChemGamma, 60.0), 0.0);
        // Standard secondary damage constant residual (soft falloff host path).
        assert!((DemoTrapProfile::Standard.secondary_damage() - 400.0).abs() < 0.01);
    }

    #[test]
    fn standard_demo_trap_uses_dual_ring_600_25_and_400_50() {
        // Given: Weapon.ini DemoTrapDetonationWeapon Primary 600 r25 + Secondary 400 r50
        // When: damage is sampled at inner / mid / outer / beyond
        // Then: full 600 inside 25, 400 from 25–50, 0 beyond 50 (no soft falloff)
        assert!(DemoTrapProfile::Standard.uses_dual_ring());
        assert!((demo_trap_damage_at(DemoTrapProfile::Standard, 0.0) - 600.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::Standard, 25.0) - 600.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::Standard, 25.01) - 400.0).abs() < 0.01);
        assert!((demo_trap_damage_at(DemoTrapProfile::Standard, 50.0) - 400.0).abs() < 0.01);
        assert_eq!(demo_trap_damage_at(DemoTrapProfile::Standard, 50.01), 0.0);
        let data = HostMineData::demo_trap_with_profile(DemoTrapProfile::Standard);
        assert!((data.detonation_radius - 50.0).abs() < 0.01);
        assert!((data.secondary_damage - 400.0).abs() < 0.01);
    }

    #[test]
    fn burton_sticky_max_count_residual_honesty() {
        assert!(honesty_burton_charge_max_residual_ok());
        assert!(!can_place_remote_charge(BURTON_MAX_REMOTE_CHARGES));
        assert!(!can_place_timed_charge(BURTON_MAX_TIMED_CHARGES));
        assert_eq!(BURTON_CHARGE_UNPACK_TIME_FRAMES, 165);
    }

    #[test]
    fn cluster_mines_drop_variance_ocl_residual_honesty() {
        assert!(honesty_cluster_mines_ocl_residual_ok());
        let center = Vec3::new(100.0, 0.0, 200.0);
        // unit 0.5 → no offset residual mid-sample
        let mid = apply_cluster_mines_drop_variance(center, 0.5, 0.5);
        assert!((mid.x - 100.0).abs() < 0.01);
        assert!((mid.z - 200.0).abs() < 0.01);
        // unit 1.0 → +variance on X/Y residual
        let hi = apply_cluster_mines_drop_variance(center, 1.0, 1.0);
        assert!((hi.x - 120.0).abs() < 0.01);
        assert!((hi.z - 220.0).abs() < 0.01);
        // unit 0.0 → -variance
        let lo = apply_cluster_mines_drop_variance(center, 0.0, 0.0);
        assert!((lo.x - 80.0).abs() < 0.01);
        assert!((lo.z - 180.0).abs() < 0.01);
        // Z variance residual is 0
        assert!((hi.y - center.y).abs() < 0.01);
    }

    #[test]
    fn cluster_mines_residual_pack_wave78_honesty() {
        assert!(honesty_cluster_mines_residual_pack_wave78());
        assert_eq!(CLUSTER_MINES_DECAL_TEXTURE, "SCCClusterMines_China");
        assert_eq!(CLUSTER_MINES_DECAL_COLOR, (255, 156, 0, 255));
        assert_eq!(SCIENCE_CLUSTER_MINES, "SCIENCE_ClusterMines");
        assert_eq!(CLUSTER_MINES_MAX_ATTEMPTS, 4);
        assert_eq!(CLUSTER_MINES_DROP_OFFSET, (0.0, 0.0, -2.0));
        assert_eq!(CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES, 900);
        assert_eq!(CLUSTER_MINES_SCIENCE_POINT_COST, 1);
    }

    #[test]
    fn mines_residual_pack_honesty() {
        assert!(honesty_mines_residual_pack_ok());
    }

    /// Wave 72 residual pack honesty gate (wrapper residual_pack_ok).
    #[test]
    fn mines_residual_pack_honesty_wave72() {
        assert!(honesty_mines_residual_pack_ok());
        assert!(honesty_demo_trap_mode_residual_ok());
        assert!(honesty_cluster_mines_ocl_residual_ok());
        assert_eq!(BURTON_MAX_REMOTE_CHARGES, 8);
        assert_eq!(SUPERWEAPON_CLUSTER_MINES_RELOAD_FRAMES, 7_200);
    }

    #[test]
    fn structure_mine_ring_has_8() {
        let pts = structure_minefield_positions(glam::Vec3::ZERO, 8, 40.0);
        assert_eq!(pts.len(), 8);
        let d0 = (pts[0].x * pts[0].x + pts[0].z * pts[0].z).sqrt();
        assert!((d0 - 40.0).abs() < 0.01);
    }

    #[test]
    fn demo_trap_detonate_when_killed_policy() {
        // C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
        assert!(should_detonate_when_killed(
            HostMineKind::DemoTrap,
            DEMO_TRAP_DETONATE_WHEN_KILLED,
            true,
            false,
            false,
        ));
        assert!(!should_detonate_when_killed(
            HostMineKind::DemoTrap,
            DEMO_TRAP_DETONATE_WHEN_KILLED,
            false,
            false,
            false,
        ));
        assert!(!should_detonate_when_killed(
            HostMineKind::LandMine,
            DEMO_TRAP_DETONATE_WHEN_KILLED,
            true,
            false,
            false,
        ));
        assert!(!should_detonate_when_killed(
            HostMineKind::DemoTrap,
            DEMO_TRAP_DETONATE_WHEN_KILLED,
            true,
            true,
            false,
        ));
        assert!(!should_detonate_when_killed(
            HostMineKind::DemoTrap,
            DEMO_TRAP_DETONATE_WHEN_KILLED,
            true,
            false,
            true,
        ));
        let d = HostMineData::demo_trap();
        assert!(d.detonate_when_killed());
    }

    #[test]
    fn virtual_mines_persist_until_empty() {
        let mut d = HostMineData::land_mine_for_template("ChinaClusterMine");
        assert_eq!(d.virtual_mines_remaining, CLUSTER_MINE_NUM_VIRTUAL);
        assert!(!d.consume_virtual_mine());
        assert_eq!(d.virtual_mines_remaining, CLUSTER_MINE_NUM_VIRTUAL - 1);
        assert!(d.is_active());
        for _ in 1..CLUSTER_MINE_NUM_VIRTUAL {
            let last = d.consume_virtual_mine();
            if d.virtual_mines_remaining == 0 {
                assert!(last);
            } else {
                assert!(!last);
            }
        }
        assert_eq!(d.virtual_mines_remaining, 0);
        assert!(!d.is_active());
    }

    #[test]
    fn workers_do_not_detonate_china_mines() {
        assert!(minefield_skips_worker(false, true, true));
        assert!(!minefield_skips_worker(true, true, true));
        assert!(!minefield_skips_worker(false, true, false));
        assert!(!minefield_skips_worker(false, false, true));
    }

    #[test]
    fn clearing_dozer_with_goal_is_immune() {
        assert!(is_clearing_mines_with_goal(true, true));
        assert!(!is_clearing_mines_with_goal(true, false));
        assert!(!is_clearing_mines_with_goal(false, true));
    }

    #[test]
    fn repeat_detonate_requires_move() {
        let mut d = HostMineData::land_mine();
        let id = ObjectId(7);
        assert!(d.allow_repeat_detonate(id, Vec3::ZERO));
        assert!(!d.allow_repeat_detonate(id, Vec3::new(0.5, 0.0, 0.0)));
        assert!(d.allow_repeat_detonate(id, Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn sticky_immobile_keeps_plant_xy() {
        let planted = Vec3::new(12.0, 3.0, 44.0);
        let follow = sticky_immobile_follow_pos(planted, 0.0);
        assert!((follow.x - 12.0).abs() < 0.01);
        assert!((follow.z - 44.0).abs() < 0.01);
        assert!(follow.y.abs() < 0.01);
        let veh = sticky_vehicle_follow_pos(Vec3::new(1.0, 2.0, 3.0), STICKY_OFFSET_Z);
        assert!((veh.y - 10.0).abs() < 0.01);
    }

    #[test]
    fn unit_bomb_ping_interval_is_one_second() {
        assert_eq!(UNIT_BOMB_PING_FRAMES, 30);
        assert_eq!(sticky_next_ping_frame(10, None), 40);
        assert_eq!(sticky_next_ping_frame(10, Some(310)), 10);
        assert_eq!(UNIT_BOMB_PING_AUDIO, "UnitBombPing");
        assert_eq!(STICKY_BOMB_CREATED_AUDIO, "StickyBombCreated");
    }

    #[test]
    fn smart_border_is_denser_than_fixed_ring() {
        let pts = cluster_smart_border_positions(Vec3::ZERO);
        let max_r = pts
            .iter()
            .map(|p| (p.x * p.x + p.z * p.z).sqrt())
            .fold(0.0_f32, f32::max);
        assert!(
            max_r + 0.01 >= CLUSTER_MINES_DISTANCE_AROUND_OBJECT - LAND_MINE_GEOMETRY_RADIUS * 2.0
        );
        assert!(
            max_r < CLUSTER_MINES_DISTANCE_AROUND_OBJECT,
            "SmartBorder must not place a ring at expanded radius beyond DistanceAroundObject, max_r={max_r}"
        );
        assert!(
            (max_r - 31.0).abs() < 0.01,
            "Cluster Mines outer SmartBorder ring must be 31 with geometry 30, got {max_r}"
        );
    }

    #[test]
    fn structure_border_uses_footprint_not_fixed_count() {
        let small = structure_minefield_positions_for_geom(Vec3::ZERO, 8.0, 8.0, false);
        let large = structure_minefield_positions_for_geom(Vec3::ZERO, 40.0, 25.0, true);
        assert!(small.len() >= CHINA_STRUCTURE_MINE_RING_COUNT as usize);
        assert!(large.len() > small.len());
    }

    #[test]
    fn emp_upgrade_selects_emp_template() {
        assert!(is_china_mines_upgrade("Upgrade_ChinaMines"));
        assert!(is_china_mines_upgrade("Upgrade_ChinaEMPMines"));
        assert!(is_china_standard_mines_upgrade("Upgrade_ChinaMines"));
        assert!(!is_china_standard_mines_upgrade("Upgrade_ChinaEMPMines"));
        assert!(is_china_emp_mines_upgrade("Upgrade_ChinaEMPMines"));
        assert!(!is_china_emp_mines_upgrade("Upgrade_ChinaMines"));
        assert_eq!(
            china_mine_template_for_upgrade("Upgrade_ChinaMines"),
            CHINA_STANDARD_MINE_TEMPLATE
        );
        assert_eq!(
            china_mine_template_for_upgrade("Upgrade_ChinaEMPMines"),
            CHINA_EMP_MINE_TEMPLATE
        );
    }

    #[test]
    fn regenerating_disarm_keeps_pad_and_refills_virtual() {
        let mut d = HostMineData::land_mine_for_template("ChinaStandardMine");
        assert!(d.regenerates);
        assert_eq!(d.virtual_mines_remaining, STANDARD_MINE_NUM_VIRTUAL);
        assert!(d.disarm_regenerating_pad());
        assert_eq!(d.virtual_mines_remaining, 0);
        assert!(d.regenerates);
        assert!(!d.consume_virtual_mine());
        // Healing 50% of max → floor(8 * 0.5) = 4 virtual.
        assert_eq!(
            d.apply_on_damage_step(50.0, 100.0, true, false),
            MineOnDamageStep::Done
        );
        assert_eq!(d.virtual_mines_remaining, 4);
        let healed = d.tick_regen_auto_heal(0, 0.1, 100.0);
        assert!((healed - 0.1).abs() < 1e-4, "first pulse only arms wake");
        let healed = d.tick_regen_auto_heal(MINE_AUTO_HEAL_DELAY_FRAMES, 0.1, 100.0);
        assert!((healed - (0.1 + MINE_AUTO_HEAL_AMOUNT)).abs() < 1e-4);
    }

    #[test]
    fn weapon_damage_trips_virtual_band() {
        let mut d = HostMineData::land_mine_for_template("ChinaStandardMine");
        // 8 virtual, 100 HP → 87.5 health expects ceil(7.0)=7 → one detonate.
        assert_eq!(
            d.apply_on_damage_step(87.5, 100.0, false, false),
            MineOnDamageStep::Detonate
        );
        assert_eq!(d.virtual_mines_remaining, 8);
        assert!(!d.consume_virtual_mine());
        assert_eq!(d.virtual_mines_remaining, 7);
        assert_eq!(
            d.apply_on_damage_step(87.5, 100.0, false, false),
            MineOnDamageStep::Done
        );
    }

    #[test]
    fn lethal_health_expects_zero_virtuals_and_defers_destroy() {
        let mut d = HostMineData::land_mine_for_template("ChinaStandardMine");
        assert!(d.defers_lethal_body_destroy());
        assert_eq!(d.virtual_mines_expected_from_health(0.0, 100.0, false), 0);
        assert_eq!(
            d.apply_on_damage_step(0.0, 100.0, false, false),
            MineOnDamageStep::Detonate
        );
        assert_eq!(d.virtual_mines_remaining, STANDARD_MINE_NUM_VIRTUAL);
    }

    #[test]
    fn producer_death_stops_regen_and_drains() {
        let mut d = HostMineData::land_mine_for_template("ChinaEMPMine");
        d.note_producer_dead(10);
        assert!(!d.regenerates);
        assert!(d.draining);
        assert!(d.auto_heal_stopped);
        assert!(d.drain_health_amount(100.0) > 0.0);
        assert_eq!(
            d.apply_on_damage_step(50.0, 100.0, false, true),
            MineOnDamageStep::Continue
        );
        assert_eq!(d.virtual_mines_remaining, STANDARD_MINE_NUM_VIRTUAL - 1);
    }

    #[test]
    fn land_mines_trip_neutral_demo_traps_do_not() {
        assert!(land_mine_proximity_trips(true, false));
        assert!(land_mine_proximity_trips(false, true));
        assert!(!land_mine_proximity_trips(false, false));
        assert!(!demo_trap_proximity_requires_enemies(false));
        assert!(demo_trap_proximity_requires_enemies(true));
    }

    #[test]
    fn clear_pre_attack_waits_then_ready() {
        let mut d = HostMineData::land_mine();
        let cid = ObjectId(3);
        assert!(!d.begin_or_ready_clear_pre_attack(cid, 10, 36));
        assert!(!d.begin_or_ready_clear_pre_attack(cid, 45, 36));
        assert!(d.begin_or_ready_clear_pre_attack(cid, 46, 36));
        assert_eq!(
            mine_clear_pre_attack_frames(false, "AmericaVehicleDozer"),
            36
        );
        assert_eq!(mine_clear_pre_attack_frames(true, "GLAInfantryWorker"), 30);
        assert_eq!(mine_clear_pre_attack_frames(true, "TestDozer"), 36);
    }

    #[test]
    fn cluster_drop_samples_are_not_always_midpoint() {
        let (a, b) = cluster_mines_drop_unit_samples(1);
        let (c, d) = cluster_mines_drop_unit_samples(99);
        assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
        assert!(
            (a - 0.5).abs() > 1e-3 || (b - 0.5).abs() > 1e-3 || (c - 0.5).abs() > 1e-3,
            "DropVariance samples must scatter"
        );
        let _ = d;
    }

    #[test]
    fn china_pad_geometry_radius_is_retail_30() {
        assert!((LAND_MINE_GEOMETRY_RADIUS - 30.0).abs() < f32::EPSILON);
        assert!((HostMineKind::LandMine.default_trigger_range() - 30.0).abs() < f32::EPSILON);
        assert!(land_mine_geometry_contacts(
            Vec3::ZERO,
            LAND_MINE_GEOMETRY_RADIUS,
            Vec3::new(20.0, 0.0, 0.0),
            8.0
        ));
    }

    #[test]
    fn demo_trap_disarm_is_allowed() {
        assert!(can_clear_mine_kind(HostMineKind::DemoTrap));
    }

    #[test]
    fn demo_trap_proximity_skips_above_terrain_not_aircraft_kind() {
        assert!(is_above_terrain(1.0, 0.0));
        assert!(!is_above_terrain(0.0, 0.0));
        assert!(!is_above_terrain(0.0, 5.0));
    }

    #[test]
    fn demo_trap_warning_fuse_is_one_second() {
        let mut d = HostMineData::demo_trap();
        assert!(d.arm_demo_trap_warning(10));
        assert!(d.demo_trap_warning_armed());
        assert!(!d.demo_trap_warning_ready(10));
        assert!(!d.demo_trap_warning_ready(39));
        assert!(d.demo_trap_warning_ready(40));
        assert!(!d.proximity_enabled);
        assert!(!d.arm_demo_trap_warning(10));
        assert_eq!(DEMO_TRAP_WARNING_AUDIO, "GLADemoTrapWarning");
    }

    #[test]
    fn emp_mine_is_neutron_blast_not_he() {
        let emp = HostMineData::land_mine_for_template("ChinaEMPMine");
        assert!(emp.neutron_blast);
        assert!((emp.detonation_damage - 1.0).abs() < 0.01);
        assert!((emp.detonation_radius - 20.0).abs() < 0.01);
        let std = HostMineData::land_mine_for_template("ChinaStandardMine");
        assert!(!std.neutron_blast);
        assert!((std.detonation_damage - 100.0).abs() < 0.01);
        assert_eq!(
            neutron_mine_victim_effect(10.0, false, false, true, false, false, false),
            NeutronMineVictimEffect::KillInfantry
        );
        assert_eq!(
            neutron_mine_victim_effect(10.0, false, false, false, true, false, false),
            NeutronMineVictimEffect::UnmanVehicle
        );
        assert_eq!(
            neutron_mine_victim_effect(10.0, true, false, false, true, false, false),
            NeutronMineVictimEffect::None
        );
        assert_eq!(
            neutron_mine_victim_effect(10.0, false, true, true, false, false, false),
            NeutronMineVictimEffect::None
        );
        assert_eq!(
            neutron_mine_victim_effect(21.0, false, false, true, false, false, false),
            NeutronMineVictimEffect::None
        );
        assert_eq!(
            neutron_mine_victim_effect(10.0, false, false, false, true, false, true),
            NeutronMineVictimEffect::KillCliffJumper
        );
        assert_eq!(
            neutron_mine_victim_effect(10.0, false, false, false, true, true, false),
            NeutronMineVictimEffect::None
        );
    }
}
