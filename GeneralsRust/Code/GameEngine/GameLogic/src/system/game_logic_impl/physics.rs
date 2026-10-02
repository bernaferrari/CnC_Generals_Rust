/// Lightweight physics queue for deferred damage and collisions.
#[derive(Debug, Default)]
pub struct PhysicsWorld {
    pending_damage: Vec<PendingDamage>,
    pending_collisions: Vec<PendingCollision>,
}

#[derive(Debug, Clone)]
struct PendingDamage {
    target_id: ObjectID,
    attacker_id: ObjectID,
    damage_amount: f32,
    damage_type: crate::damage::DamageType,
    death_type: crate::damage::DeathType,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PendingCollision {
    object_a: ObjectID,
    object_b: ObjectID,
    collision_point: (f32, f32, f32),
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_damage(&mut self, target: ObjectID, attacker: ObjectID, amount: f32) {
        self.queue_damage_with_type(
            target,
            attacker,
            amount,
            crate::damage::DamageType::Crush,
            crate::damage::DeathType::Normal,
        );
    }

    pub fn queue_damage_with_type(
        &mut self,
        target: ObjectID,
        attacker: ObjectID,
        amount: f32,
        damage_type: crate::damage::DamageType,
        death_type: crate::damage::DeathType,
    ) {
        self.pending_damage.push(PendingDamage {
            target_id: target,
            attacker_id: attacker,
            damage_amount: amount,
            damage_type,
            death_type,
        });
    }

    pub fn queue_collision(
        &mut self,
        object_a: ObjectID,
        object_b: ObjectID,
        collision_point: (f32, f32, f32),
    ) {
        self.pending_collisions.push(PendingCollision {
            object_a,
            object_b,
            collision_point,
        });
    }

    pub fn resolve_all(&mut self, game_logic: &mut GameLogic) -> Result<(), GameLogicError> {
        // Process pending damage
        for damage in self.pending_damage.drain(..) {
            // Scoped map borrow — no Arc handle clone. The write guard is
            // dropped before destroy_object so the destroy pass can re-take
            // the same object's lock instead of recursing on it.
            let destroyed = game_logic
                .objects
                .get(&damage.target_id)
                .and_then(|obj_ref| obj_ref.write().ok())
                .map(|mut obj| {
                    let mut info = crate::damage::DamageInfo::with_simple(
                        damage.damage_amount,
                        damage.attacker_id,
                        damage.damage_type,
                        damage.death_type,
                    );
                    let _ = obj.attempt_damage(&mut info);
                    obj.is_destroyed()
                })
                .unwrap_or(false);
            if destroyed {
                game_logic.destroy_object(damage.target_id);
            }
        }

        // Process collisions (collision system handles most interactions elsewhere)
        self.pending_collisions.clear();

        Ok(())
    }
}
