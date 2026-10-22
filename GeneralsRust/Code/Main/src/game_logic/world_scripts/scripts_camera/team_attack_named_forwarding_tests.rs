//! Actual factory containment and physical hive forwarding; CPP AIGroup2102–2174.
use super::named_command_test_support::{execute, world};
use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn roster(owner: &GameLogic, members: &[ObjectId]) {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("ForwardTeam".into(), "".into(), false, None)
        .unwrap();
    let team = factory.create_team("ForwardTeam").unwrap();
    let mut team = team.write().unwrap();
    for member in members {
        team.add_member(member.0);
    }
}
fn container(owner: &mut GameLogic, name: &str, pos: Vec3) -> ObjectId {
    let mut template = ThingTemplate::new(name);
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    template.set_authored_ai_update_interface(Some(false));
    template.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(2),
        is_enclosing_container: true,
        initial_roster_template: "OwnedNamedInfantry".into(),
        initial_roster_count: 2,
        ..Default::default()
    };
    owner.templates.insert(name.into(), template);
    owner.create_object_for_player(name, 1, pos).unwrap()
}
#[test]
fn team_attack_named_forwards_owned_passengers_then_locked_hive_in_near_to_far_order() {
    let (mut owner, _, _) = world();
    let (mut peer, _, _) = world();
    let near = container(&mut owner, "ForwardNear", Vec3::new(60.0, 0.0, 60.0));
    let far = container(&mut owner, "ForwardFar", Vec3::new(20.0, 0.0, 20.0));
    let peer_near = container(&mut peer, "ForwardNear", Vec3::new(60.0, 0.0, 60.0));
    let peer_far = container(&mut peer, "ForwardFar", Vec3::new(20.0, 0.0, 20.0));
    assert_eq!((near, far), (peer_near, peer_far));
    let near_passengers = owner.host_object(near).unwrap().contained_units();
    let far_passengers = owner.host_object(far).unwrap().contained_units();
    assert_eq!(near_passengers.len(), 2);
    assert_eq!(far_passengers.len(), 2);
    // A real native target's targeter list exposes actual admitted order order.
    let mut target_template = ThingTemplate::new("AmericaJetStealthFighter");
    target_template
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    owner
        .templates
        .insert(target_template.name.clone(), target_template);
    let target = owner
        .create_object_for_player("AmericaJetStealthFighter", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    owner.host_object_mut(target).unwrap().name = "ForwardTarget".into();
    owner.host_object_mut(target).unwrap().status.stealthed = false;
    let mut hive_template = ThingTemplate::new("GLAStingerSite");
    hive_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    owner
        .templates
        .insert(hive_template.name.clone(), hive_template);
    let hive = owner
        .create_object_for_player("GLAStingerSite", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(
        owner
            .host_object(hive)
            .unwrap()
            .hive_slaves
            .iter()
            .any(|slave| slave.alive)
    );
    // Reverse insertion order calibrates the original near-to-far pass.
    roster(&owner, &[far, hive, near]);
    roster(&peer, &[peer_far, peer_near]);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamAttackNamed,
                "ForwardTeam",
                "ForwardTarget"
            )]
        ),
        1
    );
    for passenger in near_passengers.iter().chain(&far_passengers) {
        assert_eq!(owner.host_object(*passenger).unwrap().target, Some(target));
        assert_eq!(
            owner.host_object(*passenger).unwrap().last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
        assert_eq!(peer.host_object(*passenger).unwrap().target, None);
    }
    let targeters = &owner.host_object(target).unwrap().jet_ai.targeted_by;
    assert_eq!(
        targeters
            .iter()
            .copied()
            .filter(|id| near_passengers.contains(id) || far_passengers.contains(id))
            .collect::<Vec<_>>(),
        near_passengers
            .iter()
            .chain(&far_passengers)
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(
        owner
            .host_object(hive)
            .unwrap()
            .hive_slaves
            .iter()
            .filter(|slave| slave.alive)
            .all(|slave| slave.ai_attacking && slave.attack_target_id == target.0)
    );
    // Held excludes a whole owner's forwarding before the sort, while subdued
    // garrison disables passenger forwarding without changing its roster.
    owner.host_object_mut(near).unwrap().status.disabled_held = true;
    owner.host_object_mut(far).unwrap().status.disabled_subdued = true;
    // CPP GarrisonContain1691 queries the Object DISABLED_SUBDUED bit.
    assert!(owner.host_object(far).unwrap().is_subdued_disabled());
    for passenger in near_passengers.iter().chain(&far_passengers) {
        owner
            .host_object_mut(*passenger)
            .unwrap()
            .clear_target(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    }
    execute(
        &mut owner,
        &[(
            ScriptActionType::TeamAttackNamed,
            "ForwardTeam",
            "ForwardTarget",
        )],
    );
    for passenger in near_passengers.iter().chain(&far_passengers) {
        assert_eq!(owner.host_object(*passenger).unwrap().target, None);
    }
}
