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

const SW_OBJECT_ID: u32 = 960_701;
const SW_POWER_NAME: &str = "SWHudChainScudStorm";
const UC_OBJECT_ID: u32 = 972_771;
const OCL_OBJECT_ID: u32 = 972_883;

fn install_named_window(name: &str, width: i32, height: i32) {
    game_client::gui::with_window_manager(|manager| {
        let win = manager.create_window(None, 0, 0, width, height).expect(name);
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
        manager.find_window_by_name(name).map(|w| w.borrow().is_hidden())
    })
}

/// A superweapon object feeding GameLogic's `TheInGameUI::add_superweapon`
/// store must produce a drawn countdown entry that counts down and hits
/// READY (C++ InGameUI.cpp:548-580 + 3487-3697).
#[test]
fn superweapon_object_produces_counting_down_strip_entry_that_hits_ready() {
    let _guard = HUD_FEEDBACK_CHAIN_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    game_client::gui::ingame_ui::live_superweapon_strip_clear();

    // A superweapon structure object in the dual-world registry (no live
    // module: the stored ready frame owns the countdown, C++ parity for the
    // residual path).
    let object = std::sync::Arc::new(std::sync::RwLock::new(
        gamelogic::object::Object::new_for_xfer_load(SW_OBJECT_ID, 100.0),
    ));
    gamelogic::object::registry::OBJECT_REGISTRY.register_object(SW_OBJECT_ID, &object);

    // Public-timer shared-n-sync template with a 5s (150-frame) recharge.
    let base =
        gamelogic::object::special_power_template::find_or_create_special_power_template(
            &gamelogic::common::AsciiString::from(SW_POWER_NAME),
        );
    let template = gamelogic::object::special_power_template::SpecialPowerTemplate::new(
        SW_POWER_NAME.to_string(),
        base.get_id(),
    )
    .with_power_type(gamelogic::object::special_power_types::SpecialPowerType::ScudStorm)
    .with_reload_time(150)
    .with_public_timer(true)
    .with_shared_n_sync(true);
    if let Some(mut store) =
        gamelogic::object::special_power_template::get_special_power_store_mut()
    {
        store.add_template(template.clone());
    }
    let template = std::sync::Arc::new(template);

    // Special-power attach: GameLogic registers the timer with TheInGameUI.
    gamelogic::helpers::TheInGameUI::add_superweapon(
        0,
        SW_POWER_NAME.to_string(),
        SW_OBJECT_ID,
        &template,
    );

    let mut client = game_client::core::game_client::GameClient::new().expect("GameClient::new");
    client.set_in_game_ui_subsystem(std::sync::Arc::new(std::sync::Mutex::new(
        game_client::core::subsystems::InGameUISubsystem::default(),
    )));

    // Resolve the attach-time ready frame (frame + ReloadTime).
    let ready_frame = gamelogic::helpers::TheInGameUI::superweapon_entries()
        .into_iter()
        .find(|(_, power_name, object_id, _)| {
            power_name == SW_POWER_NAME && *object_id == SW_OBJECT_ID
        })
        .map(|(_, _, _, ready_frame)| ready_frame)
        .expect("TheInGameUI::add_superweapon store entry");

    // First HUD build: the store drains into the live strip and bridges into
    // the drawn presentation residual with a live countdown.
    client.set_frame(ready_frame - 90);
    client.sync_superweapon_strip_from_logic();

    let strip = game_client::gui::ingame_ui::live_superweapon_draw_entries();
    assert!(
        strip
            .iter()
            .any(|(name, countdown, ready)| name.contains(SW_POWER_NAME)
                && countdown == "0:03"
                && !*ready),
        "superweapon strip must draw a 0:03 countdown, got {strip:?}"
    );
    let residual = client
        .in_game_ui_subsystem()
        .expect("in_game_ui subsystem")
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .presentation_superweapon_timers()
        .to_vec();
    assert!(
        residual
            .iter()
            .any(|t| t.name.contains(SW_POWER_NAME) && t.countdown_text == "0:03" && !t.ready),
        "drawn presentation residual must carry the 0:03 countdown, got {residual:?}"
    );

    // The strip counts down as frames advance.
    client.set_frame(ready_frame - 30);
    client.sync_superweapon_strip_from_logic();
    let strip = game_client::gui::ingame_ui::live_superweapon_draw_entries();
    assert!(
        strip
            .iter()
            .any(|(name, countdown, ready)| name.contains(SW_POWER_NAME)
                && countdown == "0:01"
                && !*ready),
        "superweapon strip must count down to 0:01, got {strip:?}"
    );

    // Past the ready frame the entry hits READY (bold/blink state flagged).
    client.set_frame(ready_frame + 10);
    client.sync_superweapon_strip_from_logic();
    let strip = game_client::gui::ingame_ui::live_superweapon_draw_entries();
    assert!(
        strip
            .iter()
            .any(|(name, countdown, ready)| name.contains(SW_POWER_NAME)
                && countdown == "0:00"
                && *ready),
        "superweapon strip must hit READY at 0:00, got {strip:?}"
    );
    let residual = client
        .in_game_ui_subsystem()
        .expect("in_game_ui subsystem")
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .presentation_superweapon_timers()
        .to_vec();
    assert!(
        residual
            .iter()
            .any(|t| t.name.contains(SW_POWER_NAME) && t.countdown_text == "0:00" && t.ready),
        "drawn presentation residual must flag READY, got {residual:?}"
    );

    gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(SW_OBJECT_ID);
    game_client::gui::ingame_ui::live_superweapon_strip_clear();
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
    client.set_in_game_ui_subsystem(std::sync::Arc::new(std::sync::Mutex::new(
        game_client::core::subsystems::InGameUISubsystem::default(),
    )));

    // The local player has been eliminated (VictoryConditions latch).
    gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(true);
    gamelogic::helpers::TheInGameUI::display_message("GUI:PlayerHasBeenDefeated Napoleon");

    client.drain_logic_hud_messages();

    let subsystem = client.in_game_ui_subsystem().expect("in_game_ui subsystem");
    let messages = subsystem
        .lock()
        .unwrap_or_else(|e| e.into_inner())
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
    let _guard = HUD_FEEDBACK_CHAIN_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    install_named_window("ControlBar.wnd:UnderConstructionDesc", 80, 24);
    install_named_window("ControlBar.wnd:UnderConstructionWindow", 200, 60);
    set_window_hidden("ControlBar.wnd:UnderConstructionWindow", true);
    game_client::gui::with_window_manager(|manager| {
        if let Some(win) = manager.find_window_by_name("ControlBar.wnd:UnderConstructionDesc") {
            let _ = win.borrow_mut().set_text("CONTROLBAR:UnderConstructionDesc");
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
