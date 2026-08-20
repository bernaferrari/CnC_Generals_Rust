//! Wave 236 residual peels: `CommandSystem::process_mouse_input` takes
//! `Option<&GameLogic>` and engine InGame callers pass `None` when a
//! presentation frame is installed (RMB/cursor/minimap). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 235 RMB full presentation classify residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` process_mouse_input / create_selection_command /
//!   create_select_similar_command Option path + presentation box/similar ids
//! - `cnc_game_engine.rs` process_mouse_input callers pass None with frame
//!
//! Fail-closed:
//! - Boot/no-frame path still passes Some(&game_logic)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Mouse input presentation-only residual method names.
pub const LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236: &[&str] = &[
    "process_mouse_input",
    "Option<&GameLogic>",
    "presentation_box_select_units",
    "presentation_select_similar_units",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236: &[&str] = &[
    "REQUIRE_MOUSE_INPUT_OPTION_GAME_LOGIC",
    "REQUIRE_ENGINE_PASSES_NONE_WITH_FRAME",
    "LIVE_MOUSE_INPUT_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MOUSE_INPUT_PRESENTATION_ONLY_CMD_NAMES_WAVE236: &[&str] = &[
    "click_live_mouse_input_presentation_only_ok_prepare",
    "click_live_mouse_input_presentation_only_ok_live",
    "click_live_mouse_input_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_mouse_input_presentation_only_method_names_residual_wave236() -> bool {
    LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236.len() == 5
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "process_mouse_input",
        ) == Some(0)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "presentation_box_select_units",
        ) == Some(2)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236() -> bool {
    LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236.len() == 4
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236,
            "REQUIRE_MOUSE_INPUT_OPTION_GAME_LOGIC",
        ) == Some(0)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236,
            "LIVE_MOUSE_INPUT_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_MOUSE_INPUT_PRESENTATION_ONLY_CMD_NAMES_WAVE236.len() == 3
}

/// Wave 236 composite residual honesty pack.
pub fn honesty_live_mouse_input_presentation_only_residual_pack_wave236() -> bool {
    honesty_live_mouse_input_presentation_only_method_names_residual_wave236()
        && honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Wave 236: InGame with a presentation freeze → None (no GameLogic dual-read).
/// Boot/no-frame → Some(game_logic) so classify still has a live residual.
pub fn mouse_game_logic_for_process_mouse_input<'a>(
    presentation_installed: bool,
    game_logic: &'a crate::game_logic::GameLogic,
) -> Option<&'a crate::game_logic::GameLogic> {
    if presentation_installed {
        None
    } else {
        Some(game_logic)
    }
}

/// Source residual: Option mouse input + engine None-with-frame.
pub fn honesty_mouse_input_presentation_only_source() -> bool {
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let helper = include_str!("host_live_mouse_input_presentation_only_residual_wave236.rs");
    let Some(pmi) = fn_body(cs, "pub fn process_mouse_input(")
        .or_else(|| fn_body(cs, "fn process_mouse_input("))
    else {
        return false;
    };
    if !(pmi.contains("game_logic: Option<&GameLogic>")
        || cs.contains("fn process_mouse_input(\n        &mut self,\n        context: &MouseCommandContext,\n        selected_units: &[ObjectId],\n        player_id: u32,\n        game_logic: Option<&GameLogic>"))
    {
        // Fall through to string search
        if !cs.contains("game_logic: Option<&GameLogic>") {
            return false;
        }
    }
    if !(cs.contains("presentation_box_select_units")
        && cs.contains("presentation_select_similar_units")
        && cs.contains("Wave 236"))
    {
        return false;
    }
    if !helper.contains("pub fn mouse_game_logic_for_process_mouse_input") {
        return false;
    }
    let Some(wrap) = fn_body(eng, "fn presentation_mouse_game_logic(") else {
        return false;
    };
    // Engine InGame callers pass None when presentation frame installed;
    // boot/no-frame still passes Some(&self.game_logic).
    wrap.contains("last_presentation_frame.is_some()")
        && wrap.contains("Some(&self.game_logic)")
        && wrap.contains("host_presentation_mouse_game_logic")
        && wrap.contains("mouse_game_logic_for_process_mouse_input")
        && wrap.contains("Wave 236")
        && eng.contains("process_mouse_input")
        && eng.contains("last_presentation_frame.is_some()")
        && eng.contains("Some(&self.game_logic)")
        && eng.matches("process_mouse_input").count() >= 3
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_mouse_input_presentation_only_honesty() -> bool {
    honesty_live_mouse_input_presentation_only_residual_pack_wave236()
        && honesty_mouse_input_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_mouse_input_presentation_only_method_names_residual_wave236());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236());
    }

    #[test]
    fn wave236_composite_pack() {
        assert!(honesty_live_mouse_input_presentation_only_residual_pack_wave236());
    }

    #[test]
    fn mouse_input_presentation_only_sources() {
        assert!(honesty_mouse_input_presentation_only_source());
    }

    #[test]
    fn simulate_live_mouse_input_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_mouse_input_presentation_only_honesty(),
            "mouse input presentation-only residual must latch"
        );
    }

    #[test]
    fn mouse_game_logic_for_process_mouse_input_splits_frame_vs_boot() {
        use crate::game_logic::GameLogic;
        use crate::gameworld_shadow::ensure_gate_damage_authority;

        ensure_gate_damage_authority();
        let logic = GameLogic::new();
        assert!(
            mouse_game_logic_for_process_mouse_input(true, &logic).is_none(),
            "presentation freeze must pass None (no live GameLogic dual-read)"
        );
        assert!(
            mouse_game_logic_for_process_mouse_input(false, &logic).is_some(),
            "boot/no-frame must pass Some(&game_logic)"
        );
    }

    #[test]
    fn process_mouse_input_presentation_only_rmb_attack_without_live_logic() {
        use crate::command_system::{
            CommandSystem, CommandType, ModifierKeys, MouseButton, MouseCommandContext,
            PresentationSelectedUnitHint, PresentationTargetHint,
        };
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
        use crate::gameworld_shadow::ensure_gate_damage_authority;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let mut ranger_t = ThingTemplate::new("Wave236Ranger");
        ranger_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("Wave236Ranger".into(), ranger_t);
        let mut rebel_t = ThingTemplate::new("Wave236Rebel");
        rebel_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("Wave236Rebel".into(), rebel_t);
        let attacker = logic
            .create_object("Wave236Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker");
        let target = logic
            .create_object("Wave236Rebel", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
            .expect("target");

        let ctx = MouseCommandContext {
            world_position: glam::Vec3::new(50.0, 0.0, 0.0),
            target_object: Some(target),
            target_presentation: Some(PresentationTargetHint {
                id: target,
                is_alive: true,
                is_structure: false,
                is_resource: false,
                under_construction: false,
                sold: false,
                team: Team::GLA,
                is_enemy_of_local: true,
                is_neutral: false,
                template_name: "Wave236Rebel".into(),
                can_be_entered: false,
                enter_available_capacity: 0,
                enter_uses_transport_slots: false,
                enter_requires_infantry: false,
                enter_forbids_aircraft: false,
                enter_disabled_subdued: false,
                enter_is_rider_change: false,
                rider_change_allowed_templates: Vec::new(),
                is_damaged: false,
                is_friendly_of_local: false,
                provides_vehicle_repair: false,
                provides_aircraft_repair: false,
                provides_heal: false,
                can_provide_service: true,
                dock_kind: crate::game_logic::DockKind::None,
                dock_controller_is_local: false,
                stored_supplies: 0,
                capturable: false,
                immune_to_capture: false,
                capture_garrisonable: false,
                capture_nonstealthed_garrison_count: 0,
                capture_friendly_garrison_count: 0,
                capture_target_effectively_stealthed: false,
                is_crate: false,
                is_salvage_crate: false,
                is_vehicle: false,
                is_aircraft: false,
                is_drone: false,
                is_carbomb: false,
            }),
            selected_presentation: vec![PresentationSelectedUnitHint {
                id: attacker,
                is_alive: true,
                is_resource_collector: false,
                is_worker: false,
                can_attack: true,
                can_move: true,
                can_request_service: true,
                can_capture: false,
                template_name: "Wave236Ranger".into(),
                can_repair: false,
                is_damaged: false,
                is_vehicle: false,
                is_aircraft: false,
                is_above_terrain: false,
                is_infantry: true,
                transport_slot_count: 1,
                stored_supplies: 0,
                is_controlled_by_local: true,
                capture_power: crate::game_logic::CapturePowerKind::None,
                capture_power_ready: false,
                is_salvager: false,
            }],
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut sys = CommandSystem::new();
        let gl = mouse_game_logic_for_process_mouse_input(true, &logic);
        assert!(gl.is_none(), "frame path must not borrow live GameLogic");
        let cmd = sys
            .process_mouse_input(&ctx, &[attacker], 0, gl)
            .expect("presentation RMB must produce command");
        match cmd.command_type {
            CommandType::AttackObject { target_id } => assert_eq!(target_id, target),
            other => panic!("expected AttackObject from presentation freeze, got {other:?}"),
        }
    }

    #[test]
    fn process_mouse_input_presentation_box_select_without_live_logic() {
        use crate::command_system::{
            CommandSystem, CommandType, ModifierKeys, MouseButton, MouseCommandContext,
        };
        use crate::game_logic::{GameLogic, ObjectId};
        use crate::gameworld_shadow::ensure_gate_damage_authority;

        ensure_gate_damage_authority();
        let logic = GameLogic::new();
        let a = ObjectId(11);
        let b = ObjectId(22);
        let ctx = MouseCommandContext {
            world_position: glam::Vec3::ZERO,
            target_object: None,
            target_presentation: None,
            selected_presentation: Vec::new(),
            presentation_box_select_units: vec![a, b],
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(10.0, 10.0),
            viewport_size: Some(glam::Vec2::new(800.0, 600.0)),
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Left,
            modifier_keys: ModifierKeys::default(),
            is_drag: true,
            drag_start: Some(glam::Vec2::ZERO),
            drag_end: Some(glam::Vec2::new(10.0, 10.0)),
            drag_start_world: Some(glam::Vec3::ZERO),
            drag_end_world: Some(glam::Vec3::new(10.0, 0.0, 10.0)),
        };
        let mut sys = CommandSystem::new();
        let gl = mouse_game_logic_for_process_mouse_input(true, &logic);
        let cmd = sys
            .process_mouse_input(&ctx, &[], 0, gl)
            .expect("presentation box-select must produce command");
        match cmd.command_type {
            CommandType::CreateSelectedGroup { create_new, units } => {
                assert!(create_new);
                assert_eq!(units, vec![a, b]);
            }
            other => {
                panic!("expected CreateSelectedGroup from presentation box ids, got {other:?}")
            }
        }
    }

    #[test]
    fn process_mouse_input_boot_some_game_logic_classifies_force_attack() {
        use crate::command_system::{
            CommandSystem, CommandType, ModifierKeys, MouseButton, MouseCommandContext,
        };
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
        use crate::gameworld_shadow::ensure_gate_damage_authority;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let mut ranger_t = ThingTemplate::new("Wave236BootRanger");
        ranger_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("Wave236BootRanger".into(), ranger_t);
        let mut rebel_t = ThingTemplate::new("Wave236BootRebel");
        rebel_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("Wave236BootRebel".into(), rebel_t);
        let attacker = logic
            .create_object(
                "Wave236BootRanger",
                Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("attacker");
        let target = logic
            .create_object(
                "Wave236BootRebel",
                Team::GLA,
                glam::Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("target");

        let ctx = MouseCommandContext {
            world_position: glam::Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(target),
            target_presentation: None,
            selected_presentation: Vec::new(),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut sys = CommandSystem::new();
        let gl = mouse_game_logic_for_process_mouse_input(false, &logic);
        assert!(gl.is_some(), "boot path must pass Some(&game_logic)");
        let cmd = sys
            .process_mouse_input(&ctx, &[attacker], 0, gl)
            .expect("boot ctrl RMB should produce command");
        match cmd.command_type {
            CommandType::ForceAttackObject { target_id } => assert_eq!(target_id, target),
            other => panic!("expected ForceAttackObject on boot Some path, got {other:?}"),
        }
    }
}
