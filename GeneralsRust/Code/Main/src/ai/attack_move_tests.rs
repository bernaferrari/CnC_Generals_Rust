//! Attack-move orders are observed on the production host object.
use super::*;

#[test]
fn launch_attack_installs_owned_attack_move_state_and_path() {
    // C++ AIAttackMoveToState / AIInternalMoveToState::onEnter (AIStates.cpp).
    // Observe the path request and attack-move state on the live host object.
    use crate::game_logic::{GameLogic, Team};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Opt into AI decision authority via GameLogic::set_ai_decision_authority(true)
    // (retired GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY env).

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiSm");

    // Authored before NewGame: an armed walking USA infantry and the GLA
    // command center that anchors the enemy base center.
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("AiSmU")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("AiSmGun"),
            crate::fixture_content::FixtureObject::new("AiSmE")
                .kind_of("STRUCTURE COMMANDCENTER")
                .health(100.0),
        ],
        &[crate::fixture_content::FixtureWeapon::new("AiSmGun").damage(10.0).range(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let usa_unit = logic
        .create_object("AiSmU", Team::USA, glam::Vec3::ZERO)
        .expect("usa unit");
    logic
        .create_object("AiSmE", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("gla command center");
    let usa_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::USA)
        .map(|(id, _)| *id)
        .unwrap_or(0);
    let gla_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::GLA)
        .map(|(id, _)| *id);
    let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = gla_id;
    ai.is_active = true;

    let destination = ai.find_enemy_base_center(&logic, gla_id.expect("GLA player"));
    assert_eq!(destination, glam::Vec3::new(80.0, 0.0, 0.0));
    ai.launch_attack(&mut logic, 1000.0);

    let unit = logic.host_object(usa_unit).expect("live attacking unit");
    assert_eq!(unit.ai_state(), AIState::AttackMoving);
    assert_eq!(unit.requested_destination, Some(destination));
    assert!(unit.is_attack_path);
    assert!(unit.status.moving);
    assert!(
        unit.waiting_for_path || !unit.movement.path.is_empty(),
        "attack-move must request or install a path on the live unit"
    );
}
