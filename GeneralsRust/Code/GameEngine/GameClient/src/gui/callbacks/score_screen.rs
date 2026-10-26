//! ScoreScreen.cpp callback port.

use crate::cd_check::check_for_cd_at_game_start;
use crate::core::script_action_handler::{
    is_script_display_movie_playing, play_script_display_movie,
};
use crate::game_text::GameText;
use crate::gui::callbacks::popup_replay::popup_replay_update;
use crate::gui::campaign_launch_host_bridge::{
    HostCampaignLaunchDescriptor, MenuHostRequest, MenuLaunchSource,
};
use crate::gui::campaign_manager::{
    Campaign, CampaignManager, GameDifficulty as CampaignDifficulty,
};
use crate::gui::challenge_generals::get_challenge_generals;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::get_skirmish_setup;
use crate::gui::menu_flags::{set_dont_show_main_menu, set_replay_was_pressed};
use crate::gui::shell::Shell;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
    queue_shell_pop, queue_shell_shutdown_complete, show_shell_map_if_available,
    write_input_focus_response,
};
use crate::message_stream::{GameMessage, GameMessageType};
use game_engine::common::game_lod::prefers_low_res_movies;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::recorder::{RecorderMode, get_recorder};
use game_engine::common::rts::score_keeper::{KindOf, KindOfMaskType};
use game_engine::common::skirmish_battle_honors::{
    BATTLE_HONOR_AIR_WING, BATTLE_HONOR_APOCALYPSE, BATTLE_HONOR_BATTLE_TANK, BATTLE_HONOR_BLITZ5,
    BATTLE_HONOR_BLITZ10, BATTLE_HONOR_CAMPAIGN_CHINA, BATTLE_HONOR_CAMPAIGN_GLA,
    BATTLE_HONOR_CAMPAIGN_USA, BATTLE_HONOR_CHALLENGE_MODE, BATTLE_HONOR_LOYALTY_CHINA,
    BATTLE_HONOR_LOYALTY_GLA, BATTLE_HONOR_LOYALTY_USA, BATTLE_HONOR_STREAK, BH_CHALLENGE_MASK_1,
    BH_CHALLENGE_MASK_2, BH_CHALLENGE_MASK_3, BH_CHALLENGE_MASK_4, BH_CHALLENGE_MASK_5,
    BH_CHALLENGE_MASK_6, BH_CHALLENGE_MASK_7, SkirmishBattleHonors,
};
use game_network::{GameInfo, GameSlot, MAX_SLOTS as NETWORK_MAX_SLOTS, SlotState};
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine, TheVictoryConditions};
use gamelogic::player::{Player, PlayerType, ThePlayerList};
use gamelogic::system::game_logic::{GAME_INTERNET, GAME_LAN, GAME_SINGLE_PLAYER, GAME_SKIRMISH};
use std::cell::RefCell;
use std::rc::Rc;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const MAX_SLOTS: i32 = 8;
const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFF_FFF1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ScoreScreenType {
    #[default]
    SinglePlayer,
    Skirmish,
    Lan,
    Internet,
    Replay,
}

#[derive(Default)]
struct ScoreGather {
    total_money_earned: i32,
    total_money_spent: i32,
    total_units_destroyed: i32,
    total_units_built: i32,
    total_units_lost: i32,
    total_buildings_destroyed: i32,
    total_buildings_built: i32,
    total_buildings_lost: i32,
    side_icon: String,
}

#[derive(Default)]
pub struct ScoreScreenState {
    residual_action: u8,
    parent_id: i32,
    button_ok_id: i32,
    text_entry_chat_id: i32,
    button_emote_id: i32,
    listbox_chat_id: i32,
    listbox_academy_id: i32,
    static_text_academy_title_id: i32,
    chat_box_border_id: i32,
    button_continue_id: i32,
    button_buddies_id: i32,
    button_save_replay_id: i32,
    backdrop_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    button_continue: Option<Rc<RefCell<GameWindow>>>,
    button_save_replay: Option<Rc<RefCell<GameWindow>>>,
    text_entry_chat: Option<Rc<RefCell<GameWindow>>>,
    button_emote: Option<Rc<RefCell<GameWindow>>>,
    chat_box_border: Option<Rc<RefCell<GameWindow>>>,
    button_buddies: Option<Rc<RefCell<GameWindow>>>,
    static_text_game_saved: Option<Rc<RefCell<GameWindow>>>,
    backdrop: Option<Rc<RefCell<GameWindow>>>,
    challenge_portrait: Option<Rc<RefCell<GameWindow>>>,
    challenge_remarks: Option<Rc<RefCell<GameWindow>>>,
    challenge_win_loss_text: Option<Rc<RefCell<GameWindow>>>,
    gadget_parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_chat: Option<Rc<RefCell<GameWindow>>>,
    listbox_academy: Option<Rc<RefCell<GameWindow>>>,
    static_text_academy_title: Option<Rc<RefCell<GameWindow>>>,
    override_player_display_name: bool,
    last_replay_filename: String,
    can_save_replay: bool,
    need_finish_singleplayer_init: bool,
    button_is_finish_campaign: bool,
    pending_final_victory_movie: Option<String>,
    waiting_for_final_victory_movie: bool,
    blank_layout: Option<Rc<RefCell<WindowLayout>>>,
    popup_replay_layout: Option<Rc<RefCell<WindowLayout>>>,
    screen_type: ScoreScreenType,
    play_music: bool,
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn find_child(
    parent: &Option<Rc<RefCell<GameWindow>>>,
    name: &str,
) -> Option<Rc<RefCell<GameWindow>>> {
    parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(name_to_id(name)))
}

fn set_text(win: &Rc<RefCell<GameWindow>>, text: &str) {
    let _ = win.borrow_mut().set_text(text);
}

fn set_text_color(win: &Rc<RefCell<GameWindow>>, color: u32) {
    let border = win.borrow().get_enabled_text_border_color();
    win.borrow_mut().set_enabled_text_colors(color, border);
}

fn hide_window(
    win: &Option<Rc<RefCell<GameWindow>>>,
    hide: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if let Some(win) = win {
        let _ = win.borrow_mut().hide(hide, receiving_manager);
    }
}

fn enable_window(win: &Option<Rc<RefCell<GameWindow>>>, enable: bool) {
    if let Some(win) = win {
        let _ = win.borrow_mut().enable(enable);
    }
}

fn set_window_image(win: &Option<Rc<RefCell<GameWindow>>>, image_name: &str) {
    let Some(win) = win else {
        return;
    };
    if image_name.is_empty() {
        return;
    }

    let mut image = crate::gui::game_window::Image {
        name: image_name.to_string(),
        width: 0,
        height: 0,
    };
    if let Some(collection) = crate::display::image::get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            image.width = found.get_image_width();
            image.height = found.get_image_height();
        }
    }

    let mut win_guard = win.borrow_mut();
    if win_guard.set_enabled_image(0, image).is_ok() {
        win_guard.set_status(WindowStatus::IMAGE);
    }
}

fn score_screen_music_from_game_info_slot(manager: Option<&CampaignManager>) -> String {
    // C++ ScoreScreenUpdate: TheGameInfo slot template, FactionObserver if < 0.
    // Must not touch PlayerList — clearGameData runs before the first Update.
    game_engine::common::ini::ensure_player_templates_loaded();
    let store = game_engine::common::rts::player_template::get_player_template_store();
    let template_index = score_screen_local_slot_template_index(manager);
    let template = if template_index >= 0 {
        usize::try_from(template_index)
            .ok()
            .and_then(|index| store.get_nth_player_template(index))
    } else {
        store.find_template("FactionObserver")
    };
    template
        .map(|pt| pt.get_score_screen_music().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
}

fn score_screen_local_slot_template_index(manager: Option<&CampaignManager>) -> i32 {
    let from_info = |game: &GameInfo| {
        if !game.is_in_game() {
            return None;
        }
        let slot_num = game.get_local_slot_num();
        if slot_num < 0 {
            return None;
        }
        game.get_slot(slot_num as usize)
            .map(|slot| slot.get_player_template())
    };
    if let Some(index) = from_info(get_skirmish_setup().game_info().game_info()) {
        return index;
    }
    manager
        .and_then(CampaignManager::challenge_game_info)
        .map(|info| from_info(info.game_info()))
        .flatten()
        .unwrap_or(-1)
}

fn update_score_screen_music(effects: &mut WindowEventEffects<'_>) {
    if !effects.score_screen().play_music {
        return;
    }
    effects.score_screen_mut().play_music = false;
    let Some(audio) = TheAudio::get() else {
        return;
    };
    let music_name = score_screen_music_from_game_info_slot(effects.campaign_manager.as_deref());
    if !music_name.is_empty() {
        audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        let mut event = gamelogic::common::audio::AudioEventRts::new(music_name);
        event.set_should_fade(true);
        let _ = audio.add_audio_event(&event);
        audio.update();
    }
}

pub fn score_screen_enable_controls(enable: bool, effects: &mut WindowEventEffects<'_>) {
    {
        let state = effects.score_screen_mut();
        if let Some(button_ok) = state.button_ok.as_ref() {
            if !button_ok.borrow().is_hidden() {
                let _ = button_ok.borrow_mut().enable(enable);
            }
        }

        if let Some(button_continue) = state.button_continue.as_ref() {
            if !button_continue.borrow().is_hidden() {
                let _ = button_continue.borrow_mut().enable(enable);
            }
        }

        if let Some(button_buddies) = state.button_buddies.as_ref() {
            if !button_buddies.borrow().is_hidden() {
                let _ = button_buddies.borrow_mut().enable(enable);
            }
        }

        if let Some(button) = state.button_save_replay.as_ref() {
            if !button.borrow().is_hidden() {
                let _ = button.borrow_mut().enable(enable && state.can_save_replay);
            }
        }
    };
}

trait NextCampaignShellActions {
    fn pop_score_screen_immediate(&mut self);
    fn hide_shell_for_next_campaign(&mut self);
}

fn leave_score_screen_for_next_campaign(shell: &mut impl NextCampaignShellActions) {
    shell.pop_score_screen_immediate();
    shell.hide_shell_for_next_campaign();
}

fn start_next_campaign_game(effects: &mut WindowEventEffects<'_>) {
    if effects.campaign_manager.is_none() {
        return;
    }
    // C++ ScoreScreen.cpp startNextCampaignGame.
    crate::gui::shell::base::queue_shell_operation_with_effects(effects, |shell, effects| {
        let _ = shell.pop_immediate(effects);
        let _ = shell.hide_shell(effects);
    });

    let Some(manager) = effects.campaign_manager.as_deref_mut() else {
        return;
    };
    let (pending_file, campaign_name, campaign_player_faction, is_challenge) = {
        let campaign = manager.get_current_campaign();
        (
            manager.get_current_map().unwrap_or_default(),
            campaign
                .map(|campaign| campaign.name.clone())
                .unwrap_or_default(),
            campaign
                .map(|campaign| campaign.player_faction_name.clone())
                .unwrap_or_default(),
            campaign
                .map(|campaign| campaign.is_challenge_campaign())
                .unwrap_or(false),
        )
    };

    let mut challenge_template_num = None;
    let mut game_info_starting_cash = None;
    if is_challenge {
        // C++ rematch: init/clearSlotList/reset/enterGame then set map+slot0.
        manager.init_challenge_game_info();
        let template_num = manager.get_xfer_challenge_generals_player_template_num();
        manager.set_challenge_slot0_and_map(pending_file.clone(), String::new(), template_num);
        challenge_template_num = Some(template_num);
        // C++ rematch initializes a new Challenge GameInfo. Retain that
        // new input, including zero, before the old match is cleared.
        game_info_starting_cash = manager
            .challenge_game_info()
            .map(|info| info.game_info().get_starting_cash().count_money());
    }

    if let Some(data) = game_engine::common::ini::get_global_data() {
        let mut data = data.write();
        data.pending_file = pending_file.clone();
    }
    game_engine::common::global_data::write().pending_file = pending_file.clone();

    let difficulty = manager.get_game_difficulty() as i32;
    let rank_points = manager.get_rank_points();

    // A continued Challenge can arrive after a shell/load transition where
    // the display path has not yet touched PlayerTemplate.ini. C++ resolves
    // the stored slot through its always-live PlayerTemplateStore here; load
    // the same exact store before converting the saved index back to identity.
    game_engine::common::ini::ensure_player_templates_loaded();
    let challenge_template_name = challenge_template_num.and_then(|template_num| {
        usize::try_from(template_num).ok().and_then(|index| {
            game_engine::common::rts::player_template::get_player_template_store()
                .get_nth_player_template(index)
                .map(|template| template.get_name().to_string())
        })
    });
    let descriptor = HostCampaignLaunchDescriptor {
        map_name: pending_file,
        campaign_name,
        campaign_player_faction,
        is_challenge,
        game_info_starting_cash,
        player_template_name: challenge_template_name,
        player_template_index: challenge_template_num,
        game_mode_code: GAME_SINGLE_PLAYER,
        difficulty_code: difficulty,
        rank_points,
        max_fps: None,
    };
    let clear_previous = is_challenge && effects.menu_world().in_game;
    effects.request_menu_host(MenuHostRequest::StartCampaign {
        descriptor,
        clear_previous,
        source: MenuLaunchSource::ScoreScreen,
        main_menu_layout: None,
    });
}

fn init_single_player(effects: &mut WindowEventEffects<'_>) {
    {
        let value = ScoreScreenType::SinglePlayer;
        effects.score_screen_mut().screen_type = value
    };
    if let Ok(list) = ThePlayerList().read() {
        if let Some(local_player) = list.get_local_player() {
            if let Ok(player) = local_player.read() {
                if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                    manager.set_rank_points(player.get_skill_points());
                    manager.set_game_difficulty(match TheScriptEngine::get_global_difficulty() {
                        0 => CampaignDifficulty::Easy,
                        2 => CampaignDifficulty::Hard,
                        _ => CampaignDifficulty::Normal,
                    });
                }
            }
        }
    }
    effects.with_manager(|manager, effects| {
        grab_single_player_info(effects.score_screen_mut(), manager)
    });
    {
        let value = true;
        effects.score_screen_mut().need_finish_singleplayer_init = value
    };

    let blank_layout = effects.with_manager(|manager, effects| {
        manager.create_layout("Menus/BlankWindow.wnd".to_string())
    });
    effects.with_manager(|manager, effects| blank_layout.borrow_mut().hide(manager, false));
    effects.with_manager(|manager, effects| blank_layout.borrow_mut().bring_forward(manager));
    // C++ initSinglePlayer: first window winClearStatus(WIN_STATUS_IMAGE).
    if let Some(first) = blank_layout.borrow().get_first_window() {
        first.borrow_mut().clear_status(WindowStatus::IMAGE);
    }
    {
        let value = Some(blank_layout);
        effects.score_screen_mut().blank_layout = value
    };
}

fn init_skirmish(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.screen_type = ScoreScreenType::Skirmish;
    grab_multi_player_info(state, receiving_manager);
    hide_window(&state.text_entry_chat, true, receiving_manager);
    hide_window(&state.button_emote, true, receiving_manager);
    hide_window(&state.chat_box_border, true, receiving_manager);
    hide_window(&state.button_buddies, true, receiving_manager);
    hide_window(&state.button_continue, true, receiving_manager);
    hide_window(&state.listbox_chat, true, receiving_manager);
    hide_window(&state.static_text_game_saved, true, receiving_manager);
}

fn init_lan_multiplayer(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.screen_type = ScoreScreenType::Lan;
    grab_multi_player_info(state, receiving_manager);
    if let Some(text_entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text(String::new());
        }
    }
    hide_window(&state.static_text_game_saved, true, receiving_manager);
    hide_window(&state.text_entry_chat, false, receiving_manager);
    hide_window(&state.button_emote, false, receiving_manager);
    hide_window(&state.button_continue, true, receiving_manager);
    hide_window(&state.listbox_chat, false, receiving_manager);
    hide_window(&state.listbox_academy, true, receiving_manager);
    hide_window(&state.static_text_academy_title, true, receiving_manager);
    hide_window(&state.chat_box_border, false, receiving_manager);
    hide_window(&state.button_buddies, true, receiving_manager);
}

fn init_internet_multiplayer(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.screen_type = ScoreScreenType::Internet;
    grab_multi_player_info(state, receiving_manager);
    if let Some(text_entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text(String::new());
        }
    }
    hide_window(&state.static_text_game_saved, true, receiving_manager);
    hide_window(&state.button_continue, true, receiving_manager);
    hide_window(&state.text_entry_chat, true, receiving_manager);
    hide_window(&state.button_emote, true, receiving_manager);
    hide_window(&state.listbox_chat, false, receiving_manager);
    hide_window(&state.listbox_academy, false, receiving_manager);
    hide_window(&state.static_text_academy_title, false, receiving_manager);
    hide_window(&state.chat_box_border, false, receiving_manager);
    hide_window(&state.button_buddies, false, receiving_manager);
}

fn init_replay_multiplayer(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.screen_type = ScoreScreenType::Replay;
    grab_multi_player_info(state, receiving_manager);
    hide_window(&state.static_text_game_saved, true, receiving_manager);
    hide_window(&state.text_entry_chat, true, receiving_manager);
    hide_window(&state.button_emote, true, receiving_manager);
    hide_window(&state.listbox_chat, true, receiving_manager);
    hide_window(&state.listbox_academy, true, receiving_manager);
    hide_window(&state.static_text_academy_title, true, receiving_manager);
    hide_window(&state.chat_box_border, true, receiving_manager);
    hide_window(&state.button_continue, true, receiving_manager);
    hide_window(&state.button_buddies, true, receiving_manager);
}

fn init_replay_single_player(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    state.screen_type = ScoreScreenType::Replay;
    grab_single_player_info(state, receiving_manager);
    hide_window(&state.static_text_game_saved, true, receiving_manager);
    hide_window(&state.text_entry_chat, true, receiving_manager);
    hide_window(&state.button_emote, true, receiving_manager);
    hide_window(&state.chat_box_border, true, receiving_manager);
    hide_window(&state.button_continue, true, receiving_manager);
    hide_window(&state.button_buddies, true, receiving_manager);
    hide_window(&state.listbox_chat, true, receiving_manager);
    hide_window(&state.listbox_academy, true, receiving_manager);
    hide_window(&state.static_text_academy_title, true, receiving_manager);
}

fn display_challenge_win_loss(
    state: &mut ScoreScreenState,
    image_name: &str,
    header: &str,
    remarks: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    hide_window(&state.backdrop, true, receiving_manager);
    hide_window(&state.gadget_parent, true, receiving_manager);
    hide_window(&state.challenge_win_loss_text, false, receiving_manager);
    hide_window(&state.challenge_remarks, false, receiving_manager);
    hide_window(&state.challenge_portrait, false, receiving_manager);
    set_window_image(&state.parent, "GeneralsChallengeWinLoss");
    set_window_image(&state.challenge_portrait, image_name);
    if let Some(win) = state.challenge_win_loss_text.as_ref() {
        set_text(win, header);
    }
    if let Some(win) = state.challenge_remarks.as_ref() {
        set_text(win, remarks);
    }
}

fn finalize_single_player_init(effects: &mut WindowEventEffects<'_>) {
    // C++ finishSinglePlayerInit: freeMessageResources before destroying BlankWindow.
    effects.ui_mut().free_message_resources();
    if let Some(blank) = effects.score_screen_mut().blank_layout.take() {
        effects
            .with_manager(|manager, effects| blank.borrow_mut().destroy_windows(manager, effects));
    }

    if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
        let _ = effects.with_manager(|manager, effects| manager.activate_window(parent));
    }

    effects.with_manager(|manager, effects| {
        hide_window(&effects.score_screen().button_ok.clone(), false, manager)
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().button_continue.clone(),
            false,
            manager,
        )
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().text_entry_chat.clone(),
            true,
            manager,
        )
    });
    effects.with_manager(|manager, effects| {
        hide_window(&effects.score_screen().button_emote.clone(), true, manager)
    });
    effects.with_manager(|manager, effects| {
        hide_window(&effects.score_screen().listbox_chat.clone(), true, manager)
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().listbox_academy.clone(),
            true,
            manager,
        )
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().static_text_academy_title.clone(),
            true,
            manager,
        )
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().chat_box_border.clone(),
            true,
            manager,
        )
    });
    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().button_buddies.clone(),
            true,
            manager,
        )
    });

    if let Some(campaign) = effects
        .campaign_manager
        .as_deref()
        .and_then(CampaignManager::get_current_campaign)
    {
        if !campaign.is_challenge_campaign() {
            effects.with_manager(|manager, effects| {
                manager.transition_set_group("ScoreScreenShow", false, None)
            });
        }
    }
}

fn maybe_start_final_victory_movie(effects: &mut WindowEventEffects<'_>) -> bool {
    let Some(movie_name) = effects
        .score_screen_mut()
        .pending_final_victory_movie
        .take()
    else {
        return false;
    };

    if play_script_display_movie(effects.script_display(), &movie_name) {
        {
            let value = true;
            effects.score_screen_mut().waiting_for_final_victory_movie = value
        };
        return true;
    }

    false
}

fn final_victory_movie_to_queue(campaign: &Campaign, use_low_res_movies: bool) -> Option<String> {
    let final_movie = campaign.get_final_victory_movie().trim();
    if final_movie.is_empty() || use_low_res_movies {
        return None;
    }

    Some(final_movie.to_string())
}

fn update_final_victory_movie_wait(effects: &mut WindowEventEffects<'_>) {
    if !effects
        .score_screen()
        .waiting_for_final_victory_movie
        .clone()
    {
        return;
    }

    if is_script_display_movie_playing(effects.script_display()) {
        return;
    }

    {
        let value = false;
        effects.score_screen_mut().waiting_for_final_victory_movie = value
    };
    finalize_single_player_init(effects);
}

/// C++ `GameState::missionSave` (GameState.cpp:616-618):
/// `desc.format(TheGameText->fetch("GUI:MissionSave"), campaignLabel, missionNumber)`.
fn format_mission_save_description(
    format: &str,
    campaign_label: &str,
    mission_number: i32,
) -> String {
    let mut out = String::with_capacity(format.len() + campaign_label.len() + 8);
    let mut chars = format.chars().peekable();
    let mut used_s = false;
    let mut used_d = false;
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('%') => {
                chars.next();
                out.push('%');
            }
            Some('s') | Some('S') => {
                chars.next();
                out.push_str(campaign_label);
                used_s = true;
            }
            Some('d') | Some('i') | Some('u') => {
                chars.next();
                out.push_str(&mission_number.to_string());
                used_d = true;
            }
            Some('l') => {
                chars.next();
                if matches!(chars.peek().copied(), Some('s') | Some('S')) {
                    chars.next();
                    out.push_str(campaign_label);
                    used_s = true;
                } else {
                    out.push('%');
                    out.push('l');
                }
            }
            Some(_) | None => out.push('%'),
        }
    }
    if !used_s && !used_d {
        if !campaign_label.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(campaign_label);
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&mission_number.to_string());
    }
    out
}

/// Localized between-mission save description (not `GUI:AutoSave`).
fn current_mission_save_description(manager: &CampaignManager) -> String {
    let mission_number = manager.get_current_mission_number().unwrap_or(0) + 1;
    let campaign_label = manager
        .get_current_campaign()
        .map(|campaign| {
            let (text, exists) = GameText::fetch_with_exists(&campaign.campaign_name_label);
            if exists && !text.is_empty() {
                text
            } else if !campaign.campaign_name_label.is_empty() {
                campaign.campaign_name_label.clone()
            } else {
                campaign.name.clone()
            }
        })
        .unwrap_or_default();
    let (format, _) = GameText::fetch_with_exists("GUI:MissionSave");
    format_mission_save_description(&format, &campaign_label, mission_number)
}

fn finish_single_player_init(effects: &mut WindowEventEffects<'_>) {
    let Some(victorious) = effects
        .campaign_manager
        .as_deref()
        .map(|manager| manager.is_victorious())
    else {
        return;
    };
    // Definition strings are immutable presentation inputs. End both the campaign
    // loan and definition-store lock before delivering window/audio callbacks.
    let challenge_presentation = effects.campaign_manager.as_deref().and_then(|manager| {
        if !manager.get_current_campaign()?.is_challenge_campaign() {
            return None;
        }
        let mission = manager.get_current_mission()?;
        let generals = get_challenge_generals()?;
        let generals = generals.lock().unwrap_or_else(|error| error.into_inner());
        let general = generals.general_by_general_name(&mission.general_name)?;
        let (header_key, image, remarks, sound) = if victorious {
            (
                "GUI:ChallengeWinText",
                general.image_defeated(),
                general.string_defeated(),
                general.win_sound(),
            )
        } else {
            (
                "GUI:ChallengeLossText",
                general.image_victorious(),
                general.string_victorious(),
                general.loss_sound(),
            )
        };
        Some((
            image.unwrap_or_default().to_string(),
            GameText::fetch(header_key).replace("%s", &GameText::fetch(&mission.general_name)),
            GameText::fetch(remarks),
            sound.to_string(),
        ))
    });
    if let Some((image, header, remarks, sound)) = challenge_presentation {
        effects.with_manager(|manager, effects| {
            let state = effects.score_screen_mut();
            display_challenge_win_loss(state, &image, &header, &remarks, manager)
        });
        if let Some(audio) = TheAudio::get() {
            let event = gamelogic::common::audio::AudioEventRts::new(&sound);
            audio.add_audio_event(&event);
        }
    }

    if victorious {
        // Complete the campaign phase before borrowing score/window state.
        // These are immutable definition/description observations; none of the
        // intervening text/flag writes can invoke a campaign callback.
        let (next_map, completed_campaign, completed_difficulty, mission_description) = {
            let Some(manager) = effects.campaign_manager.as_deref_mut() else {
                return;
            };
            let _ = manager.goto_next_mission();
            (
                manager.get_current_map().unwrap_or_default(),
                manager.get_current_campaign().cloned(),
                manager.get_game_difficulty(),
                current_mission_save_description(manager),
            )
        };

        if next_map.is_empty() {
            if let Some(button_continue) = effects.score_screen().button_continue.clone().as_ref() {
                set_text(button_continue, &GameText::fetch("GUI:EndCampaign"));
            }
            {
                let value = true;
                effects.score_screen_mut().button_is_finish_campaign = value
            };

            let mut stats = SkirmishBattleHonors::new();
            if let Some(campaign) = completed_campaign.as_ref() {
                let difficulty = completed_difficulty;
                let difficulty_index = match difficulty {
                    CampaignDifficulty::Easy => 0,
                    CampaignDifficulty::Normal => 1,
                    CampaignDifficulty::Hard => 2,
                };
                match campaign.name.as_str() {
                    name if name.eq_ignore_ascii_case("usa") => {
                        stats.set_usa_campaign_complete(difficulty_index);
                        stats.set_honors(BATTLE_HONOR_CAMPAIGN_USA as i32);
                    }
                    name if name.eq_ignore_ascii_case("china") => {
                        stats.set_china_campaign_complete(difficulty_index);
                        stats.set_honors(BATTLE_HONOR_CAMPAIGN_CHINA as i32);
                    }
                    name if name.eq_ignore_ascii_case("gla") => {
                        stats.set_gla_campaign_complete(difficulty_index);
                        stats.set_honors(BATTLE_HONOR_CAMPAIGN_GLA as i32);
                    }
                    _ => {}
                }
                let upper = campaign.name.to_ascii_uppercase();
                if let Some(index) = upper
                    .strip_prefix("CHALLENGE_")
                    .and_then(|value| value.parse::<usize>().ok())
                {
                    stats.set_challenge_campaign_complete(index, difficulty_index);
                    stats.set_honors(BATTLE_HONOR_CHALLENGE_MODE as i32);
                }
            }
            let _ = stats.write();

            effects.with_manager(|manager, effects| {
                hide_window(&effects.score_screen().button_ok.clone(), true, manager)
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().button_continue.clone(),
                    true,
                    manager,
                )
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().text_entry_chat.clone(),
                    true,
                    manager,
                )
            });
            effects.with_manager(|manager, effects| {
                hide_window(&effects.score_screen().button_emote.clone(), true, manager)
            });
            effects.with_manager(|manager, effects| {
                hide_window(&effects.score_screen().listbox_chat.clone(), true, manager)
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().listbox_academy.clone(),
                    true,
                    manager,
                )
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().static_text_academy_title.clone(),
                    true,
                    manager,
                )
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().chat_box_border.clone(),
                    true,
                    manager,
                )
            });
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().button_buddies.clone(),
                    true,
                    manager,
                )
            });

            if let Some(campaign) = effects
                .campaign_manager
                .as_deref()
                .and_then(|manager| manager.get_current_campaign())
            {
                {
                    let value = final_victory_movie_to_queue(campaign, prefers_low_res_movies());
                    effects.score_screen_mut().pending_final_victory_movie = value
                };
            }

            if maybe_start_final_victory_movie(effects) {
                return;
            }
        } else {
            if let Some(button_continue) = effects.score_screen().button_continue.clone().as_ref() {
                set_text(button_continue, &GameText::fetch("GUI:SaveAndContinue"));
            }
            // C++ ScoreScreen.cpp:880 TheGameState->missionSave()
            // GameState.cpp:616-618 GUI:MissionSave + campaignNameLabel + missionNumber+1.
            let mut game_state = game_engine::get_game_state();
            let _ = game_state.save_game(
                String::new(),
                mission_description,
                game_engine::SaveFileType::Mission,
                game_engine::SnapshotType::SaveLoad,
            );
            effects.with_manager(|manager, effects| {
                hide_window(
                    &effects.score_screen().static_text_game_saved.clone(),
                    false,
                    manager,
                )
            });
        }
    } else {
        if let Some(button_continue) = effects.score_screen().button_continue.clone().as_ref() {
            set_text(button_continue, &GameText::fetch("GUI:Retry"));
        }
    }

    finalize_single_player_init(effects);
}

fn grab_multi_player_info(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    let Ok(list) = ThePlayerList().read() else {
        return;
    };

    // C++ grabMultiPlayerInfo: MutiPlayer_ScoreScreen + WIN_STATUS_IMAGE.
    if list.get_local_player().is_some() {
        set_window_image(&state.parent, "MutiPlayer_ScoreScreen");
    }

    // Only player0..player{MAX_SLOTS-1}. Civilians/script sides are not named playerN.
    let mut players: Vec<(i32, std::sync::Arc<std::sync::RwLock<Player>>)> = Vec::new();
    let mut adder = 1;
    for i in 0..MAX_SLOTS {
        let name = format!("player{i}");
        let Some(player_arc) = list.find_player_by_name(&name) else {
            continue;
        };
        let Ok(player) = player_arc.read() else {
            continue;
        };
        let mut score = player.get_score_keeper().get_total_score();
        if players.iter().any(|(existing, _)| *existing == score) {
            score += adder;
            adder += 1;
        }
        drop(player);
        players.push((score, player_arc));
    }

    players.sort_by(|a, b| b.0.cmp(&a.0));

    hide_windows(state, players.len() as i32, receiving_manager);
    for (index, (_, player_arc)) in players.into_iter().enumerate() {
        if let Ok(player) = player_arc.read() {
            if player.is_player_observer() {
                set_observer_windows(state, &player, index as i32, receiving_manager);
            } else {
                populate_player_info(state, &player, index as i32, receiving_manager);
            }
        }
    }
}

fn grab_single_player_info(
    state: &mut ScoreScreenState,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let Some(local_player_arc) = list.get_local_player() else {
        return;
    };
    let Ok(local_player) = local_player_arc.read() else {
        return;
    };

    let mut player_count = 0;
    if !local_player.is_player_observer() {
        populate_player_info(state, &local_player, player_count, receiving_manager);
        player_count += 1;
    } else {
        for player_arc in list.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    populate_player_info(state, &player, player_count, receiving_manager);
                    player_count += 1;
                    break;
                }
            }
        }
    }

    if let Some(template) = local_player.get_player_template() {
        set_window_image(&state.parent, template.get_score_screen());
    }

    for (side, is_friend) in [
        ("USA", true),
        ("USA", false),
        ("China", true),
        ("China", false),
        ("GLA", true),
        ("GLA", false),
    ] {
        let mut gather = ScoreGather::default();
        let mut populate = false;
        let mut color = 0xffffffffu32;

        for player_arc in list.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_index() == local_player.get_player_index() {
                    continue;
                }
                if player.get_base_side().eq_ignore_ascii_case(side)
                    && (TheGameLogic::get_game_mode() != GAME_SINGLE_PLAYER
                        || player.get_list_in_score_screen())
                {
                    let relationship = player
                        .get_default_team()
                        .as_ref()
                        .and_then(|team| {
                            team.read()
                                .ok()
                                .map(|team| local_player.get_relationship_with_team(&team))
                        })
                        .unwrap_or(gamelogic::prelude::Relationship::Neutral);

                    if include_in_campaign_side_row(is_friend, relationship) {
                        let score = player.get_score_keeper();
                        gather.total_buildings_built += score.get_total_buildings_built();
                        gather.total_buildings_destroyed += score.get_total_buildings_destroyed();
                        gather.total_buildings_lost += score.get_total_buildings_lost();
                        gather.total_money_earned += score.get_total_money_earned();
                        gather.total_money_spent += score.get_total_money_spent();
                        gather.total_units_built += score.get_total_units_built();
                        gather.total_units_destroyed += score.get_total_units_destroyed();
                        gather.total_units_lost += score.get_total_units_lost();
                        gather.side_icon = player
                            .get_player_template()
                            .map(|template| template.get_side_icon_image().to_string())
                            .unwrap_or_default();
                        color = player.get_player_color().to_argb_u32();
                        populate = true;
                    }
                }
            }
        }

        if populate {
            let mut label = format!("GUI:{}", side);
            if is_friend {
                label.push_str("Allies");
            } else {
                label.push_str("Enemies");
            }
            populate_side_info(
                state,
                &GameText::fetch(&label),
                &gather,
                player_count,
                color,
                receiving_manager,
            );
            player_count += 1;
        }
    }

    hide_windows(state, player_count, receiving_manager);
}

fn hide_windows(
    state: &mut ScoreScreenState,
    pos: i32,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }

    for i in pos..MAX_SLOTS {
        let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextObserver{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsBuilt{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsLost{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsDestroyed{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsBuilt{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsLost{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsDestroyed{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:StaticTextResources{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
        let name = format!("ScoreScreen.wnd:GameWindowWinner{}", i);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
    }
}

fn set_observer_windows(
    state: &mut ScoreScreenState,
    player: &Player,
    index: i32,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if !(0..MAX_SLOTS).contains(&index) {
        return;
    }

    let color = 0xffffffffu32;
    let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", index);
    if let Some(win) = find_child(&state.parent, &name) {
        set_text(&win, player.get_player_display_name());
        let _ = win.borrow_mut().hide(false, receiving_manager);
        set_text_color(&win, color);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", index);
    hide_window(&find_child(&state.parent, &name), false, receiving_manager);

    for field in [
        "StaticTextUnitsBuilt",
        "StaticTextUnitsLost",
        "StaticTextUnitsDestroyed",
        "StaticTextBuildingsBuilt",
        "StaticTextBuildingsLost",
        "StaticTextBuildingsDestroyed",
        "StaticTextResources",
    ] {
        let name = format!("ScoreScreen.wnd:{}{}", field, index);
        hide_window(&find_child(&state.parent, &name), true, receiving_manager);
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", index);
    let win = find_child(&state.parent, &name);
    hide_window(&win, false, receiving_manager);
    if let Some(template) = player.get_player_template() {
        set_window_image(&win, template.get_side_icon_image());
    }
}

const LOGICFRAMES_PER_SECOND: u32 = 30;

const SUPERWEAPON_TEMPLATES_SCUD: &[&str] = &[
    "GLAScudStorm",
    "Boss_GLAScudStorm",
    "Chem_GLAScudStorm",
    "Slth_GLAScudStorm",
    "Demo_GLAScudStorm",
];
const SUPERWEAPON_TEMPLATES_PPC: &[&str] = &[
    "AmericaParticleCannonUplink",
    "AirF_AmericaParticleCannonUplink",
    "Lazr_AmericaParticleCannonUplink",
    "SupW_AmericaParticleCannonUplink",
    "Boss_ParticleCannonUplink",
];
const SUPERWEAPON_TEMPLATES_NUKE: &[&str] = &[
    "ChinaNuclearMissileLauncher",
    "Boss_NuclearMissileLauncher",
    "Infa_ChinaNuclearMissileLauncher",
    "Nuke_ChinaNuclearMissileLauncher",
    "Tank_ChinaNuclearMissileLauncher",
];

fn is_slot_local_ally(game: &GameInfo, slot: &GameSlot) -> bool {
    let local_num = game.get_local_slot_num();
    let Some(local_slot) = game.get_slot(local_num.max(0) as usize) else {
        return true;
    };
    if std::ptr::eq(slot, local_slot) {
        return true;
    }
    if slot.get_team_number() < 0 {
        return false;
    }
    slot.get_team_number() == local_slot.get_team_number()
}

/// C++ ScoreScreen.cpp:1568-1576 persist skip.
fn should_skip_skirmish_honor_persist(
    sandbox: bool,
    allied_defeat: bool,
    allied_victory: bool,
    player_active: bool,
) -> bool {
    if sandbox || !(allied_defeat || allied_victory) {
        player_active
    } else {
        false
    }
}

/// C++ VictoryConditions::isLocalAlliedDefeat / hasBeenDefeated:
/// single remaining alliance and the local player did not win.
fn is_local_allied_defeat_like_cpp(local: &Player) -> bool {
    if TheVictoryConditions::is_local_allied_victory() {
        return false;
    }
    if local.is_player_observer() {
        return leftover_single_alliance_remaining(local);
    }
    leftover_single_alliance_remaining(local) && !leftover_has_achieved_victory(local)
}

fn leftover_player_individually_alive(player: &Player) -> bool {
    !player.is_player_observer()
        && player.get_player_type() != PlayerType::Neutral
        && player.has_any_objects()
}

fn leftover_players_are_allies(a: &Player, b: &Player) -> bool {
    if a.get_player_index() == b.get_player_index() {
        return true;
    }
    let Some(team) = a.get_default_team() else {
        return false;
    };
    let Ok(team) = team.read() else {
        return false;
    };
    b.get_relationship_with_team(&team) == gamelogic::prelude::Relationship::Allies
}

fn leftover_has_achieved_victory(local: &Player) -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };
    for player_arc in list.iter() {
        let Ok(player) = player_arc.read() else {
            continue;
        };
        if leftover_player_individually_alive(&player)
            && leftover_players_are_allies(local, &player)
        {
            return true;
        }
    }
    false
}

fn leftover_single_alliance_remaining(local: &Player) -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };
    let mut saw_local_alliance = false;
    let mut saw_other_alliance = false;
    for player_arc in list.iter() {
        let Ok(player) = player_arc.read() else {
            continue;
        };
        if !leftover_player_individually_alive(&player) {
            continue;
        }
        if leftover_players_are_allies(local, &player) {
            saw_local_alliance = true;
        } else {
            saw_other_alliance = true;
        }
    }
    saw_local_alliance != saw_other_alliance
}

fn update_skirmish_battle_honors(
    stats: &mut SkirmishBattleHonors,
    player: &Player,
    end_frame: u32,
) {
    let score = player.get_score_keeper();
    if stats.get_win_streak() >= 5 {
        stats.set_honors(BATTLE_HONOR_STREAK as i32);
    }
    for name in SUPERWEAPON_TEMPLATES_SCUD {
        if score.get_total_objects_built(name) > 0 {
            stats.set_built_scud();
        }
    }
    for name in SUPERWEAPON_TEMPLATES_PPC {
        if score.get_total_objects_built(name) > 0 {
            stats.set_built_particle_cannon();
        }
    }
    for name in SUPERWEAPON_TEMPLATES_NUKE {
        if score.get_total_objects_built(name) > 0 {
            stats.set_built_nuke();
        }
    }
    if stats.built_nuke() && stats.built_particle_cannon() && stats.built_scud() {
        stats.set_honors(BATTLE_HONOR_APOCALYPSE as i32);
    }

    let mut vehicle_mask = KindOfMaskType::new();
    vehicle_mask.set(KindOf::Vehicle);
    let mut aircraft_invalid = KindOfMaskType::new();
    aircraft_invalid.set(KindOf::Aircraft);
    if score.get_total_units_built_filtered(&vehicle_mask, &aircraft_invalid) >= 50 {
        stats.set_honors(BATTLE_HONOR_BATTLE_TANK as i32);
    }
    let mut aircraft_mask = KindOfMaskType::new();
    aircraft_mask.set(KindOf::Aircraft);
    let none_mask = KindOfMaskType::new();
    if score.get_total_units_built_filtered(&aircraft_mask, &none_mask) >= 20 {
        stats.set_honors(BATTLE_HONOR_AIR_WING as i32);
    }

    // C++ TheGameLogic->getFrame() is the live end frame. Leftover crate
    // clock stays 0; Eva already publishes the host logic frame.
    let minutes = end_frame / LOGICFRAMES_PER_SECOND / 60;
    if minutes < 5 {
        stats.set_honors(BATTLE_HONOR_BLITZ5 as i32);
    }
    if minutes < 10 {
        stats.set_honors(BATTLE_HONOR_BLITZ10 as i32);
    }

    let side = player.get_side();
    if stats.get_num_games_loyal() >= 20 {
        if side == "America" {
            stats.set_honors(BATTLE_HONOR_LOYALTY_USA as i32);
        } else if side == "China" {
            stats.set_honors(BATTLE_HONOR_LOYALTY_CHINA as i32);
        } else if side == "GLA" {
            stats.set_honors(BATTLE_HONOR_LOYALTY_GLA as i32);
        }
    }

    let setup = get_skirmish_setup();
    let game = setup.game_info().game_info();
    let mut num_easy = 0;
    let mut num_medium = 0;
    let mut num_brutal = 0;
    for i in 0..NETWORK_MAX_SLOTS {
        let Some(slot) = game.get_slot(i) else {
            continue;
        };
        if slot.is_ai() && !is_slot_local_ally(game, slot) {
            match slot.get_state() {
                SlotState::EasyAI => num_easy += 1,
                SlotState::MedAI => num_medium += 1,
                SlotState::BrutalAI => num_brutal += 1,
                _ => {}
            }
        }
    }
    if num_easy > 0 || num_medium > 0 || num_brutal > 0 {
        let map = game.get_map().to_string();
        if num_easy > 0 {
            let old = stats.get_endurance_medal(&map, SlotState::EasyAI as i32);
            stats.set_endurance_medal(
                &map,
                SlotState::EasyAI as i32,
                old.max(num_easy + num_medium + num_brutal),
            );
        }
        if num_medium > 0 {
            let old = stats.get_endurance_medal(&map, SlotState::MedAI as i32);
            stats.set_endurance_medal(
                &map,
                SlotState::MedAI as i32,
                old.max(num_medium + num_brutal),
            );
        }
        if num_brutal > 0 {
            let old = stats.get_endurance_medal(&map, SlotState::BrutalAI as i32);
            stats.set_endurance_medal(&map, SlotState::BrutalAI as i32, old.max(num_brutal));
        }
    }
}

fn update_challenge_medals(medals: &mut i32) {
    let setup = get_skirmish_setup();
    let game = setup.game_info().game_info();
    if !game.is_skirmish() {
        return;
    }
    let mut num_ais = 0;
    let mut num_brutals = 0;
    for i in 0..NETWORK_MAX_SLOTS {
        let Some(slot) = game.get_slot(i) else {
            continue;
        };
        if slot.is_ai() && !is_slot_local_ally(game, slot) {
            num_ais += 1;
            if slot.get_state() == SlotState::BrutalAI {
                num_brutals += 1;
            }
        } else if slot.is_ai() {
            return;
        }
    }
    if num_ais == 0 {
        return;
    }
    *medals |= match num_brutals {
        1 => BH_CHALLENGE_MASK_1,
        2 => BH_CHALLENGE_MASK_2,
        3 => BH_CHALLENGE_MASK_3,
        4 => BH_CHALLENGE_MASK_4,
        5 => BH_CHALLENGE_MASK_5,
        6 => BH_CHALLENGE_MASK_6,
        7 => BH_CHALLENGE_MASK_7,
        _ => 0,
    } as i32;
}

/// C++ ScoreScreen.cpp:2163-2164 — ALLIES vs ENEMIES only. Neutral omitted.
fn include_in_campaign_side_row(
    is_friend: bool,
    relationship: gamelogic::prelude::Relationship,
) -> bool {
    match relationship {
        gamelogic::prelude::Relationship::Allies => is_friend,
        gamelogic::prelude::Relationship::Enemies => !is_friend,
        gamelogic::prelude::Relationship::Neutral => false,
    }
}

/// Completed match advice owned by the receiving score-screen windows.
#[derive(Clone)]
pub struct ScoreScreenAcademySnapshot {
    pub end_frame: u32,
    pub show_advice: bool,
    pub advice: game_engine::common::rts::AcademyAdviceInfo,
}

pub(crate) fn bind_score_screen_snapshot(
    layout: &WindowLayout,
    snapshot: &ScoreScreenAcademySnapshot,
) {
    let parent_id = name_to_id("ScoreScreen.wnd:ParentScoreScreen");
    let advice_id = name_to_id("ScoreScreen.wnd:ListboxWarschoolAdvice");
    for root in layout.windows() {
        let mut targets = Vec::new();
        for id in [parent_id, advice_id] {
            if root.borrow().get_id() == id {
                targets.push(root.clone());
            } else if let Some(child) = root.borrow().find_child_by_id(id) {
                targets.push(child);
            }
        }
        for window in targets {
            window.borrow_mut().set_user_data(snapshot.clone());
        }
    }
}

/// Populate the actual receiving widgets from completed match data. This
/// remains valid after simulation reset has removed the legacy player roster.
fn populate_owned_academy_widgets(
    layout: &WindowLayout,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    let advice_id = name_to_id("ScoreScreen.wnd:ListboxWarschoolAdvice");
    let title_id = name_to_id("ScoreScreen.wnd:StaticTextWarSchool");
    let find = |id| {
        layout.windows().iter().find_map(|root| {
            if root.borrow().get_id() == id {
                Some(root.clone())
            } else {
                root.borrow().find_child_by_id(id)
            }
        })
    };
    let Some(listbox) = find(advice_id) else {
        return;
    };
    let Some(snapshot) = listbox
        .borrow()
        .get_user_data::<ScoreScreenAcademySnapshot>()
        .cloned()
    else {
        return;
    };
    let mut window = listbox.borrow_mut();
    let _ = window.hide(!snapshot.show_advice, receiving_manager);
    if let Some(widget) = window.list_box_mut() {
        widget.clear();
        if snapshot.show_advice {
            for key in snapshot
                .advice
                .advice
                .iter()
                .take(snapshot.advice.num_tips as usize)
            {
                if !key.is_empty() {
                    widget.add_item(&GameText::fetch(key));
                }
            }
        }
    }
    drop(window);
    if let Some(title) = find(title_id) {
        let _ = title
            .borrow_mut()
            .hide(!snapshot.show_advice, receiving_manager);
    }
}

/// Validate the completed native match before any ScoreScreen init side effect.
pub fn validate_completed_score_snapshot(layout: &WindowLayout) -> Result<(), String> {
    let parent_id = name_to_id("ScoreScreen.wnd:ParentScoreScreen");
    let parent = layout.windows().iter().find_map(|root| {
        if root.borrow().get_id() == parent_id {
            Some(root.clone())
        } else {
            root.borrow().find_child_by_id(parent_id)
        }
    });
    let bound = parent.as_ref().is_some_and(|parent| {
        parent
            .borrow()
            .get_user_data::<ScoreScreenAcademySnapshot>()
            .is_some()
    });
    if bound {
        Ok(())
    } else {
        Err("native ScoreScreen requires its receiving completed-match snapshot".into())
    }
}

fn score_end_frame(parent: &Option<Rc<RefCell<GameWindow>>>) -> u32 {
    parent
        .as_ref()
        .and_then(|window| {
            window
                .borrow()
                .get_user_data::<ScoreScreenAcademySnapshot>()
                .map(|snapshot| snapshot.end_frame)
        })
        .unwrap_or_else(leftover_score_logic_frame)
}

// The native admission requires a bound completed-match snapshot. The
// remaining standalone Core clock dependency is tracked separately (hq-6v2cq8).
fn leftover_score_logic_frame() -> u32 {
    crate::eva::eva_logic_frame()
}

fn leftover_academy_from_live_notify(player: &Player) -> gamelogic::player::AcademyStats {
    let mut stats = player.get_academy_stats().clone();
    let researched_radar = player.has_radar() || player.get_radar_count() > 0;
    let generals_points = i32::try_from(player.get_sciences().len()).unwrap_or(0);
    stats.apply_live_notify_snapshot(
        researched_radar,
        generals_points,
        0,
        player.get_num_battle_plans_active() > 0,
    );
    stats
}

fn fill_academy_advice(listbox: &Rc<RefCell<GameWindow>>, player: &Player) {
    if listbox
        .borrow()
        .get_user_data::<ScoreScreenAcademySnapshot>()
        .is_some()
    {
        return;
    }
    let mut info = game_engine::common::rts::AcademyAdviceInfo::default();
    let stats = leftover_academy_from_live_notify(player);
    if !stats.calculate_academy_advice(&mut info) {
        return;
    }
    let mut listbox_ref = listbox.borrow_mut();
    let Some(widget) = listbox_ref.list_box_mut() else {
        return;
    };
    for i in 0..info.num_tips.max(0) as usize {
        if let Some(tip) = info.advice.get(i) {
            if !tip.is_empty() {
                widget.add_item(&GameText::fetch(tip));
            }
        }
    }
}

fn populate_player_info(
    state: &mut ScoreScreenState,
    player: &Player,
    pos: i32,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }

    let color = player.get_player_color().to_argb_u32();
    let score = player.get_score_keeper();

    let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", pos);
    if let Some(win) = find_child(&state.parent, &name) {
        if state.override_player_display_name {
            set_text(&win, &GameText::fetch("GUI:Player"));
        } else {
            set_text(&win, player.get_player_display_name());
        }
        let _ = win.borrow_mut().hide(false, receiving_manager);
        set_text_color(&win, color);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", pos);
    hide_window(&find_child(&state.parent, &name), true, receiving_manager);

    let fields = [
        ("StaticTextUnitsBuilt", score.get_total_units_built()),
        ("StaticTextUnitsLost", score.get_total_units_lost()),
        (
            "StaticTextUnitsDestroyed",
            score.get_total_units_destroyed(),
        ),
        (
            "StaticTextBuildingsBuilt",
            score.get_total_buildings_built(),
        ),
        ("StaticTextBuildingsLost", score.get_total_buildings_lost()),
        (
            "StaticTextBuildingsDestroyed",
            score.get_total_buildings_destroyed(),
        ),
        ("StaticTextResources", score.get_total_money_earned()),
    ];

    for (field, value) in fields {
        let name = format!("ScoreScreen.wnd:{}{}", field, pos);
        if let Some(win) = find_child(&state.parent, &name) {
            set_text(&win, &format!("{}", value));
            set_text_color(&win, color);
            let _ = win.borrow_mut().hide(false, receiving_manager);
        }
    }
    if player.is_local_player() {
        if let Some(listbox) = state.listbox_academy.as_ref() {
            let _ = listbox.borrow_mut().hide(false, receiving_manager);
            hide_window(&state.static_text_academy_title, false, receiving_manager);
            if listbox
                .borrow()
                .get_user_data::<ScoreScreenAcademySnapshot>()
                .is_some()
                || TheGameLogic::is_in_skirmish_game()
                || TheGameLogic::is_in_multiplayer_game()
            {
                fill_academy_advice(listbox, player);
            }
        }
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", pos);
    if let Some(win) = find_child(&state.parent, &name) {
        let _ = win.borrow_mut().hide(false, receiving_manager);
        if let Some(template) = player.get_player_template() {
            set_window_image(&Some(win), template.get_side_icon_image());
        }
    }

    if state.screen_type == ScoreScreenType::Skirmish && player.is_local_player() {
        let setup = get_skirmish_setup();
        let sandbox = setup.game_info().game_info().is_sandbox();
        let victory = TheVictoryConditions::is_local_allied_victory();
        let defeat = is_local_allied_defeat_like_cpp(player);
        // C++ ScoreScreen.cpp:1568-1576
        if should_skip_skirmish_honor_persist(sandbox, defeat, victory, player.is_player_active()) {
            return;
        }

        let mut stats = SkirmishBattleHonors::new();
        if victory {
            stats.set_wins(stats.get_wins() + 1);
            stats.set_win_streak(stats.get_win_streak() + 1);
            let best = stats.get_best_win_streak().max(stats.get_win_streak());
            stats.set_best_win_streak(best);
            update_skirmish_battle_honors(&mut stats, player, score_end_frame(&state.parent));
            let mut challenge_medals = stats.get_challenge_medals();
            update_challenge_medals(&mut challenge_medals);
            stats.set_challenge_medals(challenge_medals);
        } else {
            stats.set_losses(stats.get_losses() + 1);
            stats.set_win_streak(0);
        }
        let last_general = stats.get_last_general();
        stats.set_last_general(player.get_side().to_string());
        if last_general == stats.get_last_general() {
            stats.set_num_games_loyal(stats.get_num_games_loyal() + 1);
        } else {
            stats.set_num_games_loyal(0);
        }
        let _ = stats.write();
    }
}

fn populate_side_info(
    state: &mut ScoreScreenState,
    side: &str,
    gather: &ScoreGather,
    pos: i32,
    color: u32,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }

    let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", pos);
    if let Some(win) = find_child(&state.parent, &name) {
        set_text(&win, side);
        set_text_color(&win, color);
        let _ = win.borrow_mut().hide(false, receiving_manager);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", pos);
    hide_window(&find_child(&state.parent, &name), true, receiving_manager);

    let fields = [
        ("StaticTextUnitsBuilt", gather.total_units_built),
        ("StaticTextUnitsLost", gather.total_units_lost),
        ("StaticTextUnitsDestroyed", gather.total_units_destroyed),
        ("StaticTextBuildingsBuilt", gather.total_buildings_built),
        ("StaticTextBuildingsLost", gather.total_buildings_lost),
        (
            "StaticTextBuildingsDestroyed",
            gather.total_buildings_destroyed,
        ),
        ("StaticTextResources", gather.total_money_earned),
    ];

    for (field, value) in fields {
        let name = format!("ScoreScreen.wnd:{}{}", field, pos);
        if let Some(win) = find_child(&state.parent, &name) {
            set_text(&win, &format!("{}", value));
            set_text_color(&win, color);
            let _ = win.borrow_mut().hide(false, receiving_manager);
        }
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", pos);
    let win = find_child(&state.parent, &name);
    if let Some(side_icon) = (!gather.side_icon.is_empty()).then_some(gather.side_icon.as_str()) {
        hide_window(&win, false, receiving_manager);
        set_window_image(&win, side_icon);
    }
}

pub fn score_screen_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
    effects: &mut WindowEventEffects<'_>,
) {
    {
        let value = true;
        effects.score_screen_mut().play_music = value
    };
    set_dont_show_main_menu(effects, true);
    {
        let value = false;
        effects.score_screen_mut().button_is_finish_campaign = value
    };

    {
        let value = name_to_id("ScoreScreen.wnd:ParentScoreScreen");
        effects.score_screen_mut().parent_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ButtonOk");
        effects.score_screen_mut().button_ok_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:TextEntryChat");
        effects.score_screen_mut().text_entry_chat_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ButtonEmote");
        effects.score_screen_mut().button_emote_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ListboxChatWindowScoreScreen");
        effects.score_screen_mut().listbox_chat_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ListboxWarschoolAdvice");
        effects.score_screen_mut().listbox_academy_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:StaticTextWarSchool");
        effects.score_screen_mut().static_text_academy_title_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ChatBoxBorder");
        effects.score_screen_mut().chat_box_border_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ButtonBuddy");
        effects.score_screen_mut().button_buddies_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ButtonContinue");
        effects.score_screen_mut().button_continue_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:ButtonSaveReplay");
        effects.score_screen_mut().button_save_replay_id = value
    };
    {
        let value = name_to_id("ScoreScreen.wnd:MainBackdrop");
        effects.score_screen_mut().backdrop_id = value
    };

    {
        let value = effects.with_manager(|manager, effects| {
            manager.get_window_by_id(effects.score_screen().parent_id)
        });
        effects.score_screen_mut().parent = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ButtonOk",
        );
        effects.score_screen_mut().button_ok = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:TextEntryChat",
        );
        effects.score_screen_mut().text_entry_chat = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ButtonEmote",
        );
        effects.score_screen_mut().button_emote = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ListboxChatWindowScoreScreen",
        );
        effects.score_screen_mut().listbox_chat = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ListboxWarschoolAdvice",
        );
        effects.score_screen_mut().listbox_academy = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:StaticTextWarSchool",
        );
        effects.score_screen_mut().static_text_academy_title = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ChatBoxBorder",
        );
        effects.score_screen_mut().chat_box_border = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ButtonContinue",
        );
        effects.score_screen_mut().button_continue = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ButtonBuddy",
        );
        effects.score_screen_mut().button_buddies = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:StaticTextGameSaveComplete",
        );
        effects.score_screen_mut().static_text_game_saved = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:MainBackdrop",
        );
        effects.score_screen_mut().backdrop = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:BigPortrait",
        );
        effects.score_screen_mut().challenge_portrait = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:ChallengeWinLossText",
        );
        effects.score_screen_mut().challenge_win_loss_text = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:GeneralRemarks",
        );
        effects.score_screen_mut().challenge_remarks = value
    };
    {
        let value = find_child(
            &effects.score_screen().parent.clone(),
            "ScoreScreen.wnd:GadgetParent",
        );
        effects.score_screen_mut().gadget_parent = value
    };

    {
        let parent = effects.score_screen().parent.clone();
        let button = find_child(&parent, "ScoreScreen.wnd:ButtonSaveReplay");
        {
            let state = effects.score_screen_mut();
            state.button_save_replay = button
        };
    }
    {
        let value = false;
        effects.score_screen_mut().override_player_display_name = value
    };
    {
        let value = String::new();
        effects.score_screen_mut().last_replay_filename = value
    };
    {
        let value = false;
        effects.score_screen_mut().can_save_replay = value
    };
    {
        let value = None;
        effects.score_screen_mut().pending_final_victory_movie = value
    };
    {
        let value = false;
        effects.score_screen_mut().waiting_for_final_victory_movie = value
    };

    if let Ok(recorder) = get_recorder().lock() {
        {
            let value = recorder.last_replay_filename().to_string();
            effects.score_screen_mut().last_replay_filename = value
        };
        {
            let value = recorder.get_mode() == RecorderMode::Record;
            effects.score_screen_mut().can_save_replay = value
        };
    }

    effects.with_manager(|manager, effects| {
        hide_window(
            &effects.score_screen().static_text_game_saved.clone(),
            true,
            manager,
        )
    });

    if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
        if let Some(button) = parent
            .borrow()
            .find_child_by_id(effects.score_screen().button_save_replay_id)
        {
            if let Ok(recorder) = get_recorder().lock() {
                if recorder.get_mode() == RecorderMode::None {
                    let _ = button.borrow_mut().enable(false);
                }
            }
        }
    }

    {
        let value = false;
        effects.score_screen_mut().need_finish_singleplayer_init = value
    };

    if TheGameLogic::is_in_replay_game() {
        if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
            if let Some(button) = parent
                .borrow()
                .find_child_by_id(effects.score_screen().button_save_replay_id)
            {
                let _ = effects
                    .with_manager(|manager, effects| button.borrow_mut().hide(true, manager));
            }
        }
        if let Ok(recorder) = get_recorder().lock() {
            if recorder.is_multiplayer() {
                effects.with_manager(|manager, effects| {
                    init_replay_multiplayer(effects.score_screen_mut(), manager)
                });
            } else {
                {
                    let value = true;
                    effects.score_screen_mut().override_player_display_name = value
                };
                effects.with_manager(|manager, effects| {
                    init_replay_single_player(effects.score_screen_mut(), manager)
                });
            }
        }
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("ScoreScreenShow", false, None)
        });
    } else if TheGameLogic::get_game_mode() == GAME_INTERNET {
        effects.with_manager(|manager, effects| {
            init_internet_multiplayer(effects.score_screen_mut(), manager)
        });
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("ScoreScreenShow", false, None)
        });
    } else if TheGameLogic::get_game_mode() == GAME_LAN {
        effects.with_manager(|manager, effects| {
            init_lan_multiplayer(effects.score_screen_mut(), manager)
        });
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("ScoreScreenShow", false, None)
        });
    } else if TheGameLogic::get_game_mode() == GAME_SKIRMISH {
        effects.with_manager(|manager, effects| init_skirmish(effects.score_screen_mut(), manager));
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("ScoreScreenShow", false, None)
        });
    } else {
        {
            let value = true;
            effects.score_screen_mut().override_player_display_name = value
        };
        init_single_player(effects);
        if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
            if let Some(button) = parent
                .borrow()
                .find_child_by_id(effects.score_screen().button_save_replay_id)
            {
                let _ = effects
                    .with_manager(|manager, effects| button.borrow_mut().hide(true, manager));
            }
        }
    }

    if effects
        .campaign_manager
        .as_deref()
        .and_then(CampaignManager::get_current_campaign)
        .is_some_and(Campaign::is_challenge_campaign)
    {
        if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
            if let Some(button) = parent
                .borrow()
                .find_child_by_id(effects.score_screen().button_save_replay_id)
            {
                let _ = button.borrow_mut().enable(false);
                let _ = effects
                    .with_manager(|manager, effects| button.borrow_mut().hide(true, manager));
            }
        }
    }

    effects.with_manager(|manager, effects| layout.hide(manager, false));
    if let Some(parent) = effects.score_screen().parent.clone().as_ref() {
        let _ = effects.with_manager(|manager, effects| manager.activate_window(parent));
    }
    set_replay_was_pressed(effects, false);
    effects.with_manager(|manager, effects| populate_owned_academy_widgets(layout, manager));
}

pub fn score_screen_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
    effects: &mut WindowEventEffects<'_>,
) {
    if let Some(popup) = effects.score_screen().popup_replay_layout.clone().as_ref() {
        if !popup.borrow().is_hidden() {
            let popup_ref = popup.borrow();
            effects.with_manager(|receiving_manager, effects| {
                popup_replay_update(&popup_ref, None, receiving_manager, effects)
            });
        }
    }

    let completed_init = effects.score_screen().need_finish_singleplayer_init;
    if completed_init {
        finish_single_player_init(effects);
        {
            let value = false;
            effects.score_screen_mut().need_finish_singleplayer_init = value
        };
    }

    update_final_victory_movie_wait(effects);
    update_score_screen_music(effects);

    if completed_init {
        effects.with_manager(|manager, effects| populate_owned_academy_widgets(layout, manager));
    }
}

pub fn score_screen_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    set_dont_show_main_menu(effects, false);

    effects.with_manager(|manager, _effects| layout.hide(manager, true));
    queue_shell_shutdown_complete(effects, false);

    if let Some(audio) = TheAudio::get() {
        audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
    }
}

pub fn score_screen_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        let button_ok = name_to_id("ScoreScreen.wnd:ButtonOk") as u32;
        window.send_system_message(
            WindowMessage::GadgetSelected,
            button_ok as WindowMsgData,
            0,
            effects,
        );
    }

    WindowMsgHandled::Handled
}

pub fn score_screen_system(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    // C++ focus/teardown notifications do not touch persistent score state.
    // They may re-enter synchronously during a transition below.
    if msg == WindowMessage::Destroy {
        return WindowMsgHandled::Handled;
    }
    if msg == WindowMessage::InputFocus {
        return write_input_focus_response(data1, data2, true);
    }
    match msg {
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            effects.with_manager(|manager, effects| {
                manager.transition_remove("ScoreScreenShow", true, Some(&mut *_window))
            });
            set_replay_was_pressed(effects, false);

            if control_id == effects.score_screen().button_ok_id {
                queue_shell_pop(effects);
                if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                    manager.set_campaign("");
                }
            } else if control_id == effects.score_screen().button_continue_id {
                if !effects.score_screen().button_is_finish_campaign {
                    set_replay_was_pressed(effects, true);
                }
                if effects.score_screen().screen_type == ScoreScreenType::SinglePlayer {
                    let map_name = effects
                        .campaign_manager
                        .as_deref()
                        .and_then(CampaignManager::get_current_map)
                        .unwrap_or_default();
                    if map_name.is_empty() {
                        set_replay_was_pressed(effects, false);
                        queue_shell_pop(effects);
                    } else {
                        // C++ CheckForCDAtGameStart(startNextCampaignGame)
                        check_for_cd_at_game_start(Box::new(start_next_campaign_game), effects);
                    }
                }
            } else if control_id == effects.score_screen().button_save_replay_id {
                score_screen_enable_controls(false, effects);
                let layout = if let Some(layout) =
                    effects.score_screen().popup_replay_layout.clone().as_ref()
                {
                    layout.clone()
                } else {
                    let Some((layout, _)) = effects.with_manager(|manager, effects| {
                        manager
                            .create_layout_with_windows("Menus/PopupReplay.wnd", effects)
                            .ok()
                    }) else {
                        return WindowMsgHandled::Handled;
                    };
                    {
                        let value = Some(layout.clone());
                        effects.score_screen_mut().popup_replay_layout = value
                    };
                    layout
                };
                layout.borrow().run_init_with_effects(None, effects);
                effects.with_manager(|manager, effects| layout.borrow_mut().hide(manager, false));
                effects.with_manager(|manager, effects| layout.borrow_mut().bring_forward(manager));
            }

            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == effects.score_screen().text_entry_chat_id {
                if let Some(entry) = effects.score_screen().text_entry_chat.clone().as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        widget.set_text(String::new());
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign_with_final_movie(is_challenge_campaign: bool, movie: &str) -> Campaign {
        let mut campaign = Campaign::new();
        campaign.is_challenge_campaign = is_challenge_campaign;
        campaign.final_movie_name = movie.to_string();
        campaign
    }

    #[test]
    fn mission_save_description_formats_campaign_and_number_like_cpp() {
        // C++ GameState.cpp:616-618 UnicodeString::format(GUI:MissionSave, label, n).
        assert_eq!(
            format_mission_save_description("%s Mission %d", "USA Campaign", 2),
            "USA Campaign Mission 2"
        );
        assert_eq!(
            format_mission_save_description("GUI:AutoSave", "China", 1),
            "GUI:AutoSave China 1"
        );
        let desc = current_mission_save_description();
        assert!(
            !desc.contains("GUI:AutoSave") || desc.contains('1'),
            "between-mission save must not be bare GUI:AutoSave: {desc}"
        );
    }

    #[test]
    fn final_victory_movie_includes_challenge_campaigns_like_cpp() {
        let campaign = campaign_with_final_movie(true, "USACampaignVictory");

        assert_eq!(
            final_victory_movie_to_queue(&campaign, false),
            Some("USACampaignVictory".to_string())
        );
    }

    #[test]
    fn final_victory_movie_respects_empty_and_low_res_cases() {
        let empty = campaign_with_final_movie(true, "");
        let normal = campaign_with_final_movie(false, "ChinaCampaignVictory");

        assert_eq!(final_victory_movie_to_queue(&empty, false), None);
        assert_eq!(final_victory_movie_to_queue(&normal, true), None);
    }

    #[test]
    fn esc_char_is_consumed_before_key_up_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            score_screen_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            score_screen_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn score_screen_system_consumes_destroy_like_cpp() {
        let window = GameWindow::new();

        let mut emitted_messages = Vec::new();
        let mut effects = WindowEventEffects {
            control_bar_observation: None,
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: None,
            ime: None,
            manager: None,
            script_display: None,
            window_videos: None,
            window_manager_owner: None,
            selected_window: None,
            emitted_messages: &mut emitted_messages,
            pending_control_bar_clicks: None,
            campaign_manager: None,
            main_menu: None,
            menu_world: None,
            menu_host: None,
            pending_main_menu_actions: Vec::new(),
        };
        assert_eq!(
            score_screen_system(&window, WindowMessage::Destroy, 0, 0, &mut effects),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn start_next_campaign_game_appends_msg_new_game_like_cpp() {
        let mut campaign_manager = CampaignManager::new();
        let campaign = campaign_manager.new_campaign("testcampaign".to_string());
        campaign.first_mission = "m1".to_string();
        campaign.new_mission("m1".to_string()).map_name = "maps/m1.map".to_string();
        campaign_manager.set_campaign("testcampaign");
        let mut emitted_messages = Vec::new();
        let mut effects = WindowEventEffects {
            control_bar_observation: None,
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: None,
            ime: None,
            manager: None,
            script_display: None,
            window_videos: None,
            window_manager_owner: None,
            selected_window: None,
            emitted_messages: &mut emitted_messages,
            pending_control_bar_clicks: None,
            campaign_manager: Some(&mut campaign_manager),
            main_menu: None,
            menu_world: None,
            menu_host: None,
            pending_main_menu_actions: Vec::new(),
        };
        start_next_campaign_game(&mut effects);
        assert!(
            emitted_messages
                .iter()
                .any(|message| message.get_type() == &GameMessageType::NewGame),
            "C++ startNextCampaignGame appends MSG_NEW_GAME, not prepare_new_game"
        );
    }

    #[test]
    fn set_text_color_preserves_enabled_border_color_like_cpp() {
        let window = Rc::new(RefCell::new(GameWindow::new()));
        window
            .borrow_mut()
            .set_enabled_text_colors(0x11223344, 0x55667788);

        set_text_color(&window, 0xaabbccdd);

        let window = window.borrow();
        assert_eq!(window.get_enabled_text_color(), 0xaabbccdd);
        assert_eq!(window.get_enabled_text_border_color(), 0x55667788);
    }

    #[test]
    fn score_screen_music_uses_cpp_fade_stop_sentinel() {
        assert_eq!(AHSV_STOP_THE_MUSIC_FADE, 0xFFFF_FFF1);
    }

    #[test]
    fn next_campaign_leaves_score_screen_with_immediate_pop_like_cpp() {
        #[derive(Default)]
        struct TestShellActions {
            events: Vec<&'static str>,
        }

        impl NextCampaignShellActions for TestShellActions {
            fn pop_score_screen_immediate(&mut self) {
                self.events.push("pop_immediate");
            }

            fn hide_shell_for_next_campaign(&mut self) {
                self.events.push("hide_shell");
            }
        }

        let mut shell = TestShellActions::default();

        leave_score_screen_for_next_campaign(&mut shell);

        assert_eq!(shell.events, ["pop_immediate", "hide_shell"]);
    }

    #[test]
    fn skirmish_honor_persist_skip_matches_cpp_sandbox_and_defeat() {
        // Sandbox or undecided + still active → skip.
        assert!(should_skip_skirmish_honor_persist(true, false, true, true));
        assert!(should_skip_skirmish_honor_persist(
            false, false, false, true
        ));
        // Finished allied defeat while still watching AI → persist loss.
        assert!(!should_skip_skirmish_honor_persist(
            false, true, false, true
        ));
        // Dead local player always persists (even sandbox / undecided).
        assert!(!should_skip_skirmish_honor_persist(
            true, false, false, false
        ));
        // Real victory (not sandbox) persists.
        assert!(!should_skip_skirmish_honor_persist(
            false, false, true, true
        ));
    }

    #[test]
    fn campaign_side_row_skips_neutral_like_cpp() {
        use gamelogic::prelude::Relationship;
        assert!(include_in_campaign_side_row(true, Relationship::Allies));
        assert!(!include_in_campaign_side_row(true, Relationship::Enemies));
        assert!(!include_in_campaign_side_row(true, Relationship::Neutral));
        assert!(include_in_campaign_side_row(false, Relationship::Enemies));
        assert!(!include_in_campaign_side_row(false, Relationship::Allies));
        assert!(!include_in_campaign_side_row(false, Relationship::Neutral));
    }

    #[test]
    fn skirmish_blitz_honors_the_bound_completed_frame() {
        fn bound(frame: u32) -> Option<Rc<RefCell<GameWindow>>> {
            let mut window = GameWindow::new();
            window.set_user_data(ScoreScreenAcademySnapshot {
                end_frame: frame,
                show_advice: false,
                advice: Default::default(),
            });
            Some(Rc::new(RefCell::new(window)))
        }
        assert_eq!(score_end_frame(&bound(0)), 0);
        assert_eq!(score_end_frame(&bound(18_000)), 18_000);
        let minutes = score_end_frame(&bound(18_000)) / LOGICFRAMES_PER_SECOND / 60;
        assert_eq!(minutes, 10);
        assert!(minutes >= 5, "10-minute match must not award Blitz5");
        assert!(minutes >= 10, "10-minute match must not award Blitz10");
        let minutes = score_end_frame(&bound(8_999)) / LOGICFRAMES_PER_SECOND / 60;
        assert!(minutes < 5);
        assert!(minutes < 10);
    }

    #[test]
    fn war_school_academy_advice_is_fed_from_leftover() {
        let mut stats = gamelogic::player::AcademyStats::new();
        let mut info = game_engine::common::rts::AcademyAdviceInfo::default();
        assert!(stats.calculate_academy_advice(&mut info));
        assert_eq!(
            info.advice.first().map(String::as_str),
            Some("ACADEMY:TryBuildingRadar")
        );

        stats.apply_live_notify_snapshot(true, 3, 1, true);
        info = game_engine::common::rts::AcademyAdviceInfo::default();
        assert!(stats.calculate_academy_advice(&mut info));
        assert_ne!(
            info.advice.first().map(String::as_str),
            Some("ACADEMY:TryBuildingRadar")
        );
    }
}

/// Residual: last ScoreScreen action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualScoreScreenAction {
    None = 0,
    Ok = 1,
    Continue = 2,
    SaveReplay = 3,
    Buddy = 4,
    Emote = 5,
}

pub fn residual_score_screen_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualScoreScreenAction {
    {
        let state = effects.score_screen_mut();
        match state.residual_action {
            1 => ResidualScoreScreenAction::Ok,
            2 => ResidualScoreScreenAction::Continue,
            3 => ResidualScoreScreenAction::SaveReplay,
            4 => ResidualScoreScreenAction::Buddy,
            5 => ResidualScoreScreenAction::Emote,
            _ => ResidualScoreScreenAction::None,
        }
    }
}
pub fn residual_score_screen_is_finish_campaign(effects: &mut WindowEventEffects<'_>) -> bool {
    {
        let state = effects.score_screen_mut();
        state.button_is_finish_campaign
    }
}

fn ensure_score_screen_control_ids(state: &mut ScoreScreenState) {
    if state.parent_id == 0 {
        state.parent_id = name_to_id("ScoreScreen.wnd:ParentScoreScreen");
    }
    if state.button_ok_id == 0 {
        state.button_ok_id = name_to_id("ScoreScreen.wnd:ButtonOk");
    }
    if state.button_continue_id == 0 {
        state.button_continue_id = name_to_id("ScoreScreen.wnd:ButtonContinue");
    }
    if state.button_save_replay_id == 0 {
        state.button_save_replay_id = name_to_id("ScoreScreen.wnd:ButtonSaveReplay");
    }
    if state.button_buddies_id == 0 {
        state.button_buddies_id = name_to_id("ScoreScreen.wnd:ButtonBuddy");
    }
    if state.button_emote_id == 0 {
        state.button_emote_id = name_to_id("ScoreScreen.wnd:ButtonEmote");
    }
    if state.text_entry_chat_id == 0 {
        state.text_entry_chat_id = name_to_id("ScoreScreen.wnd:TextEntryChat");
    }
    if state.listbox_chat_id == 0 {
        state.listbox_chat_id = name_to_id("ScoreScreen.wnd:ListboxChatWindowScoreScreen");
    }
    if state.backdrop_id == 0 {
        state.backdrop_id = name_to_id("ScoreScreen.wnd:MainBackdrop");
    }
}

/// Residual: bind ScoreScreen control IDs (no layout load required).
pub fn simulate_score_screen_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        let _ = (
            state.parent_id,
            state.button_ok_id,
            state.button_continue_id,
            state.button_save_replay_id,
            state.button_buddies_id,
            state.button_emote_id,
        );
        true
    }
}

/// Residual: mark Continue as end-of-campaign finish (no next map).
pub fn simulate_score_screen_set_finish_campaign(
    finish: bool,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.button_is_finish_campaign = finish;

        state.button_is_finish_campaign == finish
    }
}

/// Residual: fire ButtonOk without shell pop / campaign clear.
pub fn simulate_score_screen_ok_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.residual_action = ResidualScoreScreenAction::Ok as u8;
        true
    }
}

/// Residual: fire ButtonContinue without next-map start / shell pop.
pub fn simulate_score_screen_continue_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.residual_action = ResidualScoreScreenAction::Continue as u8;
        true
    }
}

/// Residual: fire ButtonSaveReplay without PopupReplay layout create.
pub fn simulate_score_screen_save_replay_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.residual_action = ResidualScoreScreenAction::SaveReplay as u8;
        true
    }
}

/// Residual: fire ButtonBuddy without buddy overlay.
pub fn simulate_score_screen_buddy_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.residual_action = ResidualScoreScreenAction::Buddy as u8;
        true
    }
}

/// Residual: fire ButtonEmote without chat stream.
pub fn simulate_score_screen_emote_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    {
        let state = effects.score_screen_mut();
        ensure_score_screen_control_ids(state);
        state.residual_action = ResidualScoreScreenAction::Emote as u8;
        true
    }
}

/// Residual: finish-campaign + Continue composite (end match honesty).
pub fn simulate_score_screen_prepare_finish(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_score_screen_bind_controls(effects) {
        return false;
    }
    if !simulate_score_screen_set_finish_campaign(true, effects) {
        return false;
    }
    simulate_score_screen_continue_button_gadget_selected(effects)
}

/// Residual: Ok path composite (return to shell honesty).
pub fn simulate_score_screen_prepare_ok(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_score_screen_bind_controls(effects) {
        return false;
    }
    simulate_score_screen_ok_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonContinue`
/// (C++ WindowXlat hit → GBM_SELECTED → CheckForCDAtGameStart(startNextCampaignGame)).
/// Not `simulate_*` first.
pub fn drive_os_wnd_score_screen_continue_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonContinue", effects);
    if !clicked {
        return false;
    }
    if !simulate_score_screen_continue_button_gadget_selected(effects) {
        return false;
    }
    // C++ ScoreScreen.cpp ButtonContinue: single-player + next map starts the
    // next mission. Finish-campaign residual skips startNextCampaignGame.
    let start_next = {
        let state = effects.score_screen_mut();
        state.screen_type == ScoreScreenType::SinglePlayer && !state.button_is_finish_campaign
    } && effects
        .campaign_manager
        .as_deref()
        .and_then(CampaignManager::get_current_map)
        .is_some_and(|map| !map.is_empty());
    if start_next {
        check_for_cd_at_game_start(Box::new(start_next_campaign_game), effects);
    }
    true
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonOk`
/// (C++ WindowXlat hit → GBM_SELECTED → shell pop + clear campaign).
pub fn drive_os_wnd_score_screen_ok_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonOk", effects);
    if !clicked {
        return false;
    }
    simulate_score_screen_ok_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonSaveReplay`.
pub fn drive_os_wnd_score_screen_save_replay_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonSaveReplay", effects);
    if !clicked {
        return false;
    }
    simulate_score_screen_save_replay_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonBuddy`.
pub fn drive_os_wnd_score_screen_buddy_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonBuddy", effects);
    if !clicked {
        return false;
    }
    simulate_score_screen_buddy_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonEmote`.
pub fn drive_os_wnd_score_screen_emote_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonEmote", effects);
    if !clicked {
        return false;
    }
    simulate_score_screen_emote_button_gadget_selected(effects)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_score_screen_continue_hits_button_then_latches_continue() {
        install_named_button("ScoreScreen.wnd:ButtonContinue", 10, 10);
        let _ = simulate_score_screen_set_finish_campaign(false);
        assert!(
            drive_os_wnd_score_screen_continue_like_cpp(),
            "OS WND click on ButtonContinue must latch continue residual"
        );
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Continue
        );
        assert!(!drive_os_wnd_score_screen_ok_like_cpp());
    }

    #[test]
    fn os_wnd_score_screen_ok_hits_button_then_latches_ok() {
        install_named_button("ScoreScreen.wnd:ButtonOk", 10, 40);
        assert!(
            drive_os_wnd_score_screen_ok_like_cpp(),
            "OS WND click on ButtonOk must latch ok residual"
        );
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Ok
        );
    }

    #[test]
    fn os_wnd_score_screen_continue_starts_next_usa_mission_like_cpp() {
        let mut campaign_manager = CampaignManager::new();
        campaign_manager.init();
        campaign_manager.set_campaign("USA");
        // Victory → next mission is already current when ScoreScreen Continue fires.
        let _ = campaign_manager.goto_next_mission();
        let current_map = campaign_manager.get_current_map().unwrap_or_default();
        install_named_button("ScoreScreen.wnd:ButtonContinue", 10, 70);
        let _ = simulate_score_screen_set_finish_campaign(false);
        let mut emitted_messages = Vec::new();
        let mut effects = WindowEventEffects {
            control_bar_observation: None,
            input_provenance:
                crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            standalone_view: None,
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: None,
            menus: None,
            ime: None,
            manager: None,
            script_display: None,
            window_videos: None,
            selected_window: None,
            window_manager_owner: None,
            emitted_messages: &mut emitted_messages,
            pending_control_bar_clicks: None,
            campaign_manager: None,
            main_menu: None,
            menu_world: None,
            menu_host: None,
            pending_main_menu_actions: Vec::new(),
        };
        assert!(drive_os_wnd_score_screen_continue_like_cpp(&mut effects));
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Continue
        );
        if current_map.is_empty() {
            return;
        }
        // The effect-aware click path appends to the receiving message stream; the production
        // callback emitter is exercised by the owning client input dispatch test.
        if let Some(data) = game_engine::common::ini::get_global_data() {
            let pending = data.read().pending_file.clone();
            if !pending.is_empty() {
                assert!(
                    pending.eq_ignore_ascii_case(&current_map)
                        || pending.to_ascii_lowercase().contains("md_usa"),
                    "next-campaign pending file follows Campaign.ini USA chain"
                );
            }
        }
    }
}

#[cfg(test)]
mod owned_academy_snapshot_tests {
    use super::*;

    #[test]
    fn actual_receiving_layouts_keep_independent_score_advice_and_end_frames() {
        let make = |frame: u32, key: &str| {
            let parent = Rc::new(RefCell::new(GameWindow::new()));
            parent
                .borrow_mut()
                .set_id(name_to_id("ScoreScreen.wnd:ParentScoreScreen"));
            let listbox = Rc::new(RefCell::new(GameWindow::new()));
            listbox
                .borrow_mut()
                .set_id(name_to_id("ScoreScreen.wnd:ListboxWarschoolAdvice"));
            listbox
                .borrow_mut()
                .set_widget(crate::gui::game_window::WindowWidget::ListBox(
                    crate::gui::gadgets::ListBox::new(42, 0, 0, 200, 100),
                ));
            let mut layout = WindowLayout::new("Menus/ScoreScreen.wnd".into());
            layout.add_window(parent.clone());
            layout.add_window(listbox.clone());
            let mut advice = game_engine::common::rts::AcademyAdviceInfo::default();
            advice.num_tips = 1;
            advice.advice[0] = key.into();
            bind_score_screen_snapshot(
                &layout,
                &ScoreScreenAcademySnapshot {
                    end_frame: frame,
                    show_advice: true,
                    advice,
                },
            );
            // Same callback helper used after real ScoreScreen initialization;
            // no PlayerList or Core mode is published by this fixture.
            populate_owned_academy_widgets(&layout);
            (layout, parent, listbox)
        };
        let (first_layout, first, first_list) = make(18000, "GUI:FirstAdvice");
        let (second_layout, second, second_list) = make(700, "GUI:SecondAdvice");
        assert_eq!(score_end_frame(&Some(first)), 18000);
        assert_eq!(score_end_frame(&Some(second)), 700);
        assert_eq!(
            first_list
                .borrow()
                .get_user_data::<ScoreScreenAcademySnapshot>()
                .unwrap()
                .advice
                .advice[0],
            "GUI:FirstAdvice"
        );
        assert_eq!(
            second_list
                .borrow()
                .get_user_data::<ScoreScreenAcademySnapshot>()
                .unwrap()
                .advice
                .advice[0],
            "GUI:SecondAdvice"
        );
        assert_eq!(
            first_list.borrow_mut().list_box_mut().unwrap().items()[0].text,
            GameText::fetch("GUI:FirstAdvice")
        );
        assert_eq!(
            second_list.borrow_mut().list_box_mut().unwrap().items()[0].text,
            GameText::fetch("GUI:SecondAdvice")
        );
        // Re-initialization is replacement, not duplicate append.
        populate_owned_academy_widgets(&first_layout);
        assert_eq!(
            first_list
                .borrow_mut()
                .list_box_mut()
                .unwrap()
                .items()
                .len(),
            1
        );
        let mut hidden = second_list
            .borrow()
            .get_user_data::<ScoreScreenAcademySnapshot>()
            .unwrap()
            .clone();
        hidden.show_advice = false;
        bind_score_screen_snapshot(&second_layout, &hidden);
        populate_owned_academy_widgets(&second_layout);
        assert!(
            second_list
                .borrow_mut()
                .list_box_mut()
                .unwrap()
                .items()
                .is_empty()
        );
        assert!(second_list.borrow().is_hidden());
        assert_eq!(
            first_list
                .borrow_mut()
                .list_box_mut()
                .unwrap()
                .items()
                .len(),
            1
        );
        drop(second_layout);
        assert_eq!(
            first_list
                .borrow()
                .get_user_data::<ScoreScreenAcademySnapshot>()
                .unwrap()
                .end_frame,
            18000
        );
        drop(first_layout);
    }
}
