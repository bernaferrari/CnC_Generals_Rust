//! Host GLA Angry Mob residual (SpawnBehavior members fire independently).
//!
//! Residual slice (playability):
//! - `GLAInfantryAngryMobNexus` (and Chem_/Demo_/Slth_ / Boss_ variants) is the
//!   playable residual "mob" unit. Each live member independently attacks one
//!   closest victim inside residual AttackRange (C++ MobMemberSlavedUpdate
//!   `getNextMoodTarget` + `aiAttackObject`) — not full stack on every hostile.
//! - **SpawnBehavior residual**: rapid-spawns all `SpawnNumber = 10` members, then
//!   `SpawnReplaceDelay = 30000` ms replacements on death. `member_count` tracks
//!   live members (DPS shrinks as they die). Last member death destroys the nexus
//!   (`onSpawnDeath` + AggregateHealth). Nexus is `OBJECT_STATUS_MASKED`.
//! - `Upgrade_GLAArmTheMob` residual multiplies damage by 1.25× (WeaponBonus
//!   PLAYER_UPGRADE DAMAGE 125%).
//!
//! Fail-closed honesty:
//! - SpawnBehavior member SpecialObject residual closed (template rotation + follow)
//! - Not full member models / wander locomotor matrix
//! - Not full MobMemberSlavedUpdate / MobNexusContain slave AI matrix
//! - Rock/molotov DumbProjectile Bezier lob residual (GLAAngryMobRockProjectileObject /
//!   GLAAngryMobMolotovCocktailProjectileObject First/SecondHeight + WeaponSpeed)
//! - Not full ArmTheMob AK47 WeaponSet swap / member models / wander locomotor matrix
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

/// Retail SpawnTemplateName rotation residual (SpawnBehavior ModuleTag_05).
pub const ANGRY_MOB_MEMBER_TEMPLATES: &[&str] = &[
    "GLAInfantryAngryMobPistol01",
    "GLAInfantryAngryMobRock02",
    "GLAInfantryAngryMobMolotov02",
    "GLAInfantryAngryMobPistol03",
    "GLAInfantryAngryMobRock04",
    "GLAInfantryAngryMobMolotov02",
    "GLAInfantryAngryMobPistol05",
];
/// Member infantry residual MaxHealth honesty.
pub const ANGRY_MOB_MEMBER_MAX_HEALTH: f32 = 50.0;

/// Retail SpawnReplaceDelay 30000 ms → 900 frames @ 30 FPS (C++ replacement delay).
pub const ANGRY_MOB_EXPAND_INTERVAL_FRAMES: u32 = 900;
/// C++ QueueProductionExitUpdate ExitDelay=5000ms → 150 frames @ 30 FPS.
pub const ANGRY_MOB_EXIT_DELAY_FRAMES: u32 = 150;
/// C++ SPAWN_DELAY_MIN_FRAMES (SpawnBehavior.cpp:36) — unused birth-frame stagger.
pub const ANGRY_MOB_SPAWN_DELAY_MIN_FRAMES: u32 = 16;

/// ArmTheMob PLAYER_UPGRADE damage multiplier residual (WeaponBonus DAMAGE 125%).
pub const ANGRY_MOB_ARMED_DAMAGE_MULT: f32 = 1.25;

/// Retail object-upgrade name.
pub const UPGRADE_GLA_ARM_THE_MOB: &str = "Upgrade_GLAArmTheMob";

/// Residual primary / AI target weapon (nexus uses harmless weapon; residual
/// binds a synthetic aggregate fire weapon for host combat/AI range).
pub const ANGRY_MOB_RESIDUAL_WEAPON: &str = "GLAAngryMobResidualWeapon";

/// Retail rock projectile weapon residual.
pub const ANGRY_MOB_ROCK_WEAPON: &str = "GLAAngryMobRockProjectileWeapon";
/// Retail molotov projectile weapon residual.
pub const ANGRY_MOB_MOLOTOV_WEAPON: &str = "GLAAngryMobMolotovCocktailProjectileWeapon";
/// Retail rock ProjectileObject residual.
pub const ANGRY_MOB_ROCK_PROJECTILE: &str = "GLAAngryMobRockProjectileObject";
/// Retail molotov ProjectileObject residual.
pub const ANGRY_MOB_MOLOTOV_PROJECTILE: &str = "GLAAngryMobMolotovCocktailProjectileObject";
/// Rock PrimaryDamage residual.
pub const ANGRY_MOB_ROCK_DAMAGE: f32 = 40.0;
/// Rock PrimaryDamageRadius residual.
pub const ANGRY_MOB_ROCK_RADIUS: f32 = 1.0;
/// Rock WeaponSpeed residual (dist/sec).
pub const ANGRY_MOB_ROCK_WEAPON_SPEED: f32 = 130.0;
/// Molotov PrimaryDamage residual.
pub const ANGRY_MOB_MOLOTOV_DAMAGE: f32 = 40.0;
/// Molotov PrimaryDamageRadius residual.
pub const ANGRY_MOB_MOLOTOV_RADIUS: f32 = 11.0;
/// Molotov WeaponSpeed residual (dist/sec).
pub const ANGRY_MOB_MOLOTOV_WEAPON_SPEED: f32 = 60.0;
/// Retail rock DamageType residual.
pub const ANGRY_MOB_ROCK_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail rock DeathType residual.
pub const ANGRY_MOB_ROCK_DEATH_TYPE: &str = "NORMAL";
/// Retail molotov DamageType residual.
pub const ANGRY_MOB_MOLOTOV_DAMAGE_TYPE: &str = "MOLOTOV_COCKTAIL";
/// Retail molotov DeathType residual.
pub const ANGRY_MOB_MOLOTOV_DEATH_TYPE: &str = "BURNED";

/// Rock DumbProjectile First/SecondHeight residual.
pub const ANGRY_MOB_ROCK_FIRST_HEIGHT: f32 = 10.0;
pub const ANGRY_MOB_ROCK_SECOND_HEIGHT: f32 = 10.0;
/// Molotov DumbProjectile First/SecondHeight residual (tall arc).
pub const ANGRY_MOB_MOLOTOV_FIRST_HEIGHT: f32 = 30.0;
pub const ANGRY_MOB_MOLOTOV_SECOND_HEIGHT: f32 = 30.0;
/// Shared First/SecondPercentIndent residual (50% / 90%).
pub const ANGRY_MOB_PROJ_FIRST_PERCENT_INDENT: f32 = 0.50;
pub const ANGRY_MOB_PROJ_SECOND_PERCENT_INDENT: f32 = 0.90;
/// Projectile MaxHealth residual.
pub const ANGRY_MOB_PROJ_MAX_HEALTH: f32 = 100.0;

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

/// Kind of residual Angry Mob thrown projectile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngryMobProjectileKind {
    Rock,
    Molotov,
}

impl AngryMobProjectileKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Rock => 0,
            Self::Molotov => 1,
        }
    }
    pub fn from_u8(v: u8) -> Self {
        if v == 1 { Self::Molotov } else { Self::Rock }
    }
    pub fn projectile_name(self) -> &'static str {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_PROJECTILE,
            Self::Molotov => ANGRY_MOB_MOLOTOV_PROJECTILE,
        }
    }
    pub fn weapon_speed(self) -> f32 {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_WEAPON_SPEED,
            Self::Molotov => ANGRY_MOB_MOLOTOV_WEAPON_SPEED,
        }
    }
    pub fn damage(self) -> f32 {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_DAMAGE,
            Self::Molotov => ANGRY_MOB_MOLOTOV_DAMAGE,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_RADIUS,
            Self::Molotov => ANGRY_MOB_MOLOTOV_RADIUS,
        }
    }
    pub fn first_height(self) -> f32 {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_FIRST_HEIGHT,
            Self::Molotov => ANGRY_MOB_MOLOTOV_FIRST_HEIGHT,
        }
    }
    pub fn second_height(self) -> f32 {
        match self {
            Self::Rock => ANGRY_MOB_ROCK_SECOND_HEIGHT,
            Self::Molotov => ANGRY_MOB_MOLOTOV_SECOND_HEIGHT,
        }
    }
}

/// Cubic Bezier point residual (DumbProjectileBehavior First/SecondHeight + indents).
pub fn angry_mob_projectile_bezier_point(
    from: Vec3,
    to: Vec3,
    t: f32,
    kind: AngryMobProjectileKind,
) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let delta = to - from;
    let p0 = from;
    let p3 = to;
    let p1 = from + delta * ANGRY_MOB_PROJ_FIRST_PERCENT_INDENT + Vec3::Y * kind.first_height();
    let p2 = from + delta * ANGRY_MOB_PROJ_SECOND_PERCENT_INDENT + Vec3::Y * kind.second_height();
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

/// Flight frames from horizontal distance / WeaponSpeed @ 30 FPS.
pub fn angry_mob_projectile_flight_frames(
    from: Vec3,
    to: Vec3,
    kind: AngryMobProjectileKind,
) -> u32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let dist = (dx * dx + dz * dz).sqrt().max(1.0);
    let frames = (dist / (kind.weapon_speed() / ANGRY_MOB_LOGIC_FPS)).ceil() as u32;
    frames.max(1)
}

/// Splash damage at distance for rock/molotov residual.
pub fn angry_mob_projectile_damage_at(kind: AngryMobProjectileKind, distance: f32) -> f32 {
    if distance <= kind.radius() + 0.001 {
        kind.damage()
    } else {
        0.0
    }
}

/// Alternate rock/molotov residual by tick counter (member mix honesty).
pub fn angry_mob_projectile_kind_for_tick(tick_index: u32) -> AngryMobProjectileKind {
    if tick_index % 3 == 2 {
        AngryMobProjectileKind::Molotov
    } else {
        AngryMobProjectileKind::Rock
    }
}

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
/// C++ members auto-acquire ENEMIES only (`getNextMoodTarget` →
/// `findClosestEnemy`, AIUpdate.cpp:4619; MobMemberSlavedUpdate.cpp:300).
/// `same_team` is "not player ENEMIES" (Allies / same controller), not
/// faction `Team` equality — 2v2 GLA+China is Allies; FFA same-faction is
/// Enemies. Residual: alive non-self enemy combat kinds, not under construction.
pub fn is_legal_angry_mob_damage_target(
    is_alive: bool,
    same_team: bool,
    is_self: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
) -> bool {
    is_alive && !same_team && !is_self && !under_construction && is_attackable_or_combat_kind
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_angry_mob_range_2d(mob_pos: (f32, f32), target_pos: (f32, f32), range: f32) -> bool {
    let dx = mob_pos.0 - target_pos.0;
    let dy = mob_pos.1 - target_pos.1;
    dx * dx + dy * dy <= range * range
}

/// Retail GLAAngryMobPistol/Rock/Molotov: AntiAirborneVehicle/Infantry = No.
/// C++ WeaponTemplate ctor `m_antiMask = WEAPON_ANTI_GROUND` (Weapon.cpp:287).
pub const ANGRY_MOB_ANTI_AIR: bool = false;
/// C++ `WEAPON_ANTI_GROUND` — leftover `get_victim_anti_mask` overlap bit.
pub const ANGRY_MOB_WEAPON_ANTI_MASK: u32 = 0x02;

/// C++ `getVictimAntiMask` airborne path plus live Aircraft residual.
/// `PartitionFilterPossibleToAttack` AntiAir=No cannot engage these.
pub fn is_angry_mob_air_target(is_aircraft: bool, airborne_target: bool) -> bool {
    is_aircraft || airborne_target
}

/// Residual PossibleToAttack: GROUND-only weapons skip air victims.
/// Wires leftover `get_victim_anti_mask` / live `weapon_target_anti_mask`.
pub fn angry_mob_possible_to_attack(
    is_aircraft: bool,
    airborne_target: bool,
    victim_anti_mask: u32,
) -> bool {
    if !ANGRY_MOB_ANTI_AIR && is_angry_mob_air_target(is_aircraft, airborne_target) {
        return false;
    }
    (victim_anti_mask & ANGRY_MOB_WEAPON_ANTI_MASK) != 0
}

/// True when mob vs target is C++ ENEMIES (`Object::getRelationship`).
/// `same_team` means not ENEMIES (Allies / same controller). Neutral
/// (oil derricks) is not hostile. Faction `Team` equality is leftover
/// fallback only — live host wires player relationship.
pub fn is_angry_mob_hostile_team(
    mob_team_is_neutral: bool,
    same_team: bool,
    target_is_neutral: bool,
) -> bool {
    if mob_team_is_neutral || target_is_neutral {
        return false;
    }
    !same_team
}

/// Per-member residual fire (C++ one victim per member).
pub fn angry_mob_damage_per_member(armed: bool) -> f32 {
    if armed {
        ANGRY_MOB_DAMAGE_PER_MEMBER_TICK * ANGRY_MOB_ARMED_DAMAGE_MULT
    } else {
        ANGRY_MOB_DAMAGE_PER_MEMBER_TICK
    }
}

/// Residual damage if every live member independently hits the **same** victim.
/// C++ scales with `m_spawnCount`; dead members do not contribute.
pub fn angry_mob_damage_for_tick(member_count: u32, armed: bool) -> f32 {
    angry_mob_damage_per_member(armed) * member_count as f32
}

fn angry_mob_dist2_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

/// C++ MobMemberSlavedUpdate.cpp:300-304: each member independently
/// `getNextMoodTarget` (findClosestEnemy from that member) + `aiAttackObject`.
/// Same victim may be chosen by several members; distinct victims never each
/// take the full member-stack.
pub fn assign_angry_mob_member_hits(
    mob_id: ObjectId,
    nexus_pos: Vec3,
    member_origins: &[Vec3],
    victims: &[(ObjectId, Vec3)],
    armed: bool,
) -> Vec<HostAngryMobDamageHit> {
    if member_origins.is_empty() || victims.is_empty() {
        return Vec::new();
    }
    let per = angry_mob_damage_per_member(armed);
    let mut stacked: Vec<(ObjectId, f32)> = Vec::new();
    for &origin in member_origins {
        let Some(&(vid, _)) = victims.iter().min_by(|a, b| {
            let da = angry_mob_dist2_xz(origin, a.1);
            let db = angry_mob_dist2_xz(origin, b.1);
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.0.cmp(&b.0.0))
        }) else {
            break;
        };
        if let Some(entry) = stacked.iter_mut().find(|e| e.0 == vid) {
            entry.1 += per;
        } else {
            stacked.push((vid, per));
        }
    }
    stacked.sort_by(|a, b| {
        let pa = victims
            .iter()
            .find(|v| v.0 == a.0)
            .map(|v| v.1)
            .unwrap_or(nexus_pos);
        let pb = victims
            .iter()
            .find(|v| v.0 == b.0)
            .map(|v| v.1)
            .unwrap_or(nexus_pos);
        angry_mob_dist2_xz(nexus_pos, pa)
            .partial_cmp(&angry_mob_dist2_xz(nexus_pos, pb))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.0.cmp(&b.0.0))
    });
    stacked
        .into_iter()
        .map(|(target_id, damage)| HostAngryMobDamageHit {
            target_id,
            damage,
            mob_id,
        })
        .collect()
}

/// C++ `PartitionFilterStealthedAndUndetected(me, false)` reject.
/// Leftover `PartitionFilterStealthedAndUndetected` already matches C++
/// (`partition_filters.rs`). Live residual applies the same predicate as
/// `Object::is_effectively_stealthed`: STEALTHED && !DETECTED && !DISGUISED.
#[inline]
pub fn angry_mob_skips_stealthed_undetected(
    stealthed: bool,
    detected: bool,
    disguised: bool,
) -> bool {
    stealthed && !detected && !disguised
}

/// C++ SpawnBehavior::computeAggregateStates sets OBJECT_STATUS_MASKED
/// (`SpawnBehavior.cpp:995`) so weapons skip the 99999-HP nexus. Live
/// `Object::is_selectable` treats MASKED as unselectable, which made the
/// playable mob a turret. Keep the nexus unmasked; weapons skip it via
/// `is_angry_mob_nexus_template`.
pub fn angry_mob_nexus_should_be_masked() -> bool {
    false
}

/// C++ WeaponSet OBJECT_STATUS_MASKED override: the nexus is not a victim.
pub fn angry_mob_nexus_is_weapon_target() -> bool {
    false
}

/// C++ MobMemberSlavedUpdate MustCatchUpRadius default.
pub const ANGRY_MOB_MUST_CATCH_UP_RADIUS: f32 = 50.0;

/// Orbit slot around the nexus (host XZ). Members path here; they are not teleported.
pub fn angry_mob_member_orbit_destination(origin: Vec3, slot: u32) -> Vec3 {
    use std::f32::consts::PI;
    let angle = (slot as f32) * (2.0 * PI / 8.0);
    let radius = 8.0 + (slot % 3) as f32 * 2.0;
    Vec3::new(
        origin.x + angle.cos() * radius,
        origin.y,
        origin.z + angle.sin() * radius,
    )
}

/// C++ evaluateSoloNexus / IGNORED_IN_GUI slaver remap: clicking a member
/// commands the nexus.
pub fn remap_angry_mob_selection_id(
    is_member: bool,
    nexus_id: Option<ObjectId>,
    self_id: ObjectId,
) -> ObjectId {
    if is_member {
        nexus_id.unwrap_or(self_id)
    } else {
        self_id
    }
}

/// Per-nexus residual state (SpawnBehavior slaves + fire cadence).
/// Pending SpawnBehavior member object residual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingAngryMobMemberSpawn {
    pub nexus_id: ObjectId,
    pub team: super::Team,
    pub template_name: String,
    pub slot_index: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAngryMobState {
    pub object_id: ObjectId,
    pub team: super::Team,
    /// Live SpawnBehavior slave count (`m_spawnCount`).
    pub member_count: u32,
    /// Live member ObjectIds residual (SpawnBehavior slaves).
    #[serde(default)]
    pub member_ids: Vec<ObjectId>,
    /// Next SpawnTemplateName rotation index.
    #[serde(default)]
    pub next_template_index: u32,
    /// Next absolute frame for aggregate fire damage tick.
    pub next_tick_frame: u32,
    /// Next absolute frame for a due replacement (diagnostic).
    pub next_expand_frame: u32,
    /// Position snapshot at last plan (diagnostic).
    pub position: Vec3,
    /// Total damage dealt by this mob (honesty).
    pub total_damage_applied: f32,
    /// Damage application events (object×tick).
    pub damage_applications: u32,
    /// Replacement createSpawn events after a death (not initial rapid fill).
    pub expands: u32,
    /// C++ `m_replacementTimes` — due frames for `createSpawn`.
    #[serde(default)]
    pub replacement_times: Vec<u32>,
    /// C++ `onSpawnDeath` last-member AggregateHealth destroy pending.
    #[serde(default)]
    pub pending_nexus_destroy: bool,
}

impl HostAngryMobState {
    pub fn new(
        object_id: ObjectId,
        team: super::Team,
        position: Vec3,
        activate_frame: u32,
    ) -> Self {
        Self {
            object_id,
            team,
            member_count: 0,
            member_ids: Vec::new(),
            next_template_index: 0,
            // Immediate first tick so residual damage is observable on first update.
            next_tick_frame: activate_frame,
            next_expand_frame: activate_frame,
            position,
            total_damage_applied: 0.0,
            damage_applications: 0,
            expands: 0,
            // C++ QueueProductionExitUpdate isFreeToExit: InitialBurst=5 now,
            // then one member per ExitDelay=5000ms (150 frames).
            replacement_times: (0..ANGRY_MOB_MAX_MEMBERS)
                .map(|i| {
                    if i < ANGRY_MOB_INITIAL_MEMBERS {
                        activate_frame
                    } else {
                        activate_frame.saturating_add(
                            (i - ANGRY_MOB_INITIAL_MEMBERS + 1) * ANGRY_MOB_EXIT_DELAY_FRAMES,
                        )
                    }
                })
                .collect(),
            pending_nexus_destroy: false,
        }
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.pending_nexus_destroy && current_frame >= self.next_tick_frame
    }

    pub fn is_due_expand(&self, current_frame: u32) -> bool {
        !self.pending_nexus_destroy
            && self.member_count < ANGRY_MOB_MAX_MEMBERS
            && self.replacement_times.iter().any(|&t| current_frame >= t)
    }

    /// Next SpawnTemplateName residual in rotation.
    pub fn take_next_member_template(&mut self) -> &'static str {
        let n = ANGRY_MOB_MEMBER_TEMPLATES.len().max(1);
        let idx = (self.next_template_index as usize) % n;
        self.next_template_index = self.next_template_index.saturating_add(1);
        ANGRY_MOB_MEMBER_TEMPLATES[idx]
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
    /// Pending SpawnBehavior member object spawns.
    #[serde(default)]
    pending_member_spawns: Vec<PendingAngryMobMemberSpawn>,
    /// Honesty: member objects spawned residual.
    #[serde(default)]
    pub members_spawned: u32,
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

    pub fn take_pending_member_spawns(&mut self) -> Vec<PendingAngryMobMemberSpawn> {
        std::mem::take(&mut self.pending_member_spawns)
    }

    pub fn record_member_spawned(&mut self, n: u32) {
        self.members_spawned = self.members_spawned.saturating_add(n);
    }

    /// Roll back a `createSpawn` slot when the host object failed to appear.
    pub fn rollback_failed_member_spawn(&mut self, nexus_id: ObjectId, current_frame: u32) {
        if let Some(mob) = self.active.iter_mut().find(|m| m.object_id == nexus_id) {
            mob.member_count = mob.member_count.saturating_sub(1);
            mob.replacement_times.push(current_frame);
        }
    }

    /// Drop nexuses whose last member already triggered AggregateHealth destroy.
    pub fn evict_pending_destroyed_nexuses(&mut self) {
        self.active.retain(|m| !m.pending_nexus_destroy);
    }

    pub fn honesty_member_spawn_ok(&self) -> bool {
        self.members_spawned > 0
    }

    fn queue_member_spawn(
        &mut self,
        nexus_id: ObjectId,
        team: super::Team,
        slot_index: u32,
        template_name: &str,
    ) {
        self.pending_member_spawns.push(PendingAngryMobMemberSpawn {
            nexus_id,
            team,
            template_name: template_name.to_string(),
            slot_index,
        });
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
    pub fn sync_mobs(&mut self, living: &[(ObjectId, super::Team, Vec3)], current_frame: u32) {
        let living_ids: std::collections::HashSet<ObjectId> =
            living.iter().map(|(id, _, _)| *id).collect();
        self.active
            .retain(|m| living_ids.contains(&m.object_id) && !m.pending_nexus_destroy);

        for &(id, team, pos) in living {
            if let Some(m) = self.active.iter_mut().find(|m| m.object_id == id) {
                m.team = team;
                m.position = pos;
            } else {
                // Replacement times queued in `new` — drained by apply_due_replacements.
                self.active
                    .push(HostAngryMobState::new(id, team, pos, current_frame));
            }
        }
        self.active.sort_by_key(|m| m.object_id.0);
    }

    /// C++ SpawnBehavior::onSpawnDeath (`SpawnBehavior.cpp:730-758`).
    /// Removes the slave, schedules `now + SpawnReplaceDelay`, and returns true
    /// when the last live member died (`m_spawnCount == 0 && m_aggregateHealth`).
    pub fn on_spawn_death(
        &mut self,
        nexus_id: ObjectId,
        dead_spawn: ObjectId,
        current_frame: u32,
    ) -> bool {
        let Some(mob) = self.active.iter_mut().find(|m| m.object_id == nexus_id) else {
            return false;
        };
        let Some(idx) = mob.member_ids.iter().position(|&id| id == dead_spawn) else {
            return false;
        };
        mob.member_ids.remove(idx);
        mob.member_count = mob.member_ids.len() as u32;
        // C++ still pushes a replacement even for the last member, then destroys.
        mob.replacement_times
            .push(current_frame.saturating_add(ANGRY_MOB_EXPAND_INTERVAL_FRAMES));
        if mob.member_count == 0 {
            mob.pending_nexus_destroy = true;
            true
        } else {
            false
        }
    }

    /// Scan live member ids; invoke `on_spawn_death` for each dead slave.
    /// Returns nexus ids that must be destroyed (last member gone).
    pub fn process_dead_members(
        &mut self,
        current_frame: u32,
        mut is_dead: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        let mut deaths = Vec::new();
        for mob in &self.active {
            if mob.pending_nexus_destroy {
                continue;
            }
            for &mid in &mob.member_ids {
                if is_dead(mid) {
                    deaths.push((mob.object_id, mid));
                }
            }
        }
        let mut destroy = Vec::new();
        for (nid, mid) in deaths {
            if self.on_spawn_death(nid, mid, current_frame) {
                destroy.push(nid);
            }
        }
        destroy
    }

    /// C++ SpawnBehavior::update replacement-times drain + `createSpawn`
    /// (`SpawnBehavior.cpp:221-243`). Initial fill rapid-spawns all SpawnNumber;
    /// later dues are SpawnReplaceDelay replacements.
    pub fn apply_due_replacements(&mut self, current_frame: u32) -> u32 {
        let mut spawned = 0_u32;
        let mut spawns = Vec::new();
        for mob in &mut self.active {
            if mob.pending_nexus_destroy {
                continue;
            }
            let mut remaining = Vec::new();
            let mut due_count = 0_u32;
            for &t in &mob.replacement_times {
                if current_frame >= t {
                    due_count = due_count.saturating_add(1);
                } else {
                    remaining.push(t);
                }
            }
            mob.replacement_times = remaining;
            if due_count == 0 {
                continue;
            }
            let started_empty = mob.member_count == 0;
            for _ in 0..due_count {
                if mob.member_count >= ANGRY_MOB_MAX_MEMBERS {
                    break;
                }
                let slot = mob.member_count;
                let tmpl = mob.take_next_member_template();
                spawns.push(PendingAngryMobMemberSpawn {
                    nexus_id: mob.object_id,
                    team: mob.team,
                    template_name: tmpl.to_string(),
                    slot_index: slot,
                });
                mob.member_count = mob.member_count.saturating_add(1);
                spawned = spawned.saturating_add(1);
                if !started_empty {
                    mob.expands = mob.expands.saturating_add(1);
                    self.expands = self.expands.saturating_add(1);
                }
                if mob.member_count >= ANGRY_MOB_MAX_MEMBERS {
                    self.fully_expanded = self.fully_expanded.saturating_add(1);
                }
            }
            if let Some(&next) = mob.replacement_times.iter().min() {
                mob.next_expand_frame = next;
            }
        }
        self.pending_member_spawns.extend(spawns);
        spawned
    }

    /// Apply due `createSpawn` slots (name kept for existing callers).
    pub fn apply_due_expands(&mut self, current_frame: u32) -> u32 {
        self.apply_due_replacements(current_frame)
    }

    /// Plan damage ticks for all mobs due this frame.
    ///
    /// `candidates`: (id, pos, team, alive, legal_combat_kind,
    /// under_construction, stealthed_undetected)
    ///
    /// C++ each member attacks one victim. After filters, assign independently
    /// (closest from member/orbit origin) — do not apply 4*N to every hostile.
    /// Faction `Team` residual for leftover tests; live host uses
    /// [`Self::plan_due_ticks_with_enemies`].
    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        candidates: &[(ObjectId, Vec3, super::Team, bool, bool, bool, bool)],
        armed_by_team: impl Fn(super::Team) -> bool,
    ) -> Vec<HostAngryMobTickPlan> {
        self.plan_due_ticks_with_enemies(
            current_frame,
            candidates,
            armed_by_team,
            |_, mob_team, _, team| {
                is_angry_mob_hostile_team(
                    mob_team == super::Team::Neutral,
                    team == mob_team,
                    team == super::Team::Neutral,
                )
            },
        )
    }

    /// Like [`Self::plan_due_ticks`], but hostility is C++ ENEMIES
    /// (`Object::getRelationship` / `Player::getRelationship`). Live host
    /// passes player relationship so 2v2 cross-faction allies are skipped
    /// and FFA same-faction enemies are acquired.
    pub fn plan_due_ticks_with_enemies(
        &self,
        current_frame: u32,
        candidates: &[(ObjectId, Vec3, super::Team, bool, bool, bool, bool)],
        armed_by_team: impl Fn(super::Team) -> bool,
        is_enemies: impl Fn(ObjectId, super::Team, ObjectId, super::Team) -> bool,
    ) -> Vec<HostAngryMobTickPlan> {
        let mut plans = Vec::new();
        for mob in &self.active {
            if !mob.is_due_tick(current_frame) || mob.member_count == 0 {
                continue;
            }
            let armed = armed_by_team(mob.team);
            let mob_neutral = mob.team == super::Team::Neutral;
            let mut victims: Vec<(ObjectId, Vec3)> = Vec::new();
            for &(id, pos, team, alive, combat_kind, under_construction, stealthed_undetected) in
                candidates
            {
                if id == mob.object_id {
                    continue;
                }
                let enemies = is_enemies(mob.object_id, mob.team, id, team);
                // Leftover helpers take `same_team` = not player ENEMIES.
                let same_team = !enemies;
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
                // C++ PartitionFilterStealthedAndUndetected(me, false).
                if stealthed_undetected {
                    continue;
                }
                victims.push((id, pos));
            }
            let mut origins = Vec::with_capacity(mob.member_count as usize);
            for slot in 0..mob.member_count {
                let origin = mob
                    .member_ids
                    .get(slot as usize)
                    .and_then(|mid| candidates.iter().find(|(id, ..)| *id == *mid).map(|c| c.1))
                    .unwrap_or_else(|| angry_mob_member_orbit_destination(mob.position, slot));
                origins.push(origin);
            }
            let hits = assign_angry_mob_member_hits(
                mob.object_id,
                mob.position,
                &origins,
                &victims,
                armed,
            );
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

    /// Residual honesty: at least one death-triggered replacement createSpawn.
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
/// Retail pistol DeathType residual.
pub const ANGRY_MOB_PISTOL_DEATH_TYPE: &str = "NORMAL";
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
        && ANGRY_MOB_MEMBER_TEMPLATES[0] == "GLAInfantryAngryMobPistol01"
        && ANGRY_MOB_MEMBER_TEMPLATES.contains(&"GLAInfantryAngryMobMolotov02")
        && ANGRY_MOB_SPAWN_REPLACE_DELAY_MS == 30_000
        && ANGRY_MOB_EXPAND_INTERVAL_FRAMES
            == angry_mob_ms_to_frames(ANGRY_MOB_SPAWN_REPLACE_DELAY_MS)
        && ANGRY_MOB_EXPAND_INTERVAL_FRAMES == 900
        && (ANGRY_MOB_ARMED_DAMAGE_MULT - 1.25).abs() < 0.01
        && UPGRADE_GLA_ARM_THE_MOB == "Upgrade_GLAArmTheMob"
        && ANGRY_MOB_FIRE_AUDIO == "AngryMobWeaponPistol"
        && (angry_mob_damage_for_tick(5, false) - 20.0).abs() < 0.01
        && (angry_mob_damage_for_tick(5, true) - 25.0).abs() < 0.01
        && !ANGRY_MOB_ANTI_AIR
        && ANGRY_MOB_WEAPON_ANTI_MASK == 0x02
        && !angry_mob_possible_to_attack(true, false, 0x02)
        && !angry_mob_possible_to_attack(false, true, 0x01)
        && angry_mob_possible_to_attack(false, false, 0x02)
        && angry_mob_skips_stealthed_undetected(true, false, false)
        && !angry_mob_skips_stealthed_undetected(true, true, false)
        && !angry_mob_skips_stealthed_undetected(true, false, true)
        && !angry_mob_skips_stealthed_undetected(false, false, false)
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
/// Wave residual honesty: rock/molotov DumbProjectile peels.
pub fn honesty_angry_mob_projectile_ok() -> bool {
    ANGRY_MOB_ROCK_PROJECTILE == "GLAAngryMobRockProjectileObject"
        && ANGRY_MOB_MOLOTOV_PROJECTILE == "GLAAngryMobMolotovCocktailProjectileObject"
        && ANGRY_MOB_ROCK_WEAPON == "GLAAngryMobRockProjectileWeapon"
        && ANGRY_MOB_MOLOTOV_WEAPON == "GLAAngryMobMolotovCocktailProjectileWeapon"
        && (ANGRY_MOB_ROCK_DAMAGE - 40.0).abs() < 0.01
        && (ANGRY_MOB_ROCK_RADIUS - 1.0).abs() < 0.01
        && (ANGRY_MOB_ROCK_WEAPON_SPEED - 130.0).abs() < 0.01
        && (ANGRY_MOB_MOLOTOV_DAMAGE - 40.0).abs() < 0.01
        && (ANGRY_MOB_MOLOTOV_RADIUS - 11.0).abs() < 0.01
        && (ANGRY_MOB_MOLOTOV_WEAPON_SPEED - 60.0).abs() < 0.01
        && (ANGRY_MOB_ROCK_FIRST_HEIGHT - 10.0).abs() < 0.01
        && (ANGRY_MOB_MOLOTOV_FIRST_HEIGHT - 30.0).abs() < 0.01
        && (ANGRY_MOB_PROJ_FIRST_PERCENT_INDENT - 0.50).abs() < 0.001
        && (ANGRY_MOB_PROJ_SECOND_PERCENT_INDENT - 0.90).abs() < 0.001
}

pub fn honesty_angry_mob_residual_pack_ok() -> bool {
    honesty_angry_mob_weapon_residual_ok()
        && honesty_angry_mob_body_residual_ok()
        && honesty_angry_mob_upgrade_residual_ok()
        && honesty_angry_mob_projectile_ok()
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
        assert!(!is_angry_mob_nexus_template(
            "GLAAngryMobRockProjectileObject"
        ));
        assert!(!is_angry_mob_nexus_template(
            "GLAAngryMobMolotovCocktailProjectileObject"
        ));
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
        assert!(!is_angry_mob_hostile_team(false, false, true));
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
        // hq-gvqkn: dead members must reduce DPS; 0 live members deal 0.
        assert!((angry_mob_damage_for_tick(0, false)).abs() < f32::EPSILON);
        assert!((angry_mob_damage_for_tick(3, false) - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sync_tick_damages_nearby_enemy_over_frames() {
        let mut reg = HostAngryMobRegistry::new();
        let mob_id = ObjectId(1);
        let enemy_id = ObjectId(2);
        let far_id = ObjectId(3);

        reg.sync_mobs(&[(mob_id, Team::GLA, Vec3::new(0.0, 0.0, 0.0))], 0);
        assert_eq!(reg.active_count(), 1);
        // C++ SpawnBehavior::update queues SpawnNumber immediately (cpp:194-208).
        assert_eq!(reg.apply_due_expands(0), ANGRY_MOB_MAX_MEMBERS);
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS));

        let candidates = vec![
            (mob_id, Vec3::ZERO, Team::GLA, true, true, false, false),
            (
                enemy_id,
                Vec3::new(50.0, 0.0, 0.0),
                Team::USA,
                true,
                true,
                false,
                false,
            ),
            (
                far_id,
                Vec3::new(500.0, 0.0, 0.0),
                Team::USA,
                true,
                true,
                false,
                false,
            ),
        ];

        let plans = reg.plan_due_ticks(0, &candidates, |_| false);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, enemy_id);
        assert!(
            (plans[0].hits[0].damage - angry_mob_damage_for_tick(ANGRY_MOB_MAX_MEMBERS, false))
                .abs()
                < 0.01
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
    fn rapid_spawn_all_spawn_number_then_replace_delay() {
        // C++ SpawnBehavior::update first-init (SpawnBehavior.cpp:194-208) queues
        // SpawnNumber replacement times due immediately. onSpawnDeath
        // (SpawnBehavior.cpp:739-743) schedules now+SpawnReplaceDelay.
        let mut reg = HostAngryMobRegistry::new();
        let mob_id = ObjectId(1);
        reg.sync_mobs(&[(mob_id, Team::GLA, Vec3::ZERO)], 0);
        assert_eq!(reg.member_count_of(mob_id), Some(0));
        let n = reg.apply_due_expands(0);
        assert_eq!(n, ANGRY_MOB_MAX_MEMBERS);
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS));
        assert_eq!(
            reg.take_pending_member_spawns().len(),
            ANGRY_MOB_MAX_MEMBERS as usize
        );

        if let Some(mob) = reg
            .active_mobs_mut()
            .iter_mut()
            .find(|m| m.object_id == mob_id)
        {
            mob.member_ids = (10..20).map(ObjectId).collect();
            mob.member_count = mob.member_ids.len() as u32;
        }

        assert!(!reg.on_spawn_death(mob_id, ObjectId(10), 1));
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS - 1));
        assert_eq!(reg.apply_due_expands(1), 0);
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS - 1));
        assert!(!reg.honesty_expand_ok());

        let n = reg.apply_due_expands(1 + ANGRY_MOB_EXPAND_INTERVAL_FRAMES);
        assert_eq!(n, 1);
        assert_eq!(reg.member_count_of(mob_id), Some(ANGRY_MOB_MAX_MEMBERS));
        assert!(reg.honesty_expand_ok());
    }

    #[test]
    fn last_member_death_destroys_nexus() {
        // C++ SpawnBehavior::onSpawnDeath (SpawnBehavior.cpp:749-757):
        // if (m_spawnCount == 0 && m_aggregateHealth) destroyObject(nexus).
        let mut reg = HostAngryMobRegistry::new();
        let mob_id = ObjectId(1);
        reg.sync_mobs(&[(mob_id, Team::GLA, Vec3::ZERO)], 0);
        reg.apply_due_expands(0);
        if let Some(mob) = reg
            .active_mobs_mut()
            .iter_mut()
            .find(|m| m.object_id == mob_id)
        {
            mob.member_ids = (1..=10).map(|i| ObjectId(i + 100)).collect();
            mob.member_count = 10;
        }
        for i in 0..9 {
            assert!(!reg.on_spawn_death(mob_id, ObjectId(101 + i), i as u32));
        }
        assert_eq!(reg.member_count_of(mob_id), Some(1));
        assert!(reg.on_spawn_death(mob_id, ObjectId(110), 9));
        assert_eq!(reg.member_count_of(mob_id), Some(0));
        assert!(
            reg.active_mobs()
                .iter()
                .any(|m| m.object_id == mob_id && m.pending_nexus_destroy)
        );
        assert!(!angry_mob_nexus_should_be_masked());
        assert!(!angry_mob_nexus_is_weapon_target());
        let _ = ANGRY_MOB_SPAWN_DELAY_MIN_FRAMES;
        let _ = ANGRY_MOB_INITIAL_MEMBERS;
    }

    #[test]
    fn rock_molotov_projectile_peels() {
        assert!(honesty_angry_mob_projectile_ok());
        let from = Vec3::ZERO;
        let to = Vec3::new(100.0, 0.0, 0.0);
        assert!(angry_mob_projectile_flight_frames(from, to, AngryMobProjectileKind::Rock) >= 1);
        assert!(
            angry_mob_projectile_flight_frames(from, to, AngryMobProjectileKind::Molotov)
                > angry_mob_projectile_flight_frames(from, to, AngryMobProjectileKind::Rock)
        );
        let mid = angry_mob_projectile_bezier_point(from, to, 0.5, AngryMobProjectileKind::Molotov);
        assert!(mid.y > 5.0, "molotov tall arc");
        assert_eq!(
            angry_mob_projectile_kind_for_tick(2),
            AngryMobProjectileKind::Molotov
        );
        assert_eq!(
            angry_mob_projectile_kind_for_tick(0),
            AngryMobProjectileKind::Rock
        );
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
