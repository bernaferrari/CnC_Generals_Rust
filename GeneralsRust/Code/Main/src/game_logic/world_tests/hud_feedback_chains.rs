//! HUD feedback chains driven through the GameEngine GameClient seams the
//! live host ticks every frame:
//! - superweapon countdown strip (GameLogic `TheInGameUI::add_superweapon`
//!   store → client strip → drawn presentation residual),
//! - defeat HUD message localization + local-defeat radar/chat UX,
//! - control-bar under-construction label,
//! - control-bar OCL timer window reveal.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// Serialize tests that mutate process-global GameEngine UI state
/// (TheInGameUI store, radar force-on, in-game chat type, window manager).
static HUD_FEEDBACK_CHAIN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const SW_POWER_NAME: &str = "SWHudChainScudStorm";
const UC_OBJECT_ID: u32 = 972_771;
const OCL_OBJECT_ID: u32 = 972_883;

fn install_named_window(name: &str, width: i32, height: i32) {
    game_client::gui::with_window_manager(|manager| {
        let win = manager
            .create_window(None, 0, 0, width, height)
            .expect(name);
        win.borrow_mut().set_name(name);
    });
}

fn set_window_hidden(name: &str, hidden: bool) {
    game_client::gui::with_window_manager(|manager| {
        if let Some(win) = manager.find_window_by_name(name) {
            let _ = win.borrow_mut().hide(hidden);
        }
    });
}

fn window_text(name: &str) -> Option<String> {
    game_client::gui::with_window_manager_ref(|manager| {
        manager
            .find_window_by_name(name)
            .map(|w| w.borrow().get_text().to_string())
    })
}

fn window_hidden(name: &str) -> Option<bool> {
    game_client::gui::with_window_manager_ref(|manager| {
        manager
            .find_window_by_name(name)
            .map(|w| w.borrow().is_hidden())
    })
}

/// The production Main object/player owner freezes PublicTimer values into
/// the receiving HUD. No Core registry or shared UI setup owns this scenario.
#[test]
fn superweapon_object_produces_counting_down_strip_entry_that_hits_ready() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    use crate::presentation_frame::PresentationFrame;
    let power = SpecialPowerType::ScudStorm;
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.apply_faction_intrinsic_sciences();
    logic.add_player(player);
    let mut template = ThingTemplate::new("OwnedScudTimer");
    template.set_health(4000.0);
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon);
    template
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_Superweapon".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: SW_POWER_NAME.into(),
            special_power_template_id: 1,
            command_power: Some(power.clone()),
            reload_time_frames: 150,
            required_science: None,
            public_timer: true,
            shared_n_sync: true,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    logic.templates.insert("OwnedScudTimer".into(), template);
    let id = logic
        .create_object_for_player("OwnedScudTimer", 0, glam::Vec3::ZERO)
        .unwrap();
    logic
        .get_player_mut(0)
        .unwrap()
        .reset_shared_special_power_timer(&power, 5.0);
    let mut hud = crate::ui::GameHUD::new();
    for (elapsed, remaining, ready) in
        [(2.0, 3.0, false), (2.0, 1.0, false), (4.0 / 3.0, 0.0, true)]
    {
        let became_ready = logic
            .get_player_mut(0)
            .unwrap()
            .tick_shared_special_power_timers(elapsed);
        assert_eq!(became_ready.contains(&power), ready);
        let frame = PresentationFrame::build_from_logic(&mut logic, 0);
        let row = frame
            .superweapon_timers()
            .iter()
            .find(|row| row.template_name == SW_POWER_NAME)
            .unwrap();
        assert!((row.remaining - remaining).abs() < 0.001);
        assert_eq!(row.ready, ready);
        assert_eq!(row.recharge_time, 5.0);
        frame.apply_to_game_hud(&mut hud);
        let drawn = hud
            .presentation_superweapon_timers()
            .iter()
            .find(|row| row.template_name == SW_POWER_NAME)
            .unwrap();
        assert!((drawn.remaining - remaining).abs() < 0.001);
        assert_eq!(drawn.ready, ready);
        assert!(drawn.unlocked);
        assert!(logic.host_object(id).unwrap().is_alive());
    }
}

/// `GUI:PlayerHasBeenDefeated {name}` must land on the drawn HUD message
/// list localized (C++ VictoryConditions.cpp:174-176), and the first local
/// defeat must force the radar on and switch chat to Everyone
/// (VictoryConditions.cpp:200-215).
#[test]
fn defeat_message_routes_localized_to_hud_and_local_defeat_forces_radar_and_chat() {
    let _guard = HUD_FEEDBACK_CHAIN_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut client = game_client::core::game_client::GameClient::new().expect("GameClient::new");
    client.set_in_game_ui_subsystem(game_client::core::subsystems::InGameUISubsystem::default());

    // The local player has been eliminated (VictoryConditions latch).
    gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(true);
    gamelogic::helpers::TheInGameUI::display_message("GUI:PlayerHasBeenDefeated Napoleon");

    client.drain_logic_hud_messages(0);

    let subsystem = client.in_game_ui_subsystem().expect("in_game_ui subsystem");
    let messages = subsystem
        .hud_messages()
        .iter()
        .map(|m| m.text.clone())
        .collect::<Vec<_>>();
    assert!(
        messages.iter().any(|m| m.contains("Napoleon")
            && m.contains("has been defeated")
            && !m.contains("GUI:PlayerHasBeenDefeated")),
        "defeat must draw as a localized HUD message, got {messages:?}"
    );

    // Local-defeat UX: radar forced on.
    assert!(
        game_engine::common::system::radar::get_radar_system()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .is_radar_forced(),
        "local defeat must force the radar on (TheRadar->forceOn(TRUE))"
    );
    assert_eq!(
        game_client::gui::callbacks::ingame_callbacks::get_in_game_chat_type(),
        game_client::gui::callbacks::ingame_callbacks::InGameChatType::Everyone,
        "local defeat must switch in-game chat to Everyone"
    );

    // Cleanup global latches.
    game_engine::common::system::radar::get_radar_system()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .force_on(false);
    gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(false);
}

/// Selecting an under-construction structure on the host presentation path
/// must write the localized `Under Construction: N%` label into
/// `ControlBar.wnd:UnderConstructionDesc` and reveal the context window
/// (C++ ControlBarUnderConstruction.cpp:24-41, ControlBar.cpp:2271).
#[test]
fn under_construction_selection_writes_localized_percent_label() {
    let owner = game_client::gui::window_manager::WindowManagerOwner::new();
    owner.with_scope(|| {
        let _guard = HUD_FEEDBACK_CHAIN_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        install_named_window("ControlBar.wnd:UnderConstructionDesc", 80, 24);
        install_named_window("ControlBar.wnd:UnderConstructionWindow", 200, 60);
        set_window_hidden("ControlBar.wnd:UnderConstructionWindow", true);
        game_client::gui::with_window_manager(|manager| {
            if let Some(win) = manager.find_window_by_name("ControlBar.wnd:UnderConstructionDesc") {
                let _ = win
                    .borrow_mut()
                    .set_text("CONTROLBAR:UnderConstructionDesc");
            }
        });

        let mut bar = game_client::gui::control_bar::ControlBar::new();
        // Host presentation freeze: one selected structure, 35% constructed.
        bar.sync_selection_display_from_presentation(
            Some("AmericaCommandCenter"),
            100.0,
            100.0,
            1,
            None,
            None,
            None,
            &[],
            false,
        );
        bar.sync_structure_context_from_presentation(0, 0, true, 0.35);

        bar.update_for_selection(vec![UC_OBJECT_ID])
            .expect("update_for_selection");

        let text = window_text("ControlBar.wnd:UnderConstructionDesc");
        let text = text.as_deref();
        assert!(
            text.is_some_and(|t| t.contains("Under Construction") && t.contains("35%")),
            "under-construction label must be localized with the live percent, got {text:?}"
        );
        assert!(
            !text.is_some_and(|t| t.contains("CONTROLBAR:")),
            "the raw WND label key must never show, got {text:?}"
        );
        assert_eq!(
            window_hidden("ControlBar.wnd:UnderConstructionWindow"),
            Some(false),
            "under-construction context window must be revealed"
        );
    });
}

/// A host presentation OCL countdown must reveal the timer window and tick
/// its seconds (C++ ControlBarOCLTimer.cpp:23-49: updateOCLTimerTextDisplay
/// runs whenever the displayed second changes).
#[test]
fn ocl_countdown_object_reveals_timer_window_with_ticking_seconds() {
    let _guard = HUD_FEEDBACK_CHAIN_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    install_named_window("ControlBar.wnd:OCLTimerWindow", 120, 40);
    install_named_window("ControlBar.wnd:OCLTimerStaticText", 80, 24);
    set_window_hidden("ControlBar.wnd:OCLTimerWindow", true);

    let mut bar = game_client::gui::control_bar::ControlBar::new();
    bar.sync_selection_display_from_presentation(
        Some("AmericaCommandCenter"),
        100.0,
        100.0,
        1,
        None,
        None,
        None,
        &[],
        false,
    );
    bar.update_for_selection(vec![OCL_OBJECT_ID])
        .expect("update_for_selection");

    // Presentation-synced countdown: the window reveals with formatted time.
    bar.sync_ocl_timer_from_presentation(7);
    assert_eq!(
        window_hidden("ControlBar.wnd:OCLTimerWindow"),
        Some(false),
        "OCL timer window must reveal on the presentation countdown"
    );
    assert_eq!(
        window_text("ControlBar.wnd:OCLTimerStaticText").as_deref(),
        Some("0:07"),
        "OCL timer text must show the synced seconds"
    );

    // The per-frame context update keeps ticking (registry-empty host path
    // no longer swallows the OCL context behind the early return).
    bar.update_for_selection(vec![OCL_OBJECT_ID])
        .expect("update_for_selection");
    assert_eq!(
        window_text("ControlBar.wnd:OCLTimerStaticText").as_deref(),
        Some("0:07"),
        "OCL timer context update must keep the revealed text live"
    );

    bar.sync_ocl_timer_from_presentation(5);
    assert_eq!(
        window_text("ControlBar.wnd:OCLTimerStaticText").as_deref(),
        Some("0:05"),
        "OCL timer text must tick with the changing second"
    );
    bar.update_for_selection(vec![OCL_OBJECT_ID])
        .expect("update_for_selection");
    assert_eq!(
        window_text("ControlBar.wnd:OCLTimerStaticText").as_deref(),
        Some("0:05"),
        "OCL timer context update must render the current second"
    );
}
