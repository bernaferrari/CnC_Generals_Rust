//! Host GLA Angry Mob residual (nexus damages nearby enemies / expands).
//!
//! Residual slice (playability):
//! - `GLAInfantryAngryMobNexus` (and Chem_/Demo_/Slth_ / Boss_ variants) is the
//!   playable residual "mob" unit. It continuously damages nearby enemies inside
//!   residual AttackRange, representing aggregate fire from SpawnBehavior members
//!   (pistol / rock / molotov residual).
//! - **Expand residual**: member strength grows from retail `InitialBurst = 5`
//!   toward `SpawnNumber = 10` on `SpawnReplaceDelay = 30000` ms residual, so
//!   damage over frames increases as the mob expands.
//! - `Upgrade_GLAArmTheMob` residual multiplies damage by 1.25× (WeaponBonus
//!   PLAYER_UPGRADE DAMAGE 125%).
//!
//! Fail-closed honesty:
//! - Not full SpawnBehavior individual member objects / models / wander locomotor
//! - Not full MobMemberSlavedUpdate / MobNexusContain slave AI matrix
//! - Not full rock/molotov projectile objects / ArmTheMob AK47 WeaponSet swap
//! - Not AggregateHealth nexus HP bar / IGNORED_IN_GUI member selection kluge
//! - Not network AngryMob replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const ANGRY_MOB_LOGIC_FPS: f32 = 30.0;

/// Retail member weapon AttackRange residual (~100; nexus AI uses 90).
pub const ANGRY_MOB_ATTACK_RANGE: f32 = 100.0;

/// Residual aggregate fire tick: pistol DelayBetweenShots 250 ms → ~8 frames @ 30 FPS.
pub const ANGRY_MOB_TICK_INTERVAL_FRAMES: u32 = 8;

/// Damage contribution per residual member per tick (aggregate residual).
/// 5 members × 4 = 20; 10 members × 4 = 40 (between pistol 10 and rock 40).
pub const ANGRY_MOB_DAMAGE_PER_MEMBER_TICK: f32 = 4.0;

/// Retail SpawnBehavior InitialBurst residual (first set does not delay).
pub const ANGRY_MOB_INITIAL_MEMBERS: u32 = 5;

/// Retail SpawnBehavior SpawnNumber residual (max members).
pub const ANGRY_MOB_MAX_MEMBERS: u32 = 10;

/// Retail SpawnReplaceDelay 30000 ms → 900 frames @ 30 FPS (expand interval).
pub const ANGRY_MOB_EXPAND_INTERVAL_FRAMES: u32 = 900;

/// ArmTheMob PLAYER_UPGRADE damage multiplier residual (WeaponBonus DAMAGE 125%).
pub const ANGRY_MOB_ARMED_DAMAGE_MULT: f32 = 1.25;

/// Retail object-upgrade name.
pub const UPGRADE_GLA_ARM_THE_MOB: &str = "Upgrade_GLAArmTheMob";

/// Residual primary / AI target weapon (nexus uses harmless weapon; residual
/// binds a synthetic aggregate fire weapon for host combat/AI range).
pub const ANGRY_MOB_RESIDUAL_WEAPON: &str = "GLAAngryMobResidualWeapon";

/// Residual fire audio (pistol/AK ambient residual cue).
pub const ANGRY_MOB_FIRE_AUDIO: &str = "AngryMobWeaponPistol";

/// Normalize template name for residual matching.
fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual Angry Mob nexus (the playable mob unit).
///
/// Fail-closed: name residual (not full KindOf MOB_NEXUS / SpawnBehavior matrix).
/// Excludes individual mob members, projectiles, weapons, and command tokens.
pub fn is_angry_mob_nexus_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testangrymob" || n == "test_angry_mob" || n == "testangrymobnexus" {
        return true;
    }
    // Projectile / weapon / command tokens are not the living nexus.
    if n.contains("projectile")
        || n.contains("weapon")
        || n.contains("command")
        || n.contains("rock") && !n.contains("nexus")
        || n.contains("molotov") && !n.contains("nexus")
        || n.contains("pistol")
        || n.contains("ak47")
    {
        return false;
    }
    // Individual mob members (GLAInfantryAngryMobPistol01, …) are not the nexus.
    if n.contains("angrymob") && !n.contains("nexus") {
        // Allow bare "AngryMob" / "GLAAngryMob" as residual shorthand for nexus.
        if n == "angrymob" || n == "glaangrymob" || n.ends_with("angrymob") {
            return true;
        }
        return false;
    }
    n.contains("angrymobnexus") || n.contains("infantryangrymobnexus")
}

/// Whether residual target can take Angry Mob damage.
///
/// Retail members fire on ENEMIES (and neutrals via RadiusDamageAffects). Residual:
/// alive non-self enemy/neutral combat kinds, not under construction.
pub fn is_legal_angry_mob_damage_target(
    is_alive: bool,
    same_team: bool,
    is_self: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_self
        && !under_construction
        && is_attackable_or_combat_kind
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_angry_mob_range_2d(mob_pos: (f32, f32), target_pos: (f32, f32), range: f32) -> bool {
    let dx = mob_pos.0 - target_pos.0;
    let dy = mob_pos.1 - target_pos.1;
    dx * dx + dy * dy <= range * range
}

/// True when mob team vs target is residual-hostile (enemy) or Neutral victim.
pub fn is_angry_mob_hostile_team(
    mob_team_is_neutral: bool,
    same_team: bool,
    target_is_neutral: bool,
) -> bool {
    if mob_team_is_neutral {
        return false;
    }
    !same_team || target_is_neutral
}

/// Residual damage for one fire tick given member count and ArmTheMob state.
pub fn angry_mob_damage_for_tick(member_count: u32, armed: bool) -> f32 {
    let members = member_count.max(1) as f32;
    let base = ANGRY_MOB_DAMAGE_PER_MEMBER_TICK * members;
    if armed {
        base * ANGRY_MOB_ARMED_DAMAGE_MULT
    } else {
        base
    }
}

/// Per-nexus residual state (member expand + fire cadence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAngryMobState {
    pub object_id: ObjectId,
    pub team: super::Team,
    /// Current residual member strength (InitialBurst..SpawnNumber).
    pub member_count: u32,
    /// Next absolute frame for aggregate fire damage tick.
    pub next_tick_frame: u32,
    /// Next absolute frame for expand residual (+1 member toward max).
    pub next_expand_frame: u32,
    /// Position snapshot at last plan (diagnostic).
    pub position: Vec3,
    /// Total damage dealt by this mob (honesty).
    pub total_damage_applied: f32,
    /// Damage application events (object×tick).
    pub damage_applications: u32,
    /// Expand events applied to this mob.
    pub expands: u32,
}

impl HostAngryMobState {
    pub fn new(object_id: ObjectId, team: super::Team, position: Vec3, activate_frame: u32) -> Self {
        Self {
            object_id,
            team,
            member_count: ANGRY_MOB_INITIAL_MEMBERS,
            // Immediate first tick so residual damage is observable on first update.
            next_tick_frame: activate_frame,
            next_expand_frame: activate_frame.saturating_add(ANGRY_MOB_EXPAND_INTERVAL_FRAMES),
            position,
            total_damage_applied: 0.0,
            damage_applications: 0,
            expands: 0,
        }
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        current_frame >= self.next_tick_frame
    }

    pub fn is_due_expand(&self, current_frame: u32) -> bool {
        self.member_count < ANGRY_MOB_MAX_MEMBERS && current_frame >= self.next_expand_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostAngryMobDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub mob_id: ObjectId,
}

/// Result of resolving one mob's damage tick.
#[derive(Debug, Clone)]
pub struct HostAngryMobTickPlan {
    pub mob_id: ObjectId,
    pub source_team: super::Team,
    pub member_count: u32,
    pub hits: Vec<HostAngryMobDamageHit>,
}

/// Host residual registry for Angry Mob nexus units.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAngryMobRegistry {
    /// Active residual mobs keyed by object id (stable order via vec).
    active: Vec<HostAngryMobState>,
    /// Total residual fire ticks that applied ≥1 hit.
    pub fire_ticks: u32,
    /// Total residual damage applications (object×tick).
    pub damage_applications: u32,
    /// Total residual damage applied.
    pub total_damage_applied: f32,
    /// Objects destroyed by residual mob fire.
    pub objects_destroyed: u32,
    /// Expand residual events (member count growth).
    pub expands: u32,
    /// Mobs that reached max member residual.
    pub fully_expanded: u32,
}

impl HostAngryMobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_mobs(&self) -> &[HostAngryMobState] {
        &self.active
    }

    pub fn active_mobs_mut(&mut self) -> &mut [HostAngryMobState] {
        &mut self.active
    }

    /// Ensure living nexus objects are tracked; drop dead / removed.
    pub fn sync_mobs(
        &mut self,
        living: &[(ObjectId, super::Team, Vec3)],
        current_frame: u32,
    ) {
        let living_ids: std::collections::HashSet<ObjectId> =
            living.iter().map(|(id, _, _)| *id).collect();
        self.active.retain(|m| living_ids.contains(&m.object_id));

        for &(id, team, pos) in living {
            if let Some(m) = self.active.iter_mut().find(|m| m.object_id == id) {
                m.team = team;
                m.position = pos;
            } else {
                self.active
                    .push(HostAngryMobState::new(id, team, pos, current_frame));
            }
        }
        self.active.sort_by_key(|m| m.object_id.0);
    }

    /// Apply expand residual: grow member_count toward max on interval.
    pub fn apply_due_expands(&mut self, current_frame: u32) -> u32 {
        let mut expanded = 0_u32;
        for mob in &mut self.active {
            if !mob.is_due_expand(current_frame) {
                continue;
            }
            mob.member_count = mob.member_count.saturating_add(1).min(ANGRY_MOB_MAX_MEMBERS);
            mob.expands = mob.expands.saturating_add(1);
            mob.next_expand_frame =
                current_frame.saturating_add(ANGRY_MOB_EXPAND_INTERVAL_FRAMES);
            self.expands = self.expands.saturating_add(1);
            expanded = expanded.saturating_add(1);
            if mob.member_count >= ANGRY_MOB_MAX_MEMBERS {
                self.fully_expanded = self.fully_expanded.saturating_add(1);
            }
        }
        expanded
    }

    /// Plan damage ticks for all mobs due this frame.
    ///
    /// `candidates`: (id, pos, team, alive, legal_combat_kind, under_construction)
    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        candidates: &[(ObjectId, Vec3, super::Team, bool, bool, bool)],
        armed_by_team: impl Fn(super::Team) -> bool,
    ) -> Vec<HostAngryMobTickPlan> {
        let mut plans = Vec::new();
        for mob in &self.active {
            if !mob.is_due_tick(current_frame) {
                continue;
            }
            let armed = armed_by_team(mob.team);
            let damage = angry_mob_damage_for_tick(mob.member_count, armed);
            let mob_neutral = mob.team == super::Team::Neutral;
            let mut hits = Vec::new();
            for &(id, pos, team, alive, combat_kind, under_construction) in candidates {
                if id == mob.object_id {
                    continue;
                }
                let same_team = team == mob.team;
                let target_neutral = team == super::Team::Neutral;
                if !is_angry_mob_hostile_team(mob_neutral, same_team, target_neutral) {
                    continue;
                }
                if !is_legal_angry_mob_damage_target(
                    alive,
                    same_team,
                    false,
                    under_construction,
                    combat_kind,
                ) {
                    continue;
                }
                if !in_angry_mob_range_2d(
                    (mob.position.x, mob.position.z),
                    (pos.x, pos.z),
                    ANGRY_MOB_ATTACK_RANGE,
                ) {
                    continue;
                }
                hits.push(HostAngryMobDamageHit {
                    target_id: id,
                    damage,
                    mob_id: mob.object_id,
                });
            }
            plans.push(HostAngryMobTickPlan {
                mob_id: mob.object_id,
                source_team: mob.team,
                member_count: mob.member_count,
                hits,
            });
        }
        plans.sort_by_key(|p| p.mob_id.0);
        plans
    }

    /// Record results after GameLogic applied a tick's damage.
    pub fn record_tick_complete(
        &mut self,
        mob_id: ObjectId,
        damage_applied: f32,
        applications: u32,
        destroyed: u32,
        current_frame: u32,
        had_hits: bool,
    ) {
        if let Some(mob) = self.active.iter_mut().find(|m| m.object_id == mob_id) {
            mob.total_damage_applied += damage_applied;
            mob.damage_applications = mob.damage_applications.saturating_add(applications);
            mob.next_tick_frame = current_frame.saturating_add(ANGRY_MOB_TICK_INTERVAL_FRAMES);
        }
        if had_hits {
            self.fire_ticks = self.fire_ticks.saturating_add(1);
        }
        self.total_damage_applied += damage_applied;
        self.damage_applications = self.damage_applications.saturating_add(applications);
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    /// Residual honesty: at least one fire tick damaged something.
    pub fn honesty_damage_ok(&self) -> bool {
        self.damage_applications > 0 && self.total_damage_applied > 0.0
    }

    /// Residual honesty: expand residual grew member count at least once.
    pub fn honesty_expand_ok(&self) -> bool {
        self.expands > 0
    }

    /// Combined host path honesty (damage and/or expand residual exercised).
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_damage_ok() || self.honesty_expand_ok()
    }

    /// Member count for a tracked mob (tests / diagnostics).
    pub fn member_count_of(&self, mob_id: ObjectId) -> Option<u32> {
        self.active
            .iter()
            .find(|m| m.object_id == mob_id)
            .map(|m| m.member_count)
    }
}


// --- Wave 69 residual honesty peels (retail weapon / body / upgrade) ---

/// Convert residual msec → logic frames @ 30 FPS (round half-up).
pub fn angry_mob_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * ANGRY_MOB_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail GLAAngryMobPistolWeapon PrimaryDamage residual.
pub const ANGRY_MOB_PISTOL_DAMAGE: f32 = 10.0;
/// Retail pistol AttackRange residual.
pub const ANGRY_MOB_PISTOL_RANGE: f32 = 100.0;
/// Retail pistol DelayBetweenShots residual (msec).
pub const ANGRY_MOB_PISTOL_DELAY_MS: u32 = 250;
/// Retail pistol ClipSize residual.
pub const ANGRY_MOB_PISTOL_CLIP: u32 = 8;
/// Retail pistol ClipReloadTime residual (msec).
pub const ANGRY_MOB_PISTOL_CLIP_RELOAD_MS: u32 = 3_000;
/// Retail pistol DamageType residual.
pub const ANGRY_MOB_PISTOL_DAMAGE_TYPE: &str = "MOLOTOV_COCKTAIL";
/// Retail GLAAngryMobPistolWeapon name residual.
pub const ANGRY_MOB_PISTOL_WEAPON: &str = "GLAAngryMobPistolWeapon";
/// Retail SpawnReplaceDelay residual (msec).
pub const ANGRY_MOB_SPAWN_REPLACE_DELAY_MS: u32 = 30_000;

/// Retail nexus MaxHealth residual (effectively immortal AggregateHealth).
pub const ANGRY_MOB_MAX_HEALTH: f32 = 99_999.0;
/// Retail BuildCost residual.
pub const ANGRY_MOB_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const ANGRY_MOB_BUILD_TIME_SEC: f32 = 15.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const ANGRY_MOB_BUILD_TIME_FRAMES: u32 = 450;
/// Retail VisionRange residual.
pub const ANGRY_MOB_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const ANGRY_MOB_SHROUD_CLEARING_RANGE: f32 = 0.0;
/// Retail TransportSlotCount residual (not transportable).
pub const ANGRY_MOB_TRANSPORT_SLOT_COUNT: u32 = 0;
/// Retail nexus locomotor speed residual.
pub const ANGRY_MOB_LOCOMOTOR_SPEED: f32 = 18.0;

/// Retail ArmTheMob BuildCost residual.
pub const ARM_THE_MOB_BUILD_COST: u32 = 1_000;
/// Retail ArmTheMob BuildTime residual (seconds).
pub const ARM_THE_MOB_BUILD_TIME_SEC: f32 = 30.0;
/// Retail ArmTheMob BuildTime → frames.
pub const ARM_THE_MOB_BUILD_TIME_FRAMES: u32 = 900;
/// Retail ArmTheMob research audio residual.
pub const ARM_THE_MOB_RESEARCH_SOUND: &str = "AngryMobVoiceUpgradeArmTheMob";

/// Wave 69 residual honesty: aggregate weapon / spawn residual peel.
pub fn honesty_angry_mob_weapon_residual_ok() -> bool {
    ANGRY_MOB_PISTOL_WEAPON == "GLAAngryMobPistolWeapon"
        && (ANGRY_MOB_PISTOL_DAMAGE - 10.0).abs() < 0.01
        && (ANGRY_MOB_PISTOL_RANGE - 100.0).abs() < 0.01
        && (ANGRY_MOB_ATTACK_RANGE - 100.0).abs() < 0.01
        && ANGRY_MOB_PISTOL_DELAY_MS == 250
        && ANGRY_MOB_TICK_INTERVAL_FRAMES == angry_mob_ms_to_frames(ANGRY_MOB_PISTOL_DELAY_MS)
        && ANGRY_MOB_TICK_INTERVAL_FRAMES == 8
        && ANGRY_MOB_PISTOL_CLIP == 8
        && ANGRY_MOB_PISTOL_CLIP_RELOAD_MS == 3_000
        && ANGRY_MOB_PISTOL_DAMAGE_TYPE == "MOLOTOV_COCKTAIL"
        && ANGRY_MOB_INITIAL_MEMBERS == 5
        && ANGRY_MOB_MAX_MEMBERS == 10
        && ANGRY_MOB_SPAWN_REPLACE_DELAY_MS == 30_000
        && ANGRY_MOB_EXPAND_INTERVAL_FRAMES
            == angry_mob_ms_to_frames(ANGRY_MOB_SPAWN_REPLACE_DELAY_MS)
        && ANGRY_MOB_EXPAND_INTERVAL_FRAMES == 900
        && (ANGRY_MOB_ARMED_DAMAGE_MULT - 1.25).abs() < 0.01
        && UPGRADE_GLA_ARM_THE_MOB == "Upgrade_GLAArmTheMob"
        && ANGRY_MOB_FIRE_AUDIO == "AngryMobWeaponPistol"
        && (angry_mob_damage_for_tick(5, false) - 20.0).abs() < 0.01
        && (angry_mob_damage_for_tick(5, true) - 25.0).abs() < 0.01
}

/// Wave 69 residual honesty: nexus body residual peel.
pub fn honesty_angry_mob_body_residual_ok() -> bool {
    (ANGRY_MOB_MAX_HEALTH - 99_999.0).abs() < 0.01
        && ANGRY_MOB_BUILD_COST == 800
        && (ANGRY_MOB_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && ANGRY_MOB_BUILD_TIME_FRAMES
            == ((ANGRY_MOB_BUILD_TIME_SEC * ANGRY_MOB_LOGIC_FPS).round() as u32)
        && ANGRY_MOB_BUILD_TIME_FRAMES == 450
        && (ANGRY_MOB_VISION_RANGE - 150.0).abs() < 0.01
        && (ANGRY_MOB_SHROUD_CLEARING_RANGE - 0.0).abs() < 0.01
        && ANGRY_MOB_TRANSPORT_SLOT_COUNT == 0
        && (ANGRY_MOB_LOCOMOTOR_SPEED - 18.0).abs() < 0.01
        && is_angry_mob_nexus_template("GLAInfantryAngryMobNexus")
        && !is_angry_mob_nexus_template("GLAInfantryAngryMobPistol01")
}

/// Wave 69 residual honesty: ArmTheMob upgrade residual peel.
pub fn honesty_angry_mob_upgrade_residual_ok() -> bool {
    UPGRADE_GLA_ARM_THE_MOB == "Upgrade_GLAArmTheMob"
        && ARM_THE_MOB_BUILD_COST == 1_000
        && (ARM_THE_MOB_BUILD_TIME_SEC - 30.0).abs() < 0.01
        && ARM_THE_MOB_BUILD_TIME_FRAMES
            == ((ARM_THE_MOB_BUILD_TIME_SEC * ANGRY_MOB_LOGIC_FPS).round() as u32)
        && ARM_THE_MOB_BUILD_TIME_FRAMES == 900
        && ARM_THE_MOB_RESEARCH_SOUND == "AngryMobVoiceUpgradeArmTheMob"
        && (ANGRY_MOB_ARMED_DAMAGE_MULT - 1.25).abs() < 0.01
}

/// Combined Wave 69 Angry Mob residual honesty pack.
pub fn honesty_angry_mob_residual_pack_ok() -> bool {
    honesty_angry_mob_weapon_residual_ok()
        && honesty_angry_mob_body_residual_ok()
        && honesty_angry_mob_upgrade_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn angry_mob_nexus_name_matrix() {
        assert!(is_angry_mob_nexus_template("GLAInfantryAngryMobNexus"));
        assert!(is_angry_mob_nexus_template("Chem_GLAInfantryAngryMobNexus"));
        assert!(is_angry_mob_nexus_template("Demo_GLAInfantryAngryMobNexus"));
        assert!(is_angry_mob_nexus_template("Slth_GLAInfantryAngryMobNexus"));
        assert!(is_angry_mob_nexus_template("TestAngryMob"));
        assert!(is_angry_mob_nexus_template("GLAAngryMob"));
        assert!(!is_angry_mob_nexus_template("GLAInfantryAngryMobPistol01"));
        assert!(!is_angry_mob_nexus_template("GLAAngryMobRockProjectileObject"));
        assert!(!is_angry_mob_nexus_template("GLAAngryMobMolotovCocktailProjectileObject"));
        assert!(!is_angry_mob_nexus_template("GLAAngryMobPistolWeapon"));
        assert!(!is_angry_mob_nexus_template("USA_Ranger"));
        assert!(!is_angry_mob_nexus_template("GLAInfantryRebel"));
    }

    #[test]
    fn legal_target_and_range_matrix() {
        assert!(is_legal_angry_mob_damage_target(
            true, false, false, false, true
        ));
        assert!(!is_legal_angry_mob_damage_target(
            false, false, false, false, true
        ));
        assert!(!is_legal_angry_mob_damage_target(
            true, true, false, false, true
        ));
        assert!(!is_legal_angry_mob_damage_target(
            true, false, true, false, true
        ));
        assert!(!is_legal_angry_mob_damage_target(
            true, false, false, true, true
        ));
        assert!(!is_legal_angry_mob_damage_target(
            true, false, false, false, false
        ));

        assert!(in_angry_mob_range_2d((0.0, 0.0), (50.0, 0.0), 100.0));
        assert!(!in_angry_mob_range_2d((0.0, 0.0), (150.0, 0.0), 100.0));
        assert!(is_angry_mob_hostile_team(false, false, false));
        assert!(is_angry_mob_hostile_team(false, false, true));
        assert!(!is_angry_mob_hostile_team(false, true, false));
        assert!(!is_angry_mob_hostile_team(true, false, false));
    }

    #[test]
    fn damage_scales_with_members_and_arm_upgrade() {
        let base5 = angry_mob_damage_for_tick(5, false);
        let base10 = angry_mob_damage_for_tick(10, false);
        let armed5 = angry_mob_damage_for_tick(5, true);
        assert!((base5 - 20.0).abs() < f32::EPSILON);
        assert!((base10 - 40.0).abs() < f32::EPSILON);
        assert!((armed5 - 25.0).abs() < f32::EPSILON);
        assert!(base10 > base5);
        assert!(armed5 > base5);
    }

    #[test]
    fn sync_tick_damages_nearby_enemy_over_frames() {
        let mut reg = HostAngryMobRegistry::new();
        let mob_id = ObjectId(1);
        let enemy_id = ObjectId(2);
        let far_id = ObjectId(3);

        reg.sync_mobs(
            &[(mob_id, Team::GLA, Vec3::new(0.0, 0.0, 0.0))],
            0,
        );
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_INITIAL_MEMBERS));

        let candidates = vec![
            (
                mob_id,
                Vec3::ZERO,
                Team::GLA,
                true,
                true,
                false,
            ),
            (
                enemy_id,
                Vec3::new(50.0, 0.0, 0.0),
                Team::USA,
                true,
                true,
                false,
            ),
            (
                far_id,
                Vec3::new(500.0, 0.0, 0.0),
                Team::USA,
                true,
                true,
                false,
            ),
        ];

        let plans = reg.plan_due_ticks(0, &candidates, |_| false);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, enemy_id);
        assert!(
            (plans[0].hits[0].damage - angry_mob_damage_for_tick(5, false)).abs() < 0.01
        );

        reg.record_tick_complete(mob_id, plans[0].hits[0].damage, 1, 0, 0, true);
        assert!(reg.honesty_damage_ok());
        assert!(reg.honesty_host_path_ok());

        // Not due again until interval elapses.
        assert!(reg.plan_due_ticks(1, &candidates, |_| false).is_empty());
        let second = reg.plan_due_ticks(ANGRY_MOB_TICK_INTERVAL_FRAMES, &candidates, |_| false);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].hits.len(), 1);
    }

    #[test]
    fn expand_residual_grows_member_count() {
        let mut reg = HostAngryMobRegistry::new();
        let mob_id = ObjectId(1);
        reg.sync_mobs(&[(mob_id, Team::GLA, Vec3::ZERO)], 0);
        assert_eq!(reg.member_count_of(mob_id), Some(5));

        // Not due yet.
        assert_eq!(reg.apply_due_expands(1), 0);
        assert!(!reg.honesty_expand_ok());

        let n = reg.apply_due_expands(ANGRY_MOB_EXPAND_INTERVAL_FRAMES);
        assert_eq!(n, 1);
        assert_eq!(reg.member_count_of(mob_id), Some(6));
        assert!(reg.honesty_expand_ok());

        // Cap at max.
        for i in 0..10 {
            let frame = ANGRY_MOB_EXPAND_INTERVAL_FRAMES * (i + 2);
            reg.apply_due_expands(frame);
        }
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS));
    }

    #[test]
    fn angry_mob_residual_pack_honesty_wave69() {
        assert_eq!(angry_mob_ms_to_frames(250), 8);
        assert_eq!(angry_mob_ms_to_frames(30_000), 900);
        assert!(honesty_angry_mob_weapon_residual_ok());
        assert!(honesty_angry_mob_body_residual_ok());
        assert!(honesty_angry_mob_upgrade_residual_ok());
        assert!(honesty_angry_mob_residual_pack_ok());
        assert_eq!(ANGRY_MOB_BUILD_TIME_FRAMES, 450);
        assert_eq!(ARM_THE_MOB_BUILD_COST, 1_000);
        assert_eq!(ANGRY_MOB_PISTOL_DAMAGE_TYPE, "MOLOTOV_COCKTAIL");
    }
}
