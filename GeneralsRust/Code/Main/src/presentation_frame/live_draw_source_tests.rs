use super::*;
use crate::game_logic::{Object, Player, ThingTemplate, WeaponFireStatus};

fn receiving_world(case: u8) -> GameLogic {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object CameraSubject\n Draw = W3DModelDraw ModuleTag_Draw\n DefaultConditionState\n Model = SubjectModel\n End\n ConditionState = DAMAGED\n Model = DamagedModel\n End\n End\nEnd\n", "live_camera_source.ini").unwrap();
    let definition = parser.get_definition("CameraSubject").unwrap();
    let mut template = ThingTemplate::new("CameraSubject");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    template.set_health(100.0);
    template.add_kind_of(KindOf::Structure);
    let mut obj = Object::new(template, ObjectId(42), Team::China);
    match case {
        0 => {}
        1 => {
            obj.status.moving = true;
            obj.status.attacking = true;
            obj.is_panicking = true;
            obj.active_weapon_slot = 2;
            obj.weapon_fire_status = WeaponFireStatus::ReloadingClip;
            obj.continuous_fire_coast_until_frame = 100;
        }
        2 => {
            obj.status.under_construction = true;
            obj.construction_percent = 0.375;
            obj.production_door_phase = 2;
            obj.health.current = 40.0;
        }
        3 => {
            obj.passengers_allowed_to_fire = true;
            obj.stored_resources.supplies = 12;
            obj.combat_cycle_rider = 2;
            let mut building =
                crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
            building.hide_garrisoned_state = true;
            obj.building_data = Some(building);
            assert!(
                obj.add_occupant(ObjectId(43)),
                "admit the actual garrison roster"
            );
        }
        4 => {
            obj.status.destroyed = true;
            obj.health.current = 0.0;
            obj.front_crushed = true;
            obj.back_crushed = true;
            obj.armor_set_second_life = true;
        }
        5 => {
            obj.status.airborne_target = true;
            obj.cell_is_cliff = true;
            obj.cell_is_underwater = true;
            obj.over_water = true;
            obj.weapon_set_player_upgrade = true;
            obj.weapon_crate_upgrade = 2;
            obj.armor_crate_upgrade = 1;
            obj.is_surrendered = true;
        }
        _ => unreachable!(),
    }
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Receiving", true));
    world.add_player(Player::new(1, Team::China, "ApparentController", false));
    world.add_object(obj);
    if case == 3 {
        let mut occupant = Object::new(ThingTemplate::new("Passenger"), ObjectId(43), Team::China);
        occupant.contained_by = Some(ObjectId(42));
        world.add_object(occupant);
    }
    world
}

#[test]
fn live_camera_conditions_match_actual_frozen_draw_route_without_consuming_audio() {
    use crate::game_logic::host_economy_log::HostMoneyAudio;
    for case in 0..6 {
        let mut world = receiving_world(case);
        let mut peer = receiving_world((case + 1) % 6);
        world
            .economy_events
            .record_money_audio(0, HostMoneyAudio::Deposit);
        peer.economy_events
            .record_money_audio(0, HostMoneyAudio::Withdraw);
        let live = world.current_native_slave_source(ObjectId(42), 0).unwrap();
        let peer_live = peer.current_native_slave_source(ObjectId(42), 0).unwrap();
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        let frozen = frame
            .native_resident_drawable_inputs()
            .into_iter()
            .find(|i| i.id == ObjectId(42))
            .unwrap();
        assert_eq!(
            live.condition_bits,
            frozen.model_condition_bits_with_combat_flags(),
            "case {case}"
        );
        assert_eq!(live.template_name, frozen.template_name);
        assert_eq!(live.mesh_scale, frozen.mesh_scale);
        assert_eq!(live.under_construction, frozen.under_construction);
        assert_eq!(live.construction_percent, frozen.construction_percent);
        assert_eq!(
            live.receiving_draw_models
                .first_object_draw(live.condition_bits),
            frozen.first_object_draw()
        );
        // Only the actual frame admission drains these completed receipts.
        assert_eq!(
            frame.money_audio_receipts.len(),
            1,
            "live query drained money audio case {case}"
        );
        assert_eq!(frame.money_audio_receipts[0].kind, HostMoneyAudio::Deposit);
        assert_eq!(
            peer.economy_events.take_money_audio()[0].kind,
            HostMoneyAudio::Withdraw
        );
        assert_eq!(
            peer_live.condition_bits,
            peer.current_native_slave_source(ObjectId(42), 0)
                .unwrap()
                .condition_bits
        );
    }
}

#[test]
fn current_slave_source_reports_missing_metadata_without_template_or_frame_fallback() {
    let mut world = GameLogic::new();
    world.add_object(Object::new(
        ThingTemplate::new("NoReceivingDefinitions"),
        ObjectId(42),
        Team::USA,
    ));
    assert!(world.current_native_slave_source(ObjectId(42), 0).is_err());
    assert!(world.current_native_slave_source(ObjectId(999), 0).is_err());
}

#[test]
fn live_camera_conditions_use_explicit_receiving_viewer_for_hidden_garrison() {
    let mut world = receiving_world(3);
    assert_eq!(
        world.host_object(ObjectId(42)).unwrap().contained_units(),
        vec![ObjectId(43)]
    );
    assert_eq!(
        world.host_authoritative_occupant_count(ObjectId(42)),
        Some(1)
    );
    assert!(!world.host_object(ObjectId(43)).unwrap().status.detected);
    let hidden = world.current_native_slave_source(ObjectId(42), 0).unwrap();
    let apparent = world.current_native_slave_source(ObjectId(42), 1).unwrap();
    let garrisoned = 1u128 << crate::game_logic::host_enum_table_residual::garrisoned_model_bit();
    assert_eq!(hidden.condition_bits & garrisoned, 0);
    assert_ne!(apparent.condition_bits & garrisoned, 0);
    for (viewer, live) in [(0, hidden), (1, apparent)] {
        let frame = PresentationFrame::build_from_logic(&mut world, viewer);
        let input = frame
            .native_resident_drawable_inputs()
            .into_iter()
            .find(|i| i.id == ObjectId(42))
            .unwrap();
        assert_eq!(
            live.condition_bits,
            input.model_condition_bits_with_combat_flags()
        );
    }
}
