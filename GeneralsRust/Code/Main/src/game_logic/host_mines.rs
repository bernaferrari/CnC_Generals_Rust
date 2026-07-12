//! Host mine / demo-trap / demo-charge residual.
//!
//! Residual slice (playability):
//! - Place land mines (ChinaStandardMine / ClusterMines special power residual)
//! - Place GLA demo traps (proximity detonation when enemies enter range)
//! - Place timed demo charges (Burton / Tank Hunter sticky residual)
//! - Enemy/neutral proximity trigger → area damage + destroy mine/trap
//! - Timed charges detonate at absolute frame
//!
//! Fail-closed honesty:
//! - Not full C++ MinefieldBehavior virtual-mine regen / scoot / immunity slots
//! - Not full DemoTrapUpdate weapon-slot mode matrix / dozer-disarm scan
//! - Not full StickyBombUpdate attach bones / geometry-based splash
//! - Not full OCL ClusterMinesBomb aircraft path

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const MINE_LOGIC_FPS: f32 = 30.0;

/// Host residual mine/trap kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostMineKind {
    /// Collision / proximity land mine (ChinaStandardMine / ChinaClusterMine).
    LandMine,
    /// GLA demo trap (DemoTrapUpdate proximity residual).
    DemoTrap,
    /// Timed demo charge (TNTStickyBomb / Burton timed residual).
    TimedDemoCharge,
}

impl HostMineKind {
    pub fn label(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "LandMine",
            HostMineKind::DemoTrap => "DemoTrap",
            HostMineKind::TimedDemoCharge => "TimedDemoCharge",
        }
    }

    /// Retail-inspired residual defaults (Weapon.ini / Object INI).
    pub fn default_trigger_range(self) -> f32 {
        match self {
            // ChinaStandardMine geometry major radius residual for trigger.
            HostMineKind::LandMine => 8.0,
            // GLADemoTrap DemoTrapUpdate TriggerDetonationRange = 40.
            HostMineKind::DemoTrap => 40.0,
            // Timed charges do not proximity-trigger by default.
            HostMineKind::TimedDemoCharge => 0.0,
        }
    }

    pub fn default_damage(self) -> f32 {
        match self {
            // StructureMineWeapon PrimaryDamage residual.
            HostMineKind::LandMine => 100.0,
            // DemoTrapDetonationWeapon PrimaryDamage residual.
            HostMineKind::DemoTrap => 600.0,
            // TNTDetonationWeapon PrimaryDamage residual.
            HostMineKind::TimedDemoCharge => 500.0,
        }
    }

    pub fn default_damage_radius(self) -> f32 {
        match self {
            // StructureMineWeapon secondary radius residual.
            HostMineKind::LandMine => 5.0,
            // DemoTrapDetonationWeapon primary radius residual.
            HostMineKind::DemoTrap => 25.0,
            // TNTDetonationWeapon secondary radius residual (observable splash).
            HostMineKind::TimedDemoCharge => 50.0,
        }
    }

    /// Default timed lifetime frames (only TimedDemoCharge uses this).
    pub fn default_lifetime_frames(self) -> Option<u32> {
        match self {
            // TNTStickyBomb LifetimeUpdate Min/MaxLifetime = 10000 ms @ 30 FPS.
            HostMineKind::TimedDemoCharge => Some(300),
            HostMineKind::LandMine | HostMineKind::DemoTrap => None,
        }
    }

    pub fn defaults_to_proximity(self) -> bool {
        match self {
            HostMineKind::LandMine | HostMineKind::DemoTrap => true,
            HostMineKind::TimedDemoCharge => false,
        }
    }

    pub fn place_audio(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "MineFieldPlaced",
            HostMineKind::DemoTrap => "DemoTrapPlaced",
            HostMineKind::TimedDemoCharge => "ColonelBurtonSetDemoCharge",
        }
    }

    pub fn detonate_audio(self) -> &'static str {
        match self {
            HostMineKind::LandMine => "ExplosionClusterMine",
            HostMineKind::DemoTrap => "DemoTrapExplosion",
            HostMineKind::TimedDemoCharge => "RemoteDemoChargeExplosion",
        }
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
    /// When true, enemies in trigger_range detonate (DemoTrap / LandMine).
    pub proximity_enabled: bool,
    pub detonated: bool,
    /// Absolute logic frame for timed detonation (TimedDemoCharge).
    pub detonate_at_frame: Option<u32>,
    /// Optional sticky target (fail-closed residual bookkeeping).
    pub attached_to: Option<ObjectId>,
    /// Source that placed this residual (producer).
    pub producer_id: Option<ObjectId>,
}

impl HostMineData {
    pub fn new(kind: HostMineKind) -> Self {
        Self {
            kind,
            trigger_range: kind.default_trigger_range(),
            detonation_damage: kind.default_damage(),
            detonation_radius: kind.default_damage_radius(),
            proximity_enabled: kind.defaults_to_proximity(),
            detonated: false,
            detonate_at_frame: None,
            attached_to: None,
            producer_id: None,
        }
    }

    pub fn land_mine() -> Self {
        Self::new(HostMineKind::LandMine)
    }

    pub fn demo_trap() -> Self {
        Self::new(HostMineKind::DemoTrap)
    }

    pub fn timed_demo_charge(current_frame: u32) -> Self {
        let mut data = Self::new(HostMineKind::TimedDemoCharge);
        let delay = HostMineKind::TimedDemoCharge
            .default_lifetime_frames()
            .unwrap_or(300);
        data.detonate_at_frame = Some(current_frame.saturating_add(delay));
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
pub const CLUSTER_MINE_COUNT: usize = 6;
pub const CLUSTER_MINE_RING_RADIUS: f32 = 40.0;

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
    n.contains("stickybomb")
        || n.contains("democharge")
        || n.contains("tntsticky")
        || n == "testtimeddemocharge"
}

/// Infer residual mine kind from template name, if any.
pub fn infer_mine_kind(template_name: &str) -> Option<HostMineKind> {
    if is_demo_trap_template(template_name) {
        Some(HostMineKind::DemoTrap)
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
        HostMineKind::DemoTrap => Some(HostMineData::demo_trap()),
        HostMineKind::TimedDemoCharge => Some(HostMineData::timed_demo_charge(current_frame)),
    }
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
    fn timed_charge_schedules_frame() {
        let d = HostMineData::timed_demo_charge(10);
        assert!(!d.proximity_enabled);
        assert_eq!(d.detonate_at_frame, Some(310));
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
        assert_eq!(infer_mine_kind("AmericaRanger"), None);
    }

    #[test]
    fn damage_falloff_full_then_zero() {
        assert!((damage_at_distance(100.0, 10.0, 0.0) - 100.0).abs() < 0.01);
        assert!((damage_at_distance(100.0, 10.0, 4.0) - 100.0).abs() < 0.01);
        assert_eq!(damage_at_distance(100.0, 10.0, 11.0), 0.0);
    }
}
