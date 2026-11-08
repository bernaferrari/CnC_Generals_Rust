use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
use crate::game_logic::{AIState, GameLogic, ObjectId, Team};
use glam::Vec3;

fn passenger_world(air: bool, distance: f32, air_damage: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut weapons = std::collections::HashMap::new();
    parse_weapon_definitions_ini_into(&mut weapons, &format!(r#"
Weapon TransportGroundGun
 PrimaryDamage = 11
 AttackRange = 100
 AntiGround = Yes
 AntiAirborneVehicle = No
End
Weapon TransportAirGun
 PrimaryDamage = {air_damage}
 AttackRange = 150
 AntiGround = No
 AntiAirborneVehicle = Yes
End
"#), WeaponDefinitionLoadType::Normal);
    world.admit_weapon_content_snapshot(
        weapons.into_values().map(|weapon| (weapon.name.clone(), weapon)).collect(), &[],
    ).unwrap();
    world.admit_object_ini_sources([("transport-weapon-choice.ini".into(), r#"
Object ChoiceCarrier
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 300
  InitialHealth = 300
 End
 Behavior = TransportContain Contain
  Slots = 2
  PassengersAllowedToFire = Yes
  AllowInsideKindOf = INFANTRY
 End
End
Object ChoicePassenger
 KindOf = INFANTRY SELECTABLE CAN_ATTACK
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 WeaponSet
  Conditions = None
  Weapon = PRIMARY TransportGroundGun
  Weapon = SECONDARY TransportAirGun
 End
End
Object ChoiceGroundTarget
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 200
  InitialHealth = 200
 End
End
Object ChoiceAirTarget
 KindOf = AIRCRAFT VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 200
  InitialHealth = 200
 End
End
"#.into())]).unwrap();
    let carrier = world.create_object("ChoiceCarrier", Team::USA, Vec3::ZERO).unwrap();
    let rider = world.create_object("ChoicePassenger", Team::USA, Vec3::ZERO).unwrap();
    let target = world.create_object(if air { "ChoiceAirTarget" } else { "ChoiceGroundTarget" }, Team::GLA, Vec3::new(distance, 0.0, 0.0)).unwrap();
    world.host_object_mut(carrier).unwrap().occupants.push(rider);
    if air {
        world.host_object_mut(target).unwrap().status.airborne_target = true;
    }
    let passenger = world.host_object_mut(rider).unwrap();
    passenger.contained_by = Some(carrier);
    passenger.set_status_disabled_held(true);
    passenger.restore_ai_state(AIState::Docked);
    world.frame = 30;
    (world, rider, target)
}

#[test]
fn authored_passenger_selects_secondary_for_air_and_keeps_worlds_isolated() {
    let (mut owner, rider, target) = passenger_world(true, 120.0, 37);
    let (mut peer, peer_rider, peer_target) = passenger_world(true, 120.0, 23);
    let before = owner.host_object(target).unwrap().health.current;
    owner.try_transport_passenger_residual_fire(rider);
    assert_eq!(owner.host_object(target).unwrap().health.current, before - 37.0);
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, before);
    peer.try_transport_passenger_residual_fire(peer_rider);
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, before - 23.0);
}

#[test]
fn authored_passenger_uses_primary_for_ground_and_cannot_reach_outside_selected_range() {
    for (air, distance, damage) in [(false, 50.0, 11.0), (false, 120.0, 0.0), (true, 160.0, 0.0)] {
        let (mut owner, rider, target) = passenger_world(air, distance, 37);
        let (peer, _, peer_target) = passenger_world(air, distance, 23);
        let before = owner.host_object(target).unwrap().health.current;
        owner.try_transport_passenger_residual_fire(rider);
        assert_eq!(owner.host_object(target).unwrap().health.current, before - damage, "air={air}, distance={distance}");
        assert_eq!(peer.host_object(peer_target).unwrap().health.current, before);
    }
}

#[test]
fn authored_passenger_chooses_nearest_eligible_target_with_its_own_slot() {
    let (mut world, rider, air) = passenger_world(true, 120.0, 37);
    let ground = world.create_object(
        "ChoiceGroundTarget", Team::GLA, Vec3::new(50.0, 0.0, 0.0),
    ).unwrap();
    let air_health = world.host_object(air).unwrap().health.current;
    let ground_health = world.host_object(ground).unwrap().health.current;
    world.try_transport_passenger_residual_fire(rider);
    assert_eq!(world.host_object(ground).unwrap().health.current, ground_health - 11.0);
    assert_eq!(world.host_object(air).unwrap().health.current, air_health);
}
