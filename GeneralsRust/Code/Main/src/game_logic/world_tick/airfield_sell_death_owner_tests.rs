//! ParkingPlace has one sale notification and its own actual onDie slot.
//! Ordinary ParkingPlace update purges/heals; it does not repeat killAll.
use super::*;

#[test]
fn authored_airfield_update_does_not_retry_a_kill_rejected_when_sold() {
    let (mut world, rack) = owner();
    let id = aircraft(&mut world, INSTANT, 12.0, 12.0);
    assert_eq!(world.reserve_airfield_parking_space(rack, id), Some(0));
    world.host_object_mut(id).unwrap().indestructible = true;
    assert!(world.start_sell_object(rack));
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    assert!(!world.host_object(id).unwrap().status.on_die_started);
    assert!(world.airfield_has_reserved_space(rack, id));
    world.host_object_mut(id).unwrap().indestructible = false;
    for frame in [41, 46, 70] {
        world.frame = frame;
        world.tick_airfield_parking_heal();
        let object = world.host_object(id).unwrap();
        assert_eq!(
            object.health.current, 100.0,
            "frame {frame}: sale is not retried"
        );
        assert!(!object.status.on_die_started);
        assert!(!object.status.destroyed);
        assert!(object.native_die_runtime.completed.is_empty());
        assert!(world.airfield_has_reserved_space(rack, id));
        assert!(queued(&world).is_empty());
        assert_eq!(world.sell_parked_units_killed, 0);
    }
}

#[test]
fn authored_parking_on_die_kills_reserved_aircraft_synchronously_in_slot_order() {
    let (mut world, rack, slots, unreserved) = ordered_roster();
    let result = world
        .apply_owned_kill(
            rack,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        )
        .unwrap();
    assert!(result.destroyed);
    assert!(world.host_object(rack).unwrap().status.on_die_started);
    assert!(!world.host_object(rack).unwrap().status.sold);
    assert_instant_die(&world, slots[0]);
    assert_instant_die(&world, slots[2]);
    assert_eq!(
        queued(&world)
            .into_iter()
            .filter(|id| slots.contains(id))
            .collect::<Vec<_>>(),
        vec![slots[0], slots[2]],
        "parking callbacks finished before the original Body kill returned"
    );
    let slow = world.host_object(slots[1]).unwrap();
    assert_eq!(slow.health.current, 0.0);
    assert!(slow.status.on_die_started);
    assert!(!slow.status.destroyed);
    assert_eq!(slow.slow_death.as_ref().unwrap().begin_frame, 40);
    assert_eq!(slow.slow_death.as_ref().unwrap().destroy_at_frame, 70);
    let completed = slow.native_die_runtime.completed.clone();
    assert!(!completed.is_empty());
    let pending = queued(&world);
    for frame in [41, 46] {
        world.frame = frame;
        world.tick_airfield_parking_heal();
        assert_eq!(
            queued(&world),
            pending,
            "ordinary update never repeats onDie"
        );
        assert_eq!(
            world
                .host_object(slots[1])
                .unwrap()
                .native_die_runtime
                .completed,
            completed
        );
        assert_eq!(
            world
                .host_object(slots[1])
                .unwrap()
                .slow_death
                .as_ref()
                .unwrap()
                .begin_frame,
            40
        );
        let untouched = world.host_object(unreserved).unwrap();
        assert_eq!(untouched.health.current, 100.0);
        assert!(!untouched.status.on_die_started);
    }
}
