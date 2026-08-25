// C++ ownership: Weapon.cpp fire acceptance, authored effects, and delayed-shot queues.

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
        /// Leftover WeaponStore `radius_damage_angle` (C++ PI = full circle).
        radius_damage_angle: f32,
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

/// C++ `WeaponTemplate::getProjectileTemplate()==NULL` (empty / `NONE`).
fn is_projectileless_object_name(name: &str) -> bool {
    let n = name.trim();
    n.is_empty() || n.eq_ignore_ascii_case("NONE")
}

fn host_vec_to_leftover_coord(pos: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

/// Live-host delayed damage for projectileless finite-speed weapons.
/// Leftover `WeaponStore::m_weaponDDI` is the C++ queue; this applies HP on
/// live objects because leftover `dealDamageInternal` looks up leftover
/// GameObjects that the player path does not own.
#[derive(Debug, Clone)]
struct LiveProjectilelessDelayedDamage {
    when: u32,
    pending: PendingProjectile,
    damage_pos: Vec3,
    damage_id: Option<ObjectId>,
}

static LIVE_PROJECTILELESS_DELAYED: std::sync::Mutex<Vec<LiveProjectilelessDelayedDamage>> =
    std::sync::Mutex::new(Vec::new());

fn leftover_weapon_is_laser(weapon_name: &str) -> bool {
    let name = weapon_name.trim();
    if name.is_empty() {
        return false;
    }
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
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

fn leftover_set_delayed_damage(
    weapon_name: &str,
    pos: Vec3,
    when: u32,
    source_id: ObjectId,
    victim_id: ObjectId,
) {
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let key = weapon_name.trim();
    let coord = host_vec_to_leftover_coord(pos);
    let bonus = gamelogic::weapon::WeaponBonus::default();
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        if let Some(template) = store.find_weapon_template_ci(key).cloned() {
            store.set_delayed_damage_from_template(
                &template,
                &coord,
                when,
                source_id.0,
                victim_id.0,
                &bonus,
            );
            return;
        }
        let mut template = gamelogic::weapon::WeaponTemplate::new(if key.is_empty() {
            "Projectileless".to_string()
        } else {
            key.to_string()
        });
        template.projectile_name.clear();
        store.set_delayed_damage_from_template(
            &template,
            &coord,
            when,
            source_id.0,
            victim_id.0,
            &bonus,
        );
    });
}

fn queue_live_projectileless_delayed(
    when: u32,
    pending: PendingProjectile,
    damage_pos: Vec3,
    damage_id: Option<ObjectId>,
) {
    if let Ok(mut queue) = LIVE_PROJECTILELESS_DELAYED.lock() {
        queue.push(LiveProjectilelessDelayedDamage {
            when,
            pending,
            damage_pos,
            damage_id,
        });
    }
}

/// C++ `Weapon.cpp:998-1075` projectileless fire: lasers and sub-frame travel
/// deal damage now; otherwise leftover `setDelayedDamage` + live apply later.
fn handle_projectileless_pending(
    pending: &PendingProjectile,
    damage_pos: Vec3,
    damage_id: Option<ObjectId>,
    flight_speed: f32,
) {
    let delay_in_frames = if flight_speed > 0.0 && !Projectile::is_instant_speed(flight_speed) {
        pending.shooter_pos.distance(damage_pos) / flight_speed
    } else {
        0.0
    };
    let now = crate::game_logic::host_historic_bonus::logic_frame();
    let laser = leftover_weapon_is_laser(&pending.historic_weapon_key);
    if laser || delay_in_frames < 1.0 {
        queue_live_projectileless_delayed(now, pending.clone(), damage_pos, damage_id);
        return;
    }
    let delay_whole_frames = delay_in_frames.ceil() as u32;
    let when = now.saturating_add(delay_whole_frames);
    leftover_set_delayed_damage(
        &pending.historic_weapon_key,
        damage_pos,
        when,
        pending.shooter_id,
        damage_id.unwrap_or(ObjectId(0)),
    );
    queue_live_projectileless_delayed(when, pending.clone(), damage_pos, damage_id);
}

/// Apply leftover delayed-damage entries whose frame has arrived to live objects.
pub fn apply_ready_projectileless_delayed_damage(
    combat: &mut CombatSystem,
    objects: &mut HashMap<ObjectId, Object>,
    current_frame: u32,
    players: Option<&HashMap<u32, crate::game_logic::Player>>,
) {
    let ready = if let Ok(mut queue) = LIVE_PROJECTILELESS_DELAYED.lock() {
        let mut ready = Vec::new();
        let mut i = 0;
        while i < queue.len() {
            if queue[i].when <= current_frame {
                ready.push(queue.remove(i));
            } else {
                i += 1;
            }
        }
        ready
    } else {
        Vec::new()
    };
    for shot in ready {
        combat.apply_projectileless_delayed_shot(&shot, objects, players);
    }
}

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

/// Test helper: leftover WeaponStore delayed-damage queue length.
#[cfg(test)]
pub fn leftover_delayed_damage_count_for_test() -> usize {
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store(|store| store.get_delayed_damage_count()).unwrap_or(0)
}

/// Test helper: live projectileless delayed-damage queue length.
#[cfg(test)]
pub fn live_projectileless_delayed_count_for_test() -> usize {
    LIVE_PROJECTILELESS_DELAYED
        .lock()
        .map(|q| q.len())
        .unwrap_or(0)
}

/// Test helper: clear live projectileless delayed-damage queue.
#[cfg(test)]
pub fn clear_live_projectileless_delayed_for_test() {
    if let Ok(mut q) = LIVE_PROJECTILELESS_DELAYED.lock() {
        q.clear();
    }
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
        let actual_target_pos = p
            .target_id
            .and_then(|tid| objects.get(&tid))
            .map(|obj| {
                if let Some(off) = obj.get_sneaky_targeting_offset(now) {
                    obj.get_position() + off
                } else if obj.template_name.to_ascii_lowercase().contains("bridge") {
                    nearer_live_bridge_attack_point(p.shooter_pos, obj)
                } else {
                    obj.get_position()
                }
            })
            .or(p.target_pos);

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
                    target_pos = crate::game_logic::weapon_bootstrap::structure_scatter_aim_origin(
                        target_pos,
                        &target.thing.template.geometry_info,
                    );
                }
            }
            let offset =
                crate::game_logic::weapon_bootstrap::scatter_aim_offset_logic(p.scatter_radius);
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

        // C++ DamageDealtAtSelfPosition: fire/detonate at source, not victim.
        if crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name(
            &p.historic_weapon_key,
        ) {
            fire_target_id = None;
            target_pos = p.shooter_pos;
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

        // C++ fireWeaponTemplate: no ProjectileObject → leftover delayed damage
        // (or same-frame dealDamageInternal). Do not spawn a CombatSystem
        // dummy that can collide mid-flight.
        if is_projectileless_object_name(&p.projectile_object_name) {
            handle_projectileless_pending(&p, target_pos, fire_target_id, flight_speed);
            continue;
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
            proj.source_owner_player_id = source_context
                .source_owner_player_id
                .or_else(|| objects.get(&p.shooter_id).and_then(|o| o.owner_player_id));
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
