use super::projectile_lifecycle::ProjectileLifecycle;
use crate::damage::{CombatDamage, DeathOutcome};
use crate::object::{ObjectStatusTypes, VeterancyLevel};
use crate::time::LOGIC_FRAMES_PER_SECOND;
use generals_game_domain::{ObjectId, Team};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Authored speed sentinels that resolve without travel.
pub fn is_instant_weapon_speed(speed: f32) -> bool {
    speed <= 0.0 || speed >= 999_999.0
}

/// Unit of the pending speed and `min_weapon_speed` pair. Weapon.ini stores
/// these in distance per 30 Hz logic frame; low-level CombatSystem callers
/// often already supply distance per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectileSpeedUnit {
    DistancePerLogicFrame,
    DistancePerSecond,
}

impl ProjectileSpeedUnit {
    fn distance_per_logic_frame(self, speed: f32) -> f32 {
        match self {
            Self::DistancePerLogicFrame => speed,
            Self::DistancePerSecond => speed / LOGIC_FRAMES_PER_SECOND,
        }
    }

    pub fn distance_per_second(self, speed: f32) -> f32 {
        if is_instant_weapon_speed(speed) {
            return speed;
        }
        match self {
            Self::DistancePerLogicFrame => speed * LOGIC_FRAMES_PER_SECOND,
            Self::DistancePerSecond => speed,
        }
    }
}

/// Data needed to spawn a projectile (enqueued by Object::fire_at).
///
/// C++ executes `FireOCL` synchronously while the firing object still exists.
/// The host queues projectile creation for the combat phase, so retain the
/// source transform/team at acceptance time instead of sampling a potentially
/// deleted or re-owned object while draining that queue.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProjectileLaunchContext {
    pub source_team: Team,
    /// Exact default Team assigned to the C++ projectile object at admission.
    /// This is projectile ownership, not the shooter's possibly custom Team.
    #[serde(skip)]
    pub projectile_team_id: Option<u32>,
    /// Controlling player frozen at fire acceptance (C++ Object::getRelationship).
    pub source_owner_player_id: Option<u32>,
    pub source_veterancy: VeterancyLevel,
    pub source_orientation: f32,
    pub source_velocity: Vec3,
    /// Exact firing barrel transform captured at acceptance, when available.
    pub launch_transform: Option<glam::Mat4>,
    /// Launcher Object conditions captured by projectileLaunchAtObjectOrPosition.
    pub launcher_bonus_mask: u64,
}

/// Data needed to spawn a projectile (enqueued by Object::fire_at).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingProjectile {
    pub shooter_id: ObjectId,
    pub shooter_pos: Vec3,
    /// Frozen firing state used by FireOCL. Older synthetic/test callers may
    /// omit it; drain then uses a live source only when one is available.
    pub source_context: Option<ProjectileLaunchContext>,
    pub target_id: Option<ObjectId>,
    /// Target position for position-based attacks. For object-based attacks
    /// (target_id = Some), the drain function resolves the position from the
    /// objects map and falls back to this value if the target is gone.
    pub target_pos: Option<Vec3>,
    /// Source position for a self-position weapon, sampled after fire effects.
    /// The intended target remains separate for travel-time calculation.
    #[serde(skip)]
    pub self_damage_position: Option<Vec3>,
    /// Whether the accepted weapon is a laser, from its owning definition.
    /// None is reserved for old or unbound programmatic shots.
    #[serde(skip)]
    pub selected_weapon_is_laser: Option<bool>,
    pub damage: f32,
    pub speed: f32,
    /// Shared by `speed` and `min_weapon_speed`; both come from the same source.
    pub speed_unit: ProjectileSpeedUnit,
    /// C++ radius damage residual at impact (0 = direct only).
    pub splash_radius: f32,
    /// C++ projectile homing residual (retarget velocity toward live target).
    pub is_homing: bool,
    /// Host combat damage class residual for Armor.ini coefficients.
    pub damage_type: CombatDamage,
    /// C++ Weapon.ini DeathType residual applied on killing blow.
    pub death_type: DeathOutcome,
    /// C++ Weapon.ini ProjectileObject residual template name (empty = hitscan/no mesh).
    pub projectile_object_name: String,
    /// Parsed Object INI behavior frozen at fire acceptance.  This keeps
    /// projectile materialization independent of later content lookups.
    pub projectile_lifecycle: Option<ProjectileLifecycle>,
    /// C++ Weapon.ini FireFX residual (played at the source when the shot is
    /// accepted, before projectile flight).
    pub fire_fx_name: String,
    /// C++ Weapon.ini FireOCL residual (executed at the source object at fire).
    pub fire_ocl_name: String,
    /// C++ Weapon.ini ProjectileDetonationFX residual (empty = no impact FX name).
    pub detonation_fx_name: String,
    /// C++ Weapon.ini ProjectileDetonationOCL residual (empty = no impact OCL name).
    pub detonation_ocl_name: String,
    /// C++ Weapon.ini ProjectileExhaust residual (empty = no in-flight trail name).
    pub exhaust_name: String,
    /// C++ SecondaryDamage residual.
    pub secondary_damage: f32,
    /// C++ SecondaryDamageRadius residual.
    pub secondary_damage_radius: f32,
    /// C++ ShockWaveAmount residual.
    pub shock_wave_amount: f32,
    /// C++ ShockWaveRadius residual.
    pub shock_wave_radius: f32,
    /// C++ ShockWaveTaperOff residual.
    pub shock_wave_taper_off: f32,
    /// C++ RadiusDamageAffects residual mask.
    pub radius_damage_affects: u32,
    /// C++ ProjectileCollidesWith residual mask.
    pub projectile_collides: u32,
    /// C++ effective ScatterRadius residual at fire time (0 = no scatter).
    pub scatter_radius: f32,
    /// C++ ScatterTarget table offset (host XZ). When set, ScatterRadius is skipped.
    pub scatter_table_offset: Option<glam::Vec2>,

    /// C++ MinWeaponSpeed residual (used when ScaleWeaponSpeed).
    pub min_weapon_speed: f32,
    /// C++ ScaleWeaponSpeed residual flag.
    pub scale_weapon_speed: bool,
    /// C++ AttackRange residual for ScaleWeaponSpeed ratio.
    pub attack_range: f32,
    /// C++ MinimumAttackRange residual for ScaleWeaponSpeed ratio.
    pub min_attack_range: f32,
    /// C++ HistoricBonus residual peels (stamped at fire).
    pub historic_weapon_key: String,
    /// C++ DamageInfoInput::m_damageStatusType copied from the fired
    /// WeaponTemplate (Weapon.cpp dealDamageInternal). Instant/laser status
    /// weapons never persist in flight, so it is not part of the wire format.
    #[serde(skip)]
    pub damage_status_type: Option<ObjectStatusTypes>,
    pub historic_bonus_time_frames: u32,
    pub historic_bonus_count: i32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_weapon: String,
    /// C++ MissileCallsOnDie residual.
    pub die_on_detonate: bool,
}

/// Projectileless impact carrying the accepted shot and its delivery frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "LiveProjectilelessDelayedDamage")]
pub struct DelayedDamage {
    when: u32,
    pub pending: PendingProjectile,
    pub damage_pos: Vec3,
    pub damage_id: Option<ObjectId>,
}

impl DelayedDamage {
    pub fn new(
        when: u32,
        pending: PendingProjectile,
        damage_pos: Vec3,
        damage_id: Option<ObjectId>,
    ) -> Self {
        Self {
            when,
            pending,
            damage_pos,
            damage_id,
        }
    }

    pub fn frame(&self) -> u32 {
        self.when
    }

    /// Authored weapon name frozen at acceptance for owner-side rule lookups.
    pub fn weapon_key(&self) -> &str {
        &self.pending.historic_weapon_key
    }

    /// Beam source, muzzle origin, and resolved shot/scatter aim for the
    /// LaserName object born before Body damage.
    pub fn laser_endpoints(&self) -> (ObjectId, Vec3, Vec3) {
        (
            self.pending.shooter_id,
            self.pending.shooter_pos,
            self.damage_pos,
        )
    }
}

impl PendingProjectile {
    /// Travel time for WeaponTemplate's projectileless impact, in logic frames.
    pub fn damage_delay_frames(&self, victim_position: Vec3) -> f32 {
        let source = self.self_damage_position.unwrap_or(self.shooter_pos);
        let speed_per_frame = self.speed_unit.distance_per_logic_frame(self.speed);
        if speed_per_frame > 0.0 && !is_instant_weapon_speed(self.speed) {
            source.distance(victim_position) / speed_per_frame
        } else {
            0.0
        }
    }
}

/// Accepted and materialized deliveries for one receiving combat instance.
#[derive(Debug, Default)]
pub struct ShotQueue {
    accepted: Vec<PendingProjectile>,
    delayed: Vec<DelayedDamage>,
}

impl ShotQueue {
    pub fn accept(&mut self, mut shot: PendingProjectile) {
        if let Some(lifecycle) = shot.projectile_lifecycle {
            shot.is_homing = lifecycle.follows_target()
                && shot.target_id.is_some()
                && !is_instant_weapon_speed(shot.speed);
        }
        self.accepted.push(shot);
    }

    pub fn clear_accepted(&mut self) {
        self.accepted.clear();
    }

    pub fn take_accepted(&mut self) -> Vec<PendingProjectile> {
        std::mem::take(&mut self.accepted)
    }

    pub fn accepted(&self) -> &[PendingProjectile] {
        &self.accepted
    }

    pub fn defer(&mut self, shot: DelayedDamage) {
        self.delayed.push(shot);
    }

    /// WeaponTemplate::fireWeaponTemplate delivers sub-frame travel now and
    /// queues longer travel at the rounded-up logic frame.
    pub fn schedule_projectileless(
        &mut self,
        frame: u32,
        travel_frames: f32,
        shot: PendingProjectile,
        damage_pos: Vec3,
        damage_id: Option<ObjectId>,
    ) -> Option<DelayedDamage> {
        if travel_frames < 1.0 {
            return Some(DelayedDamage::new(frame, shot, damage_pos, damage_id));
        }
        let delay = game_engine::base_type::real_to_int_ceil(travel_frames) as u32;
        self.defer(DelayedDamage::new(
            frame.wrapping_add(delay),
            shot,
            damage_pos,
            damage_id,
        ));
        None
    }

    pub fn delayed(&self) -> &[DelayedDamage] {
        &self.delayed
    }

    /// WeaponStore::update scans in insertion order, skipping future entries.
    /// Leave the cursor at the removed entry so the caller can apply one impact
    /// and append another due entry before the scan resumes.
    pub fn take_due(&mut self, current_frame: u32, cursor: &mut usize) -> Option<DelayedDamage> {
        while let Some(shot) = self.delayed.get(*cursor) {
            if shot.when <= current_frame {
                return Some(self.delayed.remove(*cursor));
            }
            *cursor += 1;
        }
        None
    }

    pub fn snapshot(&self) -> PendingCombatSnapshot {
        PendingCombatSnapshot {
            accepted: self.accepted.clone(),
            delayed: self.delayed.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PendingCombatSnapshot) {
        self.accepted.clone_from(&snapshot.accepted);
        self.delayed.clone_from(&snapshot.delayed);
    }
}

// Typed Rust world-save records. C++ WeaponStore is not a Snapshot subsystem.
// These queues bridge host acceptance to later materialization; preserving
// their records does not claim the original C++ WeaponStore wire layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingCombatSnapshot {
    accepted: Vec<PendingProjectile>,
    delayed: Vec<DelayedDamage>,
}

impl PendingCombatSnapshot {
    /// Validate both accepted and delayed records before committing a restore.
    pub fn validate_launch_transforms(&self) -> Result<(), String> {
        if self
            .accepted
            .iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .filter_map(|pending| pending.source_context.as_ref())
            .filter_map(|context| context.launch_transform)
            .any(|transform| !transform.is_finite())
        {
            return Err("Invalid pending projectile launch transform".into());
        }
        Ok(())
    }

    pub fn projectile_team_ids(&self) -> Vec<Option<u32>> {
        self.accepted
            .iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .map(|pending| {
                pending
                    .source_context
                    .and_then(|context| context.projectile_team_id)
            })
            .collect()
    }

    pub fn projectileless_weapon_details(&self) -> Vec<(Option<[f32; 3]>, Option<bool>)> {
        self.accepted
            .iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .map(|pending| {
                (
                    pending
                        .self_damage_position
                        .map(|position| position.to_array()),
                    pending.selected_weapon_is_laser,
                )
            })
            .collect()
    }

    pub fn restore_projectileless_weapon_details(
        &mut self,
        details: &[(Option<[f32; 3]>, Option<bool>)],
    ) -> Result<(), String> {
        if details.len() != self.accepted.len() + self.delayed.len() {
            return Err("Pending projectileless weapon detail count mismatch".into());
        }
        if details.iter().any(|(position, _)| {
            position.is_some_and(|position| position.iter().any(|value| !value.is_finite()))
        }) {
            return Err("Invalid pending self-position damage location".into());
        }
        for (pending, (position, is_laser)) in self
            .accepted
            .iter_mut()
            .chain(self.delayed.iter_mut().map(|entry| &mut entry.pending))
            .zip(details.iter().copied())
        {
            pending.self_damage_position = position.map(Vec3::from_array);
            pending.selected_weapon_is_laser = is_laser;
        }
        Ok(())
    }

    pub fn restore_projectile_team_ids(&mut self, team_ids: &[Option<u32>]) -> Result<(), String> {
        if team_ids.len() != self.accepted.len() + self.delayed.len() {
            return Err("Pending projectile Team identity count mismatch".into());
        }
        for (pending, team_id) in self
            .accepted
            .iter_mut()
            .chain(self.delayed.iter_mut().map(|entry| &mut entry.pending))
            .zip(team_ids.iter().copied())
        {
            if let Some(context) = &mut pending.source_context {
                context.projectile_team_id = team_id;
            } else if team_id.is_some() {
                return Err("Pending projectile Team identity has no launch context".into());
            }
        }
        Ok(())
    }
}
