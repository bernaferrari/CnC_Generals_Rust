//! Attack-move orders are observed on the production host object.
use super::*;

#[test]
fn launch_attack_uses_assign_unit_path_surface() {
    let src = include_str!("combat.rs");
    // Do not split on cfg(test) — nested test modules can appear earlier.
    let i = src
        .find("fn attack_move_units(")
        .expect("attack_move_units");
    let w = &src[i..i + 4500.min(src.len() - i)];
    assert!(
        w.contains("assign_unit_path")
            && (w.contains("AIState::AttackMoving") || w.contains("record_set_state")),
        "AI launch_attack must pathfind then restore AttackMoving (host or decision log)"
    );
    // Fallback may call move_to after assign_unit_path fails; primary path
    // must call assign_unit_path first.
    let path_i = w.find("assign_unit_path").expect("path");
    let move_i = w.find("move_to(enemy_base)");
    assert!(
        move_i.is_none() || move_i.unwrap() > path_i,
        "move_to fallback must come after assign_unit_path"
    );
}

#[test]
fn launch_attack_installs_owned_attack_move_state_and_path() {
    // C++ AIAttackMoveToState / AIInternalMoveToState::onEnter (AIStates.cpp).
    // Observe the path request and attack-move state on the live host object.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Opt into AI decision authority via GameLogic::set_ai_decision_authority(true)
    // (retired GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY env).
    crate::gameworld_shadow::begin_shadow_coupled_tick();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiSm");
    logic.set_ai_decision_authority(true);
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for (name, team, x) in [("AiSmU", Team::USA, 0.0f32), ("AiSmE", Team::GLA, 80.0)] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(100.0);
            if team == Team::GLA {
                tmpl.add_kind_of(KindOf::Structure);
                tmpl.add_kind_of(KindOf::CommandCenter);
            } else {
                tmpl.add_kind_of(KindOf::Infantry);
            }
            tmpl.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), tmpl);
        }
        let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
    }
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
    let usa_unit = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.team == Team::USA && o.is_alive())
        .map(|(id, _)| *id)
        .expect("usa unit");
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            ..Weapon::default()
        });
    }

    let destination = ai.find_enemy_base_center(&logic, gla_id.expect("GLA player"));
    assert_eq!(destination, glam::Vec3::new(80.0, 0.0, 0.0));
    ai.launch_attack(&mut logic, 1000.0);

    let unit = logic.host_object(usa_unit).expect("live attacking unit");
    assert_eq!(unit.ai_state, AIState::AttackMoving);
    assert_eq!(unit.requested_destination, Some(destination));
    assert!(unit.is_attack_path);
    assert!(unit.status.moving);
    assert!(
        unit.waiting_for_path || !unit.movement.path.is_empty(),
        "attack-move must request or install a path on the live unit"
    );

    crate::gameworld_shadow::end_shadow_coupled_tick();
}
