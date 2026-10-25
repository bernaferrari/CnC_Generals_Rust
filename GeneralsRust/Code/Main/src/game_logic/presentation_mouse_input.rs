//! Presentation mouse input.

/// Lookup residual name index (exact match).

/// Mouse input presentation-only residual method names.

/// Ordered residual navigation steps.

/// Runtime-host command residual names.

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 236 composite residual honesty pack.

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

/// Live residual: source honesty pack latches.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_game_logic_for_process_mouse_input_splits_frame_vs_boot() {
        use crate::game_logic::GameLogic;

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
                is_unmanned: false,
                is_mine: false,

                authors_carbomb_weapon_set: false,
            }),
            selected_presentation: vec![PresentationSelectedUnitHint {
                id: attacker,
                reject_unmanned: false,
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
                can_override_special_power_destination: false,
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
