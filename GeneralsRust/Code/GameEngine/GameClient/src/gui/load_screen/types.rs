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
pub(crate) const LOAD_SCREEN_PRELUDE_MAX_PUMPS: usize = 4_096;
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

// Optional external progress callbacks retain their existing adapter boundary.
// Per-screen mutable state is owned by LoadScreenRuntime below.
thread_local! {
    static MAP_TRANSFER_LITEUPDATE_HOOK: RefCell<Option<MapTransferLiteupdateHook>> =
        const { RefCell::new(None) };
    static MULTIPLAYER_LOAD_PROGRESS_HOOK: RefCell<Option<MultiplayerLoadProgressHook>> =
        const { RefCell::new(None) };
    #[cfg(test)]
    static LOAD_SCREEN_FINISH_UPDATE_HOOK: RefCell<Option<LoadScreenFinishUpdateHook>> =
        const { RefCell::new(None) };
    #[cfg(test)]
    static SINGLE_PLAYER_MOVIE_PLAY_HOOK: RefCell<Option<SinglePlayerMoviePlayHook>> =
        const { RefCell::new(None) };
    #[cfg(test)]
    static SINGLE_PLAYER_MOVIE_PLAYING_HOOK: RefCell<Option<SinglePlayerMoviePlayHook>> =
        const { RefCell::new(None) };
    #[cfg(test)]
    static CHALLENGE_MOVIE_PLAY_HOOK: RefCell<Option<ChallengeMoviePlayHook>> =
        const { RefCell::new(None) };
    #[cfg(test)]
    static CHALLENGE_MOVIE_ADVANCE_HOOK: RefCell<Option<ChallengeMovieAdvanceHook>> =
        const { RefCell::new(None) };
}

type MapTransferLiteupdateHook = Arc<dyn Fn() + Send + Sync + 'static>;
type MultiplayerLoadProgressHook = Arc<dyn Fn(i32, i32) + Send + Sync + 'static>;
/// Explicit bridge from one load-screen operation to its receiving display.
/// The weak handle keeps the GameClient as the display owner while retaining
/// the display mutex required by native draw/update callers.
/// Device/presentation capability only. All mutable UI lives in the operation.
pub struct LoadScreenPresentationPump<'a> {
    recipient: LoadScreenPresentationRecipient<'a>,
    mouse: Option<crate::core::subsystems::MouseHandle>,
}
enum LoadScreenPresentationRecipient<'a> {
    Display(Option<std::sync::Weak<std::sync::Mutex<crate::display::display::Display>>>),
    Native(&'a mut dyn for<'draw> FnMut(LoadScreenDrawServices<'draw>) -> Result<(), String>),
}
pub struct LoadScreenDrawServices<'a> {
    pub ui: Option<&'a crate::core::subsystems::InGameUISubsystem>,
    pub client_frame: u32,
    pub display: &'a mut crate::core::script_action_handler::ScriptDisplayReceiver,
    pub renderer: Option<&'a mut crate::gui::UIRenderer>,
    pub windows: &'a crate::gui::window_manager::WindowManagerOwner,
    pub ime: &'a crate::gui::ime_manager::ImeManager,
}
impl<'a> LoadScreenPresentationPump<'a> {
    pub fn for_display(
        display: Option<&std::sync::Arc<std::sync::Mutex<crate::display::display::Display>>>,
    ) -> Self {
        Self {
            recipient: LoadScreenPresentationRecipient::Display(
                display.map(std::sync::Arc::downgrade),
            ),
            mouse: None,
        }
    }
    pub fn for_native(
        present: &'a mut dyn for<'draw> FnMut(LoadScreenDrawServices<'draw>) -> Result<(), String>,
    ) -> Self {
        Self {
            recipient: LoadScreenPresentationRecipient::Native(present),
            mouse: None,
        }
    }
    pub fn with_mouse(mut self, mouse: Option<crate::core::subsystems::MouseHandle>) -> Self {
        self.mouse = mouse;
        self
    }
    pub fn clear_cursor_tooltip(&mut self) -> Result<(), String> {
        if let Some(mouse) = self.mouse.as_ref() {
            mouse
                .lock()
                .map_err(|_| "load-screen receiving mouse poisoned".to_owned())?
                .set_cursor_tooltip(String::new(), None, None, None);
        }
        Ok(())
    }
    pub fn pump(&mut self, operation: &mut LoadScreenOperation<'_>) -> Result<(), String> {
        operation.complete_callbacks();
        match &mut self.recipient {
            LoadScreenPresentationRecipient::Native(present) => {
                operation.with_draw_services(|services| present(services))
            }
            LoadScreenPresentationRecipient::Display(display) => {
                let Some(display) = display.as_ref().and_then(std::sync::Weak::upgrade) else {
                    #[cfg(test)]
                    {
                        pump_load_screen_presentation_test_hook();
                        return Ok(());
                    }
                    #[cfg(not(test))]
                    return Err("load screen has no receiving display presentation service".into());
                };
                operation.with_draw_services(|services| {
                    let mut display = display
                        .lock()
                        .map_err(|_| "load-screen display poisoned".to_owned())?;
                    display
                        .update_receiving(services.display.fx_mut())
                        .map_err(|error| error.to_string())?;
                    display
                        .draw(
                            services.display.fx_mut(),
                            services.renderer,
                            Some(services.windows),
                            services.ui,
                            services.client_frame,
                        )
                        .map_err(|error| error.to_string())
                })
            }
        }
    }
}
/// Historical API name now denotes the same single mutable operation.
pub type LoadScreenOwner<'a> = LoadScreenOperation<'a>;

#[cfg(test)]
pub type LoadScreenPresentationPumpTestHook = Rc<dyn Fn() + 'static>;
#[cfg(test)]
type LoadScreenFinishUpdateHook = Arc<dyn Fn() + Send + Sync + 'static>;
#[cfg(test)]
type SinglePlayerMoviePlayHook = Arc<dyn Fn(&str) -> bool + Send + Sync + 'static>;
#[cfg(test)]
type ChallengeMoviePlayHook = Arc<dyn Fn(&str) -> bool + Send + Sync + 'static>;
#[cfg(test)]
type ChallengeMovieAdvanceHook =
    Arc<dyn Fn(&str) -> Option<LoadScreenMovieAdvance> + Send + Sync + 'static>;

#[cfg(test)]
thread_local! {
    static LOAD_SCREEN_PRESENTATION_PUMP_TEST_HOOK: RefCell<Option<LoadScreenPresentationPumpTestHook>> =
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

/// Mutable loading presentation belongs to one client, independent of its thread.
#[derive(Debug, Default)]
pub struct LoadScreenRuntime {
    single_player: SinglePlayerLoadScreenState,
    multiplayer: MultiplayerLoadScreenState,
    map_transfer: MapTransferLoadScreenState,
    challenge: ChallengeLoadScreenState,
}

/// One bounded loading operation borrows its client's state and window streams.
pub struct LoadScreenOperation<'a> {
    pub state: &'a mut LoadScreenRuntime,
    pub windows: &'a WindowManagerOwner,
    pub(crate) first_load: &'a mut bool,
    pub(crate) effects: crate::gui::game_window::WindowEventEffects<'a>,
    pub(crate) renderer: Option<&'a mut crate::gui::UIRenderer>,
    pub(crate) message_stream: &'a mut crate::message_stream::message_stream::MessageStream,
}
impl LoadScreenOperation<'_> {
    pub fn runtime_mut(&mut self) -> &mut Self {
        self
    }
    pub fn windows(&self) -> &WindowManagerOwner {
        self.windows
    }
    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }
    pub fn videos(&mut self) -> &mut super::window_video_manager::WindowVideoManager {
        self.effects
            .window_videos
            .as_deref_mut()
            .expect("load operation requires receiving loading videos")
    }
    fn campaign(&self) -> &crate::gui::campaign_manager::CampaignManager {
        self.effects
            .campaign_manager
            .as_deref()
            .expect("load operation requires receiving campaign")
    }
    pub fn init(
        &mut self,
        kind: LoadScreenKind,
        context: &LoadScreenInitContext,
        pump: &mut LoadScreenPresentationPump<'_>,
    ) -> Result<bool, String> {
        init_load_screen_with_state(kind, context, Some(pump), self)
    }
    pub(crate) fn complete_callbacks(&mut self) {
        self.windows.complete_event_callbacks(&mut self.effects);
        for message in self.effects.emitted_messages.drain(..) {
            self.message_stream.append_message_value(message);
        }
    }
    fn with_draw_services<R>(
        &mut self,
        operation: impl FnOnce(LoadScreenDrawServices<'_>) -> R,
    ) -> R {
        operation(LoadScreenDrawServices {
            ui: self.effects.ui.as_deref(),
            client_frame: self
                .effects
                .client_frame
                .expect("loading requires receiving client frame"),
            display: self
                .effects
                .script_display
                .as_deref_mut()
                .expect("load operation requires receiving display state"),
            renderer: self.renderer.as_deref_mut(),
            windows: self.windows,
            ime: self
                .effects
                .ime
                .as_deref()
                .expect("load operation requires receiving IME"),
        })
    }
}
impl Drop for LoadScreenOperation<'_> {
    fn drop(&mut self) {
        for message in self.effects.emitted_messages.drain(..) {
            self.message_stream.append_message_value(message);
        }
    }
}
