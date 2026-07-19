//! ScoreScreen.cpp callback port.

use crate::core::script_action_handler::{
    is_script_display_movie_playing, play_script_display_movie,
};
use crate::game_text::GameText;
use crate::gui::callbacks::popup_replay::popup_replay_update;
use crate::gui::campaign_manager::{
    get_campaign_manager, Campaign, GameDifficulty as CampaignDifficulty,
};
use crate::gui::challenge_generals::get_challenge_generals;
use crate::gui::menu_flags::{set_dont_show_main_menu, set_replay_was_pressed};
use crate::gui::shell::Shell;
use crate::gui::{
    get_shell, show_shell_map_if_available, try_with_shell_mut, with_window_manager,
    write_input_focus_response, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowStatus,
};
use game_engine::common::game_lod::prefers_low_res_movies;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::{get_recorder, RecorderMode};
use game_engine::common::skirmish_battle_honors::{
    SkirmishBattleHonors, BATTLE_HONOR_CAMPAIGN_CHINA, BATTLE_HONOR_CAMPAIGN_GLA,
    BATTLE_HONOR_CAMPAIGN_USA, BATTLE_HONOR_CHALLENGE_MODE,
};
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine};
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

fn update_score_screen_music(state: &mut ScoreScreenState) {
    if !state.play_music {
        return;
    }

    state.play_music = false;

    let Some(audio) = TheAudio::get() else {
        return;
    };

    let local_template_name = {
        let Ok(list) = ThePlayerList().read() else {
            return;
        };
        let Some(local_player) = list.get_local_player() else {
            return;
        };
        let Ok(player) = local_player.read() else {
            return;
        };
        player
            .get_player_template()
            .map(|template| template.get_score_screen_music().to_string())
            .unwrap_or_default()
    };

    if !local_template_name.is_empty() {
        audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        let mut event = gamelogic::common::audio::AudioEventRts::new(local_template_name);
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
    let _ = try_with_shell_mut(|shell| {
        leave_score_screen_for_next_campaign(shell);
    });

    let pending_file = {
        let manager = get_campaign_manager();
        manager.get_current_map().unwrap_or_default()
    };

    if let Some(data) = game_engine::common::ini::get_global_data() {
        let mut data = data.write();
        data.pending_file = pending_file;
    }

    let difficulty = TheScriptEngine::get_global_difficulty();
    let rank_points = if let Ok(list) = ThePlayerList().read() {
        list.get_local_player()
            .and_then(|player| player.read().ok().map(|p| p.get_skill_points()))
            .unwrap_or(0)
    } else {
        0
    };

    TheGameLogic::prepare_new_game(GAME_SINGLE_PLAYER, difficulty, rank_points);
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
            let mut game_state = game_engine::get_game_state();
            let _ = game_state.save_game(
                String::new(),
                GameText::fetch("GUI:AutoSave"),
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
    let mut players: Vec<(i32, std::sync::Arc<std::sync::RwLock<Player>>)> = Vec::new();
    let Ok(list) = ThePlayerList().read() else {
        return;
    };

    for player_arc in list.iter() {
        if let Ok(player) = player_arc.read() {
            if player.get_player_type() == PlayerType::Neutral {
                continue;
            }
            let score = player.get_score_keeper().get_total_score();
            players.push((score, std::sync::Arc::clone(player_arc)));
        }
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

                    let is_allied = relationship == gamelogic::prelude::Relationship::Allies;
                    if (is_friend && is_allied) || (!is_friend && !is_allied) {
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
                let _info = game_engine::common::rts::AcademyAdviceInfo::default();
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
        let mut stats = SkirmishBattleHonors::new();
        if TheGameLogic::is_in_skirmish_game() {
            if TheGameLogic::is_in_multiplayer_game() {
                stats.set_losses(stats.get_losses() + 1);
                stats.set_win_streak(0);
            } else {
                stats.set_wins(stats.get_wins() + 1);
                stats.set_win_streak(stats.get_win_streak() + 1);
                let best = stats.get_best_win_streak().max(stats.get_win_streak());
                stats.set_best_win_streak(best);
            }
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
    let _ = try_with_shell_mut(|shell| shell.shutdown_complete(None, false));

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
                let _ = try_with_shell_mut(|shell| shell.pop());
                get_campaign_manager().set_campaign("");
            } else if control_id == state.button_continue_id {
                if !state.button_is_finish_campaign {
                    set_replay_was_pressed(true);
                }
                if state.screen_type == ScoreScreenType::SinglePlayer {
                    let map_name = get_campaign_manager().get_current_map().unwrap_or_default();
                    if map_name.is_empty() {
                        set_replay_was_pressed(false);
                        let _ = try_with_shell_mut(|shell| shell.pop());
                    } else {
                        start_next_campaign_game();
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
}
