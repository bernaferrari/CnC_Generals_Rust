//! C++ parity wrapper for `LoadScreen.cpp`.

pub use super::loading_screen::*;

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::input::with_mouse;
use crate::map_util::{find_draw_positions, get_map_cache_manager, get_map_preview_image};

use super::campaign_manager::{
    get_campaign_manager, Mission, MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES,
};
use super::challenge_generals::{
    get_challenge_generals, get_challenge_generals_mut, init_challenge_generals, ChallengeGenerals,
    GeneralPersona,
};
use super::game_window::{GameWindow, Image as WindowImage, WindowMessage, GPM_SET_PROGRESS};
use super::window_video_manager::{with_window_video_manager, WindowVideoPlayType};
use super::{with_window_manager, WindowManager, WindowStatus};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::player_template::{get_player_template_store, PlayerTemplate};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_LOAD_SCREEN_SLOTS: usize = 8;
const FRAME_FUDGE_ADD: f32 = 30.0;
const FRAME_FUDGE_SCALE: f32 = 1.3;
const FRAME_TITLES_START: i32 = 20;
const FRAME_TELETYPE_START: i32 = 24;
const FRAME_PORTRAITS_START: i32 = 35;
const FRAME_OUTER_CIRCLE_ALPHA_SHOW: i32 = 63;
const FRAME_INNER_CIRCLE_ALPHA_SHOW: i32 = 74;
const FRAME_INNER_BACKDROP_ALPHA_SHOW: i32 = 80;
const FRAME_VS_ANIM_START: i32 = 98;
const FRAME_RIGHT_VOICE: i32 = 140;
const TELETYPE_UPDATE_FREQ: i32 = 2;
const SHELL_GAME_LEGAL_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadScreenGameMode {
    SinglePlayer,
    Skirmish,
    Multiplayer,
    Replay,
    Internet,
    Lan,
    Shell,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadScreenKind {
    ShellGame,
    SinglePlayer,
    Challenge,
    Multiplayer,
    GameSpy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadScreenRequest {
    pub mode: LoadScreenGameMode,
    pub loading_save_game: bool,
    pub has_current_campaign: bool,
    pub current_campaign_is_challenge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadScreenDescriptor {
    pub kind: LoadScreenKind,
    pub layout: &'static str,
    pub root: &'static str,
    pub primary_progress: &'static str,
    pub progress_prefix: &'static str,
    pub slot_count: usize,
    pub uses_progress_fudge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadScreenInitContext {
    pub local_player_name: String,
    pub local_side_name: String,
    pub local_template_name: String,
    pub local_general_name: String,
    pub local_general_features: String,
    pub local_general_portrait: Option<String>,
    pub local_load_screen_music: String,
    pub local_team_number: i32,
    pub map_name: Option<String>,
    pub start_positions: Vec<Option<usize>>,
    pub slots: Vec<LoadScreenSlotInitContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadScreenSlotInitContext {
    pub player_id: i32,
    pub player_name: String,
    pub side_name: String,
    pub team_number: i32,
    pub apparent_color: Option<i32>,
    pub apparent_text_color: Option<u32>,
    pub is_ai: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MultiplayerLoadScreenState {
    player_lookup: [i32; MAX_LOAD_SCREEN_SLOTS],
    local_player_id: i32,
}

impl Default for MultiplayerLoadScreenState {
    fn default() -> Self {
        Self {
            player_lookup: [-1; MAX_LOAD_SCREEN_SLOTS],
            local_player_id: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SinglePlayerMissionText {
    objective_lines: [String; MAX_OBJECTIVE_LINES],
    unit_descriptions: [String; MAX_DISPLAYED_UNITS],
    location: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SinglePlayerLoadScreenState {
    mission_text: SinglePlayerMissionText,
    current_objective_line: usize,
    current_objective_width_offset: i32,
    current_objective_line_character: usize,
    finished_objective_text: bool,
}

static SINGLE_PLAYER_LOAD_SCREEN_STATE: OnceLock<Mutex<SinglePlayerLoadScreenState>> =
    OnceLock::new();
static SHELL_GAME_FIRST_LOAD: OnceLock<Mutex<bool>> = OnceLock::new();
static MULTIPLAYER_LOAD_SCREEN_STATE: OnceLock<Mutex<MultiplayerLoadScreenState>> = OnceLock::new();

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ChallengePersonaText {
    big_name: String,
    name: String,
    rank: String,
    strategy: String,
    portrait_large: Option<String>,
    portrait_movie_left: String,
    portrait_movie_right: String,
    name_sound: String,
    taunt_sounds: [String; 3],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ChallengeLoadScreenState {
    player: Option<ChallengePersonaText>,
    opponent: Option<ChallengePersonaText>,
    high_spec_prelude_active: bool,
    current_frame: i32,
    postlude_audio_played: bool,
    ambient_loop_handle: u32,
    text_pos_big_name_right: usize,
    text_pos_name_right: usize,
    text_pos_birthplace_right: usize,
    text_pos_strategy_right: usize,
    text_pos_big_name_left: usize,
    text_pos_name_left: usize,
    text_pos_birthplace_left: usize,
    text_pos_strategy_left: usize,
}

static CHALLENGE_LOAD_SCREEN_STATE: OnceLock<Mutex<ChallengeLoadScreenState>> = OnceLock::new();

const CHALLENGE_BIO_LABEL_WINDOWS: &[&str] = &[
    "ChallengeLoadScreen.wnd:BioNameLeft",
    "ChallengeLoadScreen.wnd:BioBirthplaceLeft",
    "ChallengeLoadScreen.wnd:BioStrategyLeft",
    "ChallengeLoadScreen.wnd:BioNameRight",
    "ChallengeLoadScreen.wnd:BioBirthplaceRight",
    "ChallengeLoadScreen.wnd:BioStrategyRight",
];

const CHALLENGE_BIO_ENTRY_WINDOWS: &[&str] = &[
    "ChallengeLoadScreen.wnd:BigNameEntryLeft",
    "ChallengeLoadScreen.wnd:BioNameEntryLeft",
    "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft",
    "ChallengeLoadScreen.wnd:BioStrategyEntryLeft",
    "ChallengeLoadScreen.wnd:BigNameEntryRight",
    "ChallengeLoadScreen.wnd:BioNameEntryRight",
    "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight",
    "ChallengeLoadScreen.wnd:BioStrategyEntryRight",
];

impl ChallengeLoadScreenState {
    fn reset_teletype_positions(&mut self) {
        self.text_pos_big_name_right = 0;
        self.text_pos_name_right = 0;
        self.text_pos_birthplace_right = 0;
        self.text_pos_strategy_right = 0;
        self.text_pos_big_name_left = 0;
        self.text_pos_name_left = 0;
        self.text_pos_birthplace_left = 0;
        self.text_pos_strategy_left = 0;
    }
}

impl Default for LoadScreenInitContext {
    fn default() -> Self {
        Self {
            local_player_name: "Player".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: Vec::new(),
        }
    }
}

pub fn select_load_screen(request: LoadScreenRequest) -> Option<LoadScreenKind> {
    match request.mode {
        LoadScreenGameMode::Shell | LoadScreenGameMode::Replay => Some(LoadScreenKind::ShellGame),
        LoadScreenGameMode::SinglePlayer => {
            if request.loading_save_game || !request.has_current_campaign {
                Some(LoadScreenKind::ShellGame)
            } else if request.current_campaign_is_challenge {
                Some(LoadScreenKind::Challenge)
            } else {
                Some(LoadScreenKind::SinglePlayer)
            }
        }
        LoadScreenGameMode::Skirmish
        | LoadScreenGameMode::Lan
        | LoadScreenGameMode::Multiplayer => Some(LoadScreenKind::Multiplayer),
        LoadScreenGameMode::Internet => Some(LoadScreenKind::GameSpy),
        LoadScreenGameMode::None => None,
    }
}

pub fn descriptor_for_kind(kind: LoadScreenKind) -> LoadScreenDescriptor {
    match kind {
        LoadScreenKind::ShellGame => LoadScreenDescriptor {
            kind,
            layout: "Menus/ShellGameLoadScreen.wnd",
            root: "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen",
            primary_progress: "ShellGameLoadScreen.wnd:ProgressLoad",
            progress_prefix: "ShellGameLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: false,
        },
        LoadScreenKind::SinglePlayer => LoadScreenDescriptor {
            kind,
            layout: "Menus/SinglePlayerLoadScreen.wnd",
            root: "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
            primary_progress: "SinglePlayerLoadScreen.wnd:ProgressLoad",
            progress_prefix: "SinglePlayerLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: true,
        },
        LoadScreenKind::Challenge => LoadScreenDescriptor {
            kind,
            layout: "Menus/ChallengeLoadScreen.wnd",
            root: "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
            primary_progress: "ChallengeLoadScreen.wnd:ProgressLoad",
            progress_prefix: "ChallengeLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: true,
        },
        LoadScreenKind::Multiplayer => LoadScreenDescriptor {
            kind,
            layout: "Menus/MultiplayerLoadScreen.wnd",
            root: "MultiplayerLoadScreen.wnd:ParentMultiplayerLoadScreen",
            primary_progress: "MultiplayerLoadScreen.wnd:ProgressLoad0",
            progress_prefix: "MultiplayerLoadScreen.wnd:ProgressLoad",
            slot_count: MAX_LOAD_SCREEN_SLOTS,
            uses_progress_fudge: false,
        },
        LoadScreenKind::GameSpy => LoadScreenDescriptor {
            kind,
            layout: "Menus/GameSpyLoadScreen.wnd",
            root: "GameSpyLoadScreen.wnd:ParentMultiplayerLoadScreen",
            primary_progress: "GameSpyLoadScreen.wnd:ProgressLoad0",
            progress_prefix: "GameSpyLoadScreen.wnd:ProgressLoad",
            slot_count: MAX_LOAD_SCREEN_SLOTS,
            uses_progress_fudge: false,
        },
    }
}

pub fn transformed_progress_percent(descriptor: LoadScreenDescriptor, raw_percent: f32) -> f32 {
    if descriptor.uses_progress_fudge {
        (raw_percent + FRAME_FUDGE_ADD) / FRAME_FUDGE_SCALE
    } else {
        raw_percent
    }
}

pub fn init_load_screen(kind: LoadScreenKind, context: &LoadScreenInitContext) -> bool {
    let descriptor = descriptor_for_kind(kind);
    with_window_manager(|wm| {
        if wm.create_layout_with_windows(descriptor.layout).is_err() {
            return false;
        }

        if let Some(root) = wm.find_window_by_name(descriptor.root) {
            let mut root = root.borrow_mut();
            let _ = root.hide(false);
            let _ = root.bring_to_front();
        }

        initialize_progress_windows(wm, descriptor);
        initialize_kind_windows(wm, descriptor.kind, context);
        true
    })
}

pub fn load_screen_init_context_from_game_info(
    game_info: &crate::game_network::GameInfo,
) -> LoadScreenInitContext {
    let slots: Vec<_> = (0..MAX_LOAD_SCREEN_SLOTS)
        .filter_map(|player_id| {
            let slot = game_info.get_slot(player_id)?;
            slot.is_occupied().then(|| LoadScreenSlotInitContext {
                player_id: player_id as i32,
                player_name: slot.get_name().to_string(),
                side_name: slot.get_apparent_player_template_display_name(),
                team_number: slot.get_team_number(),
                apparent_color: (slot.get_apparent_color() >= 0)
                    .then_some(slot.get_apparent_color()),
                apparent_text_color: multiplayer_apparent_text_color(slot.get_apparent_color()),
                is_ai: slot.is_ai(),
                visible: true,
            })
        })
        .collect();
    let start_positions = (0..MAX_LOAD_SCREEN_SLOTS)
        .map(|player_id| {
            let slot = game_info.get_slot(player_id)?;
            let start_pos = slot.get_apparent_start_pos();
            (start_pos >= 0 && slot.get_player_template() > crate::game_network::PLAYERTEMPLATE_MIN)
                .then_some(start_pos as usize)
        })
        .collect();

    let local_player_id = game_info.get_local_slot_num();
    let local_slot = if local_player_id >= 0 {
        slots.iter().find(|slot| slot.player_id == local_player_id)
    } else {
        slots.first()
    };

    if let Some(local_slot) = local_slot {
        let local_template = if local_slot.player_id >= 0 {
            let template_index =
                game_info
                    .get_slot(local_slot.player_id as usize)
                    .and_then(|slot| {
                        let player_template = slot.get_player_template();
                        (player_template >= 0).then_some(player_template as usize)
                    });
            let store = get_player_template_store();
            template_index
                .and_then(|index| store.get_nth_player_template(index).cloned())
                .or_else(|| store.find_template("FactionObserver").cloned())
        } else {
            None
        };
        let local_general_presentation =
            multiplayer_local_general_presentation(local_template.as_ref(), &local_slot.side_name);
        LoadScreenInitContext {
            local_player_name: local_slot.player_name.clone(),
            local_side_name: local_slot.side_name.clone(),
            local_template_name: local_general_presentation.template_name,
            local_general_name: local_general_presentation.name,
            local_general_features: local_general_presentation.features,
            local_general_portrait: local_general_presentation.portrait,
            local_load_screen_music: local_general_presentation.load_screen_music,
            local_team_number: local_slot.player_id,
            map_name: load_screen_map_name_from_game_info(game_info),
            start_positions,
            slots,
        }
    } else {
        LoadScreenInitContext::default()
    }
}

fn load_screen_map_name_from_game_info(
    game_info: &crate::game_network::GameInfo,
) -> Option<String> {
    let map_name = game_info.get_map();
    if map_name.is_empty() {
        return None;
    }

    if !game_info.is_game_in_progress() {
        let local_slot_num = game_info.get_local_slot_num();
        if local_slot_num < 0 {
            return None;
        }

        let Some(local_slot) = game_info.get_slot(local_slot_num as usize) else {
            return None;
        };
        if !local_slot.has_map() {
            return None;
        }
    }

    Some(map_name.to_string())
}

pub fn reset_load_screen(kind: LoadScreenKind) {
    let descriptor = descriptor_for_kind(kind);
    with_window_manager(|wm| {
        if let Some(root) = wm.find_window_by_name(descriptor.root) {
            let _ = wm.destroy_window(root);
            wm.flush_destroy_queue();
        }
    });
    if kind == LoadScreenKind::Challenge {
        reset_challenge_load_screen_audio_state();
    } else if descriptor.slot_count > 0 {
        reset_multiplayer_load_screen_state();
    }
}

pub fn update_load_screen(kind: LoadScreenKind, raw_percent: f32) {
    let descriptor = descriptor_for_kind(kind);
    let percent = transformed_progress_percent(descriptor, raw_percent);
    clear_load_screen_cursor_tooltip();
    if descriptor.slot_count > 0 {
        let local_player_id = with_multiplayer_load_screen_state(|state| state.local_player_id);
        let _ = process_load_screen_progress(kind, local_player_id, percent);
        return;
    }
    with_window_manager(|wm| {
        set_progress_window(wm, descriptor.primary_progress, percent);
        if kind == LoadScreenKind::SinglePlayer {
            set_window_text(
                wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &format!("{}%", percent as i32),
            );
        } else if kind == LoadScreenKind::Challenge {
            update_challenge_load_screen_prelude(wm);
            if raw_percent >= 100.0 {
                finish_challenge_load_screen_audio_postlude();
            }
        }
    });
}

fn clear_load_screen_cursor_tooltip() {
    with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));
}

pub fn process_load_screen_progress(kind: LoadScreenKind, player_id: i32, percentage: f32) -> bool {
    let descriptor = descriptor_for_kind(kind);
    if descriptor.slot_count == 0 || !(0.0..=100.0).contains(&percentage) {
        return false;
    }

    let compact_slot = with_multiplayer_load_screen_state(|state| {
        if player_id < 0 || player_id as usize >= MAX_LOAD_SCREEN_SLOTS {
            None
        } else {
            let compact_slot = state.player_lookup[player_id as usize];
            (compact_slot >= 0).then_some(compact_slot as usize)
        }
    });
    let Some(compact_slot) = compact_slot else {
        return false;
    };

    with_window_manager(|wm| {
        set_progress_window(
            wm,
            &format!("{}{}", descriptor.progress_prefix, compact_slot),
            percentage,
        );
    });
    true
}

fn initialize_progress_windows(wm: &mut WindowManager, descriptor: LoadScreenDescriptor) {
    if descriptor.slot_count == 0 {
        set_progress_window(wm, descriptor.primary_progress, 0.0);
        if descriptor.kind == LoadScreenKind::ShellGame {
            hide_window(wm, descriptor.primary_progress, true);
        }
        hide_window(wm, descriptor.primary_progress, false);
        return;
    }

    for slot in 0..descriptor.slot_count {
        let name = format!("{}{}", descriptor.progress_prefix, slot);
        set_progress_window(wm, &name, 0.0);
    }
}

fn initialize_kind_windows(
    wm: &mut WindowManager,
    kind: LoadScreenKind,
    context: &LoadScreenInitContext,
) {
    match kind {
        LoadScreenKind::ShellGame => initialize_shell_game_windows(wm),
        LoadScreenKind::SinglePlayer => initialize_single_player_windows(wm),
        LoadScreenKind::Challenge => initialize_challenge_windows(wm),
        LoadScreenKind::Multiplayer => {
            initialize_multiplayer_windows(wm, "MultiplayerLoadScreen.wnd", context)
        }
        LoadScreenKind::GameSpy => initialize_gamespy_windows(wm, context),
    }
}

fn initialize_shell_game_windows(wm: &mut WindowManager) {
    let is_first_load = with_shell_game_first_load(|first_load| {
        let was_first_load = *first_load;
        *first_load = false;
        was_first_load
    });

    if is_first_load {
        set_window_image(
            wm,
            "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen",
            0,
            "TitleScreen",
            true,
        );
        hide_window(wm, "ShellGameLoadScreen.wnd:StaticTextLegal", false);
        hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", true);
        run_shell_game_legal_hold(wm);
        hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", false);
    }
}

fn with_shell_game_first_load<R>(f: impl FnOnce(&mut bool) -> R) -> R {
    let state = SHELL_GAME_FIRST_LOAD.get_or_init(|| Mutex::new(true));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn with_multiplayer_load_screen_state<R>(
    f: impl FnOnce(&mut MultiplayerLoadScreenState) -> R,
) -> R {
    let state = MULTIPLAYER_LOAD_SCREEN_STATE
        .get_or_init(|| Mutex::new(MultiplayerLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn reset_multiplayer_load_screen_state() {
    with_multiplayer_load_screen_state(|state| *state = MultiplayerLoadScreenState::default());
}

#[cfg(not(test))]
fn shell_game_legal_hold_duration() -> Duration {
    Duration::from_millis(3000)
}

#[cfg(test)]
fn shell_game_legal_hold_duration() -> Duration {
    Duration::ZERO
}

fn run_shell_game_legal_hold(wm: &mut WindowManager) {
    let hold_duration = shell_game_legal_hold_duration();
    if hold_duration.is_zero() {
        wm.update();
        return;
    }

    let show_start = Instant::now();
    while show_start.elapsed() < hold_duration {
        wm.update();
        std::thread::sleep(SHELL_GAME_LEGAL_UPDATE_INTERVAL);
    }
}

fn initialize_single_player_windows(wm: &mut WindowManager) {
    with_single_player_load_screen_state(|state| *state = SinglePlayerLoadScreenState::default());

    set_window_text(wm, "SinglePlayerLoadScreen.wnd:Percent", "0%");
    hide_window(wm, "SinglePlayerLoadScreen.wnd:Percent", true);
    hide_window(wm, "SinglePlayerLoadScreen.wnd:ObjectivesWin", true);

    for line in 0..MAX_OBJECTIVE_LINES {
        set_window_text(
            wm,
            &format!("SinglePlayerLoadScreen.wnd:StaticTextLine{line}"),
            "",
        );
    }

    for cameo in 0..4 {
        hide_window(
            wm,
            &format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{cameo}"),
            true,
        );
    }

    let campaign_manager = get_campaign_manager();
    if let Some(campaign) = campaign_manager.get_current_campaign() {
        if let Some((background, progress)) = single_player_campaign_images(&campaign.name) {
            set_window_image(
                wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                0,
                background,
                true,
            );
            set_window_image(
                wm,
                "SinglePlayerLoadScreen.wnd:ProgressLoad",
                6,
                progress,
                false,
            );
        }
    }

    if let Some(mission) = campaign_manager.get_current_mission() {
        let text = single_player_mission_text(mission);
        with_single_player_load_screen_state(|state| {
            state.mission_text = text.clone();
            state.current_objective_line = 0;
            state.current_objective_width_offset = 0;
            state.current_objective_line_character = 0;
            state.finished_objective_text = false;
        });
        for unit in 0..MAX_DISPLAYED_UNITS {
            set_window_text(
                wm,
                &format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{unit}"),
                &text.unit_descriptions[unit],
            );
        }
        set_window_text(
            wm,
            "SinglePlayerLoadScreen.wnd:StaticTextCameoText3",
            &text.location,
        );
    }
}

fn with_single_player_load_screen_state<R>(
    f: impl FnOnce(&mut SinglePlayerLoadScreenState) -> R,
) -> R {
    let state = SINGLE_PLAYER_LOAD_SCREEN_STATE
        .get_or_init(|| Mutex::new(SinglePlayerLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn initialize_challenge_windows(wm: &mut WindowManager) {
    with_challenge_load_screen_state(|state| *state = ChallengeLoadScreenState::default());
    with_window_video_manager(|manager| manager.init());

    for name in [
        "ChallengeLoadScreen.wnd:PortraitLeft",
        "ChallengeLoadScreen.wnd:PortraitRight",
        "ChallengeLoadScreen.wnd:CircleAlphaOuter",
        "ChallengeLoadScreen.wnd:CircleAlphaInner",
        "ChallengeLoadScreen.wnd:VersusBackdrop",
        "ChallengeLoadScreen.wnd:OverlayVs",
        "ChallengeLoadScreen.wnd:PortraitMovieLeft",
        "ChallengeLoadScreen.wnd:PortraitMovieRight",
        "ChallengeLoadScreen.wnd:BioNameLeft",
        "ChallengeLoadScreen.wnd:BioBirthplaceLeft",
        "ChallengeLoadScreen.wnd:BioStrategyLeft",
        "ChallengeLoadScreen.wnd:BigNameEntryLeft",
        "ChallengeLoadScreen.wnd:BioNameEntryLeft",
        "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft",
        "ChallengeLoadScreen.wnd:BioStrategyEntryLeft",
        "ChallengeLoadScreen.wnd:BioNameRight",
        "ChallengeLoadScreen.wnd:BioBirthplaceRight",
        "ChallengeLoadScreen.wnd:BioStrategyRight",
        "ChallengeLoadScreen.wnd:BigNameEntryRight",
        "ChallengeLoadScreen.wnd:BioNameEntryRight",
        "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight",
        "ChallengeLoadScreen.wnd:BioStrategyEntryRight",
    ] {
        hide_window(wm, name, true);
    }

    if let Some((player, opponent)) = current_challenge_persona_text() {
        let movie_label = current_challenge_movie_label();
        with_challenge_load_screen_state(|state| {
            state.player = Some(player.clone());
            state.opponent = Some(opponent.clone());
            state.high_spec_prelude_active = movie_label.is_some();
            state.current_frame = 0;
            state.postlude_audio_played = false;
            state.ambient_loop_handle = 0;
        });
        if let Some(image) = player.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitLeft", 0, image, true);
        }
        if let Some(image) = opponent.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitRight", 0, image, true);
        }
        if let Some(movie_label) = movie_label {
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
                &movie_label,
            );
        } else {
            activate_challenge_pieces_min_spec_windows(wm);
            finish_challenge_load_screen_audio_postlude();
        }
    }
}

fn with_challenge_load_screen_state<R>(f: impl FnOnce(&mut ChallengeLoadScreenState) -> R) -> R {
    let state =
        CHALLENGE_LOAD_SCREEN_STATE.get_or_init(|| Mutex::new(ChallengeLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

pub fn activate_challenge_load_screen_frame(frame: i32) {
    with_window_manager(|wm| activate_challenge_pieces_frame_windows(wm, frame));
}

pub fn activate_challenge_load_screen_min_spec() {
    with_window_manager(activate_challenge_pieces_min_spec_windows);
}

fn update_challenge_load_screen_prelude(wm: &mut WindowManager) {
    let frame = with_challenge_load_screen_state(|state| {
        if !state.high_spec_prelude_active {
            return None;
        }
        state.current_frame += 1;
        Some(state.current_frame)
    });

    if let Some(frame) = frame {
        activate_challenge_pieces_frame_windows(wm, frame);
        with_window_video_manager(|manager| manager.update());
    }
}

fn finish_challenge_load_screen_audio_postlude() {
    let postlude = with_challenge_load_screen_state(|state| {
        if state.postlude_audio_played {
            return None;
        }
        let taunt = {
            let opponent = state.opponent.as_ref()?;
            challenge_taunt_sound(opponent, challenge_taunt_seed()).map(str::to_string)
        };
        state.postlude_audio_played = true;
        state.high_spec_prelude_active = false;
        Some(taunt)
    });

    let Some(taunt) = postlude else {
        return;
    };
    if let Some(taunt) = taunt {
        play_audio_event(&taunt);
    }
    let ambient_handle = add_audio_event("LoadScreenAmbient");
    with_challenge_load_screen_state(|state| {
        state.ambient_loop_handle = ambient_handle;
    });
}

fn reset_challenge_load_screen_audio_state() {
    let ambient_handle = with_challenge_load_screen_state(|state| {
        let handle = state.ambient_loop_handle;
        state.high_spec_prelude_active = false;
        state.current_frame = 0;
        state.postlude_audio_played = false;
        state.ambient_loop_handle = 0;
        handle
    });
    if ambient_handle != 0 {
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(ambient_handle);
        }
    }
}

fn challenge_taunt_seed() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as usize)
        .unwrap_or(0)
}

fn challenge_taunt_sound(persona: &ChallengePersonaText, seed: usize) -> Option<&str> {
    Some(persona.taunt_sounds[seed % persona.taunt_sounds.len()].as_str())
}

fn activate_challenge_pieces_frame_windows(wm: &mut WindowManager, frame: i32) {
    let personas = with_challenge_load_screen_state(|state| {
        let player = state.player.clone()?;
        let opponent = state.opponent.clone()?;
        Some((player, opponent))
    });
    let Some((player, opponent)) = personas else {
        return;
    };

    match frame {
        FRAME_TITLES_START => {
            for name in CHALLENGE_BIO_LABEL_WINDOWS {
                hide_window(wm, name, false);
            }
        }
        FRAME_TELETYPE_START => {
            with_challenge_load_screen_state(ChallengeLoadScreenState::reset_teletype_positions);
            for name in CHALLENGE_BIO_ENTRY_WINDOWS {
                hide_window(wm, name, false);
                set_window_text(wm, name, "");
            }
        }
        FRAME_PORTRAITS_START => {
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                &player.portrait_movie_left,
            );
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:PortraitMovieRight",
                &opponent.portrait_movie_right,
            );
            hide_window(wm, "ChallengeLoadScreen.wnd:PortraitMovieLeft", false);
            hide_window(wm, "ChallengeLoadScreen.wnd:PortraitMovieRight", false);
            play_audio_event(&player.name_sound);
        }
        FRAME_OUTER_CIRCLE_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaOuter", false);
        }
        FRAME_INNER_CIRCLE_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaInner", false);
        }
        FRAME_INNER_BACKDROP_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
        }
        FRAME_VS_ANIM_START => {
            hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
            hide_window(wm, "ChallengeLoadScreen.wnd:OverlayVs", false);
            play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
            play_audio_event("Taunts_GCAnnouncer12");
        }
        FRAME_RIGHT_VOICE => {
            play_audio_event(&opponent.name_sound);
        }
        _ => {}
    }

    if frame > FRAME_TELETYPE_START && frame % TELETYPE_UPDATE_FREQ == 0 {
        with_challenge_load_screen_state(|state| {
            state.text_pos_name_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioNameEntryLeft",
                &player.name,
                state.text_pos_name_left,
            );
            state.text_pos_big_name_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BigNameEntryLeft",
                &player.big_name,
                state.text_pos_big_name_left,
            );
            state.text_pos_birthplace_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft",
                &player.rank,
                state.text_pos_birthplace_left,
            );
            state.text_pos_strategy_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioStrategyEntryLeft",
                &player.strategy,
                state.text_pos_strategy_left,
            );
            state.text_pos_name_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioNameEntryRight",
                &opponent.name,
                state.text_pos_name_right,
            );
            state.text_pos_big_name_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BigNameEntryRight",
                &opponent.big_name,
                state.text_pos_big_name_right,
            );
            state.text_pos_birthplace_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight",
                &opponent.rank,
                state.text_pos_birthplace_right,
            );
            state.text_pos_strategy_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioStrategyEntryRight",
                &opponent.strategy,
                state.text_pos_strategy_right,
            );
        });
    }
}

fn activate_challenge_pieces_min_spec_windows(wm: &mut WindowManager) {
    let personas = with_challenge_load_screen_state(|state| {
        let player = state.player.clone()?;
        let opponent = state.opponent.clone()?;
        Some((player, opponent))
    });
    let Some((player, opponent)) = personas else {
        return;
    };

    for name in CHALLENGE_BIO_LABEL_WINDOWS
        .iter()
        .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
    {
        hide_window(wm, name, false);
    }

    set_challenge_bio_entry_text(wm, "Left", &player);
    set_challenge_bio_entry_text(wm, "Right", &opponent);

    if let Some(image) = player.portrait_large.as_deref() {
        set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitLeft", 0, image, true);
    }
    if let Some(image) = opponent.portrait_large.as_deref() {
        set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitRight", 0, image, true);
    }
    hide_window(wm, "ChallengeLoadScreen.wnd:PortraitLeft", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:PortraitRight", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaOuter", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaInner", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:OverlayVs", false);
    play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
}

fn set_challenge_bio_entry_text(
    wm: &mut WindowManager,
    side: &str,
    persona: &ChallengePersonaText,
) {
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BigNameEntry{side}"),
        &persona.big_name,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioNameEntry{side}"),
        &persona.name,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioBirthplaceEntry{side}"),
        &persona.rank,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioStrategyEntry{side}"),
        &persona.strategy,
    );
}

fn update_teletype_text(
    wm: &mut WindowManager,
    window_name: &str,
    full_text: &str,
    current_text_pos: usize,
) -> usize {
    let Some(window) = wm.find_window_by_name(window_name) else {
        return current_text_pos;
    };
    let Some(next_char) = full_text.chars().nth(current_text_pos) else {
        return current_text_pos;
    };
    let mut window = window.borrow_mut();
    let mut current = window.get_text().to_string();
    current.push(next_char);
    let _ = window.set_text(&current);
    current_text_pos + 1
}

fn play_challenge_movie(wm: &mut WindowManager, window_name: &str, movie_name: &str) {
    if movie_name.is_empty() {
        return;
    }
    if let Some(window) = wm.find_window_by_name(window_name) {
        with_window_video_manager(|manager| {
            manager.play_movie(
                window,
                movie_name.to_string(),
                WindowVideoPlayType::ShowLastFrame,
            )
        });
    }
}

fn play_audio_event(event_name: &str) {
    let _ = add_audio_event(event_name);
}

#[cfg(not(test))]
fn add_audio_event(event_name: &str) -> u32 {
    if event_name.is_empty() {
        return 0;
    }
    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new(event_name);
        audio.add_audio_event(&event)
    } else {
        0
    }
}

#[cfg(test)]
fn add_audio_event(event_name: &str) -> u32 {
    if event_name.is_empty() {
        0
    } else {
        event_name
            .bytes()
            .fold(1_u32, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(byte as u32)
            })
            .max(1)
    }
}

fn initialize_multiplayer_windows(
    wm: &mut WindowManager,
    prefix: &str,
    context: &LoadScreenInitContext,
) {
    let has_team_windows = load_screen_has_team_windows(prefix);
    if let Some(portrait_image) = context
        .local_general_portrait
        .as_deref()
        .or_else(|| multiplayer_local_general_faction_logo(&context.local_template_name, prefix))
        .or_else(|| multiplayer_local_general_faction_logo(&context.local_side_name, prefix))
    {
        set_window_image(
            wm,
            &format!("{prefix}:LocalGeneralPortrait"),
            0,
            portrait_image,
            false,
        );
    }
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralFeatures"),
        multiplayer_local_general_text_fallback(
            &context.local_general_features,
            &context.local_side_name,
        ),
    );
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralName"),
        multiplayer_local_general_text_fallback(
            &context.local_general_name,
            &context.local_side_name,
        ),
    );
    if prefix == "MultiplayerLoadScreen.wnd" {
        play_multiplayer_load_screen_music(&context.local_load_screen_music);
    }
    initialize_multiplayer_map_preview(
        wm,
        prefix,
        context.map_name.as_deref(),
        &context.start_positions,
    );

    let slots = multiplayer_slot_contexts(context);
    with_multiplayer_load_screen_state(|state| {
        *state = MultiplayerLoadScreenState::default();
        state.local_player_id = context.local_team_number;
        for (compact_slot, slot_context) in slots.iter().enumerate() {
            if slot_context.player_id >= 0
                && (slot_context.player_id as usize) < MAX_LOAD_SCREEN_SLOTS
            {
                state.player_lookup[slot_context.player_id as usize] = compact_slot as i32;
            }
        }
    });
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        set_progress_window(wm, &format!("{prefix}:ProgressLoad{slot}"), 0.0);
        if let Some(slot_context) = slots.get(slot) {
            if let Some(progress_image) = multiplayer_progress_bar_image(slot_context) {
                set_window_image(
                    wm,
                    &format!("{prefix}:ProgressLoad{slot}"),
                    6,
                    &progress_image,
                    false,
                );
            }
            set_window_text(
                wm,
                &format!("{prefix}:StaticTextPlayer{slot}"),
                &slot_context.player_name,
            );
            set_window_text(
                wm,
                &format!("{prefix}:StaticTextSide{slot}"),
                &slot_context.side_name,
            );
            if has_team_windows {
                set_window_text(
                    wm,
                    &format!("{prefix}:StaticTextTeam{slot}"),
                    &GameText::fetch(&multiplayer_team_text(slot_context)),
                );
            }
            if let Some(text_color) = slot_context.apparent_text_color {
                let suffixes = if has_team_windows {
                    &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
                } else {
                    &["StaticTextPlayer", "StaticTextSide"][..]
                };
                for suffix in suffixes {
                    set_window_enabled_text_color(
                        wm,
                        &format!("{prefix}:{suffix}{slot}"),
                        text_color,
                    );
                }
            }

            let suffixes = if has_team_windows {
                &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
            } else {
                &["StaticTextPlayer", "StaticTextSide"][..]
            };
            for suffix in suffixes {
                hide_window(wm, &format!("{prefix}:{suffix}{slot}"), false);
            }
            hide_window(
                wm,
                &format!("{prefix}:ProgressLoad{slot}"),
                slot_context.is_ai,
            );
            continue;
        }

        let suffixes = if has_team_windows {
            &[
                "ProgressLoad",
                "StaticTextPlayer",
                "StaticTextSide",
                "StaticTextTeam",
            ][..]
        } else {
            &["ProgressLoad", "StaticTextPlayer", "StaticTextSide"][..]
        };
        for suffix in suffixes {
            hide_window(wm, &format!("{prefix}:{suffix}{slot}"), true);
        }
    }
}

fn load_screen_has_team_windows(prefix: &str) -> bool {
    !prefix.eq_ignore_ascii_case("GameSpyLoadScreen.wnd")
}

fn initialize_multiplayer_map_preview(
    wm: &mut WindowManager,
    prefix: &str,
    map_name: Option<&str>,
    start_positions: &[Option<usize>],
) {
    let preview_window_name = format!("{prefix}:WinMapPreview");
    let Some(preview) = wm.find_window_by_name(&preview_window_name) else {
        return;
    };

    let preview_image = map_name.and_then(get_map_preview_image);
    let metadata = map_name.and_then(multiplayer_map_metadata);
    let Some(preview_image) = preview_image else {
        preview.borrow_mut().clear_status(WindowStatus::IMAGE);
        update_multiplayer_start_position_buttons(wm, prefix, metadata.as_ref(), start_positions);
        return;
    };

    set_window_image(wm, &preview_window_name, 0, &preview_image, true);
    update_multiplayer_start_position_buttons(wm, prefix, metadata.as_ref(), start_positions);
}

fn multiplayer_map_metadata(map_name: &str) -> Option<MapMetaData> {
    let cache = get_map_cache_manager();
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.update_cache();
    cache.find_map(map_name)
}

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn update_multiplayer_start_position_buttons(
    wm: &mut WindowManager,
    prefix: &str,
    metadata: Option<&MapMetaData>,
    start_positions: &[Option<usize>],
) {
    let preview_window_name = format!("{prefix}:WinMapPreview");
    let Some(preview) = wm.find_window_by_name(&preview_window_name) else {
        return;
    };

    let Some(metadata) = metadata.filter(|metadata| metadata.is_multiplayer) else {
        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            hide_window(wm, &format!("{prefix}:ButtonMapStartPosition{slot}"), true);
        }
        return;
    };

    position_multiplayer_start_position_buttons(wm, prefix, &preview, metadata);
    apply_multiplayer_start_position_labels(wm, prefix, metadata, start_positions);
}

fn position_multiplayer_start_position_buttons(
    wm: &mut WindowManager,
    prefix: &str,
    preview: &Rc<RefCell<GameWindow>>,
    metadata: &MapMetaData,
) {
    let preview = preview.borrow();
    let (map_x, map_y) = preview.get_screen_position();
    let (map_w, map_h) = preview.get_size();
    let extent = metadata.extent;
    let (ul, lr) = find_draw_positions(map_x, map_y, map_w, map_h, extent);
    let extent_width = (extent.hi.x - extent.lo.x).max(1.0);
    let extent_height = (extent.hi.y - extent.lo.y).max(1.0);
    drop(preview);

    let mut placed_buttons: Vec<(i32, i32, i32, i32)> = Vec::new();
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        let button_name = format!("{prefix}:ButtonMapStartPosition{slot}");
        let Some(button) = wm.find_window_by_name(&button_name) else {
            continue;
        };
        let waypoint = if (slot as i32) < metadata.num_players {
            metadata.get_waypoint(&map_start_waypoint_name(slot))
        } else {
            None
        };
        let mut button = button.borrow_mut();
        if let Some(coord) = waypoint {
            let ratio_x = (coord.x - extent.lo.x) / extent_width;
            let ratio_y = (extent.hi.y - coord.y) / extent_height;
            let draw_x = ul.x as f32 + (lr.x - ul.x) as f32 * ratio_x;
            let draw_y = ul.y as f32 + (lr.y - ul.y) as f32 * ratio_y;
            let (btn_w, btn_h) = button.get_size();
            let mut new_x = draw_x.round() as i32 - btn_w / 2 - map_x;
            let mut new_y = draw_y.round() as i32 - btn_h / 2 - map_y;
            let gadget_size = btn_w.max(btn_h);
            for (x, y, w, h) in &placed_buttons {
                if new_x >= *x && new_x < *x + *w && new_y >= *y && new_y < *y + *h {
                    if new_y + gadget_size + 1 < map_h {
                        new_y += gadget_size + 1;
                    } else {
                        new_x += gadget_size + 1;
                    }
                }
            }
            let _ = button.set_position(new_x, new_y);
            let _ = button.hide(false);
            let _ = button.enable(true);
            placed_buttons.push((new_x, new_y, btn_w, btn_h));
        } else {
            let _ = button.hide(true);
        }
    }
}

fn apply_multiplayer_start_position_labels(
    wm: &mut WindowManager,
    prefix: &str,
    metadata: &MapMetaData,
    start_positions: &[Option<usize>],
) {
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        set_window_text(wm, &format!("{prefix}:ButtonMapStartPosition{slot}"), "");
    }

    let max_players = metadata.num_players.max(0) as usize;
    for (player_index, start_pos) in start_positions.iter().enumerate() {
        let Some(start_pos) = start_pos else {
            continue;
        };
        if *start_pos < max_players {
            set_window_text(
                wm,
                &format!("{prefix}:ButtonMapStartPosition{start_pos}"),
                &GameText::fetch(&format!("NUMBER:{}", player_index + 1)),
            );
        }
    }
}

fn initialize_gamespy_windows(wm: &mut WindowManager, context: &LoadScreenInitContext) {
    initialize_multiplayer_windows(wm, "GameSpyLoadScreen.wnd", context);

    let slots = multiplayer_slot_contexts(context);
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        let slot_context = slots.get(slot);
        hide_window(
            wm,
            &format!("GameSpyLoadScreen.wnd:WinPlayer{slot}"),
            slot_context.is_none(),
        );
        let hide_stats = slot_context.map(|slot| slot.is_ai).unwrap_or(true);
        for suffix in gamespy_stats_suffixes() {
            hide_window(
                wm,
                &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"),
                hide_stats,
            );
        }
    }
}

fn multiplayer_slot_contexts(context: &LoadScreenInitContext) -> Vec<LoadScreenSlotInitContext> {
    let slots: Vec<_> = context
        .slots
        .iter()
        .filter(|slot| slot.visible)
        .take(MAX_LOAD_SCREEN_SLOTS)
        .cloned()
        .collect();

    if slots.is_empty() {
        vec![LoadScreenSlotInitContext {
            player_id: context.local_team_number,
            player_name: context.local_player_name.clone(),
            side_name: context.local_side_name.clone(),
            team_number: context.local_team_number,
            apparent_color: None,
            apparent_text_color: None,
            is_ai: false,
            visible: true,
        }]
    } else {
        slots
    }
}

fn multiplayer_team_text(slot: &LoadScreenSlotInitContext) -> String {
    format!("Team:{}", slot.team_number + 1)
}

fn multiplayer_progress_bar_image(slot: &LoadScreenSlotInitContext) -> Option<String> {
    let image_name = slot
        .apparent_color
        .filter(|color| *color >= 0)
        .map(|color| format!("LoadingBar_ProgressCenter{color}"))?;
    if mapped_image_exists(&image_name) || !mapped_image_exists("LoadingBar_Progress") {
        Some(image_name)
    } else {
        Some("LoadingBar_Progress".to_string())
    }
}

fn multiplayer_apparent_text_color(apparent_color: i32) -> Option<u32> {
    with_multiplayer_settings(|settings| settings.get_color_value(apparent_color))
}

fn mapped_image_exists(image_name: &str) -> bool {
    get_mapped_image_collection()
        .try_read()
        .and_then(|collection| collection.find_image_by_name(image_name).map(|_| ()))
        .is_some()
}

fn gamespy_stats_suffixes() -> [&'static str; 4] {
    [
        "StaticTextTotalDisconnects",
        "StaticTextWinLoss",
        "WinRank",
        "WinOfficer",
    ]
}

fn multiplayer_local_general_faction_logo(side_name: &str, prefix: &str) -> Option<&'static str> {
    let gamespy = prefix.eq_ignore_ascii_case("GameSpyLoadScreen.wnd");
    let side = side_name.trim();
    if side.eq_ignore_ascii_case("USA")
        || side.eq_ignore_ascii_case("America")
        || side.eq_ignore_ascii_case("FactionAmerica")
    {
        Some(if gamespy {
            "SAFactionLogo144_US"
        } else {
            "SAFactionLogoLg_US"
        })
    } else if side.eq_ignore_ascii_case("GLA") || side.eq_ignore_ascii_case("FactionGLA") {
        Some(if gamespy {
            "SUFactionLogo144_GLA"
        } else {
            "SUFactionLogoLg_GLA"
        })
    } else if side.eq_ignore_ascii_case("China") || side.eq_ignore_ascii_case("FactionChina") {
        Some(if gamespy {
            "SNFactionLogo144_China"
        } else {
            "SNFactionLogoLg_China"
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MultiplayerLocalGeneralPresentation {
    template_name: String,
    name: String,
    features: String,
    portrait: Option<String>,
    load_screen_music: String,
}

fn multiplayer_local_general_presentation(
    template: Option<&PlayerTemplate>,
    fallback_side_name: &str,
) -> MultiplayerLocalGeneralPresentation {
    let Some(template) = template else {
        return MultiplayerLocalGeneralPresentation {
            template_name: fallback_side_name.to_string(),
            name: fallback_side_name.to_string(),
            features: fallback_side_name.to_string(),
            portrait: None,
            load_screen_music: String::new(),
        };
    };

    let mut presentation = MultiplayerLocalGeneralPresentation {
        template_name: template.name.clone(),
        name: template.get_display_name().to_string(),
        features: if template.features.is_empty() {
            GameText::fetch("GUI:PlayerObserver")
        } else {
            GameText::fetch(&template.features)
        },
        portrait: None,
        load_screen_music: template.load_screen_music.clone(),
    };

    if let Some(generals) = get_challenge_generals() {
        if let Ok(generals) = generals.lock() {
            if let Some(general) = generals.general_by_template_name(&template.name) {
                presentation.name = GameText::fetch(general.bio_name());
                presentation.portrait = general.bio_portrait_large().map(str::to_string);
            }
        }
    }

    presentation
}

fn multiplayer_local_general_text_fallback<'a>(text: &'a str, fallback: &'a str) -> &'a str {
    if text.is_empty() {
        fallback
    } else {
        text
    }
}

fn play_multiplayer_load_screen_music(music_name: &str) {
    if music_name.is_empty() {
        return;
    }
    #[cfg(not(test))]
    {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(music_name);
            event.set_should_fade(true);
            let _ = audio.add_audio_event(&event);
            audio.update();
        }
    }
    #[cfg(test)]
    {
        let _ = music_name;
    }
}

fn set_progress_window(wm: &mut WindowManager, name: &str, percent: f32) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().send_system_message(
            WindowMessage::User(GPM_SET_PROGRESS),
            (percent as i32) as u32,
            0,
        );
    }
}

fn set_window_text(wm: &mut WindowManager, name: &str, text: &str) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().set_text(text);
    }
}

fn set_window_enabled_text_color(wm: &mut WindowManager, name: &str, color: u32) {
    if let Some(window) = wm.find_window_by_name(name) {
        let border_color = window.borrow().get_enabled_text_border_color();
        window
            .borrow_mut()
            .set_enabled_text_colors(color, border_color);
    }
}

fn set_window_image(
    wm: &mut WindowManager,
    window_name: &str,
    image_index: usize,
    image_name: &str,
    mark_image_status: bool,
) {
    let mut image = WindowImage {
        name: image_name.to_string(),
        width: 0,
        height: 0,
    };
    if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            image.width = found.get_image_width();
            image.height = found.get_image_height();
        }
    }

    if let Some(window) = wm.find_window_by_name(window_name) {
        let mut window = window.borrow_mut();
        if window.set_enabled_image(image_index, image).is_ok() && mark_image_status {
            window.set_status(WindowStatus::IMAGE);
        }
    }
}

fn hide_window(wm: &mut WindowManager, name: &str, hidden: bool) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().hide(hidden);
    }
}

fn single_player_campaign_images(campaign_name: &str) -> Option<(&'static str, &'static str)> {
    if campaign_name.eq_ignore_ascii_case("USA") {
        Some(("MissionLoad_USA", "LoadingBar_ProgressCenter2"))
    } else if campaign_name.eq_ignore_ascii_case("GLA") {
        Some(("MissionLoad_GLA", "LoadingBar_ProgressCenter3"))
    } else if campaign_name.eq_ignore_ascii_case("China") {
        Some(("MissionLoad_China", "LoadingBar_ProgressCenter1"))
    } else {
        None
    }
}

fn single_player_mission_text(mission: &Mission) -> SinglePlayerMissionText {
    SinglePlayerMissionText {
        objective_lines: mission.mission_objectives_label.each_ref().map(|label| {
            if label.is_empty() {
                String::new()
            } else {
                GameText::fetch(label)
            }
        }),
        unit_descriptions: mission
            .unit_names
            .each_ref()
            .map(|label| GameText::fetch(label)),
        location: GameText::fetch(&mission.location_name_label),
    }
}

fn challenge_persona_text(persona: &GeneralPersona) -> ChallengePersonaText {
    let name = GameText::fetch(persona.bio_name());
    ChallengePersonaText {
        big_name: name.clone(),
        name,
        rank: GameText::fetch(persona.bio_rank()),
        strategy: GameText::fetch(persona.bio_strategy()),
        portrait_large: persona.bio_portrait_large().map(str::to_string),
        portrait_movie_left: persona.portrait_movie_left_name().to_string(),
        portrait_movie_right: persona.portrait_movie_right_name().to_string(),
        name_sound: persona.name_sound().to_string(),
        taunt_sounds: [
            persona.taunt_sound_1().to_string(),
            persona.taunt_sound_2().to_string(),
            persona.taunt_sound_3().to_string(),
        ],
    }
}

fn challenge_persona_text_for_current_mission(
    campaign_name: &str,
    mission_general_name: &str,
    generals: &ChallengeGenerals,
) -> Option<(ChallengePersonaText, ChallengePersonaText)> {
    let player = generals.player_general_by_campaign_name(campaign_name)?;
    let opponent = generals.general_by_general_name(mission_general_name)?;
    Some((
        challenge_persona_text(player),
        challenge_persona_text(opponent),
    ))
}

fn current_challenge_persona_text() -> Option<(ChallengePersonaText, ChallengePersonaText)> {
    let campaign_manager = get_campaign_manager();
    let campaign = campaign_manager.get_current_campaign()?;
    let mission = campaign_manager.get_current_mission()?;
    if get_challenge_generals().is_none() {
        init_challenge_generals();
    }
    let generals = get_challenge_generals()?;
    let generals = generals.lock().ok()?;
    challenge_persona_text_for_current_mission(&campaign.name, &mission.general_name, &generals)
}

fn current_challenge_movie_label() -> Option<String> {
    let campaign_manager = get_campaign_manager();
    let mission = campaign_manager.get_current_mission()?;
    let movie_label = mission.movie_label.trim();
    (!movie_label.is_empty()).then(|| movie_label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_network::{GameInfo, GameSlot, SlotState};
    use crate::gui::gadgets::progressbar::ProgressBar;
    use crate::gui::game_window::WindowWidget;
    use game_engine::common::ini::ini_map_cache::{Coord3D, Region3D};
    use game_engine::common::language::Language;
    use std::sync::{Mutex, OnceLock};

    static TEST_LANGUAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static TEST_LOAD_SCREEN_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static TEST_MOUSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_test_language() -> std::sync::MutexGuard<'static, ()> {
        TEST_LANGUAGE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_test_load_screen_state() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOAD_SCREEN_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_test_mouse() -> std::sync::MutexGuard<'static, ()> {
        TEST_MOUSE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn selection_matches_cpp_game_logic_modes() {
        let base = LoadScreenRequest {
            mode: LoadScreenGameMode::None,
            loading_save_game: false,
            has_current_campaign: false,
            current_campaign_is_challenge: false,
        };

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Shell,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Replay,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Skirmish,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Lan,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Internet,
                ..base
            }),
            Some(LoadScreenKind::GameSpy)
        );
        assert_eq!(select_load_screen(base), None);
    }

    #[test]
    fn single_player_selection_matches_campaign_and_save_rules() {
        let normal_campaign = LoadScreenRequest {
            mode: LoadScreenGameMode::SinglePlayer,
            loading_save_game: false,
            has_current_campaign: true,
            current_campaign_is_challenge: false,
        };
        assert_eq!(
            select_load_screen(normal_campaign),
            Some(LoadScreenKind::SinglePlayer)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                current_campaign_is_challenge: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::Challenge)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                loading_save_game: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                has_current_campaign: false,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );
    }

    #[test]
    fn descriptors_match_cpp_layout_names() {
        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert_eq!(single.layout, "Menus/SinglePlayerLoadScreen.wnd");
        assert_eq!(
            single.primary_progress,
            "SinglePlayerLoadScreen.wnd:ProgressLoad"
        );
        assert!(single.uses_progress_fudge);

        let multiplayer = descriptor_for_kind(LoadScreenKind::Multiplayer);
        assert_eq!(multiplayer.layout, "Menus/MultiplayerLoadScreen.wnd");
        assert_eq!(
            multiplayer.primary_progress,
            "MultiplayerLoadScreen.wnd:ProgressLoad0"
        );
        assert_eq!(multiplayer.slot_count, MAX_LOAD_SCREEN_SLOTS);
    }

    #[test]
    fn multiplayer_init_compacts_visible_context_slots_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_status(WindowStatus::IMAGE);

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot_with_text_color(
                    "Alice",
                    "USA",
                    0,
                    Some(2),
                    Some(0xFF11_2233),
                    false,
                    true,
                ),
                load_screen_slot("Empty", "GLA", 1, false, false),
                load_screen_slot_with_color("Bob", "China", 2, Some(4), false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            "Alice"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer1"),
            "Bob"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam1"),
            "Team:3"
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextSide0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            0xFF11_2233
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogoLg_US".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "USA"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "USA"
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:ProgressLoad1", 6),
            Some("LoadingBar_ProgressCenter4".to_string())
        );
        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad1"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:StaticTextPlayer2"
        ));
    }

    #[test]
    fn multiplayer_row_team_text_is_localized_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("Team:2", "Team Two");

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 1);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Local", "USA", 1, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            "Team Two"
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_progress_bar_image_falls_back_when_color_image_missing_like_cpp() {
        let collection = get_mapped_image_collection();
        {
            let mut collection = collection.write();
            collection.clear();
            collection.add_image(crate::display::image::Image::with_name(
                "LoadingBar_Progress",
            ));
        }

        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_Progress".to_string())
        );

        collection.write().clear();
    }

    #[test]
    fn multiplayer_init_uses_context_general_portrait_features_and_name_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 1);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmericaSuperWeaponGeneral".to_string(),
            local_general_name: "General Alexander".to_string(),
            local_general_features: "Super Weapon General".to_string(),
            local_general_portrait: Some("SAGeneralPortrait".to_string()),
            local_load_screen_music: "Load_USA".to_string(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Local", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAGeneralPortrait".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "Super Weapon General"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "General Alexander"
        );
    }

    #[test]
    fn gamespy_init_keeps_player_row_for_ai_but_hides_ai_stats() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "GameSpyLoadScreen.wnd", 3);
        create_gamespy_slot_windows(&mut wm, 3);
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot("Human", "USA", 0, false, true),
                load_screen_slot("AI", "GLA", -1, true, true),
            ],
        };

        initialize_gamespy_windows(&mut wm, &context);

        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer0"));
        assert!(!window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss0"
        ));
        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer1"));
        assert!(window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss1"
        ));
        assert!(wm
            .find_window_by_name("GameSpyLoadScreen.wnd:StaticTextTeam1")
            .is_none());
        assert_eq!(
            window_image_name(&wm, "GameSpyLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogo144_US".to_string())
        );
        assert!(window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer2"));
    }

    #[test]
    fn multiplayer_process_progress_uses_cpp_player_lookup_mapping() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot("Alice", "USA", 0, false, true),
                load_screen_slot("Empty", "GLA", 1, false, false),
                load_screen_slot("Bob", "China", 2, false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        assert!(process_load_screen_progress(
            LoadScreenKind::Multiplayer,
            2,
            62.0
        ));
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1"),
            Some(0.62)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
        assert!(!process_load_screen_progress(
            LoadScreenKind::Multiplayer,
            1,
            30.0
        ));
    }

    #[test]
    fn multiplayer_update_without_lookup_does_not_fallback_to_slot_zero() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 2);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Missing".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 7,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::Multiplayer, 41.0);

        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
    }

    #[test]
    fn multiplayer_init_resets_all_progress_bars_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        for slot in 0..3 {
            let name = format!("MultiplayerLoadScreen.wnd:ProgressLoad{slot}");
            wm.find_window_by_name(&name)
                .expect("progress")
                .borrow_mut()
                .progress_bar_mut()
                .expect("progress widget")
                .set_value(0.87);
        }

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1"),
            Some(0.0)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad2"),
            Some(0.0)
        );
    }

    #[test]
    fn load_screen_init_context_default_preserves_single_local_slot() {
        let context = LoadScreenInitContext {
            local_player_name: "Fallback".to_string(),
            local_side_name: "GLA".to_string(),
            local_template_name: "FactionGLA".to_string(),
            local_general_name: "GLA".to_string(),
            local_general_features: "GLA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 4,
            map_name: None,
            start_positions: Vec::new(),
            slots: Vec::new(),
        };

        let slots = multiplayer_slot_contexts(&context);

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].player_name, "Fallback");
        assert_eq!(slots[0].side_name, "GLA");
        assert_eq!(multiplayer_team_text(&slots[0]), "Team:5");
    }

    #[test]
    fn multiplayer_team_text_matches_cpp_team_plus_one() {
        let ai_slot = load_screen_slot("AI", "GLA", -1, true, true);

        assert_eq!(multiplayer_team_text(&ai_slot), "Team:0");
        assert!(!load_screen_has_team_windows("GameSpyLoadScreen.wnd"));
        assert!(load_screen_has_team_windows("MultiplayerLoadScreen.wnd"));
    }

    #[test]
    fn multiplayer_start_position_buttons_match_map_waypoints_and_apparent_slots() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(&mut wm, "MultiplayerLoadScreen.wnd");

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 3;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(75.0, 25.0, 0.0));
        metadata.set_waypoint("Player_3_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));

        update_multiplayer_start_position_buttons(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(&metadata),
            &[Some(1), None, Some(0)],
        );

        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition3"
        ));
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            (20, 20)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            (70, 70)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            (20, 31)
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:3")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            ""
        );
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_map_preview_keeps_start_positions_when_preview_image_missing_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        game_engine::common::ini::ini_map_cache::init_global_map_cache();

        let map_name = "Maps/TestNoPreview/TestNoPreview.map";
        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 2;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(20.0, 80.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(80.0, 20.0, 0.0));
        {
            let mut cache =
                game_engine::common::ini::ini_map_cache::get_map_cache_mut().expect("map cache");
            cache.insert(map_name.to_string(), metadata);
        }

        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(&mut wm, "MultiplayerLoadScreen.wnd");

        initialize_multiplayer_map_preview(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(map_name),
            &[Some(1), Some(0)],
        );

        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:2")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );

        game_engine::common::ini::ini_map_cache::get_map_cache_mut()
            .expect("map cache")
            .remove(map_name);
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_start_position_buttons_hide_without_multiplayer_metadata() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:WinMapPreview");
        create_multiplayer_start_position_windows(&mut wm, "GameSpyLoadScreen.wnd");

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = false;
        metadata.num_players = 2;

        update_multiplayer_start_position_buttons(
            &mut wm,
            "GameSpyLoadScreen.wnd",
            Some(&metadata),
            &[Some(0)],
        );

        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            assert!(window_hidden(
                &wm,
                &format!("GameSpyLoadScreen.wnd:ButtonMapStartPosition{slot}")
            ));
        }
    }

    #[test]
    fn game_info_context_preserves_original_slot_ids_and_apparent_colors() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        game_info.set_map("Maps/Test/Test.map".to_string());

        let mut alice = GameSlot::new();
        alice.set_state(SlotState::Player, "Alice".to_string(), 0x7F00_0001);
        alice.set_player_template(-1);
        alice.set_team_number(0);
        alice.set_color(2);
        alice.set_start_pos(1);

        let mut empty = GameSlot::new();
        empty.set_state(SlotState::Open, String::new(), 0);

        let mut bob = GameSlot::new();
        bob.set_state(SlotState::BrutalAI, String::new(), 0);
        bob.set_player_template(-1);
        bob.set_team_number(-1);
        bob.set_color(5);
        bob.set_start_pos(0);

        game_info.set_slot(0, alice);
        game_info.set_slot(1, empty);
        game_info.set_slot(2, bob);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(context.map_name.as_deref(), Some("Maps/Test/Test.map"));
        assert_eq!(context.start_positions[0], Some(1));
        assert_eq!(context.start_positions[1], None);
        assert_eq!(context.start_positions[2], Some(0));
        assert_eq!(context.local_player_name, "Alice");
        assert_eq!(context.local_team_number, 0);
        assert_eq!(context.slots.len(), 2);
        assert_eq!(context.slots[0].player_id, 0);
        assert_eq!(context.slots[0].apparent_color, Some(2));
        assert_eq!(context.slots[1].player_id, 2);
        assert_eq!(context.slots[1].team_number, -1);
        assert_eq!(context.slots[1].apparent_color, Some(5));
        assert!(context.slots[1].is_ai);
    }

    #[test]
    fn game_info_context_hides_map_when_local_pregame_slot_lacks_map_like_cpp() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0002);
        game_info.set_map("Maps/MissingLocal/MissingLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0002);
        local.set_player_template(-1);
        local.set_start_pos(1);
        local.set_map_availability(false);
        game_info.set_slot(1, local);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(game_info.get_local_slot_num(), 1);
        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.map_name, None);
        assert_eq!(context.start_positions[0], Some(0));
        assert_eq!(context.start_positions[1], Some(1));

        game_info.set_game_in_progress(true);
        let context = load_screen_init_context_from_game_info(&game_info);
        assert_eq!(
            context.map_name.as_deref(),
            Some("Maps/MissingLocal/MissingLocal.map")
        );
    }

    #[test]
    fn game_info_context_hides_map_when_local_slot_is_missing_like_cpp() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0009);
        game_info.set_map("Maps/NoLocal/NoLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(game_info.get_local_slot_num(), -1);
        assert_eq!(context.local_player_name, "Host");
        assert_eq!(context.map_name, None);
    }

    #[test]
    fn game_info_context_uses_local_template_for_general_presentation_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("GUI:AirFeatures", "Air Force General");
        Language::register_localized_string("CHALLENGE:AirName", "General Granger");

        {
            let mut store =
                game_engine::common::rts::player_template::get_player_template_store_mut();
            store.clear();
            let mut template = PlayerTemplate::new("FactionAirForceGeneral".to_string());
            template.display_name = "Air Force".to_string();
            template.features = "GUI:AirFeatures".to_string();
            template.load_screen_music = "Load_USA".to_string();
            store.add_template(template);
        }

        init_challenge_generals();
        {
            let mut generals = get_challenge_generals_mut().expect("challenge generals");
            let positions = generals.challenge_generals_mut();
            positions[0] = GeneralPersona::new();
            positions[0].set_player_template_name("FactionAirForceGeneral".to_string());
            positions[0].set_bio_name("CHALLENGE:AirName".to_string());
            positions[0].set_bio_portrait_large(Some("AirGeneralPortrait".to_string()));
        }

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0001);
        local.set_player_template(0);
        local.set_team_number(0);
        local.set_color(2);
        local.set_start_pos(0);
        game_info.set_slot(0, local);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.local_general_name, "General Granger");
        assert_eq!(context.local_general_features, "Air Force General");
        assert_eq!(
            context.local_general_portrait.as_deref(),
            Some("AirGeneralPortrait")
        );
        assert_eq!(context.local_load_screen_music, "Load_USA");

        game_engine::common::rts::player_template::get_player_template_store_mut().clear();
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_progress_bar_images_match_cpp_apparent_color_names() {
        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_ProgressCenter6".to_string())
        );

        let fallback = load_screen_slot("Player", "USA", 3, false, true);
        assert_eq!(multiplayer_progress_bar_image(&fallback), None);

        let invalid = load_screen_slot_with_color("Player", "USA", 3, Some(-1), false, true);
        assert_eq!(multiplayer_progress_bar_image(&invalid), None);
    }

    #[test]
    fn progress_fudge_matches_single_player_cpp_formula() {
        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert!((transformed_progress_percent(single, 0.0) - (30.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 100.0) - 100.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 150.0) - (180.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, -50.0) - (-20.0 / 1.3)).abs() < f32::EPSILON);

        let shell = descriptor_for_kind(LoadScreenKind::ShellGame);
        assert!((transformed_progress_percent(shell, 42.0) - 42.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(shell, 150.0) - 150.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_progress_window_uses_cpp_progress_message_range_rules() {
        let mut wm = WindowManager::new();
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 37.9);
        let progress = wm
            .find_window_by_name("SinglePlayerLoadScreen.wnd:ProgressLoad")
            .expect("progress");
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 138.4);
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", -4.2);
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );
    }

    #[test]
    fn update_load_screen_clears_cursor_tooltip_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| {
            mouse.set_cursor_tooltip("Stale tooltip".to_string(), Some(0), None, None);
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "Stale tooltip");
            assert!(!mouse.cursor_tooltip_state().is_tooltip_empty);
        });

        update_load_screen(LoadScreenKind::SinglePlayer, 50.0);

        with_mouse(|mouse| {
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
            assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
        });
    }

    #[test]
    fn single_player_campaign_images_match_cpp_side_mapping() {
        assert_eq!(
            single_player_campaign_images("USA"),
            Some(("MissionLoad_USA", "LoadingBar_ProgressCenter2"))
        );
        assert_eq!(
            single_player_campaign_images("gla"),
            Some(("MissionLoad_GLA", "LoadingBar_ProgressCenter3"))
        );
        assert_eq!(
            single_player_campaign_images("China"),
            Some(("MissionLoad_China", "LoadingBar_ProgressCenter1"))
        );
        assert_eq!(single_player_campaign_images("Challenge"), None);
    }

    #[test]
    fn multiplayer_local_general_faction_logos_match_cpp_fallbacks() {
        assert_eq!(
            multiplayer_local_general_faction_logo("USA", "MultiplayerLoadScreen.wnd"),
            Some("SAFactionLogoLg_US")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("FactionGLA", "MultiplayerLoadScreen.wnd"),
            Some("SUFactionLogoLg_GLA")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("China", "GameSpyLoadScreen.wnd"),
            Some("SNFactionLogo144_China")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("Random", "GameSpyLoadScreen.wnd"),
            None
        );
    }

    #[test]
    fn single_player_mission_text_fetches_cpp_labels() {
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("MISSION:Objective0", "Capture the base");
        Language::register_localized_string("MISSION:Objective2", "Hold position");
        Language::register_localized_string("UNIT:Ranger", "Ranger");
        Language::register_localized_string("UNIT:Humvee", "Humvee");
        Language::register_localized_string("MISSION:Location", "Northern sector");

        let mut mission = Mission::new();
        mission.mission_objectives_label[0] = "MISSION:Objective0".to_string();
        mission.mission_objectives_label[2] = "MISSION:Objective2".to_string();
        mission.unit_names[0] = "UNIT:Ranger".to_string();
        mission.unit_names[1] = "UNIT:Humvee".to_string();
        mission.location_name_label = "MISSION:Location".to_string();

        let text = single_player_mission_text(&mission);

        assert_eq!(text.objective_lines[0], "Capture the base");
        assert_eq!(text.objective_lines[1], "");
        assert_eq!(text.objective_lines[2], "Hold position");
        assert_eq!(text.unit_descriptions[0], "Ranger");
        assert_eq!(text.unit_descriptions[1], "Humvee");
        assert_eq!(text.unit_descriptions[2], "");
        assert_eq!(text.location, "Northern sector");

        with_single_player_load_screen_state(|state| {
            state.mission_text = text.clone();
            state.current_objective_line = 0;
            state.current_objective_width_offset = 0;
            state.current_objective_line_character = 0;
            state.finished_objective_text = false;
        });
        let cached = with_single_player_load_screen_state(|state| state.clone());
        assert_eq!(cached.mission_text.objective_lines[0], "Capture the base");
        assert_eq!(cached.current_objective_line, 0);
        assert_eq!(cached.current_objective_width_offset, 0);
        assert_eq!(cached.current_objective_line_character, 0);
        assert!(!cached.finished_objective_text);

        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_persona_text_matches_cpp_load_screen_fields() {
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        let mut generals = ChallengeGenerals::new();
        {
            let positions = generals.challenge_generals_mut();
            positions[0].set_campaign("ChallengeCampaign".to_string());
            positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
            positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
            positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
            positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
            positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
            positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
            positions[0].set_name_sound("PlayerNameSound".to_string());
            positions[0].set_taunt_sound_1("PlayerTaunt1".to_string());
            positions[0].set_taunt_sound_2("PlayerTaunt2".to_string());
            positions[0].set_taunt_sound_3("PlayerTaunt3".to_string());

            positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
            positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
            positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
            positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
            positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
            positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
            positions[1].set_name_sound("OpponentNameSound".to_string());
            positions[1].set_taunt_sound_1("OpponentTaunt1".to_string());
            positions[1].set_taunt_sound_2("OpponentTaunt2".to_string());
            positions[1].set_taunt_sound_3("OpponentTaunt3".to_string());
        }

        let (player, opponent) = challenge_persona_text_for_current_mission(
            "ChallengeCampaign",
            "CHALLENGE:OpponentName",
            &generals,
        )
        .expect("challenge personas");

        assert_eq!(player.big_name, "General Player");
        assert_eq!(player.name, "General Player");
        assert_eq!(player.rank, "General");
        assert_eq!(player.strategy, "Air superiority");
        assert_eq!(player.portrait_large.as_deref(), Some("PlayerPortrait"));
        assert_eq!(player.portrait_movie_left, "PlayerMovieLeft");
        assert_eq!(player.portrait_movie_right, "PlayerMovieRight");
        assert_eq!(player.name_sound, "PlayerNameSound");
        assert_eq!(
            player.taunt_sounds,
            ["PlayerTaunt1", "PlayerTaunt2", "PlayerTaunt3"]
        );

        assert_eq!(opponent.big_name, "General Opponent");
        assert_eq!(opponent.name, "General Opponent");
        assert_eq!(opponent.rank, "Prince");
        assert_eq!(opponent.strategy, "Ambush");
        assert_eq!(opponent.portrait_large.as_deref(), Some("OpponentPortrait"));
        assert_eq!(opponent.portrait_movie_left, "OpponentMovieLeft");
        assert_eq!(opponent.portrait_movie_right, "OpponentMovieRight");
        assert_eq!(opponent.name_sound, "OpponentNameSound");
        assert_eq!(
            opponent.taunt_sounds,
            ["OpponentTaunt1", "OpponentTaunt2", "OpponentTaunt3"]
        );

        Language::clear_localized_strings();
    }

    fn named_test_window(wm: &mut WindowManager, name: &str) {
        let window = wm.create_window(None, 0, 0, 100, 20).expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        let _ = window.hide(true);
    }

    fn named_progress_test_window(wm: &mut WindowManager, name: &str) {
        let window = wm.create_window(None, 0, 0, 100, 20).expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        window.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
            0, 0, 0, 100, 20,
        )));
        let _ = window.hide(true);
    }

    fn create_multiplayer_slot_windows(wm: &mut WindowManager, prefix: &str, count: usize) {
        for slot in 0..count {
            named_progress_test_window(wm, &format!("{prefix}:ProgressLoad{slot}"));
            let suffixes = if load_screen_has_team_windows(prefix) {
                &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
            } else {
                &["StaticTextPlayer", "StaticTextSide"][..]
            };
            for suffix in suffixes {
                named_test_window(wm, &format!("{prefix}:{suffix}{slot}"));
            }
        }
    }

    fn create_multiplayer_start_position_windows(wm: &mut WindowManager, prefix: &str) {
        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            let name = format!("{prefix}:ButtonMapStartPosition{slot}");
            named_test_window(wm, &name);
            wm.find_window_by_name(&name)
                .expect("start position button")
                .borrow_mut()
                .set_size(10, 10)
                .expect("button size");
        }
    }

    fn create_gamespy_slot_windows(wm: &mut WindowManager, count: usize) {
        for slot in 0..count {
            named_test_window(wm, &format!("GameSpyLoadScreen.wnd:WinPlayer{slot}"));
            for suffix in gamespy_stats_suffixes() {
                named_test_window(wm, &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"));
            }
        }
    }

    fn load_screen_slot(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_color(player_name, side_name, team_number, None, is_ai, visible)
    }

    fn load_screen_slot_with_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_text_color(
            player_name,
            side_name,
            team_number,
            apparent_color,
            None,
            is_ai,
            visible,
        )
    }

    fn load_screen_slot_with_text_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        apparent_text_color: Option<u32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        LoadScreenSlotInitContext {
            player_id: team_number,
            player_name: player_name.to_string(),
            side_name: side_name.to_string(),
            team_number,
            apparent_color,
            apparent_text_color,
            is_ai,
            visible,
        }
    }

    fn window_text(wm: &WindowManager, name: &str) -> String {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_text()
            .to_string()
    }

    fn window_text_color(wm: &WindowManager, name: &str) -> u32 {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_text_color()
    }

    fn window_hidden(wm: &WindowManager, name: &str) -> bool {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .is_hidden()
    }

    fn window_position(wm: &WindowManager, name: &str) -> (i32, i32) {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_position()
    }

    fn window_image_name(wm: &WindowManager, name: &str, index: usize) -> Option<String> {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_draw_data(index)?
            .image
            .map(|image| image.name)
    }

    fn window_status(wm: &WindowManager, name: &str) -> WindowStatus {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_status()
    }

    fn progress_value(name: &str) -> Option<f32> {
        with_window_manager(|wm| {
            let window = wm.find_window_by_name(name)?;
            let mut window = window.borrow_mut();
            Some(window.progress_bar_mut()?.value())
        })
    }

    fn reset_shell_game_first_load_for_tests(value: bool) {
        with_shell_game_first_load(|first_load| *first_load = value);
    }

    #[test]
    fn shell_game_first_load_matches_cpp_title_and_legal_state() {
        reset_shell_game_first_load_for_tests(true);
        let mut wm = WindowManager::new();
        let root = wm.create_window(None, 0, 0, 800, 600).expect("root");
        root.borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:StaticTextLegal");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:ProgressLoad");

        initialize_shell_game_windows(&mut wm);

        let root = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
            .expect("root");
        assert_eq!(
            root.borrow()
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("TitleScreen".to_string())
        );
        let legal = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal");
        assert!(!legal.borrow().is_hidden());
        let progress = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:ProgressLoad")
            .expect("progress");
        assert!(!progress.borrow().is_hidden());

        let mut second_wm = WindowManager::new();
        let second_root = second_wm.create_window(None, 0, 0, 800, 600).expect("root");
        second_root
            .borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut second_wm, "ShellGameLoadScreen.wnd:StaticTextLegal");

        initialize_shell_game_windows(&mut second_wm);

        let second_legal = second_wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal");
        assert!(second_legal.borrow().is_hidden());
        reset_shell_game_first_load_for_tests(true);
    }

    fn challenge_test_windows(wm: &mut WindowManager) {
        named_test_window(wm, "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen");
        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                    "ChallengeLoadScreen.wnd:PortraitMovieRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            named_test_window(wm, name);
        }
    }

    fn setup_current_challenge_for_tests(movie_label: &str) {
        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        init_challenge_generals();
        let mut generals = get_challenge_generals_mut().expect("challenge generals");
        let positions = generals.challenge_generals_mut();
        positions[0] = GeneralPersona::new();
        positions[0].set_campaign("challengecampaign".to_string());
        positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
        positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
        positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
        positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
        positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
        positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
        positions[0].set_name_sound("PlayerNameSound".to_string());

        positions[1] = GeneralPersona::new();
        positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
        positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
        positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
        positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
        positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
        positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
        positions[1].set_name_sound("OpponentNameSound".to_string());
        drop(generals);

        let mut manager = get_campaign_manager();
        {
            let campaign = manager.new_campaign("challengecampaign".to_string());
            campaign.first_mission = "mission1".to_string();
            campaign.is_challenge_campaign = true;
            let mission = campaign.new_mission("mission1".to_string());
            mission.general_name = "CHALLENGE:OpponentName".to_string();
            mission.movie_label = movie_label.to_string();
        }
        manager.set_campaign_and_mission("challengecampaign", "mission1");
    }

    fn cache_challenge_test_personas() {
        with_challenge_load_screen_state(|state| {
            *state = ChallengeLoadScreenState {
                player: Some(ChallengePersonaText {
                    big_name: "General Player".to_string(),
                    name: "General Player".to_string(),
                    rank: "General".to_string(),
                    strategy: "Air superiority".to_string(),
                    portrait_large: Some("PlayerPortrait".to_string()),
                    portrait_movie_left: "PlayerMovieLeft".to_string(),
                    portrait_movie_right: "PlayerMovieRight".to_string(),
                    name_sound: "PlayerNameSound".to_string(),
                    taunt_sounds: [
                        "PlayerTaunt1".to_string(),
                        "PlayerTaunt2".to_string(),
                        "PlayerTaunt3".to_string(),
                    ],
                }),
                opponent: Some(ChallengePersonaText {
                    big_name: "General Opponent".to_string(),
                    name: "General Opponent".to_string(),
                    rank: "Prince".to_string(),
                    strategy: "Ambush".to_string(),
                    portrait_large: Some("OpponentPortrait".to_string()),
                    portrait_movie_left: "OpponentMovieLeft".to_string(),
                    portrait_movie_right: "OpponentMovieRight".to_string(),
                    name_sound: "OpponentNameSound".to_string(),
                    taunt_sounds: [
                        "OpponentTaunt1".to_string(),
                        "OpponentTaunt2".to_string(),
                        "OpponentTaunt3".to_string(),
                    ],
                }),
                ..ChallengeLoadScreenState::default()
            };
        });
    }

    #[test]
    fn challenge_init_with_movie_waits_for_frame_activation_like_cpp_high_spec() {
        let _language_guard = lock_test_language();
        setup_current_challenge_for_tests("ChallengeIntro");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        initialize_challenge_windows(&mut wm);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        for _ in 0..FRAME_TITLES_START {
            update_challenge_load_screen_prelude(&mut wm);
        }

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_init_resets_window_video_manager_like_cpp() {
        let _language_guard = lock_test_language();
        setup_current_challenge_for_tests("ChallengeIntro");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);
        with_window_video_manager(|manager| manager.set_global_flags_for_tests(true, true));

        initialize_challenge_windows(&mut wm);

        let flags = with_window_video_manager(|manager| manager.global_flags_for_tests());
        assert_eq!(flags, (false, false));
        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_init_without_movie_uses_cpp_min_spec_final_reveal() {
        let _language_guard = lock_test_language();
        setup_current_challenge_for_tests("");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        initialize_challenge_windows(&mut wm);

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
                .expect("right strategy")
                .borrow()
                .get_text(),
            "Ambush"
        );
        let postlude_played = with_challenge_load_screen_state(|state| state.postlude_audio_played);
        assert!(postlude_played);

        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_postlude_audio_fires_once_and_selects_opponent_taunt() {
        cache_challenge_test_personas();

        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap()),
                0
            ),
            Some("OpponentTaunt1")
        );
        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap()),
                4
            ),
            Some("OpponentTaunt2")
        );
        let sparse_taunts = ChallengePersonaText {
            taunt_sounds: [
                String::new(),
                "SparseOpponentTaunt2".to_string(),
                String::new(),
            ],
            ..ChallengePersonaText::default()
        };
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 0), Some(""));
        assert_eq!(
            challenge_taunt_sound(&sparse_taunts, 1),
            Some("SparseOpponentTaunt2")
        );
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 2), Some(""));

        finish_challenge_load_screen_audio_postlude();
        let first = with_challenge_load_screen_state(|state| {
            (
                state.postlude_audio_played,
                state.high_spec_prelude_active,
                state.ambient_loop_handle,
            )
        });
        assert!(first.0);
        assert!(!first.1);

        finish_challenge_load_screen_audio_postlude();
        let second = with_challenge_load_screen_state(|state| state.ambient_loop_handle);
        assert_eq!(second, first.2);
    }

    #[test]
    fn challenge_frame_activation_matches_cpp_teletype_gates() {
        cache_challenge_test_personas();
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TITLES_START);
        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START);
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            let window = window.borrow();
            assert!(!window.is_hidden(), "{name}");
            assert_eq!(window.get_text(), "");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 1);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            ""
        );

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 2);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            "G"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "P"
        );
    }

    #[test]
    fn challenge_min_spec_activation_matches_cpp_final_reveal() {
        cache_challenge_test_personas();
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        activate_challenge_pieces_min_spec_windows(&mut wm);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BigNameEntryLeft")
                .expect("left big name")
                .borrow()
                .get_text(),
            "General Player"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "Prince"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
                .expect("right strategy")
                .borrow()
                .get_text(),
            "Ambush"
        );

        let left_portrait = wm
            .find_window_by_name("ChallengeLoadScreen.wnd:PortraitLeft")
            .expect("left portrait");
        let left_portrait = left_portrait.borrow();
        assert_eq!(
            left_portrait
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("PlayerPortrait".to_string())
        );
    }
}
