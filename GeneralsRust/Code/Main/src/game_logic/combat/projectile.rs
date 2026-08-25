// C++ ownership: Projectile, DumbProjectileBehavior, and MissileAIUpdate flight state.

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
            self.flight =
                crate::game_logic::weapon_bootstrap::host_projectile_flight_for_object_name(
                    &self.projectile_object_name,
                );
        }
    }

    /// Bind parsed C++ flight/warhead and build the DumbProjectile Bezier.
    pub fn bind_authored_flight(&mut self, start: Vec3, end: Vec3, speed: f32) {
        if self.flight.is_none() && !self.projectile_object_name.is_empty() {
            self.flight =
                crate::game_logic::weapon_bootstrap::host_projectile_flight_for_object_name(
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
                if let Some(id) = mgr
                    .create_attached_particle_system_id(Some(self.exhaust_name.as_str()), self.id.0)
                {
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
