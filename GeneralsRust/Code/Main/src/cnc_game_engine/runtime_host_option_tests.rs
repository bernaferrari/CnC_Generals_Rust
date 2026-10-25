//! Runtime option, command mapping, and C++ display timing regressions.

use super::*;

// Source-only wiring checks. These delimit the actual function rather than an
// arbitrary character window; behavioral acceptance lives in runtime suites.

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
        kind_names: std::sync::Arc::new(Vec::new()),
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
fn return_to_base_aircraft_residual() {
    // Existing Rust UI capability. No original retail Alt+R mapping is claimed.

    assert!(
        matches!(
            crate::command_system::command_type_from_button_name("Command_ReturnToBase"),
            Some(crate::command_system::CommandType::ReturnToBase)
        ),
        "live named RTB route must resolve the executable command"
    );
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

    /// Collect `write_control(...)` argument-list slices from Windowed launch arms only.

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
    }
}
