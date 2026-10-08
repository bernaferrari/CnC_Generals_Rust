//! Driving-world policy regressions through real Main object creation.
use super::*;
#[cfg(test)]
mod ai_decision_owner_tests {
    use super::*;
    use crate::game_logic::{AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    fn created_world(initial_state: AIState) -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("AiDecisionOwnerProbe");
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        logic
            .templates
            .insert("AiDecisionOwnerProbe".to_string(), template);
        let id = logic
            .create_object("AiDecisionOwnerProbe", Team::USA, Vec3::ZERO)
            .expect("real Main host object creation");
        logic
            .host_object_mut(id)
            .expect("created host object")
            .set_ai_state(initial_state);
        (logic, id)
    }

    #[test]
    fn ai_state_command_uses_receiver_policy_when_another_world_published_last() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                let _authority = AuthorityEnvGuard::lock()
                    .set("GENERALS_GAMEWORLD_SHADOW", "1")
                    .couple();
                crate::game_logic::host_ai_decision_log::clear();

                let (mut a, a_id) = created_world(AIState::Idle);
                let (mut b, b_id) = created_world(AIState::Attacking);
                assert_eq!(
                    a_id, b_id,
                    "both real worlds must exercise the same host ID"
                );

                // A is the receiver under test. Publish B's opposite policy afterward.
                a.set_ai_decision_authority(true);
                b.set_ai_decision_authority(false);
                a.set_ai_state_decision_aware_for_ai(a_id, AIState::Patrolling);

                assert_eq!(a.host_object(a_id).unwrap().ai_state, AIState::Patrolling);
                assert_eq!(b.host_object(b_id).unwrap().ai_state, AIState::Attacking);
                let events = crate::game_logic::host_ai_decision_log::drain();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].host_object, a_id);
                assert_eq!(
                    events[0].kind,
                    crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                );
                assert_eq!(
                    events[0].ai_state_ordinal,
                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                        &AIState::Patrolling
                    )
                );
            },
        );
    }

    #[test]
    fn ai_state_command_does_not_borrow_true_policy_from_other_world() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                let _authority = AuthorityEnvGuard::lock()
                    .set("GENERALS_GAMEWORLD_SHADOW", "1")
                    .couple();
                crate::game_logic::host_ai_decision_log::clear();

                let (mut a, a_id) = created_world(AIState::Idle);
                let (mut b, b_id) = created_world(AIState::Attacking);
                assert_eq!(a_id, b_id);

                a.set_ai_decision_authority(false);
                b.set_ai_decision_authority(true); // B publishes last.
                a.set_ai_state_decision_aware_for_ai(a_id, AIState::FacingPosition);

                assert_eq!(
                    a.host_object(a_id).unwrap().ai_state,
                    AIState::FacingPosition
                );
                assert_eq!(b.host_object(b_id).unwrap().ai_state, AIState::Attacking);
                assert!(
                    crate::game_logic::host_ai_decision_log::drain().is_empty(),
                    "receiver A is host-only even though B most recently published true"
                );
            },
        );
    }
}

#[cfg(test)]
mod ai_engagement_owner_tests {
    use super::*;
    use crate::game_logic::{AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    fn create_attack_pair(logic: &mut GameLogic) -> (ObjectId, ObjectId) {
        let mut attacker_template = ThingTemplate::new("AiDecisionAttacker");
        attacker_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        logic
            .templates
            .insert("AiDecisionAttacker".into(), attacker_template);
        let mut victim_template = ThingTemplate::new("AiDecisionVictim");
        victim_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        logic
            .templates
            .insert("AiDecisionVictim".into(), victim_template);

        let attacker = logic
            .create_object("AiDecisionAttacker", Team::USA, Vec3::ZERO)
            .expect("Main host attacker");
        let victim = logic
            .create_object("AiDecisionVictim", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .expect("Main host victim");
        let source = logic.host_object_mut(attacker).expect("attacker");
        source.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        (attacker, victim)
    }

    #[test]
    fn public_ai_engagement_uses_the_receivers_decision_policy_and_rejects_invalid_target() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                let _authority = AuthorityEnvGuard::lock()
                    .set("GENERALS_GAMEWORLD_SHADOW", "1")
                    .couple();
                crate::game_logic::host_ai_decision_log::clear();

                let mut a = GameLogic::new();
                let mut b = GameLogic::new();
                let (a_attacker, a_victim) = create_attack_pair(&mut a);
                let (b_attacker, b_victim) = create_attack_pair(&mut b);
                assert_eq!((a_attacker, a_victim), (b_attacker, b_victim));

                a.set_ai_decision_authority(true);
                b.set_ai_decision_authority(false); // opposite owner publishes last
                assert!(a.apply_engagement_decision_aware_for_ai(a_attacker, a_victim));
                let attacker = a.host_object(a_attacker).expect("A attacker");
                assert_eq!(attacker.target, Some(a_victim));
                assert_eq!(attacker.ai_state, AIState::Attacking);
                assert_eq!(b.host_object(b_attacker).unwrap().target, None);
                let events = crate::game_logic::host_ai_decision_log::drain();
                assert_eq!(
                    events.len(),
                    2,
                    "AttackTarget then SetAIState are both observed"
                );
                assert!(events.iter().all(|event| event.host_object == a_attacker));
                assert_eq!(
                    events[0].kind,
                    crate::game_logic::host_ai_decision_log::AI_DECISION_ATTACK
                );
                assert_eq!(events[0].target_host, a_victim.0);
                assert_eq!(
                    events[1].kind,
                    crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                );
                assert_eq!(events[1].ai_state_ordinal, 2);

                // A same-team target is rejected at the actual public engagement boundary.
                let friendly = a
                    .create_object("AiDecisionVictim", Team::USA, Vec3::new(10.0, 0.0, 0.0))
                    .expect("friendly control");
                crate::game_logic::host_ai_decision_log::clear();
                let before = a.host_object(a_attacker).unwrap().target;
                assert!(!a.apply_engagement_decision_aware_for_ai(a_attacker, friendly));
                assert_eq!(a.host_object(a_attacker).unwrap().target, before);
                assert!(crate::game_logic::host_ai_decision_log::drain().is_empty());

                // Reverse owner settings: B's later publication must not cause A to log.
                a.set_ai_decision_authority(false);
                b.set_ai_decision_authority(true);
                crate::game_logic::host_ai_decision_log::clear();
                assert!(a.apply_engagement_decision_aware_for_ai(a_attacker, a_victim));
                assert_eq!(a.host_object(a_attacker).unwrap().target, Some(a_victim));
                assert!(crate::game_logic::host_ai_decision_log::drain().is_empty());
            },
        );
    }
}

#[cfg(test)]
mod ai_fire_owner_tests {
    use super::*;
    use crate::game_logic::host_ai_decision_log::{
        self, AI_DECISION_ATTACK, AI_DECISION_SET_STATE, AI_DECISION_STOP_ATTACK,
    };
    use crate::game_logic::{
        AIState, AttackFireResult, GameLogic, KindOf, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;

    fn fire_world() -> (GameLogic, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        for name in ["DecisionFireSource", "DecisionFireVictim"] {
            let mut template = ThingTemplate::new(name);
            template
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Attackable)
                .set_health(100.0);
            logic.templates.insert(name.into(), template);
        }
        let source = logic
            .create_object("DecisionFireSource", Team::USA, Vec3::ZERO)
            .unwrap();
        let victim = logic
            .create_object("DecisionFireVictim", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        {
            let unit = logic.host_object_mut(source).unwrap();
            unit.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                reload_time: 0.0,
                last_fire_time: -10.0,
                ammo: Some(4),
                clip_size: 4,
                pre_attack_delay: 1.0,
                can_target_ground: true,
                ..Weapon::default()
            });
            unit.prev_victim_pos = Some(Vec3::new(20.0, 0.0, 0.0));
        }
        (logic, source, victim)
    }

    fn assert_policy_through_windup_discharge_and_stop(selected: bool) {
        let _authority = AuthorityEnvGuard::lock()
            .set("GENERALS_GAMEWORLD_SHADOW", "1")
            .couple();
        let (mut a, source, victim) = fire_world();
        let (mut b, foreign_source, foreign_victim) = fire_world();
        assert_eq!((source, victim), (foreign_source, foreign_victim));
        a.set_ai_decision_authority(selected);
        b.set_ai_decision_authority(!selected);
        host_ai_decision_log::clear();
        assert_eq!(
            a.attack_fire_weapon_update(source, victim, 10.0),
            AttackFireResult::Continue
        );
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.ai_state, AIState::Attacking);
        assert_eq!(unit.target, Some(victim));
        assert_eq!(unit.pre_attack_target, Some(victim));
        assert_eq!(unit.pre_attack_ready_at, 11.0);
        assert_eq!(unit.weapon.as_ref().unwrap().ammo, Some(4));
        assert_eq!(unit.weapon_discharge_marker().sequence, 0);
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            if selected {
                vec![
                    AI_DECISION_ATTACK,
                    AI_DECISION_SET_STATE,
                    AI_DECISION_ATTACK,
                ]
            } else {
                vec![]
            },
            "outer AI state and inner Object wind-up must use A's policy"
        );
        assert!(events.iter().all(|event| event.host_object == source));
        if selected {
            assert_eq!(events[0].target_host, victim.0);
            assert_eq!(events[1].ai_state_ordinal, 2);
            assert_eq!(events[2].target_host, victim.0);
        }
        // An active pre-attack wait neither consumes ammo nor repeats decisions.
        b.set_ai_decision_authority(!selected);
        assert_eq!(
            a.attack_fire_weapon_update(source, victim, 10.5),
            AttackFireResult::Continue
        );
        assert!(host_ai_decision_log::drain().is_empty());
        assert_eq!(
            a.host_object(source).unwrap().weapon.as_ref().unwrap().ammo,
            Some(4)
        );
        b.set_ai_decision_authority(!selected);
        assert_eq!(
            a.attack_fire_weapon_update(source, victim, 11.0),
            AttackFireResult::Success
        );
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.weapon.as_ref().unwrap().ammo, Some(3));
        assert_eq!(unit.weapon_discharge_marker().sequence, 1);
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            if selected {
                vec![AI_DECISION_ATTACK]
            } else {
                vec![]
            }
        );
        if selected {
            assert_eq!(events[0].target_host, victim.0);
        }
        b.set_ai_decision_authority(!selected);
        assert!(a.private_stop(source));
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.ai_state, AIState::Idle);
        assert_eq!(unit.target, None);
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            if selected {
                vec![AI_DECISION_STOP_ATTACK, AI_DECISION_SET_STATE]
            } else {
                vec![]
            }
        );
        if selected {
            assert_eq!(events[1].ai_state_ordinal, 0);
        }
        let foreign = b.host_object(foreign_source).unwrap();
        assert_eq!(foreign.ai_state, AIState::Idle);
        assert_eq!(foreign.target, None);
        assert_eq!(foreign.pre_attack_target, None);
        assert_eq!(foreign.weapon.as_ref().unwrap().ammo, Some(4));
        assert_eq!(foreign.weapon_discharge_marker().sequence, 0);
    }

    #[test]
    fn windup_discharge_and_stop_use_true_receiver_policy_after_false_foreign_publish() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                assert_policy_through_windup_discharge_and_stop(true);
            },
        );
    }

    #[test]
    fn windup_discharge_and_stop_use_false_receiver_policy_after_true_foreign_publish() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                assert_policy_through_windup_discharge_and_stop(false);
            },
        );
    }
}

#[cfg(test)]
mod ai_move_owner_tests {
    use super::*;
    use crate::game_logic::host_ai_decision_log::{
        self, AI_DECISION_MOVE_TO, AI_DECISION_SET_STATE,
    };
    use crate::game_logic::{AICommand, AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    fn moving_world() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("DecisionMoveSource");
        template.add_kind_of(KindOf::Aircraft).set_health(100.0);
        logic
            .templates
            .insert("DecisionMoveSource".into(), template);
        let source = logic
            .create_object("DecisionMoveSource", Team::USA, Vec3::ZERO)
            .unwrap();
        logic.host_object_mut(source).unwrap().locomotor_surfaces =
            crate::game_logic::object::LOCO_SURFACE_AIR;
        (logic, source)
    }

    fn command_policy(selected: bool) {
        let _authority = AuthorityEnvGuard::lock()
            .set("GENERALS_GAMEWORLD_SHADOW", "1")
            .couple();
        let (mut a, source) = moving_world();
        let (mut b, foreign_source) = moving_world();
        assert_eq!(source, foreign_source);
        a.set_ai_decision_authority(selected);
        b.set_ai_decision_authority(!selected);
        host_ai_decision_log::clear();
        let destination = Vec3::new(50.0, 10.0, 20.0);
        // The thin test entry executes the existing production command dispatch
        // and its real path installation, with no replacement command runner.
        a.apply_ai_command_for_test(AICommand::MoveTo {
            object_id: source,
            position: destination,
        });
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.ai_state, AIState::Moving);
        assert_eq!(unit.path_goal_position, Some(destination));
        assert_eq!(unit.movement.path.last(), Some(&destination));
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            if selected {
                vec![AI_DECISION_SET_STATE, AI_DECISION_MOVE_TO]
            } else {
                vec![]
            }
        );
        if selected {
            assert_eq!(events[0].ai_state_ordinal, 1);
            assert_eq!(events[1].destination, Some([50.0, 10.0, 20.0]));
        }
        assert!(events.iter().all(|event| event.host_object == source));
        let foreign = b.host_object(foreign_source).unwrap();
        assert_eq!(foreign.ai_state, AIState::Idle);
        assert!(foreign.movement.path.is_empty());
        assert_eq!(foreign.path_goal_position, None);
    }

    #[test]
    fn move_command_and_installed_state_use_true_receiver_policy() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                command_policy(true);
            },
        );
    }
    #[test]
    fn move_command_and_installed_state_use_false_receiver_policy() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                command_policy(false);
            },
        );
    }
    fn exact_path_policy(selected: bool) {
        let _authority = AuthorityEnvGuard::lock()
            .set("GENERALS_GAMEWORLD_SHADOW", "1")
            .couple();
        let (mut a, source) = moving_world();
        let (mut b, foreign_source) = moving_world();
        assert_eq!(source, foreign_source);
        a.set_ai_decision_authority(selected);
        b.set_ai_decision_authority(!selected);
        host_ai_decision_log::clear();
        let destination = Vec3::new(50.0, 10.0, 20.0);
        assert!(a.assign_unit_path_exact(source, destination, &[destination]));
        assert_eq!(a.host_object(source).unwrap().ai_state, AIState::Moving);
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events
                .iter()
                .map(|event| (event.kind, event.ai_state_ordinal))
                .collect::<Vec<_>>(),
            if selected {
                vec![(AI_DECISION_SET_STATE, 1)]
            } else {
                vec![]
            }
        );
        b.set_ai_decision_authority(!selected);
        assert!(a.private_face_position(source, Vec3::new(-20.0, 0.0, 0.0)));
        assert_eq!(
            a.host_object(source).unwrap().ai_state,
            AIState::FacingPosition
        );
        let events = host_ai_decision_log::drain();
        assert_eq!(events.len(), usize::from(selected));
        if selected {
            assert_eq!(events[0].kind, AI_DECISION_SET_STATE);
            assert_eq!(
                events[0].ai_state_ordinal,
                GameWorldShadow::host_ai_state_ordinal(&AIState::FacingPosition)
            );
        }
        b.set_ai_decision_authority(!selected);
        assert!(a.append_unit_waypoint(source, Vec3::new(70.0, 10.0, 30.0)));
        assert_eq!(a.host_object(source).unwrap().ai_state, AIState::Moving);
        let events = host_ai_decision_log::drain();
        assert_eq!(
            events
                .iter()
                .map(|event| (event.kind, event.ai_state_ordinal))
                .collect::<Vec<_>>(),
            if selected {
                vec![(AI_DECISION_SET_STATE, 1)]
            } else {
                vec![]
            }
        );
        assert_eq!(
            b.host_object(foreign_source).unwrap().ai_state,
            AIState::Idle
        );
        assert!(
            b.host_object(foreign_source)
                .unwrap()
                .movement
                .path
                .is_empty()
        );
    }

    #[test]
    fn exact_path_face_and_waypoint_use_true_receiver_policy() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                exact_path_policy(true);
            },
        );
    }
    #[test]
    fn exact_path_face_and_waypoint_use_false_receiver_policy() {
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::current_gameworld_authority(),
            || {
                exact_path_policy(false);
            },
        );
    }
}

#[test]
fn receiver_decision_policy_requires_enabled_shadow_and_coupled_frame() {
    crate::gameworld_shadow::with_gameworld_authority(
        crate::game_logic::current_gameworld_authority(),
        || {
            for (shadow, coupled) in [("0", true), ("1", false)] {
                let env = AuthorityEnvGuard::lock().set("GENERALS_GAMEWORLD_SHADOW", shadow);
                let _env = if coupled { env.couple() } else { env };
                let mut logic = GameLogic::new();
                logic.set_ai_decision_authority(true);
                assert!(!logic.ai_decision_authority_live());
            }
            let body = rust_fn_body(GAME_LOGIC_HOST_SRC, "ai_decision_authority_live").unwrap();
            assert!(body.contains("self.gameworld_authority.ai_decision"));
            assert!(!body.contains("current_gameworld_authority"));
        },
    );
}
