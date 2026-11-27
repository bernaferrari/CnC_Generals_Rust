//! Receiving catalog, actor damage, and terrain callbacks for projectile flight.

use crate::game_logic::object::Object;
use crate::game_logic::{KindOf, ObjectId};
use glam::Vec3;
use std::collections::HashMap;

pub use generals_sim::combat::projectile_flight::{
    DumbProjectileFlight as HostDumbProjectileFlight, MissileFlight as HostMissileFlight,
    MissilePhase as HostMissilePhase, ProjectileFlight as HostProjectileFlight,
    ProjectileFlightState as HostProjectileRuntime, adjust_flight_path_end, build_dumb_bezier_path,
    missile_inside_lock_distance, missile_lock_distance,
};
pub(crate) use generals_sim::combat::projectile_flight::{parse_dumb_flight, parse_missile_flight};

const BRIDGE_DECK_FUDGE: f32 = 2.0;
const LAYER_GROUND: u8 = 1;

/// Resolve authored flight/warhead fields from the same Object INI store as
/// `HostProjectileLifecycle`.
pub(in crate::game_logic) fn host_projectile_flight_for_object_name(
    definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
    name: &str,
) -> Option<HostProjectileFlight> {
    definitions
        .get(name.trim())
        .map(|profile| profile.flight.clone())
}

fn kindof_multi(obj: &Object, required: &[KindOf], forbidden: &[KindOf]) -> bool {
    required.iter().all(|kind| obj.is_kind_of(*kind))
        && forbidden.iter().all(|kind| !obj.is_kind_of(*kind))
}

fn is_garrisonable_container(obj: &Object) -> bool {
    if obj.thing().template.garrison_contain_max.is_some() {
        return true;
    }
    obj.building_data
        .as_ref()
        .is_some_and(|building| building.max_garrison > 0)
}

/// C++ GarrisonHitKillCount collide path. Returns the building center when the
/// grenade/missile consumed itself without a normal detonation weapon.
pub fn apply_garrison_hit_kill(
    objects: &mut crate::game_logic::HostObjectStore,
    container_id: ObjectId,
    launcher_id: ObjectId,
    flight: &HostProjectileFlight,
    ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
) -> Option<Vec3> {
    let count = flight.garrison_hit_kill_count();
    if count <= 0 {
        return None;
    }
    let contained = {
        let container = objects.get(&container_id)?;
        if !container.is_alive() || !is_garrisonable_container(container) {
            return None;
        }
        if container.is_immune_to_clear_building_attacks() {
            return None;
        }
        let units = container.contained_units();
        if units.is_empty() {
            return None;
        }
        units
    };

    let required = flight.garrison_required().to_vec();
    let forbidden = flight.garrison_forbidden().to_vec();
    let mut num_killed = 0;
    let mut killed = Vec::new();
    for occupant_id in contained {
        if num_killed >= count {
            break;
        }
        let Some(occupant) = objects.get_mut(&occupant_id) else {
            continue;
        };
        if !occupant.is_alive() || !kindof_multi(occupant, &required, &forbidden) {
            continue;
        }
        let _ = launcher_id;
        occupant.internal_change_health(-occupant.health.current);
        occupant.status.destroyed = true;
        occupant.status.effectively_dead = true;
        killed.push(occupant_id);
        num_killed += 1;
    }
    if num_killed == 0 {
        return None;
    }
    if let Some(container) = objects.get_mut(&container_id) {
        for occupant_id in killed {
            let _ = container.remove_occupant(occupant_id, ownership_events);
        }
        return Some(container.get_position());
    }
    None
}

/// C++ getHighestLayerForDestination + GROUND transition while still over the
/// previous layer's XY. Armed missiles and all dumb projectiles detonate.
pub(crate) fn bridge_deck_detonate_pose(
    terrain: &crate::game_logic::game_logic::FlightTerrainView<'_>,
    pos: Vec3,
    old_layer: u8,
    armed: bool,
) -> (Vec3, u8) {
    let ground = terrain.raw_ground(pos, 0.0);
    let new_layer = terrain.highest_layer(pos, ground, false);
    if !armed || old_layer == LAYER_GROUND || new_layer != LAYER_GROUND {
        return (pos, new_layer);
    }
    let mut test = pos;
    test.y = 9999.0;
    let test_layer = terrain.highest_layer(test, ground, false);
    if test_layer != old_layer {
        return (pos, new_layer);
    }
    let height = terrain
        .grid
        .layer_height_unclipped(test, test_layer, ground)
        + BRIDGE_DECK_FUDGE;
    let mut snapped = pos;
    snapped.y = height;
    (snapped, new_layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missile_phase_retains_legacy_bincode_discriminants() {
        for (phase, encoded) in [
            (HostMissilePhase::Launch, 0_u32),
            (HostMissilePhase::Attack, 1_u32),
            (HostMissilePhase::Kill, 2_u32),
            (HostMissilePhase::Detonated, 3_u32),
        ] {
            let bytes = bincode_legacy::serialize(&phase).unwrap();
            assert_eq!(bytes, encoded.to_le_bytes());
            assert_eq!(bincode_legacy::deserialize::<HostMissilePhase>(&bytes).unwrap(), phase);
        }
    }

    #[test]
    fn bezier_clears_intervening_ridge() {
        let flight = HostDumbProjectileFlight {
            first_height: 50.0,
            second_height: 150.0,
            first_percent_indent: 0.2,
            second_percent_indent: 0.9,
            ..HostDumbProjectileFlight::default()
        };
        let start = Vec3::new(0.0, 10.0, 0.0);
        let end = Vec3::new(100.0, 10.0, 0.0);
        let (path, segs) = build_dumb_bezier_path(start, end, &flight, 250.0, 0, 0.0);
        assert!(segs > 1);
        assert!(path.len() > 1);
        let mid = path[path.len() / 2];
        assert!(
            mid.y > start.y + 40.0,
            "terrain-clearing arc must rise by FirstHeight, got {}",
            mid.y
        );
    }

    #[test]
    fn lock_distance_halves_for_ground_shots() {
        assert!((missile_lock_distance(75.0, true) - 75.0).abs() < f32::EPSILON);
        assert!((missile_lock_distance(75.0, false) - 37.5).abs() < f32::EPSILON);
        assert!(missile_inside_lock_distance(
            Vec3::ZERO,
            Vec3::new(30.0, 0.0, 0.0),
            75.0,
            false
        ));
        assert!(!missile_inside_lock_distance(
            Vec3::ZERO,
            Vec3::new(40.0, 0.0, 0.0),
            75.0,
            false
        ));
    }

    #[test]
    fn flight_path_adjust_clamps_end_point() {
        let flight = HostDumbProjectileFlight {
            flight_path_adjust_per_frame: 5.0,
            first_height: 10.0,
            second_height: 10.0,
            first_percent_indent: 0.5,
            second_percent_indent: 0.5,
            ..HostDumbProjectileFlight::default()
        };
        let mut runtime = HostProjectileRuntime {
            path_start: Vec3::ZERO,
            path_end: Vec3::new(100.0, 0.0, 0.0),
            path_speed_per_frame: 10.0,
            path_segments: 8,
            ..HostProjectileRuntime::default()
        };
        assert!(adjust_flight_path_end(&mut runtime, &flight, Vec3::new(130.0, 0.0, 0.0), |_, _| Some(0.0)));
        assert!((runtime.path_end.x - 105.0).abs() < 0.01);
        assert!(!runtime.path.is_empty());
    }

    #[test]
    fn receiving_fixture_flashbang_and_patriot_flight_fields_parse() {
        match host_projectile_flight_for_object_name(
            &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
            "RangerFlashBangGrenade",
        ) {
            Some(HostProjectileFlight::Dumb(dumb)) => {
                assert!(dumb.first_height >= 0.0);
                assert!(dumb.second_height >= dumb.first_height || dumb.second_height >= 0.0);
            }
            other => panic!("expected DumbProjectile flight, got {other:?}"),
        }
        match host_projectile_flight_for_object_name(
            &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
            "PatriotMissile",
        ) {
            Some(HostProjectileFlight::Missile(missile)) => {
                assert!(missile.lock_distance > 0.0);
                // Parsed even when empty; NONE is filtered to "".
                assert!(!missile.ignition_fx.eq_ignore_ascii_case("none"));
            }
            other => panic!("expected Missile flight, got {other:?}"),
        }
        for name in ["TomahawkMissile", "PatriotMissile", "StingerMissile"] {
            if let Some(HostProjectileFlight::Missile(missile)) =
                host_projectile_flight_for_object_name(
                    &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
                    name,
                )
            {
                assert!(
                    !missile.ignition_fx.eq_ignore_ascii_case("none"),
                    "{name} IgnitionFX must not stay as NONE"
                );
            }
        }
    }
}
