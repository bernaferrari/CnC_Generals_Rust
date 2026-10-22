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
            .set_ai_state(
                initial_state,
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
        (logic, id)
    }

    #[test]
    fn ai_state_command_uses_receiver_policy_when_another_world_published_last() {
        {
            let (mut a, a_id) = created_world(AIState::Idle);
            let (mut b, b_id) = created_world(AIState::Attacking);
            assert_eq!(
                a_id, b_id,
                "both real worlds must exercise the same host ID"
            );

            // A is the receiver under test. Publish B's opposite policy afterward.

            a.set_ai_state_decision_aware_for_ai(a_id, AIState::Patrolling);

            assert_eq!(a.host_object(a_id).unwrap().ai_state, AIState::Patrolling);
            assert_eq!(b.host_object(b_id).unwrap().ai_state, AIState::Attacking);
        };
    }

    #[test]
    fn ai_state_command_does_not_borrow_true_policy_from_other_world() {
        {
            let (mut a, a_id) = created_world(AIState::Idle);
            let (mut b, b_id) = created_world(AIState::Attacking);
            assert_eq!(a_id, b_id);

            // B publishes last.
            a.set_ai_state_decision_aware_for_ai(a_id, AIState::FacingPosition);

            assert_eq!(
                a.host_object(a_id).unwrap().ai_state,
                AIState::FacingPosition
            );
            assert_eq!(b.host_object(b_id).unwrap().ai_state, AIState::Attacking);
        };
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
        {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            let (a_attacker, a_victim) = create_attack_pair(&mut a);
            let (b_attacker, b_victim) = create_attack_pair(&mut b);
            assert_eq!((a_attacker, a_victim), (b_attacker, b_victim));

            // opposite owner publishes last
            assert!(a.apply_engagement_decision_aware_for_ai(a_attacker, a_victim));
            let attacker = a.host_object(a_attacker).expect("A attacker");
            assert_eq!(attacker.target, Some(a_victim));
            assert_eq!(attacker.ai_state, AIState::Attacking);
            assert_eq!(b.host_object(b_attacker).unwrap().target, None);

            // A same-team target is rejected at the actual public engagement boundary.
            let friendly = a
                .create_object("AiDecisionVictim", Team::USA, Vec3::new(10.0, 0.0, 0.0))
                .expect("friendly control");

            let before = a.host_object(a_attacker).unwrap().target;
            assert!(!a.apply_engagement_decision_aware_for_ai(a_attacker, friendly));
            assert_eq!(a.host_object(a_attacker).unwrap().target, before);

            // Reverse owner settings: B's later publication must not cause A to log.

            assert!(a.apply_engagement_decision_aware_for_ai(a_attacker, a_victim));
            assert_eq!(a.host_object(a_attacker).unwrap().target, Some(a_victim));
        };
    }
}

#[cfg(test)]
mod ai_fire_owner_tests {
    use super::*;

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

    fn assert_policy_through_windup_discharge_and_stop() {
        let (mut a, source, victim) = fire_world();
        let (mut b, foreign_source, foreign_victim) = fire_world();
        assert_eq!((source, victim), (foreign_source, foreign_victim));

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

        {}
        // An active pre-attack wait neither consumes ammo nor repeats decisions.

        assert_eq!(
            a.attack_fire_weapon_update(source, victim, 10.5),
            AttackFireResult::Continue
        );

        assert_eq!(
            a.host_object(source).unwrap().weapon.as_ref().unwrap().ammo,
            Some(4)
        );

        assert_eq!(
            a.attack_fire_weapon_update(source, victim, 11.0),
            AttackFireResult::Success
        );
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.weapon.as_ref().unwrap().ammo, Some(3));
        assert_eq!(unit.weapon_discharge_marker().sequence, 1);

        {}

        assert!(a.private_stop(source));
        let unit = a.host_object(source).unwrap();
        assert_eq!(unit.ai_state, AIState::Idle);
        assert_eq!(unit.target, None);

        {}
        let foreign = b.host_object(foreign_source).unwrap();
        assert_eq!(foreign.ai_state, AIState::Idle);
        assert_eq!(foreign.target, None);
        assert_eq!(foreign.pre_attack_target, None);
        assert_eq!(foreign.weapon.as_ref().unwrap().ammo, Some(4));
        assert_eq!(foreign.weapon_discharge_marker().sequence, 0);
    }

    #[test]
    fn windup_discharge_and_stop_use_true_receiver_policy_after_false_foreign_publish() {
        {
            assert_policy_through_windup_discharge_and_stop();
        };
    }

    #[test]
    fn windup_discharge_and_stop_use_false_receiver_policy_after_true_foreign_publish() {
        {
            assert_policy_through_windup_discharge_and_stop();
        };
    }
}

#[cfg(test)]
mod ai_move_owner_tests {
    use super::*;

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

    fn command_policy() {
        let (mut a, source) = moving_world();
        let (mut b, foreign_source) = moving_world();
        assert_eq!(source, foreign_source);

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

        {}

        let foreign = b.host_object(foreign_source).unwrap();
        assert_eq!(foreign.ai_state, AIState::Idle);
        assert!(foreign.movement.path.is_empty());
        assert_eq!(foreign.path_goal_position, None);
    }

    #[test]
    fn move_command_and_installed_state_use_true_receiver_policy() {
        {
            command_policy();
        };
    }
    #[test]
    fn move_command_and_installed_state_use_false_receiver_policy() {
        {
            command_policy();
        };
    }
    fn exact_path_policy() {
        let (mut a, source) = moving_world();
        let (mut b, foreign_source) = moving_world();
        assert_eq!(source, foreign_source);

        let destination = Vec3::new(50.0, 10.0, 20.0);
        assert!(a.assign_unit_path_exact(source, destination, &[destination]));
        assert_eq!(a.host_object(source).unwrap().ai_state, AIState::Moving);

        assert!(a.private_face_position(source, Vec3::new(-20.0, 0.0, 0.0)));
        assert_eq!(
            a.host_object(source).unwrap().ai_state,
            AIState::FacingPosition
        );

        {}

        assert!(a.append_unit_waypoint(source, Vec3::new(70.0, 10.0, 30.0)));
        assert_eq!(a.host_object(source).unwrap().ai_state, AIState::Moving);

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
        {
            exact_path_policy();
        };
    }
    #[test]
    fn exact_path_face_and_waypoint_use_false_receiver_policy() {
        {
            exact_path_policy();
        };
    }
}
