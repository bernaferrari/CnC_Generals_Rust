//! Host GameLogic tests — `crates_and_salvage`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

fn helipad_parking_place() -> crate::game_logic::ParkingPlaceMetadata {
    crate::game_logic::ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    }
}

fn dock_helipad_comanche(logic: &mut GameLogic) -> (ObjectId, ObjectId) {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    use glam::Vec3;

    let mut pad_tmpl = ThingTemplate::new("AmericaHelipad");
    pad_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    pad_tmpl.parking_place = Some(helipad_parking_place());
    logic.templates.insert("AmericaHelipad".into(), pad_tmpl);

    let mut heli_tmpl = ThingTemplate::new("AmericaVehicleComanche");
    heli_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(220.0);
    logic
        .templates
        .insert("AmericaVehicleComanche".into(), heli_tmpl);

    let pad_id = logic
        .create_object("AmericaHelipad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("helipad");
    let heli_id = logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 4.0, 0.0),
        )
        .expect("comanche");
    {
        let heli = logic.objects.get_mut(&heli_id).unwrap();
        heli.set_contained_by(Some(pad_id));
        heli.set_ai_state(AIState::Docked);
        heli.status.airborne_target = false;
        heli.producer_id = Some(pad_id);
        heli.movement.max_speed = 30.0;
    }
    (pad_id, heli_id)
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod physics_combat_and_airfields;
mod retaliation_and_physics;
