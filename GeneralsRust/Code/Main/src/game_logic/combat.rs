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
        let speed = 200.0; // Units per second

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

        // Update position
        self.position += self.velocity * dt;

        true
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
        };
        combat.fire_projectile(
            p.shooter_pos,
            target_pos,
            &weapon,
            p.shooter_id,
            p.target_id,
            p.speed,
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

        // Use caller-specified speed (from weapon template), fallback to default.
        projectile.speed = if speed > 0.0 { speed } else { 200.0 };
        projectile.is_homing = false; // Some weapons have homing projectiles
        projectile.explosion_radius = 0.0; // Explosive weapons would have this

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
                let still_alive = projectile.update(dt);

                if !still_alive {
                    projectiles_to_remove.push(proj_id);
                    continue;
                }

                // Check for hits
                if let Some(target_id) = projectile.target_id {
                    if let Some(target) = objects.get(&target_id) {
                        let distance = projectile.position.distance(target.get_position());
                        if distance <= 5.0 {
                            damage_events.push(DamageEvent::Direct {
                                target_id,
                                position: projectile.position,
                                damage: projectile.damage,
                                damage_type: projectile.damage_type,
                            });
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
                    damage_type,
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
