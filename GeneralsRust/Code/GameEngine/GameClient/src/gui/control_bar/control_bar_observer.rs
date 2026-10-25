//! Control-bar observer helpers.
//!
//! Ported from `ControlBarObserver.cpp`.

use super::ControlBarContext;
use super::control_bar::ControlBar;
use crate::game_text::GameText;
use crate::gui::game_window::{Image, WindowStatus};
use crate::gui::{GameWindow, WindowMessage, WindowMsgHandled};
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::TheGameLogic;
use gamelogic::player::{PlayerType, player_list as logic_player_list};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

const MAX_OBSERVER_BUTTONS: usize = 8;

/// KindOf bits matching C++ KindOf.h (`ALLOW_SURRENDER` off) used by
/// `populateObserverInfoWindow`.
const KINDOF_SCORE: u128 = 1u128 << 35;
const KINDOF_STRUCTURE: u128 = 1u128 << 7;
const KINDOF_SCORE_CREATE: u128 = 1u128 << 36;
const KINDOF_SCORE_DESTROY: u128 = 1u128 << 37;

static OBSERVER_LOOK_AT: AtomicI32 = AtomicI32::new(-1);
static OBSERVER_CONTROLS_READY: AtomicU32 = AtomicU32::new(0);

/// Populate observer-specific commands.
///
/// The legacy C++ observer UI primarily drives a player-list window; command buttons are sourced
/// from an observer command set when present.
pub(super) fn populate_observer_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(control_bar) = get_control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    let observer_set_names = [
        "Observer",
        "OBSERVER",
        "MultiPlayerObserver",
        "MULTIPLAYEROBSERVER",
    ];

    for set_name in observer_set_names {
        let Some(set) = control_bar.find_command_set_by_name(set_name) else {
            continue;
        };

        for button in set.buttons.iter().flatten() {
            if let Some(common_button) = common_bar.find_command_button_resolved(button.get_name())
            {
                ControlBar::push_command_if_missing(
                    context,
                    ControlBar::command_from_definition(common_button),
                );
            } else {
                ControlBar::push_command_if_missing(
                    context,
                    ControlBar::command_from_logic_button(button),
                );
            }
        }
        break;
    }

    Ok(())
}

/// C++ `ControlBar::initObserverControls` — cache list/info windows after ControlBar.wnd load.
pub fn init_observer_controls(receiving_manager: &mut crate::gui::window_manager::WindowManager) {
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:ObserverPlayerInfoWindow");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:ObserverPlayerListWindow");
        for i in 0..MAX_OBSERVER_BUTTONS {
            let _ =
                receiving_manager.find_window_by_name(&format!("ControlBar.wnd:ButtonPlayer{i}"));
            let _ = receiving_manager
                .find_window_by_name(&format!("ControlBar.wnd:StaticTextPlayer{i}"));
        }
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextNumberOfUnits");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextNumberOfBuildings");
        let _ =
            receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextNumberOfUnitsKilled");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextNumberOfUnitsLost");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextPlayerName");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:WinFlag");
        let _ = receiving_manager.find_window_by_name("ControlBar.wnd:WinGeneralPortrait");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonCancel");
        for i in 0..MAX_OBSERVER_BUTTONS {
            let _ = NameKeyGenerator::name_to_key(&format!("ControlBar.wnd:ButtonPlayer{i}"));
        }
    })(receiving_manager);
    OBSERVER_CONTROLS_READY.store(1, Ordering::Relaxed);
}

pub fn observer_controls_ready() -> bool {
    OBSERVER_CONTROLS_READY.load(Ordering::Relaxed) != 0
}

pub fn set_observer_look_at_player(index: Option<i32>) {
    OBSERVER_LOOK_AT.store(index.unwrap_or(-1), Ordering::Relaxed);
    crate::helpers::set_live_control_bar_observer_look_at(index);
}

pub fn observer_look_at_player_index() -> Option<i32> {
    let index = OBSERVER_LOOK_AT.load(Ordering::Relaxed);
    (index >= 0).then_some(index)
}

/// C++ ControlBar.cpp switchToContext hide matrix (2135-2137, 2186-2188,
/// 2229-2231, 2250-2252, 2272-2274): every non-observer context keeps both
/// observer context parents hidden, so the StaticTextObs "Units / Buildings /
/// Units Lost" observer labels never show during normal play.
pub fn hide_observer_context_windows(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    hide_named(
        "ControlBar.wnd:ObserverPlayerInfoWindow",
        true,
        receiving_manager,
    );
    hide_named(
        "ControlBar.wnd:ObserverPlayerListWindow",
        true,
        receiving_manager,
    );
}

/// Same as `hide_observer_context_windows` plus the OCL timer window
/// (C++ ControlBar.cpp:2135, 2186, 2229, 2250, 2272 — CB_CONTEXT_NONE through
/// CB_CONTEXT_MULTI_SELECT all hide CP_OCL_TIMER as well). Used by contexts
/// that do not own the OCL timer display.
pub fn hide_observer_and_ocl_context_windows(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    hide_observer_context_windows(receiving_manager);
    hide_named("ControlBar.wnd:OCLTimerWindow", true, receiving_manager);
}

fn hide_named(
    name: &str,
    hidden: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(win) = receiving_manager.find_window_by_name(name) {
            let _ = win.borrow_mut().hide(hidden, receiving_manager);
        }
    })(receiving_manager);
}

fn set_named_text(
    name: &str,
    text: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(win) = receiving_manager.find_window_by_name(name) {
            let _ = win.borrow_mut().set_text(text);
        }
    })(receiving_manager);
}
fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn apply_enabled_image(
    name: &str,
    image_name: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if image_name.is_empty() {
        return;
    }
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        let Some(image) = receiving_manager.win_find_image(image_name) else {
            return;
        };
        if let Some(win) = receiving_manager.find_window_by_name(name) {
            let _ = win.borrow_mut().set_enabled_image(0, image);
            win.borrow_mut().set_status(WindowStatus::IMAGE);
        }
    })(receiving_manager);
}

/// C++ `ControlBar::populateObserverList`.
pub fn populate_observer_list(receiving_manager: &mut crate::gui::window_manager::WindowManager) {
    if !observer_controls_ready() {
        init_observer_controls(receiving_manager);
    }

    hide_named(
        "ControlBar.wnd:ObserverPlayerInfoWindow",
        true,
        receiving_manager,
    );
    hide_named(
        "ControlBar.wnd:ObserverPlayerListWindow",
        false,
        receiving_manager,
    );
    hide_named("ControlBar.wnd:CommandWindow", true, receiving_manager);
    hide_named("ControlBar.wnd:BeaconWindow", true, receiving_manager);
    hide_named(
        "ControlBar.wnd:UnderConstructionWindow",
        true,
        receiving_manager,
    );
    hide_named("ControlBar.wnd:OCLTimerWindow", true, receiving_manager);

    let multiplayer = TheGameLogic::is_in_multiplayer_game();
    let mut current_button = 0usize;
    let player_list = logic_player_list();
    let player_count = player_list
        .read()
        .map(|list| list.get_player_count())
        .unwrap_or(0);

    for i in 0..player_count {
        if current_button >= MAX_OBSERVER_BUTTONS {
            break;
        }
        let player_arc = player_list
            .read()
            .ok()
            .and_then(|list| list.get_player(i as i32).cloned());
        let Some(player_arc) = player_arc else {
            continue;
        };
        let Ok(player) = player_arc.read() else {
            continue;
        };
        if player.is_player_observer() {
            continue;
        }
        if !multiplayer && player.get_player_type() != PlayerType::Human {
            continue;
        }

        let display_name = player.get_player_display_name().clone();
        let pc = player.get_player_color();
        let color = pack_rgba(pc.r, pc.g, pc.b, pc.a);
        let enabled_image = player
            .get_player_template()
            .map(|template| template.get_enabled_image().to_string())
            .unwrap_or_default();
        let team_label = if multiplayer {
            let team = format!("Team:{}", i + 1);
            GameText::fetch(&team)
        } else {
            String::new()
        };
        drop(player);

        let button_name = format!("ControlBar.wnd:ButtonPlayer{current_button}");
        let text_name = format!("ControlBar.wnd:StaticTextPlayer{current_button}");
        (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
            if let Some(button) = receiving_manager.find_window_by_name(&button_name) {
                {
                    let mut win = button.borrow_mut();
                    win.set_user_data(i as i32);
                    win.set_tooltip(&display_name);
                    win.set_status(WindowStatus::USE_OVERLAY_STATES);
                    let _ = win.hide(false, receiving_manager);
                }
            }
            if let Some(text) = receiving_manager.find_window_by_name(&text_name) {
                let mut win = text.borrow_mut();
                win.set_enabled_text_colors(color, 0xFF00_0000);
                let label = if multiplayer && !team_label.is_empty() {
                    let fmt = GameText::fetch("CONTROLBAR:ObsPlayerLabel");
                    if fmt.contains("%s") || fmt.contains("{") {
                        format!("{display_name} {team_label}")
                    } else if fmt.is_empty() || fmt == "CONTROLBAR:ObsPlayerLabel" {
                        format!("{display_name} {team_label}")
                    } else {
                        format!("{fmt} {display_name} {team_label}")
                    }
                } else {
                    display_name.clone()
                };
                let _ = win.set_text(&label);
                let _ = win.hide(false, receiving_manager);
            }
        })(receiving_manager);
        apply_enabled_image(&button_name, &enabled_image, receiving_manager);

        current_button += 1;
        if !multiplayer {
            break;
        }
    }

    for rest in current_button..MAX_OBSERVER_BUTTONS {
        hide_named(
            &format!("ControlBar.wnd:ButtonPlayer{rest}"),
            true,
            receiving_manager,
        );
        hide_named(
            &format!("ControlBar.wnd:StaticTextPlayer{rest}"),
            true,
            receiving_manager,
        );
    }
}

/// C++ `ControlBar::populateObserverInfoWindow`.
pub fn populate_observer_info_window(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    populate_observer_info_window_for_target(observer_look_at_player_index(), receiving_manager);
}
pub(crate) fn populate_observer_info_window_for_target(
    target: Option<i32>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if !observer_controls_ready() {
        init_observer_controls(receiving_manager);
    }

    let info_hidden = (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        receiving_manager
            .find_window_by_name("ControlBar.wnd:ObserverPlayerInfoWindow")
            .map(|win| win.borrow().is_hidden())
            .unwrap_or(true)
    })(receiving_manager);
    if info_hidden {
        return;
    }

    let Some(index) = target else {
        hide_named(
            "ControlBar.wnd:ObserverPlayerInfoWindow",
            true,
            receiving_manager,
        );
        hide_named(
            "ControlBar.wnd:ObserverPlayerListWindow",
            false,
            receiving_manager,
        );
        populate_observer_list(receiving_manager);
        return;
    };

    let player_list = logic_player_list();
    let player_arc = player_list
        .read()
        .ok()
        .and_then(|list| list.get_player(index).cloned());
    let Some(player_arc) = player_arc else {
        hide_named(
            "ControlBar.wnd:ObserverPlayerInfoWindow",
            true,
            receiving_manager,
        );
        hide_named(
            "ControlBar.wnd:ObserverPlayerListWindow",
            false,
            receiving_manager,
        );
        populate_observer_list(receiving_manager);
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };

    let num_units = player.count_objects_by_kindof(KINDOF_SCORE, KINDOF_STRUCTURE);
    let num_buildings = player.count_objects_by_kindof(KINDOF_SCORE | KINDOF_STRUCTURE, 0)
        + player.count_objects_by_kindof(KINDOF_SCORE_CREATE | KINDOF_STRUCTURE, 0)
        + player.count_objects_by_kindof(KINDOF_SCORE_DESTROY | KINDOF_STRUCTURE, 0);
    let score_keeper = player.get_score_keeper();
    let units_killed = score_keeper.get_total_units_destroyed();
    let units_lost = score_keeper.get_total_units_lost();
    let display_name = player.get_player_display_name().clone();
    let pc = player.get_player_color();
    let color = pack_rgba(pc.r, pc.g, pc.b, pc.a);
    let flag = player
        .get_player_template()
        .map(|template| template.get_flag_water_mark().to_string())
        .unwrap_or_default();
    drop(player);

    set_named_text(
        "ControlBar.wnd:StaticTextNumberOfUnits",
        &num_units.to_string(),
        receiving_manager,
    );
    set_named_text(
        "ControlBar.wnd:StaticTextNumberOfBuildings",
        &num_buildings.to_string(),
        receiving_manager,
    );
    set_named_text(
        "ControlBar.wnd:StaticTextNumberOfUnitsKilled",
        &units_killed.to_string(),
        receiving_manager,
    );
    set_named_text(
        "ControlBar.wnd:StaticTextNumberOfUnitsLost",
        &units_lost.to_string(),
        receiving_manager,
    );
    set_named_text(
        "ControlBar.wnd:StaticTextPlayerName",
        &display_name,
        receiving_manager,
    );
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(name_win) =
            receiving_manager.find_window_by_name("ControlBar.wnd:StaticTextPlayerName")
        {
            name_win
                .borrow_mut()
                .set_enabled_text_colors(color, 0xFF00_0000);
        }
        if let Some(portrait) =
            receiving_manager.find_window_by_name("ControlBar.wnd:WinGeneralPortrait")
        {
            let _ = portrait.borrow_mut().hide(false, receiving_manager);
        }
    })(receiving_manager);
    apply_enabled_image("ControlBar.wnd:WinFlag", &flag, receiving_manager);
}

/// Show the observer list context (C++ `CB_CONTEXT_OBSERVER_LIST`).
pub fn reveal_observer_list_window(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    hide_named("ControlBar.wnd:ControlBarParent", false, receiving_manager);
    hide_named("ControlBar.wnd:CommandWindow", true, receiving_manager);
    hide_named(
        "ControlBar.wnd:UnderConstructionWindow",
        true,
        receiving_manager,
    );
    hide_named("ControlBar.wnd:OCLTimerWindow", true, receiving_manager);
    hide_named("ControlBar.wnd:BeaconWindow", true, receiving_manager);
    hide_named(
        "ControlBar.wnd:ObserverPlayerInfoWindow",
        true,
        receiving_manager,
    );
    hide_named(
        "ControlBar.wnd:ObserverPlayerListWindow",
        false,
        receiving_manager,
    );
    populate_observer_list(receiving_manager);
}

/// C++ `ControlBarObserverSystem`.
pub fn control_bar_observer_system(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: crate::gui::WindowMsgData,
    _data2: crate::gui::WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_manager(|receiving_manager, _effects| match msg {
        WindowMessage::Create | WindowMessage::Destroy | WindowMessage::None => {
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetMouseEntering | WindowMessage::GadgetMouseLeaving => {
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected | WindowMessage::GadgetRightClick => {
            let control_id = data1 as u32;
            let cancel_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonCancel");
            if control_id == cancel_id {
                set_observer_look_at_player(None);
                hide_named(
                    "ControlBar.wnd:ObserverPlayerInfoWindow",
                    true,
                    receiving_manager,
                );
                hide_named(
                    "ControlBar.wnd:ObserverPlayerListWindow",
                    false,
                    receiving_manager,
                );
                populate_observer_list(receiving_manager);
                return WindowMsgHandled::Handled;
            }
            for i in 0..MAX_OBSERVER_BUTTONS {
                let button_id =
                    NameKeyGenerator::name_to_key(&format!("ControlBar.wnd:ButtonPlayer{i}"));
                if control_id != button_id {
                    continue;
                }
                let player_index =
                    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
                        receiving_manager
                            .find_window_by_name(&format!("ControlBar.wnd:ButtonPlayer{i}"))
                            .and_then(|win| win.borrow().get_user_data::<i32>().copied())
                    })(receiving_manager);
                if let Some(index) = player_index {
                    set_observer_look_at_player(Some(index));
                    hide_named(
                        "ControlBar.wnd:ObserverPlayerInfoWindow",
                        false,
                        receiving_manager,
                    );
                    hide_named(
                        "ControlBar.wnd:ObserverPlayerListWindow",
                        true,
                        receiving_manager,
                    );
                    populate_observer_info_window(receiving_manager);
                }
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    })
}

/// Used by Close13 GameWindowTransitions (`TheControlBar->getArrowImage()`).
static GEN_ARROW: std::sync::Mutex<Option<Image>> = std::sync::Mutex::new(None);

pub fn set_gen_arrow_image(image: Option<Image>) {
    if let Ok(mut slot) = GEN_ARROW.lock() {
        *slot = image;
    }
}

pub fn get_gen_arrow_image() -> Option<Image> {
    GEN_ARROW.lock().ok().and_then(|slot| slot.clone())
}

pub fn ensure_gen_arrow_from_mapped_images(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if get_gen_arrow_image().is_some() {
        return;
    }
    (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
        for name in ["GenArrow", "ControlBarArrow", "Arrow"] {
            if let Some(image) = receiving_manager.win_find_image(name) {
                set_gen_arrow_image(Some(image));
                break;
            }
        }
    })(receiving_manager);
}

/// Residual: C++ `initObserverControls` name latch.
pub fn simulate_init_observer_controls(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    init_observer_controls(receiving_manager);
    observer_controls_ready()
}

/// Residual: C++ `populateObserverList` name latch.
pub fn simulate_populate_observer_list(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    populate_observer_list(receiving_manager);
    true
}

/// Residual: C++ `populateObserverInfoWindow` name latch.
pub fn simulate_populate_observer_info_window(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    populate_observer_info_window(receiving_manager);
    true
}

/// Immutable receiving roster admitted for one native UI operation. It is never
/// retained by ControlBar or published through the standalone Core observer.
#[derive(Clone, Debug)]
pub struct NativeObserverPlayer {
    pub index: i32,
    pub observer: bool,
    pub human: bool,
    pub display_name: String,
}
#[derive(Clone, Debug)]
pub struct NativeObserverRoster {
    pub players: Vec<NativeObserverPlayer>,
    /// Actual Recorder metadata; GameMode::Replay does not imply multiplayer.
    pub multiplayer: bool,
    pub frame: u32,
}
impl NativeObserverRoster {
    pub fn selectable(&self) -> impl Iterator<Item = &NativeObserverPlayer> {
        self.players
            .iter()
            .filter(|p| !p.observer && (self.multiplayer || p.human))
            .take(if self.multiplayer {
                MAX_OBSERVER_BUTTONS
            } else {
                1
            })
    }
    pub fn contains_target(&self, index: i32) -> bool {
        self.players.iter().any(|p| p.index == index && !p.observer)
    }
}
pub(crate) fn refresh_observer_windows_in_manager(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    roster: &NativeObserverRoster,
    target: Option<i32>,
) {
    for (name, hidden) in [
        ("ControlBar.wnd:ObserverPlayerInfoWindow", target.is_none()),
        ("ControlBar.wnd:ObserverPlayerListWindow", target.is_some()),
    ] {
        if let Some(window) = receiving_manager.find_window_by_name(name) {
            let _ = window.borrow_mut().hide(hidden, receiving_manager);
        }
    }
    for slot in 0..MAX_OBSERVER_BUTTONS {
        let player = roster.selectable().nth(slot);
        if let Some(button) =
            receiving_manager.find_window_by_name(&format!("ControlBar.wnd:ButtonPlayer{slot}"))
        {
            let mut button = button.borrow_mut();
            let _ = button.hide(player.is_none(), receiving_manager);
            if let Some(player) = player {
                button.set_user_data(player.index);
                button.set_tooltip(&player.display_name);
                button.set_status(WindowStatus::USE_OVERLAY_STATES);
            }
        }
        if let Some(text) =
            receiving_manager.find_window_by_name(&format!("ControlBar.wnd:StaticTextPlayer{slot}"))
        {
            let mut text = text.borrow_mut();
            let _ = text.hide(player.is_none(), receiving_manager);
            // Multiplayer labels also include actual GameInfo team metadata.
            // Preserve the existing compatibility label until that UI adapter
            // is explicitly migrated; do not replace it with a fabricated name.
            if !roster.multiplayer {
                if let Some(player) = player {
                    let _ = text.set_text(&player.display_name);
                }
            }
        }
    }
}
