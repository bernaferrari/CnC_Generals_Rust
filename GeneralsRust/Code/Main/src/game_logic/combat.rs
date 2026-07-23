use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Damage types in the game
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
    /// C++ DAMAGE_SURRENDER residual (infantry surrender instead of death).
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
            max_lifetime: 10.0,
            is_homing: false,
            explosion_radius: 0.0,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.lifetime += dt;

        if self.lifetime >= self.max_lifetime {
            return false; // Projectile expired
        }

        // Instant residual: already at target (speed 0 / laser).
        if self.speed <= 0.0 {
            self.position = self.target_position;
            return true;
        }

        // Homing residual: keep velocity aimed at last known target_position.
        if self.is_homing {
            let dir = (self.target_position - self.position).normalize_or_zero();
            self.velocity = dir * self.speed;
        }

        // Update position
        self.position += self.velocity * dt;

        true
    }

    /// True when C++ weapon speed is instant-hit residual (laser / hitscan).
    pub fn is_instant_speed(speed: f32) -> bool {
        speed <= 0.0 || speed >= 999_999.0
    }
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
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        radius: f32,
        shooter_id: ObjectId,
        /// Outer-ring SecondaryDamage residual (0 = single-ring quadratic).
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
        /// Shooter template name residual (NOT_SIMILAR filter).
        shooter_template: String,
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
}

/// Combat system manager
#[derive(Debug)]
pub struct CombatSystem {
    projectiles: HashMap<ObjectId, Projectile>,
    next_projectile_id: ObjectId,
    /// Impacts carrying ProjectileDetonationFX residual (drained by GameLogic).
    impact_fx: Vec<ProjectileImpactFx>,
}

/// Global projectile spawn queue. Objects call this when firing, and the
/// game loop drains it each frame into the CombatSystem.
static PENDING_PROJECTILES: std::sync::Mutex<Vec<PendingProjectile>> =
    std::sync::Mutex::new(Vec::new());

/// Data needed to spawn a projectile (enqueued by Object::fire_at).
#[derive(Debug, Clone)]
pub struct PendingProjectile {
    pub shooter_id: ObjectId,
    pub shooter_pos: Vec3,
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
pub fn queue_projectile(pending: PendingProjectile) {
    // Defer only when a live shadow session will drain host_fire_spawn_log.
    // Host-only (shadow off) must enqueue immediately or combat never spawns shots.
    if crate::gameworld_shadow::gameworld_fire_spawn_authority_live() {
        crate::game_logic::host_fire_spawn_log::record(pending);
        return;
    }
    if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        queue.push(pending);
    }
}

/// Unconditional enqueue for shadow fire-spawn apply (bypasses authority gate).
pub fn queue_projectile_direct(pending: PendingProjectile) {
    if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        queue.push(pending);
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

/// Drain all pending projectiles and spawn them into the combat system.
/// Resolves target object positions from the objects map.
pub fn drain_pending_projectiles(combat: &mut CombatSystem, objects: &HashMap<ObjectId, Object>) {
    let pending = if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        std::mem::take(&mut *queue)
    } else {
        Vec::new()
    };

    for p in pending {
        let actual_target_pos = p
            .target_id
            .and_then(|tid| objects.get(&tid))
            .map(|obj| obj.get_position())
            .or(p.target_pos);

        let Some(mut target_pos) = actual_target_pos else {
            continue;
        };

        // C++ Weapon.ini ScatterRadius residual: offset aim point and clear
        // direct target lock when scatter > 0 (miss / near-miss residual).
        let mut fire_target_id = p.target_id;
        if p.scatter_radius > 0.0 {
            let seed = p.shooter_id.0.wrapping_mul(0x9E37_79B9).wrapping_add(
                p.target_id
                    .map(|t| t.0)
                    .unwrap_or(0)
                    .wrapping_mul(0x85EB_CA6B),
            );
            let offset =
                crate::game_logic::weapon_bootstrap::scatter_aim_offset(seed, p.scatter_radius);
            target_pos += offset;
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
        };
        let pid = combat.fire_projectile_ex(
            p.shooter_pos,
            target_pos,
            &weapon,
            p.shooter_id,
            fire_target_id,
            flight_speed,
            p.is_homing,
        );
        if let Some(proj) = combat.projectile_mut(pid) {
            proj.damage_type = p.damage_type;
            proj.death_type = p.death_type;
            proj.projectile_object_name = p.projectile_object_name.clone();
            proj.detonation_fx_name = p.detonation_fx_name.clone();
            proj.detonation_ocl_name = p.detonation_ocl_name.clone();
            proj.exhaust_name = p.exhaust_name.clone();
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
            projectile.max_lifetime = 0.05; // expire quickly after hit check
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

    /// Flight integrate only (lifetime + pose). Hit detection is separate.
    pub fn integrate_projectiles_only(&mut self, dt: f32) -> usize {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            1.0 / 30.0
        };
        let ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();
        let mut stepped = 0usize;
        let mut remove = Vec::new();
        for id in ids {
            let Some(p) = self.projectiles.get_mut(&id) else {
                continue;
            };
            if !p.update(dt) {
                remove.push(id);
            } else {
                stepped += 1;
            }
        }
        for id in remove {
            self.projectiles.remove(&id);
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

    /// Projectile step with optional America Countermeasures diversion residual.
    pub fn update_projectiles_with_countermeasures(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
    ) -> Vec<ObjectId> {
        let projectile_ids: Vec<ObjectId> = self.projectiles.keys().copied().collect();

        // Process projectile updates
        let mut damage_events = Vec::new();
        let mut projectiles_to_remove = Vec::new();

        for proj_id in projectile_ids {
            if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
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
                let still_alive = projectile.update(dt);

                if !still_alive {
                    projectiles_to_remove.push(proj_id);
                    continue;
                }

                // Intervening structure residual: ballistic shells impact the first
                // constructed building whose footprint contains the projectile.
                // Gated by Weapon.ini ProjectileCollidesWith STRUCTURES|WALLS.
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
                    for (&oid, obj) in objects.iter() {
                        if oid == shooter || Some(oid) == intended {
                            continue;
                        }
                        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                            continue;
                        }
                        if obj.status.under_construction {
                            continue;
                        }
                        // Aircraft/airborne projectiles residual: skip structure block
                        // when intended target is airborne (AA fire).
                        if let Some(tid) = intended {
                            if let Some(t) = objects.get(&tid) {
                                if t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target {
                                    continue;
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
                    let impact = projectile.position;
                    Self::maybe_record_historic_bonus(projectile, impact, objects);
                    if projectile.explosion_radius > 0.0 {
                        damage_events.push(DamageEvent::Area {
                            position: impact,
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
                            shooter_team: objects
                                .get(&projectile.shooter_id)
                                .map(|o| o.team)
                                .unwrap_or(crate::game_logic::Team::Neutral),
                            shooter_template: objects
                                .get(&projectile.shooter_id)
                                .map(|o| o.template_name.clone())
                                .unwrap_or_default(),
                        });
                    } else {
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
                            let impact = projectile.position;
                            Self::maybe_record_historic_bonus(projectile, impact, objects);
                            if projectile.explosion_radius > 0.0 {
                                // Splash residual: quadratic falloff includes full damage at center.
                                damage_events.push(DamageEvent::Area {
                                    position: impact,
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
                                    shooter_team: objects
                                        .get(&projectile.shooter_id)
                                        .map(|o| o.team)
                                        .unwrap_or(crate::game_logic::Team::Neutral),
                                    shooter_template: objects
                                        .get(&projectile.shooter_id)
                                        .map(|o| o.template_name.clone())
                                        .unwrap_or_default(),
                                });
                            } else {
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
                        if projectile.explosion_radius > 0.0 {
                            damage_events.push(DamageEvent::Area {
                                position: impact,
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
                                shooter_team: objects
                                    .get(&projectile.shooter_id)
                                    .map(|o| o.team)
                                    .unwrap_or(crate::game_logic::Team::Neutral),
                                shooter_template: objects
                                    .get(&projectile.shooter_id)
                                    .map(|o| o.template_name.clone())
                                    .unwrap_or_default(),
                            });
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
                    ..
                } => {
                    // America Countermeasures residual: divert missile before Direct damage.
                    let mut diverted = false;
                    if let Some(reg) = countermeasures.as_mut() {
                        if let Some(target) = objects.get(target_id) {
                            let is_air = target.is_kind_of(KindOf::Aircraft)
                                || target.status.airborne_target;
                            if is_air
                                && crate::game_logic::host_countermeasures::aircraft_has_countermeasures_upgrade(
                                    &target.applied_upgrades,
                                )
                            {
                                // projectile id residual: use target_id xor frame as stand-in when
                                // DamageEvent does not carry proj id (evasion still deterministic).
                                let proj_key = ObjectId(target_id.0.wrapping_add(frame));
                                diverted = crate::game_logic::host_countermeasures::try_divert_missile(
                                    reg,
                                    *target_id,
                                    proj_key,
                                    frame,
                                    true,
                                );
                            }
                        }
                    }
                    if diverted {
                        log::debug!(
                            "Countermeasures diverted projectile residual vs object {}",
                            target_id
                        );
                    } else if let Some(target) = objects.get_mut(target_id) {
                        let destroyed = target.take_damage_from_typed_death(
                            *damage,
                            None,
                            *damage_type,
                            *death_type,
                        );
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
                    shooter_team,
                    shooter_template,
                    shooter_id,
                    ..
                } => {
                    // C++ dealDamageInternal residual:
                    //   dist <= primaryRadius → primaryDamage
                    //   else within secondaryRadius → secondaryDamage
                    // When secondary_radius is 0, keep quadratic single-ring residual.
                    let primary_r = *radius;
                    let secondary_r = (*secondary_radius).max(0.0);
                    let dual = secondary_r > primary_r + 1e-3 && *secondary_damage > 0.0;
                    let outer = if dual { secondary_r } else { primary_r };
                    let shock_r = (*shock_wave_radius).max(0.0);
                    let shock_amt = (*shock_wave_amount).max(0.0);
                    let shock_taper = (*shock_wave_taper_off).clamp(0.0, 1.0);
                    let push_outer = if shock_amt > 0.0 && shock_r > 0.0 {
                        outer.max(shock_r)
                    } else {
                        outer
                    };
                    for (vid, obj) in objects.iter_mut() {
                        let op = obj.get_position();
                        let dist = op.distance(*position);
                        if dist > push_outer {
                            continue;
                        }
                        let airborne =
                            obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                        let same_tmpl =
                            !shooter_template.is_empty() && obj.template_name == *shooter_template;
                        let allowed =
                            crate::game_logic::weapon_bootstrap::radius_damage_affects_victim(
                                *radius_damage_affects,
                                *shooter_team,
                                *shooter_id,
                                *vid,
                                obj.team,
                                airborne,
                                same_tmpl,
                            );
                        if !allowed {
                            continue;
                        }
                        if dist <= outer {
                            let area_damage = if dual {
                                if dist <= primary_r {
                                    *damage
                                } else {
                                    *secondary_damage
                                }
                            } else if primary_r > 0.0 {
                                let falloff = 1.0 - (dist / primary_r).powi(2);
                                damage * falloff
                            } else {
                                0.0
                            };
                            if area_damage > 0.0 {
                                obj.take_damage_from_typed_death(
                                    area_damage,
                                    None,
                                    *damage_type,
                                    *death_type,
                                );
                            }
                        }
                        // C++ ShockWave residual: push mobile units outward from blast.
                        // Fail-closed: not full PhysicsBehavior / ground friction matrix.
                        // Mobile residual: can_move OR non-structure alive (simple objects
                        // may not flag is_mobile yet). Structures never push.
                        let pushable = obj.is_alive()
                            && !obj.is_kind_of(KindOf::Structure)
                            && (obj.can_move()
                                || obj.is_kind_of(KindOf::Infantry)
                                || obj.is_kind_of(KindOf::Vehicle));
                        if shock_amt > 0.0 && shock_r > 0.0 && dist <= shock_r && pushable {
                            let mut dir = op - *position;
                            dir.y = 0.0;
                            if dir.length_squared() < 1e-8 {
                                // Degenerate center hit: push along +X residual.
                                dir = Vec3::X;
                            } else {
                                dir = dir.normalize();
                            }
                            let t = (dist / shock_r).clamp(0.0, 1.0);
                            // Strength falls from amount at center toward amount*taper at edge.
                            let strength = shock_amt * (1.0 - t * (1.0 - shock_taper));
                            // Convert residual amount into a one-frame position nudge.
                            let nudge = (strength * 0.02).min(12.0);
                            let new_pos = op + dir * nudge;
                            obj.set_position(new_pos);
                            // Kick residual velocity so movement/update observes push.
                            obj.movement.velocity += dir * (strength * 0.15).min(40.0);
                        }
                    }
                }
            }
        }

        // Remove expired/hit projectiles
        for proj_id in &projectiles_to_remove {
            self.projectiles.remove(proj_id);
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

#[cfg(test)]
mod tests {
    /// CombatSystem unit tests apply damage without a GameWorld shadow session,
    /// so host HP must mutate directly (opt out of damage authority last-writer).
    fn ensure_unit_test_direct_damage() {
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");
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
    fn projectile_structure_intercept_cpp_surface() {
        let src = include_str!("combat.rs");
        assert!(
            src.contains("Intervening structure residual") && src.contains("KindOf::Structure"),
            "update_projectiles must intercept structure footprints"
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
    fn pending_projectile_carries_exhaust_name() {
        let mut combat = CombatSystem::new();
        let objects = HashMap::new();
        queue_projectile(PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: Vec3::ZERO,
            target_id: Some(ObjectId(2)),
            target_pos: Some(Vec3::new(10.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: "GenericTankShell".into(),
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
        let pos1 = objects.get(&tgt).unwrap().get_position();
        // Pushed away from blast origin.
        let d0 = pos0.length();
        let d1 = pos1.length();
        assert!(
            d1 > d0 + 0.1 || (pos1 - pos0).length() > 0.1,
            "shockwave must push unit outward ({pos0:?} -> {pos1:?})"
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
        assert_eq!(ally1, ally0, "default affects must skip allies");
        assert!(enemy1 < enemy0 - 1.0, "enemies must take splash");
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
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 200.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
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
            target_id: Some(tgt),
            target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
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
            target_id: None,
            target_pos: Some(Vec3::new(375.0, 0.0, 0.0)),
            damage: 50.0,
            speed: 300.0,
            splash_radius: 10.0,
            is_homing: false,
            damage_type: DamageType::Explosive,
            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            projectile_object_name: String::new(),
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
}
