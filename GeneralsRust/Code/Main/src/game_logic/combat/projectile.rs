/// Event-time object borrowed only for the synchronous FX invocation.
pub(in crate::game_logic) enum ProjectileFxObject<'a> {
    Projectile(&'a Projectile),
    Object(&'a Object),
}

struct MissileKillTarget {
    origin: Vec3,
    edge_distance_squared: f32,
}

fn projectile_forward_transform(position: Vec3, direction: Vec3) -> glam::Mat4 {
    let basis = glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
    let transform = gamelogic::common::build_transform_matrix(
        basis.transform_point3(position),
        basis.transform_vector3(direction),
    );
    basis * transform * basis
}

pub(in crate::game_logic) trait ProjectilePresentationRuntime {
    fn fx(&mut self, name: &str, object: ProjectileFxObject<'_>);
    fn pose(&mut self, projectile: &Projectile);
    fn attach_exhaust(&mut self, projectile: &Projectile) -> Option<u32>;
}
struct ProjectileFxCallbacks<'a> {
    fx: &'a mut dyn FnMut(&str, ProjectileFxObject<'_>),
    pose: &'a mut dyn FnMut(&Projectile),
}
impl ProjectilePresentationRuntime for ProjectileFxCallbacks<'_> {
    fn fx(&mut self, name: &str, object: ProjectileFxObject<'_>) {
        (self.fx)(name, object);
    }
    fn pose(&mut self, p: &Projectile) {
        (self.pose)(p);
    }
    // Explicit low-level adapter has no native particle manager recipient.
    fn attach_exhaust(&mut self, _: &Projectile) -> Option<u32> {
        None
    }
}
impl ProjectilePresentationRuntime for () {
    fn fx(&mut self, _: &str, _: ProjectileFxObject<'_>) {}
    fn pose(&mut self, _: &Projectile) {}
    fn attach_exhaust(&mut self, _: &Projectile) -> Option<u32> {
        None
    }
}

fn dispatch_projectile_fx_standalone(name: &str, object: ProjectileFxObject<'_>) {
    let (id, position, yaw, owner) = match object {
        ProjectileFxObject::Projectile(p) => (
            p.id,
            p.position,
            p.missile_fx_yaw(),
            p.source_owner_player_id,
        ),
        ProjectileFxObject::Object(o) => (
            o.id,
            o.get_position(),
            o.get_orientation(),
            o.owner_player_id,
        ),
    };
    crate::game_logic::publish_host_fx_object(
        id.0,
        position,
        yaw,
        owner.map(|p| p as i32).unwrap_or(-1),
    );
    crate::game_logic::dispatch_fx_list_at_object(name, id.0, None);
}

// C++ ownership: Projectile, DumbProjectileBehavior, and MissileAIUpdate flight state.

/// C++ Weapon::newProjectileFired binds the exact firing Weapon template.
/// Empty names are authoritative; None on Projectile denotes unbound legacy input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectileStreamBinding {
    pub slot: u8,
    pub name: String,
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
    /// Runtime velocity in world distance per second; pending Weapon.ini
    /// distance-per-frame values are normalized before this object is created.
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
    /// Geometry captured from the receiving projectile template at admission.
    /// Framed separately to preserve the positional projectile snapshot.
    #[serde(skip)]
    pub collision_geometry: Option<crate::game_logic::HostGeometryInfo>,
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
    /// Exact default Team assigned to this projectile at launch. Unlike the
    /// firing object's Team, this is the projectile's own C++ object Team.
    #[serde(skip)]
    pub source_projectile_team_id: Option<u32>,
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
    /// Fired WeaponTemplate DamageStatusType (C++ DamageInfoInput).
    #[serde(skip)]
    pub damage_status_type: Option<gamelogic::common::ObjectStatusTypes>,
    /// Runtime continuation is framed separately by CPRJ v2, preserving v1 bytes.
    #[serde(skip)]
    pub stream_binding: Option<ProjectileStreamBinding>,
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
    /// Live Object-owned THRUST and Physics continuation, framed by CPRJ.
    #[serde(skip)]
    pub(crate) missile_motion: Option<MissileMotion>,
    /// Factory-created projectile Object generation and world epoch.
    #[serde(skip)]
    pub(crate) projectile_actor_birth: Option<(u64, u64)>,
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
    /// Conditions retained by the flight module, and conditions set on the
    /// projectile itself by its authored upgrade modules.
    pub launcher_bonus_mask: u64,
    pub projectile_bonus_mask: u64,
    pub projectile_upgrades_initialized: bool,
    pub computed_bonus_mask: Option<u64>,
}

impl Projectile {
    pub(crate) fn detonate_calls_kill(&self) -> bool {
        self.projectile_lifecycle.is_some_and(
            crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::detonate_calls_kill,
        )
    }

    pub(crate) fn owns_receiving_actor(&self) -> bool {
        self.missile_motion.is_some()
            || self.detonate_calls_kill()
            || self.is_dumb_flight()
    }

    pub(in crate::game_logic) fn is_dumb_flight(&self) -> bool {
        matches!(self.flight, Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(_)))
    }

    /// DumbProjectileBehavior.cpp:611–643 uses the first tangent on the
    /// zeroeth sample and the previous curve sample on subsequent updates.
    pub(in crate::game_logic) fn dumb_flight_transform(&self, position: Vec3) -> Option<glam::Mat4> {
        use crate::game_logic::weapon_bootstrap::HostProjectileFlight;
        let Some(HostProjectileFlight::Dumb(flight)) = &self.flight else {
            return None;
        };
        if !flight.orient_to_flight_path || flight.tumble_randomly {
            return None;
        }
        let index = self.flight_runtime.step.saturating_sub(1).max(1);
        let path = &self.flight_runtime.path;
        let direction = (*path.get(index)? - path[index - 1]).normalize_or_zero();
        (direction != Vec3::ZERO).then(|| projectile_forward_transform(position, direction))
    }

    pub(in crate::game_logic) fn warhead_kills_source(affects: u32) -> bool {
        affects & generals_sim::weapon::WeaponAffectsMask::KILLS_SELF != 0
    }

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
            collision_geometry: None,
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            source_team: crate::game_logic::Team::Neutral,
            source_projectile_team_id: None,
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
            damage_status_type: None,
            stream_binding: None,
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
            flight: None,
            flight_runtime: crate::game_logic::weapon_bootstrap::HostProjectileRuntime::default(),
            missile_motion: None,
            projectile_actor_birth: None,
            frames_till_decoyed: 0,
            no_damage: false,
            cm_reported: false,
            is_small_missile: false,
            leftover_exhaust_id: 0,
            launcher_bonus_mask: 0,
            projectile_bonus_mask: 0,
            projectile_upgrades_initialized: false,
            computed_bonus_mask: None,
        }
    }

    /// Attach resolved Object INI lifecycle; flight binding selects content.
    pub fn set_projectile_lifecycle(
        &mut self,
        lifecycle: Option<crate::game_logic::weapon_bootstrap::HostProjectileLifecycle>,
    ) {
        self.projectile_lifecycle = lifecycle;
        self.missile_kill_self_started_frame = None;
        self.max_lifetime = lifecycle
            .map(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::lifetime_seconds)
            .unwrap_or(0.0);
    }

    /// Bind parsed C++ flight/warhead and build the DumbProjectile Bezier.
    pub(crate) fn bind_authored_flight(
        &mut self,
        start: Vec3,
        end: Vec3,
        speed_per_second: f32,
        partition: Option<&crate::game_logic::partition_manager::PartitionManager>,
    ) -> bool {
        let Some(flight) = self.flight.clone() else {
            return true;
        };
        self.flight_runtime.path_start = start;
        self.flight_runtime.path_end = end;
        self.flight_runtime.original_target_pos = end;
        self.flight_runtime.path_speed_per_frame = if speed_per_second > 0.0 {
            speed_per_second / gamelogic::common::LOGICFRAMES_PER_SECOND as f32
        } else {
            0.0
        };
        match &flight {
            crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb) => {
                let Some(highest) = partition.and_then(|owner| owner.terrain_high_along_line(start, end)) else {
                    return false;
                };
                let (path, segs) = crate::game_logic::weapon_bootstrap::build_dumb_bezier_path(
                    start,
                    end,
                    dumb,
                    speed_per_second,
                    0,
                    highest,
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
        true
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
    fn try_enter_missile_ignition(
        &mut self,
        frame: u32,
        runtime: &mut dyn ProjectilePresentationRuntime,
    ) -> bool {
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
        self.play_missile_ignition(&ignition_fx, runtime);
        self.flight_runtime.missile_armed = true;
        self.flight_runtime.missile_phase =
            crate::game_logic::weapon_bootstrap::HostMissilePhase::Attack;
        true
    }

    fn missile_fx_yaw(&self) -> f32 {
        self.velocity.z.atan2(self.velocity.x)
    }

    fn publish_missile_fx_pose(&self, is_shrouded: bool) {
        crate::game_logic::host_fx_list_dispatch::publish_host_fx_object_with_visibility(
            self.id.0,
            self.position,
            self.missile_fx_yaw(),
            self.source_owner_player_id.map(|p| p as i32).unwrap_or(-1),
            0.0,
            is_shrouded,
        );
    }

    /// C++ `MissileAIUpdate::doIgnitionState` (MissileAIUpdate.cpp:462-466).
    fn play_missile_ignition(
        &mut self,
        ignition_fx: &str,
        runtime: &mut dyn ProjectilePresentationRuntime,
    ) {
        runtime.pose(self);
        if !ignition_fx.is_empty() {
            runtime.fx(ignition_fx, ProjectileFxObject::Projectile(self));
        }
        if !self.exhaust_name.is_empty() && self.leftover_exhaust_id == 0 {
            if let Some(id) = runtime.attach_exhaust(self) {
                self.leftover_exhaust_id = id;
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

    /// Whether an authored MissileAIUpdate has entered C++ `KILL_SELF`.
    ///
    /// This state belongs to the parsed missile behavior and can retain
    /// the projectile long enough for its contrail to finish.
    pub(crate) fn is_missile_kill_self_holding(&self) -> bool {
        matches!(
            self.projectile_lifecycle,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
        ) && self.missile_kill_self_started_frame.is_some()
    }

    /// A detonated missile retains its object and exhaust, but not its drawable.
    pub(crate) fn missile_drawable_hidden(&self) -> bool {
        self.flight_runtime.missile_phase
            == crate::game_logic::weapon_bootstrap::HostMissilePhase::Detonated
    }

    fn elapsed_logic_frames(&self) -> u32 {
        // Recover the elapsed logic frames from the combat clock at 30 Hz.
        (self.lifetime.max(0.0) * 30.0).round() as u32
    }

    /// MissileAIUpdate::detonate enters KILL_SELF after emitting its impact.
    fn retain_detonated_missile(&mut self) -> bool {
        if matches!(
            self.projectile_lifecycle,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
        ) {
            if self.missile_kill_self_started_frame.is_none() {
                self.missile_kill_self_started_frame = Some(self.elapsed_logic_frames());
            }
            self.flight_runtime.missile_phase =
                crate::game_logic::weapon_bootstrap::HostMissilePhase::Detonated;
            true
        } else {
            false
        }
    }

    fn detonation_step(&mut self, pre_physics_pose: Option<glam::Mat4>) -> ProjectileStep {
        ProjectileStep::Detonate {
            retain_actor: self.retain_detonated_missile(),
            pre_physics_pose,
        }
    }

    fn lifecycle_step(&mut self, target_is_live: bool, defer_target_loss: bool) -> ProjectileStep {
        use crate::game_logic::weapon_bootstrap::HostProjectileLifecycle;

        let Some(lifecycle) = self.projectile_lifecycle else {
            return ProjectileStep::Alive;
        };
        let frame = self.elapsed_logic_frames();
        match lifecycle {
            HostProjectileLifecycle::DumbProjectile {
                max_lifespan_frames, ..
            } if frame >= max_lifespan_frames => ProjectileStep::DETONATE,
            HostProjectileLifecycle::Missile {
                try_to_follow_target: _,
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
                    )) => missile.ignition_delay_frames.max(1),
                    _ => 0,
                };
                if fuel_lifetime_frames > 0
                    && frame >= fuel_start.saturating_add(fuel_lifetime_frames)
                {
                    if detonate_on_no_fuel {
                        return self.detonation_step(None);
                    }
                    if self.flight_runtime.missile_phase
                        == crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
                    {
                        self.missile_kill_self_started_frame = Some(frame);
                        return ProjectileStep::Hold;
                    }
                }

                // Tracking follows the current AI goal, including a diverted
                // flare, rather than the original designated victim.
                if self.is_homing && self.target_id.is_some() && !target_is_live && !defer_target_loss {
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

    /// Standalone mapless flight has only the ground layer; it never selects
    /// an ambient terrain/AI runtime. Main supplies its borrowed terrain below.
    pub fn update(&mut self, dt: f32, target_is_live: bool) -> ProjectileStep {
        self.update_with_terrain(dt, target_is_live, None)
    }

    pub(crate) fn update_with_terrain(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) -> ProjectileStep {
        self.update_with_terrain_and_fx(
            dt,
            target_is_live,
            terrain,
            &mut dispatch_projectile_fx_standalone,
        )
    }

    pub(in crate::game_logic) fn update_with_terrain_and_fx(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
    ) -> ProjectileStep {
        self.update_with_terrain_fx_and_pose(dt, target_is_live, terrain, fx, &mut |projectile| {
            projectile.publish_missile_fx_pose(
                crate::game_logic::host_fx_list_dispatch::host_object_is_shrouded_for_local(
                    projectile.id.0,
                ),
            );
        })
    }

    pub(in crate::game_logic) fn update_with_terrain_fx_and_pose(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
        publish: &mut dyn FnMut(&Projectile),
    ) -> ProjectileStep {
        self.update_with_terrain_runtime(
            dt,
            target_is_live,
            None,
            terrain,
            None,
            None,
            None,
            &mut ProjectileFxCallbacks { fx, pose: publish },
        )
    }
    fn completed_missile_kill_move(&self) -> bool {
        self.flight_runtime.missile_phase
            == crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
            && self.missile_motion.as_ref().is_some_and(MissileMotion::is_move_idle)
    }

    fn update_with_terrain_runtime(
        &mut self,
        dt: f32,
        target_is_live: bool,
        kill_target: Option<MissileKillTarget>,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        mut actor: Option<&mut Object>,
        mut partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
        world_frame: Option<u32>,
        runtime: &mut dyn ProjectilePresentationRuntime,
    ) -> ProjectileStep {
        use missile_motion::MissileSteering;

        if let Some(actor) = actor.as_deref() {
            self.position = actor.get_position();
            self.velocity = actor.movement.velocity;
        }
        if dt.is_finite() && dt > 0.0 {
            self.lifetime += dt;
        }

        // C++ removes a missile below the world before dispatching any state.
        if matches!(
            &self.flight,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_))
        ) && self.position.y < 0.0 {
            return ProjectileStep::Remove;
        }

        // C++ runs LAUNCH/IGNITION before airborneTargetGone (doAttackState).
        let frame = self.elapsed_logic_frames();
        let physics_frame = world_frame.unwrap_or(frame);
        let missile_was_armed = self.flight_runtime.missile_armed;
        let ignition_ready = self.try_enter_missile_ignition(frame, runtime);
        if !ignition_ready {
            if let Some(motion) = &mut self.missile_motion {
                let actor = actor.as_deref_mut().expect("missile motion requires its world Object");
                motion.advance(actor, self.target_position, false, false, MissileSteering::NoTurn, physics_frame, terrain, partition.as_deref_mut());
                self.position = actor.get_position();
                self.velocity = actor.movement.velocity;
            }
            return ProjectileStep::Alive;
        }

        let was_kill = self.flight_runtime.missile_phase
            == crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill;
        let completed_move = self.completed_missile_kill_move();
        // doAttackState (MissileAIUpdate.cpp:506-553) checks the retained
        // goal and returns on lock before it handles a vanished target.
        // Plan the transition here so lifecycle can still resolve fuel first.
        let entering_kill = self.missile_motion.is_some()
            && missile_was_armed
            && !was_kill
            && matches!(
                &self.flight,
                Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(rule))
                    if crate::game_logic::weapon_bootstrap::missile_inside_lock_distance(
                        self.position, self.target_position, rule.lock_distance, self.is_homing,
                    )
            );

        // IGNITION arms the missile but does not run ATTACK until the next
        // update. In particular, a missing target cannot skip ignition.
        if missile_was_armed || !matches!(
            &self.flight,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_))
        ) {
            let lifecycle_step = self.lifecycle_step(target_is_live, entering_kill || completed_move);
            if lifecycle_step != ProjectileStep::Alive {
                let detonating = matches!(lifecycle_step, ProjectileStep::Detonate { retain_actor: true, .. });
                if detonating || lifecycle_step == ProjectileStep::Hold {
                    let impact_pose = detonating
                        .then(|| actor.as_deref().map_or_else(
                            || glam::Mat4::from_translation(self.position),
                            Object::get_transform_matrix,
                        ));
                    if let Some(motion) = &mut self.missile_motion {
                        let actor = actor.as_deref_mut().expect("missile motion requires its world Object");
                        let contact = motion.advance_held(actor, self.target_position, physics_frame, terrain, partition.as_deref_mut());
                        self.position = actor.get_position();
                        self.velocity = actor.movement.velocity;
                        if contact && !self.missile_drawable_hidden() {
                            let delta = self.position - self.target_position;
                            let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                            if delta.y <= cell || delta.length_squared() <= (3.0 * cell).powi(2) {
                                return self.detonation_step(None);
                            }
                        }
                    }
                    if let Some(pose) = impact_pose {
                        return ProjectileStep::Detonate { retain_actor: true, pre_physics_pose: Some(pose) };
                    }
                }
                return lifecycle_step;
            }
        }
        if let Some(motion) = &mut self.missile_motion {
            let mut goal = self.target_position;
            if entering_kill {
                self.flight_runtime.missile_phase =
                    crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill;
                if !self.is_homing {
                    motion.restart_move();
                }
            }
            if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(missile)) = &self.flight {
                if !self.is_homing
                    && self.flight_runtime.missile_phase != crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill
                    && missile.lock_distance > 0.0 {
                    goal.y += 10.0;
                }
            }
            let expired = match self.projectile_lifecycle {
                Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { fuel_lifetime_frames, .. }) =>
                    fuel_lifetime_frames > 0 && frame >= fuel_lifetime_frames.saturating_add(
                        match &self.flight {
                            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(m)) => m.ignition_delay_frames.max(1),
                            _ => 0,
                        }),
                _ => false,
            };
            let actor = actor.as_deref_mut().expect("missile motion requires its world Object");
            let mut impact = None;
            if completed_move {
                match kill_target {
                    None => impact = Some(self.position),
                    Some(target) => {
                        let close_enough = actor.movement.max_speed
                            * gamelogic::common::SECONDS_PER_LOGICFRAME_REAL;
                        if target.edge_distance_squared < close_enough * close_enough {
                            // KILL snaps to the actual object origin, not the
                            // height-adjusted MoveTo destination.
                            let mut effects = crate::game_logic::ScriptPresentationEffects::default();
                            if let Some(owner) = partition.as_deref_mut() {
                                actor.set_position_with_partition_owner(target.origin, physics_frame, &mut effects, owner);
                            } else {
                                actor.set_position_with_effects(target.origin, physics_frame, &mut effects);
                            }
                            impact = Some(target.origin);
                        } else {
                            motion.restart_move();
                        }
                    }
                }
            }
            if impact.is_none() {
                motion.update_move(actor, goal);
            }
            let steering = if was_kill {
                MissileSteering::Braking
            } else if entering_kill {
                MissileSteering::LockTransition
            } else if missile_was_armed {
                MissileSteering::Attack
            } else {
                MissileSteering::NoTurn
            };
            let impact_pose = impact.map(|position| {
                let mut pose = actor.get_transform_matrix();
                pose.w_axis = position.extend(1.0);
                pose
            });
            let ground_contact = motion.advance(actor, goal, true, !expired, steering, physics_frame, terrain, partition.as_deref_mut());
            self.position = actor.get_position();
            self.velocity = actor.movement.velocity;
            if let Some(pose) = impact_pose {
                return self.detonation_step(Some(pose));
            }
            if ground_contact {
                // MissileAIUpdate::projectileHandleCollision ignores a
                // ground contact only when above and far from its AI goal.
                let delta = self.position - goal;
                let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                if delta.y <= cell || delta.length_squared() <= (3.0 * cell).powi(2) {
                    return self.detonation_step(None);
                }
            }
            return self.finish_flight_step(terrain);
        }

        if self.speed <= 0.0 {
            self.position = self.target_position;
            return self.finish_flight_step(terrain);
        }

        if matches!(
            &self.flight,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(_))
        ) && !self.flight_runtime.path.is_empty()
        {
            if self.flight_runtime.step >= self.flight_runtime.path.len() {
                return ProjectileStep::DETONATE;
            }
            let next = self.flight_runtime.path[self.flight_runtime.step];
            let tangent = next - self.position;
            if tangent.length_squared() > 0.0001 {
                self.velocity = tangent.normalize() * self.speed;
            }
            self.position = next;
            self.flight_runtime.step += 1;
            return self.finish_flight_step(terrain);
        }

        if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(missile)) =
            &self.flight
        {
            let was_kill = self.flight_runtime.missile_phase
                == crate::game_logic::weapon_bootstrap::HostMissilePhase::Kill;
            // LAUNCH may fall through to IGNITION, but neither IGNITION nor
            // ATTACK dispatches the next state until the following update.
            if missile_was_armed
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
            if was_kill
            {
                let goal = self.target_position;
                let close_enough = (self.speed / 30.0).max(1.0);
                if self.position.distance(goal) <= close_enough {
                    self.position = goal;
                    return self.detonation_step(None);
                }
                let dir = (goal - self.position).normalize_or_zero();
                self.velocity = dir * self.speed;
                self.position += self.velocity * dt;
                return self.finish_flight_step(terrain);
            }
        }

        if self.is_homing {
            let dir = (self.target_position - self.position).normalize_or_zero();
            self.velocity = dir * self.speed;
        }
        self.position += self.velocity * dt;
        self.finish_flight_step(terrain)
    }

    fn finish_flight_step(
        &mut self,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) -> ProjectileStep {
        let armed = self.is_warhead_armed();
        let (snapped, new_layer) = terrain.map_or((self.position, 1), |terrain| {
            crate::game_logic::weapon_bootstrap::bridge_deck_detonate_pose(
                terrain,
                self.position,
                self.flight_runtime.layer,
                armed,
            )
        });
        let old_layer = self.flight_runtime.layer;
        self.flight_runtime.layer = new_layer;
        if armed && old_layer != 1 && new_layer == 1 && snapped != self.position {
            self.position = snapped;
            return self.detonation_step(None);
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
#[derive(Debug, Clone, Copy, PartialEq)]
enum ProjectileStep {
    Alive,
    Hold,
    Detonate {
        /// Missiles enter KILL_SELF for the authored contrail delay.
        retain_actor: bool,
        /// Pose before physics when the same frame advances beyond the impact.
        pre_physics_pose: Option<glam::Mat4>,
    },
    Remove,
}

impl ProjectileStep {
    const DETONATE: Self = Self::Detonate { retain_actor: false, pre_physics_pose: None };
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
