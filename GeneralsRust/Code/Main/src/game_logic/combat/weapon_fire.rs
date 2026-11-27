// C++ ownership: Weapon.cpp fire acceptance, authored effects, and delayed-shot queues.

pub use generals_sim::combat::DamageEvent;

#[derive(Debug, Clone)]
pub(in crate::game_logic) enum ProjectileDetonationSource {
    DelayedDamage,
    VirtualProjectile(VirtualProjectileImpact),
    Object {
        id: ObjectId,
        generation: u64,
        epoch: u64,
        pre_physics_pose: Option<glam::Mat4>,
        weapon_key: String,
        primary_damage_radius: f32,
    },
}

#[derive(Debug, Clone)]
pub(in crate::game_logic) struct VirtualProjectileImpact {
    pub id: ObjectId,
    pub controller: Option<u32>,
    pub layer: u8,
    pub pose: glam::Mat4,
    pub primary_damage_radius: f32,
    pub team: crate::game_logic::Team,
    pub veterancy: crate::game_logic::VeterancyLevel,
    pub velocity: Vec3,
}

impl VirtualProjectileImpact {
    fn capture(projectile: &Projectile) -> Self {
        let position = projectile.position;
        let pose = projectile.dumb_flight_transform(position).unwrap_or_else(|| {
            projectile.velocity.try_normalize().map_or_else(
                || glam::Mat4::from_translation(position),
                |direction| projectile_forward_transform(position, direction),
            )
        });
        Self {
            id: projectile.id,
            controller: projectile.source_owner_player_id,
            layer: projectile.flight_runtime.layer,
            pose,
            primary_damage_radius: projectile.explosion_radius,
            team: projectile.source_team,
            veterancy: projectile.source_veterancy,
            velocity: projectile.velocity,
        }
    }
}

/// The Weapon::handleProjectileDetonation effects that precede warhead damage.
#[derive(Debug, Clone)]
pub struct ProjectileImpactFx {
    pub position: Vec3,
    pub shooter_id: ObjectId,
    pub(in crate::game_logic) detonation_source: ProjectileDetonationSource,
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

impl ProjectileImpactFx {
    fn weapon_detonation(
        projectile: &Projectile,
        position: Vec3,
        target_id: Option<ObjectId>,
        pre_physics_pose: Option<glam::Mat4>,
    ) -> Option<Self> {
        // The launch rank can leave both names empty while the live actor's
        // detonation rank has an authored list. Deliver its weapon identity.
        let has_authored_source =
            projectile.projectile_actor_birth.is_some() && !projectile.historic_weapon_key.is_empty();
        if !has_authored_source
            && projectile.detonation_fx_name.is_empty()
            && projectile.detonation_ocl_name.is_empty()
        {
            return None;
        }
        let id = projectile.id;
        let detonation_source = match projectile.projectile_actor_birth {
            Some((generation, epoch)) => ProjectileDetonationSource::Object {
                id, generation, epoch, pre_physics_pose,
                weapon_key: projectile.historic_weapon_key.to_ascii_lowercase(),
                primary_damage_radius: projectile.explosion_radius,
            },
            None => ProjectileDetonationSource::VirtualProjectile(
                VirtualProjectileImpact::capture(projectile),
            ),
        };
        Some(Self {
            position,
            shooter_id: projectile.shooter_id,
            detonation_source,
            target_id,
            detonation_fx_name: projectile.detonation_fx_name.clone(),
            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
            source_team: projectile.source_team,
            source_veterancy: projectile.source_veterancy,
            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
            source_velocity: projectile.velocity,
        })
    }
}

/// Ordered receiving Object::onDie callbacks for a projectile with no registered Body.
pub(in crate::game_logic) struct VirtualProjectileDeath {
    pub projectile: VirtualProjectileImpact,
    pub effects: Vec<ProjectileDieEffect>,
}

#[derive(Debug, Clone)]
pub(in crate::game_logic) enum ProjectileDieEffect {
    Fx {
        name: String,
        audio: Option<String>,
        orient_to_object: bool,
    },
    Ocl(String),
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
    pub(crate) fire_sound_loop_events:
        crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    projectiles: HashMap<ObjectId, Projectile>,
    /// Materialized flights awaiting their owner's existing upgrade phase.
    pending_projectile_weapon_bonus_ids: game_engine::common::fast_hash::FxHashSet<ObjectId>,
    shots: generals_sim::combat::ShotQueue,
    next_projectile_id: ObjectId,
    /// Impacts carrying ProjectileDetonationFX residual (drained by GameLogic).
    impact_fx: Vec<ProjectileImpactFx>,
    /// C++ TheRadar->tryUnderAttackEvent victims from this projectile pass.
    pending_under_attack: Vec<ObjectId>,
    /// Victims whose HP hit zero this projectile/splash pass; world tick runs onDie.
    pending_on_die: Vec<ObjectId>,
    /// Parsed fire-time FX/OCL references accepted with queued projectile shots.
    fire_ocl: Vec<WeaponFireOcl>,
    /// Effective GameData retention admitted by this receiving match.
    historic_damage_limit: u32,
    /// Same-template impact history and pending bonuses belong to this match.
    historic_bonus: crate::game_logic::host_historic_bonus::HostHistoricBonus,
}

/// C++ `WeaponTemplate::getProjectileTemplate()==NULL` (empty / `NONE`).
pub(in crate::game_logic) fn is_projectileless_object_name(name: &str) -> bool {
    let n = name.trim();
    n.is_empty() || n.eq_ignore_ascii_case("NONE")
}

fn host_vec_to_leftover_coord(pos: Vec3) -> glam::Vec3 {
    glam::Vec3::new(pos.x, pos.z, pos.y)
}

fn leftover_weapon_is_laser(weapon_name: &str) -> bool {
    let name = weapon_name.trim();
    if name.is_empty() {
        return false;
    }
    if !crate::game_logic::weapon_bootstrap::host_laser_name_for_weapon_name(name).is_empty() {
        return true;
    }
    gamelogic::weapon::with_weapon_store(|store| {
        store
            .find_weapon_template_ci(name)
            .map(|template| template.is_laser())
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

#[cfg(test)]
fn queue_live_projectileless_delayed(
    combat: &mut CombatSystem,
    when: u32,
    pending: PendingProjectile,
    damage_pos: Vec3,
    damage_id: Option<ObjectId>,
) {
    combat.shots.defer(LiveProjectilelessDelayedDamage::new(
        when, pending, damage_pos, damage_id,
    ));
}

/// C++ `Weapon.cpp:998-1075` projectileless fire: lasers and sub-frame travel
/// deal damage now; otherwise leftover `setDelayedDamage` + live apply later.
fn handle_projectileless_pending(
    combat: &mut CombatSystem,
    objects: HostObjectRead<'_>,
    pending: PendingProjectile,
    victim_pos: Vec3,
    aim_pos: Vec3,
    victim_id: Option<ObjectId>,
    rolled_radius: f32,
    frame: u32,
) -> Option<MaterializedWeaponShot> {
    let source_pos = pending.self_damage_position.unwrap_or(pending.shooter_pos);
    let delay_in_frames = pending.damage_delay_frames(victim_pos);
    let laser = pending
        .selected_weapon_is_laser
        .unwrap_or_else(|| leftover_weapon_is_laser(&pending.historic_weapon_key));
    // C++ creates the LaserName beam before Body damage from the resolved
    // shot aim. A near scatter resets projectileDestination to the victim
    // object's actual position only when that object exists; a roll past both
    // authored radii or a coordinate shot keeps the scattered aim.
    if laser {
        let target = if rolled_radius <= pending.splash_radius
            || rolled_radius <= pending.secondary_damage_radius
        {
            victim_id.and_then(|id| objects.lookup(&id))
        } else {
            None
        };
        let beam_target = target.map(|object| object.id);
        let damage_pos = target.map_or(aim_pos, Object::get_position);
        let damage_id = if pending.self_damage_position.is_some() {
            None
        } else {
            beam_target
        };
        return Some(MaterializedWeaponShot::Laser {
            damage: LiveProjectilelessDelayedDamage::new(frame, pending, damage_pos, damage_id),
            target: beam_target,
        });
    }
    // Self-position weapons capture the source position while retaining
    // target travel time; ordinary shots damage at the victim position.
    let damage_id = if pending.self_damage_position.is_some() {
        None
    } else {
        victim_id
    };
    let damage_pos = if pending.self_damage_position.is_some() {
        source_pos
    } else {
        victim_pos
    };
    combat.shots
        .schedule_projectileless(frame, delay_in_frames, pending, damage_pos, damage_id)
        .map(MaterializedWeaponShot::ImmediateDamage)
}

/// Apply leftover delayed-damage entries whose frame has arrived to live objects.
pub fn apply_ready_projectileless_delayed_damage(
    combat: &mut CombatSystem,
    objects: &mut crate::game_logic::HostObjectStore,
    current_frame: u32,
    players: Option<&HashMap<u32, crate::game_logic::Player>>,
    health_events: &mut crate::game_logic::HostHealthEvents,
    ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
) {
    // Weapon.cpp:1589-1604 — dealDamageInternal then erase, continue the same
    // list walk so same-frame callback-appended due entries still fire now.
    let historic_damage_limit = combat.historic_damage_limit();
    drain_ready_projectileless_delayed(combat, current_frame, |combat, shot| {
        combat.apply_projectileless_delayed_shot(
            &shot,
            objects,
            players,
            current_frame,
            historic_damage_limit,
            health_events,
            ownership_events,
        );
    });
}

/// C++ WeaponStore::update delayed-damage walk: process one due entry at a time
/// so a dealDamageInternal callback can append another due entry and have it
/// run before this update returns.
pub(in crate::game_logic) fn drain_ready_projectileless_delayed(
    combat: &mut CombatSystem,
    current_frame: u32,
    mut apply: impl FnMut(&mut CombatSystem, LiveProjectilelessDelayedDamage),
) {
    let mut cursor = 0;
    while let Some(shot) =
        combat.take_due_projectileless_shot(current_frame, &mut cursor)
    {
        apply(combat, shot);
    }
}

/// Test/inspection helper: extract all currently-due entries without applying.
/// Production delivery uses [`drain_ready_projectileless_delayed`] so same-frame
/// appends still run (Weapon.cpp:1589-1604).
#[cfg(test)]
pub(in crate::game_logic) fn take_ready_projectileless_delayed_damage(
    combat: &mut CombatSystem,
    current_frame: u32,
) -> Vec<LiveProjectilelessDelayedDamage> {
    let mut ready = Vec::new();
    drain_ready_projectileless_delayed(combat, current_frame, |_combat, shot| {
        ready.push(shot);
    });
    ready
}

/// Queue a projectile for spawning. Called from Object::fire_at().
pub fn queue_projectile(combat: &mut CombatSystem, pending: PendingProjectile) {
    combat.shots.accept(pending);
}

impl CombatSystem {
    pub(in crate::game_logic) fn take_pending_projectiles(&mut self) -> Vec<PendingProjectile> {
        self.shots.take_accepted()
    }

    pub(in crate::game_logic) fn defer_projectileless_shot(
        &mut self,
        shot: LiveProjectilelessDelayedDamage,
    ) {
        self.shots.defer(shot);
    }

    pub(in crate::game_logic) fn take_due_projectileless_shot(
        &mut self,
        current_frame: u32,
        cursor: &mut usize,
    ) -> Option<LiveProjectilelessDelayedDamage> {
        self.shots.take_due(current_frame, cursor)
    }
}

/// Test helper: length of one owner's pending projectile queue.
#[cfg(test)]
pub fn pending_projectile_queue_len_for_test(combat: &CombatSystem) -> usize {
    combat.shots.accepted().len()
}

/// Test helper: clear one owner's pending projectile queue.
#[cfg(test)]
pub fn clear_pending_projectile_queue_for_test(combat: &mut CombatSystem) {
    combat.shots.clear_accepted();
}

/// Test helper: last queued projectile DamageType (fire_at → take_damage path).
#[cfg(test)]
pub fn last_pending_projectile_identity_for_test(
    combat: &CombatSystem,
) -> Option<(ObjectId, Option<ObjectId>, String)> {
    combat.shots.accepted().last().map(|shot| {
        (
            shot.shooter_id,
            shot.target_id,
            shot.historic_weapon_key.clone(),
        )
    })
}

#[cfg(test)]
pub fn last_pending_projectile_damage_type_for_test(combat: &CombatSystem) -> Option<DamageType> {
    combat.shots.accepted().last().map(|p| p.damage_type)
}

/// Test helper: last queued projectile secondary-ring amount.
#[cfg(test)]
pub fn last_pending_projectile_secondary_damage_for_test(combat: &CombatSystem) -> Option<f32> {
    combat
        .shots.accepted()
        .last()
        .map(|p| p.secondary_damage)
}

/// Test helper: leftover WeaponStore delayed-damage queue length.
#[cfg(test)]
pub fn leftover_delayed_damage_count_for_test() -> usize {
    crate::game_logic::weapon_bootstrap::with_host_weapon_store(|store| {
        store.get_delayed_damage_count()
    })
    .unwrap_or(0)
}

/// Test helper: live projectileless delayed-damage queue length.
#[cfg(test)]
pub fn live_projectileless_delayed_count_for_test(combat: &CombatSystem) -> usize {
    combat.shots.delayed().len()
}

#[cfg(test)]
pub fn last_live_projectileless_delayed_identity_for_test(
    combat: &CombatSystem,
) -> Option<(u32, ObjectId, Option<ObjectId>)> {
    combat.shots.delayed().last().map(|entry| {
        (
            entry.frame(),
            entry.pending.shooter_id,
            entry.pending.target_id,
        )
    })
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
/// Target expiry and projectileless travel time use the driving game's frame.
/// Explicit low-level mapless adapter. Main sessions use the receiving variant below.
#[cfg(test)]
pub fn drain_pending_projectiles(
    combat: &mut CombatSystem,
    objects: HostObjectRead<'_>,
    frame: u32,
    definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
) {
    let mut standalone_next = generals_sim::object::ObjectIds::from_next(combat.next_projectile_id);
    drain_pending_projectiles_with_partition(
        combat,
        objects,
        frame,
        definitions,
        None,
        &mut standalone_next,
        &game_engine::common::random_value::SessionRandom::default(),
    );
    combat.next_projectile_id = standalone_next.next();
}

/// Actual Main admissions borrow the receiving partition for each path calculation.
pub(in crate::game_logic) fn drain_pending_projectiles_with_partition(
    combat: &mut CombatSystem,
    objects: HostObjectRead<'_>,
    frame: u32,
    definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
    partition: Option<&crate::game_logic::partition_manager::PartitionManager>,
    object_ids: &mut generals_sim::object::ObjectIds,
    random: &game_engine::common::random_value::SessionRandom,
) {
    let pending = combat.take_pending_projectiles();

    for p in pending {
        let profile = definitions.get(&p.projectile_object_name);
        let geometry = definitions.geometry(&p.projectile_object_name);
        if let Some(
            MaterializedWeaponShot::ImmediateDamage(immediate)
            | MaterializedWeaponShot::Laser {
                damage: immediate, ..
            },
        ) = materialize_accepted_projectile(
            combat,
            objects,
            frame,
            p,
            None,
            profile,
            geometry,
            partition,
            object_ids,
            ProjectilePlacement::StandaloneFlight,
            random,
        ) {
            combat.defer_projectileless_shot(immediate);
        }
    }
}

pub(in crate::game_logic) enum ProjectilePlacement {
    ReceivingFlight(ObjectId),
    StandaloneFlight,
    AtDestination,
}

pub(in crate::game_logic) enum MaterializedWeaponShot {
    ImmediateDamage(LiveProjectilelessDelayedDamage),
    /// Projectileless authored beam: its named laser object must be born
    /// before the same shot's Body damage runs.
    Laser {
        damage: LiveProjectilelessDelayedDamage,
        target: Option<ObjectId>,
    },
    PlaceProjectile(Vec3),
}

/// Materialize exactly one already accepted shot in its receiving match.
/// This does not read, drain, or rearrange that match's previously queued shots.
/// The ordinary queued phase and synchronous passive-fire route share this body.
/// Return work that requires the owning match: immediate Body damage or a
/// stationary projectile Object. Later delayed shots are installed directly.
pub(in crate::game_logic) fn materialize_accepted_projectile(
    combat: &mut CombatSystem,
    objects: HostObjectRead<'_>,
    frame: u32,
    p: PendingProjectile,
    stream_binding: Option<ProjectileStreamBinding>,
    profile: Option<&crate::game_logic::projectile_definitions::ProjectileProfile>,
    geometry: Option<crate::game_logic::HostGeometryInfo>,
    partition: Option<&crate::game_logic::partition_manager::PartitionManager>,
    object_ids: &mut generals_sim::object::ObjectIds,
    placement: ProjectilePlacement,
    random: &game_engine::common::random_value::SessionRandom,
) -> Option<MaterializedWeaponShot> {
    // Accepted records retain their captured lifecycle. Otherwise resolve only
    // the already-borrowed effective profile of this same receiving session.
    let projectile_lifecycle = p
        .projectile_lifecycle
        .or_else(|| profile.map(|profile| profile.lifecycle));
    // C++ Weapon::fireWeaponTemplate executes FireOCL at the source when
    // the weapon fires, independent of whether a later projectile target
    // can still be resolved.  Freeze the source state at launch because
    // the source may die before the host drains this queue.
    let source_context = p.source_context.or_else(|| {
        objects
            .lookup(&p.shooter_id)
            .map(|source| ProjectileLaunchContext {
                source_team: source.team,
                projectile_team_id: None,
                source_owner_player_id: source.owner_player_id,
                source_veterancy: source.experience.level,
                source_orientation: source.get_orientation(),
                source_velocity: source.movement.velocity,
                launch_transform: None,
                launcher_bonus_mask: source.projectile_launch_bonus_mask(),
            })
    });
    let source_context = source_context.unwrap_or(ProjectileLaunchContext {
        source_team: crate::game_logic::Team::Neutral,
        projectile_team_id: None,
        source_owner_player_id: None,
        source_veterancy: crate::game_logic::VeterancyLevel::Rookie,
        source_orientation: 0.0,
        source_velocity: Vec3::ZERO,
        launch_transform: None,
        launcher_bonus_mask: 0,
    });
    let has_fire_fx =
        !p.fire_fx_name.trim().is_empty() && !p.fire_fx_name.trim().eq_ignore_ascii_case("None");
    let has_fire_ocl =
        !p.fire_ocl_name.trim().is_empty() && !p.fire_ocl_name.trim().eq_ignore_ascii_case("None");
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

    let actual_target_pos = p
        .target_id
        .and_then(|tid| objects.lookup(&tid))
        .map(|obj| {
            if let Some(off) = obj.get_sneaky_targeting_offset(frame) {
                obj.get_position() + off
            } else if obj.template_name.to_ascii_lowercase().contains("bridge") {
                nearer_live_bridge_attack_point(p.shooter_pos, obj)
            } else {
                obj.get_position()
            }
        })
        .or(p.target_pos);

    let Some(mut target_pos) = actual_target_pos else {
        return None;
    };

    let projectileless = is_projectileless_object_name(&p.projectile_object_name);
    let mut victim_pos = target_pos;
    let mut rolled_radius = 0.0;

    // Coordinate-target overrides discard the object; radius scatter only changes aim.
    let mut fire_target_id = p.target_id;
    if let Some(tid) = p.target_id {
        if let Some(obj) = objects.lookup(&tid) {
            if obj.get_sneaky_targeting_offset(frame).is_some() {
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
        if let Some(target) = p.target_id.and_then(|tid| objects.lookup(&tid)) {
            if target.ground_height_from_terrain {
                target_pos.y = target.ground_height;
            }
        }
        // The ScatterTarget pick makes the victim a coordinate before
        // fireWeaponTemplate computes travel and damage.
        victim_pos = target_pos;
        fire_target_id = None;
    } else if p.scatter_radius > 0.0 {
        // C++ fireWeaponTemplate: STRUCTURE aims at geometry center, then
        // each shot rolls GameLogicRandomValueReal radius + angle.
        if let Some(target) = p.target_id.and_then(|tid| objects.lookup(&tid)) {
            if target.is_kind_of(crate::game_logic::KindOf::Structure) {
                target_pos = crate::game_logic::weapon_bootstrap::structure_scatter_aim_origin(
                    target_pos,
                    &(*target.geometry_info()),
                );
            }
        }
        let (radius, offset) =
            crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(p.scatter_radius, random);
        rolled_radius = radius;
        target_pos += offset;
        // C++ snaps Z to the victim pathfind layer so a miss hits dirt,
        // not a far point past the volume. Host Y is up.
        if let Some(target) = p.target_id.and_then(|tid| objects.lookup(&tid)) {
            if target.ground_height_from_terrain {
                target_pos.y = target.ground_height;
            }
        }
        // Weapon.cpp:1127 checks the sampled radius at launch. A zero
        // roll still consumes the angle draw and keeps the projectile's victim.
        // Only a spawned projectile loses its victim to a positive roll;
        // projectileless damage keeps the object for its own victim rules.
        if rolled_radius > 0.0 && !projectileless {
            fire_target_id = None;
        }
    }

    let actor_id = match placement {
        ProjectilePlacement::ReceivingFlight(id) => Some(id),
        ProjectilePlacement::StandaloneFlight => None,
        ProjectilePlacement::AtDestination => {
            // Weapon.cpp:1137 places Objects with no ProjectileUpdateInterface
            // at the destination, where their own Update/Die modules run.
            return Some(MaterializedWeaponShot::PlaceProjectile(target_pos));
        }
    };

    // The firing owner captured any self-damage source position separately
    // from the target used for travel and projectile flight.

    let launch_pos = source_context.launch_transform
        .map_or(p.shooter_pos, |transform| transform.w_axis.truncate());
    let scale_target = if matches!(
        profile.map(|profile| &profile.flight),
        Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(_))
    ) {
        fire_target_id.and_then(|id| objects.lookup(&id))
            .map(dumb_projectile_target_center).unwrap_or(target_pos)
    } else {
        target_pos
    };
    // C++ DumbProjectileBehavior.cpp:348 ScaleWeaponSpeed residual (2D range
    // ratio) runs on the spawned projectile's behavior, never on a
    // projectileless shot's Weapon.cpp speed path.
    let mut flight_speed = p.speed;
    if p.scale_weapon_speed && !projectileless {
        let dx = scale_target.x - launch_pos.x;
        let dz = scale_target.z - launch_pos.z;
        let range_2d = (dx * dx + dz * dz).sqrt();
        let peel = crate::game_logic::weapon_bootstrap::HostWeaponSpeedPeel {
            weapon_speed: p.speed,
            min_weapon_speed: p.min_weapon_speed,
            scale_weapon_speed: true,
            attack_range: p.attack_range,
            min_attack_range: p.min_attack_range,
        };
        flight_speed =
            crate::game_logic::weapon_bootstrap::host_scaled_weapon_speed(&peel, range_2d).max(0.0);
    }

    // C++ fireWeaponTemplate: no ProjectileObject → leftover delayed damage
    // (or same-frame dealDamageInternal). Do not spawn a CombatSystem
    // dummy that can collide mid-flight.
    if projectileless {
        return handle_projectileless_pending(
            combat,
            objects,
            p,
            victim_pos,
            target_pos,
            fire_target_id,
            rolled_radius,
            frame,
        );
    }

    // C++ WeaponSpeed is distance per logic frame. CombatSystem integrates
    // velocity against seconds, so convert only as the projectile is
    // materialized. Already-normalized callers keep their public meaning.
    let flight_speed_per_second = p.speed_unit.distance_per_second(flight_speed);

    let weapon = Weapon {
        damage: p.damage,
        range: 100.0,
        min_range: 0.0,
        reload_time: 1.0,
        last_fire_time: 0.0,
        last_fire_frame: 0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: flight_speed_per_second,
        pre_attack_delay: 0.0,
        splash_radius: p.splash_radius,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
        next_fire_frame: 0,
        reload_started_frame: 0,
        frame_clock_active: false,
        frame_clock_status: Default::default(),
    };
    // The accepted flight and its receiving Object share one identity.
    let id = if let Some(id) = actor_id {
        id
    } else {
        crate::game_logic::world_objects::allocate_live_object_id(object_ids, objects, combat)
    };
    let pid = combat.fire_projectile_ex_with_id(
        id,
        launch_pos,
        target_pos,
        &weapon,
        p.shooter_id,
        fire_target_id,
        flight_speed_per_second,
        // MissileAIUpdate sets m_isTrackingTarget only for an object shot
        // with TryToFollowTarget, not for coordinate or scattered shots.
        // This also selects the full rather than halved lock distance.
        match projectile_lifecycle {
            Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile {
                try_to_follow_target,
                ..
            }) => try_to_follow_target && fire_target_id.is_some(),
            Some(lifecycle) => lifecycle.follows_target(),
            None => p.is_homing,
        },
    );
    let mut flight_valid = true;
    if let Some(proj) = combat.projectile_mut(pid) {
        proj.damage_type = p.damage_type;
        proj.death_type = p.death_type;
        proj.projectile_object_name = p.projectile_object_name.clone();
        proj.collision_geometry = geometry;
        proj.detonation_fx_name = p.detonation_fx_name.clone();
        proj.detonation_ocl_name = p.detonation_ocl_name.clone();
        proj.exhaust_name = p.exhaust_name.clone();
        proj.source_team = source_context.source_team;
        proj.launcher_bonus_mask = source_context.launcher_bonus_mask;
        proj.source_projectile_team_id = source_context.projectile_team_id;
        proj.source_owner_player_id = source_context.source_owner_player_id;
        proj.source_team_instance_name = objects
            .lookup(&p.shooter_id)
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
        proj.damage_status_type = p.damage_status_type;
        proj.stream_binding = stream_binding;
        proj.historic_bonus_time_frames = p.historic_bonus_time_frames;
        proj.historic_bonus_count = p.historic_bonus_count;
        proj.historic_bonus_radius = p.historic_bonus_radius;
        proj.historic_bonus_weapon = p.historic_bonus_weapon.clone();
        proj.die_on_detonate = p.die_on_detonate;
        proj.projectile_collides = p.projectile_collides;
        proj.set_projectile_lifecycle(projectile_lifecycle);
        if let Some(profile) = profile {
            proj.flight = Some(profile.flight.clone());
            if matches!(profile.flight, crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_)) {
                // WeaponSpeed may be zero for a real MissileAI projectile:
                // its locomotor, not WeaponSpeed, supplies flight. The generic
                // instant-hit constructor must not discard its tracked victim.
                proj.is_homing = matches!(
                    projectile_lifecycle,
                    Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile {
                        try_to_follow_target: true, ..
                    })
                ) && fire_target_id.is_some();
            }
        }
        if profile.is_some() {
            let flight_target = if matches!(
                proj.flight,
                Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(_))
            ) {
                fire_target_id
                    .and_then(|id| objects.lookup(&id))
                    .map(dumb_projectile_target_center)
                    .unwrap_or(target_pos)
            } else {
                target_pos
            };
            flight_valid = proj.bind_authored_flight(launch_pos, flight_target, flight_speed_per_second, partition);
        }
        proj.is_small_missile = profile.is_some_and(|profile| profile.is_small_missile);
    }
    if !flight_valid {
        combat.remove_projectile(pid);
    }
    None
}
