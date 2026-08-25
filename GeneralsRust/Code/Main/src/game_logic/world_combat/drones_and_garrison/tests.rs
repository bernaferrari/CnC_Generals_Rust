use super::super::super::*;
use super::firepoints::{
    HELIX_OCCUPANT_FIRE_HEIGHT, garrison_occupant_fire_point, open_contain_exit_path,
    transport_passenger_fire_origin,
};
use crate::game_logic::{AIState, GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

/// C++ NeutonBlastBehavior.cpp:124-127 — unmanned vehicles are aiIdle'd
/// and deselectObject'd so they do not stay selected or keep orders.
#[test]
fn neutron_unman_deselects_and_idles_ai() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));

    let mut tank = ThingTemplate::new("NeutronUnmanTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0);
    logic.templates.insert("NeutronUnmanTank".to_string(), tank);

    let tank_id = logic
        .create_object("NeutronUnmanTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank");
    logic.select_objects(0, vec![tank_id]);
    {
        let obj = logic.host_object_mut(tank_id).expect("tank mut");
        obj.set_ai_state(AIState::Moving);
        obj.target_location = Some(Vec3::new(80.0, 0.0, 0.0));
        obj.set_status_moving(true);
        assert!(obj.selected || obj.status.selected);
        assert!(matches!(obj.ai_state, AIState::Moving));
    }
    assert!(
        logic
            .players
            .get(&0)
            .unwrap()
            .selected_objects
            .contains(&tank_id)
    );

    let (kills, unmanned, vehicle_kills) =
        logic.apply_neutron_blast_at(Vec3::ZERO, Team::China, None, true);
    assert_eq!(kills, 0);
    assert_eq!(unmanned, 1);
    assert_eq!(vehicle_kills, 0);

    let obj = logic.host_object(tank_id).expect("husk");
    assert!(obj.is_unmanned(), "neutron must unman the vehicle");
    assert_eq!(obj.team, Team::Neutral);
    assert!(
        !obj.selected,
        "C++ deselectObject must clear object.selected"
    );
    assert!(
        !obj.status.selected,
        "C++ deselectObject must clear status.selected"
    );
    assert!(
        matches!(obj.ai_state, AIState::Idle),
        "C++ aiIdle(CMD_FROM_AI) must idle the husk"
    );
    assert!(
        obj.target_location.is_none(),
        "idle unman must drop pending move orders"
    );
    assert!(
        !logic
            .players
            .get(&0)
            .unwrap()
            .selected_objects
            .contains(&tank_id),
        "PLAYERMASK_ALL deselect must drop the husk from the player roster"
    );
    assert!(!logic.selected_objects.contains(&tank_id));
}

/// C++ OverlordContain.cpp:553 — BattleBunker infantry fire from the tank.
#[test]
fn overlord_bunker_infantry_residual_fire_without_helix_flag() {
    let mut logic = GameLogic::new();
    let mut overlord = ThingTemplate::new("ChinaTankOverlord");
    overlord
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    logic
        .templates
        .insert("ChinaTankOverlord".to_string(), overlord);
    let mut red = ThingTemplate::new("ChinaRedguard");
    red.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic.templates.insert("ChinaRedguard".to_string(), red);
    let mut enemy = ThingTemplate::new("UsaRanger");
    enemy
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("UsaRanger".to_string(), enemy);

    let tank = logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    {
        let o = logic.host_object_mut(tank).unwrap();
        o.install_overlord_battle_bunker(5);
        o.passengers_allowed_to_fire = false;
    }
    let rider = logic
        .create_object("ChinaRedguard", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("rider");
    {
        let o = logic.host_object_mut(tank).unwrap();
        assert!(o.add_occupant(rider), "bunker must accept infantry");
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.contained_by = Some(tank);
        r.set_ai_state(AIState::Docked);
        if r.weapon.is_none() {
            r.weapon = Some(crate::game_logic::Weapon::default());
        }
        if let Some(w) = r.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 150.0;
            w.damage = 10.0;
        }
    }
    let victim = logic
        .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_transport_passenger_residual_fire(rider);
    let hp_after = logic.host_object(victim).unwrap().health.current;
    assert!(
        hp_after < hp_before - 0.01,
        "bunker infantry must fire (before={hp_before} after={hp_after})"
    );
    assert!(
        logic.host_object(tank).unwrap().passengers_allowed_to_fire,
        "live bunker fire sets passengers_allowed_to_fire"
    );
}

/// C++ TransportContain::isPassengerAllowedToFire — vehicles ride silent.
#[test]
fn combat_chinook_vehicle_rider_does_not_residual_fire() {
    let mut logic = GameLogic::new();
    let mut chinook = ThingTemplate::new("AirF_AmericaVehicleChinook");
    chinook
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    logic
        .templates
        .insert("AirF_AmericaVehicleChinook".to_string(), chinook);
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee);
    let mut enemy = ThingTemplate::new("GLAVehicleTechnical");
    enemy
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), enemy);

    let bird = logic
        .create_object(
            "AirF_AmericaVehicleChinook",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("chinook");
    {
        let c = logic.host_object_mut(bird).unwrap();
        c.install_combat_chinook_transport();
    }
    let rider = logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    {
        let o = logic.host_object_mut(bird).unwrap();
        assert!(o.add_occupant(rider), "Combat Chinook admits vehicles");
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.contained_by = Some(bird);
        r.set_ai_state(AIState::Docked);
        r.weapon = Some(crate::game_logic::Weapon {
            last_fire_time: -10.0,
            reload_time: 0.1,
            range: 150.0,
            damage: 40.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    let victim = logic
        .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_transport_passenger_residual_fire(rider);
    let hp_after = logic.host_object(victim).unwrap().health.current;
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "vehicle rider must not fire out of Combat Chinook (before={hp_before} after={hp_after})"
    );
}

/// hq-8eobz: C++ onSubdualChange → orderAllPassengersToIdle.
#[test]
fn subdued_transport_stops_passenger_residual_fire() {
    let mut logic = GameLogic::new();
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger);
    let mut enemy = ThingTemplate::new("GLARebel");
    enemy
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("GLARebel".to_string(), enemy);

    let truck = logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    {
        let t = logic.host_object_mut(truck).unwrap();
        t.passengers_allowed_to_fire = true;
    }
    let rider = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let o = logic.host_object_mut(truck).unwrap();
        assert!(o.add_occupant(rider), "humvee must accept infantry");
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.contained_by = Some(truck);
        r.set_ai_state(AIState::Docked);
        r.weapon = Some(crate::game_logic::Weapon {
            last_fire_time: -10.0,
            reload_time: 0.1,
            range: 150.0,
            damage: 10.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    let victim = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_transport_passenger_residual_fire(rider);
    let hp_after_open = logic.host_object(victim).unwrap().health.current;
    assert!(
        hp_after_open < hp_before - 0.01,
        "unjammed Humvee passengers must fire (before={hp_before} after={hp_after_open})"
    );

    {
        let t = logic.host_object_mut(truck).unwrap();
        t.set_disabled_subdued(true);
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        if let Some(w) = r.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    logic.set_current_frame(60);
    logic.try_transport_passenger_residual_fire(rider);
    let hp_after_jam = logic.host_object(victim).unwrap().health.current;
    assert!(
        (hp_after_jam - hp_after_open).abs() < 0.01,
        "subdued Humvee must stop passenger fire (open={hp_after_open} jammed={hp_after_jam})"
    );
}

fn garrison_template(name: &str, immune: bool, enclosing: bool) -> ThingTemplate {
    use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        immune_to_clear_building_attacks: immune,
        is_enclosing_container: enclosing,
        ..ContainModuleMetadata::default()
    };
    t
}

#[test]
fn immune_to_clear_bunker_keeps_occupants() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "ChinaBunker".into(),
        garrison_template("ChinaBunker", true, true),
    );
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);

    let bunker = logic
        .create_object("ChinaBunker", Team::China, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    let killed = logic.apply_kill_garrisoned_to_target(bunker, Team::USA, 5.0, None);
    assert_eq!(killed, 0, "ImmuneToClear bunkers keep occupants");
    assert!(logic.host_object(ranger_id).unwrap().is_alive());
    assert_eq!(
        logic.host_object(bunker).unwrap().contained_units(),
        vec![ranger_id]
    );
}

/// C++ MicrowaveTankBuildingClearer DelayBetweenShots 100ms → 3f; 1 occupant/shot.
#[test]
fn microwave_clearer_delay_is_100ms_one_occupant_per_shot() {
    use crate::game_logic::host_microwave::{
        HOST_MICROWAVE_CLEAR_PER_SHOT, HOST_MICROWAVE_DELAY_FRAMES, MICROWAVE_LOGIC_FPS,
    };
    use crate::game_logic::weapon_bootstrap::{
        MICROWAVE_BUILDING_CLEARER_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();
    let w = ThingTemplate::weapon_from_store(MICROWAVE_BUILDING_CLEARER_WEAPON)
        .expect("MicrowaveTankBuildingClearer seeded");
    let expected = HOST_MICROWAVE_DELAY_FRAMES as f32 / MICROWAVE_LOGIC_FPS;
    assert!(
        (w.reload_time - expected).abs() < 1e-3,
        "clearer DelayBetweenShots 100ms → reload {}, got {}",
        expected,
        w.reload_time
    );
    assert!((w.damage - HOST_MICROWAVE_CLEAR_PER_SHOT).abs() < 1e-3);

    let mut logic = GameLogic::new();
    logic.templates.insert(
        "ChinaBunker".into(),
        garrison_template("ChinaBunker", false, true),
    );
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);

    let bunker = logic
        .create_object("ChinaBunker", Team::China, Vec3::ZERO)
        .unwrap();
    let a = logic
        .create_object("AmericaRanger", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("AmericaRanger", Team::China, Vec3::new(6.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(bunker).unwrap();
        assert!(o.add_occupant(a));
        assert!(o.add_occupant(b));
    }
    for id in [a, b] {
        if let Some(r) = logic.host_object_mut(id) {
            r.set_contained_by(Some(bunker));
        }
    }
    let killed = logic.apply_kill_garrisoned_to_target(
        bunker,
        Team::USA,
        HOST_MICROWAVE_CLEAR_PER_SHOT,
        None,
    );
    assert_eq!(
        killed, 1,
        "PrimaryDamage 1 kills one occupant per 100ms shot"
    );
    assert_eq!(
        logic.host_object(bunker).unwrap().contained_units().len(),
        1
    );
    logic.microwaves.record_clear_shot();
    assert!(logic.microwave_residual().clear_shots > 0);
}

#[test]
fn occupied_building_gets_can_attack_and_loses_it_when_empty() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    logic.apply_garrison_contain_on_enter(bunker, ranger_id);
    {
        let b = logic.host_object(bunker).unwrap();
        assert!(b.has_object_status_bit("CAN_ATTACK"));
        assert!(
            b.can_attack(),
            "occupied garrison must accept attack orders"
        );
    }
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .remove_occupant(ranger_id)
    );
    let b = logic.host_object(bunker).unwrap();
    assert!(!b.has_object_status_bit("CAN_ATTACK"));
}

#[test]
fn garrison_fire_point_is_not_eight_point_ring() {
    use crate::game_logic::ContainModuleKind;
    let mut t = garrison_template("CivBunker", false, true);
    t.model_name = Some("nosuchmodel".into());
    let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
    obj.set_position(Vec3::new(10.0, 0.0, 20.0));
    obj.building_data = Some(crate::game_logic::BuildingData::new(
        crate::game_logic::BuildingType::Bunker,
    ));
    assert_eq!(
        obj.thing.template.contain_module.kind,
        ContainModuleKind::Garrison
    );
    let (idx, pos) = garrison_occupant_fire_point(
        &obj,
        crate::game_logic::ObjectId(2),
        Vec3::new(100.0, 0.0, 20.0),
    );
    assert_eq!(idx, 0);
    // C++ with no FIREPOINT bones uses the building origin, not a r=12 ring.
    assert!((pos - obj.get_position()).length() < 0.01);
}

#[test]
fn script_evac_left_is_not_a_circle() {
    let origin = Vec3::ZERO;
    let (start, end) =
        super::super::registries::garrison_evac_side_points_for_test(origin, 0.0, 20.0, 10.0, 1, 1);
    assert!(
        end.z.abs() >= 50.0,
        "left evac must spread along the side, not an 8-unit ring"
    );
    assert!(start.z.signum() == end.z.signum() || start.z.abs() > 0.0);
}

#[test]
fn fire_base_is_not_enclosing() {
    let t = garrison_template("AmericaFireBase", false, false);
    assert!(!t.contain_module.is_enclosing_container);
    let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(3), Team::USA);
    obj.building_data = Some(crate::game_logic::BuildingData::new(
        crate::game_logic::BuildingType::Bunker,
    ));
    assert!(!obj.is_enclosing_garrison_container());
}

#[test]
fn garrison_enter_deselects_occupant() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.select();
        r.owner_player_id = Some(0);
    }
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .selected_objects
        .push(ranger_id);
    logic.selected_objects.push(ranger_id);
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
        r.deselect();
    }
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .selected_objects
        .retain(|id| *id != ranger_id);
    logic.selected_objects.retain(|id| *id != ranger_id);
    let r = logic.host_object(ranger_id).unwrap();
    assert!(!r.selected);
    assert!(!logic.selected_objects.contains(&ranger_id));
    assert!(r.status.unselectable, "hq-4ai0f enter sets UNSELECTABLE");
    assert!(r.status.masked, "hq-4ai0f enclosing enter sets MASKED");
    assert!(!r.is_selectable());
}

/// hq-4ai0f: live enter path sets UNSELECTABLE/MASKED (enclosing bunker).
#[test]
fn support_states_enter_sets_unselectable_and_masked() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        b.owner_player_id = Some(0);
        if b.building_data.is_none() {
            b.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Bunker,
            ));
        }
    }
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger_id).unwrap();
        r.owner_player_id = Some(0);
        r.set_ai_state(AIState::Entering);
        r.target = Some(bunker);
    }
    logic.update_support_states(&[ranger_id], 1.0 / 30.0);
    let r = logic.host_object(ranger_id).unwrap();
    assert_eq!(r.contained_by, Some(bunker), "ranger must enter bunker");
    assert!(r.status.unselectable, "enter sets UNSELECTABLE");
    assert!(r.status.masked, "enclosing bunker sets MASKED");
    assert!(!r.is_selectable());
}

/// hq-4ai0f: Fire Base (IsEnclosingContainer=No) is UNSELECTABLE but not MASKED.
#[test]
fn support_states_firebase_enter_does_not_mask() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic.templates.insert(
        "AmericaFireBase".into(),
        garrison_template("AmericaFireBase", false, false),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let fb = logic
        .create_object("AmericaFireBase", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(fb).unwrap();
        b.owner_player_id = Some(0);
        if b.building_data.is_none() {
            b.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Bunker,
            ));
        }
    }
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger_id).unwrap();
        r.owner_player_id = Some(0);
        r.set_ai_state(AIState::Entering);
        r.target = Some(fb);
    }
    logic.update_support_states(&[ranger_id], 1.0 / 30.0);
    let r = logic.host_object(ranger_id).unwrap();
    assert_eq!(r.contained_by, Some(fb), "ranger must enter Fire Base");
    assert!(r.status.unselectable, "enter still sets UNSELECTABLE");
    assert!(!r.status.masked, "non-enclosing Fire Base must not MASK");
    assert!(!r.is_selectable());
}

fn infantry_template(name: &str) -> ThingTemplate {
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    t.transport_slot_count = Some(1);
    t
}

#[test]
fn hide_uses_stealth_garrison_kind_not_stealthed_bits() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut ranger = infantry_template("AmericaRanger");
    ranger.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaRanger".into(), ranger);
    let mut ninja = infantry_template("JapanNinja");
    ninja.add_kind_of(KindOf::StealthGarrison);
    logic.templates.insert("JapanNinja".into(), ninja);

    let bunker = logic
        .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.status.stealthed = true;
        r.status.detected = false;
    }
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    logic.recalc_garrison_apparent_controller(bunker);
    assert!(
        !logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .hide_garrisoned_state,
        "ordinary stealthed infantry must not hide the building"
    );

    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .remove_occupant(ranger_id)
    );
    let ninja_id = logic
        .create_object("JapanNinja", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.status.stealthed = false;
        n.status.detected = false;
    }
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ninja_id)
    );
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.set_contained_by(Some(bunker));
    }
    logic.recalc_garrison_apparent_controller(bunker);
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .hide_garrisoned_state,
        "STEALTH_GARRISON kind hides even while destalthed and not DETECTED"
    );
}

#[test]
fn enemy_may_enter_stealth_garrison_only_civilian_and_kick() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", false));
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut ninja = infantry_template("JapanNinja");
    ninja.add_kind_of(KindOf::StealthGarrison);
    logic.templates.insert("JapanNinja".into(), ninja);
    logic
        .templates
        .insert("ChinaRedguard".into(), infantry_template("ChinaRedguard"));

    let bunker = logic
        .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
        .unwrap();
    if let Some(b) = logic.host_object_mut(bunker) {
        b.owner_player_id = None;
    }
    let ninja_id = logic
        .create_object("JapanNinja", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.owner_player_id = Some(0);
        n.status.detected = false;
    }
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ninja_id)
    );
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.set_contained_by(Some(bunker));
    }
    let enemy = logic
        .create_object("ChinaRedguard", Team::China, Vec3::new(3.0, 0.0, 0.0))
        .unwrap();
    if let Some(e) = logic.host_object_mut(enemy) {
        e.owner_player_id = Some(1);
    }
    assert!(
        logic.can_unit_enter_normal_target(enemy, bunker),
        "C++ lets a non-owner Enter a stealth-garrison-only civilian"
    );
    logic.kick_other_controller_occupants_for_enter(bunker, enemy);
    let ninja = logic.host_object(ninja_id).unwrap();
    assert!(
        ninja.status.detected,
        "STEALTH_GARRISON kick markAsDetected"
    );
    assert!(ninja.contained_by.is_none());
    assert!(
        matches!(ninja.ai_state, AIState::Moving),
        "kicked occupant must walk out, not idle"
    );
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn evac_burst_walks_out_instead_of_idling_at_origin() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::new(10.0, 0.0, 20.0))
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(12.0, 0.0, 20.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    assert!(logic.evacuate_container_now(bunker, false));
    let r = logic.host_object(ranger_id).unwrap();
    assert!(matches!(r.ai_state, AIState::Moving));
    assert!(r.status.moving);
    let dest = r.movement.target_position.unwrap_or(r.get_position());
    assert!(
        (dest - Vec3::new(10.0, 0.0, 20.0)).length() > 1.0,
        "burst dest must leave the building origin"
    );
}

#[test]
fn really_damaged_garrison_rejects_enter_unless_firebase() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut firebase = garrison_template("AmericaFireBase", false, false);
    firebase.add_kind_of(KindOf::GarrisonableUntilDestroyed);
    logic.templates.insert("AmericaFireBase".into(), firebase);
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.can_unit_enter_normal_target(ranger, bunker));
    {
        let b = logic.host_object_mut(bunker).unwrap();
        b.health.current = 200.0;
        b.refresh_model_condition_bits();
        assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }
    assert!(
        !logic.can_unit_enter_normal_target(ranger, bunker),
        "C++ isValidContainerFor rejects BODY_REALLYDAMAGED civilian/faction buildings"
    );

    let fb = logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let b = logic.host_object_mut(fb).unwrap();
        b.health.current = 200.0;
        b.refresh_model_condition_bits();
        assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }
    assert!(
        logic.can_unit_enter_normal_target(ranger, fb),
        "KINDOF_GARRISONABLE_UNTIL_DESTROYED stays occupiable through ReallyDamaged"
    );
}

#[test]
fn really_damaged_ejects_garrison_with_burst_walk() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::new(10.0, 0.0, 20.0))
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(12.0, 0.0, 20.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    {
        let b = logic.host_object_mut(bunker).unwrap();
        b.health.current = 200.0;
        b.refresh_model_condition_bits();
        assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }
    logic.check_building_damage_states(&[bunker]);
    let r = logic.host_object(ranger_id).unwrap();
    assert!(r.contained_by.is_none());
    assert!(
        matches!(r.ai_state, AIState::Moving),
        "ReallyDamaged eject must walk out, not Idle on an 8-unit ring"
    );
    assert!(r.status.moving);
    let dest = r.movement.target_position.unwrap_or(r.get_position());
    assert!(
        (dest - Vec3::new(10.0, 0.0, 20.0)).length() > 1.0,
        "burst dest must leave the building origin"
    );
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn firebase_really_damaged_does_not_eject() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut logic = GameLogic::new();
    let mut firebase = garrison_template("AmericaFireBase", false, false);
    firebase.add_kind_of(KindOf::GarrisonableUntilDestroyed);
    logic.templates.insert("AmericaFireBase".into(), firebase);
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let fb = logic
        .create_object("AmericaFireBase", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.host_object_mut(fb).unwrap().add_occupant(ranger_id));
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(fb));
        r.set_ai_state(AIState::Garrisoned);
    }
    {
        let b = logic.host_object_mut(fb).unwrap();
        b.health.current = 200.0;
        b.refresh_model_condition_bits();
        assert_eq!(b.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }
    logic.check_building_damage_states(&[fb]);
    assert_eq!(
        logic.host_object(ranger_id).unwrap().contained_by,
        Some(fb),
        "GARRISONABLE_UNTIL_DESTROYED must keep occupants through ReallyDamaged"
    );
}

#[test]
fn garrison_fire_points_switch_with_body_damage() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let t = garrison_template("CivBunker", false, true);
    let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
    obj.set_position(Vec3::new(10.0, 0.0, 20.0));
    let mut bd = crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
    bd.garrison_fire_points = vec![Vec3::new(11.0, 0.0, 20.0)];
    bd.garrison_fire_points_damaged = vec![Vec3::new(30.0, 0.0, 20.0)];
    bd.garrison_fire_points_really_damaged = vec![Vec3::new(50.0, 0.0, 20.0)];
    bd.garrison_point_occupant = vec![None];
    obj.building_data = Some(bd);
    obj.body_damage_state = HostBodyDamageType::Pristine;
    let (_, p0) = garrison_occupant_fire_point(
        &obj,
        crate::game_logic::ObjectId(2),
        Vec3::new(100.0, 0.0, 20.0),
    );
    assert!((p0 - Vec3::new(11.0, 0.0, 20.0)).length() < 0.01);
    obj.body_damage_state = HostBodyDamageType::Damaged;
    let (_, p1) = garrison_occupant_fire_point(
        &obj,
        crate::game_logic::ObjectId(2),
        Vec3::new(100.0, 0.0, 20.0),
    );
    assert!((p1 - Vec3::new(30.0, 0.0, 20.0)).length() < 0.01);
    obj.body_damage_state = HostBodyDamageType::ReallyDamaged;
    let (_, p2) = garrison_occupant_fire_point(
        &obj,
        crate::game_logic::ObjectId(2),
        Vec3::new(100.0, 0.0, 20.0),
    );
    assert!((p2 - Vec3::new(50.0, 0.0, 20.0)).length() < 0.01);
}

#[test]
fn garrison_fire_releases_old_firepoint_before_taking_closer() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GLARebel".into(), infantry_template("GLARebel"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        let mut bd = crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
        bd.garrison_fire_points = vec![Vec3::new(-20.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0)];
        bd.garrison_point_occupant = vec![None, None];
        bd.garrison_points_initialized = true;
        b.building_data = Some(bd);
    }
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 40.0,
            range: 200.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));
    {
        let b = logic.host_object_mut(bunker).unwrap();
        if let Some(bd) = b.building_data.as_mut() {
            bd.garrison_point_occupant = vec![Some(ranger), None];
        }
    }
    let _enemy = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    let slots = logic
        .host_object(bunker)
        .and_then(|b| b.building_data.as_ref())
        .map(|bd| bd.garrison_point_occupant.clone())
        .unwrap_or_default();
    assert_eq!(
        slots,
        vec![None, Some(ranger)],
        "C++ trackTargets frees the old FIREPOINT before taking the closer window: {slots:?}"
    );
}

#[test]
fn garrison_fire_frees_firepoint_when_no_in_range_victim() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        let mut bd = crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
        bd.garrison_fire_points = vec![Vec3::new(-8.0, 0.0, 0.0), Vec3::new(8.0, 0.0, 0.0)];
        bd.garrison_point_occupant = vec![None, None];
        bd.garrison_points_initialized = true;
        b.building_data = Some(bd);
    }
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 20.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));
    {
        let b = logic.host_object_mut(bunker).unwrap();
        if let Some(bd) = b.building_data.as_mut() {
            bd.garrison_point_occupant = vec![Some(ranger), None];
        }
    }
    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    let slots = logic
        .host_object(bunker)
        .and_then(|b| b.building_data.as_ref())
        .map(|bd| bd.garrison_point_occupant.clone())
        .unwrap_or_default();
    assert_eq!(
        slots,
        vec![None, None],
        "C++ removeInvalid must free the window when no in-range victim: {slots:?}"
    );
}

#[test]
fn transport_fire_origin_uses_firepoint_or_hull() {
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee.add_kind_of(KindOf::Vehicle).set_health(240.0);
    humvee.model_name = Some("nosuchmodel".into());
    let mut obj = crate::game_logic::Object::new(humvee, crate::game_logic::ObjectId(9), Team::USA);
    obj.set_position(Vec3::new(7.0, 0.0, 3.0));
    let origin = transport_passenger_fire_origin(&obj, 0);
    assert!(
        (origin - obj.get_position()).length() < 0.01,
        "no FIREPOINT bones → hull center (C++ m_noFirePointsInArt)"
    );
}

#[test]
fn helix_fire_origin_is_hull_plus_eight_not_firepoint() {
    let mut helix = ThingTemplate::new("ChinaVehicleHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(600.0);
    helix.model_name = Some("nosuchmodel".into());
    let mut obj =
        crate::game_logic::Object::new(helix, crate::game_logic::ObjectId(8), Team::China);
    obj.install_helix_transport();
    obj.set_position(Vec3::new(7.0, 10.0, 3.0));
    let origin = transport_passenger_fire_origin(&obj, 0);
    let expected = obj.get_position() + Vec3::new(0.0, HELIX_OCCUPANT_FIRE_HEIGHT, 0.0);
    assert!(
        (origin - expected).length() < 0.01,
        "HelixContain::redeployOccupants is hull+8 (host Y), not FIREPOINT: {origin:?}"
    );
}

#[test]
fn heal_contain_auto_exit_walks_exit_path() {
    use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut pad = ThingTemplate::new("AmericaBarracks");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::HealPad)
        .set_health(800.0);
    pad.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Heal,
        slots: Some(10),
        admission: ContainAdmission::InfantryOnly,
        frames_for_full_heal: Some(0),
        ..ContainModuleMetadata::default()
    };
    logic.templates.insert("AmericaBarracks".into(), pad);
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let barracks = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.health.current = 20.0;
        r.owner_player_id = Some(0);
    }
    assert!(
        logic
            .host_object_mut(barracks)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(barracks));
    }
    logic.tunnel_network.stamp_contained_by_frame(ranger_id, 0);
    logic.frame = 1;
    logic.update_support_states(&[ranger_id], 1.0 / 30.0);
    let r = logic.host_object(ranger_id).unwrap();
    assert!(r.contained_by.is_none());
    assert!(
        matches!(r.ai_state, AIState::Moving),
        "HealContain auto-exit must follow ExitStart/End, not Idle on an 8-unit circle"
    );
    assert!(r.status.moving);
}

#[test]
fn open_contain_exit_path_cycles_numbered_like_cpp() {
    let mut t = ThingTemplate::new("HV_EXIT");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    let mut obj = crate::game_logic::Object::new(t, crate::game_logic::ObjectId(1), Team::USA);
    obj.set_position(Vec3::new(10.0, 0.0, 4.0));
    obj.set_orientation(0.0);
    let origin = obj.get_position();
    let (s1, e1, n1) = open_contain_exit_path(&obj, 0, 3);
    assert_eq!(n1, 2, "C++ m_whichExitPath cycles 1→2 after ExitStart01");
    assert!((s1 - origin).length() < 0.01, "missing bone → hull start");
    assert!(
        (e1 - origin).length() > 8.0,
        "missing bone → forward ExitEnd, not Idle ring: e1={e1:?}"
    );
    let (_, _, n2) = open_contain_exit_path(&obj, n1, 3);
    assert_eq!(n2, 3);
    let (_, _, n3) = open_contain_exit_path(&obj, n2, 3);
    assert_eq!(n3, 1);
    let (_, e_single, next_single) = open_contain_exit_path(&obj, 1, 1);
    assert_eq!(next_single, 1);
    assert!((e_single - origin).length() > 8.0);
}

#[test]
fn walk_unit_via_open_contain_exit_cycles_humvee_paths() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("HV_CYCLE");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("HV_CYCLE".into(), t);
    let mut p = ThingTemplate::new("HV_CYCLE_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("HV_CYCLE_P".into(), p);
    let transport = logic
        .create_object("HV_CYCLE", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        logic
            .host_object_mut(transport)
            .unwrap()
            .install_humvee_transport();
    }
    let a = logic
        .create_object("HV_CYCLE_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("HV_CYCLE_P", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    logic.walk_unit_via_open_contain_exit(a, transport);
    assert_eq!(
        logic.host_object(transport).unwrap().which_exit_path,
        2,
        "first rider consumes ExitStart01"
    );
    logic.walk_unit_via_open_contain_exit(b, transport);
    assert_eq!(
        logic.host_object(transport).unwrap().which_exit_path,
        3,
        "second rider consumes ExitStart02"
    );
    assert_eq!(logic.host_object(a).unwrap().ai_state, AIState::Moving);
    assert_eq!(logic.host_object(b).unwrap().ai_state, AIState::Moving);
}

#[test]
fn walk_unit_via_open_contain_exit_resets_mood_and_plays_template_audio() {
    // hq-j0ggx / hq-c77h2: execute_exit walk resets next_mood_check_time
    // and drains leftover onRemoving SoundExit + SoundFallingFromPlane.
    let mut logic = GameLogic::new();
    logic.frame = 77;
    let mut t = ThingTemplate::new("MOOD_HV");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("MOOD_HV".into(), t);
    let mut p = ThingTemplate::new("MOOD_HV_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("MOOD_HV_P".into(), p);
    let transport = logic
        .create_object("MOOD_HV", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let rider = logic
        .create_object("MOOD_HV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    if let Some(u) = logic.host_object_mut(rider) {
        u.next_mood_check_time = 9999;
        u.set_contained_by(Some(transport));
    }
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let u = logic.host_object(rider).unwrap();
    assert_eq!(u.next_mood_check_time, 77);
    assert_eq!(u.ai_state, AIState::Moving);
    let audio = gamelogic::object::contain::open_contain::leftover_last_on_removing_template_call()
        .expect("onRemoving template audio");
    assert_eq!(audio.container_template, "MOOD_HV");
    assert_eq!(audio.container_id, transport.0);
    assert_eq!(audio.rider_template, "MOOD_HV_P");
    assert_eq!(audio.rider_id, rider.0);
}

#[test]
fn walk_unit_via_open_contain_exit_airborne_falls_without_invented_hull_kick() {
    // hq-qhzox: C++ onRemoving setAllowToFall when above terrain.
    // KeepContainerVelocityOnExit defaults false; do not invent hull motive.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AIR_DROP_T");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(8),
        ..Default::default()
    };
    logic.templates.insert("AIR_DROP_T".into(), t);
    let mut p = ThingTemplate::new("AIR_DROP_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("AIR_DROP_P".into(), p);
    let transport = logic
        .create_object("AIR_DROP_T", Team::USA, Vec3::new(0.0, 20.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.status.airborne_target = true;
        h.movement.velocity = Vec3::new(12.0, 0.0, 0.0);
    }
    let rider = logic
        .create_object("AIR_DROP_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    if let Some(u) = logic.host_object_mut(rider) {
        u.set_contained_by(Some(transport));
    }
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let u = logic.host_object(rider).unwrap();
    assert!(u.allow_to_fall);
    assert_eq!(u.motive_frames_remaining, 0);
    assert_eq!(u.physics_accel, Vec3::ZERO);
}

#[test]
fn walk_unit_via_open_contain_exit_keep_velocity_applies_authored_kick() {
    // hq-qhzox: leftover/C++ kick only when KeepContainerVelocityOnExit is Yes.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("KEEP_VEL_T");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        keep_container_velocity_on_exit: true,
        ..Default::default()
    };
    logic.templates.insert("KEEP_VEL_T".into(), t);
    let mut p = ThingTemplate::new("KEEP_VEL_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("KEEP_VEL_P".into(), p);
    let transport = logic
        .create_object("KEEP_VEL_T", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        logic.host_object_mut(transport).unwrap().movement.velocity = Vec3::new(9.0, 0.0, 0.0);
    }
    let rider = logic
        .create_object("KEEP_VEL_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    if let Some(u) = logic.host_object_mut(rider) {
        u.set_contained_by(Some(transport));
    }
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let u = logic.host_object(rider).unwrap();
    assert_eq!(
        u.motive_frames_remaining,
        crate::game_logic::MOTIVE_FRAMES_RESIDUAL
    );
    assert!((u.physics_accel.x - 9.0).abs() < 1e-4);
    assert!(!u.allow_to_fall);
}

#[test]
fn walk_unit_via_open_contain_exit_opens_and_closes_leftover_door() {
    // hq-jl6xr: leftover DoorOpenTime default 1 flashes OPENING then CLOSING.
    use crate::game_logic::host_enum_table_residual::{
        door_1_closing_model_bit, door_1_opening_model_bit,
    };
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("DOOR_HV");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("DOOR_HV".into(), t);
    let mut p = ThingTemplate::new("DOOR_HV_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("DOOR_HV_P".into(), p);
    let transport = logic
        .create_object("DOOR_HV", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let rider = logic
        .create_object("DOOR_HV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    if let Some(u) = logic.host_object_mut(rider) {
        u.set_contained_by(Some(transport));
    }
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let open_b = door_1_opening_model_bit();
    let close_b = door_1_closing_model_bit();
    let bits = logic.host_object(transport).unwrap().model_condition_bits;
    assert_ne!(
        bits & (1u128 << open_b),
        0,
        "exit must leftover-open the door"
    );
    assert_eq!(bits & (1u128 << close_b), 0);
    assert_eq!(
        logic.host_object(transport).unwrap().door_close_countdown,
        1,
        "C++ default DoorOpenTime is 1 frame"
    );
    logic.update_support_states(&[transport, rider], 1.0 / 30.0);
    let bits = logic.host_object(transport).unwrap().model_condition_bits;
    assert_eq!(
        bits & (1u128 << open_b),
        0,
        "next OpenContain::update closes"
    );
    assert_ne!(bits & (1u128 << close_b), 0);
    assert_eq!(
        logic.host_object(transport).unwrap().door_close_countdown,
        0
    );
}

#[test]
fn walk_unit_via_open_contain_exit_skips_garrison_door_bits() {
    // C++ GarrisonContain::exitObjectViaDoor never sets DoorOpenTime bits.
    use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "DOOR_BUNKER".into(),
        garrison_template("DOOR_BUNKER", false, true),
    );
    logic
        .templates
        .insert("DOOR_BUNKER_P".into(), infantry_template("DOOR_BUNKER_P"));
    let bunker = logic
        .create_object("DOOR_BUNKER", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let rider = logic
        .create_object("DOOR_BUNKER_P", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    logic.walk_unit_via_open_contain_exit(rider, bunker);
    let bits = logic.host_object(bunker).unwrap().model_condition_bits;
    assert_eq!(bits & (1u128 << door_1_opening_model_bit()), 0);
}

#[test]
fn walk_unit_via_open_contain_exit_door_open_time_zero_skips() {
    // DeliverPayloadAIUpdate authors DoorOpenTime=0 to opt out.
    use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("DOOR_ZERO");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        door_open_time: 0,
        ..Default::default()
    };
    logic.templates.insert("DOOR_ZERO".into(), t);
    let mut p = ThingTemplate::new("DOOR_ZERO_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("DOOR_ZERO_P".into(), p);
    let transport = logic
        .create_object("DOOR_ZERO", Team::USA, Vec3::ZERO)
        .unwrap();
    let rider = logic
        .create_object("DOOR_ZERO_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let c = logic.host_object(transport).unwrap();
    assert_eq!(c.door_close_countdown, 0);
    assert_eq!(
        c.model_condition_bits & (1u128 << door_1_opening_model_bit()),
        0
    );
}

#[test]
fn walk_unit_via_open_contain_exit_copies_layer_and_gates_allow_to_fall() {
    // hq-csdhg: leftover set_layer(owner) + temp setAllowToFall(false).
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("LAYER_HV");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(5),
        door_open_time: 0,
        ..Default::default()
    };
    logic.templates.insert("LAYER_HV".into(), t);
    let mut p = ThingTemplate::new("LAYER_HV_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("LAYER_HV_P".into(), p);
    let transport = logic
        .create_object("LAYER_HV", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.pathfind_layer = 3;
    }
    let rider = logic
        .create_object("LAYER_HV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    if let Some(u) = logic.host_object_mut(rider) {
        u.allow_to_fall = true;
        u.pathfind_layer = 1;
        u.set_contained_by(Some(transport));
    }
    logic.walk_unit_via_open_contain_exit(rider, transport);
    let u = logic.host_object(rider).unwrap();
    assert_eq!(u.pathfind_layer, 3, "rider must copy container layer");
    assert!(
        u.allow_to_fall,
        "allowToFall restored after pathfind gate (hull not airborne)"
    );
    assert_eq!(u.ai_state, AIState::Moving);
}

#[test]
fn play_container_enter_sound_drains_leftover_template_sound_enter() {
    // hq-c77h2: live enter path must call leftover onContaining SoundEnter.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ENTER_AUD");
    t.add_kind_of(KindOf::Vehicle);
    t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Transport,
        slots: Some(1),
        ..Default::default()
    };
    logic.templates.insert("ENTER_AUD".into(), t);
    let transport = logic
        .create_object("ENTER_AUD", Team::USA, Vec3::ZERO)
        .unwrap();
    logic.play_container_enter_sound(transport);
    let audio =
        gamelogic::object::contain::open_contain::leftover_last_on_containing_template_call()
            .expect("onContaining template audio");
    assert_eq!(audio.template_name, "ENTER_AUD");
    assert_eq!(audio.object_id, transport.0);
    assert!(audio.load_sounds_enabled);
}

/// C++ ActionManager.cpp:1696-1710 + Object.cpp:6111-6132.
#[test]
fn defector_rejects_structure_contained_and_unfinished() {
    let mut logic = GameLogic::new();
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic.templates.insert("TestTank".into(), tank);
    let mut barracks = ThingTemplate::new("GLABarracks");
    barracks.add_kind_of(KindOf::Structure).set_health(800.0);
    logic.templates.insert("GLABarracks".into(), barracks);

    let caster = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
        .unwrap();
    let building = logic
        .create_object("GLABarracks", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let unfinished = logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(unfinished) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.2;
    }
    let contained = logic
        .create_object("TestTank", Team::GLA, Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(contained) {
        o.set_contained_by(Some(building));
    }
    let sold = logic
        .create_object("TestTank", Team::GLA, Vec3::new(70.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(sold) {
        o.set_status_sold(true);
    }

    assert!(!logic.activate_defector(caster, building));
    assert_eq!(logic.host_object(building).unwrap().team, Team::GLA);
    assert!(!logic.activate_defector(caster, unfinished));
    assert_eq!(logic.host_object(unfinished).unwrap().team, Team::GLA);
    assert!(!logic.activate_defector(caster, contained));
    assert_eq!(logic.host_object(contained).unwrap().team, Team::GLA);
    assert!(!logic.activate_defector(caster, sold));
    assert_eq!(logic.host_object(sold).unwrap().team, Team::GLA);
}

/// C++ Object.cpp:6167-6192 — idle + VoiceDefect + kickOutOnCapture.
#[test]
fn defector_idles_and_kicks_cargo() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(2, Player::new(2, Team::GLA, "GLA", false));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let caster = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim = logic
        .create_object("AmericaVehicleHumvee", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let cargo = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::GLA,
            Vec3::new(42.0, 0.0, 0.0),
        )
        .unwrap();
    if let Some(v) = logic.host_object_mut(victim) {
        v.set_ai_state(AIState::Attacking);
        v.set_target(Some(caster));
        v.set_status_attacking(true);
        v.occupants.push(cargo);
    }
    if let Some(c) = logic.host_object_mut(cargo) {
        c.set_contained_by(Some(victim));
    }

    assert!(logic.activate_defector(caster, victim));
    let v = logic.host_object(victim).unwrap();
    assert_eq!(v.team, Team::USA);
    assert!(matches!(v.ai_state, AIState::Idle));
    assert!(!v.status.attacking);
    assert!(v.target.is_none());
    assert!(v.is_undetected_defector());
    let rider = logic.host_object(cargo).unwrap();
    assert!(rider.contained_by.is_none());
    assert_eq!(rider.team, Team::GLA);
}

/// hq-am2jn: garrison auto-fire must skip undetected stealth (C++ acquire filters).
#[test]
fn garrison_residual_fire_skips_undetected_stealth() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GLARebel".into(), infantry_template("GLARebel"));

    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let rebel = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 40.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));
    {
        let e = logic.host_object_mut(rebel).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
        assert!(e.is_effectively_stealthed());
    }

    let hp_before = logic.host_object(rebel).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    let hp_after = logic.host_object(rebel).unwrap().health.current;
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "undetected stealth must not be auto-acquired (before={hp_before} after={hp_after})"
    );
    assert_eq!(logic.garrison_residual_fires(), 0);

    {
        let e = logic.host_object_mut(rebel).unwrap();
        e.set_status_detected(true);
        assert!(!e.is_effectively_stealthed());
    }
    logic.try_garrison_residual_fire(ranger);
    let hp_detected = logic.host_object(rebel).unwrap().health.current;
    assert!(
        hp_detected < hp_before - 0.01,
        "detected stealth remains a legal acquire"
    );
}

/// hq-nzyae: garrison fire uses GARRISONED 133% range, not raw weapon.range.
#[test]
fn garrison_residual_fire_uses_garrisoned_133_range() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GLATank".into(), infantry_template("GLATank"));

    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    // 120 units: out of raw 100, inside 100 * 1.33.
    let enemy = logic
        .create_object("GLATank", Team::GLA, Vec3::new(120.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
        assert!(
            (r.effective_weapon_range(100.0) - 133.0).abs() < 0.01,
            "Garrisoned infantry must receive RANGE 133%"
        );
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));

    let hp_before = logic.host_object(enemy).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    let hp_after = logic.host_object(enemy).unwrap().health.current;
    assert!(
        hp_after < hp_before - 0.01,
        "garrisoned 133% range must reach 120 with base 100 (before={hp_before} after={hp_after})"
    );
    assert!(logic.honesty_garrison_fire_ok());
}

/// hq-nzyae: Helix infantry stay Docked but still get GARRISONED 133% range.
#[test]
fn helix_infantry_residual_fire_uses_garrisoned_133_range() {
    let mut logic = GameLogic::new();
    let mut helix = ThingTemplate::new("ChinaHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(600.0);
    logic.templates.insert("ChinaHelix".into(), helix);
    logic
        .templates
        .insert("ChinaRedguard".into(), infantry_template("ChinaRedguard"));
    logic
        .templates
        .insert("UsaRanger".into(), infantry_template("UsaRanger"));

    let heli = logic
        .create_object("ChinaHelix", Team::China, Vec3::ZERO)
        .unwrap();
    {
        let h = logic.host_object_mut(heli).unwrap();
        h.install_helix_transport();
        h.passengers_allowed_to_fire = true;
    }
    let rider = logic
        .create_object("ChinaRedguard", Team::China, Vec3::ZERO)
        .unwrap();
    assert!(logic.host_object_mut(heli).unwrap().add_occupant(rider));
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.set_contained_by(Some(heli));
        r.set_ai_state(AIState::Docked);
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        // Docked + contained must not grant the bunker AIState bonus.
        assert!((r.effective_weapon_range(100.0) - 100.0).abs() < 0.01);
    }
    let victim = logic
        .create_object("UsaRanger", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_transport_passenger_residual_fire(rider);
    let hp_after = logic.host_object(victim).unwrap().health.current;
    assert!(
        hp_after < hp_before - 0.01,
        "Helix infantry GARRISONED 133% must reach 120 with base 100 (before={hp_before} after={hp_after})"
    );
}

#[test]
fn garrison_initial_roster_spawns_occupants_on_create() {
    let mut logic = GameLogic::new();
    let mut bunker = garrison_template("RosterBunker", false, true);
    bunker.contain_module.initial_roster_template = "AmericaRanger".to_string();
    bunker.contain_module.initial_roster_count = 3;
    logic.templates.insert("RosterBunker".into(), bunker);
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);

    let bunker_id = logic
        .create_object("RosterBunker", Team::USA, Vec3::ZERO)
        .expect("roster bunker");
    let occupants = logic
        .host_object(bunker_id)
        .map(|o| o.contained_units())
        .unwrap_or_default();
    assert_eq!(
        occupants.len(),
        3,
        "C++ GarrisonContain::onObjectCreated must add InitialRoster count"
    );
    for occupant_id in occupants {
        let occupant = logic.host_object(occupant_id).expect("roster occupant");
        assert_eq!(occupant.template_name, "AmericaRanger");
        assert_eq!(occupant.contained_by, Some(bunker_id));
        assert_eq!(occupant.team, Team::USA);
    }
}

#[test]
fn garrison_without_heal_objects_does_not_heal_occupants() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut ranger = ThingTemplate::new("AmericaRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic.templates.insert("AmericaRanger".into(), ranger);
    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.health.current = 40.0;
        r.health.maximum = 120.0;
        r.set_contained_by(Some(bunker));
    }
    logic
        .tunnel_network
        .stamp_contained_by_frame(ranger_id, logic.frame);
    logic.frame = logic.frame.saturating_add(1);
    logic.update_support_states(&[ranger_id], 1.0 / 30.0);
    let after = logic.host_object(ranger_id).unwrap().health.current;
    assert!(
        (after - 40.0).abs() < 0.01,
        "HealObjects=No must not regenerate occupants, got {after}"
    );
}

fn garrison_gun_template() -> ThingTemplate {
    let mut t = ThingTemplate::new("GarrisonGun");
    t.set_health(1.0);
    t
}

/// hq-xr8yk: TunnelContain is not garrisonable — occupants do not fire out.
#[test]
fn tunnel_occupants_do_not_garrison_fire() {
    use crate::game_logic::{ContainAdmission, ContainModuleKind, ContainModuleMetadata};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut tunnel_t = ThingTemplate::new("GLATunnelNetwork");
    tunnel_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    tunnel_t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Tunnel,
        slots: Some(10),
        admission: ContainAdmission::InfantryOrVehicle,
        ..ContainModuleMetadata::default()
    };
    logic.templates.insert("GLATunnelNetwork".into(), tunnel_t);
    logic
        .templates
        .insert("GLARebel".into(), infantry_template("GLARebel"));
    logic
        .templates
        .insert("UsaRanger".into(), infantry_template("UsaRanger"));

    let tunnel = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .unwrap();
    {
        let t = logic.host_object_mut(tunnel).unwrap();
        t.owner_player_id = Some(0);
        t.install_tunnel_network_residual();
    }
    let rebel = logic
        .create_object("GLARebel", Team::GLA, Vec3::ZERO)
        .unwrap();
    {
        let r = logic.host_object_mut(rebel).unwrap();
        r.owner_player_id = Some(0);
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 40.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_ai_state(AIState::Entering);
        r.target = Some(tunnel);
    }
    logic.update_support_states(&[rebel], 1.0 / 30.0);
    let after_enter = logic.host_object(rebel).unwrap();
    assert_eq!(after_enter.contained_by, Some(tunnel));
    assert_eq!(
        after_enter.ai_state,
        AIState::Docked,
        "TunnelContain must not stamp Garrisoned"
    );

    let enemy = logic
        .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(enemy).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(rebel);
    let hp_after = logic.host_object(enemy).unwrap().health.current;
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "tunnel occupants must not fire as garrison (before={hp_before} after={hp_after})"
    );
    assert_eq!(logic.garrison_residual_fires(), 0);

    // Defense: even a wrongly stamped Garrisoned occupant stays silent.
    if let Some(r) = logic.host_object_mut(rebel) {
        r.set_ai_state(AIState::Garrisoned);
    }
    logic.try_garrison_residual_fire(rebel);
    let hp_forced = logic.host_object(enemy).unwrap().health.current;
    assert!(
        (hp_forced - hp_before).abs() < 0.01,
        "try_garrison_residual_fire must refuse TunnelContain"
    );
}

/// hq-8fh9p: Fire Base (IsEnclosingContainer=No) never creates GarrisonGun.
#[test]
fn firebase_does_not_spawn_garrison_gun() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "AmericaFireBase".into(),
        garrison_template("AmericaFireBase", false, false),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GLARebel".into(), infantry_template("GLARebel"));
    logic
        .templates
        .insert("GarrisonGun".into(), garrison_gun_template());

    let fb = logic
        .create_object("AmericaFireBase", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(fb).unwrap();
        if b.building_data.is_none() {
            b.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Bunker,
            ));
        }
    }
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(fb));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(fb).unwrap().add_occupant(ranger));

    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    assert!(logic.honesty_garrison_fire_ok());
    let guns = logic
        .host_object(fb)
        .and_then(|c| c.building_data.as_ref())
        .map(|bd| {
            bd.garrison_guns
                .iter()
                .filter(|g| g.drawable_id.is_some())
                .count()
        })
        .unwrap_or(0);
    assert_eq!(guns, 0, "Fire Base must not spawn GarrisonGun drawables");
    let spawned = logic
        .objects
        .values()
        .filter(|o| o.template_name == "GarrisonGun")
        .count();
    assert_eq!(spawned, 0, "no GarrisonGun object at Fire Base stations");
    let _ = enemy;
}

/// hq-r3dcp: DAMAGE_POISON occupant shots skip window FIRING_A muzzle.
#[test]
fn poison_garrison_shot_skips_muzzle_flash() {
    use crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A;
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    let mut toxin = infantry_template("GLAToxinRebel");
    toxin.set_primary_weapon_name("ToxinTruckGun");
    logic.templates.insert("GLAToxinRebel".into(), toxin);
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GarrisonGun".into(), garrison_gun_template());

    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        if b.building_data.is_none() {
            b.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Bunker,
            ));
        }
    }
    let rebel = logic
        .create_object("GLAToxinRebel", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("AmericaRanger", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let r = logic.host_object_mut(rebel).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(rebel));

    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(rebel);
    assert!(logic.honesty_garrison_fire_ok());
    let gun_id = logic
        .host_object(bunker)
        .and_then(|c| c.building_data.as_ref())
        .and_then(|bd| bd.garrison_guns.iter().find_map(|g| g.drawable_id));
    let Some(gid) = gun_id else {
        panic!("enclosing bunker must still spawn GarrisonGun");
    };
    let bits = logic.host_object(gid).unwrap().model_condition_bits;
    assert_eq!(
        bits & (1u128 << MC_BIT_FIRING_A),
        0,
        "DAMAGE_POISON must not set MODELCONDITION_FIRING_A"
    );
    let _ = enemy;
}

/// hq-csl6v: GarrisonGun barrels aim at the occupant's victim.
#[test]
fn garrison_gun_aims_at_occupant_target() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    logic
        .templates
        .insert("GLARebel".into(), infantry_template("GLARebel"));
    logic
        .templates
        .insert("GarrisonGun".into(), garrison_gun_template());

    let bunker = logic
        .create_object("CivBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        if b.building_data.is_none() {
            b.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Bunker,
            ));
        }
    }
    let ranger = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(20.0, 0.0, 10.0))
        .unwrap();
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.weapon = Some(crate::game_logic::Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
        r.set_contained_by(Some(bunker));
        r.set_ai_state(AIState::Garrisoned);
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));

    logic.set_current_frame(30);
    logic.try_garrison_residual_fire(ranger);
    assert!(logic.honesty_garrison_fire_ok());
    let gun_id = logic
        .host_object(bunker)
        .and_then(|c| c.building_data.as_ref())
        .and_then(|bd| bd.garrison_guns.iter().find_map(|g| g.drawable_id));
    let Some(gid) = gun_id else {
        panic!("enclosing bunker must spawn GarrisonGun");
    };
    let gun_pos = logic.host_object(gid).unwrap().get_position();
    let enemy_pos = logic.host_object(enemy).unwrap().get_position();
    let expected = (enemy_pos.z - gun_pos.z).atan2(enemy_pos.x - gun_pos.x);
    let got = logic.host_object(gid).unwrap().get_orientation();
    assert!(
        (got - expected).abs() < 0.01,
        "GarrisonGun yaw {got} must face victim {expected} (gun={gun_pos:?} tgt={enemy_pos:?})"
    );
}

#[test]
fn partial_exit_recalcs_hide_garrisoned_state() {
    // hq-ct5z2: C++ GarrisonContain::onRemoving always recals hide.
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let mut ninja = infantry_template("JapanNinja");
    ninja.add_kind_of(KindOf::StealthGarrison);
    logic.templates.insert("JapanNinja".into(), ninja);

    let bunker = logic
        .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    let ninja_id = logic
        .create_object("JapanNinja", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ninja_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.set_contained_by(Some(bunker));
        n.status.detected = false;
    }
    logic.recalc_garrison_apparent_controller(bunker);
    assert!(
        !logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .hide_garrisoned_state,
        "mixed garrison must not hide"
    );

    assert!(logic.unit_command_remove_occupant(bunker, ranger_id));
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .hide_garrisoned_state,
        "stealth-only remainder after ranger exit must hide from non-allies"
    );
}

#[test]
fn occupant_death_recalcs_hide_garrisoned_state() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let mut ninja = infantry_template("JapanNinja");
    ninja.add_kind_of(KindOf::StealthGarrison);
    logic.templates.insert("JapanNinja".into(), ninja);

    let bunker = logic
        .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    let ninja_id = logic
        .create_object("JapanNinja", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ninja_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    if let Some(n) = logic.host_object_mut(ninja_id) {
        n.set_contained_by(Some(bunker));
        n.status.detected = false;
    }
    logic.recalc_garrison_apparent_controller(bunker);
    logic.mark_object_for_destruction(ranger_id, None);
    logic.process_destroy_list();
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .hide_garrisoned_state,
        "stealth-only remainder after ranger death must hide"
    );
}

#[test]
fn player_who_entered_pulses_one_frame() {
    // hq-5yuxu: C++ OpenContain::update zeros the mask; BUILDING_ENTERED is not sticky.
    use gamelogic::scripting::{clear_host_script_query_snapshot, host_building_entered_by_player};
    clear_host_script_query_snapshot();
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "PlyrAmerica", true));
    logic.templates.insert(
        "CivBunker".into(),
        garrison_template("CivBunker", false, true),
    );
    logic
        .templates
        .insert("AmericaRanger".into(), infantry_template("AmericaRanger"));
    let bunker = logic
        .create_object("CivBunker", Team::Neutral, Vec3::ZERO)
        .unwrap();
    if let Some(b) = logic.host_object_mut(bunker) {
        b.name = "NamedBunker".into();
    }
    let ranger_id = logic
        .create_object("AmericaRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.owner_player_id = Some(0);
    }
    assert!(
        logic
            .host_object_mut(bunker)
            .unwrap()
            .add_occupant(ranger_id)
    );
    if let Some(r) = logic.host_object_mut(ranger_id) {
        r.set_contained_by(Some(bunker));
    }
    logic.stamp_player_who_entered(bunker, ranger_id);
    logic.inject_host_script_query_snapshot();
    assert_eq!(
        host_building_entered_by_player("NamedBunker", "PlyrAmerica"),
        Some(true),
        "enter frame must pulse BUILDING_ENTERED"
    );

    logic.update_support_states(&[bunker, ranger_id], 1.0 / 30.0);
    logic.inject_host_script_query_snapshot();
    assert_eq!(
        host_building_entered_by_player("NamedBunker", "PlyrAmerica"),
        Some(false),
        "next OpenContain::update must clear the pulse even while occupied"
    );
    clear_host_script_query_snapshot();
}
