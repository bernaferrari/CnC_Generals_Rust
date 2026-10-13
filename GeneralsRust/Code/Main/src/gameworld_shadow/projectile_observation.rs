//! Copy the receiving Main CombatSystem's projectiles into its observation.
//! Flight, impacts and retirement are performed by Main before this read.
use super::*;
use gamelogic::world::ProjectileFlightState;

impl GameWorldShadow {
    pub(super) fn sync_projectile_observations(&mut self, logic: &GameLogic) {
        let projectiles = logic.combat_system.get_projectiles();
        let retired: Vec<_> = self
            .world
            .projectiles()
            .keys()
            .copied()
            .filter(|id| !projectiles.contains_key(&ObjectId(*id)))
            .collect();
        for id in retired {
            self.world
                .queue_mutation(WorldMutation::SetProjectileFlight {
                    host_id: id,
                    position: [0.0; 3],
                    velocity: [0.0; 3],
                    target_position: [0.0; 3],
                    damage: 0.0,
                    shooter_host: 0,
                    target_host: 0,
                    speed: 0.0,
                    lifetime: 0.0,
                    max_lifetime: 0.0,
                    is_homing: false,
                    flight_state: ProjectileFlightState::InFlight,
                    active: false,
                });
        }
        for projectile in projectiles.values() {
            self.world
                .queue_mutation(WorldMutation::SetProjectileFlight {
                    host_id: projectile.id.0,
                    position: projectile.position.to_array(),
                    velocity: projectile.velocity.to_array(),
                    target_position: projectile.target_position.to_array(),
                    damage: projectile.damage,
                    shooter_host: projectile.shooter_id.0,
                    target_host: projectile.target_id.map_or(0, |id| id.0),
                    speed: projectile.speed,
                    lifetime: projectile.lifetime,
                    max_lifetime: projectile.max_lifetime,
                    is_homing: projectile.is_homing,
                    flight_state: if projectile.is_missile_kill_self_holding() {
                        ProjectileFlightState::MissileKillSelfHold
                    } else {
                        ProjectileFlightState::InFlight
                    },
                    active: true,
                });
        }
        // Only the private observation is changed. No Main loan is mutable,
        // and this boundary neither integrates a projectile nor resolves damage.
        self.world.apply_pending_mutations();
    }
}
