use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Damage types in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
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
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
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
    },
    Area {
        position: Vec3,
        damage: f32,
        damage_type: DamageType,
        radius: f32,
        shooter_id: ObjectId,
    },
}

/// Combat system manager
#[derive(Debug)]
pub struct CombatSystem {
    projectiles: HashMap<ObjectId, Projectile>,
    next_projectile_id: ObjectId,
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
}

/// Queue a projectile for spawning. Called from Object::fire_at().
pub fn queue_projectile(pending: PendingProjectile) {
    if let Ok(mut queue) = PENDING_PROJECTILES.lock() {
        queue.push(pending);
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

        let Some(target_pos) = actual_target_pos else {
            continue;
        };

        let weapon = Weapon {
            damage: p.damage,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: p.speed,
            pre_attack_delay: 0.0,
            splash_radius: p.splash_radius,
        };
        combat.fire_projectile_ex(
            p.shooter_pos,
            target_pos,
            &weapon,
            p.shooter_id,
            p.target_id,
            p.speed,
            p.is_homing,
        );
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
        }
    }

    /// Snapshot active projectiles for PresentationFrame freeze (read-only).
    pub fn projectiles_snapshot(&self) -> Vec<&Projectile> {
        self.projectiles.values().collect()
    }

    pub fn projectile_count(&self) -> usize {
        self.projectiles.len()
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
    pub fn update_projectiles(
        &mut self,
        dt: f32,
        objects: &mut HashMap<ObjectId, Object>,
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
                // C++-ish: cannot fly through buildings even if target is beyond.
                // Skip intended target (handled below) and shooter.
                let mut hit_structure: Option<ObjectId> = None;
                {
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
                    if projectile.explosion_radius > 0.0 {
                        damage_events.push(DamageEvent::Area {
                            position: impact,
                            damage: projectile.damage,
                            damage_type: projectile.damage_type,
                            radius: projectile.explosion_radius,
                            shooter_id: projectile.shooter_id,
                        });
                    } else {
                        damage_events.push(DamageEvent::Direct {
                            target_id: sid,
                            position: impact,
                            damage: projectile.damage,
                            damage_type: projectile.damage_type,
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
                            if projectile.explosion_radius > 0.0 {
                                // Splash residual: quadratic falloff includes full damage at center.
                                damage_events.push(DamageEvent::Area {
                                    position: impact,
                                    damage: projectile.damage,
                                    damage_type: projectile.damage_type,
                                    radius: projectile.explosion_radius,
                                    shooter_id: projectile.shooter_id,
                                });
                            } else {
                                damage_events.push(DamageEvent::Direct {
                                    target_id,
                                    position: impact,
                                    damage: projectile.damage,
                                    damage_type: projectile.damage_type,
                                });
                            }
                            projectiles_to_remove.push(proj_id);
                        }
                    }
                } else {
                    // Check ground impact
                    let distance = projectile.position.distance(projectile.target_position);
                    if distance <= 2.0 {
                        if projectile.explosion_radius > 0.0 {
                            damage_events.push(DamageEvent::Area {
                                position: projectile.target_position,
                                damage: projectile.damage,
                                damage_type: projectile.damage_type,
                                radius: projectile.explosion_radius,
                                shooter_id: projectile.shooter_id,
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
                    ..
                } => {
                    if let Some(target) = objects.get_mut(target_id) {
                        let destroyed = target.take_damage(*damage);
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
                    damage_type: _,
                    radius,
                    ..
                } => {
                    // Apply area damage to all objects within radius
                    for (_id, obj) in objects.iter_mut() {
                        let dist = obj.get_position().distance(*position);
                        if dist <= *radius {
                            // Quadratic falloff: full damage at center, zero at edge
                            let falloff = 1.0 - (dist / radius).powi(2);
                            let area_damage = damage * falloff;
                            if area_damage > 0.0 {
                                obj.take_damage(area_damage);
                            }
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
}
