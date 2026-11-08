//! Initial transport payload owner tests.

use super::*;

#[test]
fn factory_defers_payload_until_first_update_and_empty_carrier_never_respawns() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "factory_defers_payload_until_first_update_and_empty_carrier_never_respawns",
        || {
            let mut world = GameLogic::new();
            let mut template = ThingTemplate::new("ChinaVehicleTroopCrawler");
            template.add_kind_of(KindOf::Vehicle).set_health(240.0);
            world
                .templates
                .insert("ChinaVehicleTroopCrawler".into(), template);
            let requested = Vec3::new(27.0, 39.0, 0.0);
            let id = world
                .create_object("ChinaVehicleTroopCrawler", Team::China, requested)
                .unwrap();
            assert_eq!(world.objects[&id].get_position(), requested);
            assert!(world.objects[&id].occupants.is_empty());
            assert!(!world.objects[&id].initial_transport_payload_created);
            world
                .update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert!(world.objects[&id].occupants.is_empty());
            world.frame = 1;
            world
                .update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert_eq!(world.objects[&id].occupants.len(), 8);
            assert!(world.objects[&id].initial_transport_payload_created);
            let children = world.objects[&id].occupants.clone();
            world.objects.get_mut(&id).unwrap().occupants.clear();
            for child in children {
                world.objects.remove(&child);
            }
            let before = world.objects.registered_len();
            world.frame = 2;
            world
                .update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert!(world.objects[&id].occupants.is_empty());
            assert_eq!(world.objects.registered_len(), before);
        },
    );
}

#[test]
fn same_id_worlds_keep_initial_payload_latch_and_children_isolated() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "same_id_worlds_keep_initial_payload_latch_and_children_isolated",
        || {
            fn world_with_carrier() -> (GameLogic, ObjectId) {
                let mut world = GameLogic::new();
                let mut template = ThingTemplate::new("ChinaVehicleTroopCrawler");
                template.add_kind_of(KindOf::Vehicle).set_health(240.0);
                world
                    .templates
                    .insert("ChinaVehicleTroopCrawler".into(), template);
                let id = world
                    .create_object("ChinaVehicleTroopCrawler", Team::China, Vec3::ZERO)
                    .unwrap();
                (world, id)
            }
            let (mut a, id_a) = world_with_carrier();
            let (mut b, id_b) = world_with_carrier();
            assert_eq!(id_a, id_b, "fixture needs colliding object ids");
            let b_registered = b.objects.registered_len();
            a.frame = 1;
            a.update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert_eq!(a.objects[&id_a].occupants.len(), 8);
            assert!(a.objects[&id_a].initial_transport_payload_created);
            assert!(b.objects[&id_b].occupants.is_empty());
            assert!(!b.objects[&id_b].initial_transport_payload_created);
            assert_eq!(b.objects.registered_len(), b_registered);
            b.frame = 1;
            b.update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert_eq!(b.objects[&id_b].occupants.len(), 8);
            assert_eq!(a.objects[&id_a].occupants.len(), 8);
        },
    );
}

#[test]
fn authored_rider_payload_creates_a_real_rider_on_first_update() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_rider_payload_creates_a_real_rider_on_first_update",
        || {
            let mut world = GameLogic::new();
            world.admit_object_ini_sources([("authored-rider.ini".into(), r#"
Object RenamedRiderVehicle
  KindOf = SELECTABLE VEHICLE
  Locomotor = SET_NORMAL CombatBikeGroundLocomotor CombatBikeCliffLocomotor
  Behavior = RiderChangeContain RiderModule
    Slots = 1
    AllowInsideKindOf = INFANTRY
    InitialPayload = CustomVariant
    Rider2 = CustomRebel RIDER2 WEAPON_RIDER2 STATUS_RIDER2 CustomCommands SET_NORMAL
    ScuttleDelay = 1500
    ScuttleStatus = TOPPLED
  End
End
Object CustomRebel
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
End
ObjectReskin CustomVariant CustomRebel
End
"#.into())]).unwrap();
            world.add_player(Player::new(1, Team::GLA, "PayloadOwner", true));
            let id = world.create_object_for_player_with_effects_checked(
                "RenamedRiderVehicle", 1, Vec3::ZERO, None, &mut Default::default(),
            ).expect("checked bike factory").expect("admitted bike");
            assert_eq!(world.objects[&id].thing().template.contain_module.initial_transport_payload,
                Some(("CustomVariant".to_string(), 1)));
            assert!(world.objects[&id].occupants.is_empty());
            assert!(world.objects[&id].weapon.is_none());
            world.frame = 1;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            let bike = &world.objects[&id];
            assert_eq!(bike.occupants.len(), 1);
            assert_eq!(bike.rider_change_active_slot, Some(2));
            assert_eq!(bike.rider_change_weapon_set.as_deref(), Some("WEAPON_RIDER2"));
            assert!(bike.weapon.is_none(), "an absent authored weapon is not a synthetic MG");
            let rider_id = bike.occupants[0];
            assert_eq!(world.objects[&rider_id].template_name, "CustomVariant");
            assert_eq!(world.objects[&rider_id].contained_by, Some(id));
            assert!(world.objects[&id].initial_transport_payload_created);
            let before = world.objects.registered_len();
            world.frame = 2;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            assert_eq!(world.objects.registered_len(), before);
            assert_eq!(world.objects[&id].occupants, vec![rider_id]);
        },
    );
}

#[test]
fn authored_payload_and_replacement_select_their_real_weapon_sets() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_payload_and_replacement_select_their_real_weapon_sets",
        || {
            use crate::assets::ini_template_loader::{
                parse_weapon_definitions_ini_into, WeaponDefinitionLoadType,
            };
            let mut world = GameLogic::new();
            world.admit_object_ini_sources([("rider-weapons.ini".into(), r#"
Object RenamedArmedCycle
 KindOf = SELECTABLE VEHICLE CAN_ATTACK
 Locomotor = SET_NORMAL CombatBikeGroundLocomotor CombatBikeCliffLocomotor
 WeaponSet
  Conditions = None
  Weapon = PRIMARY NONE
 End
 WeaponSet
  Conditions = WEAPON_RIDER2
  Weapon = PRIMARY CustomBikerGun
 End
 WeaponSet
  Conditions = WEAPON_RIDER3
  Weapon = PRIMARY CustomBikerRocket
 End
 Behavior = RiderChangeContain RiderModule
  Slots = 1
  AllowInsideKindOf = INFANTRY
  InitialPayload = CustomRifleRider 1
  Rider2 = CustomRifleRider RIDER2 WEAPON_RIDER2 STATUS_RIDER2 RifleCommands SET_NORMAL
  Rider3 = CustomRocketRider RIDER3 WEAPON_RIDER3 STATUS_RIDER3 RocketCommands SET_NORMAL
  ScuttleDelay = 1500
  ScuttleStatus = TOPPLED
 End
End
Object CustomRifleRider
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
End
Object CustomRocketRider
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
End
"#.into())]).unwrap();
            let mut weapons = std::collections::HashMap::new();
            parse_weapon_definitions_ini_into(&mut weapons, r#"
Weapon CustomBikerGun
 PrimaryDamage = 13
 AttackRange = 100
 DelayBetweenShots = 500
End
Weapon CustomBikerRocket
 PrimaryDamage = 47
 AttackRange = 150
 DelayBetweenShots = 1000
End
"#, WeaponDefinitionLoadType::Normal);
            world.admit_weapon_content_snapshot(
                weapons.into_values().map(|weapon| (weapon.name.clone(), weapon)).collect(), &[],
            ).unwrap();
            world.add_player(Player::new(1, Team::GLA, "CycleOwner", true));
            let bike = world.create_object_for_player("RenamedArmedCycle", 1, Vec3::ZERO).unwrap();
            assert!(world.objects[&bike].weapon.is_none());
            world.frame = 1;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            let first = world.objects[&bike].occupants[0];
            assert_eq!(world.objects[&bike].weapon.as_ref().unwrap().damage, 13.0);
            let replacement = world.create_object_for_player("CustomRocketRider", 1, Vec3::ZERO).unwrap();
            assert!(world.rider_change_enter_at_arrival_with_effects(
                replacement, bike, &mut Default::default(),
            ).unwrap());
            assert_eq!(world.objects[&bike].occupants, vec![replacement]);
            assert_eq!(world.objects[&first].contained_by, None);
            assert_eq!(world.objects[&replacement].contained_by, Some(bike));
            assert_eq!(world.objects[&bike].weapon.as_ref().unwrap().damage, 47.0);
            world.refresh_combat_cycle_rider_weapon(bike);
            assert_eq!(world.objects[&bike].weapon.as_ref().unwrap().damage, 47.0);
        },
    );
}

#[test]
fn multiple_authored_payloads_preserve_existing_rider_until_replacement() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "multiple_authored_payloads_preserve_existing_rider_until_replacement",
        || {
            let mut world = GameLogic::new();
            world.admit_object_ini_sources([("multi-rider.ini".into(), r#"
Object MultiRiderBike
 KindOf = SELECTABLE VEHICLE
 Locomotor = SET_NORMAL CombatBikeGroundLocomotor CombatBikeCliffLocomotor
 Behavior = AIUpdateInterface BikeBrain
 End
 Behavior = RiderChangeContain RiderModule
  Slots = 1
  AllowInsideKindOf = INFANTRY
  InitialPayload = MultiRiderNew 2
  Rider2 = MultiRiderOld RIDER2 WEAPON_RIDER2 STATUS_RIDER2 OldCommands SET_NORMAL
  Rider3 = MultiRiderNew RIDER3 WEAPON_RIDER3 STATUS_RIDER3 NewCommands SET_NORMAL
 End
End
Object MultiRiderOld
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
 Behavior = AIUpdateInterface RiderBrain
 End
End
Object MultiRiderNew
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
 Behavior = AIUpdateInterface RiderBrain
 End
End
"#.into())]).unwrap();
            world.add_player(Player::new(1, Team::GLA, "PayloadOwner", true));
            let bike = world.create_object_for_player_with_effects_checked(
                "MultiRiderBike", 1, Vec3::ZERO, None, &mut Default::default(),
            ).expect("checked bike factory").expect("admitted bike");
            let old = world.create_object_for_player_with_effects_checked(
                "MultiRiderOld", 1, Vec3::ZERO, None, &mut Default::default(),
            ).expect("checked old rider factory").expect("admitted old rider");
            assert!(world.rider_change_initial_payload_with_effects(
                old, bike, &mut Default::default(),
            ).unwrap());
            world.frame = 1;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            let riders = world.objects[&bike].occupants.clone();
            assert_eq!(riders.len(), 3);
            assert_eq!(riders[0], old);
            assert!(world.objects[&bike].test_weapon_condition(10));
            assert!(world.objects[&bike].test_weapon_condition(11));
            for rider in &riders {
                assert_eq!(world.objects[rider].contained_by, Some(bike));
                assert!(world.objects[rider].is_held_disabled());
            }
            assert!(world.objects[&bike].has_object_status_bit("STATUS_RIDER2"));
            assert!(world.objects[&bike].has_object_status_bit("STATUS_RIDER3"));
            world.objects.get_mut(&bike).unwrap()
                .apply_status_bits_upgrade_masks(&["NO_ATTACK"], &[]);
            let saved_model_mask = world.objects[&bike].rider_change_model_condition_mask;
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            builder.restore_from_snapshot(&snapshot, &mut world).unwrap();
            assert_eq!(world.objects[&bike].occupants, riders);
            assert!(world.objects[&bike].initial_transport_payload_created);
            assert!(world.objects[&bike].has_object_status_bit("STATUS_RIDER2"));
            assert!(world.objects[&bike].has_object_status_bit("STATUS_RIDER3"));
            assert!(world.objects[&bike].has_object_status_bit("NO_ATTACK"));
            assert_eq!(world.objects[&bike].rider_change_model_condition_mask, saved_model_mask);
            assert_eq!(world.objects[&bike].model_condition_bits & saved_model_mask, saved_model_mask);
            assert!(world.objects[&bike].test_weapon_condition(10));
            assert!(world.objects[&bike].test_weapon_condition(11));
            for rider in &riders {
                assert_eq!(world.objects[rider].contained_by, Some(bike));
                assert!(world.objects[rider].is_held_disabled());
            }
            let next = world.create_object("MultiRiderOld", Team::GLA, Vec3::ZERO).unwrap();
            assert!(world.rider_change_enter_at_arrival_with_effects(
                next, bike, &mut Default::default(),
            ).unwrap());
            assert_eq!(world.objects[&bike].occupants, vec![next]);
            for rider in riders {
                assert_eq!(world.objects[&rider].contained_by, None);
                assert!(!world.objects[&rider].is_held_disabled());
            }
            assert!(world.objects[&bike].test_weapon_condition(10));
            assert!(!world.objects[&bike].test_weapon_condition(11));
        },
    );
}

#[test]
fn rejected_initial_payloads_remain_uncontained_and_never_respawn() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "rejected_initial_payloads_remain_uncontained_and_never_respawn",
        || {
            let mut world = GameLogic::new();
            world.admit_object_ini_sources([("rejected-rider.ini".into(), r#"
Object RejectingBike
 KindOf = SELECTABLE VEHICLE
 Locomotor = SET_NORMAL CombatBikeGroundLocomotor CombatBikeCliffLocomotor
 Behavior = RiderChangeContain RiderModule
  Slots = 1
  AllowInsideKindOf = INFANTRY
  InitialPayload = RejectedPayload 2
  Rider2 = LegalPayload RIDER2 WEAPON_RIDER2 STATUS_RIDER2 LegalCommands SET_NORMAL
 End
End
Object LegalPayload
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
End
Object RejectedPayload
 KindOf = SELECTABLE INFANTRY
 TransportSlotCount = 1
End
"#.into())]).unwrap();
            world.add_player(Player::new(1, Team::GLA, "PayloadOwner", true));
            let bike = world.create_object_for_player_with_effects_checked(
                "RejectingBike", 1, Vec3::ZERO, None, &mut Default::default(),
            ).expect("checked bike factory").expect("admitted bike");
            world.frame = 1;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            let rejected: Vec<_> = world.objects.registered_values()
                .filter(|object| object.template_name == "RejectedPayload")
                .map(|object| object.id).collect();
            assert_eq!(rejected.len(), 2);
            assert!(world.objects[&bike].occupants.is_empty());
            assert!(world.objects[&bike].initial_transport_payload_created);
            for id in &rejected {
                assert_eq!(world.objects[id].contained_by, None);
            }
            world.frame = 2;
            world.update_initial_transport_payloads_with_effects(&mut Default::default()).unwrap();
            assert_eq!(world.objects.registered_values()
                .filter(|object| object.template_name == "RejectedPayload").count(), 2);
        },
    );
}
