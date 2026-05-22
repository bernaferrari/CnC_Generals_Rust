//! C++ parity wrapper for `LoadScreen.cpp`.

pub use super::loading_screen::*;

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;

use super::campaign_manager::{
    get_campaign_manager, Mission, MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES,
};
use super::challenge_generals::{
    get_challenge_generals, init_challenge_generals, ChallengeGenerals, GeneralPersona,
};
use super::game_window::Image as WindowImage;
use super::window_video_manager::{with_window_video_manager, WindowVideoPlayType};
use super::{with_window_manager, WindowManager, WindowStatus};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use std::sync::{Mutex, OnceLock};

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
    pub local_team_number: i32,
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
            local_team_number: 0,
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
    let raw_percent = raw_percent.clamp(0.0, 100.0);
    if descriptor.uses_progress_fudge {
        ((raw_percent + FRAME_FUDGE_ADD) / FRAME_FUDGE_SCALE).clamp(0.0, 100.0)
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

pub fn reset_load_screen(kind: LoadScreenKind) {
    let descriptor = descriptor_for_kind(kind);
    with_window_manager(|wm| {
        if let Some(root) = wm.find_window_by_name(descriptor.root) {
            let _ = wm.destroy_window(root);
            wm.flush_destroy_queue();
        }
    });
}

pub fn update_load_screen(kind: LoadScreenKind, raw_percent: f32) {
    let descriptor = descriptor_for_kind(kind);
    let percent = transformed_progress_percent(descriptor, raw_percent);
    with_window_manager(|wm| {
        set_progress_window(wm, descriptor.primary_progress, percent);
        if kind == LoadScreenKind::SinglePlayer {
            set_window_text(
                wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &format!("{}%", percent as i32),
            );
        }
    });
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
    }
}

fn with_shell_game_first_load<R>(f: impl FnOnce(&mut bool) -> R) -> R {
    let state = SHELL_GAME_FIRST_LOAD.get_or_init(|| Mutex::new(true));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
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
        with_challenge_load_screen_state(|state| {
            state.player = Some(player.clone());
            state.opponent = Some(opponent.clone());
        });
        if let Some(image) = player.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitLeft", 0, image, true);
        }
        if let Some(image) = opponent.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitRight", 0, image, true);
        }
        activate_challenge_pieces_min_spec_windows(wm);
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
    if event_name.is_empty() {
        return;
    }
    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new(event_name);
        audio.add_audio_event(&event);
    }
}

fn initialize_multiplayer_windows(
    wm: &mut WindowManager,
    prefix: &str,
    context: &LoadScreenInitContext,
) {
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralFeatures"),
        &context.local_side_name,
    );
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralName"),
        &context.local_side_name,
    );

    set_window_text(
        wm,
        &format!("{prefix}:StaticTextPlayer0"),
        &context.local_player_name,
    );
    set_window_text(
        wm,
        &format!("{prefix}:StaticTextSide0"),
        &context.local_side_name,
    );
    set_window_text(
        wm,
        &format!("{prefix}:StaticTextTeam0"),
        &format!("Team:{}", context.local_team_number.saturating_add(1)),
    );

    for slot in 1..MAX_LOAD_SCREEN_SLOTS {
        for suffix in [
            "ProgressLoad",
            "StaticTextPlayer",
            "StaticTextSide",
            "StaticTextTeam",
        ] {
            hide_window(wm, &format!("{prefix}:{suffix}{slot}"), true);
        }
    }
}

fn initialize_gamespy_windows(wm: &mut WindowManager, context: &LoadScreenInitContext) {
    initialize_multiplayer_windows(wm, "GameSpyLoadScreen.wnd", context);

    for slot in 1..MAX_LOAD_SCREEN_SLOTS {
        for suffix in [
            "WinPlayer",
            "StaticTextTotalDisconnects",
            "StaticTextWinLoss",
            "WinRank",
            "WinOfficer",
        ] {
            hide_window(wm, &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"), true);
        }
    }
}

fn set_progress_window(wm: &mut WindowManager, name: &str, percent: f32) {
    if let Some(window) = wm.find_window_by_name(name) {
        let mut window = window.borrow_mut();
        if let Some(progress) = window.progress_bar_mut() {
            progress.set_progress(percent);
        }
    }
}

fn set_window_text(wm: &mut WindowManager, name: &str, text: &str) {
    if let Some(window) = wm.find_window_by_name(name) {
        let _ = window.borrow_mut().set_text(text);
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::language::Language;
    use std::sync::{Mutex, OnceLock};

    static TEST_LANGUAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_test_language() -> std::sync::MutexGuard<'static, ()> {
        TEST_LANGUAGE_LOCK
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
    fn progress_fudge_matches_single_player_cpp_formula() {
        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert!((transformed_progress_percent(single, 0.0) - (30.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 100.0) - 100.0).abs() < f32::EPSILON);

        let shell = descriptor_for_kind(LoadScreenKind::ShellGame);
        assert!((transformed_progress_percent(shell, 42.0) - 42.0).abs() < f32::EPSILON);
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
