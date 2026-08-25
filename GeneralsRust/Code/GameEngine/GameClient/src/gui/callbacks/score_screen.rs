//! ScoreScreen.cpp callback port.

use crate::cd_check::check_for_cd_at_game_start;
use crate::core::script_action_handler::{
    is_script_display_movie_playing, play_script_display_movie,
};
use crate::game_text::GameText;
use crate::gui::callbacks::popup_replay::popup_replay_update;
use crate::gui::campaign_launch_host_bridge::{
    HostCampaignLaunchDescriptor, publish_host_campaign_launch,
};
use crate::gui::campaign_manager::{
    Campaign, GameDifficulty as CampaignDifficulty, get_campaign_manager,
};
use crate::gui::challenge_generals::get_challenge_generals;
use crate::gui::get_skirmish_setup;
use crate::gui::menu_flags::{set_dont_show_main_menu, set_replay_was_pressed};
use crate::gui::shell::Shell;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
    queue_shell_operation, queue_shell_pop, queue_shell_shutdown_complete,
    show_shell_map_if_available, with_window_manager, write_input_focus_response,
};
use crate::message_stream::{GameMessageType, get_message_stream};
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
use std::cell::{Cell, RefCell};
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
struct ScoreScreenState {
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

thread_local! {
    static SCORE_SCREEN_STATE: RefCell<ScoreScreenState> = RefCell::new(ScoreScreenState::default());
}

fn with_score_screen_state<R>(f: impl FnOnce(&mut ScoreScreenState) -> R) -> R {
    SCORE_SCREEN_STATE.with(|state| {
        let mut state = state.borrow_mut();
        f(&mut state)
    })
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

fn hide_window(win: &Option<Rc<RefCell<GameWindow>>>, hide: bool) {
    if let Some(win) = win {
        let _ = win.borrow_mut().hide(hide);
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

fn score_screen_music_from_game_info_slot() -> String {
    // C++ ScoreScreenUpdate: TheGameInfo slot template, FactionObserver if < 0.
    // Must not touch PlayerList — clearGameData runs before the first Update.
    game_engine::common::ini::ensure_player_templates_loaded();
    let store = game_engine::common::rts::player_template::get_player_template_store();
    let template_index = score_screen_local_slot_template_index();
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

fn score_screen_local_slot_template_index() -> i32 {
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
    crate::gui::challenge_game_info::with_challenge_game_info(|info| from_info(info.game_info()))
        .flatten()
        .unwrap_or(-1)
}

fn update_score_screen_music(state: &mut ScoreScreenState) {
    if !state.play_music {
        return;
    }

    state.play_music = false;

    let Some(audio) = TheAudio::get() else {
        return;
    };

    let music_name = score_screen_music_from_game_info_slot();
    if !music_name.is_empty() {
        audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        let mut event = gamelogic::common::audio::AudioEventRts::new(music_name);
        event.set_should_fade(true);
        let _ = audio.add_audio_event(&event);
        audio.update();
    }
}

pub fn score_screen_enable_controls(enable: bool) {
    with_score_screen_state(|state| {
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

        if let Some(parent) = state.parent.as_ref() {
            let button = parent
                .borrow()
                .find_child_by_id(state.button_save_replay_id);
            if let Some(button) = button {
                if !button.borrow().is_hidden() {
                    let mut should_enable = enable;
                    if !state.can_save_replay {
                        should_enable = false;
                    }
                    let _ = button.borrow_mut().enable(should_enable);
                }
            }
        }
    });
}

trait NextCampaignShellActions {
    fn pop_score_screen_immediate(&mut self);
    fn hide_shell_for_next_campaign(&mut self);
}

impl NextCampaignShellActions for Shell {
    fn pop_score_screen_immediate(&mut self) {
        let _ = self.pop_immediate();
    }

    fn hide_shell_for_next_campaign(&mut self) {
        let _ = self.hide_shell();
    }
}

fn leave_score_screen_for_next_campaign(shell: &mut impl NextCampaignShellActions) {
    shell.pop_score_screen_immediate();
    shell.hide_shell_for_next_campaign();
}

fn start_next_campaign_game() {
    // C++ ScoreScreen.cpp startNextCampaignGame.
    queue_shell_operation(|shell| {
        leave_score_screen_for_next_campaign(shell);
    });

    let (pending_file, campaign_name, campaign_player_faction, is_challenge) = {
        let manager = get_campaign_manager();
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
    if is_challenge {
        // C++ rematch: init/clearSlotList/reset/enterGame then set map+slot0.
        crate::gui::challenge_game_info::init_challenge_game_info();
        let template_num = get_challenge_generals()
            .and_then(|m| m.lock().ok().map(|g| g.current_player_template_num()))
            .unwrap_or_else(|| {
                get_campaign_manager().get_xfer_challenge_generals_player_template_num()
            });
        crate::gui::challenge_game_info::set_challenge_slot0_and_map(
            pending_file.clone(),
            String::new(),
            template_num,
        );
        challenge_template_num = Some(template_num);
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }
    }

    if let Some(data) = game_engine::common::ini::get_global_data() {
        let mut data = data.write();
        data.pending_file = pending_file.clone();
    }
    game_engine::common::global_data::write().pending_file = pending_file.clone();

    let manager = get_campaign_manager();
    let difficulty = manager.get_game_difficulty() as i32;
    let rank_points = manager.get_rank_points();
    drop(manager);

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
    let _ = publish_host_campaign_launch(HostCampaignLaunchDescriptor {
        generation: 0,
        map_name: pending_file,
        campaign_name,
        campaign_player_faction,
        is_challenge,
        player_template_name: challenge_template_name,
        player_template_index: challenge_template_num,
        game_mode_code: GAME_SINGLE_PLAYER,
        difficulty_code: difficulty,
        rank_points,
        max_fps: None,
    });

    let message_stream = get_message_stream();
    let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
    let msg = stream.append_message(GameMessageType::NewGame);
    msg.append_integer_argument(GAME_SINGLE_PLAYER);
    msg.append_integer_argument(difficulty);
    msg.append_integer_argument(rank_points);
    init_random_with_seed(0);
}

fn init_single_player(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::SinglePlayer;
    if let Ok(list) = ThePlayerList().read() {
        if let Some(local_player) = list.get_local_player() {
            if let Ok(player) = local_player.read() {
                let mut manager = get_campaign_manager();
                manager.set_rank_points(player.get_skill_points());
                manager.set_game_difficulty(match TheScriptEngine::get_global_difficulty() {
                    0 => CampaignDifficulty::Easy,
                    2 => CampaignDifficulty::Hard,
                    _ => CampaignDifficulty::Normal,
                });
            }
        }
    }
    grab_single_player_info(state);
    state.need_finish_singleplayer_init = true;

    let blank_layout =
        with_window_manager(|manager| manager.create_layout("Menus/BlankWindow.wnd".to_string()));
    blank_layout.borrow_mut().hide(false);
    blank_layout.borrow_mut().bring_forward();
    // C++ initSinglePlayer: first window winClearStatus(WIN_STATUS_IMAGE).
    if let Some(first) = blank_layout.borrow().get_first_window() {
        first.borrow_mut().clear_status(WindowStatus::IMAGE);
    }
    state.blank_layout = Some(blank_layout);
}

fn init_skirmish(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::Skirmish;
    grab_multi_player_info(state);
    hide_window(&state.text_entry_chat, true);
    hide_window(&state.button_emote, true);
    hide_window(&state.chat_box_border, true);
    hide_window(&state.button_buddies, true);
    hide_window(&state.button_continue, true);
    hide_window(&state.listbox_chat, true);
    hide_window(&state.static_text_game_saved, true);
}

fn init_lan_multiplayer(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::Lan;
    grab_multi_player_info(state);
    if let Some(text_entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text(String::new());
        }
    }
    hide_window(&state.static_text_game_saved, true);
    hide_window(&state.text_entry_chat, false);
    hide_window(&state.button_emote, false);
    hide_window(&state.button_continue, true);
    hide_window(&state.listbox_chat, false);
    hide_window(&state.listbox_academy, true);
    hide_window(&state.static_text_academy_title, true);
    hide_window(&state.chat_box_border, false);
    hide_window(&state.button_buddies, true);
}

fn init_internet_multiplayer(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::Internet;
    grab_multi_player_info(state);
    if let Some(text_entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text(String::new());
        }
    }
    hide_window(&state.static_text_game_saved, true);
    hide_window(&state.button_continue, true);
    hide_window(&state.text_entry_chat, true);
    hide_window(&state.button_emote, true);
    hide_window(&state.listbox_chat, false);
    hide_window(&state.listbox_academy, false);
    hide_window(&state.static_text_academy_title, false);
    hide_window(&state.chat_box_border, false);
    hide_window(&state.button_buddies, false);
}

fn init_replay_multiplayer(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::Replay;
    grab_multi_player_info(state);
    hide_window(&state.static_text_game_saved, true);
    hide_window(&state.text_entry_chat, true);
    hide_window(&state.button_emote, true);
    hide_window(&state.listbox_chat, true);
    hide_window(&state.listbox_academy, true);
    hide_window(&state.static_text_academy_title, true);
    hide_window(&state.chat_box_border, true);
    hide_window(&state.button_continue, true);
    hide_window(&state.button_buddies, true);
}

fn init_replay_single_player(state: &mut ScoreScreenState) {
    state.screen_type = ScoreScreenType::Replay;
    grab_single_player_info(state);
    hide_window(&state.static_text_game_saved, true);
    hide_window(&state.text_entry_chat, true);
    hide_window(&state.button_emote, true);
    hide_window(&state.chat_box_border, true);
    hide_window(&state.button_continue, true);
    hide_window(&state.button_buddies, true);
    hide_window(&state.listbox_chat, true);
    hide_window(&state.listbox_academy, true);
    hide_window(&state.static_text_academy_title, true);
}

fn display_challenge_win_loss(
    state: &mut ScoreScreenState,
    image_name: &str,
    header: &str,
    remarks: &str,
) {
    hide_window(&state.backdrop, true);
    hide_window(&state.gadget_parent, true);
    hide_window(&state.challenge_win_loss_text, false);
    hide_window(&state.challenge_remarks, false);
    hide_window(&state.challenge_portrait, false);
    set_window_image(&state.parent, "GeneralsChallengeWinLoss");
    set_window_image(&state.challenge_portrait, image_name);
    if let Some(win) = state.challenge_win_loss_text.as_ref() {
        set_text(win, header);
    }
    if let Some(win) = state.challenge_remarks.as_ref() {
        set_text(win, remarks);
    }
}

fn finalize_single_player_init(state: &mut ScoreScreenState) {
    // C++ finishSinglePlayerInit: freeMessageResources before destroying BlankWindow.
    crate::helpers::TheInGameUI::free_message_resources();
    if let Some(blank) = state.blank_layout.take() {
        blank.borrow_mut().destroy_windows();
    }

    if let Some(parent) = state.parent.as_ref() {
        let _ = with_window_manager(|manager| manager.activate_window(parent));
    }

    hide_window(&state.button_ok, false);
    hide_window(&state.button_continue, false);
    hide_window(&state.text_entry_chat, true);
    hide_window(&state.button_emote, true);
    hide_window(&state.listbox_chat, true);
    hide_window(&state.listbox_academy, true);
    hide_window(&state.static_text_academy_title, true);
    hide_window(&state.chat_box_border, true);
    hide_window(&state.button_buddies, true);

    if let Some(manager) = get_campaign_manager().get_current_campaign() {
        if !manager.is_challenge_campaign() {
            with_window_manager(|manager| manager.transition_set_group("ScoreScreenShow", false));
        }
    }
}

fn maybe_start_final_victory_movie(state: &mut ScoreScreenState) -> bool {
    let Some(movie_name) = state.pending_final_victory_movie.take() else {
        return false;
    };

    if play_script_display_movie(&movie_name) {
        state.waiting_for_final_victory_movie = true;
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

fn update_final_victory_movie_wait(state: &mut ScoreScreenState) {
    if !state.waiting_for_final_victory_movie {
        return;
    }

    if is_script_display_movie_playing() {
        return;
    }

    state.waiting_for_final_victory_movie = false;
    finalize_single_player_init(state);
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
fn current_mission_save_description() -> String {
    let manager = get_campaign_manager();
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

fn finish_single_player_init(state: &mut ScoreScreenState) {
    let victorious = {
        let manager = get_campaign_manager();
        manager.is_victorious()
    };

    if victorious {
        let is_challenge = {
            let manager = get_campaign_manager();
            manager
                .get_current_campaign()
                .map(|campaign| campaign.is_challenge_campaign())
                .unwrap_or(false)
        };
        if is_challenge {
            if let Some(generals_mutex) = get_challenge_generals() {
                let generals = generals_mutex.lock().unwrap_or_else(|e| e.into_inner());
                let manager = get_campaign_manager();
                if let Some(mission) = manager.get_current_mission() {
                    if let Some(general) = generals.general_by_general_name(&mission.general_name) {
                        let header = GameText::fetch("GUI:ChallengeWinText")
                            .replace("%s", &GameText::fetch(&mission.general_name));
                        let remarks = GameText::fetch(general.string_defeated());
                        display_challenge_win_loss(
                            state,
                            general.image_defeated().unwrap_or_default(),
                            &header,
                            &remarks,
                        );
                        if let Some(audio) = TheAudio::get() {
                            let event =
                                gamelogic::common::audio::AudioEventRts::new(general.win_sound());
                            audio.add_audio_event(&event);
                        }
                    }
                }
            }
        }

        let next_map = {
            let mut manager = get_campaign_manager();
            let _ = manager.goto_next_mission();
            manager.get_current_map().unwrap_or_default()
        };

        if next_map.is_empty() {
            if let Some(button_continue) = state.button_continue.as_ref() {
                set_text(button_continue, &GameText::fetch("GUI:EndCampaign"));
            }
            state.button_is_finish_campaign = true;

            let mut stats = SkirmishBattleHonors::new();
            let manager = get_campaign_manager();
            if let Some(campaign) = manager.get_current_campaign() {
                let difficulty = manager.get_game_difficulty();
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

            hide_window(&state.button_ok, true);
            hide_window(&state.button_continue, true);
            hide_window(&state.text_entry_chat, true);
            hide_window(&state.button_emote, true);
            hide_window(&state.listbox_chat, true);
            hide_window(&state.listbox_academy, true);
            hide_window(&state.static_text_academy_title, true);
            hide_window(&state.chat_box_border, true);
            hide_window(&state.button_buddies, true);

            let manager = get_campaign_manager();
            if let Some(campaign) = manager.get_current_campaign() {
                state.pending_final_victory_movie =
                    final_victory_movie_to_queue(campaign, prefers_low_res_movies());
            }

            if maybe_start_final_victory_movie(state) {
                return;
            }
        } else {
            if let Some(button_continue) = state.button_continue.as_ref() {
                set_text(button_continue, &GameText::fetch("GUI:SaveAndContinue"));
            }
            // C++ ScoreScreen.cpp:880 TheGameState->missionSave()
            // GameState.cpp:616-618 GUI:MissionSave + campaignNameLabel + missionNumber+1.
            let mut game_state = game_engine::get_game_state();
            let _ = game_state.save_game(
                String::new(),
                current_mission_save_description(),
                game_engine::SaveFileType::Mission,
                game_engine::SnapshotType::SaveLoad,
            );
            hide_window(&state.static_text_game_saved, false);
        }
    } else {
        let is_challenge = {
            let manager = get_campaign_manager();
            manager
                .get_current_campaign()
                .map(|campaign| campaign.is_challenge_campaign())
                .unwrap_or(false)
        };
        if is_challenge {
            if let Some(generals_mutex) = get_challenge_generals() {
                let generals = generals_mutex.lock().unwrap_or_else(|e| e.into_inner());
                let manager = get_campaign_manager();
                if let Some(mission) = manager.get_current_mission() {
                    if let Some(general) = generals.general_by_general_name(&mission.general_name) {
                        let header = GameText::fetch("GUI:ChallengeLossText")
                            .replace("%s", &GameText::fetch(&mission.general_name));
                        let remarks = GameText::fetch(general.string_victorious());
                        display_challenge_win_loss(
                            state,
                            general.image_victorious().unwrap_or_default(),
                            &header,
                            &remarks,
                        );
                        if let Some(audio) = TheAudio::get() {
                            let event =
                                gamelogic::common::audio::AudioEventRts::new(general.loss_sound());
                            audio.add_audio_event(&event);
                        }
                    }
                }
            }
        }
        if let Some(button_continue) = state.button_continue.as_ref() {
            set_text(button_continue, &GameText::fetch("GUI:Retry"));
        }
    }

    finalize_single_player_init(state);
}

fn grab_multi_player_info(state: &mut ScoreScreenState) {
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

    hide_windows(state, players.len() as i32);
    for (index, (_, player_arc)) in players.into_iter().enumerate() {
        if let Ok(player) = player_arc.read() {
            if player.is_player_observer() {
                set_observer_windows(state, &player, index as i32);
            } else {
                populate_player_info(state, &player, index as i32);
            }
        }
    }
}

fn grab_single_player_info(state: &mut ScoreScreenState) {
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
        populate_player_info(state, &local_player, player_count);
        player_count += 1;
    } else {
        for player_arc in list.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    populate_player_info(state, &player, player_count);
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
            );
            player_count += 1;
        }
    }

    hide_windows(state, player_count);
}

fn hide_windows(state: &mut ScoreScreenState, pos: i32) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }

    for i in pos..MAX_SLOTS {
        let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextObserver{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsBuilt{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsLost{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextUnitsDestroyed{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsBuilt{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsLost{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextBuildingsDestroyed{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:StaticTextResources{}", i);
        hide_window(&find_child(&state.parent, &name), true);
        let name = format!("ScoreScreen.wnd:GameWindowWinner{}", i);
        hide_window(&find_child(&state.parent, &name), true);
    }
}

fn set_observer_windows(state: &mut ScoreScreenState, player: &Player, index: i32) {
    if !(0..MAX_SLOTS).contains(&index) {
        return;
    }

    let color = 0xffffffffu32;
    let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", index);
    if let Some(win) = find_child(&state.parent, &name) {
        set_text(&win, player.get_player_display_name());
        let _ = win.borrow_mut().hide(false);
        set_text_color(&win, color);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", index);
    hide_window(&find_child(&state.parent, &name), false);

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
        hide_window(&find_child(&state.parent, &name), true);
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", index);
    let win = find_child(&state.parent, &name);
    hide_window(&win, false);
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

fn update_skirmish_battle_honors(stats: &mut SkirmishBattleHonors, player: &Player) {
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
    let minutes = leftover_score_logic_frame() / LOGICFRAMES_PER_SECOND / 60;
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

thread_local! {
    /// Live host end frame published before `reset_match_state` zeros Eva.
    static LEFTOVER_SCORE_END_FRAME: Cell<u32> = const { Cell::new(0) };
}

/// Snapshot `TheGameLogic->getFrame()` from the live host before reset.
pub fn publish_leftover_score_end_frame(frame: u32) {
    LEFTOVER_SCORE_END_FRAME.with(|cell| cell.set(frame));
}

fn leftover_score_logic_frame() -> u32 {
    let published = LEFTOVER_SCORE_END_FRAME.with(|cell| cell.get());
    if published != 0 {
        published
    } else {
        crate::eva::eva_logic_frame()
    }
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
                widget.add_item(tip);
            }
        }
    }
}

fn populate_player_info(state: &mut ScoreScreenState, player: &Player, pos: i32) {
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
        let _ = win.borrow_mut().hide(false);
        set_text_color(&win, color);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", pos);
    hide_window(&find_child(&state.parent, &name), true);

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
            let _ = win.borrow_mut().hide(false);
        }
    }
    if player.is_local_player() {
        if let Some(listbox) = state.listbox_academy.as_ref() {
            let _ = listbox.borrow_mut().hide(false);
            hide_window(&state.static_text_academy_title, false);
            if TheGameLogic::is_in_skirmish_game() || TheGameLogic::is_in_multiplayer_game() {
                fill_academy_advice(listbox, player);
            }
        }
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", pos);
    if let Some(win) = find_child(&state.parent, &name) {
        let _ = win.borrow_mut().hide(false);
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
            update_skirmish_battle_honors(&mut stats, player);
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
) {
    if !(0..MAX_SLOTS).contains(&pos) {
        return;
    }

    let name = format!("ScoreScreen.wnd:StaticTextPlayer{}", pos);
    if let Some(win) = find_child(&state.parent, &name) {
        set_text(&win, side);
        set_text_color(&win, color);
        let _ = win.borrow_mut().hide(false);
    }

    let name = format!("ScoreScreen.wnd:StaticTextObserver{}", pos);
    hide_window(&find_child(&state.parent, &name), true);

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
            let _ = win.borrow_mut().hide(false);
        }
    }

    let name = format!("ScoreScreen.wnd:GameWindowWinner{}", pos);
    let win = find_child(&state.parent, &name);
    if let Some(side_icon) = (!gather.side_icon.is_empty()).then_some(gather.side_icon.as_str()) {
        hide_window(&win, false);
        set_window_image(&win, side_icon);
    }
}

pub fn score_screen_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    with_score_screen_state(|state| {
        state.play_music = true;
        set_dont_show_main_menu(true);
        state.button_is_finish_campaign = false;

        state.parent_id = name_to_id("ScoreScreen.wnd:ParentScoreScreen");
        state.button_ok_id = name_to_id("ScoreScreen.wnd:ButtonOk");
        state.text_entry_chat_id = name_to_id("ScoreScreen.wnd:TextEntryChat");
        state.button_emote_id = name_to_id("ScoreScreen.wnd:ButtonEmote");
        state.listbox_chat_id = name_to_id("ScoreScreen.wnd:ListboxChatWindowScoreScreen");
        state.listbox_academy_id = name_to_id("ScoreScreen.wnd:ListboxWarschoolAdvice");
        state.static_text_academy_title_id = name_to_id("ScoreScreen.wnd:StaticTextWarSchool");
        state.chat_box_border_id = name_to_id("ScoreScreen.wnd:ChatBoxBorder");
        state.button_buddies_id = name_to_id("ScoreScreen.wnd:ButtonBuddy");
        state.button_continue_id = name_to_id("ScoreScreen.wnd:ButtonContinue");
        state.button_save_replay_id = name_to_id("ScoreScreen.wnd:ButtonSaveReplay");
        state.backdrop_id = name_to_id("ScoreScreen.wnd:MainBackdrop");

        state.parent = with_window_manager(|manager| manager.get_window_by_id(state.parent_id));
        state.button_ok = find_child(&state.parent, "ScoreScreen.wnd:ButtonOk");
        state.text_entry_chat = find_child(&state.parent, "ScoreScreen.wnd:TextEntryChat");
        state.button_emote = find_child(&state.parent, "ScoreScreen.wnd:ButtonEmote");
        state.listbox_chat = find_child(
            &state.parent,
            "ScoreScreen.wnd:ListboxChatWindowScoreScreen",
        );
        state.listbox_academy = find_child(&state.parent, "ScoreScreen.wnd:ListboxWarschoolAdvice");
        state.static_text_academy_title =
            find_child(&state.parent, "ScoreScreen.wnd:StaticTextWarSchool");
        state.chat_box_border = find_child(&state.parent, "ScoreScreen.wnd:ChatBoxBorder");
        state.button_continue = find_child(&state.parent, "ScoreScreen.wnd:ButtonContinue");
        state.button_buddies = find_child(&state.parent, "ScoreScreen.wnd:ButtonBuddy");
        state.static_text_game_saved =
            find_child(&state.parent, "ScoreScreen.wnd:StaticTextGameSaveComplete");
        state.backdrop = find_child(&state.parent, "ScoreScreen.wnd:MainBackdrop");
        state.challenge_portrait = find_child(&state.parent, "ScoreScreen.wnd:BigPortrait");
        state.challenge_win_loss_text =
            find_child(&state.parent, "ScoreScreen.wnd:ChallengeWinLossText");
        state.challenge_remarks = find_child(&state.parent, "ScoreScreen.wnd:GeneralRemarks");
        state.gadget_parent = find_child(&state.parent, "ScoreScreen.wnd:GadgetParent");

        state.override_player_display_name = false;
        state.last_replay_filename = String::new();
        state.can_save_replay = false;
        state.pending_final_victory_movie = None;
        state.waiting_for_final_victory_movie = false;

        if let Ok(recorder) = get_recorder().lock() {
            state.last_replay_filename = recorder.last_replay_filename().to_string();
            state.can_save_replay = recorder.get_mode() == RecorderMode::Record;
        }

        hide_window(&state.static_text_game_saved, true);

        if let Some(parent) = state.parent.as_ref() {
            if let Some(button) = parent
                .borrow()
                .find_child_by_id(state.button_save_replay_id)
            {
                if let Ok(recorder) = get_recorder().lock() {
                    if recorder.get_mode() == RecorderMode::None {
                        let _ = button.borrow_mut().enable(false);
                    }
                }
            }
        }

        state.need_finish_singleplayer_init = false;

        if TheGameLogic::is_in_replay_game() {
            if let Some(parent) = state.parent.as_ref() {
                if let Some(button) = parent
                    .borrow()
                    .find_child_by_id(state.button_save_replay_id)
                {
                    let _ = button.borrow_mut().hide(true);
                }
            }
            if let Ok(recorder) = get_recorder().lock() {
                if recorder.is_multiplayer() {
                    init_replay_multiplayer(state);
                } else {
                    state.override_player_display_name = true;
                    init_replay_single_player(state);
                }
            }
            with_window_manager(|manager| manager.transition_set_group("ScoreScreenShow", false));
        } else if TheGameLogic::get_game_mode() == GAME_INTERNET {
            init_internet_multiplayer(state);
            with_window_manager(|manager| manager.transition_set_group("ScoreScreenShow", false));
        } else if TheGameLogic::get_game_mode() == GAME_LAN {
            init_lan_multiplayer(state);
            with_window_manager(|manager| manager.transition_set_group("ScoreScreenShow", false));
        } else if TheGameLogic::get_game_mode() == GAME_SKIRMISH {
            init_skirmish(state);
            with_window_manager(|manager| manager.transition_set_group("ScoreScreenShow", false));
        } else {
            state.override_player_display_name = true;
            init_single_player(state);
            if let Some(parent) = state.parent.as_ref() {
                if let Some(button) = parent
                    .borrow()
                    .find_child_by_id(state.button_save_replay_id)
                {
                    let _ = button.borrow_mut().hide(true);
                }
            }
        }

        if let Some(manager) = get_campaign_manager().get_current_campaign() {
            if manager.is_challenge_campaign() {
                if let Some(parent) = state.parent.as_ref() {
                    if let Some(button) = parent
                        .borrow()
                        .find_child_by_id(state.button_save_replay_id)
                    {
                        let _ = button.borrow_mut().enable(false);
                        let _ = button.borrow_mut().hide(true);
                    }
                }
            }
        }

        layout.hide(false);
        if let Some(parent) = state.parent.as_ref() {
            let _ = with_window_manager(|manager| manager.activate_window(parent));
        }
        set_replay_was_pressed(false);
    });
}

pub fn score_screen_update(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    with_score_screen_state(|state| {
        if let Some(popup) = state.popup_replay_layout.as_ref() {
            if !popup.borrow().is_hidden() {
                let popup_ref = popup.borrow();
                popup_replay_update(&popup_ref, None);
            }
        }

        if state.need_finish_singleplayer_init {
            finish_single_player_init(state);
            state.need_finish_singleplayer_init = false;
        }

        update_final_victory_movie_wait(state);
        update_score_screen_music(state);

        let _ = layout;
    });
}

pub fn score_screen_shutdown(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    set_dont_show_main_menu(false);

    layout.hide(true);
    queue_shell_shutdown_complete(false);

    if let Some(audio) = TheAudio::get() {
        audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
    }
}

pub fn score_screen_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        let button_ok = name_to_id("ScoreScreen.wnd:ButtonOk") as u32;
        let _ = with_window_manager(|manager| {
            manager.get_window_by_id(window.get_id()).map(|handle| {
                handle.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    button_ok as WindowMsgData,
                    0,
                )
            })
        });
    }

    WindowMsgHandled::Handled
}

pub fn score_screen_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    with_score_screen_state(|state| match msg {
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            with_window_manager(|manager| manager.transition_remove("ScoreScreenShow", true));
            set_replay_was_pressed(false);

            if control_id == state.button_ok_id {
                queue_shell_pop();
                get_campaign_manager().set_campaign("");
            } else if control_id == state.button_continue_id {
                if !state.button_is_finish_campaign {
                    set_replay_was_pressed(true);
                }
                if state.screen_type == ScoreScreenType::SinglePlayer {
                    let map_name = get_campaign_manager().get_current_map().unwrap_or_default();
                    if map_name.is_empty() {
                        set_replay_was_pressed(false);
                        queue_shell_pop();
                    } else {
                        // C++ CheckForCDAtGameStart(startNextCampaignGame)
                        check_for_cd_at_game_start(start_next_campaign_game);
                    }
                }
            } else if control_id == state.button_save_replay_id {
                score_screen_enable_controls(false);
                let layout = if let Some(layout) = state.popup_replay_layout.as_ref() {
                    layout.clone()
                } else {
                    let Some((layout, _)) = with_window_manager(|manager| {
                        manager
                            .create_layout_with_windows("Menus/PopupReplay.wnd")
                            .ok()
                    }) else {
                        return WindowMsgHandled::Handled;
                    };
                    state.popup_replay_layout = Some(layout.clone());
                    layout
                };
                layout.borrow().run_init(None);
                layout.borrow_mut().hide(false);
                layout.borrow_mut().bring_forward();
            }

            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == state.text_entry_chat_id {
                if let Some(entry) = state.text_entry_chat.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        widget.set_text(String::new());
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    })
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

        assert_eq!(
            score_screen_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn start_next_campaign_game_appends_msg_new_game_like_cpp() {
        start_next_campaign_game();
        let stream = get_message_stream();
        let guard = stream.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            guard.contains_message_of_type(&GameMessageType::NewGame),
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
    fn skirmish_blitz_honors_live_end_frame_not_leftover_zero() {
        // Leftover TheGameLogic::get_frame() stays 0 after reset; published
        // live end frame is the C++ getFrame() used for Blitz5/10.
        publish_leftover_score_end_frame(0);
        crate::eva::set_eva_host_frame(0);
        assert_eq!(leftover_score_logic_frame(), 0);

        publish_leftover_score_end_frame(18_000);
        crate::eva::set_eva_host_frame(0);
        assert_eq!(leftover_score_logic_frame(), 18_000);
        let minutes = leftover_score_logic_frame() / LOGICFRAMES_PER_SECOND / 60;
        assert_eq!(minutes, 10);
        assert!(minutes >= 5, "10-minute match must not award Blitz5");
        assert!(minutes >= 10, "10-minute match must not award Blitz10");

        publish_leftover_score_end_frame(8_999);
        let minutes = leftover_score_logic_frame() / LOGICFRAMES_PER_SECOND / 60;
        assert!(minutes < 5);
        assert!(minutes < 10);

        publish_leftover_score_end_frame(0);
        crate::eva::set_eva_host_frame(0);
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

static RESIDUAL_SCORE_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SCORE_FINISH_CAMPAIGN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_score_action_store(action: ResidualScoreScreenAction) {
    RESIDUAL_SCORE_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last ScoreScreen residual action.
pub fn residual_score_screen_last_action() -> ResidualScoreScreenAction {
    match RESIDUAL_SCORE_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualScoreScreenAction::Ok,
        2 => ResidualScoreScreenAction::Continue,
        3 => ResidualScoreScreenAction::SaveReplay,
        4 => ResidualScoreScreenAction::Buddy,
        5 => ResidualScoreScreenAction::Emote,
        _ => ResidualScoreScreenAction::None,
    }
}

/// Residual: ButtonContinue is finish-campaign residual latch.
pub fn residual_score_screen_is_finish_campaign() -> bool {
    RESIDUAL_SCORE_FINISH_CAMPAIGN.load(std::sync::atomic::Ordering::Relaxed)
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
pub fn simulate_score_screen_bind_controls() -> bool {
    with_score_screen_state(|state| {
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
    })
}

/// Residual: mark Continue as end-of-campaign finish (no next map).
pub fn simulate_score_screen_set_finish_campaign(finish: bool) -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        state.button_is_finish_campaign = finish;
        RESIDUAL_SCORE_FINISH_CAMPAIGN.store(finish, std::sync::atomic::Ordering::Relaxed);
        residual_score_screen_is_finish_campaign() == finish
    })
}

/// Residual: fire ButtonOk without shell pop / campaign clear.
pub fn simulate_score_screen_ok_button_gadget_selected() -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        residual_score_action_store(ResidualScoreScreenAction::Ok);
        true
    })
}

/// Residual: fire ButtonContinue without next-map start / shell pop.
pub fn simulate_score_screen_continue_button_gadget_selected() -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        residual_score_action_store(ResidualScoreScreenAction::Continue);
        true
    })
}

/// Residual: fire ButtonSaveReplay without PopupReplay layout create.
pub fn simulate_score_screen_save_replay_button_gadget_selected() -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        residual_score_action_store(ResidualScoreScreenAction::SaveReplay);
        true
    })
}

/// Residual: fire ButtonBuddy without buddy overlay.
pub fn simulate_score_screen_buddy_button_gadget_selected() -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        residual_score_action_store(ResidualScoreScreenAction::Buddy);
        true
    })
}

/// Residual: fire ButtonEmote without chat stream.
pub fn simulate_score_screen_emote_button_gadget_selected() -> bool {
    with_score_screen_state(|state| {
        ensure_score_screen_control_ids(state);
        residual_score_action_store(ResidualScoreScreenAction::Emote);
        true
    })
}

/// Residual: finish-campaign + Continue composite (end match honesty).
pub fn simulate_score_screen_prepare_finish() -> bool {
    if !simulate_score_screen_bind_controls() {
        return false;
    }
    if !simulate_score_screen_set_finish_campaign(true) {
        return false;
    }
    simulate_score_screen_continue_button_gadget_selected()
}

/// Residual: Ok path composite (return to shell honesty).
pub fn simulate_score_screen_prepare_ok() -> bool {
    if !simulate_score_screen_bind_controls() {
        return false;
    }
    simulate_score_screen_ok_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonContinue`
/// (C++ WindowXlat hit → GBM_SELECTED → CheckForCDAtGameStart(startNextCampaignGame)).
/// Not `simulate_*` first.
pub fn drive_os_wnd_score_screen_continue_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonContinue");
    if !clicked {
        return false;
    }
    if !simulate_score_screen_continue_button_gadget_selected() {
        return false;
    }
    // C++ ScoreScreen.cpp ButtonContinue: single-player + next map starts the
    // next mission. Finish-campaign residual skips startNextCampaignGame.
    let start_next = with_score_screen_state(|state| {
        state.screen_type == ScoreScreenType::SinglePlayer && !state.button_is_finish_campaign
    }) && get_campaign_manager()
        .get_current_map()
        .map(|m| !m.is_empty())
        .unwrap_or(false);
    if start_next {
        check_for_cd_at_game_start(start_next_campaign_game);
    }
    true
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonOk`
/// (C++ WindowXlat hit → GBM_SELECTED → shell pop + clear campaign).
pub fn drive_os_wnd_score_screen_ok_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonOk");
    if !clicked {
        return false;
    }
    simulate_score_screen_ok_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonSaveReplay`.
pub fn drive_os_wnd_score_screen_save_replay_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonSaveReplay");
    if !clicked {
        return false;
    }
    simulate_score_screen_save_replay_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonBuddy`.
pub fn drive_os_wnd_score_screen_buddy_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonBuddy");
    if !clicked {
        return false;
    }
    simulate_score_screen_buddy_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ScoreScreen.wnd:ButtonEmote`.
pub fn drive_os_wnd_score_screen_emote_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("ScoreScreen.wnd:ButtonEmote");
    if !clicked {
        return false;
    }
    simulate_score_screen_emote_button_gadget_selected()
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
        install_named_button("ScoreScreen.wnd:ButtonContinue", 10, 70);
        {
            let mut manager = get_campaign_manager();
            manager.init();
            manager.set_campaign("USA");
            // Victory → next mission is already current when ScoreScreen Continue fires.
            let _ = manager.goto_next_mission();
        }
        let _ = simulate_score_screen_set_finish_campaign(false);
        assert!(drive_os_wnd_score_screen_continue_like_cpp());
        assert_eq!(
            residual_score_screen_last_action(),
            ResidualScoreScreenAction::Continue
        );
        let current_map = get_campaign_manager().get_current_map().unwrap_or_default();
        if current_map.is_empty() {
            return;
        }
        let stream = get_message_stream();
        let guard = stream.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            guard.contains_message_of_type(&GameMessageType::NewGame),
            "C++ ButtonContinue CheckForCDAtGameStart(startNextCampaignGame) posts MSG_NEW_GAME"
        );
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
