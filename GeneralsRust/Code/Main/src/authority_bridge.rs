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

/// Host-only combat slice: no dual tick, no engine_object_id, victory via real combat.
/// No take_damage fallbacks, no HP caps after spawn, no re-teaming.
pub fn run_host_only_combat_victory() -> (bool, String) {
    // Ensure dual-tick env is not required (we do not set it).
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostOnlyBridge");
    let _ = apply_skirmish_config(&mut logic, &cfg);
    // Structure-scale enemy HP (200); default Weapon combat must finish via update_combat.
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
    logic.set_ai_active(1, false);

    let _cc = logic.create_object("HostCC", Team::USA, Vec3::ZERO);
    let ranger = logic
        .create_object("HostRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ranger");
    // Within default Weapon range (100).
    let enemy = logic
        .create_object("HostEnemy", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
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

    // Default infantry weapon from create_object — only ensure present, no custom DPS.
    // engine_object_id stays None from create_object on the default host path; no
    // mid-scenario force-clear residual (matches golden combat policy).
    if let Some(r) = logic.get_object_mut(ranger) {
        if r.weapon.is_none() {
            r.weapon = Some(Weapon::default());
        }
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
            o.ai_state == AIState::Moving || o.movement.target_position.is_some() || o.status.moving
        })
        .unwrap_or(false);

    let hp_before = logic
        .get_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // 200 HP / 25 dmg ≈ 8 shots at 1s reload → need ~240+ logic frames.
    let mut combat_killed = false;
    for round in 0..400u32 {
        logic.queue_command(cmd(
            2 + round,
            CommandType::AttackObject { target_id: enemy },
            vec![ranger],
        ));
        logic.update();
        let still_alive = logic
            .get_object(enemy)
            .map(|o| o.is_alive())
            .unwrap_or(false);
        if !still_alive {
            combat_killed = true;
            break;
        }
    }
    let hp_after = logic
        .get_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let damaged = hp_after < hp_before || combat_killed;

    // Victory only if combat removed the sole enemy CC (no force-destroy of leftovers).
    let victory = combat_killed
        && matches!(
            logic.evaluate_victory_condition(),
            Some(VictoryCondition::Winner(0))
        );

    let ok = moved && damaged && victory && !any_bridged;
    (
        ok,
        format!(
            "moved={moved} damaged={damaged} killed={combat_killed} victory={victory} bridged={any_bridged}"
        ),
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

    #[test]
    fn honest_weapon_combat_kills_low_hp_structure() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HonestCombat");
        let _ = apply_skirmish_config(&mut logic, &cfg);
        let mut ranger_t = ThingTemplate::new("R");
        ranger_t.set_health(100.0);
        ranger_t.add_kind_of(KindOf::Infantry);
        ranger_t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("R".into(), ranger_t);
        let mut enemy_t = ThingTemplate::new("E");
        enemy_t.set_health(200.0);
        enemy_t.add_kind_of(KindOf::Structure);
        enemy_t.add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("E".into(), enemy_t);
        let ranger = logic
            .create_object("R", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let enemy = logic
            .create_object("E", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        assert!(logic.get_object(ranger).unwrap().weapon.is_some());
        logic.queue_command(cmd(
            1,
            CommandType::AttackObject { target_id: enemy },
            vec![ranger],
        ));
        // 200 HP structure; default weapon needs multi-second fight.
        for _ in 0..400 {
            logic.update();
            if !logic
                .get_object(enemy)
                .map(|o| o.is_alive())
                .unwrap_or(false)
            {
                break;
            }
        }
        let enemy_dead = logic
            .get_object(enemy)
            .map(|o| !o.is_alive())
            .unwrap_or(true); // removed from world after destroy list
        assert!(enemy_dead, "enemy should die via update_combat weapon path");
    }
}
