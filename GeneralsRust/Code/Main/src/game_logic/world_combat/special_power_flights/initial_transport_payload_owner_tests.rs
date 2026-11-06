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
