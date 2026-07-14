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
//! Fail-closed honesty:
//! - Not full C++ MinefieldBehavior virtual-mine regen / scoot / immunity slots
//! - Not full DemoTrapUpdate PreAttack scoop animation / weapon-lock UI matrix
//! - Not full WEAPONSET_MINE_CLEARING_DETAIL / Weapon AntiMine targeting matrix
//! - Not full StickyBombUpdate attach bones / geometry-based splash / live max-charge list
//! - Not full OCL ClusterMinesBomb aircraft path / GenerateMinefieldBehavior SmartBorder

use super::ObjectId;
use crate::game_logic::host_toxin_tractor::{is_chem_general_template, AnthraxResidualTier};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const MINE_LOGIC_FPS: f32 = 30.0;

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
            // ChinaStandardMine geometry major radius residual for trigger.
            HostMineKind::LandMine => 8.0,
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
            HostMineKind::LandMine
            | HostMineKind::DemoTrap
            | HostMineKind::RemoteDemoCharge => None,
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

    /// Whether dual-ring step residual is used (Chem/Demo profiles).
    /// Standard keeps legacy single-radius falloff residual for parity with
    /// existing host path.
    pub fn uses_dual_ring(self) -> bool {
        !matches!(self, Self::Standard)
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
pub fn demo_trap_profile(
    template_name: &str,
    has_gamma: bool,
    has_beta: bool,
) -> DemoTrapProfile {
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

/// Dual-ring residual damage for Chem/Demo demo traps.
pub fn demo_trap_damage_at(profile: DemoTrapProfile, distance: f32) -> f32 {
    if profile.uses_dual_ring() {
        if distance <= profile.primary_radius() {
            profile.primary_damage()
        } else if distance <= profile.secondary_radius() {
            profile.secondary_damage()
        } else {
            0.0
        }
    } else {
        // Standard residual: primary damage with soft falloff to secondary radius.
        damage_at_distance(
            profile.primary_damage(),
            profile.primary_radius(),
            distance,
        )
    }
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
}

fn default_demo_trap_mode() -> DemoTrapMode {
    DemoTrapMode::Proximity
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
        data
    }

    /// Remote demo charge: sticky until DetonateRemoteDemoCharges (no auto-timer).
    pub fn remote_demo_charge() -> Self {
        Self::new(HostMineKind::RemoteDemoCharge)
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
        self
    }

    pub fn is_active(&self) -> bool {
        !self.detonated
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostMineDetonateReason {
    Proximity,
    Timed,
    Manual,
}

/// Cluster-mine ring residual (not full OCL scatter density).
/// Host placement ring uses a residual subset of retail NumVirtualMines / DistanceAroundObject.
pub const CLUSTER_MINE_COUNT: usize = 6;
pub const CLUSTER_MINE_RING_RADIUS: f32 = 40.0;

/// Retail ChinaClusterMine MinefieldBehavior NumVirtualMines residual.
pub const CLUSTER_MINE_NUM_VIRTUAL: u32 = 8;

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

/// Whether residual unit can clear mines (C++ KINDOF_DOZER / Worker + DISARM weapon residual).
/// Fail-closed: not full weapon-set / AntiMine bit matrix.
pub fn is_mine_clearer(is_worker: bool, template_name: &str) -> bool {
    if is_worker {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    n.contains("dozer") || n.contains("worker")
}

/// Residual kinds that can be disarmed (DAMAGE_DISARM → destroy without detonation).
pub fn can_clear_mine_kind(kind: HostMineKind) -> bool {
    match kind {
        HostMineKind::LandMine
        | HostMineKind::DemoTrap
        | HostMineKind::TimedDemoCharge
        | HostMineKind::RemoteDemoCharge => true,
    }
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
        || n == "testtimeddemocharge"
}

/// Template names recognized as residual remote demo charges.
pub fn is_remote_demo_charge_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("remotedemocharge")
        || n.contains("remotecharge")
        || n.contains("remotec4")
        || n == "testremotedemocharge"
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

/// Build residual mine data for a newly created host object (if template matches).
pub fn residual_data_for_template(template_name: &str, current_frame: u32) -> Option<HostMineData> {
    match infer_mine_kind(template_name)? {
        HostMineKind::LandMine => Some(HostMineData::land_mine()),
        HostMineKind::DemoTrap => {
            // Fail-closed: no live anthrax flags at template residual bind;
            // Chem_/Demo_ prefixes select profile; gamma applied later if tags present.
            let profile = demo_trap_profile(template_name, false, false);
            Some(HostMineData::demo_trap_with_profile(profile))
        }
        HostMineKind::TimedDemoCharge => Some(HostMineData::timed_demo_charge(current_frame)),
        HostMineKind::RemoteDemoCharge => Some(HostMineData::remote_demo_charge()),
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

/// Wave 51 residual honesty: DemoTrap DefaultProximity / trigger / death fuse constants.
pub fn honesty_demo_trap_mode_residual_ok() -> bool {
    DEMO_TRAP_DEFAULT_PROXIMITY_MODE
        && (DEMO_TRAP_TRIGGER_RANGE - 40.0).abs() < 0.01
        && DEMO_TRAP_DESTRUCTION_DELAY_MS == 1_000
        && DEMO_TRAP_DESTRUCTION_DELAY_FRAMES
            == mine_ms_to_frames(DEMO_TRAP_DESTRUCTION_DELAY_MS)
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
        && DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES
            == mine_ms_to_frames(DOZER_MINE_CLEAR_PRE_ATTACK_MS)
        && DOZER_MINE_CLEAR_CLIP_RELOAD_MS == 4_000
        && DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES
            == mine_ms_to_frames(DOZER_MINE_CLEAR_CLIP_RELOAD_MS)
        && WORKER_MINE_CLEAR_PRE_ATTACK_MS == 1_000
        && WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES
            == mine_ms_to_frames(WORKER_MINE_CLEAR_PRE_ATTACK_MS)
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
        && !std.uses_dual_ring()
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
        && BURTON_CHARGE_UNPACK_TIME_FRAMES
            == mine_ms_to_frames(BURTON_CHARGE_UNPACK_TIME_MS)
        && (BURTON_FLEE_RANGE_AFTER_CHARGE - 100.0).abs() < 0.01
        && BURTON_REMOTE_CHARGE_OBJECT == "RemoteC4Charge"
        && BURTON_TIMED_CHARGE_OBJECT == "TimedC4Charge"
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
}
