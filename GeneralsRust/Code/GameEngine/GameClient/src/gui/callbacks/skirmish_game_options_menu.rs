//! SkirmishGameOptionsMenu.cpp callback port.
//!
//! Uses thread-local RefCell for window state and Cell<bool> global flags
//! matching C++ statics (via mod.rs helpers).

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::game_window::Image as WindowImage;
use crate::gui::{
    get_shell, get_skirmish_setup, message_box_ok, message_box_ok_cancel, with_window_manager, GameWindow,
    SkirmishPreferences, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    WindowStatus,
};
use crate::map_util::{find_draw_positions, get_map_cache_manager, get_map_preview_image};
use crate::message_stream::{get_message_stream, GameMessageType};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::rts::player_template::get_player_template_store;
use game_engine::common::skirmish_battle_honors::{
    SkirmishBattleHonors, BATTLE_HONOR_AIR_WING, BATTLE_HONOR_APOCALYPSE, BATTLE_HONOR_BATTLE_TANK,
    BATTLE_HONOR_BLITZ10, BATTLE_HONOR_BLITZ5, BATTLE_HONOR_CAMPAIGN_CHINA,
    BATTLE_HONOR_CAMPAIGN_GLA, BATTLE_HONOR_CAMPAIGN_USA, BATTLE_HONOR_CHALLENGE_MODE,
    BATTLE_HONOR_DOMINATION, BATTLE_HONOR_ENDURANCE, BATTLE_HONOR_OFFICERSCLUB,
    BATTLE_HONOR_STREAK, BATTLE_HONOR_ULTIMATE,
};
use game_engine::common::system::copy_protection::{get_protection_manager, ProtectionStatus};
use game_network::matchmaking::slots::PlayerColor;
use game_network::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, Money, SlotState,
    PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_RANDOM,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::{GAME_SINGLE_PLAYER, GAME_SKIRMISH};
use std::cell::RefCell;
use std::rc::Rc;

// Import global state helpers from mod.rs - matches C++ static globals
use super::{
    set_skirmish_button_pushed, set_skirmish_is_initing, set_skirmish_is_shutting_down,
    set_skirmish_slot_updates_enabled, skirmish_button_pushed, skirmish_is_initing,
    skirmish_is_shutting_down, skirmish_slot_updates_enabled,
};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;
const MAX_SLOTS: usize = 8;
const DIFFICULTY_NORMAL: i32 = 1;
const MAX_FPS_SLIDER_VALUE: i32 = 60;
const GREATER_NO_FPS_LIMIT: i32 = 1000;

/// Window state stored in thread-local RefCell (single-threaded GUI)
#[derive(Default)]
struct SkirmishGameOptionsState {
    parent_id: i32,
    button_start_id: i32,
    button_reset_id: i32,
    button_back_id: i32,
    button_select_map_id: i32,
    text_entry_map_display_id: i32,
    text_entry_player_name_id: i32,
    map_window_id: i32,
    listbox_info_id: i32,
    slider_game_speed_id: i32,
    static_text_game_speed_id: i32,
    static_text_streak_id: i32,
    static_text_best_streak_id: i32,
    static_text_wins_id: i32,
    static_text_losses_id: i32,
    checkbox_limit_superweapons_id: i32,
    combo_box_starting_cash_id: i32,
    start_position_ids: [i32; MAX_SLOTS],
    combo_box_player_ids: [i32; MAX_SLOTS],
    combo_box_color_ids: [i32; MAX_SLOTS],
    combo_box_team_ids: [i32; MAX_SLOTS],
    combo_box_template_ids: [i32; MAX_SLOTS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    map_window: Option<Rc<RefCell<GameWindow>>>,
    text_entry_map_display: Option<Rc<RefCell<GameWindow>>>,
    text_entry_player_name: Option<Rc<RefCell<GameWindow>>>,
    static_text_game_speed: Option<Rc<RefCell<GameWindow>>>,
    listbox_info: Option<Rc<RefCell<GameWindow>>>,
    selected_map: Option<String>,
    just_entered: bool,
    initial_gadget_delay: i32,
    still_needs_to_set_options: bool,
    is_shutting_down: bool,
    button_pushed: bool,
}

/// Thread-local state using RefCell - zero-overhead for single-threaded GUI
thread_local! {
    static SKIRMISH_GAME_OPTIONS_STATE: RefCell<SkirmishGameOptionsState> =
        RefCell::new(SkirmishGameOptionsState::default());
    static SKIRMISH_MAP_SELECT_LAYOUT: RefCell<Option<Rc<RefCell<WindowLayout>>>> =
        const { RefCell::new(None) };
}

/// Access state with closure
fn with_state<R>(f: impl FnOnce(&mut SkirmishGameOptionsState) -> R) -> R {
    SKIRMISH_GAME_OPTIONS_STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Access state immutably
fn with_state_ref<R>(f: impl FnOnce(&SkirmishGameOptionsState) -> R) -> R {
    SKIRMISH_GAME_OPTIONS_STATE.with(|s| f(&s.borrow()))
}

fn with_skirmish_map_select_layout<R>(
    f: impl FnOnce(&mut Option<Rc<RefCell<WindowLayout>>>) -> R,
) -> R {
    SKIRMISH_MAP_SELECT_LAYOUT.with(|layout| f(&mut layout.borrow_mut()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn set_window_image(win: &Option<Rc<RefCell<GameWindow>>>, image_name: &str) {
    let Some(win) = win else {
        return;
    };
    if image_name.is_empty() {
        return;
    }

    let (width, height) = if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            let size = found.get_image_size();
            (size.x, size.y)
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };
    let image = WindowImage {
        name: image_name.to_string(),
        width,
        height,
    };

    let mut win_guard = win.borrow_mut();
    if win_guard.set_enabled_image(0, image).is_ok() {
        win_guard.set_status(WindowStatus::IMAGE);
    }
}

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn format_game_text_number(key: &str, value: i32) -> String {
    let template = GameText::fetch(key);
    if template.contains("%d") {
        template.replace("%d", &value.to_string())
    } else {
        format!("{} {}", template, value)
    }
}

fn position_start_buttons(state: &mut SkirmishGameOptionsState, meta: Option<&MapMetaData>) {
    let Some(map_window) = state.map_window.as_ref() else {
        return;
    };
    let map_guard = map_window.borrow();
    let (map_x, map_y) = map_guard.get_screen_position();
    let (map_w, map_h) = map_guard.get_size();

    let extent = meta.map(|meta| meta.extent).unwrap_or_default();
    let (ul, lr) = find_draw_positions(map_x, map_y, map_w, map_h, extent);
    let extent_width = (extent.hi.x - extent.lo.x).max(1.0);
    let extent_height = (extent.hi.y - extent.lo.y).max(1.0);

    for i in 0..MAX_SLOTS {
        let Some(button) =
            with_window_manager(|manager| manager.get_window_by_id(state.start_position_ids[i]))
        else {
            continue;
        };
        let mut button_guard = button.borrow_mut();
        let waypoint = meta.and_then(|meta| meta.get_waypoint(&map_start_waypoint_name(i)));
        if let Some(coord) = waypoint {
            let ratio_x = (coord.x - extent.lo.x) / extent_width;
            let ratio_y = (extent.hi.y - coord.y) / extent_height;
            let draw_x = ul.x as f32 + (lr.x - ul.x) as f32 * ratio_x;
            let draw_y = ul.y as f32 + (lr.y - ul.y) as f32 * ratio_y;
            let (btn_w, btn_h) = button_guard.get_size();
            let new_x = draw_x.round() as i32 - btn_w / 2 - map_x;
            let new_y = draw_y.round() as i32 - btn_h / 2 - map_y;
            let _ = button_guard.set_position(new_x, new_y);
            let _ = button_guard.hide(false);
            let _ = button_guard.enable(true);
        } else {
            let _ = button_guard.hide(true);
            let _ = button_guard.enable(false);
        }
    }
}

fn update_map_start_spots(state: &mut SkirmishGameOptionsState, meta: Option<&MapMetaData>) {
    for button_id in state.start_position_ids {
        if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id)) {
            let mut guard = button.borrow_mut();
            let _ = guard.set_text("");
            guard.set_tooltip(&GameText::fetch("TOOLTIP:StartPosition"));
        }
    }

    let Some(meta) = meta else {
        return;
    };
    let max_players = meta.num_players.max(0) as i32;
    let setup = get_skirmish_setup();
    let info = setup.game_info().game_info();
    for i in 0..MAX_SLOTS {
        let Some(slot) = info.get_slot(i) else {
            continue;
        };
        let pos = slot.get_start_pos();
        if pos >= 0 && pos < max_players && slot.get_player_template() > PLAYERTEMPLATE_MIN {
            let button_id = state.start_position_ids[pos as usize];
            if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id))
            {
                let mut guard = button.borrow_mut();
                let number_key = format!("NUMBER:{}", i + 1);
                let label = GameText::fetch(&number_key);
                let _ = guard.set_text(&label);
                let tooltip = format_game_text_number("TOOLTIP:StartPositionN", (i + 1) as i32);
                guard.set_tooltip(&tooltip);
            }
        }
    }
}

fn map_display_name(map_name: &str, meta: Option<&MapMetaData>) -> String {
    if let Some(meta) = meta {
        if !meta.display_name.is_empty() {
            return meta.display_name.as_str().to_string();
        }
    }
    map_name
        .rsplit('/')
        .next()
        .unwrap_or(map_name)
        .trim_end_matches(".map")
        .to_string()
}

fn update_map_preview(state: &mut SkirmishGameOptionsState) {
    let Some(map_name) = state.selected_map.clone() else {
        return;
    };
    let preview_name = get_map_preview_image(&map_name).unwrap_or_default();
    set_window_image(&state.map_window, &preview_name);
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let meta = cache_guard.find_map(&map_name);
    position_start_buttons(state, meta.as_ref());
    update_map_start_spots(state, meta.as_ref());
    if let Some(text_entry) = state.text_entry_map_display.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().static_text_mut() {
            let label = map_display_name(&map_name, meta.as_ref());
            widget.set_text(label);
        }
    }
}

fn ensure_default_slots() {
    let mut setup = get_skirmish_setup();
    let info = setup.game_info_mut().game_info_mut();
    let has_players = info
        .get_slot(0)
        .map(|slot| slot.is_occupied())
        .unwrap_or(false);
    if !has_players {
        if let Some(slot) = info.get_slot_mut(0) {
            slot.set_state(SlotState::Player, GameText::fetch("GUI:Player"), 0);
            slot.set_color(0);
        }
        for index in 1..MAX_SLOTS {
            if let Some(slot) = info.get_slot_mut(index) {
                slot.set_state(SlotState::Open, String::new(), 0);
            }
        }
    }
}

fn combo_selected_id(window_id: i32) -> Option<u32> {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).and_then(|window| {
        let guard = window.borrow();
        guard.widget().and_then(|widget| match widget {
            crate::gui::WindowWidget::ComboBox(combo) => combo.selected_id(),
            _ => None,
        })
    })
}

fn checkbox_is_checked(window_id: i32) -> Option<bool> {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).and_then(|window| {
        let guard = window.borrow();
        guard.widget().and_then(|widget| match widget {
            crate::gui::WindowWidget::CheckBox(check) => Some(check.is_checked()),
            _ => None,
        })
    })
}

fn local_player_name(state: &SkirmishGameOptionsState) -> String {
    if let Some(text_entry) = state.text_entry_player_name.as_ref() {
        if let Some(widget) = text_entry.borrow().widget() {
            if let crate::gui::WindowWidget::TextEntry(entry) = widget {
                return entry.text().to_string();
            }
        }
    }
    GameText::fetch("GUI:Player")
}

fn populate_player_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        for state in [
            SlotState::Open,
            SlotState::Closed,
            SlotState::EasyAI,
            SlotState::MedAI,
            SlotState::BrutalAI,
            SlotState::Player,
        ] {
            let label = match state {
                SlotState::Open => GameText::fetch("GUI:Open"),
                SlotState::Closed => GameText::fetch("GUI:Closed"),
                SlotState::EasyAI => GameText::fetch("GUI:EasyAI"),
                SlotState::MedAI => GameText::fetch("GUI:MediumAI"),
                SlotState::BrutalAI => GameText::fetch("GUI:HardAI"),
                SlotState::Player => GameText::fetch("GUI:Player"),
            };
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(state as u32, label));
        }
    });
}

fn populate_color_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            u32::MAX,
            GameText::fetch("GUI:Random"),
        ));
        for color in PlayerColor::all() {
            let label = GameText::fetch(&format!("GUI:{}", color.name()));
            let display = if label.starts_with("GUI:") {
                color.name().to_string()
            } else {
                label
            };
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                *color as u32,
                display,
            ));
        }
    });
}

fn populate_template_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            u32::MAX,
            GameText::fetch("GUI:Random"),
        ));
        let store = get_player_template_store();
        if store.is_empty() {
            for (index, name) in ["USA", "China", "GLA"].iter().enumerate() {
                combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                    index as u32,
                    name.to_string(),
                ));
            }
        } else {
            for index in 0..store.len() {
                if let Some(template) = store.get_nth_player_template(index) {
                    if !template.playable {
                        continue;
                    }
                    combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                        index as u32,
                        template.get_display_name().to_string(),
                    ));
                }
            }
        }
    });
}

fn populate_team_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            0,
            "None".to_string(),
        ));
        let base_label = GameText::fetch("GUI:Team");
        for i in 1..=MAX_SLOTS {
            let label = if base_label.starts_with("GUI:") {
                format!("Team {}", i)
            } else {
                format!("{} {}", base_label, i)
            };
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(i as u32, label));
        }
    });
}

fn populate_slot_controls(state: &mut SkirmishGameOptionsState) {
    for i in 0..MAX_SLOTS {
        populate_player_combo(state.combo_box_player_ids[i]);
        populate_color_combo(state.combo_box_color_ids[i]);
        populate_template_combo(state.combo_box_template_ids[i]);
        populate_team_combo(state.combo_box_team_ids[i]);
        update_slot_selection(state, i);
    }
}

fn update_slot_selection(state: &mut SkirmishGameOptionsState, index: usize) {
    let setup = get_skirmish_setup();
    let Some(slot) = setup.game_info().game_info().get_slot(index) else {
        return;
    };
    let player_id = slot.get_state() as u32;
    let color_id = if slot.get_color() < 0 {
        u32::MAX
    } else {
        slot.get_color() as u32
    };
    let template_id = if slot.get_player_template() < 0 {
        u32::MAX
    } else {
        slot.get_player_template() as u32
    };
    let team_id = if slot.get_team_number() < 0 {
        0
    } else {
        (slot.get_team_number() + 1) as u32
    };

    for (id, value) in [
        (state.combo_box_player_ids[index], player_id),
        (state.combo_box_color_ids[index], color_id),
        (state.combo_box_template_ids[index], template_id),
        (state.combo_box_team_ids[index], team_id),
    ] {
        with_window_manager(|manager| manager.get_window_by_id(id)).map(|window| {
            let mut guard = window.borrow_mut();
            if let Some(combo) = guard.combo_box_mut() {
                let _ = combo.select_item(value);
            }
        });
    }

    let enable = slot.get_state() != SlotState::Closed;
    for id in [
        state.combo_box_color_ids[index],
        state.combo_box_template_ids[index],
        state.combo_box_team_ids[index],
    ] {
        with_window_manager(|manager| manager.get_window_by_id(id)).map(|window| {
            let _ = window.borrow_mut().enable(enable);
        });
    }
}

fn populate_global_controls() {
    if let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(name_to_id(
            "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
        ))
    }) {
        let mut guard = window.borrow_mut();
        if let Some(combo) = guard.combo_box_mut() {
            combo.clear();
            for amount in [10000u32, 20000, 30000, 40000, 50000] {
                combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                    amount,
                    format!("${}", amount),
                ));
            }
            let setup = get_skirmish_setup();
            let selected = setup
                .game_info()
                .game_info()
                .get_starting_cash()
                .count_money();
            let _ = combo.select_item(selected);
        }
    }

    if let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(name_to_id(
            "SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
        ))
    }) {
        let mut guard = window.borrow_mut();
        if let Some(check) = guard.check_box_mut() {
            let setup = get_skirmish_setup();
            let enabled = setup.game_info().game_info().get_superweapon_restriction() != 0;
            check.set_checked(enabled);
        }
    }
}

fn sync_map_metadata(map_name: &str) {
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let mut setup = get_skirmish_setup();
    let info = setup.game_info_mut().game_info_mut();
    if let Some(meta) = cache_guard.find_map(map_name) {
        info.set_map_crc(meta.crc);
        info.set_map_size(meta.filesize);
    } else {
        info.set_map_crc(0);
        info.set_map_size(0);
    }
}

fn sync_map_to_game_info(state: &SkirmishGameOptionsState) {
    if let Some(map) = state.selected_map.as_ref() {
        {
            let mut setup = get_skirmish_setup();
            let info = setup.game_info_mut().game_info_mut();
            info.set_map(map.clone());
            info.reset_start_spots();
            info.adjust_slots_for_map();
        }
        sync_map_metadata(map);
    }
}

pub(crate) fn show_skirmish_game_options_underlying_gui_elements(show: bool) {
    const LAYOUT: &str = "SkirmishGameOptionsMenu.wnd:";
    let gadgets = [
        "MapWindow",
        "StaticTextTeam",
        "StaticTextFaction",
        "StaticTextColor",
        "TextEntryMapDisplay",
        "ButtonSelectMap",
        "ButtonStart",
        "StaticTextMapPreview",
    ];

    with_window_manager(|manager| {
        for name in gadgets {
            let id = name_to_id(&format!("{LAYOUT}{name}"));
            if let Some(win) = manager.get_window_by_id(id) {
                let mut guard = win.borrow_mut();
                let _ = guard.hide(!show);
                let _ = guard.enable(show);
            }
        }

        for i in 0..MAX_SLOTS {
            for base in ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"] {
                let id = name_to_id(&format!("{LAYOUT}{base}{i}"));
                if let Some(win) = manager.get_window_by_id(id) {
                    let mut guard = win.borrow_mut();
                    let _ = guard.hide(!show);
                    let _ = guard.enable(show);
                }
            }
        }

        for name in ["ButtonReset", "ButtonBack"] {
            let id = name_to_id(&format!("{LAYOUT}{name}"));
            if let Some(win) = manager.get_window_by_id(id) {
                let _ = win.borrow_mut().enable(show);
            }
        }
    });
}

pub(crate) fn destroy_skirmish_map_select_overlay() {
    with_skirmish_map_select_layout(|layout| {
        let Some(layout) = layout.take() else {
            return;
        };
        with_window_manager(|manager| manager.destroy_layout(&layout));
    });
}

pub(crate) fn refresh_skirmish_game_options_from_setup() {
    with_state(|state| {
        let selected_map = {
            let setup = get_skirmish_setup();
            setup.selected_map().to_string()
        };

        if !selected_map.is_empty() {
            state.selected_map = Some(selected_map);
            update_map_preview(state);
            sync_map_to_game_info(state);
            skirmish_update_slot_list(state);
        }
    });
}

fn set_fps_text(state: &SkirmishGameOptionsState, slider_pos: i32) {
    let Some(window) = state.static_text_game_speed.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if slider_pos > 60 {
        let _ = guard.set_text("--");
        let _ = guard.enable(true);
        return;
    }

    let default_limit = game_engine::common::ini::get_global_data()
        .map(|data| data.read().frames_per_second_limit)
        .unwrap_or(30);
    if slider_pos == default_limit {
        let _ = guard.enable(false);
    } else {
        let _ = guard.enable(true);
    }
    let _ = guard.set_text(&format!("{:2}", slider_pos));
}

fn set_gadget_visible(name: &str, visible: bool) {
    let id = name_to_id(&format!("SkirmishGameOptionsMenu.wnd:{}", name));
    if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
        let _ = window.borrow_mut().hide(!visible);
    }
}

fn set_per_player_visible(base: &str, visible: bool) {
    for i in 0..MAX_SLOTS {
        let id = name_to_id(&format!("SkirmishGameOptionsMenu.wnd:{}{}", base, i));
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let _ = window.borrow_mut().hide(!visible);
        }
    }
}

fn update_skirmish_game_options(state: &SkirmishGameOptionsState) {
    let map_name = state.selected_map.clone().unwrap_or_else(|| {
        let setup = get_skirmish_setup();
        setup.game_info().game_info().get_map().to_string()
    });
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let is_skirmish = cache_guard
        .find_map(&map_name)
        .map(|meta| meta.is_multiplayer)
        .unwrap_or(true);

    if is_skirmish {
        set_gadget_visible("TextEntryPlayerName", true);
        set_gadget_visible("StaticTextPlayers", true);
        set_gadget_visible("StaticTextColor", true);
        set_gadget_visible("StaticTextTeam", true);
        set_gadget_visible("StaticTextFaction", true);
        set_per_player_visible("ComboBoxPlayer", true);
        set_per_player_visible("ComboBoxTeam", true);
        set_per_player_visible("ComboBoxColor", true);
        set_per_player_visible("ComboBoxPlayerTemplate", true);
    } else {
        set_gadget_visible("TextEntryPlayerName", false);
        set_gadget_visible("StaticTextPlayers", false);
        set_gadget_visible("StaticTextColor", false);
        set_gadget_visible("StaticTextTeam", false);
        set_gadget_visible("StaticTextFaction", false);
        set_per_player_visible("ComboBoxPlayer", false);
        set_per_player_visible("ComboBoxTeam", false);
        set_per_player_visible("ComboBoxColor", false);
        set_per_player_visible("ComboBoxPlayerTemplate", false);

        let mut setup = get_skirmish_setup();
        let info = setup.game_info_mut().game_info_mut();
        for i in 1..MAX_SLOTS {
            if let Some(slot) = info.get_slot_mut(i) {
                slot.set_state(SlotState::Open, String::new(), 0);
            }
        }
    }

    populate_global_controls();
}

fn skirmish_update_slot_list(state: &mut SkirmishGameOptionsState) {
    // Guard checks matching C++: if(!AreSlotListUpdatesEnabled() || s_isIniting) return;
    if !skirmish_slot_updates_enabled() || skirmish_is_initing() {
        return;
    }

    if let Some(text_entry) = state.text_entry_player_name.as_ref() {
        let setup = get_skirmish_setup();
        if let Some(slot) = setup.game_info().game_info().get_slot(0) {
            let mut guard = text_entry.borrow_mut();
            if let Some(entry) = guard.text_entry_mut() {
                entry.set_text(slot.get_name().to_string());
            }
        }
    }

    let setup = get_skirmish_setup();
    let info = setup.game_info().game_info();
    let map_name = info.get_map().to_string();
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let meta = cache_guard.find_map(&map_name);
    if let Some(text_entry) = state.text_entry_map_display.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().static_text_mut() {
            let label = map_display_name(&map_name, meta.as_ref());
            widget.set_text(label);
        }
    }

    for i in 0..MAX_SLOTS {
        update_slot_selection(state, i);
    }
    update_map_start_spots(state, meta.as_ref());
    update_skirmish_game_options(state);
}

fn populate_skirmish_battle_honors(state: &SkirmishGameOptionsState) {
    let stats = SkirmishBattleHonors::new();
    let honors = stats.get_honors();

    if let Some(listbox) = state.listbox_info.as_ref() {
        if let Some(widget) = listbox.borrow_mut().list_box_mut() {
            widget.clear();
            let entries = [
                ("Campaign China", BATTLE_HONOR_CAMPAIGN_CHINA),
                ("Campaign GLA", BATTLE_HONOR_CAMPAIGN_GLA),
                ("Campaign USA", BATTLE_HONOR_CAMPAIGN_USA),
                ("Challenge Mode", BATTLE_HONOR_CHALLENGE_MODE),
                ("Air Wing", BATTLE_HONOR_AIR_WING),
                ("Battle Tank", BATTLE_HONOR_BATTLE_TANK),
                ("Endurance", BATTLE_HONOR_ENDURANCE),
                ("Apocalypse", BATTLE_HONOR_APOCALYPSE),
                ("Blitz 5", BATTLE_HONOR_BLITZ5),
                ("Blitz 10", BATTLE_HONOR_BLITZ10),
                ("Streak", BATTLE_HONOR_STREAK),
                ("Domination", BATTLE_HONOR_DOMINATION),
                ("Ultimate", BATTLE_HONOR_ULTIMATE),
                ("Officers Club", BATTLE_HONOR_OFFICERSCLUB),
            ];
            for (label, flag) in entries {
                let status = if honors & flag != 0 { "[X]" } else { "[ ]" };
                widget.add_item_with_id(-1, &format!("{} {}", status, label));
            }
        }
    }

    for (name, value) in [
        (
            "SkirmishGameOptionsMenu.wnd:StaticTextStreakValue",
            stats.get_win_streak(),
        ),
        (
            "SkirmishGameOptionsMenu.wnd:StaticTextBestStreakValue",
            stats.get_best_win_streak(),
        ),
        (
            "SkirmishGameOptionsMenu.wnd:StaticTextWinsValue",
            stats.get_wins(),
        ),
        (
            "SkirmishGameOptionsMenu.wnd:StaticTextLossesValue",
            stats.get_losses(),
        ),
    ] {
        let id = name_to_id(name);
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let _ = window.borrow_mut().set_text(&format!("{}", value));
        }
    }
}

fn handle_combo_selection(state: &mut SkirmishGameOptionsState, control_id: i32) -> bool {
    for index in 0..MAX_SLOTS {
        if control_id == state.combo_box_player_ids[index] {
            if index == 0 {
                return true;
            }
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_skirmish_setup();
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(index) {
                    let state_value = match selected {
                        x if x == SlotState::Open as u32 => SlotState::Open,
                        x if x == SlotState::Closed as u32 => SlotState::Closed,
                        x if x == SlotState::EasyAI as u32 => SlotState::EasyAI,
                        x if x == SlotState::MedAI as u32 => SlotState::MedAI,
                        x if x == SlotState::BrutalAI as u32 => SlotState::BrutalAI,
                        x if x == SlotState::Player as u32 => SlotState::Player,
                        _ => SlotState::Open,
                    };
                    let name = if state_value == SlotState::Player {
                        local_player_name(state)
                    } else {
                        String::new()
                    };
                    slot.set_state(state_value, name, 0);
                }
                update_slot_selection(state, index);
            }
            return true;
        }
        if control_id == state.combo_box_color_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_skirmish_setup();
                let color = if selected == u32::MAX {
                    -1
                } else {
                    selected as i32
                };
                let color_taken = color >= 0
                    && setup
                        .game_info()
                        .game_info()
                        .is_color_taken(color, index as i32);
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(index) {
                    if color_taken {
                        update_slot_selection(state, index);
                    } else {
                        slot.set_color(color);
                    }
                }
            }
            return true;
        }
        if control_id == state.combo_box_template_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_skirmish_setup();
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(index) {
                    let template = if selected == u32::MAX {
                        -1
                    } else {
                        selected as i32
                    };
                    slot.set_player_template(template);
                }
            }
            return true;
        }
        if control_id == state.combo_box_team_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_skirmish_setup();
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(index) {
                    let team = if selected == 0 {
                        -1
                    } else {
                        selected as i32 - 1
                    };
                    slot.set_team_number(team);
                }
            }
            return true;
        }
    }

    if control_id == state.combo_box_starting_cash_id {
        if let Some(selected) = combo_selected_id(control_id) {
            let mut setup = get_skirmish_setup();
            setup
                .game_info_mut()
                .game_info_mut()
                .set_starting_cash(Money::new(selected));
        }
        return true;
    }
    if control_id == state.checkbox_limit_superweapons_id {
        if let Some(checked) = checkbox_is_checked(control_id) {
            let mut setup = get_skirmish_setup();
            setup
                .game_info_mut()
                .game_info_mut()
                .set_superweapon_restriction(if checked { 1 } else { 0 });
        }
        return true;
    }

    false
}

fn next_selectable_player(info: &game_network::GameInfo, start: usize) -> Option<usize> {
    if !info.am_i_host() {
        return None;
    }
    for index in start..MAX_SLOTS {
        if let Some(slot) = info.get_slot(index) {
            if slot.get_start_pos() == -1
                && (index as i32 == info.get_local_slot_num() || slot.is_ai())
            {
                return Some(index);
            }
        }
    }
    None
}

fn set_start_position(info: &mut game_network::GameInfo, index: usize, position: i32) {
    let Some(current_pos) = info.get_slot(index).map(|slot| slot.get_start_pos()) else {
        return;
    };
    if position == current_pos {
        return;
    }
    if position >= 0 && info.is_start_position_taken(position, index as i32) {
        return;
    }
    if let Some(slot) = info.get_slot_mut(index) {
        slot.set_start_pos(if position < 0 { -1 } else { position });
    }
}

fn handle_start_position_click(state: &mut SkirmishGameOptionsState, control_id: i32) -> bool {
    for (index, button_id) in state.start_position_ids.iter().enumerate() {
        if control_id == *button_id {
            let mut setup = get_skirmish_setup();
            let info = setup.game_info_mut().game_info_mut();
            let mut player_in_pos: Option<usize> = None;
            for slot_index in 0..MAX_SLOTS {
                if let Some(slot) = info.get_slot(slot_index) {
                    if slot.get_start_pos() == index as i32 {
                        player_in_pos = Some(slot_index);
                        break;
                    }
                }
            }
            if let Some(player_idx) = player_in_pos {
                let is_local = player_idx as i32 == info.get_local_slot_num();
                let can_move = is_local
                    || (info.am_i_host()
                        && info
                            .get_slot(player_idx)
                            .map(|slot| slot.is_ai())
                            .unwrap_or(false));
                if can_move {
                    let next_player = next_selectable_player(info, player_idx + 1);
                    set_start_position(info, player_idx, -1);
                    if let Some(next) = next_player {
                        set_start_position(info, next, index as i32);
                    }
                }
            } else {
                let mut next_player = next_selectable_player(info, 0);
                if next_player.is_none() {
                    let local = info.get_local_slot_num();
                    if local >= 0 {
                        next_player = Some(local as usize);
                    }
                }
                if let Some(next) = next_player {
                    set_start_position(info, next, index as i32);
                }
            }
            return true;
        }
    }
    false
}

fn handle_start_position_right_click(
    state: &mut SkirmishGameOptionsState,
    control_id: i32,
) -> bool {
    for (index, button_id) in state.start_position_ids.iter().enumerate() {
        if control_id == *button_id {
            let mut setup = get_skirmish_setup();
            let info = setup.game_info_mut().game_info_mut();
            let mut player_in_pos: Option<usize> = None;
            for slot_index in 0..MAX_SLOTS {
                if let Some(slot) = info.get_slot(slot_index) {
                    if slot.get_start_pos() == index as i32 {
                        player_in_pos = Some(slot_index);
                        break;
                    }
                }
            }
            if let Some(player_idx) = player_in_pos {
                let is_local = player_idx as i32 == info.get_local_slot_num();
                let can_move = is_local
                    || (info.am_i_host()
                        && info
                            .get_slot(player_idx)
                            .map(|slot| slot.is_ai())
                            .unwrap_or(false));
                if can_move {
                    set_start_position(info, player_idx, -1);
                }
            }
            return true;
        }
    }
    false
}

fn choose_default_map(state: &mut SkirmishGameOptionsState) {
    if state.selected_map.is_some() {
        return;
    }
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let mut candidates: Vec<_> = cache_guard
        .iter_maps()
        .into_iter()
        .filter(|(_, meta)| meta.is_multiplayer)
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some((name, _)) = candidates.first() {
        state.selected_map = Some(name.clone());
        let mut setup = get_skirmish_setup();
        setup.set_selected_map(name.clone());
    }
}

fn apply_skirmish_preferences(state: &mut SkirmishGameOptionsState) {
    let mut prefs = SkirmishPreferences::new();
    let map_name = prefs.get_preferred_map();
    let uses_system_maps = prefs.uses_system_map_dir();
    let starting_cash = prefs.get_starting_cash();
    let superweapon_restricted = prefs.get_superweapon_restricted();
    {
        let mut setup = get_skirmish_setup();
        {
            let info = setup.game_info_mut().game_info_mut();

            info.init();
            info.clear_slot_list();
            info.reset();
            let local_ip = info.get_slot(0).map(|slot| slot.get_ip()).unwrap_or(0);
            info.set_local_ip(local_ip);
            info.enter_game();

            if let Some(slot) = info.get_slot_mut(0) {
                let user_name = prefs.get_user_name();
                slot.set_state(SlotState::Player, user_name.clone(), local_ip);
                slot.set_color(prefs.get_preferred_color());
                slot.set_player_template(prefs.get_preferred_faction());
            }

            let honors = SkirmishBattleHonors::new();
            let ai_state = if honors.get_wins() > 10 {
                SlotState::BrutalAI
            } else if honors.get_wins() > 5 {
                SlotState::MedAI
            } else {
                SlotState::EasyAI
            };
            if let Some(slot) = info.get_slot_mut(1) {
                slot.set_state(ai_state, String::new(), 0);
            }

            let slot_list = prefs.get_slot_list();
            if !slot_list.is_empty() {
                let _ = parse_ascii_string_to_game_info(info, &slot_list);
            }

            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i32;
            info.set_seed(seed);
            info.set_map(map_name.clone());
            info.set_starting_cash(starting_cash);
            info.set_superweapon_restriction(if superweapon_restricted { 1 } else { 0 });
        }
        setup.set_selected_map(map_name.clone());
        setup.set_use_system_maps(uses_system_maps);
    }
    sync_map_metadata(&map_name);
    state.selected_map = Some(map_name);
}

fn write_skirmish_preferences(state: &SkirmishGameOptionsState) {
    let mut prefs = SkirmishPreferences::new();
    let setup = get_skirmish_setup();
    let info = setup.game_info().game_info();

    if let Some(slot) = info.get_slot(0) {
        prefs.set_user_name(slot.get_name().to_string());
        prefs.set_preferred_color(slot.get_color());
        prefs.set_preferred_faction(slot.get_player_template());
    }
    prefs.set_starting_cash(*info.get_starting_cash());
    prefs.set_superweapon_restricted(info.get_superweapon_restriction() != 0);
    prefs.set_preferred_map(info.get_map().to_string());
    prefs.set_slot_list(game_info_to_ascii_string(info));
    prefs.set_use_system_map_dir(setup.use_system_maps());

    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.slider_game_speed_id))
    {
        let mut guard = window.borrow_mut();
        if let Some(slider) = guard.horizontal_slider_mut() {
            prefs.set_int("FPS", slider.value());
        }
    }

    prefs.write();
}

fn start_skirmish_game(state: &mut SkirmishGameOptionsState) {
    let Some(map_name) = state.selected_map.clone() else {
        state.button_pushed = false;
        return;
    };

    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    let Some(meta) = cache_guard.find_map(&map_name) else {
        state.button_pushed = false;
        let _ = message_box_ok(
            &GameText::fetch("GUI:ErrorStartingGame"),
            &GameText::fetch("GUI:CantFindMap"),
            None,
        );
        return;
    };

    let player_count = {
        let setup = get_skirmish_setup();
        setup.game_info().game_info().get_num_players()
    };
    if player_count > meta.num_players.max(0) as usize {
        state.button_pushed = false;
        let body = format!(
            "{} {}",
            GameText::fetch("GUI:TooManyPlayers"),
            meta.num_players.max(0)
        );
        let _ = message_box_ok(&GameText::fetch("GUI:ErrorStartingGame"), &body, None);
        return;
    }
    if !check_for_cd_at_game_start(state) {
        return;
    }

    let mut max_fps = with_window_manager(|manager| manager.get_window_by_id(state.slider_game_speed_id))
        .and_then(|slider_window| {
            let mut slider_guard = slider_window.borrow_mut();
            slider_guard.horizontal_slider_mut().map(|slider| slider.value())
        })
        .unwrap_or(MAX_FPS_SLIDER_VALUE);
    if max_fps > MAX_FPS_SLIDER_VALUE {
        max_fps = GREATER_NO_FPS_LIMIT;
    }
    if max_fps < 15 {
        max_fps = 15;
    }

    let mut is_skirmish = true;
    if !meta.is_multiplayer {
        is_skirmish = false;
    }

    if TheGameLogic::is_in_game() {
        let _ = TheGameLogic::clear_game_data();
    }

    sync_map_to_game_info(state);
    let seed = {
        let mut setup = get_skirmish_setup();
        let info = setup.game_info_mut().game_info_mut();
        info.start_game(0);
        info.get_seed() as u32
    };
    init_random_with_seed(seed);
    write_skirmish_preferences(state);

    if let Some(data) = game_engine::common::ini::get_global_data() {
        let mut data = data.write();
        data.pending_file = map_name;
    }

    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap();
    let msg = stream.append_message(GameMessageType::NewGame);
    msg.append_integer_argument(if is_skirmish {
        GAME_SKIRMISH
    } else {
        GAME_SINGLE_PLAYER
    });
    msg.append_integer_argument(DIFFICULTY_NORMAL);
    msg.append_integer_argument(0);
    msg.append_integer_argument(max_fps);
}

fn is_first_cd_present() -> bool {
    get_protection_manager()
        .map(|mut manager| manager.comprehensive_validation().status == ProtectionStatus::Valid)
        .unwrap_or(true)
}

fn check_for_cd_at_game_start(state: &mut SkirmishGameOptionsState) -> bool {
    if is_first_cd_present() {
        return true;
    }
    state.button_pushed = false;
    let _ = message_box_ok_cancel(
        &GameText::fetch("GUI:InsertCDPrompt"),
        &GameText::fetch("GUI:InsertCDMessage"),
        None,
        Some(Box::new(|| {})),
    );
    false
}

pub fn skirmish_game_options_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    with_state(|state| {
        state.parent_id = name_to_id("SkirmishGameOptionsMenu.wnd:SkirmishGameOptionsMenuParent");
        state.button_start_id = name_to_id("SkirmishGameOptionsMenu.wnd:ButtonStart");
        state.button_reset_id = name_to_id("SkirmishGameOptionsMenu.wnd:ButtonReset");
        state.button_back_id = name_to_id("SkirmishGameOptionsMenu.wnd:ButtonBack");
        state.button_select_map_id = name_to_id("SkirmishGameOptionsMenu.wnd:ButtonSelectMap");
        state.text_entry_map_display_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:TextEntryMapDisplay");
        state.text_entry_player_name_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:TextEntryPlayerName");
        state.map_window_id = name_to_id("SkirmishGameOptionsMenu.wnd:MapWindow");
        state.listbox_info_id = name_to_id("SkirmishGameOptionsMenu.wnd:ListboxInfo");
        state.slider_game_speed_id = name_to_id("SkirmishGameOptionsMenu.wnd:SliderGameSpeed");
        state.static_text_game_speed_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:StaticTextGameSpeed");
        state.static_text_streak_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:StaticTextStreakValue");
        state.static_text_best_streak_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:StaticTextBestStreakValue");
        state.static_text_wins_id = name_to_id("SkirmishGameOptionsMenu.wnd:StaticTextWinsValue");
        state.static_text_losses_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:StaticTextLossesValue");
        state.checkbox_limit_superweapons_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons");
        state.combo_box_starting_cash_id =
            name_to_id("SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash");
        for i in 0..MAX_SLOTS {
            state.start_position_ids[i] = name_to_id(&format!(
                "SkirmishGameOptionsMenu.wnd:ButtonMapStartPosition{}",
                i
            ));
            state.combo_box_team_ids[i] =
                name_to_id(&format!("SkirmishGameOptionsMenu.wnd:ComboBoxTeam{}", i));
            state.combo_box_template_ids[i] = name_to_id(&format!(
                "SkirmishGameOptionsMenu.wnd:ComboBoxPlayerTemplate{}",
                i
            ));
            state.combo_box_player_ids[i] =
                name_to_id(&format!("SkirmishGameOptionsMenu.wnd:ComboBoxPlayer{}", i));
            state.combo_box_color_ids[i] =
                name_to_id(&format!("SkirmishGameOptionsMenu.wnd:ComboBoxColor{}", i));
        }

        with_window_manager(|manager| {
            state.parent = manager.get_window_by_id(state.parent_id);
            state.map_window = manager.get_window_by_id(state.map_window_id);
            state.text_entry_map_display =
                manager.get_window_by_id(state.text_entry_map_display_id);
            state.text_entry_player_name =
                manager.get_window_by_id(state.text_entry_player_name_id);
            state.static_text_game_speed =
                manager.get_window_by_id(state.static_text_game_speed_id);
            state.listbox_info = manager.get_window_by_id(state.listbox_info_id);
            if let Some(parent) = state.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
            if let Some(sub_parent) =
                manager.get_window_by_id(name_to_id("SkirmishGameOptionsMenu.wnd:SubParent"))
            {
                let _ = sub_parent.borrow_mut().hide(true);
            }
        });

        apply_skirmish_preferences(state);
        ensure_default_slots();
        populate_slot_controls(state);
        populate_global_controls();
        choose_default_map(state);
        sync_map_to_game_info(state);
        update_map_preview(state);
        skirmish_update_slot_list(state);
        populate_skirmish_battle_honors(state);

        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.slider_game_speed_id))
        {
            let mut guard = window.borrow_mut();
            if let Some(slider) = guard.horizontal_slider_mut() {
                let default_limit = game_engine::common::ini::get_global_data()
                    .map(|data| data.read().frames_per_second_limit)
                    .unwrap_or(30);
                let prefs = SkirmishPreferences::new();
                let slider_pos = prefs.get_int("FPS", default_limit);
                slider.set_value(slider_pos);
                let id = state.static_text_game_speed_id;
                slider.set_change_callback(move |_, value| {
                    if let Some(window) =
                        with_window_manager(|manager| manager.get_window_by_id(id))
                    {
                        let mut guard = window.borrow_mut();
                        if value > 60 {
                            let _ = guard.set_text("--");
                            let _ = guard.enable(true);
                            return;
                        }
                        if let Some(data) = game_engine::common::ini::get_global_data() {
                            let data = data.read();
                            if value == data.frames_per_second_limit {
                                let _ = guard.enable(false);
                            } else {
                                let _ = guard.enable(true);
                            }
                        }
                        let _ = guard.set_text(&format!("{:2}", value));
                    }
                });
                set_fps_text(state, slider_pos);
            }
        }

        state.is_shutting_down = false;
        state.just_entered = true;
        state.initial_gadget_delay = 2;
        state.still_needs_to_set_options = false;
        state.button_pushed = false;
    });
    get_shell().show_shell_map(true);
    layout.hide(false);
}

pub fn skirmish_game_options_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    with_state(|state| {
        if state.just_entered {
            if state.initial_gadget_delay == 1 {
                state.still_needs_to_set_options = true;
                if let Some(parent) = state.parent.as_ref() {
                    let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
                }
                state.initial_gadget_delay = 2;
                state.just_entered = false;
            } else {
                state.initial_gadget_delay -= 1;
            }
        }

        if state.still_needs_to_set_options && !TheGameLogic::is_loading_map() {
            with_window_manager(|manager| {
                manager.transition_set_group("SkirmishGameOptionsMenuFade", false)
            });
            state.still_needs_to_set_options = false;
        }

        let setup_map = {
            let setup = get_skirmish_setup();
            setup.selected_map().to_string()
        };
        if !setup_map.is_empty() && state.selected_map.as_deref() != Some(setup_map.as_str()) {
            state.selected_map = Some(setup_map);
            update_map_preview(state);
            sync_map_to_game_info(state);
            skirmish_update_slot_list(state);
        }

        if state.is_shutting_down
            && get_shell().is_anim_finished()
            && with_window_manager(|manager| manager.transitions_finished())
        {
            state.is_shutting_down = false;
            layout.hide(true);
            let _ = get_shell().shutdown_complete(None, false);
        }
    });
}

pub fn skirmish_game_options_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        destroy_skirmish_map_select_overlay();
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("SkirmishGameOptionsMenuFade"));

    with_state(|state| state.is_shutting_down = true);
}

pub fn skirmish_game_options_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut handled = false;
    with_state(|state| match msg {
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_start_id {
                if state.button_pushed {
                    handled = true;
                    return;
                }
                state.button_pushed = true;
                start_skirmish_game(state);
                handled = true;
                return;
            }
            if control_id == state.button_back_id {
                if state.button_pushed {
                    handled = true;
                    return;
                }
                state.button_pushed = true;
                write_skirmish_preferences(state);
                destroy_skirmish_map_select_overlay();
                let _ = get_shell().pop();
                {
                    let mut setup = get_skirmish_setup();
                    *setup = Default::default();
                }
                handled = true;
                return;
            }
            if control_id == state.button_reset_id {
                let mut stats = SkirmishBattleHonors::new();
                stats.clear();
                let _ = stats.write();
                populate_skirmish_battle_honors(state);
                handled = true;
                return;
            }
            if control_id == state.button_select_map_id {
                destroy_skirmish_map_select_overlay();
                if let Some((layout, _)) = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/SkirmishMapSelectMenu.wnd")
                        .ok()
                }) {
                    with_skirmish_map_select_layout(|current| *current = Some(layout.clone()));
                    layout.borrow().run_init(None);
                    let mut layout_mut = layout.borrow_mut();
                    layout_mut.hide(false);
                    layout_mut.bring_forward();
                }
                handled = true;
                return;
            }
            if handle_combo_selection(state, control_id) {
                skirmish_update_slot_list(state);
                handled = true;
                return;
            }
            if handle_start_position_click(state, control_id) {
                skirmish_update_slot_list(state);
                handled = true;
                return;
            }
        }
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
            if handle_combo_selection(state, control_id) {
                skirmish_update_slot_list(state);
                handled = true;
                return;
            }
            if control_id == state.text_entry_player_name_id {
                if let Some(text_entry) = state.text_entry_player_name.as_ref() {
                    if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
                        let mut setup = get_skirmish_setup();
                        if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(0) {
                            slot.set_name(widget.text().to_string());
                        }
                    }
                }
                handled = true;
                return;
            }
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == state.text_entry_player_name_id {
                if let Some(text_entry) = state.text_entry_player_name.as_ref() {
                    if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
                        let mut setup = get_skirmish_setup();
                        if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(0) {
                            slot.set_name(widget.text().to_string());
                        }
                    }
                }
                handled = true;
                return;
            }
        }
        WindowMessage::GadgetRightClick => {
            let control_id = data1 as i32;
            if handle_start_position_right_click(state, control_id) {
                skirmish_update_slot_list(state);
                handled = true;
                return;
            }
        }
        _ => {}
    });

    if handled {
        WindowMsgHandled::Handled
    } else {
        WindowMsgHandled::Ignored
    }
}

pub fn skirmish_game_options_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        let key = data1 as u32;
        let state = data2 as u32;
        if key == KEY_ESC && (state & KEY_STATE_UP) != 0 {
            with_state_ref(|state| {
                if let Some(parent) = state.parent.as_ref() {
                    let _ = parent.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        state.button_back_id as u32,
                        state.button_back_id as u32,
                    );
                }
            });
            return WindowMsgHandled::Handled;
        }
    }
    WindowMsgHandled::Ignored
}
