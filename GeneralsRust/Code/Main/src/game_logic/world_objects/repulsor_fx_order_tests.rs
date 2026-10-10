//! CPP ActiveBody.cpp:646-662: onDie, real DamageFX, then live AI policy.
use super::*;

#[test]
fn lethal_damage_observes_repulsor_policy_after_actual_fx_callback() {
    // Native catalogs and compatibility effects are process services. Bound
    // this actual-dispatch fixture without acquiring a shared test mutex.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "lethal_damage_observes_repulsor_policy_after_actual_fx_callback",
        || {
            const FX: &str = "OwnedRepulsorPolicyDamageFX";
            game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
            game_engine::common::ini::ini_damage_fx::get_damage_fx_store_mut()
                .unwrap()
                .add_damage_fx(
                    FX.into(),
                    game_engine::common::ini::ini_damage_fx::DamageFX::new(),
                );
            for initial in [false, true] {
                let mut world = GameLogic::new();
                world.set_enable_repulsors(initial);
                let mut template = ThingTemplate::new("PostFxRepulsableCivilian");
                template
                    .add_kind_of(KindOf::CanBeRepulsed)
                    .add_kind_of(KindOf::Infantry);
                template.armor_sets.push(crate::game_logic::HostArmorSet {
                    conditions: 0,
                    armor: None,
                    damage_fx: Some(FX.into()),
                });
                let id = ObjectId(732);
                let mut civilian = Object::new(template, id, Team::Neutral);
                civilian.health.current = 10.0;
                civilian.health.maximum = 10.0;
                world.objects.insert(id, civilian);
                let _ =
                    crate::game_logic::host_transition_damage_fx::take_dispatched_armor_damage_fx();
                let mut observed = false;
                let result = world.apply_owned_damage_with_killer_team_and_after_fx(
            id, 50.0, None, crate::game_logic::combat::DamageType::Bullet,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal, None,
            &DamageHitContext::default(), None, |world, victim| {
                observed = true;
                let victim = &world.objects[&victim];
                assert!(victim.status.on_die_started, "onDie precedes DamageFX");
                assert_eq!(victim.last_damage_fx_done, Some(crate::game_logic::combat::DamageType::Bullet));
                assert!(!victim.status.repulsor, "repulsor mutation follows the FX callback");
                assert!(crate::game_logic::host_transition_damage_fx::take_dispatched_armor_damage_fx()
                    .contains(&FX.to_string()), "real configured DamageFX dispatch completed");
                world.set_enable_repulsors(!initial);
            },
        ).unwrap();
                assert!(observed);
                assert!(result.destroyed);
                assert_eq!(
                    world.objects[&id].status.repulsor, !initial,
                    "read live policy after FX, not ingress policy"
                );
                assert_eq!(
                    world.objects[&id].repulsor_until_frame,
                    if initial { 0 } else { 60 }
                );
                assert!(
                    world.objects.contains_key(&id),
                    "cleanup, not callbacks, removes the canonical victim"
                );
            }
        },
    );
}

#[test]
fn owned_repulsor_definitions_admit_and_reset_without_touching_another_match() {
    // Native catalogs and compatibility effects are process services. Bound
    // this actual-dispatch fixture without acquiring a shared test mutex.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "owned_repulsor_definitions_admit_and_reset_without_touching_another_match",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let mut enabled = game_engine::common::ini::AIData::default();
            enabled.enable_repulsors = true;
            first.set_ai_definition_base(enabled);
            second.set_ai_definition_base(game_engine::common::ini::AIData::default());
            let draft = first.ai_definitions.map_override_draft();
            {
                let mut draft = draft.write().unwrap();
                draft.push_override();
                draft.get_active_mut().unwrap().enable_repulsors = false;
            }
            first.admit_ai_map_overrides(draft).unwrap();
            assert!(!first.enable_repulsors);
            assert!(!second.enable_repulsors);
            first.reset();
            assert!(
                first.enable_repulsors,
                "reset restores this owner's authored baseline"
            );
            assert!(!second.enable_repulsors);
        },
    );
}
