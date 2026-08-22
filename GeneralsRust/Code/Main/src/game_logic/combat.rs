use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// C++ `DamageType` (Damage.h:26-70) on the live host fire_at → take_damage path.
/// Legacy host names stay as real variants (match-pattern compatible). Missing
/// C++ types are first-class so `map_store_damage_type` no longer collapses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DamageType {
    #[default]
    Bullet,
    Explosive,
    Fire,
    Laser,
    Toxin,
    Radiation,
    EMP,
    /// C++ DAMAGE_MICROWAVE (Damage.h:63) — ordinary HP through armor.
    /// Not IsSubdualDamage. Host EMP remains a leftover alias of this type.
    Microwave,
    Flame,
    Anthrax,
    Unresistable,
    /// C++ DAMAGE_FALLING residual (PhysicsBehavior landing splat).
    Falling,
    /// C++ DAMAGE_STATUS residual (doStatusDamage; amount = duration msec).
    Status,
    /// C++ DAMAGE_KILL_PILOT residual (vehicle unmanned; no HP).
    KillPilot,
    /// C++ DAMAGE_DISARM residual (safe mine clear without detonation).
    Disarm,
    /// C++ DAMAGE_DEPLOY residual (AssaultTransport beginAssault; no HP).
    Deploy,
    /// C++ DAMAGE_HACK residual (timer-based hack; no HP on fire).
    Hack,
    /// C++ DAMAGE_SURRENDER (Damage.h:42). Retail ALLOW_SURRENDER is off;
    /// lethal hits deal normal HP (ActiveBody.cpp:517-537 compiled out).
    Surrender,
    /// C++ DAMAGE_PENALTY residual (game-rule HP damage; no radar event).
    Penalty,
    /// C++ DAMAGE_KILL_GARRISONED residual (kill floor(amount) occupants; no structure HP).
    KillGarrisoned,
    /// C++ DAMAGE_HEALING residual (attemptHealing; amount restores HP, never destroys).
    Healing,
    /// C++ DAMAGE_WATER residual (underwater / waveguide HP damage; no dusty FX).
    Water,
    /// C++ DAMAGE_CRUSH residual (SquishCollide / PhysicsUpdate crush).
    Crush,
    ArmorPiercing,
    Gattling,
    Sniper,
    Melee,
    HazardCleanup,
    ParticleBeam,
    Toppling,
    InfantryMissile,
    AuroraBomb,
    LandMine,
    JetMissiles,
    StealthJetMissiles,
    MolotovCocktail,
    ComancheVulcan,
    SubdualMissile,
    SubdualVehicle,
    SubdualBuilding,
    SubdualUnresistable,
}

impl DamageType {
    /// Crate / C++ ordinal identity. `DamageNumTypes` is the count, not a payload.
    #[inline]
    pub fn from_store(dt: gamelogic::damage::DamageType) -> Self {
        use gamelogic::damage::DamageType as G;
        match dt {
            G::Explosion => Self::Explosive,
            G::Crush => Self::Crush,
            G::ArmorPiercing => Self::ArmorPiercing,
            G::SmallArms => Self::Bullet,
            G::Gattling => Self::Gattling,
            G::Radiation => Self::Radiation,
            G::Flame => Self::Flame,
            G::Laser => Self::Laser,
            G::Sniper => Self::Sniper,
            G::Poison => Self::Toxin,
            G::Healing => Self::Healing,
            G::Unresistable => Self::Unresistable,
            G::Water => Self::Water,
            G::Deploy => Self::Deploy,
            G::Surrender => Self::Surrender,
            G::Hack => Self::Hack,
            G::KillPilot => Self::KillPilot,
            G::Penalty => Self::Penalty,
            G::Falling => Self::Falling,
            G::Melee => Self::Melee,
            G::Disarm => Self::Disarm,
            G::HazardCleanup => Self::HazardCleanup,
            G::ParticleBeam => Self::ParticleBeam,
            G::Toppling => Self::Toppling,
            G::InfantryMissile => Self::InfantryMissile,
            G::AuroraBomb => Self::AuroraBomb,
            G::LandMine => Self::LandMine,
            G::JetMissiles => Self::JetMissiles,
            G::StealthJetMissiles => Self::StealthJetMissiles,
            G::MolotovCocktail => Self::MolotovCocktail,
            G::ComancheVulcan => Self::ComancheVulcan,
            G::SubdualMissile => Self::SubdualMissile,
            G::SubdualVehicle => Self::SubdualVehicle,
            G::SubdualBuilding => Self::SubdualBuilding,
            G::SubdualUnresistable => Self::SubdualUnresistable,
            G::Microwave => Self::Microwave,
            G::KillGarrisoned => Self::KillGarrisoned,
            G::Status => Self::Status,
            G::DamageNumTypes => Self::Unresistable,
        }
    }

    #[inline]
    pub fn to_store(self) -> gamelogic::damage::DamageType {
        use gamelogic::damage::DamageType as G;
        match self {
            Self::Bullet => G::SmallArms,
            Self::Explosive => G::Explosion,
            Self::Fire | Self::Flame => G::Flame,
            Self::Laser => G::Laser,
            Self::Toxin | Self::Anthrax => G::Poison,
            Self::Radiation => G::Radiation,
            Self::EMP | Self::Microwave => G::Microwave,
            Self::Unresistable => G::Unresistable,
            Self::Falling => G::Falling,
            Self::Status => G::Status,
            Self::KillPilot => G::KillPilot,
            Self::Disarm => G::Disarm,
            Self::Deploy => G::Deploy,
            Self::Hack => G::Hack,
            Self::Surrender => G::Surrender,
            Self::Penalty => G::Penalty,
            Self::KillGarrisoned => G::KillGarrisoned,
            Self::Healing => G::Healing,
            Self::Water => G::Water,
            Self::Crush => G::Crush,
            Self::ArmorPiercing => G::ArmorPiercing,
            Self::Gattling => G::Gattling,
            Self::Sniper => G::Sniper,
            Self::Melee => G::Melee,
            Self::HazardCleanup => G::HazardCleanup,
            Self::ParticleBeam => G::ParticleBeam,
            Self::Toppling => G::Toppling,
            Self::InfantryMissile => G::InfantryMissile,
            Self::AuroraBomb => G::AuroraBomb,
            Self::LandMine => G::LandMine,
            Self::JetMissiles => G::JetMissiles,
            Self::StealthJetMissiles => G::StealthJetMissiles,
            Self::MolotovCocktail => G::MolotovCocktail,
            Self::ComancheVulcan => G::ComancheVulcan,
            Self::SubdualMissile => G::SubdualMissile,
            Self::SubdualVehicle => G::SubdualVehicle,
            Self::SubdualBuilding => G::SubdualBuilding,
            Self::SubdualUnresistable => G::SubdualUnresistable,
        }
    }

    /// C++ `IsSubdualDamage` (Damage.h:95-107).
    #[inline]
    pub fn is_subdual(self) -> bool {
        matches!(
            self,
            Self::SubdualMissile
                | Self::SubdualVehicle
                | Self::SubdualBuilding
                | Self::SubdualUnresistable
        )
    }

    /// C++ `IsHealthDamagingDamage` (Damage.h:110-127).
    #[inline]
    pub fn is_health_damaging(self) -> bool {
        !matches!(
            self,
            Self::Status
                | Self::SubdualMissile
                | Self::SubdualVehicle
                | Self::SubdualBuilding
                | Self::SubdualUnresistable
                | Self::KillPilot
                | Self::KillGarrisoned
        )
    }
}

/// Armor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorType {
    None,
    Infantry,
    Vehicle,
    Aircraft,
    Structure,
    Flame,
}

/// Damage calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageResult {
    pub final_damage: f32,
    pub damage_type: DamageType,
    pub was_critical: bool,
    pub armor_reduction: f32,
}

/// Projectile for ranged combat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub id: ObjectId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub damage: f32,
    pub damage_type: DamageType,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    pub speed: f32,
    pub lifetime: f32,
    /// Parsed Object INI behavior governing lifetime. `None` means the
    /// ProjectileObject was unresolved or does not expose a supported behavior;
    /// that intentionally has no invented generic timeout.
    pub projectile_lifecycle: Option<crate::game_logic::weapon_bootstrap::HostProjectileLifecycle>,
    /// C++ `MissileAIUpdate::KILL_SELF` entry frame after a followed target
    /// disappears. Kept host-side because GameWorld flight residual only owns
    /// pose/lifetime and has no detonation data.
    pub missile_kill_self_started_frame: Option<u32>,
    /// Parsed lifetime/fuel duration for presentation and the GameWorld flight
    /// mirror. Zero means unlimited or unresolved; it is not a generic expiry.
    pub max_lifetime: f32,
    pub is_homing: bool,
    pub explosion_radius: f32,
    /// C++ DeathType residual carried to kill application.
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    /// C++ ProjectileObject residual for presentation mesh key.
    pub projectile_object_name: String,
    /// C++ Weapon.ini ProjectileDetonationFX residual (spawned at impact).
    pub detonation_fx_name: String,
    /// C++ Weapon.ini ProjectileDetonationOCL residual name (spawned at impact).
    pub detonation_ocl_name: String,
    /// C++ Weapon.ini ProjectileExhaust residual PSys name (in-flight trail).
    pub exhaust_name: String,
    /// Firing object's ownership frozen at launch.  A projectile can outlive
    /// its firing object, while its detonation OCL must still create objects
    /// for the original team.
    pub source_team: crate::game_logic::Team,
    /// Firing object's controlling player frozen at launch (C++ getRelationship).
    #[serde(default)]
    pub source_owner_player_id: Option<u32>,
    /// Firing object's team instance name frozen at launch (team-id overrides).
    #[serde(default)]
    pub source_team_instance_name: String,
    /// Firing object's veterancy frozen at launch for OCL `InheritsVeterancy`.
    pub source_veterancy: crate::game_logic::VeterancyLevel,
    /// C++ SecondaryDamage residual (outer splash ring amount).
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
    /// C++ HistoricBonus weapon-template key (empty = none).
    pub historic_weapon_key: String,
    pub historic_bonus_time_frames: u32,
    pub historic_bonus_count: i32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_weapon: String,
    /// C++ Weapon.ini MissileCallsOnDie residual.
    pub die_on_detonate: bool,
    /// C++ DumbProjectile / MissileAI flight + warhead snapshot.
    pub flight: Option<crate::game_logic::weapon_bootstrap::HostProjectileFlight>,
    /// Runtime Bezier / ignition / lock / layer state.
    pub flight_runtime: crate::game_logic::weapon_bootstrap::HostProjectileRuntime,
    /// C++ MissileAIUpdate `m_framesTillDecoyed` (absolute frame, 0 = none).
    #[serde(default)]
    pub frames_till_decoyed: u32,
    /// C++ MissileAIUpdate `m_noDamage` — diverted decoy path deals no HP.
    #[serde(default)]
    pub no_damage: bool,
    /// Launch already ran `reportMissileForCountermeasures`.
    #[serde(default)]
    pub cm_reported: bool,
    /// C++ `KINDOF_SMALL_MISSILE` on the projectile object.
    #[serde(default)]
    pub is_small_missile: bool,
    /// C++ `MissileAIUpdate::m_exhaustID` from `createAttachedParticleSystemID`.
    #[serde(default)]
    pub leftover_exhaust_id: u32,
}

impl Projectile {
    pub fn new(
        id: ObjectId,
        start_pos: Vec3,
        target_pos: Vec3,
        damage: f32,
        damage_type: DamageType,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
    ) -> Self {
        let direction = (target_pos - start_pos).normalize_or_zero();
        // Caller overwrites speed/velocity via fire_projectile.
        let speed = 0.0;

        Self {
            id,
            position: start_pos,
            velocity: direction * speed,
            target_position: target_pos,
            damage,
            damage_type,
            shooter_id,
            target_id,
            speed,
            lifetime: 0.0,
            projectile_lifecycle: None,
            missile_kill_self_started_frame: None,
            max_lifetime: 0.0,
            is_homing: false,
            explosion_radius: 0.0,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            source_team: crate::game_logic::Team::Neutral,
            source_owner_player_id: None,
            source_team_instance_name: String::new(),
            source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::weapon_bootstrap::WEAPON_AFFECTS_DEFAULT,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
            flight: None,
            flight_runtime: crate::game_logic::weapon_bootstrap::HostProjectileRuntime::default(),
            frames_till_decoyed: 0,
            no_damage: false,
            cm_reported: false,
            is_small_missile: false,
            leftover_exhaust_id: 0,
        }
    }

    /// Attach exact parsed Object INI lifetime behavior at launch.
    pub fn set_projectile_lifecycle(
        &mut self,
        lifecycle: Option<crate::game_logic::weapon_bootstrap::HostProjectileLifecycle>,
    ) {
        self.projectile_lifecycle = lifecycle;
        self.missile_kill_self_started_frame = None;
        self.max_lifetime = lifecycle
            .map(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::lifetime_seconds)
            .unwrap_or(0.0);
        if self.flight.is_none() && !self.projectile_object_name.is_empty() {
            self.flight = crate::game_logic::weapon_bootstrap::host_projectile_flight_for_object_name(
                &self.projectile_object_name,
            );
        }
    }

    /// Bind parsed C++ flight/warhead and build the DumbProjectile Bezier.
    pub fn bind_authored_flight(&mut self, start: Vec3, end: Vec3, speed: f32) {
        if self.flight.is_none() && !self.projectile_object_name.is_empty() {
            self.flight = crate::game_logic::weapon_bootstrap::host_projectile_flight_for_object_name(
                &self.projectile_object_name,
            );
        }
        let Some(flight) = self.flight.clone() else {
            return;
        };
        self.flight_runtime.path_start = start;
        self.flight_runtime.path_end = end;
        self.flight_runtime.original_target_pos = end;
        self.flight_runtime.path_speed_per_frame = if speed > 0.0 { speed / 30.0 } else { 0.0 };
        match &flight {
            crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb) => {
                let (path, segs) = crate::game_logic::weapon_bootstrap::build_dumb_bezier_path(
                    start, end, dumb, speed, 0,
                );
                self.flight_runtime.path = path;
                self.flight_runtime.path_segments = segs;
                self.flight_runtime.step = 0;
                self.flight_runtime.missile_armed = true;
                self.flight_runtime.missile_phase =
                    crate::game_logic::weapon_bootstrap::HostMissilePhase::Attack;
            }
            crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_) => {
                // C++ always starts LAUNCH. Delay 0 still visits IGNITION on the
                // first update so authored IgnitionFX plays (MissileAIUpdate.cpp:675-688).
                self.flight_runtime.missile_armed = false;
                self.flight_runtime.missile_phase =
                    crate::game_logic::weapon_bootstrap::HostMissilePhase::Launch;
            }
        }
    }

    pub fn is_warhead_armed(&self) -> bool {
        match &self.flight {
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_)) => {
                self.flight_runtime.missile_armed
            }
            _ => true,
        }
    }

    /// C++ `MissileAIUpdate::doLaunchState` then fall-through `doIgnitionState`.
    ///
    /// Returns `false` while still waiting `IgnitionDelay` (pose-hold, unarmed).
    /// On the ignition frame: leftover `FXList::doFXObj`, attach exhaust, arm.
    fn try_enter_missile_ignition(&mut self, frame: u32) -> bool {
        let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(missile)) =
            &self.flight
        else {
            return true;
        };
        if self.flight_runtime.missile_armed {
            return true;
        }
        if frame < missile.ignition_delay_frames {
            return false;
        }
        let ignition_fx = missile.ignition_fx.clone();
        self.play_missile_ignition(&ignition_fx);
        self.flight_runtime.missile_armed = true;
        self.flight_runtime.missile_phase =
            crate::game_logic::weapon_bootstrap::HostMissilePhase::Attack;
        true
    }

    fn missile_fx_yaw(&self) -> f32 {
        self.velocity.z.atan2(self.velocity.x)
    }

    fn publish_missile_fx_pose(&self) {
        crate::game_logic::publish_host_fx_object(
            self.id.0,
            self.position,
            self.missile_fx_yaw(),
            self.source_owner_player_id.map(|p| p as i32).unwrap_or(-1),
        );
    }

    /// C++ `MissileAIUpdate::doIgnitionState` (MissileAIUpdate.cpp:462-466).
    fn play_missile_ignition(&mut self, ignition_fx: &str) {
        self.publish_missile_fx_pose();
        if !ignition_fx.is_empty() {
            crate::game_logic::dispatch_fx_list_at_object(ignition_fx, self.id.0, None);
        }
        if !self.exhaust_name.is_empty() && self.leftover_exhaust_id == 0 {
            if let Some(mgr) = gamelogic::helpers::TheParticleSystemManager::get() {
                if let Some(id) = mgr.create_attached_particle_system_id(
                    Some(self.exhaust_name.as_str()),
                    self.id.0,
                ) {
                    self.leftover_exhaust_id = id;
                }
            }
        }
    }

    /// World-space exhaust residual after IGNITION. Leftover attach wins so
    /// C++ `createAttachedParticleSystemID` is not doubled by host sync.
    pub fn live_exhaust_name(&self) -> &str {
        if self.exhaust_name.is_empty() {
            return "";
        }
        match &self.flight {
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_)) => {
                if self.flight_runtime.missile_armed && self.leftover_exhaust_id == 0 {
                    self.exhaust_name.as_str()
                } else {
                    ""
                }
            }
            _ => self.exhaust_name.as_str(),
        }
    }

    pub fn publish_attached_exhaust_pose(&self) {
        if self.leftover_exhaust_id != 0 {
            self.publish_missile_fx_pose();
        }
    }

    /// Whether an authored MissileAIUpdate has entered C++ `KILL_SELF`.
    ///
    /// Do not infer this from lifetime alone: unresolved or other projectile
    /// behaviors deliberately remain in normal flight.  The GameWorld bridge
    /// uses this narrowly typed predicate to suspend only coupled pose
    /// integration while the host retains the projectile for contrail delay.
    pub(crate) fn is_missile_kill_self_holding(&self) -> bool {
        matches!(
            self.projectile_lifecycle,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
        ) && self.missile_kill_self_started_frame.is_some()
    }

    fn elapsed_logic_frames(&self) -> u32 {
        // Host combat is ordinarily stepped at exactly 30 Hz. Round the
        // observer duration back to frames so GameWorld's sole flight step can
        // write the same age back before host impact behavior resolves.
        (self.lifetime.max(0.0) * 30.0).round() as u32
    }

    fn lifecycle_step(&mut self, target_is_live: bool) -> ProjectileStep {
        use crate::game_logic::weapon_bootstrap::HostProjectileLifecycle;

        let Some(lifecycle) = self.projectile_lifecycle else {
            return ProjectileStep::Alive;
        };
        let frame = self.elapsed_logic_frames();
        match lifecycle {
            HostProjectileLifecycle::DumbProjectile {
                max_lifespan_frames,
            } if frame >= max_lifespan_frames => ProjectileStep::Detonate,
            HostProjectileLifecycle::Missile {
                try_to_follow_target,
                fuel_lifetime_frames,
                detonate_on_no_fuel,
                kill_self_delay_frames,
            } => {
                if let Some(kill_started) = self.missile_kill_self_started_frame {
                    return if frame >= kill_started.saturating_add(kill_self_delay_frames) {
                        ProjectileStep::Remove
                    } else {
                        ProjectileStep::Hold
                    };
                }

                // MissileAIUpdate::doAttackState checks fuel before its
                // tracked-target-gone transition.  A missile which loses its
                // target on precisely its authored no-fuel frame therefore
                // still detonates when DetonateOnNoFuel is set.  `detonate()`
                // then enters KILL_SELF so its contrail can catch up before
                // the projectile object is destroyed.
                let fuel_start = match &self.flight {
                    Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(
                        missile,
                    )) => missile.ignition_delay_frames,
                    _ => 0,
                };
                if fuel_lifetime_frames > 0
                    && frame >= fuel_start.saturating_add(fuel_lifetime_frames)
                    && detonate_on_no_fuel
                {
                    self.missile_kill_self_started_frame = Some(frame);
                    return ProjectileStep::DetonateAndHold;
                }

                // C++ MissileAIUpdate only invokes airborneTargetGone for a
                // missile which was launched at an object *and* was authored
                // to follow it. That transition is a delayed silent destroy,
                // even when DetonateOnNoFuel is true.
                if try_to_follow_target && self.target_id.is_some() && !target_is_live {
                    self.missile_kill_self_started_frame = Some(frame);
                    return ProjectileStep::Hold;
                }

                // `FuelLifetime = 0` means infinity. With fuel exhausted but
                // no DetonateOnNoFuel, C++ stops acceleration/turning and
                // discards exhaust; it does not fabricate a detonation or
                // remove the missile here.
                ProjectileStep::Alive
            }
            _ => ProjectileStep::Alive,
        }
    }

    pub fn update(&mut self, dt: f32, target_is_live: bool) -> ProjectileStep {
        if dt.is_finite() && dt > 0.0 {
            self.lifetime += dt;
        }

        // C++ runs LAUNCH/IGNITION before airborneTargetGone (doAttackState).
        let frame = self.elapsed_logic_frames();
        if !self.try_enter_missile_ignition(frame) {
            return ProjectileStep::Alive;
        }

        let lifecycle_step = self.lifecycle_step(target_is_live);
        if lifecycle_step != ProjectileStep::Alive {
            return lifecycle_step;
        }

        if self.speed <= 0.0 {
            self.position = self.target_position;
            return self.finish_flight_step();
        }

        if matches!(
            &self.flight,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(_))
        ) && !self.flight_runtime.path.is_empty()
        {
            if self.flight_runtime.step >= self.flight_runtime.path.len() {
                return ProjectileStep::Detonate;
            }
            let next = self.flight_runtime.path[self.flight_runtime.step];
            let tangent = next - self.position;
            if tangent.length_squared() > 0.0001 {
                self.velocity = tangent.normalize() * self.speed;
            }
            self.position = next;
            self.flight_runtime.step += 1;
            return self.finish_flight_step();
        }

        if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(missile)) =
            &self.flight
        {
            if self.flight_runtime.missile_armed
                && self.flight_runtime.missile_phase
                    != crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
                && crate::game_logic::weapon_bootstrap::missile_inside_lock_distance(
                    self.position,
                    self.target_position,
                    missile.lock_distance,
                    self.is_homing,
                )
            {
                self.flight_runtime.missile_phase =
                    crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill;
            }
            if self.flight_runtime.missile_phase
                == crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
            {
                let goal = self.target_position;
                let close_enough = (self.speed / 30.0).max(1.0);
                if self.position.distance(goal) <= close_enough {
                    self.position = goal;
                    return ProjectileStep::Detonate;
                }
                let dir = (goal - self.position).normalize_or_zero();
                self.velocity = dir * self.speed;
                self.position += self.velocity * dt;
                return self.finish_flight_step();
            }
        }

        if self.is_homing {
            let dir = (self.target_position - self.position).normalize_or_zero();
            self.velocity = dir * self.speed;
        }
        self.position += self.velocity * dt;
        self.finish_flight_step()
    }

    fn finish_flight_step(&mut self) -> ProjectileStep {
        let armed = self.is_warhead_armed();
        if let Some((snapped, new_layer)) =
            crate::game_logic::weapon_bootstrap::bridge_deck_detonate_pose(
                self.position,
                self.flight_runtime.layer,
                armed,
            )
        {
            let old_layer = self.flight_runtime.layer;
            self.flight_runtime.layer = new_layer;
            if armed && old_layer != 1 && new_layer == 1 && snapped != self.position {
                self.position = snapped;
                return ProjectileStep::Detonate;
            }
        }
        ProjectileStep::Alive
    }

    /// True when C++ weapon speed is instant-hit residual (laser / hitscan).
    pub fn is_instant_speed(speed: f32) -> bool {
        speed <= 0.0 || speed >= 999_999.0
    }
}

/// Result of one host projectile behavior step.  `Detonate` is deliberately
/// distinct from `Remove`: C++ DumbProjectile expiry and missile no-fuel
/// detonation must execute the weapon's authored impact path, whereas followed
/// target loss enters MissileAIUpdate's quiet delayed self-destroy state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectileStep {
    Alive,
    Hold,
    /// Missile `detonate()` emitted its authored impact, then entered
    /// `KILL_SELF`; retain it for the parsed contrail delay.
    DetonateAndHold,
    Detonate,
    Remove,
}

/// Projectile hit information
#[derive(Debug, Clone)]
pub enum ProjectileHit {
    Direct {
        target_id: ObjectId,
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        radius: f32,
        shooter_id: ObjectId,
    },
}

/// Damage event information  
#[derive(Debug, Clone)]
pub enum DamageEvent {
    Direct {
        target_id: ObjectId,
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        shooter_id: ObjectId,
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        radius: f32,
        shooter_id: ObjectId,
        /// Outer-ring SecondaryDamage residual (0 = C++ step uses 0 outside primary).
        secondary_damage: f32,
        secondary_radius: f32,
        /// C++ ShockWave residual (0 amount = no push).
        shock_wave_amount: f32,
        shock_wave_radius: f32,
        shock_wave_taper_off: f32,
        /// C++ RadiusDamageAffects residual mask.
        radius_damage_affects: u32,
        /// Shooter team frozen at impact for ally/enemy filter.
        shooter_team: crate::game_logic::Team,
        /// Shooter controlling player frozen at impact (C++ getRelationship).
        shooter_owner_player_id: Option<u32>,
        /// Shooter team instance frozen at impact (PLAYER_SET_OVERRIDE_RELATION_TO_TEAM).
        shooter_team_instance_name: String,
        /// Shooter template name residual (NOT_SIMILAR `isEquivalentTo` filter).
        shooter_template: String,
        /// C++ `primaryVictim`: intended target skips RadiusDamageAffects.
        primary_victim: Option<ObjectId>,
    },
}

/// Projectile impact FX residual (ProjectileDetonationFX at real hit).
#[derive(Debug, Clone)]
pub struct ProjectileImpactFx {
    pub position: Vec3,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    pub detonation_fx_name: String,
    pub detonation_ocl_name: String,
    /// Frozen launch ownership used by the host OCL bridge when the shooter
    /// was destroyed before the projectile detonated.
    pub source_team: crate::game_logic::Team,
    pub source_veterancy: crate::game_logic::VeterancyLevel,
    /// Current projectile transform peels needed by generic OCL dispositions.
    pub source_orientation: f32,
    pub source_velocity: Vec3,
}

/// C++ `Weapon::fireWeaponTemplate` fire-time authored effects.  `FireFX` and
/// `FireOCL` run when the shot is accepted, before the projectile needs a live
/// target, so they are kept separately from projectile impact events.
#[derive(Debug, Clone)]
pub struct WeaponFireOcl {
    pub origin: Vec3,
    pub shooter_id: ObjectId,
    pub source_team: crate::game_logic::Team,
    pub source_veterancy: crate::game_logic::VeterancyLevel,
    pub source_orientation: f32,
    pub source_velocity: Vec3,
    /// C++ Weapon.ini `FireFX` selected for the source's frozen veterancy.
    pub fire_fx_name: String,
    pub fire_ocl_name: String,
}

/// Combat system manager
#[derive(Debug)]
pub struct CombatSystem {
    projectiles: HashMap<ObjectId, Projectile>,
    next_projectile_id: ObjectId,
    /// Impacts carrying ProjectileDetonationFX residual (drained by GameLogic).
    impact_fx: Vec<ProjectileImpactFx>,
    /// C++ TheRadar->tryUnderAttackEvent victims from this projectile pass.
    pending_under_attack: Vec<ObjectId>,
    /// Parsed fire-time FX/OCL references accepted with queued projectile shots.
    fire_ocl: Vec<WeaponFireOcl>,
}

/// Global projectile spawn queue. Objects call this when firing, and the
/// game loop drains it each frame into the CombatSystem.
static PENDING_PROJECTILES: std::sync::Mutex<Vec<PendingProjectile>> =
    std::sync::Mutex::new(Vec::new());

/// Data needed to spawn a projectile (enqueued by Object::fire_at).
///
/// C++ executes `FireOCL` synchronously while the firing object still exists.
/// The host queues projectile creation for the combat phase, so retain the
/// source transform/team at acceptance time instead of sampling a potentially
/// deleted or re-owned object while draining that queue.
#[derive(Debug, Clone, Copy)]
pub struct ProjectileLaunchContext {
    pub source_team: crate::game_logic::Team,
    /// Controlling player frozen at fire acceptance (C++ Object::getRelationship).
    pub source_owner_player_id: Option<u32>,
    pub source_veterancy: crate::game_logic::VeterancyLevel,
    pub source_orientation: f32,
    pub source_velocity: Vec3,
}

/// Data needed to spawn a projectile (enqueued by Object::fire_at).
#[derive(Debug, Clone)]
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
    pub damage: f32,
    pub speed: f32,
    /// C++ radius damage residual at impact (0 = direct only).
    pub splash_radius: f32,
    /// C++ projectile homing residual (retarget velocity toward live target).
    pub is_homing: bool,
    /// Host combat damage class residual for Armor.ini coefficients.
    pub damage_type: DamageType,
    /// C++ Weapon.ini DeathType residual applied on killing blow.
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    /// C++ Weapon.ini ProjectileObject residual template name (empty = hitscan/no mesh).
    pub projectile_object_name: String,
    /// Parsed Object INI behavior frozen at fire acceptance.  This keeps
    /// deferred/shadow projectile materialization from re-reading or guessing
    /// the projectile template later.
    pub projectile_lifecycle: Option<crate::game_logic::weapon_bootstrap::HostProjectileLifecycle>,
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
    pub historic_bonus_time_frames: u32,
    pub historic_bonus_count: i32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_weapon: String,
    /// C++ MissileCallsOnDie residual.
    pub die_on_detonate: bool,
}

/// Queue a projectile for spawning. Called from Object::fire_at().
pub fn queue_projectile(mut pending: PendingProjectile) {
    stamp_pending_projectile_lifecycle(&mut pending);
    // Defer only when a live shadow session will drain host_fire_spawn_log.
    // Host-only (shadow off) must enqueue immediately or combat never spawns shots.
    // Wave 682: under coupled tick, host_fire_spawn_log is drained immediately
    // after the host logic frame (eager post-logic apply) — still record here.
    if crate::gameworld_shadow::gameworld_fire_spawn_authority_live() {
        crate::game_logic::host_fire_spawn_log::record(pending);
        return;
    }
    if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        queue.push(pending);
    }
}

/// Unconditional enqueue for shadow fire-spawn apply (bypasses authority gate).
pub fn queue_projectile_direct(mut pending: PendingProjectile) {
    stamp_pending_projectile_lifecycle(&mut pending);
    if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        queue.push(pending);
    }
}

/// Freeze the exact parsed Object INI lifecycle on the queue record.  A caller
/// can supply a pre-resolved lifecycle (for a copied shadow event); otherwise
/// only the parsed `ProjectileObject` is consulted.  Unknown objects stay
/// `None`, which intentionally means no synthesized timeout/detonation.
fn stamp_pending_projectile_lifecycle(pending: &mut PendingProjectile) {
    if pending.projectile_lifecycle.is_none() {
        pending.projectile_lifecycle =
            crate::game_logic::weapon_bootstrap::host_projectile_lifecycle_for_object_name(
                &pending.projectile_object_name,
            );
    }
    if let Some(lifecycle) = pending.projectile_lifecycle {
        // Parsed MissileAIUpdate semantics supersede the old broad weapon
        // target-mask heuristic. Dumb projectile paths are not homing in this
        // lightweight bridge unless a future parsed flight-path adjustment
        // explicitly supports it.
        pending.is_homing = lifecycle.follows_target()
            && pending.target_id.is_some()
            && !Projectile::is_instant_speed(pending.speed);
    }
}

/// Test helper: length of the static pending projectile queue.
#[cfg(test)]
pub fn pending_projectile_queue_len_for_test() -> usize {
    PENDING_PROJECTILES.lock().map(|q| q.len()).unwrap_or(0)
}

/// Test helper: clear static pending projectile queue.
#[cfg(test)]
pub fn clear_pending_projectile_queue_for_test() {
    if let Ok(mut q) = PENDING_PROJECTILES.lock() {
        q.clear();
    }
}

/// Test helper: last queued projectile DamageType (fire_at → take_damage path).
#[cfg(test)]
pub fn last_pending_projectile_damage_type_for_test() -> Option<DamageType> {
    PENDING_PROJECTILES
        .lock()
        .ok()
        .and_then(|q| q.last().map(|p| p.damage_type))
}

/// Test helper: last queued projectile secondary-ring amount.
#[cfg(test)]
pub fn last_pending_projectile_secondary_damage_for_test() -> Option<f32> {
    PENDING_PROJECTILES
        .lock()
        .ok()
        .and_then(|q| q.last().map(|p| p.secondary_damage))
}

/// C++ `TheTerrainLogic->getBridgeAttackPoints` nearer end (Weapon.cpp:819-831).
pub fn nearer_live_bridge_attack_point(from: glam::Vec3, victim: &Object) -> glam::Vec3 {
    let pos = victim.get_position();
    let half = victim.selection_radius.max(20.0);
    let a = glam::Vec3::new(pos.x - half, pos.y, pos.z);
    let b = glam::Vec3::new(pos.x + half, pos.y, pos.z);
    if from.distance_squared(a) <= from.distance_squared(b) {
        a
    } else {
        b
    }
}

/// Drain all pending projectiles and spawn them into the combat system.
/// Resolves target object positions from the objects map.
pub fn drain_pending_projectiles(combat: &mut CombatSystem, objects: &HashMap<ObjectId, Object>) {
    let pending = if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        std::mem::take(&mut *queue)
    } else {
        Vec::new()
    };

    for p in pending {
        // Queue acceptance normally froze this field. Retain the parsed-only
        // fallback for legacy shadow records created before that bridge; an
        // absent/unresolved Object INI still remains `None`.
        let projectile_lifecycle = p.projectile_lifecycle.or_else(|| {
            crate::game_logic::weapon_bootstrap::host_projectile_lifecycle_for_object_name(
                &p.projectile_object_name,
            )
        });
        // C++ Weapon::fireWeaponTemplate executes FireOCL at the source when
        // the weapon fires, independent of whether a later projectile target
        // can still be resolved.  Freeze the source state at launch because
        // the source may die before the host drains this queue.
        let source_context = p.source_context.or_else(|| {
            objects
                .get(&p.shooter_id)
                .map(|source| ProjectileLaunchContext {
                    source_team: source.team,
                    source_owner_player_id: source.owner_player_id,
                    source_veterancy: source.experience.level,
                    source_orientation: source.get_orientation(),
                    source_velocity: source.movement.velocity,
                })
        });
        let source_context = source_context.unwrap_or(ProjectileLaunchContext {
            source_team: crate::game_logic::Team::Neutral,
            source_owner_player_id: None,
            source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
            source_orientation: 0.0,
            source_velocity: Vec3::ZERO,
        });
        let has_fire_fx = !p.fire_fx_name.trim().is_empty()
            && !p.fire_fx_name.trim().eq_ignore_ascii_case("None");
        let has_fire_ocl = !p.fire_ocl_name.trim().is_empty()
            && !p.fire_ocl_name.trim().eq_ignore_ascii_case("None");
        // C++ `Weapon::fireWeaponTemplate` handles FireFX and FireOCL at
        // acceptance independently.  In particular, a target that vanishes
        // before this deferred host queue drains must not erase its muzzle FX.
        if has_fire_fx || has_fire_ocl {
            combat.fire_ocl.push(WeaponFireOcl {
                origin: p.shooter_pos,
                shooter_id: p.shooter_id,
                source_team: source_context.source_team,
                source_veterancy: source_context.source_veterancy,
                source_orientation: source_context.source_orientation,
                source_velocity: source_context.source_velocity,
                fire_fx_name: p.fire_fx_name.clone(),
                fire_ocl_name: p.fire_ocl_name.clone(),
            });
        }

        let now = crate::game_logic::host_historic_bonus::logic_frame();
        let actual_target_pos = p.target_id.and_then(|tid| objects.get(&tid)).map(|obj| {
            if let Some(off) = obj.get_sneaky_targeting_offset(now) {
                obj.get_position() + off
            } else if obj.template_name.to_ascii_lowercase().contains("bridge") {
                nearer_live_bridge_attack_point(p.shooter_pos, obj)
            } else {
                obj.get_position()
            }
        }).or(p.target_pos);

        let Some(mut target_pos) = actual_target_pos else {
            continue;
        };

        // C++ Weapon.ini ScatterRadius residual: offset aim point and clear
        // direct target lock when scatter > 0 (miss / near-miss residual).
        let mut fire_target_id = p.target_id;
        if let Some(tid) = p.target_id {
            if let Some(obj) = objects.get(&tid) {
                if obj.get_sneaky_targeting_offset(now).is_some() {
                    fire_target_id = None;
                }
            }
        }
        // MissileAIUpdate only retains a goal Object when TryToFollowTarget is
        // authored. A coordinate-flight missile must detonate at its frozen
        // launch point even if the original object disappears later.
        if matches!(
            projectile_lifecycle,
            Some(
                crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile {
                    try_to_follow_target: false,
                    ..
                }
            )
        ) {
            fire_target_id = None;
        }
        if let Some(table_offset) = p.scatter_table_offset {
            // C++ privateFireWeapon unused ScatterTarget pick: victim becomes
            // a position, Z snapped to ground. Host Y is up.
            target_pos.x += table_offset.x;
            target_pos.z += table_offset.y;
            if let Some(target) = p.target_id.and_then(|tid| objects.get(&tid)) {
                if target.ground_height_from_terrain {
                    target_pos.y = target.ground_height;
                }
            }
            fire_target_id = None;
        } else if p.scatter_radius > 0.0 {
            // C++ fireWeaponTemplate: STRUCTURE aims at geometry center, then
            // each shot rolls GameLogicRandomValueReal radius + angle.
            if let Some(target) = p.target_id.and_then(|tid| objects.get(&tid)) {
                if target.is_kind_of(crate::game_logic::KindOf::Structure) {
                    target_pos =
                        crate::game_logic::weapon_bootstrap::structure_scatter_aim_origin(
                            target_pos,
                            &target.thing.template.geometry_info,
                        );
                }
            }
            let offset = crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(
                p.scatter_radius,
            );
            target_pos += offset;
            // C++ snaps Z to the victim pathfind layer so a miss hits dirt,
            // not a far point past the volume. Host Y is up.
            if let Some(target) = p.target_id.and_then(|tid| objects.get(&tid)) {
                if target.ground_height_from_terrain {
                    target_pos.y = target.ground_height;
                }
            }
            fire_target_id = None;
        }

        // C++ DumbProjectileBehavior ScaleWeaponSpeed residual (2D range ratio).
        let mut flight_speed = p.speed;
        if p.scale_weapon_speed {
            let dx = target_pos.x - p.shooter_pos.x;
            let dz = target_pos.z - p.shooter_pos.z;
            let range_2d = (dx * dx + dz * dz).sqrt();
            let peel = crate::game_logic::weapon_bootstrap::HostWeaponSpeedPeel {
                weapon_speed: p.speed,
                min_weapon_speed: p.min_weapon_speed,
                scale_weapon_speed: true,
                attack_range: p.attack_range,
                min_attack_range: p.min_attack_range,
            };
            flight_speed =
                crate::game_logic::weapon_bootstrap::host_scaled_weapon_speed(&peel, range_2d)
                    .max(0.0);
        }

        let weapon = Weapon {
            damage: p.damage,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: flight_speed,
            pre_attack_delay: 0.0,
            splash_radius: p.splash_radius,
            suspend_fx_frame: 0,
                    reloading_clip: false,
            last_bonus_rof: 0.0,
};
        let pid = combat.fire_projectile_ex(
            p.shooter_pos,
            target_pos,
            &weapon,
            p.shooter_id,
            fire_target_id,
            flight_speed,
            projectile_lifecycle
                .map(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::follows_target)
                .unwrap_or(p.is_homing),
        );
        if let Some(proj) = combat.projectile_mut(pid) {
            proj.damage_type = p.damage_type;
            proj.death_type = p.death_type;
            proj.projectile_object_name = p.projectile_object_name.clone();
            proj.detonation_fx_name = p.detonation_fx_name.clone();
            proj.detonation_ocl_name = p.detonation_ocl_name.clone();
            proj.exhaust_name = p.exhaust_name.clone();
            proj.source_team = source_context.source_team;
            proj.source_owner_player_id = source_context.source_owner_player_id.or_else(|| {
                objects.get(&p.shooter_id).and_then(|o| o.owner_player_id)
            });
            proj.source_team_instance_name = objects
                .get(&p.shooter_id)
                .map(|o| o.team_instance_name.clone())
                .unwrap_or_default();
            proj.source_veterancy = source_context.source_veterancy;
            proj.secondary_damage = p.secondary_damage;
            proj.secondary_damage_radius = p.secondary_damage_radius;
            proj.shock_wave_amount = p.shock_wave_amount;
            proj.shock_wave_radius = p.shock_wave_radius;
            proj.shock_wave_taper_off = p.shock_wave_taper_off;
            proj.radius_damage_affects = p.radius_damage_affects;
            proj.historic_weapon_key = p.historic_weapon_key.clone();
            proj.historic_bonus_time_frames = p.historic_bonus_time_frames;
            proj.historic_bonus_count = p.historic_bonus_count;
            proj.historic_bonus_radius = p.historic_bonus_radius;
            proj.historic_bonus_weapon = p.historic_bonus_weapon.clone();
            proj.die_on_detonate = p.die_on_detonate;
            proj.projectile_collides = p.projectile_collides;
            proj.set_projectile_lifecycle(projectile_lifecycle);
            proj.bind_authored_flight(p.shooter_pos, target_pos, flight_speed);
            proj.is_small_missile = host_projectile_is_small_missile(proj);
        }
    }
}

impl Default for CombatSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {
            projectiles: HashMap::new(),
            next_projectile_id: ObjectId(100000), // Start high to avoid conflicts with objects
            impact_fx: Vec::new(),
            pending_under_attack: Vec::new(),
            fire_ocl: Vec::new(),
        }
    }

    /// Snapshot active projectiles for PresentationFrame freeze (read-only).
    pub fn projectiles_snapshot(&self) -> Vec<&Projectile> {
        self.projectiles.values().collect()
    }

    /// Drain ProjectileDetonationFX residual events produced by the last update.
    pub fn take_impact_fx(&mut self) -> Vec<ProjectileImpactFx> {
        std::mem::take(&mut self.impact_fx)
    }

    /// Drain victims that took live projectile/area damage this pass.
    pub fn take_pending_under_attack(&mut self) -> Vec<ObjectId> {
        std::mem::take(&mut self.pending_under_attack)
    }

    /// C++ Object.cpp:1847-1849: enqueue only when actual HP was dealt and
    /// the type is not DAMAGE_PENALTY / DAMAGE_HEALING. Local-player,
    /// sourcePlayerMask, and radar-data gates run in
    /// `GameLogic::try_under_attack_from_damage`.
    fn queue_under_attack_if_dealt(
        &mut self,
        victim_id: ObjectId,
        damage_type: DamageType,
        hp_lost: f32,
    ) {
        if hp_lost > 0.0
            && !matches!(
                damage_type,
                DamageType::Penalty | DamageType::Healing
            )
        {
            self.pending_under_attack.push(victim_id);
        }
    }

    /// Drain FireOCL events emitted while pending shots were materialized.
    pub fn take_fire_ocl(&mut self) -> Vec<WeaponFireOcl> {
        std::mem::take(&mut self.fire_ocl)
    }

    pub fn projectile_count(&self) -> usize {
        self.projectiles.len()
    }

    pub fn projectile_mut(&mut self, id: ObjectId) -> Option<&mut Projectile> {
        self.projectiles.get_mut(&id)
    }

    /// Remove one projectile by id (GameWorld projectile-authority writeback).
    pub fn remove_projectile(&mut self, id: ObjectId) -> bool {
        self.projectiles.remove(&id).is_some()
    }

    /// Fire a projectile from one object to another
    pub fn fire_projectile(
        &mut self,
        shooter_pos: Vec3,
        target_pos: Vec3,
        weapon: &Weapon,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
        speed: f32,
    ) -> ObjectId {
        self.fire_projectile_ex(
            shooter_pos,
            target_pos,
            weapon,
            shooter_id,
            target_id,
            speed,
            false,
        )
    }

    /// Fire with explicit homing residual.
    pub fn fire_projectile_ex(
        &mut self,
        shooter_pos: Vec3,
        target_pos: Vec3,
        weapon: &Weapon,
        shooter_id: ObjectId,
        target_id: Option<ObjectId>,
        speed: f32,
        is_homing: bool,
    ) -> ObjectId {
        let projectile_id = self.next_projectile_id;
        self.next_projectile_id = ObjectId(self.next_projectile_id.0 + 1);

        let mut projectile = Projectile::new(
            projectile_id,
            shooter_pos,
            target_pos,
            weapon.damage,
            DamageType::Bullet, // Default, would be set by weapon type
            shooter_id,
            target_id,
        );

        // C++ radius damage residual from WeaponTemplate splash/radius.
        projectile.explosion_radius = weapon.splash_radius.max(0.0);
        projectile.is_homing = is_homing && !Projectile::is_instant_speed(speed);

        if Projectile::is_instant_speed(speed) {
            // Laser / hitscan residual: spawn already at impact for same-frame resolve.
            projectile.speed = 0.0;
            projectile.velocity = Vec3::ZERO;
            projectile.position = target_pos;
            projectile.target_position = target_pos;
        } else {
            let spd = if speed > 0.0 { speed } else { 200.0 };
            let dir = (target_pos - shooter_pos).normalize_or_zero();
            projectile.speed = spd;
            projectile.velocity = dir * spd;
        }

        self.projectiles.insert(projectile_id, projectile);

        projectile_id
    }

    /// Update all projectiles

    fn maybe_record_historic_bonus(
        projectile: &Projectile,
        impact_pos: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) {
        if projectile.historic_bonus_count <= 0 {
            return;
        }
        let peel = crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel {
            time_frames: projectile.historic_bonus_time_frames,
            count: projectile.historic_bonus_count,
            radius: projectile.historic_bonus_radius,
            bonus_weapon: projectile.historic_bonus_weapon.clone(),
        };
        if !peel.is_active() {
            return;
        }
        let team = objects
            .get(&projectile.shooter_id)
            .map(|o| o.team)
            .unwrap_or(crate::game_logic::Team::Neutral);
        let key = if projectile.historic_weapon_key.is_empty() {
            "weapon"
        } else {
            projectile.historic_weapon_key.as_str()
        };
        let _ = crate::game_logic::host_historic_bonus::record_impact(
            key,
            &peel,
            impact_pos,
            projectile.shooter_id,
            team,
        );
    }

    pub fn update_projectiles(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_countermeasures(dt, objects, None, 0)
    }

    /// Flight integrate only (lifetime + pose). Hit/detonation behavior is
    /// separate, so this helper cannot silently discard a parsed expiry.
    pub fn integrate_projectiles_only(&mut self, dt: f32) -> usize {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            1.0 / 30.0
        };
        let ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();
        let mut stepped = 0usize;
        for id in ids {
            let Some(p) = self.projectiles.get_mut(&id) else {
                continue;
            };
            // This pose-only caller has no target-status input. Defer parsed
            // target-loss and detonation behavior to the complete combat pass.
            if p.update(dt, true) == ProjectileStep::Alive {
                stepped += 1;
            }
        }
        stepped
    }

    /// Refresh homing aim points from live object positions.
    pub fn refresh_homing_aims(&mut self, objects: &HashMap<ObjectId, Object>) {
        for p in self.projectiles.values_mut() {
            if !p.is_homing {
                continue;
            }
            if let Some(tid) = p.target_id {
                if let Some(tgt) = objects.get(&tid) {
                    if tgt.is_alive() {
                        p.target_position = tgt.get_position();
                    }
                }
            }
        }
    }

    fn splash_area_event(
        projectile: &Projectile,
        objects: &HashMap<ObjectId, Object>,
        position: Vec3,
    ) -> DamageEvent {
        let live = objects.get(&projectile.shooter_id);
        DamageEvent::Area {
            position,
            damage: projectile.damage,
            damage_type: projectile.damage_type,
            death_type: if projectile.die_on_detonate {
                crate::game_logic::host_usa_pilot::HostDeathType::Detonated
            } else {
                projectile.death_type
            },
            radius: projectile.explosion_radius,
            shooter_id: projectile.shooter_id,
            secondary_damage: projectile.secondary_damage,
            secondary_radius: projectile.secondary_damage_radius,
            shock_wave_amount: projectile.shock_wave_amount,
            shock_wave_radius: projectile.shock_wave_radius,
            shock_wave_taper_off: projectile.shock_wave_taper_off,
            radius_damage_affects: projectile.radius_damage_affects,
            shooter_team: live.map(|o| o.team).unwrap_or(projectile.source_team),
            shooter_owner_player_id: live
                .and_then(|o| o.owner_player_id)
                .or(projectile.source_owner_player_id),
            shooter_team_instance_name: live
                .map(|o| o.team_instance_name.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| projectile.source_team_instance_name.clone()),
            shooter_template: live
                .map(|o| o.template_name.clone())
                .unwrap_or_default(),
            primary_victim: projectile.target_id,
        }
    }

    /// Projectile step with optional America Countermeasures diversion residual.
    pub fn update_projectiles_with_countermeasures(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
    ) -> Vec<ObjectId> {
        self.update_projectiles_with_relationships(dt, objects, countermeasures, frame, None)
    }

    /// Projectile step that applies RadiusDamageAffects via GameWorld player relationships.
    pub fn update_projectiles_with_relationships(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
    ) -> Vec<ObjectId> {
        let projectile_ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();

        // Process projectile updates
        let mut damage_events = Vec::new();
        let mut projectiles_to_remove = Vec::new();

        for proj_id in projectile_ids {
            if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
                if let Some(reg) = countermeasures.as_mut() {
                    apply_countermeasure_report_and_decoy(
                        projectile,
                        proj_id,
                        objects,
                        reg,
                        frame,
                    );
                }
                let target_is_live = projectile
                    .target_id
                    .map(|target_id| objects.get(&target_id).is_some_and(Object::is_alive))
                    .unwrap_or(true);
                // Homing residual: refresh aim point from live target before step.
                if projectile.is_homing {
                    if let Some(tid) = projectile.target_id {
                        if let Some(tgt) = objects.get(&tid) {
                            if tgt.is_alive() {
                                projectile.target_position = tgt.get_position();
                            }
                        }
                    }
                }
                if let Some(
                    crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb),
                ) = projectile.flight.clone()
                {
                    if dumb.flight_path_adjust_per_frame > 0.0 {
                        if let Some(tid) = projectile.target_id {
                            if let Some(tgt) = objects.get(&tid) {
                                if tgt.is_alive() {
                                    crate::game_logic::weapon_bootstrap::adjust_flight_path_end(
                                        &mut projectile.flight_runtime,
                                        &dumb,
                                        tgt.get_position(),
                                    );
                                    projectile.target_position = projectile.flight_runtime.path_end;
                                }
                            }
                        }
                    }
                }
                match projectile.update(dt, target_is_live) {
                    ProjectileStep::Alive => {
                        if !projectile.is_warhead_armed() {
                            continue;
                        }
                    }
                    ProjectileStep::Hold => {
                        // C++ MissileAIUpdate::KILL_SELF holds for the parsed
                        // contrail delay, with no impact FX/OCL or damage.
                        continue;
                    }
                    ProjectileStep::Remove => {
                        projectiles_to_remove.push(proj_id);
                        continue;
                    }
                    step @ (ProjectileStep::Detonate | ProjectileStep::DetonateAndHold) => {
                        // C++ DumbProjectileBehavior lifespan and
                        // MissileAIUpdate DetonateOnNoFuel both call the
                        // detonation weapon at the projectile's current pose.
                        // A zero-radius weapon has no guessed direct victim.
                        let impact = projectile.position;
                        Self::maybe_record_historic_bonus(projectile, impact, objects);
                        if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                            damage_events.push(Self::splash_area_event(projectile, objects, impact));
                        }
                        if !projectile.detonation_fx_name.is_empty()
                            || !projectile.detonation_ocl_name.is_empty()
                        {
                            self.impact_fx.push(ProjectileImpactFx {
                                position: impact,
                                shooter_id: projectile.shooter_id,
                                target_id: None,
                                detonation_fx_name: projectile.detonation_fx_name.clone(),
                                detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                source_team: projectile.source_team,
                                source_veterancy: projectile.source_veterancy,
                                source_orientation: projectile
                                    .velocity
                                    .z
                                    .atan2(projectile.velocity.x),
                                source_velocity: projectile.velocity,
                            });
                        }
                        if step == ProjectileStep::Detonate {
                            projectiles_to_remove.push(proj_id);
                        }
                        continue;
                    }
                }

                // Intervening structure residual: ballistic shells impact the first
                // constructed building whose footprint contains the projectile.
                // Gated by Weapon.ini ProjectileCollidesWith leftover mask.
                // C++ Weapon.cpp:716-721 — same-controller structures only if
                // CONTROLLED_STRUCTURES; STRUCTURES is everyone else.
                // Skip intended target (handled below) and shooter.
                let mut hit_structure: Option<ObjectId> = None;
                let collides_structures =
                    crate::game_logic::weapon_bootstrap::projectile_collides_structures(
                        projectile.projectile_collides,
                    );
                if collides_structures {
                    let shooter = projectile.shooter_id;
                    let intended = projectile.target_id;
                    let pos = projectile.position;
                    let sneak_now = crate::game_logic::host_historic_bonus::logic_frame();
                    let shooter_obj = objects.get(&shooter);
                    let proj_owner = shooter_obj
                        .and_then(|s| s.owner_player_id)
                        .or(projectile.source_owner_player_id);
                    let proj_team = shooter_obj
                        .map(|s| s.team)
                        .unwrap_or(projectile.source_team);
                    // C++ Weapon.cpp:663-666 — never collide with the launcher's container.
                    let launcher_contained_by = shooter_obj.and_then(|s| s.contained_by);
                    // C++ Weapon.cpp:670-677 — Flame/ParticleBeam skip already-burned.
                    let skip_burned = matches!(
                        projectile.damage_type,
                        DamageType::Flame | DamageType::ParticleBeam
                    );
                    for (&oid, obj) in objects.iter() {
                        if oid == shooter || Some(oid) == intended {
                            continue;
                        }
                        if launcher_contained_by == Some(oid) {
                            continue;
                        }
                        if skip_burned && obj.has_object_status_bit("BURNED") {
                            continue;
                        }
                        if obj.get_sneaky_targeting_offset(sneak_now).is_some() {
                            continue;
                        }
                        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                            continue;
                        }
                        if obj.status.under_construction {
                            continue;
                        }
                        let same_controller =
                            crate::game_logic::weapon_bootstrap::projectile_structure_same_controller(
                                proj_owner,
                                obj.owner_player_id,
                                obj.team == proj_team,
                            );
                        if !crate::game_logic::weapon_bootstrap::projectile_collides_with_structure(
                            projectile.projectile_collides,
                            same_controller,
                        ) {
                            continue;
                        }
                        // C++ Weapon.cpp:679-699 — only skip KINDOF_FS_AIRFIELD
                        // when the intended victim reserved a parking space.
                        // Other buildings still detonate AA / airborne shots.
                        if obj.is_kind_of(KindOf::FSAirfield)
                            && obj.thing.template.parking_place.is_some()
                        {
                            if let Some(tid) = intended {
                                if let Some(t) = objects.get(&tid) {
                                    if t.producer_id == Some(oid) || t.contained_by == Some(oid) {
                                        continue;
                                    }
                                }
                            }
                        }
                        let radius = obj.selection_radius.max(8.0);
                        // Horizontal (XZ) distance — tall buildings block regardless of Y.
                        let op = obj.get_position();
                        let dx = pos.x - op.x;
                        let dz = pos.z - op.z;
                        if (dx * dx + dz * dz).sqrt() <= radius {
                            hit_structure = Some(oid);
                            break;
                        }
                    }
                }
                if let Some(sid) = hit_structure {
                    if let Some(flight) = projectile.flight.clone() {
                        if let Some(center) =
                            crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                                objects,
                                sid,
                                projectile.shooter_id,
                                &flight,
                            )
                        {
                            let fx_name = flight.garrison_hit_kill_fx().to_string();
                            if !fx_name.is_empty() {
                                self.impact_fx.push(ProjectileImpactFx {
                                    position: center,
                                    shooter_id: projectile.shooter_id,
                                    target_id: Some(sid),
                                    detonation_fx_name: fx_name,
                                    detonation_ocl_name: String::new(),
                                    source_team: projectile.source_team,
                                    source_veterancy: projectile.source_veterancy,
                                    source_orientation: projectile
                                        .velocity
                                        .z
                                        .atan2(projectile.velocity.x),
                                    source_velocity: projectile.velocity,
                                });
                            }
                            projectiles_to_remove.push(proj_id);
                            continue;
                        }
                    }
                    let impact = projectile.position;
                    Self::maybe_record_historic_bonus(projectile, impact, objects);
                    if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                        damage_events.push(Self::splash_area_event(projectile, objects, impact));
                    } else if !projectile.no_damage {
                        damage_events.push(DamageEvent::Direct {
                            target_id: sid,
                            position: impact,
                            damage: projectile.damage,
                            damage_type: projectile.damage_type,
                            death_type: if projectile.die_on_detonate {
                                crate::game_logic::host_usa_pilot::HostDeathType::Detonated
                            } else {
                                projectile.death_type
                            },
                            shooter_id: projectile.shooter_id,
                        });
                    }
                    if !projectile.detonation_fx_name.is_empty()
                        || !projectile.detonation_ocl_name.is_empty()
                    {
                        self.impact_fx.push(ProjectileImpactFx {
                            position: impact,
                            shooter_id: projectile.shooter_id,
                            target_id: Some(sid),
                            detonation_fx_name: projectile.detonation_fx_name.clone(),
                            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                            source_team: projectile.source_team,
                            source_veterancy: projectile.source_veterancy,
                            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                            source_velocity: projectile.velocity,
                        });
                    }
                    projectiles_to_remove.push(proj_id);
                    continue;
                }

                // Check for hits
                if let Some(target_id) = projectile.target_id {
                    if let Some(target) = objects.get(&target_id) {
                        let distance = projectile.position.distance(target.get_position());
                        if distance <= 5.0 {
                            if let Some(flight) = projectile.flight.clone() {
                                if let Some(center) =
                                    crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                                        objects,
                                        target_id,
                                        projectile.shooter_id,
                                        &flight,
                                    )
                                {
                                    let fx_name = flight.garrison_hit_kill_fx().to_string();
                                    if !fx_name.is_empty() {
                                        self.impact_fx.push(ProjectileImpactFx {
                                            position: center,
                                            shooter_id: projectile.shooter_id,
                                            target_id: Some(target_id),
                                            detonation_fx_name: fx_name,
                                            detonation_ocl_name: String::new(),
                                            source_team: projectile.source_team,
                                            source_veterancy: projectile.source_veterancy,
                                            source_orientation: projectile
                                                .velocity
                                                .z
                                                .atan2(projectile.velocity.x),
                                            source_velocity: projectile.velocity,
                                        });
                                    }
                                    projectiles_to_remove.push(proj_id);
                                    continue;
                                }
                            }
                            let impact = projectile.position;
                            Self::maybe_record_historic_bonus(projectile, impact, objects);
                            if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                                // C++ Weapon.cpp:1438: primary inside primaryRadius, else secondary.
                                damage_events
                                    .push(Self::splash_area_event(projectile, objects, impact));
                            } else if !projectile.no_damage {
                                damage_events.push(DamageEvent::Direct {
                                    target_id,
                                    position: impact,
                                    damage: projectile.damage,
                                    damage_type: projectile.damage_type,
                                    death_type: if projectile.die_on_detonate {
                                        crate::game_logic::host_usa_pilot::HostDeathType::Detonated
                                    } else {
                                        projectile.death_type
                                    },
                                    shooter_id: projectile.shooter_id,
                                });
                            }
                            if !projectile.detonation_fx_name.is_empty()
                                || !projectile.detonation_ocl_name.is_empty()
                            {
                                self.impact_fx.push(ProjectileImpactFx {
                                    position: impact,
                                    shooter_id: projectile.shooter_id,
                                    target_id: Some(target_id),
                                    detonation_fx_name: projectile.detonation_fx_name.clone(),
                                    detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                    source_team: projectile.source_team,
                                    source_veterancy: projectile.source_veterancy,
                                    source_orientation: projectile
                                        .velocity
                                        .z
                                        .atan2(projectile.velocity.x),
                                    source_velocity: projectile.velocity,
                                });
                            }
                            projectiles_to_remove.push(proj_id);
                        }
                    }
                } else {
                    // Check ground impact
                    let distance = projectile.position.distance(projectile.target_position);
                    if distance <= 2.0 {
                        let impact = projectile.target_position;
                        Self::maybe_record_historic_bonus(projectile, impact, objects);
                        if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                            damage_events.push(Self::splash_area_event(projectile, objects, impact));
                        }
                        if !projectile.detonation_fx_name.is_empty()
                            || !projectile.detonation_ocl_name.is_empty()
                        {
                            self.impact_fx.push(ProjectileImpactFx {
                                position: impact,
                                shooter_id: projectile.shooter_id,
                                target_id: None,
                                detonation_fx_name: projectile.detonation_fx_name.clone(),
                                detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                source_team: projectile.source_team,
                                source_veterancy: projectile.source_veterancy,
                                source_orientation: projectile
                                    .velocity
                                    .z
                                    .atan2(projectile.velocity.x),
                                source_velocity: projectile.velocity,
                            });
                        }
                        projectiles_to_remove.push(proj_id);
                    }
                }
            }
        }

        // Process damage events
        for hit in &damage_events {
            match hit {
                DamageEvent::Direct {
                    target_id,
                    damage,
                    damage_type,
                    death_type,
                    shooter_id,
                    ..
                } => {
                    // C++ MissileAIUpdate `m_noDamage`: diverted decoy detonations deal no HP.
                    // Report/seek happens at launch + decoy-timer expiry, not impact.
                    crate::game_logic::object::prime_live_damage_context(
                        objects.get(shooter_id),
                        None,
                        *damage_type,
                    );
                    if let Some(target) = objects.get_mut(target_id) {
                        let before = target.health.current;
                        let destroyed = target.take_damage_from_typed_death(
                            *damage,
                            Some(*shooter_id),
                            *damage_type,
                            *death_type,
                        );
                        let hp_lost = (before - target.health.current).max(0.0);
                        self.queue_under_attack_if_dealt(*target_id, *damage_type, hp_lost);
                        if destroyed {
                            log::debug!(
                                "Projectile destroyed object {} (damage: {:.1}, type: {:?})",
                                target_id,
                                damage,
                                damage_type,
                            );
                        }
                    }
                }
                DamageEvent::Area {
                    position,
                    damage,
                    damage_type,
                    death_type,
                    radius,
                    secondary_damage,
                    secondary_radius,
                    shock_wave_amount,
                    shock_wave_radius,
                    shock_wave_taper_off,
                    radius_damage_affects,
                    shooter_owner_player_id,
                    shooter_team_instance_name,
                    shooter_template,
                    shooter_id,
                    primary_victim,
                    ..
                } => {
                    // C++ dealDamageInternal (Weapon.cpp:1438):
                    //   FROM_BOUNDINGSPHERE_3D dist <= primaryRadius → primaryDamage
                    //   else within max(primary, secondary) → secondaryDamage
                    // Primary victim skips RadiusDamageAffects (Weapon.cpp:1316-1375).
                    // No distance falloff of the amount; only ShockWave tapers.
                    let splash_fx_source = objects.get(shooter_id).map(
                        crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source,
                    );
                    let primary_r = *radius;
                    let secondary_r = (*secondary_radius).max(0.0);
                    let dual = secondary_r > primary_r + 1e-3 && *secondary_damage > 0.0;
                    let outer = if dual { secondary_r } else { primary_r };
                    let shock_r = (*shock_wave_radius).max(0.0);
                    let shock_amt = (*shock_wave_amount).max(0.0);
                    let shock_taper = (*shock_wave_taper_off).clamp(0.0, 1.0);
                    let shooter_pos = objects.get(shooter_id).map(|s| s.get_position());
                    let shooter_producer = objects.get(shooter_id).and_then(|s| s.producer_id);
                    for (vid, obj) in objects.iter_mut() {
                        let op = obj.get_position();
                        let dist = splash_from_bounding_sphere_3d(
                            *position,
                            op,
                            victim_splash_sphere_radius(obj),
                        );
                        if dist > outer {
                            continue;
                        }
                        let is_primary = *primary_victim == Some(*vid);
                        let kills_self = (*radius_damage_affects
                            & crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF)
                            != 0
                            && *vid == *shooter_id;
                        if !is_primary && !kills_self {
                            let airborne = obj.is_significantly_above_terrain();
                            let same_tmpl = crate::game_logic::weapon_bootstrap::splash_templates_equivalent(
                                shooter_template,
                                &obj.template_name,
                            );
                            // C++ curVictim->getRelationship(source) (Weapon.cpp:1360).
                            let relationship = match players {
                                Some(map) => crate::game_logic::GameLogic::object_relationship_from_owners(
                                    map,
                                    obj.owner_player_id,
                                    &obj.team_instance_name,
                                    *shooter_owner_player_id,
                                    shooter_team_instance_name,
                                ),
                                None => gamelogic::common::Relationship::Neutral,
                            };
                            let allowed =
                                crate::game_logic::weapon_bootstrap::radius_damage_affects_victim(
                                    *radius_damage_affects,
                                    relationship,
                                    *shooter_id,
                                    *vid,
                                    shooter_producer,
                                    airborne,
                                    same_tmpl,
                                );
                            if !allowed {
                                continue;
                            }
                        }
                        if dist <= outer {
                            let area_damage = if kills_self && !is_primary {
                                HUGE_DAMAGE_AMOUNT
                            } else if dist <= primary_r {
                                *damage
                            } else if secondary_r > 0.0 {
                                *secondary_damage
                            } else {
                                0.0
                            };
                            if area_damage > 0.0 {
                                crate::game_logic::host_transition_damage_fx::set_damage_fx_source(
                                    splash_fx_source.clone(),
                                );
                                let before = obj.health.current;
                                obj.take_damage_from_typed_death(
                                    area_damage,
                                    Some(*shooter_id),
                                    *damage_type,
                                    *death_type,
                                );
                                let hp_lost = (before - obj.health.current).max(0.0);
                                self.queue_under_attack_if_dealt(*vid, *damage_type, hp_lost);
                            }
                        }
                        // C++ Object.cpp:1801-1835: attemptDamage applies PhysicsBehavior
                        // shock / random rotation / setStunned / STUNNED_FLAILING.
                        // Live projectile splash must use apply_shock_wave_impulse, not a
                        // one-frame XZ position nudge.
                        if shock_amt > 0.0 && shock_r > 0.0 {
                            let origin = shooter_pos.unwrap_or(*position);
                            if let Some(force) =
                                crate::game_logic::weapon_bootstrap::compute_shock_wave_force(
                                    origin,
                                    op,
                                    shock_amt,
                                    shock_r,
                                    shock_taper,
                                )
                            {
                                let _ = obj.apply_shock_wave_impulse(force);
                            }
                        }
                    }
                }
            }
        }

        // Remove expired/hit projectiles.  Under coupled GameWorld flight
        // authority, publish an explicit inactive residual here: a later
        // active-only snapshot cannot otherwise tell the shadow that a host
        // KILL_SELF delay (or ordinary impact) has actually completed.
        for proj_id in &projectiles_to_remove {
            if self.projectiles.remove(proj_id).is_some()
                && crate::gameworld_shadow::gameworld_projectile_authority_live()
            {
                crate::game_logic::host_projectile_log::record_retired(proj_id.0);
            }
        }

        projectiles_to_remove
    }

    /// Check if projectile collides with something
    fn check_projectile_collision(
        &self,
        projectile: &Projectile,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<ProjectileHit> {
        // Check target collision
        if let Some(target_id) = projectile.target_id {
            if let Some(target) = objects.get(&target_id) {
                let now = crate::game_logic::host_historic_bonus::logic_frame();
                if target.get_sneaky_targeting_offset(now).is_some() {
                    return None;
                }
                let distance = projectile.position.distance(target.get_position());
                if distance <= 5.0 {
                    return Some(ProjectileHit::Direct {
                        target_id,
                        position: projectile.position,
                        damage: projectile.damage,
                        damage_type: projectile.damage_type,
                        death_type: if projectile.die_on_detonate {
                            crate::game_logic::host_usa_pilot::HostDeathType::Detonated
                        } else {
                            projectile.death_type
                        },
                    });
                }
            }
        }

        // Check ground collision
        let distance = projectile.position.distance(projectile.target_position);
        if distance <= 2.0 && projectile.explosion_radius > 0.0 {
            return Some(ProjectileHit::Area {
                position: projectile.target_position,
                damage: projectile.damage,
                damage_type: projectile.damage_type,
                death_type: if projectile.die_on_detonate {
                    crate::game_logic::host_usa_pilot::HostDeathType::Detonated
                } else {
                    projectile.death_type
                },
                radius: projectile.explosion_radius,
                shooter_id: projectile.shooter_id,
            });
        }

        None
    }

    /// Get all active projectiles
    pub fn get_projectiles(&self) -> &HashMap<ObjectId, Projectile> {
        &self.projectiles
    }

    /// Clear all projectiles
    pub fn clear(&mut self) {
        self.projectiles.clear();
    }
}

fn host_projectile_is_small_missile(projectile: &Projectile) -> bool {
    if projectile.is_small_missile {
        return true;
    }
    if crate::game_logic::host_car_bomb::object_definition_has_kind(
        &projectile.projectile_object_name,
        "SMALL_MISSILE",
    ) {
        return true;
    }
    if crate::game_logic::host_car_bomb::object_definition_has_kind(
        &projectile.projectile_object_name,
        "BALLISTIC_MISSILE",
    ) {
        return false;
    }
    matches!(
        projectile.damage_type,
        DamageType::InfantryMissile
            | DamageType::JetMissiles
            | DamageType::StealthJetMissiles
            | DamageType::SubdualMissile
    ) || matches!(
        projectile.projectile_lifecycle,
        Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
    )
}

/// C++ Weapon.cpp:1144-1155 report at launch; MissileAIUpdate decoy when the timer expires.
fn apply_countermeasure_report_and_decoy(
    projectile: &mut Projectile,
    proj_id: ObjectId,
    objects: &HashMap<ObjectId, Object>,
    reg: &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
    frame: u32,
) {
    use crate::game_logic::host_countermeasures::{
        aircraft_has_countermeasures_upgrade, calculate_countermeasure_to_divert_to,
        report_missile_for_countermeasures_named, victim_locomotor_is_supersonic,
        MISSILE_DECOY_DELAY_FRAMES,
    };

    if !projectile.cm_reported {
        projectile.cm_reported = true;
        if host_projectile_is_small_missile(projectile) {
            if let Some(tid) = projectile.target_id {
                if let Some(target) = objects.get(&tid) {
                    let has_cm =
                        aircraft_has_countermeasures_upgrade(&target.applied_upgrades);
                    let airborne = target.is_kind_of(KindOf::Aircraft)
                        || target.status.airborne_target;
                    let supersonic = victim_locomotor_is_supersonic(
                        target.get_cur_locomotor_set_token(),
                    );
                    if has_cm && airborne && !supersonic {
                        let tmpl = target.template_name.clone();
                        if report_missile_for_countermeasures_named(
                            reg,
                            tid,
                            proj_id,
                            frame,
                            true,
                            Some(tmpl.as_str()),
                        ) {
                            projectile.frames_till_decoyed =
                                frame.saturating_add(MISSILE_DECOY_DELAY_FRAMES);
                        }
                    }
                }
            }
        }
    }

    if projectile.frames_till_decoyed > 0 && projectile.frames_till_decoyed <= frame {
        projectile.frames_till_decoyed = 0;
        projectile.no_damage = true;
        if let Some(victim_id) = projectile.target_id {
            let aircraft_xz = objects.get(&victim_id).map(|o| {
                let p = o.get_position();
                (p.x, p.z)
            });
            if let Some(axz) = aircraft_xz {
                let flare_xz: Vec<(ObjectId, f32, f32)> = objects
                    .iter()
                    .filter(|(_, o)| {
                        o.countermeasure_flare
                            && o.producer_id == Some(victim_id)
                            && o.is_alive()
                    })
                    .map(|(id, o)| {
                        let p = o.get_position();
                        (*id, p.x, p.z)
                    })
                    .collect();
                if let Some(fid) =
                    calculate_countermeasure_to_divert_to(reg, victim_id, axz, &flare_xz)
                {
                    projectile.target_id = Some(fid);
                    projectile.is_homing = true;
                    if let Some(flare) = objects.get(&fid) {
                        projectile.target_position = flare.get_position();
                    }
                }
            }
        }
    }
}

/// C++ `HUGE_DAMAGE_AMOUNT` (Damage.h) for `WEAPON_KILLS_SELF`.
const HUGE_DAMAGE_AMOUNT: f32 = 999_999.0;

/// C++ `DAMAGE_RANGE_CALC_TYPE = FROM_BOUNDINGSPHERE_3D` (Weapon.cpp:70):
/// center-to-center minus victim bounding-sphere radius, clamped at 0.
pub(crate) fn splash_from_bounding_sphere_3d(
    center: Vec3,
    victim_pos: Vec3,
    victim_sphere: f32,
) -> f32 {
    let center_dist = victim_pos.distance(center);
    let sphere = victim_sphere.max(0.0);
    if center_dist <= sphere {
        0.0
    } else {
        center_dist - sphere
    }
}

pub(crate) fn victim_splash_sphere_radius(obj: &Object) -> f32 {
    let geom = &obj.thing.template.geometry_info;
    crate::game_logic::host_battlemaster::leftover_horde_bounding_sphere_radius(
        geom.authored,
        geom.bounding_sphere_radius(),
        obj.selection_radius,
    )
}


#[cfg(test)]
mod tests {
    /// CombatSystem unit tests apply damage without a GameWorld shadow session,
    /// so host HP must mutate directly (opt out of damage authority last-writer).
    fn ensure_unit_test_direct_damage() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");
    }

    use super::*;
    use crate::game_logic::{KindOf, Object, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    fn make_obj(
        name: &str,
        id: ObjectId,
        team: Team,
        pos: Vec3,
        kinds: &[KindOf],
        radius: f32,
    ) -> Object {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.set_health(200.0);
        for k in kinds {
            tmpl.add_kind_of(*k);
        }
        let mut obj = Object::new(tmpl, id, team);
        obj.set_position(pos);
        obj.selection_radius = radius;
        obj
    }

    /// A long coordinate flight keeps lifecycle tests away from ordinary
    /// target/ground collision, so the assertion exercises the parsed Object
    /// behavior which owns the C++ timeout result.
    fn lifecycle_test_pending_projectile(
        projectile_object_name: &str,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> PendingProjectile {
        PendingProjectile {
            shooter_id: ObjectId(9_001),
            shooter_pos: Vec3::ZERO,
            source_context: Some(ProjectileLaunchContext {
                source_team: Team::China,
                source_owner_player_id: None,
                source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
                source_orientation: 0.0,
                source_velocity: Vec3::ZERO,
            }),
            target_id,
            target_pos: Some(target_pos),
            damage: 25.0,
            speed: 1.0,
            splash_radius: 8.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: projectile_object_name.into(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: "FX_AuthoredLifecycleImpact".into(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: 0,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        }
    }

    #[test]
    fn retail_dumb_projectile_expiry_detonates_through_pending_host_path() {
        clear_pending_projectile_queue_for_test();
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "RangerFlashBangGrenade",
            None,
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut combat, &objects);

        let projectile = combat
            .projectiles_snapshot()
            .into_iter()
            .next()
            .expect("parsed DumbProjectile must materialize");
        assert_eq!(
            projectile.projectile_lifecycle,
            Some(
                crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::DumbProjectile {
                    max_lifespan_frames: 300,
                }
            )
        );

        for _ in 0..299 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            assert_eq!(combat.projectile_count(), 1);
            assert!(combat.take_impact_fx().is_empty());
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        assert_eq!(combat.projectile_count(), 0);
        let impacts = combat.take_impact_fx();
        assert_eq!(impacts.len(), 1);
        assert_eq!(impacts[0].detonation_fx_name, "FX_AuthoredLifecycleImpact");
        assert!(
            impacts[0].position.x < 20.0,
            "expiry must detonate at its in-flight pose rather than the distant target"
        );
    }

    #[test]
    fn retail_missile_fuel_detonation_and_target_loss_use_distinct_authored_paths() {
        clear_pending_projectile_queue_for_test();

        // DragonTankFlameProjectile has FuelLifetime=350ms and
        // DetonateOnNoFuel=Yes in retail WeaponObjects.ini. C++ rounds that
        // duration up to 11 logic frames and invokes its detonation weapon.
        let mut fuel_combat = CombatSystem::new();
        let mut fuel_objects = HashMap::new();
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "DragonTankFlameProjectile",
            None,
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut fuel_combat, &fuel_objects);
        for _ in 0..10 {
            let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
            assert_eq!(fuel_combat.projectile_count(), 1);
            assert!(fuel_combat.take_impact_fx().is_empty());
        }
        let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
        assert_eq!(
            fuel_combat.projectile_count(),
            1,
            "C++ MissileAIUpdate keeps the detonated object for KillSelfDelay"
        );
        assert_eq!(fuel_combat.take_impact_fx().len(), 1);
        for _ in 0..2 {
            let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
            assert_eq!(fuel_combat.projectile_count(), 1);
            assert!(fuel_combat.take_impact_fx().is_empty());
        }
        let _ = fuel_combat.update_projectiles(1.0 / 30.0, &mut fuel_objects);
        assert_eq!(fuel_combat.projectile_count(), 0);

        // PatriotMissile follows an object and has DetonateOnNoFuel=No. Its
        // C++ airborneTargetGone transition instead enters KILL_SELF for the
        // parsed three-frame delay, producing neither a guessed explosion nor
        // impact FX when that target vanishes.
        let target = ObjectId(9_002);
        let mut target_loss_combat = CombatSystem::new();
        let mut target_loss_objects = HashMap::new();
        target_loss_objects.insert(
            target,
            make_obj(
                "LifecycleTarget",
                target,
                Team::GLA,
                Vec3::new(10_000.0, 0.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        queue_projectile_direct(lifecycle_test_pending_projectile(
            "PatriotMissile",
            Some(target),
            Vec3::new(10_000.0, 0.0, 0.0),
        ));
        drain_pending_projectiles(&mut target_loss_combat, &target_loss_objects);
        let projectile = target_loss_combat
            .projectiles_snapshot()
            .into_iter()
            .next()
            .expect("parsed MissileAIUpdate must materialize");
        assert!(projectile.is_homing);
        target_loss_objects.remove(&target);
        for _ in 0..3 {
            let _ = target_loss_combat.update_projectiles(1.0 / 30.0, &mut target_loss_objects);
            assert_eq!(target_loss_combat.projectile_count(), 1);
            assert!(target_loss_combat.take_impact_fx().is_empty());
        }
        let _ = target_loss_combat.update_projectiles(1.0 / 30.0, &mut target_loss_objects);
        assert_eq!(target_loss_combat.projectile_count(), 0);
        assert!(target_loss_combat.take_impact_fx().is_empty());
    }

    #[test]
    fn coupled_missile_kill_self_removal_publishes_inactive_shadow_residual() {
        let _authority_guard = crate::gameworld_shadow::authority_env_lock();
        let prior_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
        let prior_projectile = std::env::var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", "1");
        crate::game_logic::host_projectile_log::clear();

        let mut combat = CombatSystem::new();
        let id = combat.fire_projectile(
            Vec3::ZERO,
            Vec3::new(1_000.0, 0.0, 0.0),
            &Weapon::default(),
            ObjectId(17),
            None,
            100.0,
        );
        let projectile = combat.projectile_mut(id).expect("projectile");
        projectile.set_projectile_lifecycle(Some(
            crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile {
                try_to_follow_target: false,
                fuel_lifetime_frames: 0,
                detonate_on_no_fuel: false,
                kill_self_delay_frames: 3,
            },
        ));
        projectile.missile_kill_self_started_frame = Some(0);
        projectile.lifetime = 3.0 / 30.0;

        {
            let _couple = crate::gameworld_shadow::ShadowCoupleGuard::enter();
            let mut objects = HashMap::new();
            let removed =
                combat.update_projectiles_with_countermeasures(0.0, &mut objects, None, 0);
            assert_eq!(removed, vec![id]);
        }
        let events = crate::game_logic::host_projectile_log::drain();
        assert!(
            events
                .iter()
                .any(|event| event.host_id == id.0 && !event.active),
            "actual KILL_SELF completion must retire its GameWorld flight residual: {events:?}"
        );

        match prior_shadow {
            Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", value),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prior_projectile {
            Some(value) => crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", value),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY"),
        }
    }

    #[test]
    fn projectile_hits_intervening_structure() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(1);
        let wall = ObjectId(2);
        let tgt = ObjectId(3);
        objects.insert(
            atk,
            make_obj(
                "PrAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            wall,
            make_obj(
                "PrWall",
                wall,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "PrTgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            ..Weapon::default()
};
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        let wall_hp0 = objects.get(&wall).unwrap().health.current;
        let tgt_hp0 = objects.get(&tgt).unwrap().health.current;
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let wall_hp1 = objects.get(&wall).unwrap().health.current;
        let tgt_hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            wall_hp1 < wall_hp0 - 1.0,
            "intervening structure must take projectile damage (wall {wall_hp0}->{wall_hp1})"
        );
        assert!(
            (tgt_hp1 - tgt_hp0).abs() < 0.01,
            "target behind wall must not be hit (tgt {tgt_hp0}->{tgt_hp1})"
        );
    }

    #[test]
    fn aa_projectile_detonates_on_intervening_building() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(21);
        let factory = ObjectId(22);
        let jet = ObjectId(23);
        objects.insert(
            atk,
            make_obj(
                "AaAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Structure, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            factory,
            make_obj(
                "AaFactory",
                factory,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            jet,
            make_obj(
                "AaJet",
                jet,
                Team::GLA,
                Vec3::new(80.0, 20.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&jet).unwrap().status.airborne_target = true;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            can_target_air: true,
            ..Weapon::default()
        };
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 20.0, 0.0),
            &w,
            atk,
            Some(jet),
            40.0,
        );
        let factory_hp0 = objects.get(&factory).unwrap().health.current;
        let jet_hp0 = objects.get(&jet).unwrap().health.current;
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory_hp1 = objects.get(&factory).unwrap().health.current;
        let jet_hp1 = objects.get(&jet).unwrap().health.current;
        assert!(
            factory_hp1 < factory_hp0 - 1.0,
            "AA shot must detonate on intervening building ({factory_hp0}->{factory_hp1})"
        );
        assert!(
            (jet_hp1 - jet_hp0).abs() < 0.01,
            "jet behind factory must not be hit ({jet_hp0}->{jet_hp1})"
        );
    }

    #[test]
    fn projectile_structure_intercept_cpp_surface() {
        let src = include_str!("combat.rs");
        assert!(
            src.contains("Intervening structure residual")
                && src.contains("KindOf::Structure")
                && src.contains("CONTROLLED_STRUCTURES"),
            "update_projectiles must gate own structures on CONTROLLED_STRUCTURES"
        );
    }

    #[test]
    fn projectile_skips_own_structure_unless_controlled_bit() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(91);
        let factory = ObjectId(92);
        let tgt = ObjectId(93);
        objects.insert(
            atk,
            make_obj(
                "OwnAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            factory,
            make_obj(
                "OwnFactory",
                factory,
                Team::USA,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "OwnTgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&atk).unwrap().owner_player_id = Some(1);
        objects.get_mut(&factory).unwrap().owner_player_id = Some(1);
        objects.get_mut(&tgt).unwrap().owner_player_id = Some(2);

        let factory0 = objects.get(&factory).unwrap().health.current;
        let tgt0 = objects.get(&tgt).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 40.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides =
                crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT;
            p.source_owner_player_id = Some(1);
            p.source_team = Team::USA;
        }
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory1 = objects.get(&factory).unwrap().health.current;
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        assert_eq!(
            factory1, factory0,
            "own factory must not intercept without CONTROLLED_STRUCTURES"
        );
        assert!(
            tgt1 < tgt0 - 1.0,
            "shot must pass through own factory to the target"
        );

        objects.get_mut(&factory).unwrap().health.current = factory0;
        objects.get_mut(&tgt).unwrap().health.current = tgt0;
        let pid = combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            40.0,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides =
                crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES;
            p.source_owner_player_id = Some(1);
            p.source_team = Team::USA;
        }
        for _ in 0..120 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let factory2 = objects.get(&factory).unwrap().health.current;
        let tgt2 = objects.get(&tgt).unwrap().health.current;
        assert!(
            factory2 < factory0 - 1.0,
            "CONTROLLED_STRUCTURES must intercept own factory"
        );
        assert!(
            (tgt2 - tgt0).abs() < 0.01,
            "target behind own factory must not be hit when collide-controlled"
        );
    }

    #[test]
    fn projectile_reaches_target_without_wall() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(10);
        let tgt = ObjectId(11);
        objects.insert(
            atk,
            make_obj(
                "PrAtk2",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "PrTgt2",
                tgt,
                Team::GLA,
                Vec3::new(30.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 200.0,
            ..Weapon::default()
};
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(30.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            200.0,
        );
        let tgt_hp0 = objects.get(&tgt).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt_hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            tgt_hp1 < tgt_hp0 - 1.0,
            "open-field projectile must still hit target ({tgt_hp0}->{tgt_hp1})"
        );
    }

    #[test]
    fn projectile_splash_damages_nearby() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(20);
        let tgt = ObjectId(21);
        let near = ObjectId(22);
        objects.insert(
            atk,
            make_obj(
                "SpAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "SpTgt",
                tgt,
                Team::GLA,
                Vec3::new(20.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            near,
            make_obj(
                "SpNear",
                near,
                Team::GLA,
                Vec3::new(25.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            range: 200.0,
            projectile_speed: 500.0,
            splash_radius: 15.0,
            ..Weapon::default()
};
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(20.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
        );
        let tgt0 = objects.get(&tgt).unwrap().health.current;
        let near0 = objects.get(&near).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        let near1 = objects.get(&near).unwrap().health.current;
        assert!(tgt1 < tgt0 - 1.0, "splash center must damage target");
        assert!(
            near1 < near0 - 1.0,
            "nearby unit within splash_radius must take area damage ({near0}->{near1})"
        );
    }

    /// C++ Weapon.cpp:1438 dealDamageInternal: amount is primaryDamage inside
    /// primaryRadius and secondaryDamage outside it. No quadratic falloff.
    #[test]
    fn projectile_splash_is_flat_primary_not_quadratic() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let tgt = ObjectId(71);
        let edge = ObjectId(72);
        objects.insert(
            atk,
            make_obj(
                "StepAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "StepTgt",
                tgt,
                Team::GLA,
                Vec3::new(20.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            edge,
            make_obj(
                "StepEdge",
                edge,
                Team::GLA,
                Vec3::new(24.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            range: 200.0,
            projectile_speed: 500.0,
            splash_radius: 10.0,
            ..Weapon::default()
};
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(20.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
        );
        let tgt0 = objects.get(&tgt).unwrap().health.current;
        let edge0 = objects.get(&edge).unwrap().health.current;
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        let edge1 = objects.get(&edge).unwrap().health.current;
        let tgt_lost = tgt0 - tgt1;
        let edge_lost = edge0 - edge1;
        assert!(
            (tgt_lost - 50.0).abs() < 0.1,
            "center of primary ring takes full PrimaryDamage, lost={tgt_lost}"
        );
        assert!(
            (edge_lost - 50.0).abs() < 0.1,
            "edge of primary ring still takes full PrimaryDamage (Weapon.cpp:1438), lost={edge_lost}"
        );
        assert_eq!(
            objects.get(&tgt).unwrap().last_damage_source,
            Some(atk),
            "splash must stamp launcher as last_damage_source"
        );
    }


    #[test]
    fn instant_hit_laser_damages_same_frame() {
        let mut objects = HashMap::new();
        let atk = ObjectId(30);
        let tgt = ObjectId(31);
        objects.insert(
            atk,
            make_obj(
                "LasAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "LasTgt",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            range: 200.0,
            projectile_speed: 0.0, // instant residual
            ..Weapon::default()
};
        combat.fire_projectile(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(50.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            0.0,
        );
        let hp0 = objects.get(&tgt).unwrap().health.current;
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let hp1 = objects.get(&tgt).unwrap().health.current;
        assert!(
            hp1 < hp0 - 1.0,
            "instant laser must damage on first projectile step ({hp0}->{hp1})"
        );
        assert_eq!(
            combat.projectile_count(),
            0,
            "instant projectile should resolve and clear"
        );
    }

    #[test]
    fn homing_projectile_tracks_moving_target() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(40);
        let tgt = ObjectId(41);
        objects.insert(
            atk,
            make_obj(
                "HomAtk",
                atk,
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "HomTgt",
                tgt,
                Team::GLA,
                Vec3::new(30.0, 0.0, 0.0),
                &[KindOf::Aircraft, KindOf::Attackable],
                5.0,
            ),
        );
        objects.get_mut(&tgt).unwrap().status.airborne_target = true;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 30.0,
            range: 200.0,
            projectile_speed: 80.0,
            can_target_air: true,
            can_target_ground: false,
            ..Weapon::default()
};
        // Aim at stale point (origin line); target will drift +Z so ballistic would miss.
        combat.fire_projectile_ex(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(30.0, 0.0, 0.0),
            &w,
            atk,
            Some(tgt),
            80.0,
            true,
        );
        assert!(
            combat
                .get_projectiles()
                .values()
                .next()
                .map(|p| p.is_homing)
                .unwrap_or(false),
            "projectile must be marked homing"
        );
        // Drift target off the initial aim line.
        for step in 0..120 {
            if let Some(o) = objects.get_mut(&tgt) {
                // Move +Z so a non-homing shot at (30,0,0) would miss.
                o.set_position(Vec3::new(30.0, 0.0, (step as f32) * 0.35));
            }
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let hp = objects.get(&tgt).unwrap().health.current;
        assert!(
            hp < 200.0 - 1.0,
            "homing missile must hit target that drifted off aim line (hp={hp})"
        );
    }

    #[test]
    fn instant_and_homing_cpp_surface() {
        let src = include_str!("combat.rs");
        assert!(src.contains("is_instant_speed"));
        assert!(src.contains("fire_projectile_ex"));
        assert!(src.contains("is_homing"));
        assert!(
            src.contains("Instant residual") || src.contains("instant-hit"),
            "must document instant laser residual"
        );
    }

    #[test]
    fn projectile_impact_queues_detonation_fx() {
        let mut combat = CombatSystem::new();
        let mut objects = HashMap::new();
        let shooter = ObjectId(1);
        let target = ObjectId(2);
        let mut t = Object::new_simple(
            target,
            crate::game_logic::ObjectType::Infantry,
            "GLARebel".to_string(),
        );
        t.set_position(Vec3::new(5.0, 0.0, 0.0));
        objects.insert(target, t);

        // Instant residual: same-frame impact (ProjectileDetonationFX at hit).
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &Weapon {
                damage: 10.0,
                range: 100.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                suspend_fx_frame: 0,
                        reloading_clip: false,
            last_bonus_rof: 0.0,
},
            shooter,
            Some(target),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.detonation_fx_name = "FX_GenericTankShellDetonation".into();
            p.detonation_ocl_name = "OCL_FireFieldSmall".into();
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let fx = combat.take_impact_fx();
        assert_eq!(fx.len(), 1, "impact must queue detonation fx");
        assert_eq!(fx[0].detonation_fx_name, "FX_GenericTankShellDetonation");
        assert_eq!(fx[0].detonation_ocl_name, "OCL_FireFieldSmall");
        assert_eq!(fx[0].target_id, Some(target));
    }

    #[test]
    fn pending_projectile_preserves_exhaust_and_frozen_fire_effect_context() {
        let mut combat = CombatSystem::new();
        let objects = HashMap::new();
        queue_projectile(PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: Vec3::ZERO,
            source_context: Some(ProjectileLaunchContext {
                source_team: Team::China,
                source_owner_player_id: None,
                source_veterancy: crate::game_logic::VeterancyLevel::Heroic,
                source_orientation: std::f32::consts::FRAC_PI_2,
                source_velocity: Vec3::new(12.0, 0.0, -4.0),
            }),
            target_id: Some(ObjectId(2)),
            target_pos: Some(Vec3::new(10.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),
            projectile_lifecycle: None,
            fire_fx_name: "FX_HeroicGenericTankGunNoTracer".into(),
            fire_ocl_name: "OCL_FireFieldSmall".into(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: "MissileExhaust".into(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        // Need a dummy target for drain to resolve? target_pos is Some so OK.
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].exhaust_name, "MissileExhaust");
        assert_eq!(snaps[0].source_team, Team::China);
        assert_eq!(
            snaps[0].source_veterancy,
            crate::game_logic::VeterancyLevel::Heroic
        );

        // There is intentionally no source object in `objects`: the queued
        // FireOCL must retain the fire-time transform rather than sampling a
        // deleted/re-owned object during the later combat drain.
        let fire_ocls = combat.take_fire_ocl();
        assert_eq!(fire_ocls.len(), 1);
        assert_eq!(fire_ocls[0].fire_fx_name, "FX_HeroicGenericTankGunNoTracer");
        assert_eq!(fire_ocls[0].fire_ocl_name, "OCL_FireFieldSmall");
        assert_eq!(fire_ocls[0].source_team, Team::China);
        assert_eq!(
            fire_ocls[0].source_veterancy,
            crate::game_logic::VeterancyLevel::Heroic
        );
        assert!((fire_ocls[0].source_orientation - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(fire_ocls[0].source_velocity, Vec3::new(12.0, 0.0, -4.0));
    }

    #[test]
    fn dual_ring_secondary_damage_residual() {
        let mut objects = HashMap::new();
        let atk = ObjectId(40);
        let near = ObjectId(41);
        let far = ObjectId(42);
        objects.insert(
            near,
            Object::new_simple(
                near,
                crate::game_logic::ObjectType::Infantry,
                "GLARebel".into(),
            ),
        );
        objects.insert(
            far,
            Object::new_simple(
                far,
                crate::game_logic::ObjectType::Infantry,
                "GLARebel".into(),
            ),
        );
        objects
            .get_mut(&near)
            .unwrap()
            .set_position(Vec3::new(5.0, 0.0, 0.0));
        objects
            .get_mut(&far)
            .unwrap()
            .set_position(Vec3::new(18.0, 0.0, 0.0));
        // Pin pick-radius so this case tests ring amounts, not hull slack.
        objects.get_mut(&near).unwrap().selection_radius = 0.0;
        objects.get_mut(&far).unwrap().selection_radius = 0.0;
        let near0 = objects.get(&near).unwrap().health.current;
        let far0 = objects.get(&far).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 100.0,
            splash_radius: 10.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(near),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.secondary_damage = 25.0;
            p.secondary_damage_radius = 25.0;
            // Primary ring uses explosion_radius from splash_radius.
            p.explosion_radius = 10.0;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let near1 = objects.get(&near).unwrap().health.current;
        let far1 = objects.get(&far).unwrap().health.current;
        assert!(
            near1 <= near0 - 99.0,
            "inner ring must take primary damage ({near0}->{near1})"
        );
        assert!(
            far1 <= far0 - 24.0 && far1 > far0 - 99.0,
            "outer ring must take secondary only ({far0}->{far1})"
        );
    }

    #[test]
    fn shock_wave_pushes_mobile_units_outward() {
        let mut objects = HashMap::new();
        let atk = ObjectId(50);
        let tgt = ObjectId(51);
        let mut unit = make_obj(
            "GLARebel",
            tgt,
            Team::GLA,
            Vec3::new(5.0, 0.0, 0.0),
            &[KindOf::Infantry, KindOf::Attackable],
            5.0,
        );
        objects.insert(tgt, unit);

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 5.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(tgt),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.shock_wave_amount = 50.0;
            p.shock_wave_radius = 30.0;
            p.shock_wave_taper_off = 0.5;
        }
        let pos0 = objects.get(&tgt).unwrap().get_position();
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let victim = objects.get(&tgt).unwrap();
        assert!(
            victim.is_shock_stunned(),
            "C++ attemptDamage must stun / STUNNED_FLAILING"
        );
        assert!(
            victim.movement.velocity.length() > 0.0,
            "shockwave must toss via apply_shock_wave_impulse"
        );
        assert!(
            (victim.get_position() - pos0).length() < 0.01,
            "detonation frame must not nudge position ({pos0:?} -> {:?})",
            victim.get_position()
        );
    }

    #[test]
    fn radius_damage_affects_skips_allies_by_default() {
        let mut objects = HashMap::new();
        let atk = ObjectId(60);
        let ally = ObjectId(61);
        let enemy = ObjectId(62);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            enemy,
            make_obj(
                "GLARebel",
                enemy,
                Team::GLA,
                Vec3::new(4.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let enemy0 = objects.get(&enemy).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            &w,
            atk,
            Some(enemy),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        let enemy1 = objects.get(&enemy).unwrap().health.current;
        assert_eq!(ally1, ally0, "ENEMIES|NEUTRALS INI must skip allies");
        assert!(enemy1 < enemy0 - 1.0, "enemies must take splash");
    }

    #[test]
    fn radius_damage_affects_cpp_default_hits_allies() {
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let ally = ObjectId(71);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(ally),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::weapon_bootstrap::WEAPON_AFFECTS_DEFAULT;
            p.target_id = None; // splash neighbor, not primary victim
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        assert!(
            ally1 < ally0 - 1.0,
            "C++ default ALLIES|ENEMIES|NEUTRALS must friendly-fire"
        );
    }

    #[test]
    fn radius_damage_affects_not_airborne_skips_significantly_above() {
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(72);
        let ground = ObjectId(73);
        let high = ObjectId(74);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ground,
            make_obj(
                "GLARebel",
                ground,
                Team::GLA,
                Vec3::new(5.0, 0.0, 0.0),
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        let mut flyer = make_obj(
            "GLAQuad",
            high,
            Team::GLA,
            Vec3::new(5.0, 12.0, 0.0),
            &[KindOf::Vehicle, KindOf::Attackable],
            5.0,
        );
        flyer.ground_height = 0.0;
        flyer.selection_radius = 0.0;
        flyer.thing.template.geometry_info.authored = true;
        flyer.thing.template.geometry_info.major_radius = 0.0;
        flyer.thing.template.geometry_info.geom_type =
            crate::game_logic::HostGeometryType::Sphere;
        flyer.thing.template.geometry_info.height = 0.0;
        objects.insert(high, flyer);
        let ground0 = objects.get(&ground).unwrap().health.current;
        let high0 = objects.get(&high).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        };
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(5.0, 0.0, 0.0),
            &w,
            atk,
            Some(ground),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_DOESNT_AFFECT_AIRBORNE;
            p.target_id = None;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ground1 = objects.get(&ground).unwrap().health.current;
        let high1 = objects.get(&high).unwrap().health.current;
        assert!(
            ground1 < ground0 - 1.0,
            "grounded enemy must take splash ({ground0}->{ground1})"
        );
        assert_eq!(
            high1, high0,
            "NOT_AIRBORNE must use isSignificantlyAboveTerrain, not KindOf::Aircraft"
        );
    }

    #[test]
    fn splash_uses_from_bounding_sphere_3d() {
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(80);
        let building = ObjectId(81);
        let mut bldg = make_obj(
            "ChinaWarFactory",
            building,
            Team::GLA,
            Vec3::new(15.0, 0.0, 0.0),
            &[KindOf::Structure, KindOf::Attackable],
            1.0,
        );
        bldg.thing.template.geometry_info.authored = true;
        bldg.thing.template.geometry_info.geom_type =
            crate::game_logic::HostGeometryType::Sphere;
        bldg.thing.template.geometry_info.major_radius = 8.0;
        objects.insert(building, bldg);

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            splash_radius: 10.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(15.0, 0.0, 0.0),
            &w,
            atk,
            Some(building),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 10.0;
        }
        let hp0 = objects.get(&building).unwrap().health.current;
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let hp1 = objects.get(&building).unwrap().health.current;
        assert!(
            hp1 <= hp0 - 49.0,
            "hull inside ring must take splash ({hp0}->{hp1})"
        );
    }

    #[test]
    fn splash_primary_victim_skips_radius_damage_affects() {
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(90);
        let ally = ObjectId(91);
        let bystander = ObjectId(92);
        objects.insert(
            atk,
            make_obj(
                "USA_Ranger",
                atk,
                Team::USA,
                Vec3::ZERO,
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            ally,
            make_obj(
                "USA_Ranger",
                ally,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            bystander,
            make_obj(
                "USA_MissileDefender",
                bystander,
                Team::USA,
                Vec3::new(4.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let ally0 = objects.get(&ally).unwrap().health.current;
        let by0 = objects.get(&bystander).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 40.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(ally),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        let ally1 = objects.get(&ally).unwrap().health.current;
        let by1 = objects.get(&bystander).unwrap().health.current;
        assert!(
            ally1 < ally0 - 1.0,
            "intended ally must take splash ({ally0}->{ally1})"
        );
        assert_eq!(by1, by0, "bystander ally must still be skipped");
    }

    #[test]
    fn splash_kills_self_deals_huge_damage() {
        ensure_unit_test_direct_damage();
        let mut objects = HashMap::new();
        let atk = ObjectId(100);
        let enemy = ObjectId(101);
        objects.insert(
            atk,
            make_obj(
                "GLADemoTruck",
                atk,
                Team::GLA,
                Vec3::ZERO,
                &[KindOf::Vehicle, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            enemy,
            make_obj(
                "USA_Ranger",
                enemy,
                Team::USA,
                Vec3::new(3.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 10.0,
            splash_radius: 20.0,
            projectile_speed: 0.0,
            ..Weapon::default()
};
        let pid = combat.fire_projectile_ex(
            Vec3::ZERO,
            Vec3::new(3.0, 0.0, 0.0),
            &w,
            atk,
            Some(enemy),
            0.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.explosion_radius = 20.0;
            p.radius_damage_affects =
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF;
        }
        let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
        assert!(
            !objects.get(&atk).unwrap().is_alive(),
            "WEAPON_KILLS_SELF must destroy the shooter"
        );
    }

    #[test]
    fn projectile_collides_mask_gates_structure_intercept() {
        ensure_unit_test_direct_damage();

        let mut objects = HashMap::new();
        let atk = ObjectId(70);
        let wall = ObjectId(71);
        let tgt = ObjectId(72);
        objects.insert(
            atk,
            make_obj(
                "Atk",
                atk,
                Team::USA,
                Vec3::new(0.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        objects.insert(
            wall,
            make_obj(
                "Wall",
                wall,
                Team::Neutral,
                Vec3::new(40.0, 0.0, 0.0),
                &[KindOf::Structure],
                20.0,
            ),
        );
        objects.insert(
            tgt,
            make_obj(
                "Tgt",
                tgt,
                Team::GLA,
                Vec3::new(80.0, 5.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let wall0 = objects.get(&wall).unwrap().health.current;
        let tgt0 = objects.get(&tgt).unwrap().health.current;

        let mut combat = CombatSystem::new();
        let w = Weapon {
            damage: 50.0,
            projectile_speed: 500.0,
            ..Weapon::default()
};
        // No structure collide residual (laser-like).
        let pid = combat.fire_projectile_ex(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(80.0, 5.0, 0.0),
            &w,
            atk,
            Some(tgt),
            500.0,
            false,
        );
        if let Some(p) = combat.projectile_mut(pid) {
            p.projectile_collides = 0;
        }
        for _ in 0..60 {
            let _ = combat.update_projectiles(1.0 / 30.0, &mut objects);
            if combat.projectile_count() == 0 {
                break;
            }
        }
        let wall1 = objects.get(&wall).unwrap().health.current;
        let tgt1 = objects.get(&tgt).unwrap().health.current;
        assert_eq!(wall1, wall0, "mask=0 must not intercept structure");
        assert!(
            tgt1 < tgt0 - 1.0,
            "projectile must reach target when collides mask empty"
        );
    }

    #[test]
    fn scatter_radius_offsets_aim_and_clears_target() {
        let mut objects = HashMap::new();
        let atk = ObjectId(80);
        let tgt = ObjectId(81);
        objects.insert(
            tgt,
            make_obj(
                "GLARebel",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 0.0, 0.0),
                &[KindOf::Infantry, KindOf::Attackable],
                5.0,
            ),
        );
        let mut combat = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 200.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 10.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        // Target cleared when scatter applied.
        assert!(snaps[0].target_id.is_none());
        // Aim point moved off exact target.
        let aim = snaps[0].target_position;
        let d = (aim - Vec3::new(50.0, 0.0, 0.0)).length();
        assert!(d > 0.01 && d <= 10.0 + 1e-2, "scatter offset length {d}");
    }

    #[test]
    fn scale_weapon_speed_slows_close_shots() {
        let mut objects = HashMap::new();
        let atk = ObjectId(90);
        let tgt = ObjectId(91);
        // Place target at firebase min range (50).
        objects.insert(
            tgt,
            make_obj(
                "GLATunnelNetwork",
                tgt,
                Team::GLA,
                Vec3::new(50.0, 0.0, 0.0),
                &[KindOf::Structure, KindOf::Attackable],
                20.0,
            ),
        );
        let mut combat = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 375.0,
            min_attack_range: 50.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat, &objects);
        let snaps: Vec<_> = combat.projectiles_snapshot();
        assert_eq!(snaps.len(), 1);
        assert!(
            (snaps[0].speed - 75.0).abs() < 1e-2,
            "close lob speed {}, want ~75",
            snaps[0].speed
        );

        // Far shot at max range → full speed.
        let mut combat2 = CombatSystem::new();
        queue_projectile(PendingProjectile {
            shooter_id: atk,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: None,
            target_pos: Some(Vec3::new(375.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects:
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 375.0,
            min_attack_range: 50.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
        drain_pending_projectiles(&mut combat2, &objects);
        let snaps2: Vec<_> = combat2.projectiles_snapshot();
        assert_eq!(snaps2.len(), 1);
        assert!(
            (snaps2[0].speed - 300.0).abs() < 1e-2,
            "far lob speed {}, want ~300",
            snaps2[0].speed
        );
    }

    #[test]
    fn missile_ai_ignition_fx_plays_on_delay_zero_and_after_delay() {
        use crate::game_logic::weapon_bootstrap::{
            HostMissileFlight, HostMissilePhase, HostProjectileFlight,
        };

        let mut delay0 = Projectile::new(
            ObjectId(100_001),
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            10.0,
            DamageType::Explosive,
            ObjectId(1),
            None,
        );
        delay0.speed = 100.0;
        delay0.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            ignition_delay_frames: 0,
            ignition_fx: "FX_MissileIgnition".into(),
            ..HostMissileFlight::default()
        }));
        delay0.bind_authored_flight(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 100.0);
        assert!(
            !delay0.is_warhead_armed(),
            "C++ delay 0 still starts LAUNCH so IgnitionFX can play"
        );
        assert_eq!(delay0.flight_runtime.missile_phase, HostMissilePhase::Launch);
        let _ = delay0.update(1.0 / 30.0, true);
        assert!(delay0.is_warhead_armed());
        assert_eq!(delay0.flight_runtime.missile_phase, HostMissilePhase::Attack);

        let mut delayed = Projectile::new(
            ObjectId(100_002),
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            10.0,
            DamageType::Explosive,
            ObjectId(1),
            None,
        );
        delayed.speed = 100.0;
        delayed.flight = Some(HostProjectileFlight::Missile(HostMissileFlight {
            ignition_delay_frames: 2,
            ignition_fx: "FX_MissileIgnition".into(),
            ..HostMissileFlight::default()
        }));
        delayed.bind_authored_flight(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), 100.0);
        assert!(!delayed.is_warhead_armed());
        let _ = delayed.update(1.0 / 30.0, true);
        assert!(
            !delayed.is_warhead_armed(),
            "frame 1 must stay in LAUNCH when IgnitionDelay is 2"
        );
        let _ = delayed.update(1.0 / 30.0, true);
        assert!(delayed.is_warhead_armed());
        assert_eq!(delayed.flight_runtime.missile_phase, HostMissilePhase::Attack);

        let src = include_str!("combat.rs");
        let start = src
            .find("fn try_enter_missile_ignition")
            .expect("ignition helper");
        let body = &src[start..start + 1200];
        assert!(
            body.contains("dispatch_fx_list_at_pos(&ignition_fx, self.position)"),
            "live MissileAI ignition must play authored IgnitionFX"
        );
        assert!(body.contains("missile.ignition_fx"));
    }

}
