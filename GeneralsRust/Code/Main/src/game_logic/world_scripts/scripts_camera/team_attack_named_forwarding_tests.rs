//! Actual factory containment and physical hive forwarding; CPP AIGroup2102–2174.
use super::named_command_test_support::{execute, named_infantry, world_with};
use super::*;
use crate::fixture_content::FixtureObject;
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
/// A garrison authored with an InitialRoster of two named infantry
/// (GarrisonContain/OpenContain ContainMax, InitialRoster).
fn garrison(name: &str) -> FixtureObject {
    FixtureObject::new(name).kind_of("STRUCTURE").health(100.0).module(
        "Behavior = GarrisonContain ModuleTag_Contain\n ContainMax = 2\n InitialRoster = OwnedNamedInfantry 2\nEnd",
    )
}
/// Startup content: both garrisons, a jet whose JetAIUpdate keeps a targeter
/// list (nonzero LockonTime, JetAIUpdate.cpp:1667), and a Stinger-style site
/// whose SpawnBehavior slaves are its weapons.
fn world() -> (GameLogic, ObjectId, ObjectId) {
    world_with(
        &[
            garrison("ForwardNear"),
            garrison("ForwardFar"),
            FixtureObject::new("ForwardJet")
                .kind_of("AIRCRAFT")
                .health(100.0)
                .module("Behavior = JetAIUpdate ModuleTag_AI\n LockonTime = 1000\nEnd"),
            FixtureObject::new("ForwardStingerSite")
                .kind_of("STRUCTURE IMMOBILE SPAWNS_ARE_THE_WEAPONS")
                .health(100.0)
                .module("Behavior = DefaultProductionExitUpdate ModuleTag_Exit\nEnd")
                .module(
                    "Behavior = SpawnBehavior ModuleTag_Spawn\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = ForwardStingerSoldier\nEnd",
                ),
            named_infantry("ForwardStingerSoldier"),
        ],
        &[],
        "",
    )
}
fn container(owner: &mut GameLogic, name: &str, pos: Vec3) -> ObjectId {
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
    let target = owner
        .create_object_for_player("ForwardJet", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    owner.host_object_mut(target).unwrap().name = "ForwardTarget".into();
    owner.host_object_mut(target).unwrap().status.stealthed = false;
    let hive = owner
        .create_object_for_player("ForwardStingerSite", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    owner.frame = 1;
    owner
        .update_spawn_behavior_with_effects(&mut Default::default())
        .unwrap();
    let slaves: Vec<ObjectId> = owner
        .host_object(hive)
        .unwrap()
        .spawn_behavior_children()
        .into_iter()
        .map(|(slave, _)| slave)
        .collect();
    assert_eq!(slaves.len(), 1, "the site spawned its slave");
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
    // SpawnBehavior::orderSlavesToAttackTarget force-attacks each m_spawnIDs
    // slave (SpawnBehavior.cpp).
    for slave in &slaves {
        assert_eq!(owner.host_object(*slave).unwrap().target, Some(target));
    }
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
            .clear_target(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
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
