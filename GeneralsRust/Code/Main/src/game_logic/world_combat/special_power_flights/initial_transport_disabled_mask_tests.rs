//! Initial transport disabled-mask tests.

use super::*;
fn carrier() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("ChinaVehicleTroopCrawler");
    template.add_kind_of(KindOf::Vehicle).set_health(240.0);
    world
        .templates
        .insert("ChinaVehicleTroopCrawler".into(), template);
    let id = world
        .create_object("ChinaVehicleTroopCrawler", Team::China, Vec3::ZERO)
        .unwrap();
    world.frame = 1;
    (world, id)
}
#[test]
fn source_transport_none_mask_defers_every_disabled_bit_then_runs_when_cleared() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "source_transport_none_mask_defers_every_disabled_bit_then_runs_when_cleared",
        || {
            let cases: &[fn(&mut Object, bool)] = &[
                |o, v| o.status.disabled_default = v,
                |o, v| o.status.disabled_unmanned = v,
                |o, v| o.status.disabled_underpowered = v,
                |o, v| o.status.disabled_hacked = v,
                |o, v| o.status.disabled_emp = v,
                |o, v| o.status.disabled_paralyzed = v,
                |o, v| o.status.disabled_subdued = v,
                |o, v| o.status.disabled_freefall = v,
                |o, v| o.status.disabled_script_disabled = v,
                |o, v| o.status.disabled_script_underpowered = v,
                |o, v| o.status.disabled_held = v,
                |o, v| o.status.disabled_awestruck = v,
                |o, v| o.status.disabled_brainwashed = v,
            ];
            for set in cases {
                let (mut world, id) = carrier();
                set(world.objects.get_mut(&id).unwrap(), true);
                world
                    .update_initial_transport_payloads_with_effects(&mut Default::default())
                    .unwrap();
                assert!(!world.objects[&id].initial_transport_payload_created);
                assert!(world.objects[&id].occupants.is_empty());
                set(world.objects.get_mut(&id).unwrap(), false);
                world.frame = 2;
                world
                    .update_initial_transport_payloads_with_effects(&mut Default::default())
                    .unwrap();
                assert!(world.objects[&id].initial_transport_payload_created);
                assert_eq!(world.objects[&id].occupants.len(), 8);
            }
        },
    );
}
#[test]
fn dying_still_due_transport_is_not_silently_filtered_by_generic_alive_check() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "dying_still_due_transport_is_not_silently_filtered_by_generic_alive_check",
        || {
            let (mut world, id) = carrier();
            world.objects.get_mut(&id).unwrap().status.effectively_dead = true;
            assert!(!world.objects[&id].is_alive());
            assert!(!world.objects[&id].has_disabled_type());
            world
                .update_initial_transport_payloads_with_effects(&mut Default::default())
                .unwrap();
            assert!(world.objects[&id].initial_transport_payload_created);
        },
    );
}
