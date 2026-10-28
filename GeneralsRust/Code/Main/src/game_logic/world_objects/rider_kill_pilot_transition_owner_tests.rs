//! Moving RiderChange KillPilot yields native TransitionDamageFX to its owner
//! before resuming movement, health, source-history, and ArmorDamage callbacks.
use super::*;

fn rider_world(child: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_creation_list_sources([format!(
            "ObjectCreationList OCL_RiderTransitionOwner\nCreateObject\nObjectNames = {child}\nCount = 1\nEnd\nEnd\n"
        )])
        .expect("rider transition OCL admitted to this world");
    let mut rider = ThingTemplate::new("RiderTransitionOwnerVehicle");
    rider
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    rider.contain_module.kind = crate::game_logic::thing::ContainModuleKind::RiderChange;
    rider.set_health(100.0);
    world.templates.insert(rider.name.clone(), rider);
    world
        .templates
        .insert(child.into(), ThingTemplate::new(child));
    let id = world
        .create_object(
            "RiderTransitionOwnerVehicle",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 50.0),
        )
        .expect("RiderChange vehicle");
    let vehicle = world.objects.get_mut(&id).unwrap();
    vehicle.set_status_moving(true);
    vehicle.target = Some(ObjectId(u32::MAX));
    let mut transition =
        crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData::default();
    transition.enabled = true;
    transition.ocl_for_state[3].push("OCL_RiderTransitionOwner".into());
    vehicle.transition_damage_fx = Some(transition);
    (world, id)
}

#[test]
fn moving_rider_kill_pilot_yields_to_the_receiving_world_before_its_tail() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "moving_rider_kill_pilot_yields_to_the_receiving_world_before_its_tail",
        || {
            let (mut first, id) = rider_world("FirstRiderTransitionWitness");
            let (mut second, peer_id) = rider_world("SecondRiderTransitionWitness");
            assert_eq!(id, peer_id);

            for world in [&mut first, &mut second] {
                let result = world
                    .apply_owned_damage(
                        id,
                        10.0,
                        None,
                        crate::game_logic::combat::DamageType::KillPilot,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .expect("owned RiderChange damage");
                assert!(result.destroyed);
            }

            let has = |world: &GameLogic, name: &str| {
                world
                    .objects
                    .registered_values()
                    .any(|object| object.template_name == name)
            };
            assert!(has(&first, "FirstRiderTransitionWitness"));
            assert!(!has(&first, "SecondRiderTransitionWitness"));
            assert!(has(&second, "SecondRiderTransitionWitness"));
            assert!(!has(&second, "FirstRiderTransitionWitness"));
            for world in [&first, &second] {
                let vehicle = world.objects.get(&id).expect("callback kept vehicle installed");
                assert_eq!(vehicle.health.current, 0.0);
                assert!(!vehicle.status.moving);
                assert!(vehicle.target.is_none());
            }
        },
    );
}
