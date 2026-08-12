// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

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
// C++ runs the campaign/Challenge prelude synchronously before map work.  Rust
// must retain a finite escape hatch when a video backend never becomes ready;
// otherwise this non-event-loop pump could livelock the UI thread forever.
const LOAD_SCREEN_PRELUDE_MAX_PUMPS: usize = 4_096;
#[cfg(not(test))]
const LOAD_SCREEN_PRELUDE_MOVIE_IDLE_INTERVAL: Duration = Duration::from_millis(1);
#[cfg(test)]
const LOAD_SCREEN_PRELUDE_MOVIE_IDLE_INTERVAL: Duration = Duration::ZERO;
#[cfg(not(test))]
const LOAD_SCREEN_PRELUDE_MIN_SPEC_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(test)]
const LOAD_SCREEN_PRELUDE_MIN_SPEC_UPDATE_INTERVAL: Duration = Duration::ZERO;

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
    MapTransfer,
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
    pub shell_game_did_mem_pass: bool,
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
    pub has_map: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadScreenPreludeState {
    NotRequired,
    Movie,
    VoiceDelay,
    Complete,
    Failed,
    Skipped,
}

impl Default for LoadScreenPreludeState {
    fn default() -> Self {
        Self::NotRequired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadScreenPreludeOutcome {
    NotRequired,
    Complete,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadScreenPreludeStep {
    Pending(Duration),
    Finished(LoadScreenPreludeOutcome),
}

/// One observed advance of the authored Challenge background movie.  The
/// explicit completion bit preserves the final frame after WindowVideoManager
/// removes a one-shot movie from its active table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoadScreenMovieAdvance {
    frame_index: i32,
    frame_count: i32,
    completed: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct MapTransferLoadScreenState {
    player_lookup: [i32; MAX_LOAD_SCREEN_SLOTS],
    old_progress: [i32; MAX_LOAD_SCREEN_SLOTS],
    old_timeout: i32,
}

impl Default for MapTransferLoadScreenState {
    fn default() -> Self {
        Self {
            player_lookup: [-1; MAX_LOAD_SCREEN_SLOTS],
            old_progress: [-1; MAX_LOAD_SCREEN_SLOTS],
            old_timeout: 0,
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
    prelude_state: LoadScreenPreludeState,
    prelude_deadline: Option<Instant>,
    prelude_duration: Duration,
    movie_prelude_active: bool,
    movie_label: String,
    briefing_voice_played: bool,
    briefing_voice_handle: u32,
    ambient_loop_handle: u32,
}

static SINGLE_PLAYER_LOAD_SCREEN_STATE: OnceLock<Mutex<SinglePlayerLoadScreenState>> =
    OnceLock::new();
static SHELL_GAME_FIRST_LOAD: OnceLock<Mutex<bool>> = OnceLock::new();
static MULTIPLAYER_LOAD_SCREEN_STATE: OnceLock<Mutex<MultiplayerLoadScreenState>> = OnceLock::new();
static MAP_TRANSFER_LOAD_SCREEN_STATE: OnceLock<Mutex<MapTransferLoadScreenState>> =
    OnceLock::new();
static MAP_TRANSFER_LITEUPDATE_HOOK: OnceLock<Mutex<Option<MapTransferLiteupdateHook>>> =
    OnceLock::new();
static MULTIPLAYER_LOAD_PROGRESS_HOOK: OnceLock<Mutex<Option<MultiplayerLoadProgressHook>>> =
    OnceLock::new();
#[cfg(test)]
static LOAD_SCREEN_FINISH_UPDATE_HOOK: OnceLock<Mutex<Option<LoadScreenFinishUpdateHook>>> =
    OnceLock::new();
#[cfg(test)]
static SINGLE_PLAYER_MOVIE_PLAY_HOOK: OnceLock<Mutex<Option<SinglePlayerMoviePlayHook>>> =
    OnceLock::new();
#[cfg(test)]
static SINGLE_PLAYER_MOVIE_PLAYING_HOOK: OnceLock<Mutex<Option<SinglePlayerMoviePlayHook>>> =
    OnceLock::new();
#[cfg(test)]
static CHALLENGE_MOVIE_PLAY_HOOK: OnceLock<Mutex<Option<ChallengeMoviePlayHook>>> = OnceLock::new();
#[cfg(test)]
static CHALLENGE_MOVIE_ADVANCE_HOOK: OnceLock<Mutex<Option<ChallengeMovieAdvanceHook>>> =
    OnceLock::new();

type MapTransferLiteupdateHook = Arc<dyn Fn() + Send + Sync + 'static>;
type MultiplayerLoadProgressHook = Arc<dyn Fn(i32, i32) + Send + Sync + 'static>;
type LoadScreenPresentationPump = Rc<dyn Fn() + 'static>;
#[cfg(test)]
type LoadScreenFinishUpdateHook = Arc<dyn Fn() + Send + Sync + 'static>;
#[cfg(test)]
type SinglePlayerMoviePlayHook = Arc<dyn Fn(&str) -> bool + Send + Sync + 'static>;
#[cfg(test)]
type ChallengeMoviePlayHook = Arc<dyn Fn(&str) -> bool + Send + Sync + 'static>;
#[cfg(test)]
type ChallengeMovieAdvanceHook =
    Arc<dyn Fn(&str) -> Option<LoadScreenMovieAdvance> + Send + Sync + 'static>;

thread_local! {
    static LOAD_SCREEN_PRESENTATION_PUMP: RefCell<Option<LoadScreenPresentationPump>> =
        const { RefCell::new(None) };
}

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
    prelude_state: LoadScreenPreludeState,
    prelude_deadline: Option<Instant>,
    prelude_duration: Duration,
    background_movie_label: String,
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
            // This field predates campaign preload support, but it is the
            // shared C++ GameLODManager::didMemPass result for every load
            // screen that needs the high-spec movie gate.
            shell_game_did_mem_pass: game_engine::common::game_lod::did_mem_pass(),
            map_name: None,
            start_positions: Vec::new(),
            slots: Vec::new(),
        }
    }
}
