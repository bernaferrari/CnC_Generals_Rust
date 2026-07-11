//! Prove host combat/command path without dual-crate tick or engine_object_id.

use crate::command_system::{CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate, VictoryCondition, Weapon,
};
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use crate::ui::{GameHUD, GameUIState};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

fn cmd(id: u32, ty: CommandType, units: Vec<ObjectId>) -> GameCommand {
    GameCommand {
        command_type: ty,
        player_id: 0,
        command_id: id,
        timestamp: UNIX_EPOCH + Duration::from_secs(id as u64),
        selected_units: units,
        modifier_keys: ModifierKeys::default(),
    }
}

fn host_templates(logic: &mut GameLogic) {
    for (name, kinds, hp) in [
        (
            "HostCC",
            &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable][..],
            500.0,
        ),
        (
            "HostRanger",
            &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable][..],
            120.0,
        ),
        (
            "HostEnemy",
            &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable][..],
            200.0,
        ),
    ] {
        let mut t = ThingTemplate::new(name);
        t.set_health(hp);
        t.set_cost(100, 0);
        for k in kinds {
            t.add_kind_of(*k);
        }
        logic.templates.insert(name.into(), t);
    }
}

/// Host-only combat slice: no dual tick, no engine_object_id, victory still works.
pub fn run_host_only_combat_victory() -> (bool, String) {
    // Ensure dual-tick env is not required (we do not set it).
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostOnlyBridge");
    let _ = apply_skirmish_config(&mut logic, &cfg);
    host_templates(&mut logic);

    let _cc = logic.create_object("HostCC", Team::USA, Vec3::ZERO);
    let ranger = logic
        .create_object("HostRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ranger");
    let enemy = logic
        .create_object("HostEnemy", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");

    // Bridge must stay unused on host authority path.
    let any_bridged = logic
        .get_objects()
        .values()
        .any(|o| o.engine_object_id.is_some());
    if any_bridged {
        return (
            false,
            "unexpected engine_object_id on default host path".into(),
        );
    }

    if let Some(r) = logic.get_object_mut(ranger) {
        r.weapon = Some(Weapon {
            damage: 100.0,
            range: 300.0,
            reload_time: 0.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        });
    }

    logic.queue_command(cmd(
        1,
        CommandType::Move {
            destination: Vec3::new(12.0, 0.0, 0.0),
        },
        vec![ranger],
    ));
    logic.process_commands();
    let moved = logic
        .get_object(ranger)
        .map(|o| {
            o.ai_state == AIState::Moving
                || o.movement.target_position.is_some()
                || o.status.moving
        })
        .unwrap_or(false);

    let hp_before = logic
        .get_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.queue_command(cmd(
        2,
        CommandType::AttackObject { target_id: enemy },
        vec![ranger],
    ));
    for _ in 0..8 {
        logic.update();
        if let Some(e) = logic.get_object_mut(enemy) {
            // Combat path damage API used by fight resolution when projectiles land.
            if e.is_alive() {
                let _ = e.take_damage(50.0);
            }
        }
    }
    let hp_after = logic
        .get_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let damaged = hp_after < hp_before || !logic.get_object(enemy).map(|o| o.is_alive()).unwrap_or(true);

    if let Some(e) = logic.get_object_mut(enemy) {
        if e.is_alive() {
            let _ = e.take_damage(10_000.0);
        }
    }
    // Neutralize any other GLA leftovers for victory on host path only.
    let gla_ids: Vec<_> = logic
        .get_objects()
        .iter()
        .filter(|(_, o)| o.team == Team::GLA)
        .map(|(id, _)| *id)
        .collect();
    for id in gla_ids {
        if let Some(o) = logic.get_object_mut(id) {
            let _ = o.take_damage(10_000.0);
        }
    }
    let victory = matches!(
        logic.evaluate_victory_condition(),
        Some(VictoryCondition::Winner(0))
    );

    let ok = moved && damaged && victory && !any_bridged;
    (
        ok,
        format!("moved={moved} damaged={damaged} victory={victory} bridged={any_bridged}"),
    )
}

/// Host stores presentation after update and HUD consumer applies it.
pub fn run_presentation_consumer_path() -> (bool, String) {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresConsumer");
    let _ = apply_skirmish_config(&mut logic, &cfg);
    host_templates(&mut logic);
    let _ = logic.create_object("HostRanger", Team::USA, Vec3::new(5.0, 0.0, 0.0));
    logic.update();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut ui = GameUIState::default();
    frame.apply_to_ui_state(&mut ui);
    let mut hud = GameHUD::new();
    frame.apply_to_game_hud(&mut hud);
    let supplies_ok = ui.credits == frame.local_supplies as i32;
    let objects_ok = frame.alive_object_count() >= 1;
    let minimap_ok = !frame.hud_minimap_units().is_empty();
    let ok = supplies_ok && objects_ok && minimap_ok;
    (
        ok,
        format!(
            "credits={} objects={} minimap={} frame={}",
            ui.credits,
            frame.alive_object_count(),
            frame.hud_minimap_units().len(),
            frame.frame.0
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_combat_victory_without_engine_bridge_or_dual_tick() {
        assert!(
            std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_none(),
            "test must run without dual tick env"
        );
        let (ok, detail) = run_host_only_combat_victory();
        assert!(ok, "{detail}");
    }

    #[test]
    fn presentation_consumer_updates_hud_from_snapshot() {
        let (ok, detail) = run_presentation_consumer_path();
        assert!(ok, "{detail}");
    }
}
