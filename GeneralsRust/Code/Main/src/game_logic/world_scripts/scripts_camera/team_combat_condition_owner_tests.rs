use super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::object::{ObjectShroudStatus, ObjectStatusMaskType};
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
};

const ROSTER: &str = "CombatRoster";
const MISSING_SIDE_MASK: u32 = 0xffff_0000;

fn authored(kind: ConditionType, parameters: &[Parameter]) -> Condition {
    let mut condition = Condition::new(kind);
    for parameter in parameters {
        condition.add_parameter(parameter.clone()).unwrap();
    }
    condition
}

fn team_condition(kind: ConditionType, name: &str, operand: Parameter) -> Condition {
    authored(
        kind,
        &[
            Parameter::with_string(ParameterType::Team, name.into()),
            operand,
        ],
    )
}

fn side(name: &str) -> Parameter {
    Parameter::with_string(ParameterType::Side, name.into())
}

fn script(condition: Condition) -> Script {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    Script {
        condition: Some(Box::new(branch)),
        ..Script::new()
    }
}

fn mask(script: &Script, index: usize) -> u32 {
    script
        .condition
        .as_ref()
        .unwrap()
        .first_and
        .as_ref()
        .unwrap()
        .get_parameter(index)
        .unwrap()
        .get_int() as u32
}

fn admit_roster(session: &GameLogic, name: &str, members: &[ObjectId]) -> u32 {
    let mut factory = session.team_factory.lock().unwrap();
    factory
        .init_team_owned(name.into(), "NamedVictim".into(), false, None, Some(2))
        .unwrap();
    let team = factory.create_team(name).unwrap();
    let mut team = team.write().unwrap();
    for member in members.iter().rev() {
        team.add_member(member.0);
    }
    team.get_id()
}

fn hit(session: &mut GameLogic, victim: ObjectId, attacker: ObjectId) {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::object::DamageHitContext;

    let context = DamageHitContext::new(session.host_object(attacker), None, DamageType::Bullet);
    session
        .apply_owned_damage(
            victim,
            10.0,
            Some(attacker),
            DamageType::Bullet,
            HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
}

#[test]
fn team_attacks_use_recorded_type_and_receiving_live_attacker() {
    std::thread::spawn(|| {
        let (mut first, attacker, victim) = world();
        let (mut peer, foreign_attacker, foreign_victim) = world();
        assert_eq!((attacker, victim), (foreign_attacker, foreign_victim));
        assert_eq!(
            admit_roster(&first, ROSTER, &[victim]),
            admit_roster(&peer, ROSTER, &[foreign_victim])
        );
        let attacked = |kind, operand| team_condition(kind, ROSTER, operand);
        let by_type = attacked(
            ConditionType::TeamAttackedByObjecttype,
            Parameter::with_string(ParameterType::ObjectType, "OwnedNamedInfantry".into()),
        );
        let by_player = attacked(ConditionType::TeamAttackedByPlayer, side("NamedOwner"));

        first.inject_host_script_query_snapshot(); // The projection predates the hit.
        hit(&mut first, victim, attacker);
        assert!(
            first.owned_script_condition_for_test(by_type.clone()),
            "the real hit is visible in this script walk"
        );
        assert!(first.owned_script_condition_for_test(by_player.clone()));
        assert!(!peer.owned_script_condition_for_test(by_type.clone()));
        assert!(!peer.owned_script_condition_for_test(by_player.clone()));

        // DamageInfo stores the original template. Player comparison uses the
        // controller at condition evaluation, even if it changed after impact.
        first.host_object_mut(attacker).unwrap().owner_player_id = Some(2);
        let by_new_player = attacked(ConditionType::TeamAttackedByPlayer, side("NamedVictim"));
        assert!(!first.owned_script_condition_for_test(by_player));
        assert!(first.owned_script_condition_for_test(by_new_player.clone()));
        first.objects.remove(&attacker);
        assert!(first.owned_script_condition_for_test(by_type));
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let mut by_new_player = script(by_new_player);
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut by_new_player, None));
        assert_eq!(
            mask(&by_new_player, 1),
            0,
            "no live attacker leaves SIDE untouched"
        );
    })
    .join()
    .unwrap();
}

#[test]
fn team_discovery_and_status_use_live_ordered_membership() {
    std::thread::spawn(|| {
        let (mut first, _, member) = world();
        let (mut peer, _, peer_member) = world();
        let id = admit_roster(&first, ROSTER, &[member]);
        assert_eq!(id, admit_roster(&peer, ROSTER, &[peer_member]));
        let retired_empty = admit_roster(&first, "EmptyRoster", &[]);
        let discover = team_condition(ConditionType::TeamDiscovered, ROSTER, side("NamedOwner"));
        for (world, member, visibility) in [
            (&mut first, member, ObjectShroudStatus::PartialClear),
            (&mut peer, peer_member, ObjectShroudStatus::Fogged),
        ] {
            world
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .set_host_object_shroud_status(1, member.0, visibility);
        }
        assert!(
            first.owned_script_condition_for_test(discover.clone()),
            "partial clear counts on the receiving roster"
        );
        assert!(!peer.owned_script_condition_for_test(discover.clone()));
        first.host_object_mut(member).unwrap().status.disabled_held = true;
        assert!(!first.owned_script_condition_for_test(discover));

        let mut status = Parameter::new(ParameterType::ObjectStatus);
        status.initialized = true;
        status.object_status = ObjectStatusMaskType::UNSELECTABLE | ObjectStatusMaskType::WET;
        let all = team_condition(
            ConditionType::TeamAllHasObjectStatus,
            ROSTER,
            status.clone(),
        );
        let some = team_condition(
            ConditionType::TeamSomeHaveObjectStatus,
            ROSTER,
            status.clone(),
        );
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let evaluate = |world: &mut GameLogic, condition| {
            world.evaluate_owned_script_conditions(&engine, &mut script(condition), None)
        };
        assert!(
            first
                .team_factory
                .lock()
                .unwrap()
                .find_team_by_id(retired_empty)
                .is_none()
        );
        // Query the just-created empty team before the next team-state phase
        // retires empty non-singletons.
        admit_roster(&first, "JustCreatedEmptyRoster", &[]);
        assert!(evaluate(
            &mut first,
            team_condition(
                ConditionType::TeamAllHasObjectStatus,
                "JustCreatedEmptyRoster",
                status.clone()
            )
        ));
        assert!(!evaluate(
            &mut first,
            team_condition(
                ConditionType::TeamAllHasObjectStatus,
                "MissingRoster",
                status
            )
        ));
        first.host_object_mut(member).unwrap().status.unselectable = true;
        assert!(evaluate(&mut first, all.clone()));
        assert!(evaluate(&mut first, some.clone()));
        assert!(!evaluate(&mut peer, some.clone()));
        let team = first
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(id)
            .unwrap();
        team.write().unwrap().add_member(u32::MAX);
        assert!(
            !evaluate(&mut first, some.clone()),
            "missing head stops the search"
        );
        {
            let mut team = team.write().unwrap();
            team.remove_member(member.0);
            team.add_member(member.0);
        }
        assert!(
            evaluate(&mut first, some),
            "the matching head short-circuits"
        );
        assert!(!evaluate(&mut first, all));
    })
    .join()
    .unwrap();
}

#[test]
fn team_and_player_combat_conditions_write_side_caches_in_cpp_order() {
    std::thread::spawn(|| {
        let (mut first, _, member) = world();
        let (mut peer, _, _) = world();
        admit_roster(&first, ROSTER, &[member]);
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let mut owned = script(team_condition(
            ConditionType::TeamOwnedByPlayer,
            "MissingRoster",
            side("NamedVictim"),
        ));
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut owned, None));
        assert_eq!(
            mask(&owned, 1),
            1 << 2,
            "team ownership resolves SIDE before team"
        );
        let mut discovered = script(team_condition(
            ConditionType::TeamDiscovered,
            "MissingRoster",
            side("NamedVictim"),
        ));
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut discovered, None));
        assert_eq!(mask(&discovered, 1), 0);
        let mut owned = script(team_condition(
            ConditionType::TeamOwnedByPlayer,
            ROSTER,
            side("NamedVictim"),
        ));
        assert!(first.evaluate_owned_script_conditions(&engine, &mut owned, None));
        assert!(!peer.evaluate_owned_script_conditions(&engine, &mut owned, None));

        let frame = first.frame;
        first
            .players
            .get_mut(&2)
            .unwrap()
            .set_attacked_by_at(1, frame);
        let attacked = authored(
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer,
            &[side("NamedVictim"), side("NamedOwner")],
        );
        assert!(first.owned_script_condition_for_test(attacked.clone()));
        assert!(!peer.owned_script_condition_for_test(attacked));
        let mut missing = script(authored(
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer,
            &[side("Missing"), side("NamedOwner")],
        ));
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut missing, None));
        assert_eq!(
            (mask(&missing, 0), mask(&missing, 1)),
            (MISSING_SIDE_MASK, 0)
        );

        let mut unfinished = script(authored(
            ConditionType::PlayerDestroyedNBuildingsPlayer,
            &[
                side("Missing"),
                Parameter::with_int(ParameterType::Int, 5),
                side("NamedOwner"),
            ],
        ));
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut unfinished, None));
        assert_eq!(
            (mask(&unfinished, 0), mask(&unfinished, 2)),
            (MISSING_SIDE_MASK, 1 << 1)
        );
    })
    .join()
    .unwrap();
}
