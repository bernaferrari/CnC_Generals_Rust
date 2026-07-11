//! ChallengeMenu.cpp callback port.

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::campaign_manager::{get_campaign_manager, GameDifficulty as CampaignDifficulty};
use crate::gui::challenge_generals::{get_challenge_generals_mut, GameDifficulty, NUM_GENERALS};
use crate::gui::game_window::Image as WindowImage;
use crate::gui::get_skirmish_setup;
use crate::gui::window_video_manager::with_window_video_manager;
use crate::gui::{
    get_shell, with_window_manager, write_input_focus_response, GameWindow, WindowLayout,
    WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use crate::message_stream::{get_message_stream, GameMessageType};
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use game_engine::common::ini::{ensure_player_templates_loaded, get_global_data};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::rts::player_template::{get_player_template_store, PlayerTemplate};
use game_network::{GameSlot, SlotState};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine};
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const GGM_LEFT_DRAG: u32 = 16384;
const GBM_MOUSE_ENTERING: u32 = GGM_LEFT_DRAG + 6;
const GBM_MOUSE_LEAVING: u32 = GGM_LEFT_DRAG + 7;

#[derive(Default)]
struct ChallengeMenuState {
    parent_id: i32,
    button_play_id: i32,
    button_back_id: i32,
    gadget_parent_id: i32,
    bio_parent_id: i32,
    bio_portrait_id: i32,
    bio_name_entry_id: i32,
    bio_dob_entry_id: i32,
    bio_birthplace_entry_id: i32,
    bio_strategy_entry_id: i32,
    general_button_ids: [i32; NUM_GENERALS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_play: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    gadget_parent: Option<Rc<RefCell<GameWindow>>>,
    bio_parent: Option<Rc<RefCell<GameWindow>>>,
    bio_portrait: Option<Rc<RefCell<GameWindow>>>,
    bio_name_entry: Option<Rc<RefCell<GameWindow>>>,
    bio_dob_entry: Option<Rc<RefCell<GameWindow>>>,
    bio_birthplace_entry: Option<Rc<RefCell<GameWindow>>>,
    bio_strategy_entry: Option<Rc<RefCell<GameWindow>>>,
    just_entered: bool,
    initial_gadget_delay: i32,
    is_shutting_down: bool,
    intro_audio_magic_number: i32,
    has_played_intro_audio: bool,
    last_button_index: Option<usize>,
    last_hilited_index: Option<usize>,
    last_selection_sound: u32,
    last_preview_sound: u32,
    bio_lines: [String; 4],
    bio_readout: [String; 4],
    bio_text_position: usize,
    bio_total_length: usize,
}

thread_local! {
    static CHALLENGE_MENU_STATE: Arc<Mutex<ChallengeMenuState>> =
        Arc::new(Mutex::new(ChallengeMenuState::default()));
}

fn challenge_menu_state() -> Arc<Mutex<ChallengeMenuState>> {
    CHALLENGE_MENU_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn challenge_to_campaign_difficulty(diff: GameDifficulty) -> CampaignDifficulty {
    match diff {
        GameDifficulty::Easy => CampaignDifficulty::Easy,
        GameDifficulty::Normal => CampaignDifficulty::Normal,
        GameDifficulty::Hard => CampaignDifficulty::Hard,
    }
}

fn challenge_to_logic_difficulty(diff: GameDifficulty) -> i32 {
    match diff {
        GameDifficulty::Easy => 0,
        GameDifficulty::Normal => 1,
        GameDifficulty::Hard => 2,
    }
}

fn set_window_text(window: &Option<Rc<RefCell<GameWindow>>>, text: &str) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().set_text(text);
    }
}

fn set_window_hidden(window: &Option<Rc<RefCell<GameWindow>>>, hidden: bool) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().hide(hidden);
    }
}

fn set_general_button_checked(control_id: i32, checked: bool) {
    with_window_manager(|manager| {
        if let Some(button) = manager.get_window_by_id(control_id) {
            let mut button = button.borrow_mut();
            match button.widget() {
                Some(crate::gui::WindowWidget::CheckBox(_)) => {
                    let _ = button.gadget_check_box_set_checked(checked);
                }
                Some(crate::gui::WindowWidget::RadioButton(_))
                    if checked => {
                        if let Some(crate::gui::WindowWidget::RadioButton(radio)) =
                            button.widget_mut()
                        {
                            radio.select();
                        }
                    }
                _ => {}
            }
        }
    });
}

fn set_window_image(window: &Option<Rc<RefCell<GameWindow>>>, image_name: Option<&str>) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let Some(image_name) = image_name else {
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

    let mut guard = window.borrow_mut();
    if guard.set_enabled_image(0, image).is_ok() {
        guard.set_status(WindowStatus::IMAGE);
    }
}

fn mapped_window_image(image_name: &str) -> Option<WindowImage> {
    if image_name.is_empty() {
        return None;
    }

    let collection = get_mapped_image_collection();
    let collection = collection.try_read()?;
    let found = collection.find_image_by_name(image_name)?;
    let size = found.get_image_size();
    Some(WindowImage {
        name: image_name.to_string(),
        width: size.x,
        height: size.y,
    })
}

fn set_draw_image(
    draw_data: &mut [crate::gui::WindowDrawData],
    index: usize,
    image: Option<WindowImage>,
) {
    if let Some(slot) = draw_data.get_mut(index) {
        slot.image = image;
    }
}

fn apply_general_button_medallions(button: &mut GameWindow, template: &PlayerTemplate) {
    let normal_image = mapped_window_image(&template.medallion_regular);
    let selected_image = mapped_window_image(&template.medallion_select);
    let hilite_image = mapped_window_image(&template.medallion_hilite);

    if let Some(image) = normal_image.clone() {
        let _ = button.set_size(image.width, image.width);
    }

    let inst = button.instance_data_mut();
    set_draw_image(&mut inst.enabled_draw_data, 0, normal_image);
    set_draw_image(&mut inst.hilite_draw_data, 1, selected_image.clone());
    set_draw_image(&mut inst.disabled_draw_data, 1, selected_image);
    set_draw_image(&mut inst.hilite_draw_data, 0, hilite_image);
    button.set_status(WindowStatus::IMAGE);
}

fn sync_bio_text(state: &ChallengeMenuState) {
    set_window_text(&state.bio_name_entry, &state.bio_readout[0]);
    set_window_text(&state.bio_dob_entry, &state.bio_readout[1]);
    set_window_text(&state.bio_birthplace_entry, &state.bio_readout[2]);
    set_window_text(&state.bio_strategy_entry, &state.bio_readout[3]);
}

fn find_general_button(state: &ChallengeMenuState, control_id: i32) -> Option<usize> {
    state
        .general_button_ids
        .iter()
        .position(|button_id| *button_id == control_id)
}

fn is_general_mouse_entering(msg: WindowMessage) -> bool {
    matches!(
        msg,
        WindowMessage::GadgetMouseEntering | WindowMessage::User(GBM_MOUSE_ENTERING)
    )
}

fn is_general_mouse_leaving(msg: WindowMessage) -> bool {
    matches!(
        msg,
        WindowMessage::GadgetMouseLeaving | WindowMessage::User(GBM_MOUSE_LEAVING)
    )
}

fn set_general_bio(state: &mut ChallengeMenuState, general_index: Option<usize>) {
    let Some(general_index) = general_index else {
        return;
    };

    let Some(generals) = get_challenge_generals_mut() else {
        return;
    };
    if general_index >= NUM_GENERALS {
        return;
    }

    let general = &generals.challenge_generals()[general_index];
    set_window_hidden(&state.bio_parent, false);
    set_window_image(&state.bio_portrait, general.bio_portrait_small());

    state.bio_lines[0] = GameText::fetch(general.bio_name());
    state.bio_lines[1] = GameText::fetch(general.bio_rank());
    state.bio_lines[2] = GameText::fetch(general.bio_branch());
    state.bio_lines[3] = GameText::fetch(general.bio_strategy());
    state.bio_readout = Default::default();
    state.bio_text_position = 0;
    state.bio_total_length = state.bio_lines.iter().map(String::len).sum();
    sync_bio_text(state);
}

fn update_bio(state: &mut ChallengeMenuState, frames: usize) {
    for _ in 0..frames {
        if state.bio_text_position >= state.bio_total_length {
            break;
        }

        let line0_len = state.bio_lines[0].len();
        let line1_len = state.bio_lines[1].len();
        let line2_len = state.bio_lines[2].len();

        if state.bio_text_position < line0_len {
            if let Some(ch) = state.bio_lines[0].chars().nth(state.bio_text_position) {
                state.bio_readout[0].push(ch);
            }
        } else if state.bio_text_position < line0_len + line1_len {
            let pos = state.bio_text_position - line0_len;
            if let Some(ch) = state.bio_lines[1].chars().nth(pos) {
                state.bio_readout[1].push(ch);
            }
        } else if state.bio_text_position < line0_len + line1_len + line2_len {
            let pos = state.bio_text_position - line0_len - line1_len;
            if let Some(ch) = state.bio_lines[2].chars().nth(pos) {
                state.bio_readout[2].push(ch);
            }
        } else {
            let pos = state.bio_text_position - line0_len - line1_len - line2_len;
            if let Some(ch) = state.bio_lines[3].chars().nth(pos) {
                state.bio_readout[3].push(ch);
            }
        }

        state.bio_text_position += 1;
    }

    sync_bio_text(state);
}

fn set_general_campaign(button_index: usize) -> Option<String> {
    if button_index >= NUM_GENERALS {
        return None;
    }

    let (campaign_name, player_template_name) = {
        let generals = get_challenge_generals_mut()?;
        let general = &generals.challenge_generals()[button_index];
        (
            general.campaign().to_string(),
            general.player_template_name().to_string(),
        )
    };

    ensure_player_templates_loaded();
    let (template_num, player_display_name) = {
        let store = get_player_template_store();
        let template_num = store.find_template_index(&player_template_name)? as i32;
        let player_display_name = store
            .get_nth_player_template(template_num as usize)
            .map(|template| template.get_display_name().to_string())
            .unwrap_or_default();
        (template_num, player_display_name)
    };

    if let Some(mut generals) = get_challenge_generals_mut() {
        generals.set_current_player_template_num(template_num);
    }

    let current_map = {
        let mut campaign_manager = get_campaign_manager();
        campaign_manager.set_campaign(&campaign_name);
        campaign_manager.get_current_map().unwrap_or_default()
    };

    if !current_map.is_empty() {
        let mut setup = get_skirmish_setup();
        let info = setup.game_info_mut().game_info_mut();
        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, player_display_name, 0);
        slot.set_player_template(template_num);
        info.set_slot(0, slot);
        info.set_map(current_map.clone());
    }

    Some(current_map)
}

fn start_challenge_game() {
    let selected_index = {
        let state_handle = challenge_menu_state();
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        match state.last_button_index {
            Some(index) => index,
            None => return,
        }
    };

    let difficulty = {
        let Some(generals) = get_challenge_generals_mut() else {
            return;
        };
        generals.current_difficulty()
    };

    let Some(current_map) = set_general_campaign(selected_index) else {
        return;
    };
    let rank_points = {
        let mut campaign_manager = get_campaign_manager();
        campaign_manager.set_game_difficulty(challenge_to_campaign_difficulty(difficulty));
        campaign_manager.get_rank_points()
    };

    if current_map.is_empty() {
        return;
    }

    if let Some(data) = get_global_data() {
        data.write().pending_file = current_map;
    }
    TheScriptEngine::set_global_difficulty(challenge_to_logic_difficulty(difficulty));

    {
        let state_handle = challenge_menu_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous_index) = state.last_button_index {
            if let Some(button_id) = state.general_button_ids.get(previous_index) {
                set_general_button_checked(*button_id, false);
            }
        }
        state.last_button_index = None;
        state.last_hilited_index = None;
        state.last_selection_sound = 0;
        state.last_preview_sound = 0;
    }

    if TheGameLogic::is_in_game() {
        let _ = TheGameLogic::clear_game_data();
    }

    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
    let msg = stream.append_message(GameMessageType::NewGame);
    msg.append_integer_argument(GAME_SINGLE_PLAYER);
    msg.append_integer_argument(challenge_to_logic_difficulty(difficulty));
    msg.append_integer_argument(rank_points);
    msg.append_integer_argument(LOGICFRAMES_PER_SECOND as i32);
    init_random_with_seed(0);
}

pub fn challenge_menu_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    {
        let mut setup = get_skirmish_setup();
        let info = setup.game_info_mut().game_info_mut();
        info.init();
        info.clear_slot_list();
        info.reset();
        info.enter_game();
    }

    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("ChallengeMenu.wnd:ParentChallengeMenu");
    state.button_play_id = name_to_id("ChallengeMenu.wnd:ButtonPlay");
    state.button_back_id = name_to_id("ChallengeMenu.wnd:ButtonBack");
    state.gadget_parent_id = name_to_id("ChallengeMenu.wnd:GadgetParent");
    state.bio_parent_id = name_to_id("ChallengeMenu.wnd:GeneralsBioParent");
    state.bio_portrait_id = name_to_id("ChallengeMenu.wnd:BioPortrait");
    state.bio_name_entry_id = name_to_id("ChallengeMenu.wnd:BioNameEntry");
    state.bio_dob_entry_id = name_to_id("ChallengeMenu.wnd:BioDOBEntry");
    state.bio_birthplace_entry_id = name_to_id("ChallengeMenu.wnd:BioBirthplaceEntry");
    state.bio_strategy_entry_id = name_to_id("ChallengeMenu.wnd:BioStrategyEntry");
    for i in 0..NUM_GENERALS {
        state.general_button_ids[i] = name_to_id(&format!("ChallengeMenu.wnd:GeneralPosition{i}"));
    }

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_play = manager.get_window_by_id(state.button_play_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.gadget_parent = manager.get_window_by_id(state.gadget_parent_id);
        state.bio_parent = manager.get_window_by_id(state.bio_parent_id);
        state.bio_portrait = manager.get_window_by_id(state.bio_portrait_id);
        state.bio_name_entry = manager.get_window_by_id(state.bio_name_entry_id);
        state.bio_dob_entry = manager.get_window_by_id(state.bio_dob_entry_id);
        state.bio_birthplace_entry = manager.get_window_by_id(state.bio_birthplace_entry_id);
        state.bio_strategy_entry = manager.get_window_by_id(state.bio_strategy_entry_id);
        if let Some(parent) = state.parent.as_ref() {
            let _ = manager.set_focus(Some(parent));
        }
    });

    set_window_hidden(&state.bio_parent, true);
    set_window_hidden(&state.button_play, true);
    set_window_hidden(&state.gadget_parent, true);
    state.just_entered = true;
    state.initial_gadget_delay = 2;
    state.is_shutting_down = false;
    state.intro_audio_magic_number = 0;
    state.has_played_intro_audio = false;
    state.last_button_index = None;
    state.last_hilited_index = None;
    state.last_selection_sound = 0;
    state.last_preview_sound = 0;
    state.bio_lines = Default::default();
    state.bio_readout = Default::default();
    state.bio_text_position = 0;
    state.bio_total_length = 0;
    if let Some(generals) = get_challenge_generals_mut() {
        ensure_player_templates_loaded();
        let templates = get_player_template_store();
        with_window_manager(|manager| {
            for (index, button_id) in state.general_button_ids.iter().enumerate() {
                if let Some(button) = manager.get_window_by_id(*button_id) {
                    let general = &generals.challenge_generals()[index];
                    let enabled = general.is_starting_enabled();
                    let mut button = button.borrow_mut();
                    let _ = button.enable(enabled);
                    let _ = button.hide(!enabled);
                    if let Some(template) = templates.find_template(general.player_template_name())
                    {
                        apply_general_button_medallions(&mut button, template);
                    }
                }
            }
        });
    }

    get_shell().show_shell_map(true);
    layout.hide(false);
    with_window_video_manager(|manager| manager.init());
}

pub fn challenge_menu_update(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| manager.transition_set_group("ChallengeMenuFade", false));
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    update_bio(&mut state, 2);

    if !state.has_played_intro_audio
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.intro_audio_magic_number += 1;
        if state.intro_audio_magic_number == 10 {
            if let Some(audio) = TheAudio::get() {
                let event = AudioEventRts::new("Taunts_GCAnnouncer01");
                let _ = audio.add_audio_event(&event);
            }
            state.has_played_intro_audio = true;
        }
    }

    if state.is_shutting_down
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.is_shutting_down = false;
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
    }

    with_window_video_manager(|manager| manager.update());
}

pub fn challenge_menu_shutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    with_window_video_manager(|manager| manager.reset());

    let state_handle = challenge_menu_state();
    {
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        state.last_button_index = None;
    }

    if pop_immediate {
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    with_window_manager(|manager| manager.transition_reverse("ChallengeMenuFade"));
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = true;
    if let Some(audio) = TheAudio::get() {
        audio.remove_audio_event(state.last_selection_sound);
        audio.remove_audio_event(state.last_preview_sound);
    }
    state.last_selection_sound = 0;
    state.last_preview_sound = 0;
    state.intro_audio_magic_number = 0;
    state.has_played_intro_audio = false;
}

pub fn challenge_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        msg if is_general_mouse_entering(msg) => {
            let control_id = data1 as i32;
            if let Some(index) = find_general_button(&state, control_id) {
                if state.last_button_index != Some(index) {
                    set_general_bio(&mut state, Some(index));
                    if let Some(audio) = TheAudio::get() {
                        let event = AudioEventRts::new("GUILogoMouseOver");
                        let _ = audio.add_audio_event(&event);
                    }
                    state.last_hilited_index = Some(index);
                }
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        msg if is_general_mouse_leaving(msg) => {
            let control_id = data1 as i32;
            if let Some(index) = find_general_button(&state, control_id) {
                if state.last_button_index != Some(index) {
                    let selected_general = state.last_button_index;
                    set_general_bio(&mut state, selected_general);
                }
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if let Some(index) = find_general_button(&state, control_id) {
                if let Some(previous_index) = state.last_button_index.filter(|prev| *prev != index)
                {
                    if let Some(button_id) = state.general_button_ids.get(previous_index) {
                        set_general_button_checked(*button_id, false);
                    }
                }
                if let Some(button_id) = state.general_button_ids.get(index) {
                    set_general_button_checked(*button_id, true);
                }
                if let Some(audio) = TheAudio::get() {
                    audio.remove_audio_event(state.last_selection_sound);
                    audio.remove_audio_event(state.last_preview_sound);
                    let preview_sound = get_challenge_generals_mut().map(|generals| {
                        generals.challenge_generals()[index]
                            .preview_sound()
                            .to_string()
                    });
                    if let Some(preview_sound) = preview_sound.filter(|sound| !sound.is_empty()) {
                        let event = AudioEventRts::new(&preview_sound);
                        state.last_preview_sound = audio.add_audio_event(&event);
                    }
                }
                state.last_button_index = Some(index);
                set_general_bio(&mut state, Some(index));
                set_window_hidden(&state.button_play, false);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_play_id {
                if state.is_shutting_down {
                    return WindowMsgHandled::Handled;
                }
                drop(state);
                start_challenge_game();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_back_id {
                drop(state);
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn challenge_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        let state_handle = challenge_menu_state();
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = state.parent.as_ref() {
            let _ = parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                state.button_back_id as WindowMsgData,
                state.button_back_id as WindowMsgData,
            );
        }
    }

    WindowMsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::image::{ICoord2D, Image as MappedImage};
    use crate::gui::challenge_generals::init_challenge_generals;
    use crate::gui::gadgets::CheckBox;
    use crate::gui::WindowWidget;
    use game_engine::common::language::Language;

    fn add_test_mapped_image(name: &str, width: i32, height: i32) {
        let collection = get_mapped_image_collection();
        let mut collection = collection.write();
        let mut image = MappedImage::with_name(name);
        image.set_image_size(ICoord2D::new(width, height));
        collection.add_image(image);
    }

    #[test]
    fn esc_char_is_consumed_before_key_up_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            challenge_menu_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            challenge_menu_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn general_button_medallions_match_cpp_draw_slots_and_size() {
        add_test_mapped_image("TestMedallionNormal", 64, 48);
        add_test_mapped_image("TestMedallionSelected", 66, 50);
        add_test_mapped_image("TestMedallionHilite", 68, 52);

        let mut template = PlayerTemplate::new("TestGeneral".to_string());
        template.medallion_regular = "TestMedallionNormal".to_string();
        template.medallion_select = "TestMedallionSelected".to_string();
        template.medallion_hilite = "TestMedallionHilite".to_string();

        let mut button = GameWindow::new();
        apply_general_button_medallions(&mut button, &template);

        assert_eq!(button.get_size(), (64, 64));
        assert!(button.get_status().contains(WindowStatus::IMAGE));
        assert_eq!(
            button.instance_data().enabled_draw_data[0]
                .image
                .as_ref()
                .map(|image| image.name.as_str()),
            Some("TestMedallionNormal")
        );
        assert_eq!(
            button.instance_data().hilite_draw_data[1]
                .image
                .as_ref()
                .map(|image| image.name.as_str()),
            Some("TestMedallionSelected")
        );
        assert_eq!(
            button.instance_data().disabled_draw_data[1]
                .image
                .as_ref()
                .map(|image| image.name.as_str()),
            Some("TestMedallionSelected")
        );
        assert_eq!(
            button.instance_data().hilite_draw_data[0]
                .image
                .as_ref()
                .map(|image| image.name.as_str()),
            Some("TestMedallionHilite")
        );
    }

    #[test]
    fn general_bio_uses_localized_text_like_cpp() {
        init_challenge_generals();
        Language::clear_localized_strings();
        Language::register_localized_string("TEST:ChallengeBioName", "General Localized");
        Language::register_localized_string("TEST:ChallengeBioRank", "Field Marshal");
        Language::register_localized_string("TEST:ChallengeBioBranch", "Armor Command");
        Language::register_localized_string("TEST:ChallengeBioStrategy", "Breakthrough tactics");

        {
            let mut generals = get_challenge_generals_mut().expect("challenge generals");
            let general = &mut generals.challenge_generals_mut()[0];
            general.set_bio_name("TEST:ChallengeBioName".to_string());
            general.set_bio_rank("TEST:ChallengeBioRank".to_string());
            general.set_bio_branch("TEST:ChallengeBioBranch".to_string());
            general.set_bio_strategy("TEST:ChallengeBioStrategy".to_string());
            general.set_preview_sound(String::new());
        }

        let mut state = ChallengeMenuState::default();
        set_general_bio(&mut state, Some(0));

        assert_eq!(state.bio_lines[0], "General Localized");
        assert_eq!(state.bio_lines[1], "Field Marshal");
        assert_eq!(state.bio_lines[2], "Armor Command");
        assert_eq!(state.bio_lines[3], "Breakthrough tactics");
        assert_eq!(
            state.bio_total_length,
            "General LocalizedField MarshalArmor CommandBreakthrough tactics".len()
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn dedicated_gadget_hover_messages_match_cpp_hover_handlers() {
        assert!(is_general_mouse_entering(
            WindowMessage::GadgetMouseEntering
        ));
        assert!(is_general_mouse_entering(WindowMessage::User(
            GBM_MOUSE_ENTERING
        )));
        assert!(!is_general_mouse_entering(
            WindowMessage::GadgetMouseLeaving
        ));

        assert!(is_general_mouse_leaving(WindowMessage::GadgetMouseLeaving));
        assert!(is_general_mouse_leaving(WindowMessage::User(
            GBM_MOUSE_LEAVING
        )));
        assert!(!is_general_mouse_leaving(
            WindowMessage::GadgetMouseEntering
        ));
    }

    #[test]
    fn selecting_current_general_keeps_checkbox_checked_like_cpp() {
        let selected_id = name_to_id("ChallengeMenu.test:SelectedGeneral");
        {
            let state_handle = challenge_menu_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            state.general_button_ids = [0; NUM_GENERALS];
            state.general_button_ids[0] = selected_id;
            state.last_button_index = Some(0);
        }

        with_window_manager(|manager| {
            let _ = manager.destroy_all_windows();
            let button = manager
                .create_window_with_id(None, 0, 0, 32, 32, selected_id)
                .expect("create challenge general button");
            let mut button = button.borrow_mut();
            button.set_widget(WindowWidget::CheckBox(CheckBox::new(
                selected_id as u32,
                0,
                0,
                32,
            )));
            let _ = button.gadget_check_box_set_checked(false);
        });

        let window = GameWindow::new();
        assert_eq!(
            challenge_menu_system(
                &window,
                WindowMessage::GadgetSelected,
                selected_id as WindowMsgData,
                selected_id as WindowMsgData,
            ),
            WindowMsgHandled::Handled
        );

        with_window_manager(|manager| {
            let button = manager
                .get_window_by_id(selected_id)
                .expect("selected general button");
            let button = button.borrow();
            match button.widget() {
                Some(WindowWidget::CheckBox(check)) => assert!(check.is_checked()),
                _ => panic!("expected selected general checkbox"),
            }
        });
    }

    #[test]
    fn shutdown_clears_selected_general_before_pop_branch_like_cpp() {
        let layout = WindowLayout::new("ChallengeMenu.wnd".to_string());

        for pop_immediate in [false, true] {
            {
                let state_handle = challenge_menu_state();
                let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                state.last_button_index = Some(3);
                state.is_shutting_down = false;
            }

            challenge_menu_shutdown(&layout, Some(&pop_immediate));

            let state_handle = challenge_menu_state();
            let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(state.last_button_index, None);
        }
    }
}
