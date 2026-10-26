//! ChallengeMenu.cpp callback port.

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::campaign_launch_host_bridge::{
    HostCampaignLaunchDescriptor, MenuHostRequest, MenuLaunchSource,
};
use crate::gui::campaign_manager::{CampaignManager, GameDifficulty as CampaignDifficulty};
use crate::gui::challenge_generals::{ChallengeGenerals, GameDifficulty, NUM_GENERALS};
use crate::gui::game_window::Image as WindowImage;

use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
    queue_shell_pop, queue_shell_shutdown_complete, show_shell_map_if_available,
    write_input_focus_response,
};
use crate::message_stream::{GameMessage, GameMessageType};
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;
use game_engine::common::ini::{ensure_player_templates_loaded, get_global_data};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::rts::player_template::{PlayerTemplate, get_player_template_store};
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use std::cell::RefCell;
use std::rc::Rc;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const GGM_LEFT_DRAG: u32 = 16384;
const GBM_MOUSE_ENTERING: u32 = GGM_LEFT_DRAG + 6;
const GBM_MOUSE_LEAVING: u32 = GGM_LEFT_DRAG + 7;

#[derive(Default)]
pub(crate) struct ChallengeMenuState {
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
    last_selection_sound: Option<crate::gui::menu_audio::MenuAudioHandle>,
    last_preview_sound: Option<crate::gui::menu_audio::MenuAudioHandle>,
    bio_lines: [String; 4],
    bio_readout: [String; 4],
    bio_text_position: usize,
    bio_total_length: usize,
    play_requested: bool,
    last_requested_general: Option<usize>,
}

fn play_menu_sound(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    name: &str,
) -> Option<crate::gui::menu_audio::MenuAudioHandle> {
    effects.play_menu_audio(name)
}
fn stop_menu_sound(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    handle: Option<crate::gui::menu_audio::MenuAudioHandle>,
) {
    if let Some(handle) = handle {
        let _ = effects.stop_menu_audio(handle);
    }
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

fn set_window_hidden(
    window: &Option<Rc<RefCell<GameWindow>>>,
    hidden: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Some(window) = window.as_ref() {
        effects.with_manager(|manager, _effects| {
            let _ = window.borrow_mut().hide(hidden, manager);
        });
    }
}

fn set_general_button_checked(
    control_id: i32,
    checked: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    mut selected: Option<&mut GameWindow>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(button) =
        receiving_manager.get_window_by_id_with_selected(control_id, selected.as_deref())
    else {
        return;
    };
    let (owner, notify) = {
        let mut button = button.borrow_mut();
        let notify = match button.widget_mut() {
            Some(crate::gui::WindowWidget::CheckBox(checkbox)) => {
                checkbox.set_checked(checked);
                true
            }
            Some(crate::gui::WindowWidget::RadioButton(radio)) if checked => {
                radio.select();
                false
            }
            _ => false,
        };
        button.sync_state_from_widget();
        (button.get_owner(), notify)
    };
    if notify {
        if let Some(owner) = owner {
            if let Some(selected) = selected.as_deref_mut() {
                // Compare the original Rc recipient without opening its RefCell:
                // the dispatcher already holds this exact mutable window loan.
                if std::ptr::eq(
                    owner.as_ref().as_ptr().cast_const(),
                    selected as *const GameWindow,
                ) {
                    selected.send_system_message(
                        WindowMessage::GadgetSelected,
                        control_id as WindowMsgData,
                        0,
                        effects,
                    );
                    return;
                }
            }
            owner.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                control_id as WindowMsgData,
                0,
                effects,
            );
        }
    }
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

fn apply_general_button_medallions(
    button: &mut GameWindow,
    template: &PlayerTemplate,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let normal_image = mapped_window_image(&template.medallion_regular);
    let selected_image = mapped_window_image(&template.medallion_select);
    let hilite_image = mapped_window_image(&template.medallion_hilite);

    if let Some(image) = normal_image.clone() {
        let _ = button.set_size(image.width, image.width, effects);
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

fn set_general_bio(
    general_index: Option<usize>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(general_index) = general_index.filter(|&index| index < NUM_GENERALS) else {
        return;
    };
    let general = effects.challenge_generals().challenge_generals()[general_index].clone();
    let (bio_parent, bio_portrait) = {
        let state = effects.challenge();
        (state.bio_parent.clone(), state.bio_portrait.clone())
    };
    set_window_hidden(&bio_parent, false, effects);
    set_window_image(&bio_portrait, general.bio_portrait_small());
    let state = effects.challenge_mut();
    state.bio_lines = [
        GameText::fetch(general.bio_name()),
        GameText::fetch(general.bio_rank()),
        GameText::fetch(general.bio_branch()),
        GameText::fetch(general.bio_strategy()),
    ];
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

fn set_general_campaign(
    button_index: usize,
    campaign_manager: &mut CampaignManager,
    general: &crate::gui::challenge_generals::GeneralPersona,
) -> Option<ChallengeSelection> {
    if button_index >= NUM_GENERALS {
        return None;
    }

    let (campaign_name, player_template_name) = {
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

    campaign_manager.set_xfer_challenge_generals_player_template_num(template_num);

    campaign_manager.set_campaign(&campaign_name);
    let current_map = campaign_manager.get_current_map().unwrap_or_default();

    if !current_map.is_empty() {
        campaign_manager.set_challenge_slot0_and_map(
            current_map.clone(),
            player_display_name,
            template_num,
        );
    }

    Some(ChallengeSelection {
        map_name: current_map,
        campaign_name,
        player_template_name,
        player_template_index: template_num,
    })
}

fn start_challenge_game(
    mut selected: Option<&mut GameWindow>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(selected_index) = effects.challenge().last_button_index else {
        return;
    };
    let general = effects.challenge_generals().challenge_generals()[selected_index].clone();
    let Some(campaign_manager) = effects.campaign_manager.as_deref_mut() else {
        return;
    };
    let difficulty = match campaign_manager.get_game_difficulty() {
        CampaignDifficulty::Easy => GameDifficulty::Easy,
        CampaignDifficulty::Hard => GameDifficulty::Hard,
        CampaignDifficulty::Normal => GameDifficulty::Normal,
    };
    let Some(selection) = set_general_campaign(selected_index, campaign_manager, &general) else {
        return;
    };

    campaign_manager.set_game_difficulty(challenge_to_campaign_difficulty(difficulty));
    let rank_points = campaign_manager.get_rank_points();

    if selection.map_name.is_empty() {
        return;
    }

    // The selected Challenge GameInfo belongs to this queued launch. Freeze
    // its unsigned money before clearing the previous match or resetting the
    // application can replace the shell's GameInfo.
    let game_info_starting_cash = campaign_manager
        .challenge_game_info()
        .map(|info| info.game_info().get_starting_cash().count_money());

    if effects.input_authority() == crate::core::game_client::ClientInputAuthority::StandaloneCore {
        if let Some(data) = get_global_data() {
            data.write().pending_file = selection.map_name.clone();
        }
    }
    effects.request_menu_host(MenuHostRequest::SetDifficulty(
        challenge_to_logic_difficulty(difficulty),
    ));

    let previous_id = effects
        .challenge()
        .last_button_index
        .and_then(|index| effects.challenge().general_button_ids.get(index).copied());
    if let Some(button_id) = previous_id {
        effects.challenge_mut().is_auto_selecting = true;
        effects.with_manager(|manager, effects| {
            set_general_button_checked(button_id, false, manager, selected.as_deref_mut(), effects)
        });
        effects.challenge_mut().is_auto_selecting = false;
    }
    {
        let state = effects.challenge_mut();
        state.last_button_index = None;
        state.last_hilited_index = None;
        state.last_selection_sound = None;
        state.last_preview_sound = None;
    }

    // Keep both GlobalData residences coherent as hq-gyp/hq-c3w do, then
    // publish the C++ TheChallengeGameInfo selection atomically with the
    // NewGame payload for Main authority.
    if effects.input_authority() == crate::core::game_client::ClientInputAuthority::StandaloneCore {
        game_engine::common::global_data::write().pending_file = selection.map_name.clone();
    }
    let logic_difficulty = challenge_to_logic_difficulty(difficulty);
    let descriptor = HostCampaignLaunchDescriptor {
        map_name: selection.map_name,
        campaign_name: selection.campaign_name,
        campaign_player_faction: String::new(),
        is_challenge: true,
        game_info_starting_cash,
        player_template_name: Some(selection.player_template_name),
        player_template_index: Some(selection.player_template_index),
        game_mode_code: GAME_SINGLE_PLAYER,
        difficulty_code: logic_difficulty,
        rank_points,
        max_fps: Some(LOGICFRAMES_PER_SECOND as i32),
    };
    let clear_previous = effects.menu_world().in_game;
    effects.request_menu_host(MenuHostRequest::StartCampaign {
        descriptor,
        clear_previous,
        source: MenuLaunchSource::ChallengeMenu,
        main_menu_layout: None,
    });
}

pub fn challenge_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Some(manager) = effects.campaign_manager.as_deref_mut() {
        manager.init_challenge_game_info();
    }
    {
        let state = effects.challenge_mut();
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
            state.general_button_ids[i] =
                name_to_id(&format!("ChallengeMenu.wnd:GeneralPosition{i}"));
        }
    }
    effects.with_manager(|manager, effects| {
        let state = effects.challenge_mut();
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
    });
    let parent = effects.challenge().parent.clone();
    if let Some(parent) = parent.as_ref() {
        effects.with_manager(|manager, effects| {
            let _ = manager.set_focus(Some(parent), effects);
        });
    }
    for window in [
        effects.challenge().bio_parent.clone(),
        effects.challenge().button_play.clone(),
        effects.challenge().gadget_parent.clone(),
    ] {
        set_window_hidden(&window, true, effects);
    }
    {
        let state = effects.challenge_mut();
        state.just_entered = true;
        state.initial_gadget_delay = 2;
        state.is_shutting_down = false;
        state.intro_audio_magic_number = 0;
        state.has_played_intro_audio = false;
        state.last_button_index = None;
        state.last_hilited_index = None;
        state.last_selection_sound = None;
        state.last_preview_sound = None;
        state.bio_lines = Default::default();
        state.bio_readout = Default::default();
        state.bio_text_position = 0;
        state.bio_total_length = 0;
    }
    ensure_player_templates_loaded();
    let catalog = effects.challenge_generals().clone();
    let button_ids = effects.challenge().general_button_ids;
    let templates = get_player_template_store();
    effects.with_manager(|manager, effects| {
        for (index, button_id) in button_ids.iter().enumerate() {
            if let Some(button) = manager.get_window_by_id(*button_id) {
                let general = &catalog.challenge_generals()[index];
                let enabled = general.is_starting_enabled();
                let mut button = button.borrow_mut();
                let _ = button.enable(enabled);
                let _ = button.hide(!enabled, manager);
                if let Some(template) = templates.find_template(general.player_template_name()) {
                    apply_general_button_medallions(&mut button, template, effects);
                }
            }
        }
    });
    drop(templates);
    show_shell_map_if_available(true, effects);
    layout.hide_receiving(false, effects);
    effects.window_videos().init();
}

pub fn challenge_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let begin_transition = {
        let state = effects.challenge_mut();
        if state.just_entered {
            if state.initial_gadget_delay == 1 {
                state.initial_gadget_delay = 2;
                state.just_entered = false;
                true
            } else {
                state.initial_gadget_delay -= 1;
                false
            }
        } else {
            false
        }
    };
    if begin_transition {
        effects.with_manager(|manager, _| {
            manager.transition_set_group("ChallengeMenuFade", false, None)
        });
    }
    update_bio(effects.challenge_mut(), 2);
    if !effects.challenge().has_played_intro_audio
        && effects.with_manager(|manager, _| manager.transitions_finished())
    {
        let play_intro = {
            let state = effects.challenge_mut();
            state.intro_audio_magic_number += 1;
            state.intro_audio_magic_number == 10
        };
        if play_intro {
            play_menu_sound(effects, "Taunts_GCAnnouncer01");
            effects.challenge_mut().has_played_intro_audio = true;
        }
    }
    if effects.challenge().is_shutting_down
        && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        && effects.with_manager(|manager, _| manager.transitions_finished())
    {
        effects.challenge_mut().is_shutting_down = false;
        layout.hide_receiving(true, effects);
        queue_shell_shutdown_complete(effects, false);
    }
    effects.window_videos().update();
}

pub fn challenge_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);
    effects.window_videos().reset();
    effects.challenge_mut().last_button_index = None;
    if pop_immediate {
        layout.hide_receiving(true, effects);
        queue_shell_shutdown_complete(effects, false);
        return;
    }
    effects.with_manager(|manager, _| manager.transition_reverse("ChallengeMenuFade", None));
    effects.challenge_mut().is_shutting_down = true;
    if let Some(manager) = effects.campaign_manager.as_deref_mut() {
        manager.clear_challenge_game_info();
    }
    let (selection, preview) = {
        let state = effects.challenge();
        (
            state.last_selection_sound.clone(),
            state.last_preview_sound.clone(),
        )
    };
    stop_menu_sound(effects, selection);
    stop_menu_sound(effects, preview);
    let state = effects.challenge_mut();
    state.last_selection_sound = None;
    state.last_preview_sound = None;
    state.intro_audio_magic_number = 0;
    state.has_played_intro_audio = false;
}

pub fn challenge_menu_system(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        msg if is_general_mouse_entering(msg) => {
            let index = find_general_button(effects.challenge(), data1 as i32);
            if let Some(index) = index {
                if effects.challenge().last_button_index != Some(index) {
                    set_general_bio(Some(index), effects);
                    play_menu_sound(effects, "GUILogoMouseOver");
                    effects.challenge_mut().last_hilited_index = Some(index);
                }
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        }
        msg if is_general_mouse_leaving(msg) => {
            let index = find_general_button(effects.challenge(), data1 as i32);
            if let Some(index) = index {
                if effects.challenge().last_button_index != Some(index) {
                    let selected = effects.challenge().last_button_index;
                    set_general_bio(selected, effects);
                }
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        }
        WindowMessage::GadgetSelected => {
            if effects.challenge().is_auto_selecting {
                return WindowMsgHandled::Handled;
            }
            let control_id = data1 as i32;
            if let Some(index) = find_general_button(effects.challenge(), control_id) {
                let (previous_id, current_id) = {
                    let state = effects.challenge_mut();
                    let previous = state
                        .last_button_index
                        .filter(|prev| *prev != index)
                        .and_then(|prev| state.general_button_ids.get(prev).copied());
                    let current = state.general_button_ids.get(index).copied();
                    state.is_auto_selecting = true;
                    (previous, current)
                };
                for (id, checked) in [(previous_id, false), (current_id, true)] {
                    if let Some(id) = id {
                        effects.with_manager(|manager, effects| {
                            set_general_button_checked(
                                id,
                                checked,
                                manager,
                                Some(&mut *_window),
                                effects,
                            )
                        });
                    }
                }
                effects.challenge_mut().is_auto_selecting = false;
                let (selection, preview) = {
                    let state = effects.challenge();
                    (
                        state.last_selection_sound.clone(),
                        state.last_preview_sound.clone(),
                    )
                };
                stop_menu_sound(effects, selection);
                stop_menu_sound(effects, preview);
                let preview = effects.challenge_generals().challenge_generals()[index]
                    .preview_sound()
                    .to_owned();
                let handle = play_menu_sound(effects, &preview);
                effects.challenge_mut().last_preview_sound = handle;
                effects.challenge_mut().last_button_index = Some(index);
                set_general_bio(Some(index), effects);
                let button = effects.challenge().button_play.clone();
                set_window_hidden(&button, false, effects);
                return WindowMsgHandled::Handled;
            }
            if control_id == effects.challenge().button_play_id {
                if !effects.challenge().is_shutting_down {
                    start_challenge_game(Some(_window), effects);
                }
            } else if control_id == effects.challenge().button_back_id {
                queue_shell_pop(effects);
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn challenge_menu_input(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) != 0 {
        let (parent, back) = {
            let state = effects.challenge();
            (state.parent.clone(), state.button_back_id)
        };
        if let Some(parent) = parent.as_ref() {
            effects.queue_system_message(
                Rc::downgrade(parent),
                WindowMessage::GadgetSelected,
                back as WindowMsgData,
                back as WindowMsgData,
            );
        }
    }
    WindowMsgHandled::Handled
}

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
pub fn simulate_challenge_menu_select_general(
    index: usize,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if index >= NUM_GENERALS {
        return false;
    }
    let state = effects.challenge_mut();
    ensure_challenge_control_ids(state);
    state.last_button_index = Some(index);
    state.is_shutting_down = false;
    state.last_requested_general = Some(index);
    state.play_requested = false;
    state.last_button_index == Some(index)
}
pub fn residual_challenge_selected_general(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<usize> {
    effects.challenge().last_requested_general
}
pub fn residual_challenge_play_requested(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.challenge().play_requested
}
pub fn simulate_challenge_menu_play_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.challenge_mut();
        ensure_challenge_control_ids(state);
        if state.is_shutting_down {
            return false;
        }
        let Some(index) = state
            .last_button_index
            .filter(|&index| index < NUM_GENERALS)
        else {
            return false;
        };
        state.last_requested_general = Some(index);
        state.play_requested = true;
    }
    start_challenge_game(None, effects);
    true
}
pub fn simulate_challenge_menu_back_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let state = effects.challenge_mut();
    ensure_challenge_control_ids(state);
    state.last_button_index = None;
    state.is_shutting_down = false;
    state.play_requested = false;
    state.last_requested_general = None;
    state.button_back_id != 0 || state.button_play_id != 0
}

/// Residual: select general + ButtonPlay composite (pre-start honesty).
pub fn simulate_challenge_menu_prepare_start(
    index: usize,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_challenge_menu_select_general(index, effects) {
        return false;
    }
    simulate_challenge_menu_play_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `GeneralPositionN` then `ButtonPlay`
/// (C++ WindowXlat hit → GBM_SELECTED → startChallengeGame). Not `simulate_*`.
pub fn drive_os_wnd_challenge_start_like_cpp(
    index: usize,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if index >= NUM_GENERALS {
        return false;
    }
    let general_name = format!("ChallengeMenu.wnd:GeneralPosition{index}");
    let clicked_general = crate::gui::dispatch_os_click_named_window(&general_name, effects);
    let clicked_play =
        crate::gui::dispatch_os_click_named_window("ChallengeMenu.wnd:ButtonPlay", effects);
    if !clicked_general && !clicked_play {
        return false;
    }
    if clicked_general && !simulate_challenge_menu_select_general(index, effects) {
        return false;
    }
    if clicked_play || clicked_general {
        return simulate_challenge_menu_play_button_gadget_selected(effects);
    }
    residual_challenge_play_requested(effects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::game_client::{ClientInputAuthority, GameClient};
    use crate::display::image::{ICoord2D, Image as MappedImage};
    use crate::gui::WindowWidget;
    use crate::gui::gadgets::CheckBox;
    use crate::gui::menu_audio::SilentMenuAudio;
    use game_engine::common::language::Language;

    fn client() -> GameClient {
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap()
    }
    fn receiving<R>(
        client: &mut GameClient,
        operation: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> R {
        let mut audio = SilentMenuAudio::new();
        client.with_window_event_effects_and_menu_audio(None, &mut audio, operation)
    }
    fn isolated(name: &str) -> bool {
        matches!(
            crate::test_process::run_bounded(module_path!(), name),
            crate::test_process::TestProcess::ParentVerified
        )
    }
    fn add_test_mapped_image(name: &str, width: i32, height: i32) {
        let collection = get_mapped_image_collection();
        let mut collection = collection.write();
        let mut image = MappedImage::with_name(name);
        image.set_image_size(ICoord2D::new(width, height));
        collection.add_image(image);
    }
    #[test]
    fn esc_char_is_consumed_before_key_up_like_cpp() {
        let mut client = client();
        let mut window = GameWindow::new();
        receiving(&mut client, |effects| {
            assert_eq!(
                challenge_menu_input(
                    &mut window,
                    WindowMessage::Char,
                    KEY_ESC as WindowMsgData,
                    0,
                    effects
                ),
                WindowMsgHandled::Handled
            );
            assert_eq!(
                challenge_menu_input(
                    &mut window,
                    WindowMessage::Char,
                    b'A' as WindowMsgData,
                    0,
                    effects
                ),
                WindowMsgHandled::Ignored
            );
        });
    }
    #[test]
    fn general_button_medallions_match_cpp_draw_slots_and_size() {
        if isolated("general_button_medallions_match_cpp_draw_slots_and_size") {
            return;
        }
        add_test_mapped_image("TestMedallionNormal", 64, 48);
        add_test_mapped_image("TestMedallionSelected", 66, 50);
        add_test_mapped_image("TestMedallionHilite", 68, 52);
        let mut template = PlayerTemplate::new("TestGeneral".to_string());
        template.medallion_regular = "TestMedallionNormal".to_string();
        template.medallion_select = "TestMedallionSelected".to_string();
        template.medallion_hilite = "TestMedallionHilite".to_string();
        let mut client = client();
        let mut button = GameWindow::new();
        receiving(&mut client, |effects| {
            apply_general_button_medallions(&mut button, &template, effects)
        });
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
        if isolated("challenge_menu_init_uses_challenge_game_info_not_skirmish") {
            return;
        }
        use crate::gui::get_skirmish_setup;
        let mut client = client();
        {
            let mut setup = get_skirmish_setup();
            let info = setup.game_info_mut().game_info_mut();
            info.init();
            info.set_map("maps\\skirmish_only.map".to_string());
        }
        let layout = WindowLayout::new("Menus/ChallengeMenu.wnd".to_string());
        receiving(&mut client, |effects| {
            challenge_menu_init(&layout, None, effects)
        });
        let campaign_manager = client.campaign_manager_mut();
        assert!(
            campaign_manager.challenge_game_info().is_some(),
            "C++ ChallengeMenuInit allocates TheChallengeGameInfo"
        );
        let challenge_map = campaign_manager
            .challenge_game_info()
            .map(|info| info.game_info().get_map().to_string())
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
        campaign_manager.set_challenge_slot0_and_map(
            "maps\\GC_ChemGeneral.map".into(),
            "Test".into(),
            3,
        );
        let after = campaign_manager
            .challenge_game_info()
            .map(|info| {
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
        receiving(&mut client, |effects| {
            challenge_menu_shutdown(&layout, Some(&false), effects);
            assert!(
                effects
                    .campaign_manager
                    .as_deref()
                    .and_then(CampaignManager::challenge_game_info)
                    .is_none(),
                "C++ ChallengeMenuShutdown deletes TheChallengeGameInfo on fade path"
            );
        });
    }
    #[test]
    fn os_wnd_challenge_start_hits_named_gadgets_then_start_challenge_game() {
        let mut client = client();
        receiving(&mut client, |effects| {
            effects.with_manager(|manager, effects| {
                let general = manager
                    .create_window(None, 10, 10, 40, 40, effects)
                    .expect("general gadget");
                general
                    .borrow_mut()
                    .set_name("ChallengeMenu.wnd:GeneralPosition0");
                let _ = general.borrow_mut().hide(false, manager);
                let play = manager
                    .create_window(None, 60, 10, 40, 40, effects)
                    .expect("play gadget");
                play.borrow_mut().set_name("ChallengeMenu.wnd:ButtonPlay");
                let _ = play.borrow_mut().hide(false, manager);
            });
            assert!(
                drive_os_wnd_challenge_start_like_cpp(0, effects),
                "OS WND clicks on ChallengeMenu gadgets must startChallengeGame"
            );
            assert!(residual_challenge_play_requested(effects));
            assert_eq!(residual_challenge_selected_general(effects), Some(0));
            assert!(!drive_os_wnd_challenge_start_like_cpp(99, effects));
        });
    }
    #[test]
    fn general_bio_uses_localized_text_like_cpp() {
        if isolated("general_bio_uses_localized_text_like_cpp") {
            return;
        }
        Language::clear_localized_strings();
        Language::register_localized_string("TEST:ChallengeBioName", "General Localized");
        Language::register_localized_string("TEST:ChallengeBioRank", "Field Marshal");
        Language::register_localized_string("TEST:ChallengeBioBranch", "Armor Command");
        Language::register_localized_string("TEST:ChallengeBioStrategy", "Breakthrough tactics");
        let mut client = client();
        let mut generals = ChallengeGenerals::new();
        let general = &mut generals.challenge_generals_mut()[0];
        general.set_bio_name("TEST:ChallengeBioName".to_string());
        general.set_bio_rank("TEST:ChallengeBioRank".to_string());
        general.set_bio_branch("TEST:ChallengeBioBranch".to_string());
        general.set_bio_strategy("TEST:ChallengeBioStrategy".to_string());
        general.set_preview_sound(String::new());
        client.admit_challenge_generals(generals);
        receiving(&mut client, |effects| {
            set_general_bio(Some(0), effects);
            let state = effects.challenge();
            assert_eq!(state.bio_lines[0], "General Localized");
            assert_eq!(state.bio_lines[1], "Field Marshal");
            assert_eq!(state.bio_lines[2], "Armor Command");
            assert_eq!(state.bio_lines[3], "Breakthrough tactics");
            assert_eq!(
                state.bio_total_length,
                "General LocalizedField MarshalArmor CommandBreakthrough tactics".len()
            );
        });
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
        let mut client = client();
        receiving(&mut client, |effects| {
            let selected_id = name_to_id("ChallengeMenu.test:SelectedGeneral");
            {
                let state = effects.challenge_mut();
                state.general_button_ids = [0; NUM_GENERALS];
                state.general_button_ids[0] = selected_id;
                state.last_button_index = Some(0);
            }
            let button = effects.with_manager(|manager, effects| {
                let button = manager
                    .create_window_with_id(None, 0, 0, 32, 32, selected_id, effects)
                    .expect("create challenge general button");
                button
                    .borrow_mut()
                    .set_widget(WindowWidget::CheckBox(CheckBox::new(
                        selected_id as u32,
                        0,
                        0,
                        32,
                    )));
                let _ = button
                    .borrow_mut()
                    .gadget_check_box_set_checked(false, effects);
                button
            });
            let mut window = GameWindow::new();
            assert_eq!(
                challenge_menu_system(
                    &mut window,
                    WindowMessage::GadgetSelected,
                    selected_id as WindowMsgData,
                    selected_id as WindowMsgData,
                    effects
                ),
                WindowMsgHandled::Handled
            );
            let button = button.borrow();
            match button.widget() {
                Some(WindowWidget::CheckBox(check)) => assert!(check.is_checked()),
                _ => panic!("expected selected general checkbox"),
            }
        });
    }
    #[test]
    fn shutdown_clears_selected_general_before_pop_branch_like_cpp() {
        let mut client = client();
        let layout = WindowLayout::new("ChallengeMenu.wnd".to_string());
        for pop_immediate in [false, true] {
            receiving(&mut client, |effects| {
                {
                    let state = effects.challenge_mut();
                    state.last_button_index = Some(3);
                    state.is_shutting_down = false;
                }
                challenge_menu_shutdown(&layout, Some(&pop_immediate), effects);
                assert_eq!(effects.challenge().last_button_index, None);
            });
        }
    }
}
