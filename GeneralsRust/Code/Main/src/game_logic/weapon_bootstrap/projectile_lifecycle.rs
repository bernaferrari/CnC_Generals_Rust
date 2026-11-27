//! Tests of projectile behavior admitted through the live Object catalog.

pub use generals_sim::combat::projectile_lifecycle::ProjectileLifecycle as HostProjectileLifecycle;
pub(crate) use generals_sim::combat::projectile_lifecycle::{
    optional_bool, optional_duration_frames, optional_int, optional_percent, optional_real,
    optional_string, optional_velocity_per_frame, parse_dumb_projectile_lifecycle,
    parse_missile_lifecycle,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiving_fixture_lifecycles_are_parsed_from_object_ini() {
        assert_eq!(
            crate::game_logic::projectile_definitions::fixture_projectile_definitions()
                .get("RangerFlashBangGrenade")
                .map(|profile| profile.lifecycle),
            Some(HostProjectileLifecycle::DumbProjectile {
                max_lifespan_frames: 300,
                detonate_calls_kill: false,
            })
        );
        assert_eq!(
            crate::game_logic::projectile_definitions::fixture_projectile_definitions()
                .get("DragonTankFlameProjectile")
                .map(|profile| profile.lifecycle),
            Some(HostProjectileLifecycle::Missile {
                try_to_follow_target: false,
                fuel_lifetime_frames: 11,
                detonate_on_no_fuel: true,
                kill_self_delay_frames: 3,
            })
        );
        assert_eq!(
            crate::game_logic::projectile_definitions::fixture_projectile_definitions()
                .get("PatriotMissile")
                .map(|profile| profile.lifecycle),
            Some(HostProjectileLifecycle::Missile {
                try_to_follow_target: true,
                fuel_lifetime_frames: 300,
                detonate_on_no_fuel: false,
                kill_self_delay_frames: 3,
            })
        );
    }
}
