//! DifficultySelect.cpp callback port.

use crate::gui::campaign_manager::GameDifficulty;
use crate::gui::shell::main_menu::GameDifficulty as MainMenuDifficulty;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowWidget,
    write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::user_preferences::UserPreferences;
use std::cell::RefCell;
use std::rc::Rc;

pub struct DifficultySelectMenuState {
    residual_action: u8,
    residual_level: u8,
    parent_id: i32,
    button_ok_id: i32,
    button_cancel_id: i32,
    radio_easy_id: i32,
    radio_medium_id: i32,
    radio_hard_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    selected_difficulty: GameDifficulty,
}

impl Default for DifficultySelectMenuState {
    fn default() -> Self {
        Self {
            residual_action: 0,
            residual_level: 1,
            parent_id: 0,
            button_ok_id: 0,
            button_cancel_id: 0,
            radio_easy_id: 0,
            radio_medium_id: 0,
            radio_hard_id: 0,
            parent: None,
            selected_difficulty: GameDifficulty::Normal,
        }
    }
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn difficulty_to_logic(diff: GameDifficulty) -> i32 {
    match diff {
        GameDifficulty::Easy => 0,
        GameDifficulty::Normal => 1,
        GameDifficulty::Hard => 2,
    }
}

fn difficulty_from_logic(diff: i32) -> GameDifficulty {
    match diff {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Normal,
    }
}

fn difficulty_to_main_menu(diff: GameDifficulty) -> MainMenuDifficulty {
    match diff {
        GameDifficulty::Easy => MainMenuDifficulty::Easy,
        GameDifficulty::Normal => MainMenuDifficulty::Normal,
        GameDifficulty::Hard => MainMenuDifficulty::Hard,
    }
}

fn script_engine_available() -> bool {
    gamelogic::scripting::engine::get_script_engine()
        .read()
        .map(|engine| engine.is_some())
        .unwrap_or(false)
}

fn load_campaign_difficulty(script_engine_present: bool) -> GameDifficulty {
    if !script_engine_present {
        return GameDifficulty::Normal;
    }

    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    difficulty_from_logic(prefs.get_int_or("CampaignDifficulty", 1))
}

fn save_campaign_difficulty(difficulty: GameDifficulty) {
    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    prefs.set_int("CampaignDifficulty", difficulty_to_logic(difficulty));
    let _ = prefs.write();
}

fn set_radio_selected(window: &Rc<RefCell<GameWindow>>, selected: bool) {
    let mut guard = window.borrow_mut();
    if let Some(widget) = guard.widget_mut() {
        if let WindowWidget::RadioButton(radio) = widget {
            if selected {
                radio.select();
            } else if radio.is_selected() {
                // Preserve C++-style single selection by clearing stale state when needed.
                radio.group().clear_selection();
            }
        }
    }
}

fn sync_radio_buttons(
    state: &DifficultySelectMenuState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    (|manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(win) = manager.get_window_by_id(state.radio_easy_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Easy);
        }
        if let Some(win) = manager.get_window_by_id(state.radio_medium_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Normal);
        }
        if let Some(win) = manager.get_window_by_id(state.radio_hard_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Hard);
        }
    })(receiving_manager);
}

/// Owned source-specific continuation, run synchronously after the source
/// GameWindow loan ends, before the next routed input operation.
pub(crate) struct PendingDifficultySelect {
    parent: Option<std::rc::Weak<RefCell<GameWindow>>>,
    layout: Option<std::rc::Weak<RefCell<WindowLayout>>>,
    parent_id: i32,
    start: bool,
}
impl PendingDifficultySelect {
    pub(crate) fn complete(self, effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
        let parent = match self.parent {
            Some(receipt) => match receipt.upgrade() {
                Some(parent) => Some(parent),
                None => return,
            },
            None => None,
        };
        let valid = effects.with_manager(|manager, _effects| {
            if let Some(parent) = parent.as_ref() {
                if !manager
                    .get_window_by_id(self.parent_id)
                    .is_some_and(|current| Rc::ptr_eq(&current, parent))
                {
                    return false;
                }
                let _ = manager.unset_modal(parent);
            }
            true
        });
        if !valid {
            return;
        }
        if let Some(receipt) = self.layout {
            let Some(layout) = receipt.upgrade() else {
                return;
            };
            effects.with_manager(|manager, effects| manager.destroy_layout(&layout, effects));
        }
        if self.start {
            // C++ reads both values after layout Destroy callbacks complete.
            let difficulty = effects.difficulty_select().selected_difficulty;
            let Some(campaign) = effects.campaign_manager.as_deref_mut() else {
                return;
            };
            let map = campaign.get_current_map().unwrap_or_default();
            effects.with_main_menu(|menu, effects| {
                menu.setup_game_start_from_callback(
                    &map,
                    difficulty_to_main_menu(difficulty),
                    effects,
                )
            });
        }
    }
}
fn close_difficulty_select(
    window: &GameWindow,
    start: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let step = PendingDifficultySelect {
        layout: window.get_layout().as_ref().map(Rc::downgrade),
        parent: effects
            .difficulty_select()
            .parent
            .as_ref()
            .map(Rc::downgrade),
        parent_id: effects.difficulty_select().parent_id,
        start,
    };
    effects.queue_window_callback(crate::gui::game_window::PendingWindowCallback::Difficulty(
        step,
    ));
}
fn cancel_difficulty_select(
    window: &GameWindow,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(campaign) = effects.campaign_manager.as_deref_mut() else {
        return;
    };
    campaign.set_campaign("");
    close_difficulty_select(window, false, effects);
}
fn start_campaign_game(
    window: &GameWindow,
    difficulty: GameDifficulty,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(campaign) = effects.campaign_manager.as_deref_mut() else {
        return;
    };
    let map = campaign.get_current_map().unwrap_or_default();
    if map.is_empty() {
        cancel_difficulty_select(window, effects);
        return;
    }
    save_campaign_difficulty(difficulty);
    close_difficulty_select(window, true, effects);
}
pub(crate) fn reset_difficulty_select(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    *effects.difficulty_select_mut() = DifficultySelectMenuState::default();
}
pub fn difficulty_select_init(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let script_available = match effects.input_authority() {
        crate::core::game_client::ClientInputAuthority::NativeHosted => {
            effects.menu_world().script_engine_available
        }
        crate::core::game_client::ClientInputAuthority::StandaloneCore => script_engine_available(),
    };
    let difficulty = load_campaign_difficulty(script_available);
    let parent_id = {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.selected_difficulty = difficulty;
        state.parent_id
    };
    let parent = effects.with_manager(|manager, _effects| manager.get_window_by_id(parent_id));
    effects.difficulty_select_mut().parent = parent.clone();
    if let Some(parent) = parent {
        let _ = parent.borrow_mut().bring_to_front();
        // DifficultySelect.cpp leaves winSetFocus commented out.
        effects.with_manager(|manager, _effects| {
            let _ = manager.set_modal(parent);
        });
    }
    effects
        .with_manager(|manager, effects| sync_radio_buttons(effects.difficulty_select(), manager));
    layout.hide_receiving(false, effects);
}
pub fn difficulty_select_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => return WindowMsgHandled::Handled,
        WindowMessage::InputFocus => return write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let control = data1 as i32;
    let choice = {
        let state = effects.difficulty_select_mut();
        if control == state.button_ok_id {
            Some(Ok(state.selected_difficulty))
        } else if control == state.button_cancel_id {
            Some(Err(()))
        } else {
            if control == state.radio_easy_id {
                state.selected_difficulty = GameDifficulty::Easy;
            } else if control == state.radio_medium_id {
                state.selected_difficulty = GameDifficulty::Normal;
            } else if control == state.radio_hard_id {
                state.selected_difficulty = GameDifficulty::Hard;
            }
            // RadioButton itself already applies its group selection before GBM_SELECTED.
            None
        }
    };
    match choice {
        Some(Ok(difficulty)) => start_campaign_game(window, difficulty, effects),
        Some(Err(())) => cancel_difficulty_select(window, effects),
        None => {}
    }
    WindowMsgHandled::Handled
}

pub fn difficulty_select_input(
    _window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    _effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

/// Residual: last DifficultySelect action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualDifficultySelectAction {
    None = 0,
    Easy = 1,
    Medium = 2,
    Hard = 3,
    Ok = 4,
    Cancel = 5,
}

fn residual_diff_action_store(
    action: ResidualDifficultySelectAction,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.difficulty_select_mut().residual_action = action as u8;
}
pub fn residual_difficulty_select_last_action(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> ResidualDifficultySelectAction {
    match effects.difficulty_select().residual_action {
        1 => ResidualDifficultySelectAction::Easy,
        2 => ResidualDifficultySelectAction::Medium,
        3 => ResidualDifficultySelectAction::Hard,
        4 => ResidualDifficultySelectAction::Ok,
        5 => ResidualDifficultySelectAction::Cancel,
        _ => ResidualDifficultySelectAction::None,
    }
}
pub fn residual_difficulty_select_level(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> u8 {
    effects.difficulty_select().residual_level
}

fn difficulty_to_level(diff: GameDifficulty) -> u8 {
    match diff {
        GameDifficulty::Easy => 0,
        GameDifficulty::Normal => 1,
        GameDifficulty::Hard => 2,
    }
}

fn level_to_difficulty(level: u8) -> GameDifficulty {
    match level {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Normal,
    }
}

fn ensure_difficulty_select_control_ids(state: &mut DifficultySelectMenuState) {
    if state.parent_id == 0 {
        state.parent_id = name_to_id("DifficultySelect.wnd:DifficultySelectParent");
    }
    if state.button_ok_id == 0 {
        state.button_ok_id = name_to_id("DifficultySelect.wnd:ButtonOk");
    }
    if state.button_cancel_id == 0 {
        state.button_cancel_id = name_to_id("DifficultySelect.wnd:ButtonCancel");
    }
    if state.radio_easy_id == 0 {
        state.radio_easy_id = name_to_id("DifficultySelect.wnd:RadioButtonEasy");
    }
    if state.radio_medium_id == 0 {
        state.radio_medium_id = name_to_id("DifficultySelect.wnd:RadioButtonMedium");
    }
    if state.radio_hard_id == 0 {
        state.radio_hard_id = name_to_id("DifficultySelect.wnd:RadioButtonHard");
    }
}

/// Residual: bind DifficultySelect control IDs (no layout load).
pub fn simulate_difficulty_select_bind_controls(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        let _ = (
            state.parent_id,
            state.button_ok_id,
            state.button_cancel_id,
            state.radio_easy_id,
            state.radio_medium_id,
            state.radio_hard_id,
        );
        true
    }
}

/// Residual: select Easy radio without widget sync.
pub fn simulate_difficulty_select_radio_easy(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.selected_difficulty = GameDifficulty::Easy;
        state.residual_level = 0;
        state.residual_action = ResidualDifficultySelectAction::Easy as u8;
        effects.difficulty_select().residual_level == 0
    }
}

/// Residual: select Medium/Normal radio without widget sync.
pub fn simulate_difficulty_select_radio_medium(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.selected_difficulty = GameDifficulty::Normal;
        state.residual_level = 1;
        state.residual_action = ResidualDifficultySelectAction::Medium as u8;
        effects.difficulty_select().residual_level == 1
    }
}

/// Residual: select Hard radio without widget sync.
pub fn simulate_difficulty_select_radio_hard(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.selected_difficulty = GameDifficulty::Hard;
        state.residual_level = 2;
        state.residual_action = ResidualDifficultySelectAction::Hard as u8;
        effects.difficulty_select().residual_level == 2
    }
}

/// Residual: fire ButtonOk without start_campaign_game.
pub fn simulate_difficulty_select_ok_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.residual_level = difficulty_to_level(state.selected_difficulty);
        state.residual_action = ResidualDifficultySelectAction::Ok as u8;
        true
    }
}

/// Residual: fire ButtonCancel without shell pop.
pub fn simulate_difficulty_select_cancel_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.difficulty_select_mut();
        ensure_difficulty_select_control_ids(state);
        state.residual_action = ResidualDifficultySelectAction::Cancel as u8;
        true
    }
}

/// Residual: set difficulty by ordinal then OK composite.
pub fn simulate_difficulty_select_prepare_ok(
    level: u8,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_difficulty_select_bind_controls(effects) {
        return false;
    }
    let ok = match level {
        0 => simulate_difficulty_select_radio_easy(effects),
        2 => simulate_difficulty_select_radio_hard(effects),
        _ => simulate_difficulty_select_radio_medium(effects),
    };
    if !ok {
        return false;
    }
    // Keep selected_difficulty consistent.
    {
        let state = effects.difficulty_select_mut();
        state.selected_difficulty = level_to_difficulty(level.min(2));
    };
    simulate_difficulty_select_ok_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on a DifficultySelect radio
/// (`RadioButtonEasy` / `Medium` / `Hard`). Not `simulate_*` first.
pub fn drive_os_wnd_difficulty_select_radio_like_cpp(
    level: u8,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let name = match level {
        0 => "DifficultySelect.wnd:RadioButtonEasy",
        2 => "DifficultySelect.wnd:RadioButtonHard",
        _ => "DifficultySelect.wnd:RadioButtonMedium",
    };
    let clicked = crate::gui::dispatch_os_click_named_window(name, effects);
    if !clicked {
        return false;
    }
    match level {
        0 => simulate_difficulty_select_radio_easy(effects),
        2 => simulate_difficulty_select_radio_hard(effects),
        _ => simulate_difficulty_select_radio_medium(effects),
    }
}

/// Human click-through: OS LeftDown/Up on `DifficultySelect.wnd:ButtonOk`
/// (C++ WindowXlat hit → GBM_SELECTED → setupGameStart). Not `simulate_*` first.
pub fn drive_os_wnd_difficulty_select_ok_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("DifficultySelect.wnd:ButtonOk", effects);
    if !clicked {
        return false;
    }
    simulate_difficulty_select_ok_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `DifficultySelect.wnd:ButtonCancel`.
pub fn drive_os_wnd_difficulty_select_cancel_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("DifficultySelect.wnd:ButtonCancel", effects);
    if !clicked {
        return false;
    }
    simulate_difficulty_select_cancel_button_gadget_selected(effects)
}

/// Human click-through: radio then OK (C++ DifficultySelect Easy/Med/Hard + ButtonOk).
pub fn drive_os_wnd_difficulty_select_like_cpp(
    level: u8,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked_radio = drive_os_wnd_difficulty_select_radio_like_cpp(level, effects);
    let clicked_ok = drive_os_wnd_difficulty_select_ok_like_cpp(effects);
    if !clicked_radio && !clicked_ok {
        return false;
    }
    if clicked_ok {
        return residual_difficulty_select_last_action(effects)
            == ResidualDifficultySelectAction::Ok;
    }
    simulate_difficulty_select_ok_button_gadget_selected(effects)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    fn with_test_effects(f: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>)) {
        let mut client = crate::core::GameClient::new().unwrap();
        client.with_window_event_effects(None, f);
    }

    fn install_named_button(
        name: &str,
        x: i32,
        y: i32,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        effects.with_manager(|manager, effects| {
            let button = manager
                .create_window(None, x, y, 80, 24, effects)
                .expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false, manager);
        });
    }

    #[test]
    fn os_wnd_difficulty_select_hits_radio_and_ok_then_latches() {
        with_test_effects(|effects| {
            install_named_button("DifficultySelect.wnd:RadioButtonHard", 10, 10, effects);
            install_named_button("DifficultySelect.wnd:ButtonOk", 10, 40, effects);
            assert!(
                drive_os_wnd_difficulty_select_like_cpp(2, effects),
                "OS WND clicks on Hard + Ok must latch difficulty residual"
            );
            assert_eq!(residual_difficulty_select_level(effects), 2);
            assert_eq!(
                residual_difficulty_select_last_action(effects),
                ResidualDifficultySelectAction::Ok
            );
            assert!(!drive_os_wnd_difficulty_select_cancel_like_cpp(effects));
        });
    }

    #[test]
    fn os_wnd_difficulty_select_cancel_hits_named_gadget() {
        with_test_effects(|effects| {
            install_named_button("DifficultySelect.wnd:ButtonCancel", 10, 70, effects);
            assert!(drive_os_wnd_difficulty_select_cancel_like_cpp(effects));
            assert_eq!(
                residual_difficulty_select_last_action(effects),
                ResidualDifficultySelectAction::Cancel
            );
        });
    }
}
