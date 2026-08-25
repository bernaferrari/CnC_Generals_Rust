//! ChallengeMenu.cpp callback port.

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::campaign_launch_host_bridge::{
    HostCampaignLaunchDescriptor, publish_host_campaign_launch,
};
use crate::gui::campaign_manager::{GameDifficulty as CampaignDifficulty, get_campaign_manager};
use crate::gui::challenge_game_info::{
    clear_challenge_game_info, init_challenge_game_info, set_challenge_slot0_and_map,
};
use crate::gui::challenge_generals::{GameDifficulty, NUM_GENERALS, get_challenge_generals_mut};
use crate::gui::game_window::Image as WindowImage;
use crate::gui::window_video_manager::with_window_video_manager;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
    get_shell, queue_shell_pop, queue_shell_shutdown_complete, show_shell_map_if_available,
    try_with_shell_mut, with_window_manager, write_input_focus_response,
};
use crate::message_stream::{GameMessageType, get_message_stream};
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use game_engine::common::ini::{ensure_player_templates_loaded, get_global_data};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::rts::player_template::{PlayerTemplate, get_player_template_store};
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
    /// C++ `isAutoSelecting` — ignore GBM_SELECTED echo from GadgetCheckBoxToggle.
    is_auto_selecting: bool,
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
                Some(crate::gui::WindowWidget::RadioButton(_)) if checked => {
                    if let Some(crate::gui::WindowWidget::RadioButton(radio)) = button.widget_mut()
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

#[derive(Debug, Clone)]
struct ChallengeSelection {
    map_name: String,
    campaign_name: String,
    player_template_name: String,
    player_template_index: i32,
}

fn set_general_campaign(button_index: usize) -> Option<ChallengeSelection> {
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
        set_challenge_slot0_and_map(current_map.clone(), player_display_name, template_num);
    }

    Some(ChallengeSelection {
        map_name: current_map,
        campaign_name,
        player_template_name,
        player_template_index: template_num,
    })
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

    let Some(selection) = set_general_campaign(selected_index) else {
        return;
    };
    let rank_points = {
        let mut campaign_manager = get_campaign_manager();
        campaign_manager.set_game_difficulty(challenge_to_campaign_difficulty(difficulty));
        campaign_manager.get_rank_points()
    };

    if selection.map_name.is_empty() {
        return;
    }

    if let Some(data) = get_global_data() {
        data.write().pending_file = selection.map_name.clone();
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

    // Keep both GlobalData residences coherent as hq-gyp/hq-c3w do, then
    // publish the C++ TheChallengeGameInfo selection atomically with the
    // NewGame payload for Main authority.
    game_engine::common::global_data::write().pending_file = selection.map_name.clone();
    let logic_difficulty = challenge_to_logic_difficulty(difficulty);
    let _ = publish_host_campaign_launch(HostCampaignLaunchDescriptor {
        generation: 0,
        map_name: selection.map_name,
        campaign_name: selection.campaign_name,
        campaign_player_faction: String::new(),
        is_challenge: true,
        player_template_name: Some(selection.player_template_name),
        player_template_index: Some(selection.player_template_index),
        game_mode_code: GAME_SINGLE_PLAYER,
        difficulty_code: logic_difficulty,
        rank_points,
        max_fps: Some(LOGICFRAMES_PER_SECOND as i32),
    });

    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
    let msg = stream.append_message(GameMessageType::NewGame);
    msg.append_integer_argument(GAME_SINGLE_PLAYER);
    msg.append_integer_argument(logic_difficulty);
    msg.append_integer_argument(rank_points);
    msg.append_integer_argument(LOGICFRAMES_PER_SECOND as i32);
    init_random_with_seed(0);
}

pub fn challenge_menu_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    init_challenge_game_info();

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

    show_shell_map_if_available(true);
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
        && try_with_shell_mut(|shell| shell.is_anim_finished()).unwrap_or(false)
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.is_shutting_down = false;
        layout.hide(true);
        queue_shell_shutdown_complete(false);
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
        queue_shell_shutdown_complete(false);
        return;
    }

    with_window_manager(|manager| manager.transition_reverse("ChallengeMenuFade"));
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = true;
    // C++ ChallengeMenuShutdown: delete TheChallengeGameInfo (fade path only).
    clear_challenge_game_info();
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
            // C++ ChallengeMenuSystem: if (isAutoSelecting) break;
            if state.is_auto_selecting {
                return WindowMsgHandled::Handled;
            }
            if let Some(index) = find_general_button(&state, control_id) {
                let previous_id = state
                    .last_button_index
                    .filter(|prev| *prev != index)
                    .and_then(|prev| state.general_button_ids.get(prev).copied());
                let current_id = state.general_button_ids.get(index).copied();
                state.is_auto_selecting = true;
                drop(state);
                if let Some(button_id) = previous_id {
                    set_general_button_checked(button_id, false);
                }
                if let Some(button_id) = current_id {
                    set_general_button_checked(button_id, true);
                }
                let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                state.is_auto_selecting = false;
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
                queue_shell_pop();
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

/// Residual: last general index selected via residual peels (`usize::MAX` = none).
static RESIDUAL_CHALLENGE_SELECTED_INDEX: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(usize::MAX);
/// Residual: ButtonPlay was fired with a selected general (host honesty).
static RESIDUAL_CHALLENGE_PLAY_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn ensure_challenge_control_ids(state: &mut ChallengeMenuState) {
    if state.button_play_id == 0 {
        state.button_play_id = name_to_id("ChallengeMenu.wnd:ButtonPlay");
        if state.button_play_id == 0 {
            state.button_play_id = 0x434D_504C_u32 as i32; // 'CMPL'
        }
    }
    if state.button_back_id == 0 {
        state.button_back_id = name_to_id("ChallengeMenu.wnd:ButtonBack");
    }
    if state.parent_id == 0 {
        state.parent_id = name_to_id("ChallengeMenu.wnd:ParentChallengeMenu");
    }
    for i in 0..NUM_GENERALS {
        if state.general_button_ids[i] == 0 {
            state.general_button_ids[i] =
                name_to_id(&format!("ChallengeMenu.wnd:GeneralPosition{i}"));
        }
    }
}

/// Residual: select a challenge general (GeneralPositionN residual).
/// State-only — skips audio preview / bio image residual.
pub fn simulate_challenge_menu_select_general(index: usize) -> bool {
    if index >= NUM_GENERALS {
        return false;
    }
    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_challenge_control_ids(&mut state);
    state.last_button_index = Some(index);
    state.is_shutting_down = false;
    RESIDUAL_CHALLENGE_SELECTED_INDEX.store(index, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_CHALLENGE_PLAY_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    state.last_button_index == Some(index)
}

/// Residual: currently selected challenge general index.
pub fn residual_challenge_selected_general() -> Option<usize> {
    let idx = RESIDUAL_CHALLENGE_SELECTED_INDEX.load(std::sync::atomic::Ordering::Relaxed);
    if idx >= NUM_GENERALS { None } else { Some(idx) }
}

/// Residual: ButtonPlay was requested after a general selection.
pub fn residual_challenge_play_requested() -> bool {
    RESIDUAL_CHALLENGE_PLAY_REQUESTED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: fire retail `ChallengeMenu.wnd:ButtonPlay` via state latch.
/// C++ path calls startChallengeGame (campaign map + difficulty + MSG_NEW_GAME).
pub fn simulate_challenge_menu_play_button_gadget_selected() -> bool {
    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_challenge_control_ids(&mut state);
    if state.is_shutting_down {
        return false;
    }
    let Some(index) = state.last_button_index else {
        return false;
    };
    if index >= NUM_GENERALS {
        return false;
    }
    RESIDUAL_CHALLENGE_SELECTED_INDEX.store(index, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_CHALLENGE_PLAY_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed);
    drop(state);
    // C++ ChallengeMenu ButtonPlay → startChallengeGame (pendingFile + MSG_NEW_GAME).
    start_challenge_game();
    true
}

/// Residual: fire retail `ChallengeMenu.wnd:ButtonBack` (shell pop residual latch).
pub fn simulate_challenge_menu_back_button_gadget_selected() -> bool {
    let state_handle = challenge_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_challenge_control_ids(&mut state);
    // C++ back pops shell; residual clears selection and marks shutdown-ish clean exit.
    state.last_button_index = None;
    state.is_shutting_down = false;
    RESIDUAL_CHALLENGE_PLAY_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_CHALLENGE_SELECTED_INDEX.store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
    state.button_back_id != 0 || state.button_play_id != 0
}

/// Residual: select general + ButtonPlay composite (pre-start honesty).
pub fn simulate_challenge_menu_prepare_start(index: usize) -> bool {
    if !simulate_challenge_menu_select_general(index) {
        return false;
    }
    simulate_challenge_menu_play_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `GeneralPositionN` then `ButtonPlay`
/// (C++ WindowXlat hit → GBM_SELECTED → startChallengeGame). Not `simulate_*`.
pub fn drive_os_wnd_challenge_start_like_cpp(index: usize) -> bool {
    if index >= NUM_GENERALS {
        return false;
    }
    let general_name = format!("ChallengeMenu.wnd:GeneralPosition{index}");
    let clicked_general = crate::gui::dispatch_os_click_named_window(&general_name);
    let clicked_play = crate::gui::dispatch_os_click_named_window("ChallengeMenu.wnd:ButtonPlay");
    if !clicked_general && !clicked_play {
        return false;
    }
    if clicked_general && !simulate_challenge_menu_select_general(index) {
        return false;
    }
    if clicked_play || clicked_general {
        return simulate_challenge_menu_play_button_gadget_selected();
    }
    residual_challenge_play_requested()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::image::{ICoord2D, Image as MappedImage};
    use crate::gui::WindowWidget;
    use crate::gui::challenge_generals::init_challenge_generals;
    use crate::gui::gadgets::CheckBox;
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
    fn challenge_menu_init_uses_challenge_game_info_not_skirmish() {
        use crate::gui::challenge_game_info::{
            challenge_game_info_exists, clear_challenge_game_info, with_challenge_game_info,
        };
        use crate::gui::get_skirmish_setup;

        clear_challenge_game_info();
        {
            let mut setup = get_skirmish_setup();
            let info = setup.game_info_mut().game_info_mut();
            info.init();
            info.set_map("maps\\skirmish_only.map".to_string());
        }

        let layout = WindowLayout::new("Menus/ChallengeMenu.wnd".to_string());
        challenge_menu_init(&layout, None);

        assert!(
            challenge_game_info_exists(),
            "C++ ChallengeMenuInit allocates TheChallengeGameInfo"
        );
        let challenge_map = with_challenge_game_info(|info| info.game_info().get_map().to_string())
            .unwrap_or_default();
        assert_ne!(
            challenge_map, "maps\\skirmish_only.map",
            "challenge GameInfo must not share skirmish map"
        );
        let skirmish_map = get_skirmish_setup()
            .game_info()
            .game_info()
            .get_map()
            .to_string();
        assert_eq!(
            skirmish_map, "maps\\skirmish_only.map",
            "TheSkirmishGameInfo must stay untouched by ChallengeMenuInit"
        );

        set_challenge_slot0_and_map("maps\\GC_ChemGeneral.map".into(), "Test".into(), 3);
        let after = with_challenge_game_info(|info| {
            (
                info.game_info().get_map().to_string(),
                info.game_info()
                    .get_slot(0)
                    .map(|s| s.get_player_template())
                    .unwrap_or(-1),
            )
        })
        .unwrap();
        assert_eq!(after.0, "maps\\GC_ChemGeneral.map");
        assert_eq!(after.1, 3);
        let skirmish_map = get_skirmish_setup()
            .game_info()
            .game_info()
            .get_map()
            .to_string();
        assert_eq!(skirmish_map, "maps\\skirmish_only.map");

        let layout = WindowLayout::new("Menus/ChallengeMenu.wnd".to_string());
        challenge_menu_shutdown(&layout, Some(&false));
        assert!(
            !challenge_game_info_exists(),
            "C++ ChallengeMenuShutdown deletes TheChallengeGameInfo on fade path"
        );
    }

    #[test]
    fn simulate_challenge_play_calls_start_challenge_game_like_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/gui/callbacks/challenge_menu.rs"
        ));
        let play = src
            .split("pub fn simulate_challenge_menu_play_button_gadget_selected")
            .nth(1)
            .expect("play helper");
        let play = play
            .split("pub fn simulate_challenge_menu_back_button_gadget_selected")
            .next()
            .expect("play body");
        assert!(
            play.contains("start_challenge_game()"),
            "C++ ButtonPlay calls startChallengeGame (pendingFile + MSG_NEW_GAME)"
        );
    }

    #[test]
    fn os_wnd_challenge_start_hits_named_gadgets_then_start_challenge_game() {
        with_window_manager(|manager| {
            let general = manager
                .create_window(None, 10, 10, 40, 40)
                .expect("general gadget");
            general
                .borrow_mut()
                .set_name("ChallengeMenu.wnd:GeneralPosition0");
            let _ = general.borrow_mut().hide(false);
            let play = manager
                .create_window(None, 60, 10, 40, 40)
                .expect("play gadget");
            play.borrow_mut().set_name("ChallengeMenu.wnd:ButtonPlay");
            let _ = play.borrow_mut().hide(false);
        });
        assert!(
            drive_os_wnd_challenge_start_like_cpp(0),
            "OS WND clicks on ChallengeMenu gadgets must startChallengeGame"
        );
        assert!(residual_challenge_play_requested());
        assert_eq!(residual_challenge_selected_general(), Some(0));
        assert!(!drive_os_wnd_challenge_start_like_cpp(99));
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
