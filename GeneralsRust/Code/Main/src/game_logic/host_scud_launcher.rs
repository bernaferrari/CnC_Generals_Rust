//! Host GLA SCUD Launcher residual (area attack + toxin poison field).
//!
//! Residual slice (playability):
//! - PRIMARY `SCUDLauncherGunExplosive`: area residual
//!   PrimaryDamage 300 / radius 50 + SecondaryDamage 50 / radius 100.
//! - SECONDARY `SCUDLauncherGunToxin` (Anthrax upgrade → Anthrax secondary):
//!   area residual Primary 200 / 30 + Secondary 25 / 60, then spawns residual
//!   MediumPoisonField (2 dmg / 80 radius / 30s lifetime / 500ms ticks).
//! - AttackRange 350, MinimumAttackRange 200, clip 1 / 10s reload residual.
//! - PreferredAgainst residual: secondary preferred vs infantry (toxin).
//!
//! Fail-closed honesty:
//! - Not full SCUDMissile projectile lob / PreAttackDelay PER_SHOT animation
//! - Not full salvage PlusOne/PlusTwo range/damage weapon-set matrix
//! - Not full Anthrax Gamma particle bone / salvage PlusOne-Two matrix
//! - Not network SCUD / toxin replication (network deferred)

use super::ObjectId;
use crate::game_logic::host_toxin_tractor::{is_chem_general_template, AnthraxResidualTier};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail primary explosive weapon.
pub const SCUD_GUN_EXPLOSIVE: &str = "SCUDLauncherGunExplosive";
/// Retail secondary toxin weapon.
pub const SCUD_GUN_TOXIN: &str = "SCUDLauncherGunToxin";
/// Retail secondary after Anthrax Beta.
pub const SCUD_GUN_ANTHRAX: &str = "SCUDLauncherGunAnthrax";
/// Retail Chem Anthrax Gamma primary (Chemical General SCUD).
pub const SCUD_GUN_ANTHRAX_GAMMA: &str = "Chem_SCUDLauncherGunAnthraxGamma";
/// Retail Upgrade_GLAAnthraxBeta.
pub const UPGRADE_GLA_ANTHRAX_BETA: &str = "Upgrade_GLAAnthraxBeta";
/// Retail Chem_Upgrade_GLAAnthraxGamma.
pub const UPGRADE_GLA_ANTHRAX_GAMMA: &str = "Chem_Upgrade_GLAAnthraxGamma";

/// Explosive primary residual.
pub const SCUD_EXP_PRIMARY_DAMAGE: f32 = 300.0;
pub const SCUD_EXP_PRIMARY_RADIUS: f32 = 50.0;
pub const SCUD_EXP_SECONDARY_DAMAGE: f32 = 50.0;
pub const SCUD_EXP_SECONDARY_RADIUS: f32 = 100.0;

/// Toxin / Anthrax warhead residual (blast component).
pub const SCUD_TOX_PRIMARY_DAMAGE: f32 = 200.0;
pub const SCUD_TOX_PRIMARY_RADIUS: f32 = 30.0;
pub const SCUD_TOX_SECONDARY_DAMAGE: f32 = 25.0;
pub const SCUD_TOX_SECONDARY_RADIUS: f32 = 60.0;

/// Retail AttackRange / MinimumAttackRange.
pub const SCUD_ATTACK_RANGE: f32 = 350.0;
pub const SCUD_MIN_RANGE: f32 = 200.0;
/// Retail ClipReloadTime 10000ms → 300 frames @ 30 FPS.
pub const SCUD_RELOAD_FRAMES: u32 = 300;
/// Retail PreAttackDelay 500ms → 15 frames (recorded residual only).
pub const SCUD_PRE_ATTACK_FRAMES: u32 = 15;
/// Retail ScatterRadiusVsInfantry.
pub const SCUD_SCATTER_VS_INFANTRY: f32 = 30.0;

/// Retail MediumPoisonFieldWeapon PrimaryDamage / radius / lifetime.
pub const SCUD_POISON_DAMAGE_PER_TICK: f32 = 2.0;
/// Retail Chem_MediumPoisonFieldWeaponGamma / anthrax-upgraded residual.
pub const SCUD_POISON_DAMAGE_PER_TICK_UPGRADED: f32 = 2.5;
pub const SCUD_POISON_RADIUS: f32 = 80.0;
/// DelayBetweenShots 500ms → 15 frames @ 30 FPS.
pub const SCUD_POISON_TICK_INTERVAL_FRAMES: u32 = 15;
/// LifetimeUpdate 30000ms → 900 frames @ 30 FPS.
pub const SCUD_POISON_DURATION_FRAMES: u32 = 900;

/// Residual fire audio.
pub const SCUD_FIRE_AUDIO: &str = "ScudLauncherWeapon";
/// Residual toxin ambient.
pub const SCUD_POISON_AUDIO: &str = "ToxicPoolAmbientLoop";

/// Whether template is a residual SCUD launcher vehicle.
///
/// Fail-closed: name residual (not full Salvage / DeployStyle matrix).
/// Excludes SCUD Storm superweapon / missile projectiles.
pub fn is_scud_launcher_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Superweapon / storm / missile / storm damage are not the vehicle.
    if n.contains("storm")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("damageweapon")
    {
        return false;
    }
    n.contains("scudlauncher")
        || n.contains("scud_launcher")
        || n == "gla_scudlauncher"
        || n == "testscudlauncher"
        || (n.contains("vehiclescud") && n.contains("launcher"))
}

/// Whether residual fire should apply SCUD area residual.
pub fn should_apply_scud_area(is_scud: bool) -> bool {
    is_scud
}

/// Whether residual secondary is toxin/anthrax path (spawn poison field).
///
/// Slot 1 = secondary warhead residual. Chem SCUD residual uses primary anthrax
/// (no explosive primary) — treat slot 0 as toxin when `chem_anthrax_primary`.
pub fn should_spawn_scud_toxin_field(is_scud: bool, fired_slot: u8) -> bool {
    is_scud && fired_slot == 1
}

/// Chem General SCUD residual: primary is anthrax warhead (WeaponSet PRIMARY only).
pub fn scud_uses_anthrax_primary(template_name: &str) -> bool {
    is_scud_launcher_template(template_name) && is_chem_general_template(template_name)
}

/// Whether this SCUD fire should apply toxin warhead residual (blast + poison field).
pub fn scud_toxin_warhead_for_slot(template_name: &str, fired_slot: u8) -> bool {
    if !is_scud_launcher_template(template_name) {
        return false;
    }
    fired_slot == 1 || scud_uses_anthrax_primary(template_name)
}

/// MediumPoisonField damage-per-tick residual for SCUD anthrax tier.
pub fn scud_poison_damage_per_tick(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => SCUD_POISON_DAMAGE_PER_TICK,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => {
            SCUD_POISON_DAMAGE_PER_TICK_UPGRADED
        }
    }
}

/// Prefer secondary residual vs infantry (PreferredAgainst SECONDARY INFANTRY).
pub fn scud_prefer_secondary_vs_infantry(is_scud: bool, target_is_infantry: bool) -> bool {
    is_scud && target_is_infantry
}

/// Explosive area damage at distance (max of primary/secondary rings).
pub fn scud_explosive_damage_at(distance: f32) -> f32 {
    if distance <= SCUD_EXP_PRIMARY_RADIUS {
        SCUD_EXP_PRIMARY_DAMAGE
    } else if distance <= SCUD_EXP_SECONDARY_RADIUS {
        SCUD_EXP_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Toxin warhead blast damage at distance (poison field is separate DoT).
pub fn scud_toxin_blast_damage_at(distance: f32) -> f32 {
    if distance <= SCUD_TOX_PRIMARY_RADIUS {
        SCUD_TOX_PRIMARY_DAMAGE
    } else if distance <= SCUD_TOX_SECONDARY_RADIUS {
        SCUD_TOX_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Max splash radius for a residual SCUD warhead.
pub fn scud_splash_radius(toxin_warhead: bool) -> f32 {
    if toxin_warhead {
        SCUD_TOX_SECONDARY_RADIUS
    } else {
        SCUD_EXP_SECONDARY_RADIUS
    }
}

/// Legal residual SCUD splash target.
pub fn is_legal_scud_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// 2D distance residual.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// One active residual MediumPoisonField from SCUD toxin detonation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostScudPoisonZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    /// Anthrax residual tier for this field (Beta/Gamma use upgraded DoT).
    pub anthrax_tier: AnthraxResidualTier,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostScudPoisonZone {
    pub fn anthrax_upgraded(&self) -> bool {
        self.anthrax_tier.is_upgraded()
    }

    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostScudPoisonDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one poison zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostScudPoisonTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostScudPoisonDamageHit>,
}

/// Host residual registry for SCUD MediumPoisonField zones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostScudPoisonRegistry {
    next_id: u32,
    active: Vec<HostScudPoisonZone>,
    pub zones_spawned: u32,
    pub expirations: u32,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
    /// SCUD area residual blasts (explosive or toxin warhead impact).
    pub area_blasts: u32,
    /// Units hit by SCUD area residual.
    pub units_hit: u32,
}

impl HostScudPoisonRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostScudPoisonZone] {
        &self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn record_area_blast(&mut self, units_hit: u32) {
        self.area_blasts = self.area_blasts.saturating_add(1);
        self.units_hit = self.units_hit.saturating_add(units_hit);
    }

    /// Spawn residual MediumPoisonField at SCUD toxin impact.
    pub fn spawn_zone(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostScudPoisonZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: SCUD_POISON_RADIUS,
            damage_per_tick: scud_poison_damage_per_tick(anthrax),
            activate_frame,
            expires_frame: activate_frame.saturating_add(SCUD_POISON_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            anthrax_tier: anthrax,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        id
    }

    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostScudPoisonTickPlan> {
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == zone.source_object {
                    continue;
                }
                let dx = zone.position.x - pos.x;
                let dz = zone.position.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= zone.radius {
                    hits.push(HostScudPoisonDamageHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostScudPoisonTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.zone_id);
        plans
    }

    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += total_damage;
            zone.damage_applications += applications;
            zone.objects_destroyed += objects_destroyed;
            zone.next_tick_frame =
                current_frame.saturating_add(SCUD_POISON_TICK_INTERVAL_FRAMES);
            self.total_damage_applied += total_damage;
            self.damage_applications = self.damage_applications.saturating_add(applications);
            self.objects_destroyed = self.objects_destroyed.saturating_add(objects_destroyed);
        }
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        self.expirations = self
            .expirations
            .saturating_add((before.saturating_sub(self.active.len())) as u32);
    }

    pub fn honesty_area_ok(&self) -> bool {
        self.area_blasts > 0
    }

    pub fn honesty_toxin_ok(&self) -> bool {
        self.zones_spawned > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_area_ok() || self.honesty_toxin_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn scud_name_matrix() {
        assert!(is_scud_launcher_template("GLAVehicleScudLauncher"));
        assert!(is_scud_launcher_template("Chem_GLAVehicleScudLauncher"));
        assert!(is_scud_launcher_template("Demo_GLAVehicleScudLauncher"));
        assert!(is_scud_launcher_template("TestScudLauncher"));
        assert!(is_scud_launcher_template("GLA_ScudLauncher"));
        assert!(!is_scud_launcher_template("SCUDMissile"));
        assert!(!is_scud_launcher_template("ScudStorm"));
        assert!(!is_scud_launcher_template("GLAScudStorm"));
        assert!(!is_scud_launcher_template("GLAVehicleRocketBuggy"));
        assert!(!is_scud_launcher_template("USA_Ranger"));
    }

    #[test]
    fn explosive_damage_rings() {
        assert!(
            (scud_explosive_damage_at(0.0) - SCUD_EXP_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (scud_explosive_damage_at(50.0) - SCUD_EXP_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (scud_explosive_damage_at(75.0) - SCUD_EXP_SECONDARY_DAMAGE).abs() < 0.01
        );
        assert!((scud_explosive_damage_at(101.0)).abs() < 0.01);
    }

    #[test]
    fn toxin_blast_and_gate() {
        assert!(
            (scud_toxin_blast_damage_at(10.0) - SCUD_TOX_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (scud_toxin_blast_damage_at(45.0) - SCUD_TOX_SECONDARY_DAMAGE).abs() < 0.01
        );
        assert!(should_spawn_scud_toxin_field(true, 1));
        assert!(!should_spawn_scud_toxin_field(true, 0));
        assert!(!should_spawn_scud_toxin_field(false, 1));
        assert!(scud_uses_anthrax_primary("Chem_GLAVehicleScudLauncher"));
        assert!(!scud_uses_anthrax_primary("GLAVehicleScudLauncher"));
        assert!(scud_toxin_warhead_for_slot("Chem_GLAVehicleScudLauncher", 0));
        assert!(!scud_toxin_warhead_for_slot("GLAVehicleScudLauncher", 0));
        assert!((scud_poison_damage_per_tick(AnthraxResidualTier::None) - 2.0).abs() < 0.01);
        assert!((scud_poison_damage_per_tick(AnthraxResidualTier::Gamma) - 2.5).abs() < 0.01);
        assert!(scud_prefer_secondary_vs_infantry(true, true));
        assert!(!scud_prefer_secondary_vs_infantry(true, false));
    }

    #[test]
    fn poison_registry_spawn_and_tick() {
        let mut reg = HostScudPoisonRegistry::new();
        let id = reg.spawn_zone(
            ObjectId(1),
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
            AnthraxResidualTier::None,
        );
        assert_eq!(id, 0);
        assert!(reg.honesty_toxin_ok());
        assert_eq!(reg.active_count(), 1);
        let gamma = reg.spawn_zone(
            ObjectId(1),
            Team::GLA,
            Vec3::new(5.0, 0.0, 0.0),
            0,
            AnthraxResidualTier::Gamma,
        );
        assert_eq!(gamma, 1);
        assert!((reg.active_zones()[1].damage_per_tick - 2.5).abs() < 0.01);

        let positions = vec![
            (ObjectId(1), Vec3::ZERO, Team::GLA, true),
            (
                ObjectId(2),
                Vec3::new(10.0, 0.0, 0.0),
                Team::USA,
                true,
            ),
            (
                ObjectId(3),
                Vec3::new(200.0, 0.0, 0.0),
                Team::USA,
                true,
            ),
        ];
        let plans = reg.plan_due_ticks(0, &positions);
        // Base + gamma zones both due.
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
    }
}
