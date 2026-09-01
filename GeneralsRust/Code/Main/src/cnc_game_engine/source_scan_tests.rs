// Mechanical extract of file-level source-scan tests from cnc_game_engine.rs.
// ENGINE_SRC concatenates live split files (including runtime_host/*).

use super::*;

#[test]
fn stop_and_guard_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"s\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_Stop\")"),
        "S must issue Command_Stop residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"g\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_Guard\")"),
        "G must issue Command_Guard residual"
    );
    // Ctrl+S quick-save must remain distinct from Stop.
    assert!(
        src.contains("eq_ignore_ascii_case(\"s\") && ctrl_down")
            && src.contains("quick_save_from_hotkey"),
        "Ctrl+S quick-save residual must remain"
    );
}

#[test]
fn retail_selection_and_scatter_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        (src.contains("eq_ignore_ascii_case(\"x\") && !ctrl_down")
            || (src.contains("eq_ignore_ascii_case(\"x\")")
                && src.contains("Retail CommandMap SCATTER KEY_X residual")))
            && src.contains("issue_named_command_from_ui(\"Command_Scatter\")"),
        "X must issue Command_Scatter residual"
    );
    assert!(
        src.contains("Retail CommandMap SELECT_ALL KEY_Q residual")
            && src.contains("select_all_friendly_units"),
        "Q must SELECT_ALL residual"
    );
    assert!(
        src.contains("Retail CommandMap SELECT_MATCHING_UNITS KEY_E residual")
            && src.contains("select_matching_units_hotkey"),
        "E must SELECT_MATCHING_UNITS residual"
    );
    assert!(
        src.contains("Retail CommandMap SELECT_ALL_AIRCRAFT KEY_W residual")
            && src.contains("select_all_friendly_aircraft"),
        "W must SELECT_ALL_AIRCRAFT residual"
    );

    assert!(
        src.contains("Retail CommandMap VIEW_COMMAND_CENTER KEY_H residual")
            && src.contains("issue_named_command_from_ui(\"Command_ViewCommandCenter\")"),
        "H must VIEW_COMMAND_CENTER residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"f\")")
            && src.contains("issue_named_command_from_ui(\"Command_CreateFormation\")"),
        "Ctrl+F must CREATE_FORMATION residual"
    );

    assert!(
        src.contains("NamedKey::Space") && src.contains("Command_ViewLastRadarEvent"),
        "Space must VIEW_LAST_RADAR_EVENT residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"h\") && ctrl_down")
            && src.contains("select_hero_units_hotkey"),
        "Ctrl+H must SELECT_HERO residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"p\")") && src.contains("toggle_pause"),
        "P must remain pause residual"
    );
}

#[test]
fn select_all_adds_not_replaces() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let select_all_at = src
        .find("fn select_all_units_by_type")
        .expect("select_all_units_by_type");
    let select_all_body = &src[select_all_at..src.len().min(select_all_at + 2500)];
    assert!(
        select_all_body.contains("kindOfUnitSelection skips already-selected")
            && select_all_body.contains("if !already.contains(id)")
            && select_all_body.contains("kept.push(id)")
            && !select_all_body.contains("host_set_selection(self.current_player_id, on_screen)"),
        "SELECT_ALL must add to current selection, not replace with the on-screen subset"
    );
}

#[test]
fn escape_in_handle_key_press_opens_quit_menu_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let hk = src
        .find("fn handle_key_press(&mut self, key: &Key)")
        .expect("handle_key_press");
    let esc = src[hk..].find("NamedKey::Escape").expect("Escape arm");
    let window = &src[hk + esc..src.len().min(hk + esc + 1800)];
    assert!(
        window.contains("ToggleQuitMenu") || window.contains("host_toggle_retail_quit_menu"),
        "Escape OPTIONS must ToggleQuitMenu"
    );
    assert!(
        !window.contains("cancelled structure placement residual"),
        "Escape must not cancel structure placement (C++ OPTIONS)"
    );
    assert!(
        !window.contains("pending_map_command.take()"),
        "Escape must not cancel pending map command (C++ OPTIONS)"
    );
}

#[test]
fn beacon_and_control_bar_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"b\")") && src.contains("Command_PlaceBeacon"),
        "Ctrl+B must PLACE_BEACON residual"
    );
    assert!(
        src.contains("PendingMapCommand::PlaceBeacon")
            && src.contains("arm_radius_cursor_for_pending(\"RADAR\")"),
        "PlaceBeacon must arm pending map click",
    );
    assert!(
        src.contains("NamedKey::F9")
            && src.contains("toggle_control_bar")
            && src.contains("MSG_META_TOGGLE_CONTROL_BAR"),
        "F9 must TOGGLE_CONTROL_BAR via WND ToggleControlBar (C++ CommandXlat.cpp:3144)"
    );
}

#[test]
fn wnd_control_bar_is_live_gameplay_hud_not_only_soft_ui_manager() {
    // C++ HideControlBar / ShowControlBar / ToggleControlBar (ControlBarCallback.cpp:477-549).
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn hide_gameplay_layouts") && src.contains("hide_control_bar(true)"),
        "shell hide must call live WND HideControlBar"
    );
    assert!(
        src.contains("fn ensure_gameplay_layouts") && src.contains("show_control_bar"),
        "InGame enter must ShowControlBar on live WND tree"
    );
    let hud = include_str!("../ui/hud.rs");
    assert!(
        hud.contains("TheInGameUI::message(text)"),
        "GameHUD info messages must fan into live TheInGameUI (InGameUI.cpp:1993)"
    );
}

fn camera_bookmarks_and_delete_beacon_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("camera_view_bookmarks") && src.contains("fn handle_camera_view_hotkey"),
        "F1-F8 camera bookmark residual required"
    );
    assert!(
        src.contains("NamedKey::F1") && src.contains("handle_camera_view_hotkey(0)"),
        "F1 must recall/save view slot 0"
    );
    assert!(
        src.contains("NamedKey::F8") && src.contains("handle_camera_view_hotkey(7)"),
        "F8 must recall/save view slot 7"
    );
    assert!(
        src.contains("SAVE_VIEW")
            && src.contains("VIEW_VIEW")
            && src.contains("store_or_apply_camera_view"),
        "CommandMap SAVE_VIEW / VIEW_VIEW remaps must reach LookAt"
    );
    assert!(
        !src.contains("ctrl && slot < 4"),
        "Ctrl+F1..F4 must not steal SAVE_VIEW for debug overlays"
    );
    assert!(
        src.contains("NamedKey::Delete") && src.contains("Command_RemoveBeacon"),
        "Delete must DELETE_BEACON residual"
    );
    // Debug destroy kept behind Shift+Delete.
    assert!(
        src.contains("destroy_object") && src.contains("Shift+Delete"),
        "Shift+Delete debug destroy residual must remain"
    );
}

#[test]
fn cheer_camera_reset_unit_cycle_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_Cheer") && src.contains("eq_ignore_ascii_case(\"c\")"),
        "Ctrl+C must ALL_CHEER residual"
    );
    assert!(
        src.contains("Numpad5") && src.contains("reset_camera_view_hotkey"),
        "KP5 must CAMERA_RESET residual"
    );
    assert!(
        src.contains("ArrowRight") && src.contains("cycle_friendly_selection(1)"),
        "Ctrl+Right must SELECT_NEXT_UNIT residual"
    );
    assert!(
        src.contains("ArrowLeft") && src.contains("cycle_friendly_selection(-1)"),
        "Ctrl+Left must SELECT_PREV_UNIT residual"
    );
    assert!(
        src.contains("cycle_friendly_worker_selection"),
        "Ctrl+Up/Down must worker cycle residual"
    );
}

#[test]
fn diplomacy_and_control_group_modifiers_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("toggle_diplomacy_panel_hotkey") && src.contains("NamedKey::Tab"),
        "Tab must DIPLOMACY residual"
    );
    assert!(
        src.contains("ADD_TEAM residual") || src.contains("shift_down"),
        "Shift+digit must ADD_TEAM residual"
    );
    assert!(
        src.contains("VIEW_TEAM residual") || src.contains("alt_down"),
        "Alt+digit must VIEW_TEAM residual"
    );
    assert!(
        src.contains("Escape closed diplomacy panel residual"),
        "Escape must close diplomacy before pause"
    );
}

#[test]
fn chat_and_screenshot_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("NamedKey::Enter") && src.contains("ChatTarget::All"),
        "Enter must CHAT_EVERYONE residual"
    );
    assert!(
        src.contains("NamedKey::Backspace") && src.contains("ChatTarget::Allies"),
        "Backspace must CHAT_ALLIES residual"
    );
    assert!(
        src.contains("NamedKey::F12") && src.contains("take_screenshot_hotkey"),
        "F12 must TAKE_SCREENSHOT residual"
    );
    assert!(
        src.contains("Escape closed chat residual"),
        "Escape must close chat first"
    );
}

#[test]
fn deploy_and_numpad_camera_hold_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"d\") && !ctrl_down") && src.contains("Command_Deploy"),
        "D must Deploy residual"
    );
    assert!(
        src.contains("Numpad4") && src.contains("camera_rotate_left_held"),
        "KP4 must rotate-left hold residual"
    );
    assert!(
        src.contains("Numpad6") && src.contains("camera_rotate_right_held"),
        "KP6 must rotate-right hold residual"
    );
    assert!(
        src.contains("Numpad8") && src.contains("camera_zoom_in_held"),
        "KP8 must zoom-in hold residual"
    );
    assert!(
        src.contains("Numpad2") && src.contains("camera_zoom_out_held"),
        "KP2 must zoom-out hold residual"
    );
    let input = include_str!("input.rs");
    let wnd_used = input
        .find("_ if wnd_used && !escape_toggles_live_quit_menu")
        .expect("wnd_used WindowXlat gate");
    let numpad = input
        .find("Retail numpad camera residual")
        .expect("numpad residual");
    assert!(
        wnd_used < numpad,
        "numpad rotate/zoom must die when WindowXlat consumed the key"
    );
}

#[test]
fn show_options_event_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("UIEvent::ShowOptions") && src.contains("Screen::Options"),
        "engine must handle ShowOptions residual"
    );
    let ui = include_str!("../ui/ui_manager.rs");
    assert!(
        ui.contains("options_menu") && ui.contains("Screen::Options"),
        "UIManager must own OptionsMenu residual"
    );
}

#[test]
fn remaining_commandmap_hotkeys_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("toggle_camera_tracking_drawable_hotkey")
            && src.contains("self.camera_tracking_selection = true")
            && src.contains("set_camera_tracking_drawable(true)")
            && !src.contains("Camera tracking selection: OFF")
            && !src.contains("camera_track_ok:off"),
        "TOGGLE_CAMERA_TRACKING_DRAWABLE only enables (CommandXlat.cpp:3216-3218)"
    );
    assert!(
        src.contains("toggle_replay_fast_forward_hotkey")
            && src.contains("replay_fast_forward")
            && src.contains("m_TiVOFastMode"),
        "TOGGLE_FAST_FORWARD_REPLAY residual required"
    );
    assert!(
        src.contains("DEMO_INSTANT_QUIT") && src.contains("GameState::Exiting"),
        "DEMO_INSTANT_QUIT residual required"
    );
}

#[test]
fn victory_defeat_shows_victory_screen_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("show_match_result(true, self.current_player_id)"),
        "Victory state must open Victory screen"
    );
    assert!(
        src.contains("show_match_result(false, self.current_player_id)"),
        "Defeat state must open Defeat presentation residual"
    );
    assert!(
        src.contains("fn show_match_result")
            || include_str!("../ui/ui_manager.rs").contains("fn show_match_result"),
        "UIManager must expose show_match_result residual"
    );
}

#[test]
fn wasd_not_camera_scroll_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let i = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let body = &src[i..src.len().min(i + 3500)];
    assert!(
        !body.contains("is_character_key_pressed(\"w\")")
            && !body.contains("is_character_key_pressed(\"s\")")
            && !body.contains("is_character_key_pressed(\"a\")")
            && !body.contains("is_character_key_pressed(\"d\")"),
        "WASD must not drive camera scroll (unit hotkey conflict)"
    );
    assert!(
        body.contains("NamedKey::ArrowUp") && body.contains("NamedKey::ArrowDown"),
        "arrow keys remain camera scroll residual"
    );
}

#[test]
fn windowed_edge_scroll_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let i = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let body = &src[i..src.len().min(i + 4500)];
    assert!(
        body.contains("EDGE_SCROLL_SIZE"),
        "edge scroll residual must remain"
    );
    assert!(
        body.contains("!self.is_windowed"),
        "C++ LookAtXlat.cpp:277-293 edge-scrolls only when !windowed"
    );
    assert!(
        src.contains("EDGE_SCROLL_SIZE: f32 = 3.0"),
        "C++ edgeScrollSize is 3px"
    );
    let edge = body
        .find("let edge_allowed")
        .map(|off| &body[off..body.len().min(off + 400)])
        .expect("edge_allowed");
    assert!(
        !edge.contains("GameState")
            && !edge.contains("chat_panel")
            && !edge.contains("diplomacy_panel"),
        "C++ LookAtXlat SCREENEDGE has no InGame/chat/diplomacy gates"
    );
}

#[test]
fn settings_changed_health_bars_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("UIEvent::SettingsChanged") && src.contains("game.show_health_bars"),
        "SettingsChanged must apply show_health_bars residual"
    );
}

#[test]
fn hud_h_does_not_steal_view_command_center_residual() {
    let hud = include_str!("../ui/hud.rs");
    // After residual fix, bare KeyCode::H toggle must not remain in GameHUD key handler.
    let marker = "Global HUD hotkeys";
    let i = hud.find(marker).expect("global HUD hotkeys section");
    let section = &hud[i..hud.len().min(i + 400)];
    assert!(
        !section.contains("KeyCode::H =>"),
        "GameHUD must not bind bare H (VIEW_COMMAND_CENTER conflict)"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("Command_ViewCommandCenter")
            && eng.contains("eq_ignore_ascii_case(\"h\") && !ctrl_down"),
        "engine H must still VIEW_COMMAND_CENTER"
    );
}

#[test]
fn drag_select_rect_overlay_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("selection_start_screen")
            && src.contains("DragSelectRect")
            && src.contains("drag_rect.filter(|r| r.is_valid())"),
        "InGame render must feed DragSelectRect while dragging"
    );
    assert!(
        src.contains("Defer empty-ground clear until left-release")
            || src.contains("Instant clear on mousedown fights drag-select"),
        "mousedown must not clear selection before drag completes"
    );
}

#[test]
fn structure_placement_ghost_cursor_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("sync_pending_structure_placement_cursor")
            && src.contains("sync_structure_placement_cursor")
            && src.contains("legal_build_code_at_for_builder"),
        "placement ghost must track cursor legality each frame"
    );
    let hud = include_str!("../ui/hud.rs");
    assert!(
        hud.contains("placement: crate::ui::construction_panel::PlacementPreview")
            || hud.contains("PlacementPreview"),
        "HUD ConstructionPanel must own PlacementPreview ghost"
    );
}

#[test]
fn pending_map_radius_cursor_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("arm_radius_cursor_for_pending")
            && src.contains("sync_pending_map_command_radius_cursor")
            && src.contains("clear_radius_cursor_overlays"),
        "pending map commands must drive radius cursor residual"
    );
    assert!(
        src.contains("ATTACK_CONTINUE_AREA") && src.contains("GUARD_AREA"),
        "AttackMove/Guard must arm retail radius cursor names"
    );
    assert!(
        src.contains("PARTICLECANNON") || src.contains("OFFENSIVE_SPECIALPOWER"),
        "special power must map to radius cursor type"
    );
    assert!(
        src.contains("leftover_resolve_radius_cursor_radius")
            && src.contains("PendingMapCommand::Weapon(_) => \"ATTACK_DAMAGE_AREA\""),
        "attack-ground must use leftover damage radius, not OFFENSIVE_SPECIALPOWER table 0"
    );
    assert!(
        !src.contains("o.weapon_range") && !src.contains("o.vision_range"),
        "radius cursor must not substitute presentation weapon/vision proxies"
    );
}

#[test]
fn minimap_right_click_context_command_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn issue_minimap_move")
            && src.contains("process_mouse_input")
            && src.contains("MouseButton::Right"),
        "minimap RMB must use context-sensitive CommandSystem path"
    );
    // Ensure issue_minimap_move body is not pure command_move-only.
    let start = src
        .find("fn issue_minimap_move")
        .expect("issue_minimap_move");
    let end = src[start + 1..]
        .find(
            "
    fn ",
        )
        .map(|i| start + 1 + i)
        .unwrap_or(start + 4000);
    let body = &src[start..end];
    assert!(
        body.contains("process_mouse_input") && body.contains("find_object_at_position"),
        "minimap RMB must resolve target + command context like world RMB"
    );
}

#[test]
fn ground_marker_circles_overlay_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("collect_ground_marker_circles") && src.contains("ground_markers"),
        "engine must feed placement/radius ground markers into selection overlay"
    );
    let sel = include_str!("../graphics/selection_renderer.rs");
    assert!(
        sel.contains("ground_markers: Vec<SelectedUnit>"),
        "selection overlay must accept ground_markers residual"
    );
}

#[test]
fn dual_hud_construction_hotkey_route_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Interactive::handle_key_press(&mut self.game_hud, ui_key)")
            && src.contains("drain_pending_ui_events"),
        "engine GameHUD must receive construction/command hotkeys in InGame"
    );
    let um = include_str!("../ui/ui_manager.rs");
    assert!(
        um.contains("pending_structure_placement") && um.contains("Fall through to GameHUD"),
        "UIManager Escape must not open pause over active structure placement"
    );
}

#[test]
fn order_line_overlay_draw_residual() {
    let sel = include_str!("../graphics/selection_renderer.rs");
    assert!(
        sel.contains("draw_order_line_segments")
            && sel.contains("MoveLineUpload::pack_from_presentation")
            && sel.contains("AttackLineUpload::pack_from_presentation"),
        "selection overlay must GPU-draw move/attack order lines from presentation"
    );
}

#[test]
fn shift_select_and_ctrl_force_attack_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn toggle_select_object")
            && src.contains("fn issue_force_attack_from_left_click"),
        "left-click must support Shift multi-select and Ctrl force-attack"
    );
    let start = src.find("fn handle_left_click").expect("handle_left_click");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 2500);
    let body = &src[start..end];
    assert!(
        body.contains("shift_down") && body.contains("issue_force_attack_from_left_click"),
        "handle_left_click must still probe Shift context and Ctrl force-attack"
    );
    assert!(
        !body.contains("select_left_click_target") && !body.contains("toggle_select_object"),
        "RAW LMB down must not commit SelectionXlat"
    );
    let release = src
        .find("fn handle_left_release")
        .expect("handle_left_release");
    let release_end = src[release..]
        .find("fn handle_right_click")
        .map(|i| release + i)
        .unwrap_or(release + 12_000);
    let release_body = &src[release..release_end];
    assert!(
        release_body.contains("select_left_click_target"),
        "point LMB must commit on non-drag release at the up pixel"
    );
}

#[test]
fn select_all_is_command_map_not_ctrl_a() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Retail CommandMap SELECT_ALL KEY_Q residual"),
        "Q residual must remain the CommandMap SELECT_ALL fallback"
    );
    let invented = format!("{}{}", "Convenience alias; ", "retail SELECT_ALL is KEY_Q");
    assert!(
        !src.contains(&invented),
        "live host must not invent an unguarded Ctrl+A SELECT_ALL"
    );
}

#[test]
fn empty_lmb_does_not_force_select_can_select_rejects() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let press = src.find("fn handle_left_click").expect("handle_left_click");
    let press_end = src[press + 1..]
        .find("\n    fn ")
        .map(|i| press + 1 + i)
        .unwrap_or(press + 2500);
    let release = src
        .find("fn handle_left_release")
        .expect("handle_left_release");
    let release_end = src[release..]
        .find("fn handle_right_click")
        .map(|i| release + i)
        .unwrap_or(release + 12_000);
    let select = src
        .find("fn select_left_click_target")
        .expect("select_left_click_target");
    let select_end = src[select + 1..]
        .find("\n    fn ")
        .map(|i| select + 1 + i)
        .unwrap_or(select + 800);
    let force = format!("{}{}", "force-select local ", "object");
    assert!(
        !src[press..press_end].contains(&force)
            && !src[release..release_end].contains(&force)
            && !src[select..select_end].contains(&force),
        "empty LMB must not force-select after CanSelectDrawable miss"
    );
    let body = &src[select..select_end];
    assert!(
        body.contains("if !self.is_point_selectable_click_target(object_id)")
            && body.contains("return;"),
        "select_left_click_target must return on CanSelectDrawable reject"
    );
}

#[test]
fn cancel_unit_production_rmb_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn cancel_unit_production_from_ui") && src.contains("CancelUnitProduction"),
        "engine must handle CancelUnitProduction residual"
    );
    let hud = include_str!("../ui/hud.rs");
    assert!(
        hud.contains("CancelUnitProduction")
            && hud.contains("build_queue_cancel")
            && hud.contains("production_id")
            && hud.contains("clicked_queue_slot"),
        "HUD LMB queue slot must raise CancelUnitProduction by productionID"
    );
}

#[test]
fn context_mouse_cursor_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn sync_context_mouse_cursor")
            && src.contains("fn resolve_context_cursor_icon")
            && src.contains("set_cursor")
            && src.contains("fn sync_ingame_mouseover_hint")
            && src.contains("create_mouseover_hint"),
        "InGame mouse move must apply context cursor residual and InGameUI mouseover hints"
    );
    let start = src
        .find("fn sync_context_mouse_cursor")
        .expect("sync_context_mouse_cursor");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 400);
    let body = &src[start..end];
    let hint_at = body
        .find("self.sync_ingame_mouseover_hint()")
        .expect("SelectionXlat hover must call createMouseoverHint");
    let skip_at = body
        .find("last_context_cursor")
        .expect("unchanged-cursor skip");
    assert!(
        hint_at < skip_at,
        "C++ SelectionXlat posts MSG_MOUSEOVER_* even when the cursor icon is unchanged"
    );
    assert!(
        body.contains("host_recorder_is_playback")
            && body.contains("lookat_has_mouse_moved_recently"),
        "replay hover must keep SELECTING/ARROW until the viewer moves"
    );
    assert!(
        src.contains("\"AttackObj\"")
            && src.contains("\"Build\"")
            && src.contains("\"InvalidBuild\"")
            && src.contains("\"Waypoint\""),
        "cursor residual must cover attack/build/waypoint names"
    );
}

#[test]
fn create_mouseover_hint_sets_cursor_tooltip_for_named_object_under_cursor() {
    use game_client::gui::ingame_ui::{InGameUI, PresentationUnitCatalogEntry};
    use game_client::input::mouse::with_mouse;
    use game_engine::common::language::Language;
    use gamelogic::common::ObjectShroudStatus;

    Language::clear_localized_strings();
    Language::register_localized_string("ThingTemplate:AmericaRanger", "Ranger");
    let catalog = [PresentationUnitCatalogEntry {
        object_id: 7,
        template_name: "AmericaRanger".to_string(),
        team_name: String::new(),
        selectable: true,
        position: [0.0; 3],
        orientation: 0.0,
        disabled: false,
        under_construction: false,
        construction_percent: 0.0,
        max_garrison: 0,
        occupant_count: 0,
        ocl_timer_seconds: 0,
        sold: false,
        script_unsellable: false,
        unselectable: false,
        destroyed: false,
        masked: false,
        effectively_stealthed: false,
        disguised: false,
        disguise_as_template: None,
        disguise_as_team: None,
        kind_names: Vec::new(),
        special_power_ready: false,
        airborne_target: false,
        shroud_status: ObjectShroudStatus::Clear,
        slaver_object_id: None,
        health_current: 100.0,
        health_maximum: 100.0,
        veterancy_overlay: None,
        production_progress: None,
        production_template: None,
        production_paused: false,
        command_set_name: String::new(),
        hotkey_group: -1,
        caption: String::new(),
        supply_boxes: None,
    }];

    let moused = InGameUI::apply_catalog_mouseover_tooltip(&catalog, Some(7), false);
    assert_eq!(moused, 7);
    let tip = with_mouse(|m| m.cursor_tooltip_state().tooltip_text.clone());
    assert_eq!(tip, "Ranger");
    Language::clear_localized_strings();
}

#[test]
fn auto_dozer_structure_place_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn find_nearest_friendly_dozer"),
        "structure place must auto-pick nearest dozer residual"
    );
    assert!(
        !src.contains("Select a dozer or worker to build"),
        "hq-8955d: no-builder place must not invent a HUD toast"
    );
    let start = src.find("fn place_structure_from_ui").expect("place");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 4000);
    let body = &src[start..end];
    assert!(
        body.contains("clear_structure_placement")
            && body.contains("game_hud.construction_panel")
            && body.contains("ui_manager"),
        "legal place must dual-clear both HUD placement ghosts"
    );
}

#[test]
fn sneak_attack_place_keeps_placement_angle() {
    // C++ PlaceEventTranslator.cpp:226-229 appends placement angle on sneak-attack confirm.
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let start = src.find("fn place_structure_from_ui").expect("place");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 4000);
    let body = &src[start..end];
    assert!(
        body.contains("get_placement_angle")
            && body.contains("LocationFacing")
            && body.contains("SpecialPowerType::SneakAttack"),
        "sneak-attack place must emit DoSpecialPower with placement facing"
    );
}

#[test]
fn deploy_d_key_not_shadowed_by_debug_defeat_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let start = src.find("fn handle_key_press").expect("handle_key_press");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 8000);
    let body = &src[start..end];
    assert!(
        body.contains("Command_Deploy")
            && body.contains("eq_ignore_ascii_case(\"d\") && !ctrl_down"),
        "D must issue Command_Deploy residual"
    );
    // Bare D must not be bound to debug_show_victory(None) ahead of Deploy.
    assert!(
        !body.contains(
            "eq_ignore_ascii_case(\"d\") => {\n                self.debug_show_victory(None)"
        ),
        "debug defeat must not steal D from Deploy"
    );
}

#[test]
fn deployed_blocks_can_move_and_guard_ring_residual() {
    let obj = crate::game_logic::object::OBJECT_SRC;
    let start = obj.find("pub fn can_move").expect("can_move");
    let body = &obj[start..start + 700];
    assert!(
        body.contains("!self.status.deployed"),
        "deployed units must not can_move residual"
    );
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("GUARD_AREA_RADIUS") && src.contains("guard_position"),
        "selected guard units must draw guard-area ring residual"
    );
}

#[test]
fn eva_low_power_chat_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn sync_eva_messages_from_logic")
            && src.contains("add_eva_message")
            && src.contains("eva_low_power_count")
            && src.contains("Insufficient funds")
            && src.contains("Our base is under attack"),
        "engine must surface EVA LOWPOWER/funds/under-attack to chat residual"
    );
}

#[test]
fn pending_unit_ability_arm_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("PendingUnitAbility")
            && src.contains("fn arm_pending_unit_ability")
            && src.contains("UnitAbility(ability)"),
        "ControlBar unit abilities must arm pending target click residual"
    );
    assert!(
        src.contains("PendingUnitAbility::Hijack")
            && src.contains("PendingUnitAbility::SnipeVehicle")
            && src.contains("PendingUnitAbility::PlantTimedDemoCharge"),
        "hero/ability set must include hijack/snipe/charges residual"
    );
}

#[test]
fn presentation_event_sfx_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn play_presentation_event_sfx")
            && src.contains("SoundType::ConstructionComplete")
            && src.contains("SoundType::UnitReady"),
        "presentation complete events must play SFX residual"
    );
}

#[test]
fn sticky_waypoint_mode_toggle_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("sticky_waypoint_mode")
            && src.contains("eq_ignore_ascii_case(\"z\")")
            && !src.contains("Waypoint mode: ON")
            && !src.contains("Waypoint mode: OFF"),
        "Z must toggle sticky waypoint mode residual without HUD toast"
    );
}

#[test]
fn idle_worker_period_key_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("c == \".\"")
            && src.contains("host_select_next_idle_worker_from_control_bar")
            && src.contains("SELECT_IDLE_WORKER"),
        "period key must cycle idle workers residual"
    );
    let start = src
        .find("fn cycle_friendly_worker_selection")
        .expect("cycle_friendly_worker_selection");
    let body = &src[start..start + 2200];
    assert!(
        body.contains("KindOf::Dozer") && body.contains("host_center_camera_on"),
        "SELECT_NEXT/PREV_WORKER must be KINDOF_DOZER + lookAt (CommandXlat.cpp:2573-2798)"
    );
}

#[test]
fn structure_placement_rotate_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("rotate_structure_placement")
            && src.contains("facing_radians")
            && src.contains("pending_structure_placement.is_some()"),
        "mouse wheel must rotate structure placement ghost residual"
    );
}

#[test]
fn structure_cycle_and_auto_attack_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("sticky_auto_attack"),
        "sticky auto-attack residual required"
    );
    assert!(
        src.contains("AttackMoveTo")
            && !src.contains("Auto-attack: ON")
            && !src.contains("Auto-attack: OFF"),
        "sticky auto-attack must convert moves to attack-move without HUD toast"
    );
}

#[test]
fn force_attack_ground_t_key_and_home_structure_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("ForceAttackGround")
            && src.contains("eq_ignore_ascii_case(\"t\")")
            && !src.contains("Force-attack ground"),
        "T must issue ForceAttackGround at cursor residual without HUD toast"
    );
    // Home/End are CommandMap-bindable (VIEW_COMMAND_CENTER), not
    // invented SELECT_NEXT_STRUCTURE.
}

#[test]
fn patrol_and_sell_hotkey_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_Sell")
            && src.contains("eq_ignore_ascii_case(\"s\")")
            && src.contains("NamedKey::Shift"),
        "Ctrl+Shift+S must sell selection residual"
    );
    let cmd = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cmd.contains("Patrol") && cmd.contains("\"patrol\""),
        "Patrol command residual must exist"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_patrol") && ex.contains("AIState::Patrolling"),
        "execute_patrol must set Patrolling residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("populate_command_set_strip") && pf.contains("HUD_COMMAND_SET_RESIDUAL_PACKS"),
        "command strip must bind CommandSet slots, not invent Patrol"
    );
}

#[test]
fn evacuate_and_repair_hotkey_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"u\")") && src.contains("Command_Evacuate"),
        "U must issue Evacuate residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"r\")")
            && src.contains("Command_Repair")
            && src.contains("PendingUnitAbility::Repair"),
        "R must arm Repair residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("\"repair\"") && cs.contains("CommandType::Repair"),
        "repair button name must map residual"
    );
}

#[test]
fn rally_overcharge_capture_hotkey_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"y\")") && src.contains("Command_SetRallyPoint"),
        "Y must arm SetRallyPoint residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"o\")") && src.contains("Command_ToggleOvercharge"),
        "O must toggle overcharge residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"c\")")
            && src.contains("Command_CaptureBuilding")
            && src.contains("!ctrl_down"),
        "C must arm CaptureBuilding residual"
    );
}

#[test]
fn construction_cameo_hotkey_priority_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("construction_consumed")
            && src.contains("_ if construction_consumed")
            && src.contains("Interactive::handle_key_press(&mut self.game_hud, ui_key)"),
        "construction panel must consume build keys before global hotkeys residual"
    );
    assert!(
        src.contains("cycle_construction_tab")
            && src.contains("cycle_construction_tab(1)")
            && src.contains("force_tab"),
        "[ ] must cycle construction tabs residual",
    );
    let hud = include_str!("../ui/hud.rs");
    assert!(
        hud.contains("fn force_tab")
            // hud.rs declares the enum with bare variants; the qualified
            // `ConstructionTab::Aircraft` cycle paths live in ENGINE_SRC.
            && hud.contains("pub enum ConstructionTab")
            && hud.contains("\n    Aircraft,"),
        "construction panel force_tab residual"
    );
    assert!(
        src.contains("ConstructionTab::Aircraft"),
        "[ ] cycle must reach the Aircraft tab residual"
    );
}

#[test]
fn shift_ctrl_production_queue_multiplier_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("UIEvent::QueueUnitProduction")
            && src.contains("saturating_mul(5)")
            && src.contains("qty = 9"),
        "Shift×5 and Ctrl fill-queue residual for production"
    );
}

#[test]
fn special_power_v_key_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let start = src.find("fn handle_key_press").expect("handle_key_press");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 12000);
    let body = &src[start..end];
    assert!(
        body.contains("Command_DoSpecialPower") && body.contains("eq_ignore_ascii_case(\"v\")"),
        "V must arm Command_DoSpecialPower residual"
    );
    // Bare V must not instantly debug-win.
    assert!(
        !body.contains(
            "eq_ignore_ascii_case(\"v\") => {\n                self.debug_show_victory(Some(self.current_player_id))"
        ),
        "debug victory must not steal bare V from special power"
    );
    assert!(
        body.contains("NamedKey::Shift") && body.contains("debug_show_victory"),
        "debug victory remains behind Ctrl+Shift residual"
    );
}

#[test]
fn strategy_center_battle_plan_residual() {
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("BattlePlanBombardment")
            && cs.contains("initiatebattleplanbombardment")
            && cs.contains("BattlePlanHoldTheLine")
            && cs.contains("BattlePlanSearchAndDestroy"),
        "battle plan button names must map residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("Command_InitiateBattlePlanBombardment")
            && pf.contains("is_strategy_center_template"),
        "Strategy Center strip must expose battle plans residual"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("BattlePlanBombardment") && eng.contains("BattlePlanHoldTheLine"),
        "engine must execute battle plans without map-click residual"
    );
}

#[test]
fn named_superweapon_button_residual() {
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("spysatellitescan")
            && cs.contains("ciaintelligence")
            && cs.contains("particlecannon")
            && cs.contains("nuclearmissile")
            && cs.contains("scudstorm")
            && cs.contains("carpetbomb")
            && cs.contains("artillerybarrage")
            && cs.contains("emergencyrepair")
            && cs.contains("airstrike")
            && cs.contains("ambush")
            && cs.contains("sneakattack")
            && cs.contains("leafletdrop")
            && cs.contains("gpsscrambler")
            && cs.contains("spectregunship")
            && cs.contains("anthraxbomb"),
        "named SW button names must map residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("Command_FireParticleUplinkCannon")
            && pf.contains("Command_SpySatelliteScan")
            && pf.contains("Command_CIAIntelligence")
            && pf.contains("Command_LeafletDrop")
            && pf.contains("Command_SpectreGunship")
            && pf.contains("Command_A10ThunderboltMissileStrike"),
        "SW structures must expose CommandSet residual buttons"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("Pass 1: honor named"),
        "engine must prefer named SW type when arming residual"
    );
}

#[test]
fn damaged_structure_cycle_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn cycle_damaged_structure_selection")
            && src.contains("No damaged structures")
            && src.contains("NamedKey::Alt")
            && src.contains("cycle_damaged_structure_selection(1)"),
        "Ctrl+Alt+arrows must cycle damaged structures residual"
    );
}

#[test]
fn idle_military_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_all_idle_military")
            && src.contains("eq_ignore_ascii_case(\"i\")")
            && src.contains("select_all_idle_military()")
            && !src.contains("No idle military units"),
        "Ctrl+I must select idle military residual without HUD toast"
    );
}

#[test]
fn unit_attitude_hotkey_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_AttitudeAggressive")
            && src.contains("Command_AttitudeSleep")
            && src.contains("Command_AttitudePassive")
            && src.contains("NamedKey::Alt"),
        "Alt+A/S/D must set unit attitude residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("AttitudeAggressive")
            && cs.contains("AttitudeSleep")
            && cs.contains("\"aggressive\""),
        "attitude commands must map residual"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_set_attitude") && ex.contains("set_ai_attitude"),
        "execute_set_attitude residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("Command_AttitudeAggressive") && pf.contains("Command_AttitudeSleep"),
        "strip must expose attitude residual"
    );
}

#[test]
fn generals_science_purchase_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let fn_idx = src
        .find("fn try_purchase_next_generals_science")
        .expect("try_purchase_next_generals_science");
    let body = &src[fn_idx..src.len().min(fn_idx + 700)];
    assert!(
        src.contains("PurchaseScience")
            && src.contains("eq_ignore_ascii_case(\"g\")")
            && src.contains("NamedKey::Alt")
            && body.contains("toggle_purchase_science"),
        "Alt+G / empty-name PurchaseScience must toggle the promotion screen"
    );
    assert!(
        !body.contains("first_capable_purchase_science_residual")
            && !body.contains("Purchased ")
            && !src.contains("No science purchase points"),
        "C++ populatePurchaseScience / togglePurchaseScience never auto-buys"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("local_science_purchase_points") && pf.contains("local_has_science"),
        "strip must expose PurchaseScience when SPP residual"
    );
}

#[test]
fn wall_line_drag_placement_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn is_wall_structure_template")
            && src.contains("fn place_wall_line_from_ui")
            && src.contains("DozerConstructLine"),
        "wall/fence drag must issue DozerConstructLine residual"
    );
    assert!(
        !src.contains("Wall line ordered")
            && !src.contains("Select a dozer or worker to build wall"),
        "hq-8955d: wall place/success must not invent HUD toasts"
    );
}

#[test]
fn detonate_and_harvester_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("eq_ignore_ascii_case(\"n\")")
            && src.contains("Command_DetonateRemoteDemoCharges"),
        "N must detonate remote charges residual"
    );
    assert!(
        src.contains("fn select_all_harvesters")
            && src.contains("select_all_harvesters()")
            && !src.contains("No harvesters found"),
        "Ctrl+Shift+I must select harvesters residual without HUD toast"
    );
}

#[test]
fn switch_weapons_and_demo_suicide_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_SwitchWeapons")
            && src.contains("eq_ignore_ascii_case(\"w\")")
            && src.contains("NamedKey::Alt"),
        "Alt+W must SwitchWeapons residual"
    );
    assert!(
        src.contains("Command_DemoTertiarySuicide") && src.contains("eq_ignore_ascii_case(\"b\")"),
        "Alt+B must DemoTertiarySuicide residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("\"switchweapons\"") || cs.contains("SwitchWeapons"),
        "switchweapons button map residual"
    );
}

#[test]
fn delete_cancel_production_and_combat_drop_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn cancel_selected_production_queue_head")
            && src.contains("NamedKey::Delete"),
        "Delete must cancel production queue head residual"
    );
    assert!(
        !src.contains("Canceled production"),
        "hq-8955d: Delete cancel must not invent a HUD toast"
    );
    assert!(
        src.contains("PendingMapCommand::CombatDrop")
            && src.contains("Command_CombatDrop")
            && src.contains("arm_radius_cursor_for_pending(\"COMBATDROP\")"),
        "Alt+C / CombatDrop must arm map click residual"
    );
}

#[test]
fn hack_internet_and_cleanup_area_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_HackInternet")
            && src.contains("eq_ignore_ascii_case(\"i\")")
            && src.contains("NamedKey::Alt"),
        "Alt+I must HackInternet residual"
    );
    assert!(
        src.contains("Command_CleanupArea") && src.contains("eq_ignore_ascii_case(\"m\")"),
        "Alt+M must CleanupArea residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("HackInternet") && cs.contains("\"hackinternet\""),
        "HackInternet command map residual"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_hack_internet") && ex.contains("start_hacker_internet_hack"),
        "execute_hack_internet residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("Command_ChinaInfantryHackerInternetHack")
            && pf.contains("Command_AmbulanceCleanupArea"),
        "strip must expose hack/cleanup residual (retail CommandSet slots)"
    );
}

#[test]
fn return_to_base_aircraft_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_ReturnToBase")
            && src.contains("eq_ignore_ascii_case(\"r\")")
            && src.contains("NamedKey::Alt"),
        "Alt+R must ReturnToBase residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("ReturnToBase") && cs.contains("\"returntobase\""),
        "ReturnToBase command map residual"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_return_to_base")
            && ex.contains("request_return_to_base")
            && ex.contains("producer-first ParkingPlace reservation")
            && !ex.contains("self.execute_dock(&[unit_id], airfield_id)"),
        "execute_return_to_base must use authoritative producer-first parking, not generic Dock"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("Command_ReturnToBase"),
        "aircraft strip must expose RTB residual"
    );
}

#[test]
fn on_screen_select_and_camera_follow_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_all_friendly_on_screen")
            && src.contains("select_all_friendly_on_screen()")
            && !src.contains("No units on screen"),
        "Ctrl+Alt+A must select on-screen friendlies residual without HUD toast"
    );
    assert!(
        src.contains("fn toggle_camera_follow_selection")
            && !src.contains("Camera follow on")
            && src.contains("eq_ignore_ascii_case(\"f\")"),
        "Alt+F must toggle camera follow residual without HUD toast"
    );
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    assert!(
        gl.contains("fn set_camera_follow_object") && gl.contains("fn camera_follow_object_id"),
        "GameLogic camera follow API residual"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn alive_selectable_friendly_near"),
        "presentation near-select residual"
    );
}

#[test]
fn return_supplies_and_select_structures_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_ReturnSupplies")
            && src.contains("eq_ignore_ascii_case(\"u\")")
            && src.contains("NamedKey::Alt"),
        "Alt+U must ReturnSupplies residual"
    );
    assert!(
        src.contains("fn select_all_friendly_structures")
            && src.contains("select_all_friendly_structures()")
            && !src.contains("No structures found"),
        "Ctrl+Alt+S must select all structures residual without HUD toast"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("ReturnSupplies") && cs.contains("\"returnsupplies\""),
        "ReturnSupplies command map residual"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_return_supplies") && ex.contains("ReturningResources"),
        "execute_return_supplies residual"
    );
}

#[test]
fn clear_mines_and_unfinished_construction_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("Command_ClearMines")
            && src.contains("eq_ignore_ascii_case(\"x\")")
            && src.contains("NamedKey::Alt"),
        "Alt+X must ClearMines residual"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("ClearMines") && cs.contains("\"clearmines\""),
        "ClearMines command map residual"
    );
    let ex = crate::command_executor::COMMAND_EXECUTOR_SRC;
    assert!(
        ex.contains("fn execute_clear_mines") && ex.contains("is_mine_clearer"),
        "execute_clear_mines residual"
    );
}

#[test]
fn resume_construction_hotkey_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn resume_selected_construction")
            && src.contains("eq_ignore_ascii_case(\"e\")")
            && src.contains("NamedKey::Alt")
            && !src.contains("Resuming construction"),
        "Alt+E must resume construction residual without HUD toast"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    // Retail ControlBar has NO resume strip button: populateUnderConstruction
    // (ControlBarUnderConstruction.cpp:57-66) shows Command_CancelConstruction
    // only; resume is the ACTIONTYPE_RESUME_CONSTRUCTION cursor/click surface
    // (InGameUI.h:318) dispatched as MSG_RESUME_CONSTRUCTION
    // (GameLogicDispatch.cpp:1135-1147) — pinned engine-side above and by the
    // executor MSG residual. The strip arm must stay Cancel-only.
    assert!(
        pf.contains("Command_CancelConstruction") && !pf.contains("Command_ResumeConstruction"),
        "unfinished structure strip stays Cancel-only per retail populateUnderConstruction"
    );
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        cs.contains("\"resumeconstruction\"") || cs.contains("ResumeConstruction"),
        "resumeconstruction button map residual"
    );
}

#[test]
fn idle_harvesters_and_cancel_all_production_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_idle_harvesters")
            && src.contains("select_idle_harvesters()")
            && !src.contains("No idle harvesters"),
        "Ctrl+Alt+I must select idle harvesters residual without HUD toast"
    );
    assert!(
        src.contains("fn cancel_all_selected_production") && src.contains("ctrl_down && !shift"),
        "Ctrl+Delete must cancel all production residual"
    );
    assert!(
        !src.contains("Canceled all production")
            && !src.contains("Select a valid target")
            && !src.contains("Cancelled pending command"),
        "hq-0foy9: cancel/reject must not invent English HUD toasts"
    );
}

#[test]
fn guard_radius_and_combat_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn adjust_selected_guard_radius")
            && !src.contains("Guard radius:")
            && src.contains("adjust_selected_guard_radius(15.0)"),
        "Alt+[ ] must adjust guard radius residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_friendly_combat")
            && src.contains("select_all_friendly_combat()")
            && !src.contains("No combat units"),
        "Ctrl+Alt+Q must select combat units residual without HUD toast"
    );
}

#[test]
fn clear_path_and_damaged_unit_cycle_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn clear_selected_path_waypoints")
            && !src.contains("Path cleared")
            && src.contains("clear_selected_path_waypoints()"),
        "Alt+Z must clear path waypoints residual without HUD toast"
    );
    assert!(
        src.contains("fn cycle_damaged_unit_selection")
            && !src.contains("Damaged unit selected")
            && src.contains("cycle_damaged_unit_selection(1)"),
        "Ctrl+Alt+Up/Down must cycle damaged units residual without HUD toast"
    );
}

#[test]
fn moving_select_and_health_bars_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_all_friendly_moving")
            && src.contains("select_all_friendly_moving()")
            && !src.contains("No moving units"),
        "Ctrl+Alt+M must select moving units residual without HUD toast"
    );
    assert!(
        src.contains("fn toggle_health_bars_hotkey")
            && !src.contains("Health bars: ON")
            && src.contains("show_health_bars"),
        "Alt+H must toggle health bars residual without HUD toast"
    );
}

#[test]
fn attacking_select_and_stop_all_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_all_friendly_attacking")
            && src.contains("select_all_friendly_attacking()")
            && !src.contains("No attacking units"),
        "Ctrl+Alt+T must select attacking units residual without HUD toast"
    );
    assert!(
        src.contains("fn stop_all_friendly_units")
            && src.contains("stop_all_friendly_units()")
            && !src.contains("No units to stop"),
        "Ctrl+Shift+. must stop all friendlies residual without HUD toast"
    );
}

fn debug_producer_and_guarding_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn toggle_debug_info_hotkey") && !src.contains("Debug overlay: ON"),
        "debug overlay residual helper must remain (not bound to retail Ctrl+F1) without HUD toast"
    );
    assert!(
        src.contains("fn cycle_busy_producer_selection")
            && !src.contains("Busy producer selected")
            && src.contains("cycle_busy_producer_selection(1)"),
        "Ctrl+Alt+P must cycle busy producers residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_friendly_guarding")
            && src.contains("select_all_friendly_guarding()")
            && !src.contains("No guarding units"),
        "Ctrl+Alt+G must select guarding units residual without HUD toast"
    );
}

#[test]
fn center_selection_and_constructing_workers_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        !src.contains("fn center_camera_on_selection")
            && !src.contains("Centered on selection")
            && src.contains("C++ has no Alt+Space center binding")
            && src.contains("NamedKey::Space")
            && src.contains("Command_ViewLastRadarEvent"),
        "Space is VIEW_LAST_RADAR_EVENT; C++ has no Alt+Space center"
    );
    assert!(
        src.contains("fn select_all_constructing_workers")
            && src.contains("select_all_constructing_workers()")
            && !src.contains("No constructing workers"),
        "Ctrl+Alt+B must select constructing workers residual without HUD toast"
    );
}

#[test]
fn idle_military_cycle_and_repairing_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn cycle_idle_military_selection")
            && src.contains("cycle_idle_military_selection(1)")
            && !src.contains("Idle military selected"),
        "Ctrl+Alt+,/. must cycle idle military residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_repairing_units")
            && src.contains("select_all_repairing_units()")
            && !src.contains("No repairing units"),
        "Ctrl+Alt+R must select repairing units residual without HUD toast"
    );
}

#[test]
fn hq_0djue_hotkey_selection_residuals_must_not_invent_english_hud_toasts() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for toast in [
        "Force-attack ground",
        "Waypoint mode: ON",
        "Waypoint mode: OFF",
        "Auto-attack: ON",
        "Auto-attack: OFF",
        "Chat (All)",
        "Chat (Allies)",
        "Chat (Whisper)",
        "Screenshot:",
        "Screenshot failed:",
        "Diplomacy panel opened",
        "Diplomacy panel closed",
        "No unfinished construction to resume",
        "No dozer/worker available to resume",
        "Resuming construction",
        "Unfinished construction selected",
        "Damaged structure selected",
        "Idle military selected",
        "No repairing units",
        "Selected {} repairing",
        "No idle military units",
        "Selected {} idle military",
    ] {
        assert!(
            !src.contains(toast),
            "hq-0djue: invented English HUD toast must stay gone: {toast}"
        );
    }
}

#[test]
fn hq_siwjj_selection_residuals_must_not_invent_english_hud_toasts() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for toast in [
        "No harvesters found",
        "Selected {} harvesters",
        "No idle harvesters",
        "Selected {} idle harvesters",
        "Construction tab:",
        "No structures found",
        "Selected {} structures",
        "Path cleared",
        "Damaged unit selected",
        "Guard radius:",
    ] {
        assert!(
            !src.contains(toast),
            "hq-siwjj: invented English HUD toast must stay gone: {toast}"
        );
    }
}

#[test]
fn hq_43rto_selection_residuals_must_not_invent_english_hud_toasts() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for toast in [
        "No attacking units",
        "Selected {} attacking",
        "No units to stop",
        "Stopped {} units",
        "No moving units",
        "Selected {} moving",
        "No occupied transports",
        "Selected {} occupied transports",
        "Attack lines: ON",
        "Attack lines: OFF",
        "Move lines: ON",
        "Move lines: OFF",
        "No garrisoned structures",
        "Selected {} garrisoned",
        "FPS counter: ON",
        "FPS counter: OFF",
        "No control groups",
        "Control group {group_num} empty",
        "No stealthed units",
        "Selected {} stealthed",
        "No veteran units",
        "Selected {} veterans",
        "No docked aircraft",
        "Selected {} docked aircraft",
        "Debug overlay: ON",
        "Debug overlay: OFF",
        "Busy producer selected",
        "Ready special power selected",
        "No patrolling units",
        "Selected {} patrolling",
        "No gathering units",
        "Selected {} gathering",
        "No guarding units",
        "Selected {} guarding",
        "No combat units",
        "Selected {} combat units",
        "No units on screen",
        "Selected {} on screen",
        "No constructing workers",
        "Selected {} constructing",
        "Camera follow off",
        "Camera follow on",
        "Select a unit to follow",
        "Health bars: ON",
        "Health bars: OFF",
        "Control group {group_num}",
    ] {
        assert!(
            !src.contains(toast),
            "hq-43rto: invented English HUD toast must stay gone: {toast}"
        );
    }
}

#[test]
fn patrol_gather_and_ready_sw_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn select_all_friendly_patrolling")
            && src.contains("select_all_friendly_patrolling()")
            && !src.contains("No patrolling units"),
        "Ctrl+Alt+Y must select patrolling residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_friendly_gathering")
            && src.contains("select_all_friendly_gathering()")
            && !src.contains("No gathering units"),
        "Ctrl+Alt+H must select gathering residual without HUD toast"
    );
    assert!(
        src.contains("fn cycle_ready_special_power_structure")
            && !src.contains("Ready special power selected")
            && src.contains("cycle_ready_special_power_structure(1)"),
        "Ctrl+Alt+V must cycle ready SW residual without HUD toast"
    );
}

#[test]
fn fps_veterans_and_docked_aircraft_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn toggle_fps_counter_hotkey")
            && !src.contains("FPS counter: ON")
            && src.contains("self.show_fps = !self.show_fps"),
        "Ctrl+F2 FPS residual helper must remain without HUD toast"
    );
    assert!(
        src.contains("fn select_all_friendly_veterans")
            && !src.contains("No veteran units")
            && src.contains("select_all_friendly_veterans()"),
        "Ctrl+Alt+E must select veterans residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_docked_aircraft")
            && !src.contains("No docked aircraft")
            && src.contains("select_all_docked_aircraft()"),
        "Ctrl+Alt+W must select docked aircraft residual without HUD toast"
    );
}

#[test]
fn control_group_cycle_and_stealth_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn cycle_control_group_selection")
            && src.contains("cycle_control_group_selection")
            && !src.contains("No control groups"),
        "Ctrl+Shift+Tab must cycle control groups residual without HUD toast"
    );
    assert!(
        src.contains("fn select_all_friendly_stealthed")
            && src.contains("select_all_friendly_stealthed()")
            && !src.contains("No stealthed units"),
        "Ctrl+Alt+K must select stealthed residual without HUD toast"
    );
}

#[test]
fn move_lines_and_garrisoned_select_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn toggle_move_lines_hotkey")
            && !src.contains("Move lines: ON")
            && src.contains("show_move_lines")
            && src.contains("self.show_move_lines,"),
        "Ctrl+F3 move-lines residual helper must remain without HUD toast"
    );
    assert!(
        src.contains("fn select_all_garrisoned_structures")
            && !src.contains("No garrisoned structures")
            && src.contains("select_all_garrisoned_structures()"),
        "Ctrl+Alt+U must select garrisoned structures residual without HUD toast"
    );
}

#[test]
fn runtime_host_construct_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("dozer_construct") && src.contains("construct_ok:"),
        "runtime host must expose construct/dozer_construct residual"
    );
    assert!(
        src.contains("construct_fail_no_dozer")
            && src.contains("construct_fail_lbc:")
            && src.contains("place_structure_from_ui"),
        "construct residual must legal-build scan + place_structure_from_ui"
    );
}

#[test]
fn runtime_host_train_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("train_unit") && src.contains("train_ok:"),
        "runtime host must expose train_unit residual"
    );
    assert!(
        src.contains("train_fail_no_producer")
            && src.contains("under_construction")
            && src.contains("enqueue_production"),
        "train residual must complete unfinished barracks and enqueue production"
    );
}

#[test]
fn runtime_host_save_load_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("\"save_game\" | \"quicksave\"")
            || (src.contains("save_ok:") && src.contains("quicksave")),
        "runtime host must expose save_game/quicksave residual"
    );
    assert!(
        src.contains("quickload")
            && src.contains("load_ok:quicksave")
            && src.contains("save_game_from_ui"),
        "runtime host must expose quickload residual"
    );
}

#[test]
fn runtime_host_stop_sell_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("stop_all") && src.contains("stop_ok:"),
        "runtime host must expose stop_all residual"
    );
    assert!(
        src.contains("sell_selected") && src.contains("sell_ok:") && src.contains("Command_Sell"),
        "runtime host must expose sell residual"
    );
}

#[test]
fn runtime_host_upgrade_guard_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("queue_upgrade") && src.contains("upgrade_ok:"),
        "runtime host must expose upgrade residual"
    );
    assert!(
        src.contains("guard_position") && src.contains("guard_ok:"),
        "runtime host must expose guard residual"
    );
}

#[test]
fn runtime_host_attack_move_scatter_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("attack_move") && src.contains("attack_move_ok:"),
        "runtime host must expose attack_move residual"
    );
    assert!(
        src.contains("\"scatter\"")
            && src.contains("scatter_ok:")
            && src.contains("Command_Scatter"),
        "runtime host must expose scatter residual"
    );
}

#[test]
fn runtime_host_patrol_deploy_formation_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("\"patrol\"") && src.contains("patrol_ok:"),
        "runtime host must expose patrol residual"
    );
    assert!(
        src.contains("\"deploy\"") && src.contains("deploy_ok:"),
        "runtime host must expose deploy residual"
    );
    assert!(
        src.contains("\"cheer\"") && src.contains("cheer_ok"),
        "runtime host must expose cheer residual"
    );
    assert!(
        src.contains("create_formation") && src.contains("formation_ok:"),
        "runtime host must expose formation residual"
    );
}

#[test]
fn runtime_host_capture_economy_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("capture_building") && src.contains("capture_ok:"),
        "runtime host must expose capture residual"
    );
    assert!(
        src.contains("return_supplies") && src.contains("return_supplies_ok:"),
        "runtime host must expose return_supplies residual"
    );
    assert!(
        src.contains("\"evacuate\"") && src.contains("evacuate_ok:"),
        "runtime host must expose evacuate residual"
    );
    assert!(
        src.contains("\"repair\"") && src.contains("repair_ok:"),
        "runtime host must expose repair residual"
    );
    assert!(
        src.contains("return_to_base") && src.contains("return_to_base_ok:"),
        "runtime host must expose return_to_base residual"
    );
}

#[test]
fn runtime_host_misc_command_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("attitude_aggressive") && src.contains("attitude_ok:aggressive"),
        "runtime host must expose attitude residuals"
    );
    assert!(
        src.contains("set_rally") && src.contains("rally_ok:"),
        "runtime host must expose set_rally residual"
    );
    assert!(
        src.contains("switch_weapons") && src.contains("switch_weapons_ok:"),
        "runtime host must expose switch_weapons residual"
    );
    assert!(
        src.contains("view_command_center") && src.contains("view_cc_ok"),
        "runtime host must expose view_command_center residual"
    );
    assert!(
        src.contains("clear_mines") && src.contains("clear_mines_ok:"),
        "runtime host must expose clear_mines residual"
    );
    assert!(
        src.contains("place_beacon") && src.contains("beacon_ok:"),
        "runtime host must expose place_beacon residual"
    );
}

#[test]
fn runtime_host_special_named_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "hack_internet",
        "hack_ok:",
        "cleanup_area",
        "cleanup_ok:",
        "combat_drop",
        "combat_drop_ok:",
        "toggle_overcharge",
        "overcharge_ok",
        "do_special_power",
        "special_power_ok",
        "remove_beacon",
        "remove_beacon_ok",
        "demo_suicide",
        "demo_suicide_ok",
        "detonate_remote",
        "detonate_remote_ok",
        "view_last_radar",
        "view_radar_ok",
    ] {
        assert!(src.contains(needle), "missing host residual {needle}");
    }
}

#[test]
fn runtime_host_force_select_group_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "force_attack",
        "force_attack_ok:",
        "ForceAttackGround",
        "force_attack_object",
        "force_attack_object_ok:",
        "ForceAttackObject",
        "select_all",
        "select_all_ok:",
        "select_all_combat",
        "select_all_combat_ok:",
        "assign_control_group",
        "control_group_assign_ok:",
        "recall_control_group",
        "control_group_recall_ok:",
    ] {
        assert!(src.contains(needle), "missing host residual {needle}");
    }
}

#[test]
fn runtime_host_waypoint_box_presentation_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "waypoint_mode",
        "waypoint_mode_ok:",
        "add_waypoint",
        "waypoint_ok:",
        "AddWaypoint",
        "box_select",
        "box_select_ok:",
        "presentation_frame_ok",
        "presentation_live_fallback_reads",
        "last_presentation_live_fallback_reads",
    ] {
        assert!(src.contains(needle), "missing residual {needle}");
    }
}

#[test]
fn runtime_host_selection_filter_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "select_similar",
        "select_similar_ok:",
        "select_on_screen",
        "select_on_screen_ok:",
        "select_aircraft",
        "select_aircraft_ok:",
        "select_idle_harvesters",
        "select_idle_ok:",
        "select_structures",
        "select_structures_ok:",
        "select_moving",
        "select_moving_ok:",
    ] {
        assert!(src.contains(needle), "missing selection residual {needle}");
    }
}

#[test]
fn runtime_host_camera_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "camera_reset",
        "camera_reset_ok:",
        "reset_camera_view_hotkey",
        "camera_look_at",
        "camera_look_ok:",
        "camera_zoom",
        "camera_zoom_ok:",
        "camera_track",
        "camera_track_ok:",
    ] {
        assert!(src.contains(needle), "missing camera residual {needle}");
    }
}

#[test]
fn runtime_host_pause_cancel_diplomacy_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "pause_ok:paused",
        "pause_ok:resumed",
        "cancel_production",
        "cancel_production_ok:",
        "cancel_selected_production_queue_head",
        "open_diplomacy",
        "diplomacy_ok",
    ] {
        assert!(src.contains(needle), "missing residual {needle}");
    }
}

#[test]
fn runtime_host_live_frame_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let runtime = include_str!("runtime.rs");
    let src = format!("{src}\n{runtime}");
    assert!(
        src.contains("live_frame_ok")
            && src.contains("has_published_live_frame")
            && src.contains("png_file_looks_usable")
            && src.contains("window_visible")
            && src.contains("wnd_widget_tree_nav")
            && src.contains("wnd_menu_to_match_complete"),
        "runtime host must publish live_frame_ok / window_visible / physical WND menu-to-match honesty"
    );
    let status_fn = src.find("fn publish_status").expect("publish_status");
    let status_end = src[status_fn..]
        .find("fn publish_frame")
        .map(|i| status_fn + i)
        .unwrap_or(status_fn + 3_200);
    let status_body = &src[status_fn..status_end];
    assert!(
        status_body.contains("live_frame_ok_from_windowed_present")
            && status_body.contains("has_published_live_frame"),
        "live_frame_ok must stay capture-promoted or windowed present, not fallback PNG"
    );
    assert!(
        src.contains("note_windowed_surface_presented")
            && src.contains("has_published_live_frame = true"),
        "windowed surface present must also latch has_published_live_frame"
    );
    // Call-graph residual (not mere symbol co-presence): winit_menu_nav body must
    // call named-gadget inject and must not forge note_menu_wnd_click.
    let shell = include_str!("runtime_host/shell_core.rs");
    let nav_i = shell
        .find("fn runtime_host_cmd_winit_menu_nav")
        .expect("winit_menu_nav");
    let nav_end = shell[nav_i..]
        .find("fn runtime_host_cmd_winit_gameplay_order")
        .map(|i| nav_i + i)
        .unwrap_or(nav_i + 2500);
    let nav_body = &shell[nav_i..nav_end];
    assert!(
        nav_body.contains("inject_winit_equivalent_named_gadget_click")
            && !nav_body.contains("note_menu_wnd_click"),
        "winit_menu_nav must inject named gadgets, not forge note_menu_wnd_click"
    );
    assert!(
        nav_body.contains("ButtonSinglePlayer")
            && nav_body.contains("ButtonSkirmish")
            && !nav_body.contains("MainMenuParent")
            && !nav_body.contains("MainMenuRuler"),
        "winit_menu_nav must be menu→match gadgets only (not Parent/Ruler)"
    );
    let gp_i = shell
        .find("fn runtime_host_cmd_winit_gameplay_order")
        .expect("winit_gameplay_order");
    let gp_body = &shell[gp_i..shell.len().min(gp_i + 1800)];
    assert!(
        gp_body.contains("inject_winit_equivalent_gameplay_order_click")
            && !gp_body.contains("note_gameplay_order"),
        "winit_gameplay_order must inject RMB, not forge note_gameplay_order"
    );
    assert!(
        src.contains("handle_mouse_button_input")
            && src.contains("inject_winit_equivalent_named_gadget_click"),
        "windowed sit-through must share handle_mouse_button_input with inject"
    );
    // Honesty: under-cursor hit residual + no skip-WM forge parameters.
    let input = include_str!("input.rs");
    assert!(
        input.contains("get_window_under_cursor"),
        "named gadget inject must under-cursor hit-test"
    );
    assert!(
        !input.contains("skip_wm_dispatch") && !input.contains("preverified_gadget_hit"),
        "inject path must not forge wnd_used/hit via skip_wm / preverified"
    );
    assert!(
        input.contains("handle_right_click"),
        "RMB evidence path must call handle_right_click"
    );
    assert!(
        status_body.contains("retail_sit_through_missing"),
        "publish_status must list which of the five sit-through flags are still false"
    );
    assert!(
        status_body.contains("ingame=") && status_body.contains("gameplay="),
        "publish_status must print ingame/gameplay sit-through flags explicitly"
    );
    assert!(
        !status_body.contains("png_file_looks_usable"),
        "publish_status must not claim live_frame_ok from fallback frame.png"
    );
    let i = src.find("fn publish_runtime").expect("publish_runtime");
    let w = &src[i..src.len().min(i + 350)];
    let frame_i = w.find("publish_frame").expect("publish_frame");
    let status_i = w.find("publish_status").expect("publish_status");
    assert!(
        frame_i < status_i,
        "publish_frame must run before publish_status for live_frame_ok"
    );
}

#[test]
fn runtime_host_auto_attack_menu_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "auto_attack",
        "auto_attack_ok:on",
        "auto_attack_ok:off",
        "sticky_auto_attack",
        "quit_to_menu",
        "menu_ok",
        "options_ok",
    ] {
        assert!(src.contains(needle), "missing residual {needle}");
    }
}

#[test]
fn runtime_host_options_probe_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("options_probe") && src.contains("options_probe_ok"),
        "runtime host must expose options_probe residual that stays InGame"
    );
}

#[test]
fn runtime_host_request_capture_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    for needle in [
        "request_capture",
        "request_capture_ok",
        "runtime_host_pending_capture",
        "take_runtime_host_pending_capture",
        "force_capture_request",
        "pending_capture",
    ] {
        assert!(src.contains(needle), "missing residual {needle}");
    }
}

#[test]
fn attack_lines_and_occupied_transports_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains("fn toggle_attack_lines_hotkey")
            && !src.contains("Attack lines: ON")
            && src.contains("show_attack_lines")
            && src.contains("self.show_attack_lines,"),
        "Ctrl+F4 attack-lines residual helper must remain without HUD toast"
    );
    assert!(
        src.contains("fn select_all_occupied_transports")
            && !src.contains("No occupied transports")
            && src.contains("select_all_occupied_transports()"),
        "Ctrl+Alt+J must select occupied transports residual without HUD toast"
    );
}

#[test]
fn menu_transition_applies_product_title_and_tears_down_loading_overlay() {
    // C++ GameText.cpp:347 SetWindowText(ApplicationHWnd, ourName).
    // C++ MainMenu.cpp:528-530 initialHide + showSelectiveButtons(SHOW_NONE)
    // tears down load chrome before the shell menu; first-run only hides the
    // mouse and reverse-fades. Boot writes
    // "Command & Conquer Generals Zero Hour - Loading {phase} (92%)" from
    // update_startup_loading; timeout Loading→Menu (camera_drain
    // startup_load_should_release_to_menu) never reached
    // host_finalize_startup_map_load, so the title and ShellGame overlay
    // stayed up. The live Menu enter must own chrome itself.
    let transition = include_str!("input.rs");
    let start = transition
        .find("pub(super) fn host_transition_to_state")
        .expect("host_transition_to_state");
    let body = &transition[start..];
    let menu_arm = body
        .split("Entering Menu state — transition_to_state start")
        .nth(1)
        .and_then(|s| s.split("GameState::Loading => {").next())
        .expect("Menu enter arm");
    assert!(
        menu_arm.contains("self.apply_shell_menu_window_chrome()"),
        "Menu enter must apply product title + overlay teardown, not wait for finalize_startup: {menu_arm}"
    );

    let shell = include_str!("shell.rs");
    let chrome = shell
        .split("fn apply_shell_menu_window_chrome")
        .nth(1)
        .and_then(|s| s.split("fn show_shell_menu").next())
        .expect("apply_shell_menu_window_chrome");
    assert!(
        chrome.contains("hide_shell_loading_overlay()")
            && chrome.contains("set_title(SHELL_MENU_WINDOW_TITLE)"),
        "Menu chrome must hide the load screen and set the product title: {chrome}"
    );

    let types = include_str!("types.rs");
    assert!(
        types.contains("SHELL_MENU_WINDOW_TITLE: &str = \"Command & Conquer Generals Zero Hour\""),
        "product window title must stay Command & Conquer Generals Zero Hour"
    );
}

#[test]
fn menu_to_menu_does_not_hide_shell_or_shutdown_main_menu() {
    // C++ GameLogic.cpp:2195-2203 GAME_SHELL finish never hideShell /
    // MainMenuShutdown. Menu→Menu after Loaded startup shell map must keep
    // MainMenu.wnd (hq-3vo4 / hq-pzya).
    let transition = include_str!("input.rs");
    let start = transition
        .find("pub(super) fn host_transition_to_state")
        .expect("host_transition_to_state");
    let body = &transition[start..];
    let exit_arm = body
        .split("GameState::Menu => {")
        .nth(1)
        .and_then(|s| s.split("GameState::Loading => {").next())
        .expect("Menu exit arm");
    assert!(
        exit_arm.contains("if new_state != GameState::Menu"),
        "Menu→Menu must skip hide_shell_menu: {exit_arm}"
    );
    assert!(
        !exit_arm.contains("self.hide_shell_menu();")
            || exit_arm.contains("if new_state != GameState::Menu"),
        "unconditional hide_shell_menu on Menu exit tears down MainMenu: {exit_arm}"
    );

    let shell = include_str!("shell.rs");
    let show = shell
        .split("pub(super) fn show_shell_menu")
        .nth(1)
        .and_then(|s| s.split("pub(super) fn hide_shell_menu").next())
        .expect("show_shell_menu");
    assert!(
        !show.contains("if self.shell_menu_active"),
        "show_shell_menu must not early-return on stale shell_menu_active: {show}"
    );
    assert!(
        show.contains("get_screen_count()") && show.contains("Menus/MainMenu.wnd"),
        "show_shell_menu must inspect the live stack like C++ startNewGame: {show}"
    );
}

#[cfg(test)]
mod world_scene_skip_residual_tests {
    #[test]
    fn ingame_does_not_skip_world_for_menu_warmup_counter() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        let start = src
            .find("fn should_skip_world_scene_for_shell_menu")
            .expect("skip fn");
        let body = &src[start..src.len().min(start + 900)];
        assert!(
            body.contains("GameState::Loading"),
            "only Loading may skip the 3D world"
        );
        assert!(
            !body.contains("GameState::Menu => self.menu_world_frames_rendered"),
            "Menu must keep drawing the shell-map world (C++ W3DDisplay::draw)"
        );
    }
}

#[cfg(test)]
mod runtime_host_windowed_bridge_tests {
    use super::*;
    use crate::command_line::CommandLineArgs;

    #[test]
    fn windowed_runtime_host_is_requested_but_not_headless() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let control = dir.path().join("control.txt");
        let status = dir.path().join("status.txt");
        let frame = dir.path().join("frame.png");
        let args = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            "-runtime_host".into(),
            "windowed".into(),
            "-gpui_control".into(),
            control.to_string_lossy().into_owned(),
            "-gpui_status".into(),
            status.to_string_lossy().into_owned(),
            "-gpui_frame".into(),
            frame.to_string_lossy().into_owned(),
        ])
        .expect("parse");
        assert!(!RuntimeHostBridge::is_headless_mode(&args));
        assert!(RuntimeHostBridge::is_runtime_host_requested(&args));
        assert!(RuntimeHostBridge::from_command_line(&args).is_some());
        let headless = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            "-runtime_host".into(),
            "headless".into(),
            "-gpui_control".into(),
            control.to_string_lossy().into_owned(),
            "-gpui_status".into(),
            status.to_string_lossy().into_owned(),
            "-gpui_frame".into(),
            frame.to_string_lossy().into_owned(),
        ])
        .expect("parse headless");
        assert!(RuntimeHostBridge::is_headless_mode(&headless));
        assert!(RuntimeHostBridge::from_command_line(&headless).is_some());
    }

    #[test]
    fn hyphen_runtime_host_flags_construct_bridge() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let control = dir.path().join("control.txt");
        let status = dir.path().join("status.txt");
        let frame = dir.path().join("frame.png");
        let args = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            format!("--runtime-host=windowed"),
            format!("--gpui-control={}", control.display()),
            format!("--gpui-status={}", status.display()),
            format!("--gpui-frame={}", frame.display()),
        ])
        .expect("parse hyphen equals");
        assert!(!RuntimeHostBridge::is_headless_mode(&args));
        assert!(RuntimeHostBridge::is_runtime_host_requested(&args));
        assert!(
            RuntimeHostBridge::from_command_line(&args).is_some(),
            "hyphen --runtime-host/--gpui-* must construct RuntimeHostBridge"
        );
    }

    #[test]
    fn windowed_boot_status_uses_winit_query_not_hardcoded_false() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let control = dir.path().join("control.txt");
        let status = dir.path().join("status.txt");
        let frame = dir.path().join("frame.png");
        let args = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            "-runtime_host".into(),
            "windowed".into(),
            "-gpui_control".into(),
            control.to_string_lossy().into_owned(),
            "-gpui_status".into(),
            status.to_string_lossy().into_owned(),
            "-gpui_frame".into(),
            frame.to_string_lossy().into_owned(),
        ])
        .expect("parse");
        let mut bridge = RuntimeHostBridge::from_command_line(&args).expect("bridge");
        // No window yet: honest false.
        bridge.publish_booting_from_winit_query(false, Some(false));
        let hidden = std::fs::read_to_string(&status).expect("hidden status");
        assert!(
            hidden.contains("window_visible=false"),
            "hidden windowed boot must stay false: {hidden}"
        );
        assert!(
            hidden.contains("live_frame_ok=false"),
            "boot must not forge live_frame_ok: {hidden}"
        );
        // After set_visible(true) and winit reports Some(true).
        bridge.publish_booting_from_winit_query(false, Some(true));
        let shown = std::fs::read_to_string(&status).expect("shown status");
        assert!(
            shown.contains("window_visible=true"),
            "windowed boot must publish the honest winit residual: {shown}"
        );
        assert!(
            shown.contains("live_frame_ok=false"),
            "visibility must not forge live_frame_ok: {shown}"
        );
        // Headless stays hidden even if winit later reports shown.
        let headless_args = CommandLineArgs::parse_from_args(vec![
            "generals".into(),
            "-runtime_host".into(),
            "headless".into(),
            "-gpui_control".into(),
            control.to_string_lossy().into_owned(),
            "-gpui_status".into(),
            status.to_string_lossy().into_owned(),
            "-gpui_frame".into(),
            frame.to_string_lossy().into_owned(),
        ])
        .expect("parse headless");
        let mut headless = RuntimeHostBridge::from_command_line(&headless_args).expect("headless");
        headless.publish_booting_from_winit_query(true, Some(true));
        let headless_status = std::fs::read_to_string(&status).expect("headless status");
        assert!(
            headless_status.contains("window_visible=false"),
            "headless must stay hidden: {headless_status}"
        );
    }

    #[test]
    fn runtime_host_enabled_uses_active_not_only_headless() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        let start = src.find("fn runtime_host_enabled").expect("enabled");
        let end = src[start..]
            .find("fn runtime_host_window_visible")
            .map(|i| start + i)
            .unwrap_or(start + 120);
        let body = &src[start..end];
        assert!(
            body.contains("self.runtime_host_active"),
            "windowed runtime host must enable host cmds/status"
        );
        assert!(
            !body.contains("self.runtime_host_headless"),
            "enabled must not be headless-only"
        );
        assert!(src.contains("note_os_wnd_widget_tree_hit"));
    }

    #[test]
    fn windowed_runtime_host_publishes_visible_from_winit_is_visible() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        let start = src
            .find("fn runtime_host_window_visible")
            .expect("window_visible helper");
        let body = &src[start..src.len().min(start + 420)];
        assert!(
            body.contains("window_visible_from_winit_query")
                && body.contains("self.runtime_host_headless")
                && body.contains("is_visible()"),
            "window_visible must use shipped winit query helper (headless stays false)"
        );
        assert!(
            src.contains("window_visible: self.runtime_host_window_visible()"),
            "status snapshot must publish the honest winit visibility residual"
        );
        assert!(
            src.contains("live_frame_ok: false"),
            "snapshot live_frame_ok stays false; publish ORs a real capture"
        );
        assert!(
            src.contains("interactive_playability.wnd_menu_to_match_complete()"),
            "snapshot must publish physical WND menu-to-match evidence, not a singleton latch"
        );
        assert!(
            src.contains("retail_sit_through_missing"),
            "windowed status must list which sit-through flags are still false"
        );
        assert!(
            src.contains("fn publish_booting_from_winit_query")
                && src.contains("apply_runtime_host_window_visibility"),
            "boot residual must publish the honest winit query after set_visible"
        );
    }

    #[test]
    fn windowed_runtime_host_does_not_init_headless_ww3d() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        let headless_init = src
            .find("ww3d_engine::init_headless")
            .expect("init_headless");
        let window_init = src
            .find("ww3d_engine::init_with_window")
            .expect("init_with_window");
        assert!(window_init > headless_init);
        let gate = &src[headless_init.saturating_sub(200)..headless_init];
        assert!(
            gate.contains("if runtime_host_headless"),
            "init_headless must stay behind runtime_host_headless"
        );
        assert!(
            src[headless_init..window_init].contains("} else if"),
            "windowed runtime_host must not call init_headless"
        );
    }

    #[test]
    fn ui_render_pass_drops_write_lock_before_draw_all() {
        let src = include_str!("../graphics/ui_render_pass.rs");
        assert!(
            !src.contains("*mut UIRenderer"),
            "must not reintroduce TLS *mut UIRenderer"
        );
        let begin = src
            .find("renderer.begin_overlay_frame()")
            .expect("flush_ui_to_frame must begin an overlay frame");
        let draw = src
            .find("wm.draw_all()")
            .expect("flush_ui_to_frame must call draw_all");
        assert!(begin < draw);
        let between = &src[begin..draw];
        assert!(
            between.contains('}'),
            "UI write lock must drop before wm.draw_all()"
        );
        assert!(
            !between.contains("set_active_ui_renderer(Some"),
            "must not hold in-draw flag across draw_all"
        );
    }

    #[test]
    fn windowed_executable_smoke_construct_train_without_cheats() {
        let src = include_str!("../executable_smoke.rs");
        let windowed_writes = windowed_write_control_args(src);
        assert!(
            !windowed_writes.is_empty(),
            "must find Windowed write_control argument lists"
        );
        assert!(
            windowed_writes.contains("construct|template=USA_Barracks|auto_target=1"),
            "windowed construct write_control must be cheat-free: {windowed_writes}"
        );
        assert!(
            windowed_writes.contains("train_unit|template=AmericaInfantryRanger|auto_target=1"),
            "windowed train write_control must be cheat-free"
        );
        assert!(
            !windowed_writes.contains("train_unit|template=USA_Ranger|auto_target=1"),
            "windowed must issue one train template so train_ok is not overwritten"
        );
        assert!(
            src.contains("saw_construct_under_construction"),
            "windowed must wait for a real under_construction pulse before train"
        );
        assert!(
            include_str!("runtime_host/gameplay.rs").contains("train_fail_no_ready_barracks"),
            "infantry train must fail closed without a completed barracks"
        );
        assert!(
            windowed_writes
                .contains("upgrade|name=UpgradeAmericaRangerCaptureBuilding|auto_target=1"),
            "windowed upgrade write_control must be cheat-free"
        );
        for cheat in ["spawn_dozer=", "force_complete=", "grant_supplies="] {
            assert!(
                !windowed_writes.contains(cheat),
                "Windowed write_control arm must not contain {cheat}"
            );
        }
        assert!(src.contains("snap.under_construction == 0"));
        assert!(src.contains("spawn_dozer=1") && src.contains("force_complete=1"));
        assert!(
            !windowed_writes.contains("spawn_dozer="),
            "windowed construct command must have no spawn_dozer="
        );
        assert!(
            windowed_writes.contains("pause_save|slot=wnd_pause|via=PopupSaveLoad.wnd"),
            "windowed step 8 must drive Pause Save/Load WND: {windowed_writes}"
        );
        assert!(
            windowed_writes.contains("pause_load|slot=wnd_pause|via=PopupSaveLoad.wnd"),
            "windowed step 8 must drive Pause load WND"
        );
        assert!(
            !windowed_writes.contains("quicksave") && !windowed_writes.contains("quickload"),
            "windowed step 8 must not use host quicksave/quickload"
        );
    }

    /// Collect `write_control(...)` argument-list slices from Windowed launch arms only.
    fn windowed_write_control_args(src: &str) -> String {
        let marker = "if launch == ExecutableSmokeLaunch::Windowed";
        fn brace_block(after_if: &str) -> &str {
            let Some(open) = after_if.find('{') else {
                return "";
            };
            let mut depth = 0i32;
            for (i, ch) in after_if[open..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return &after_if[open..=open + i];
                        }
                    }
                    _ => {}
                }
            }
            &after_if[open..]
        }
        fn write_control_args(block: &str) -> String {
            let mut out = String::new();
            let mut rest = block;
            while let Some(i) = rest.find("write_control(") {
                let after = &rest[i + "write_control(".len()..];
                let mut depth = 1i32;
                let mut end = after.len();
                for (j, ch) in after.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = j;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                out.push_str(&after[..end]);
                out.push('\n');
                rest = &after[end..];
            }
            out
        }
        let mut windowed_writes = String::new();
        let mut rest = src;
        while let Some(i) = rest.find(marker) {
            let after = &rest[i + marker.len()..];
            windowed_writes.push_str(&write_control_args(brace_block(after)));
            rest = after;
        }
        windowed_writes
    }

    #[test]
    fn windowed_step_8_pause_save_load_uses_wnd_gadgets() {
        let host = include_str!("runtime_host/gameplay.rs");
        assert!(
            host.contains("fn runtime_host_cmd_pause_save"),
            "pause_save must be a shipped host command"
        );
        assert!(host.contains("fn runtime_host_cmd_pause_load"));
        let pause_save = {
            let i = host
                .find("fn runtime_host_cmd_pause_save")
                .expect("pause_save");
            &host[i..]
        };
        let pause_save = pause_save
            .split("fn runtime_host_cmd_pause_load")
            .next()
            .unwrap_or(pause_save);
        assert!(pause_save.contains("drive_os_wnd_quit_menu_save_load_like_cpp"));
        assert!(pause_save.contains("ensure_live_quit_menu_layout"));
        assert!(pause_save.contains("ensure_live_popup_save_load_layout"));
        assert!(pause_save.contains("drive_os_wnd_popup_save_load_save_like_cpp"));
        assert!(
            !pause_save.contains("simulate_quit_menu")
                && !pause_save.contains("simulate_save_load"),
            "pause_save must not call residual gadget latches"
        );
        assert!(host.contains("save_fail_wnd_missing"));
        assert!(host.contains("load_fail_wnd_missing"));
        assert!(
            host.contains("save_game_from_ui"),
            "WND save must call the live save_game_from_ui path"
        );
        assert!(
            host.contains("load_game_from_ui"),
            "WND load must call the live load_game_from_ui path"
        );
        let dispatch = include_str!("runtime_host/mod.rs");
        assert!(dispatch.contains("\"pause_save\""));
        assert!(dispatch.contains("\"pause_load\""));
    }

    #[test]
    fn windowed_open_skirmish_does_not_simulate_main_menu_skirmish_button() {
        let src = include_str!("runtime_host/skirmish.rs");
        let interactive = src
            .find("Interactive windowed: WND is the only menu owner")
            .or_else(|| src.find("// Interactive:"))
            .expect("interactive open_skirmish branch");
        let rest = &src[interactive..];
        let end = rest
            .find("pub(super) fn runtime_host_cmd_click_skirmish_start")
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            !body.contains("simulate_main_menu_skirmish_button_gadget_selected()"),
            "windowed path must not call simulate_main_menu_skirmish_button_*"
        );
        assert!(
            !body.contains("ui_manager.transition_to_screen(Screen::Skirmish)"),
            "windowed exclusive WND must not soft-transition to Screen::Skirmish"
        );
        assert!(
            body.contains("Menus/SkirmishGameOptionsMenu.wnd"),
            "interactive must still push/parse Skirmish WND"
        );
    }

    #[test]
    fn windowed_runtime_host_redraw_presents_gpu() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        assert!(
            src.contains("drive_frame(engine, current_window, &mut runtime_host_bridge, true)"),
            "windowed RedrawRequested must present (render_frame=true)"
        );
        assert!(
            src.contains("live_present_interval()"),
            "windowed AboutToWait must pace GPU presents from use_fps_limit + draw limiter"
        );
        assert!(
            src.contains("HEADLESS_PRESENT_INTERVAL"),
            "headless present interval must remain a separate path"
        );
    }

    #[test]
    fn windowed_present_cap_matches_cpp_default_max_fps_45() {
        // C++ GameEngine.h:13 `#define DEFAULT_MAX_FPS 45`
        // C++ GameEngine.cpp:271 `m_maxFPS = DEFAULT_MAX_FPS`
        // C++ GameEngine.cpp:856-857 execute cap:
        //   `DWORD limit = (1000.0f/m_maxFPS)-1`
        // Windowed WaitUntil present interval is 1/45 s; logic stays 30 Hz.
        use super::super::run_loop::{DEFAULT_MAX_FPS, FRAME_INTERVAL, HEADLESS_LOGIC_INTERVAL};
        use std::time::Duration;

        assert_eq!(DEFAULT_MAX_FPS, 45);
        assert_eq!(
            FRAME_INTERVAL,
            Duration::from_micros(1_000_000 / 45),
            "windowed FRAME_INTERVAL must be 1/45 s (~22_222 µs), not 60 Hz 16_667 µs"
        );
        assert_ne!(
            FRAME_INTERVAL,
            Duration::from_micros(1_000_000 / 60),
            "pre-fix 60 Hz windowed present cap must not remain"
        );
        assert_eq!(
            HEADLESS_LOGIC_INTERVAL,
            Duration::from_nanos(33_333_333),
            "headless 30 Hz logic interval must stay 30 logic frames/sec"
        );

        let src = include_str!("run_loop.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("run_loop live path before tests");
        assert!(
            live.contains("pub(super) const DEFAULT_MAX_FPS: u32 = 45"),
            "live run_loop must name C++ DEFAULT_MAX_FPS = 45"
        );
        assert!(
            !live.contains("from_micros(16_667)"),
            "live run_loop must not hardcode 60 Hz 16_667 µs"
        );
        assert!(
            src.contains("HEADLESS_LOGIC_INTERVAL") && src.contains("from_nanos(33_333_333)"),
            "headless logic must remain a separate 30 Hz path"
        );
    }
}

#[cfg(test)]
mod headless_edge_scroll_residual_tests {
    #[test]
    fn edge_scroll_disabled_when_headless() {
        let src = crate::cnc_game_engine::ENGINE_SRC;
        let i = src
            .find("Edge scrolling (C++ LookAt.cpp")
            .expect("edge scroll");
        let body = &src[i..src.len().min(i + 500)];
        assert!(
            body.contains("!self.runtime_host_headless"),
            "headless runtime host must not edge-scroll from stuck (0,0) mouse"
        );
    }
}
