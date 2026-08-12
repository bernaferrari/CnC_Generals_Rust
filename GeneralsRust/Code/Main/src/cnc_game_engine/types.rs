#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

pub(super) const DEFAULT_SKIRMISH_MAP: &str = "Defcon6";
pub(super) const DEFAULT_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
pub(super) const DEFAULT_VIEW_NEAR_CLIP: f32 = 1.0;
pub(super) const DEFAULT_LOADING_PHASE: &str = "Loading assets...";

// Window names from ShellGameLoadScreen.wnd (C++ parity: winCreateFromScript)
pub(super) const LOAD_SCREEN_ROOT: &str = "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen";
pub(super) const LOAD_SCREEN_PROGRESS: &str = "ShellGameLoadScreen.wnd:ProgressLoad";

pub(super) fn pack_ui_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}
pub(super) const DEFAULT_VIEW_FAR_CLIP: f32 = 20_000.0;

pub(super) fn should_keep_logic_running_while_iconic(mode: GameMode) -> bool {
    matches!(
        mode,
        GameMode::Multiplayer | GameMode::Lan | GameMode::Internet
    )
}

pub(super) fn query_window_is_iconic(window: &Window, fallback: bool) -> bool {
    let size = window.inner_size();
    let zero_sized = size.width == 0 || size.height == 0;
    window.is_minimized().unwrap_or(fallback || zero_sized) || zero_sized
}

pub(super) fn update_iconic_state_and_wake_audio(window: &Window, minimized: &mut bool) {
    let was_minimized = *minimized;
    *minimized = query_window_is_iconic(window, *minimized);

    if was_minimized && !*minimized {
        info!("Window exited iconic/minimized state");
        with_subsystem_mut::<AudioManagerSubsystem, _>(|audio| {
            audio.wake_after_iconic_return();
        });
    } else if !was_minimized && *minimized {
        info!("Window entered iconic/minimized state");
    }
}

pub(super) fn should_exit_for_smoke_test(
    smoke_test: bool,
    state: GameState,
    startup_progress: f32,
    exiting_pending: bool,
) -> bool {
    smoke_test && matches!(state, GameState::Menu) && startup_progress >= 1.0 && !exiting_pending
}

#[cfg(feature = "internal")]
pub mod parity_test_support {
    use super::GameState;
    use crate::ui::Screen;

    /// Lightweight state-machine model used by parity tests.
    ///
    /// The real engine constructor is too heavy for fast integration tests, so this
    /// harness mirrors the transition side effects that matter for startup, match
    /// start, exit-to-menu, and quit deduplication coverage.
    #[derive(Debug, Clone)]
    pub struct StateMachineParityHarness {
        pub(crate) current_state: GameState,
        pub(crate) pending_state: Option<GameState>,
        pub(crate) ui_screen: Option<Screen>,
        pub(crate) game_paused: bool,
        pub(crate) game_logic_paused: bool,
        pub(crate) match_over: bool,
        pub(crate) victory_summary_present: bool,
        pub(crate) selected_objects: Vec<u32>,
        pub(crate) quit_requests_emitted: usize,
        pub(crate) menu_world_frames_rendered: u32,
    }

    impl Default for StateMachineParityHarness {
        fn default() -> Self {
            Self {
                current_state: GameState::Menu,
                pending_state: None,
                ui_screen: Some(Screen::MainMenu),
                game_paused: false,
                game_logic_paused: false,
                match_over: false,
                victory_summary_present: false,
                selected_objects: Vec::new(),
                quit_requests_emitted: 0,
                menu_world_frames_rendered: 0,
            }
        }
    }

    impl StateMachineParityHarness {
        pub fn current_state(&self) -> GameState {
            self.current_state
        }

        pub fn pending_state(&self) -> Option<GameState> {
            self.pending_state
        }

        pub fn ui_screen(&self) -> Option<Screen> {
            self.ui_screen
        }

        pub fn game_paused(&self) -> bool {
            self.game_paused
        }

        pub fn game_logic_paused(&self) -> bool {
            self.game_logic_paused
        }

        pub fn match_over(&self) -> bool {
            self.match_over
        }

        pub fn victory_summary_present(&self) -> bool {
            self.victory_summary_present
        }

        pub fn selected_objects(&self) -> &[u32] {
            &self.selected_objects
        }

        pub fn quit_requests_emitted(&self) -> usize {
            self.quit_requests_emitted
        }

        pub fn set_loading_state(&mut self) {
            self.current_state = GameState::Loading;
            self.pending_state = None;
            self.ui_screen = Some(Screen::Loading);
        }

        pub fn set_dirty_play_state(&mut self) {
            self.current_state = GameState::InGame;
            self.pending_state = None;
            self.ui_screen = Some(Screen::GameHUD);
            self.game_paused = true;
            self.game_logic_paused = true;
            self.match_over = true;
            self.victory_summary_present = true;
            self.selected_objects = vec![101, 202, 303];
        }

        pub fn complete_startup_loading_to_menu(&mut self) {
            self.transition_to_state(GameState::Menu);
        }

        pub fn complete_new_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn complete_load_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn return_to_main_menu_after_match(&mut self) {
            self.selected_objects.clear();
            self.game_paused = false;
            self.game_logic_paused = false;
            self.match_over = false;
            self.victory_summary_present = false;
            self.pending_state = None;
            self.transition_to_state(GameState::Menu);
        }

        pub fn request_quit(&mut self) -> bool {
            if self.current_state == GameState::Exiting
                || self.pending_state == Some(GameState::Exiting)
            {
                return false;
            }

            self.pending_state = Some(GameState::Exiting);
            self.quit_requests_emitted = self.quit_requests_emitted.saturating_add(1);
            true
        }

        pub fn apply_pending_state_change(&mut self) {
            if let Some(new_state) = self.pending_state.take() {
                self.transition_to_state(new_state);
            }
        }

        fn transition_to_state(&mut self, new_state: GameState) {
            match new_state {
                GameState::Initializing => {
                    self.ui_screen = Some(Screen::Loading);
                }
                GameState::Menu => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::MainMenu);
                    self.menu_world_frames_rendered = 0;
                }
                GameState::Loading => {
                    self.ui_screen = Some(Screen::Loading);
                }
                GameState::InGame => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::GameHUD);
                }
                GameState::Paused => {
                    self.game_paused = true;
                    self.game_logic_paused = true;
                    self.ui_screen = Some(Screen::PauseMenu);
                }
                GameState::Victory | GameState::Defeat => {
                    self.game_paused = true;
                    self.game_logic_paused = true;
                    self.match_over = true;
                    self.victory_summary_present = true;
                    self.ui_screen = Some(Screen::GameHUD);
                }
                GameState::Exiting => {
                    self.ui_screen = None;
                }
            }

            self.current_state = new_state;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScriptCameraShaker {
    pub(crate) epicenter: Vec3,
    pub(crate) radius: f32,
    pub(crate) duration_seconds: f32,
    pub(crate) elapsed_seconds: f32,
    pub(crate) amplitude_degrees: f32,
    pub(crate) phase: f32,
    pub(crate) frequency_hz: f32,
}

impl ScriptCameraShaker {
    pub(super) fn new(
        epicenter: Vec3,
        radius: f32,
        duration_seconds: f32,
        amplitude_degrees: f32,
    ) -> Self {
        // Deterministic phase/frequency seed from shaker parameters.
        let seed = (epicenter.x * 0.013
            + epicenter.y * 0.021
            + epicenter.z * 0.034
            + amplitude_degrees * 0.055)
            .sin();
        let normalized = ((seed * 43_758.547).fract()).abs();
        Self {
            epicenter,
            radius: radius.max(0.01),
            duration_seconds: duration_seconds.max(0.01),
            elapsed_seconds: 0.0,
            amplitude_degrees,
            phase: normalized * TAU,
            frequency_hz: 2.0 + normalized * 4.0,
        }
    }
}

pub(crate) struct StartupLoadResult {
    pub(crate) game_logic: GameLogic,
    pub(crate) loaded_map_name: Option<String>,
    pub(crate) start_in_menu: bool,
    pub(crate) map_requested_from_cli: bool,
    pub(crate) replay_requested: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StartupNewGameDispatch {
    pub(crate) game_mode_code: i32,
    pub(crate) game_mode: GameMode,
    pub(crate) difficulty_code: i32,
    pub(crate) difficulty: GameDifficulty,
    pub(crate) rank_points: i32,
    pub(crate) max_fps: Option<i32>,
}

/// Map-click command residual armed by ControlBar buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingMapCommand {
    AttackMove,
    /// C++ GuardMode carried from the arming command button.
    Guard(crate::game_logic::GuardMode),
    SetRallyPoint,
    /// Chinook combat drop residual awaiting map click.
    CombatDrop,
    /// Armed superweapon / special power residual awaiting map click.
    SpecialPower(crate::command_system::SpecialPowerType),
    /// A `FIRE_WEAPON` CommandButton retains its exact selected weapon slot
    /// until the player chooses an object or map position.
    Weapon(crate::command_system::WeaponSlot),
    /// Retail PLACE_BEACON residual awaiting map click.
    PlaceBeacon,
    /// Unit special-ability residual awaiting object/map click.
    UnitAbility(PendingUnitAbility),
}

/// ControlBar unit ability that needs a target click residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingUnitAbility {
    Hijack,
    Sabotage,
    CaptureBuilding,
    SnipeVehicle,
    PlantTimedDemoCharge,
    PlantRemoteDemoCharge,
    StealCashHack,
    DisableVehicleHack,
    HackerDisableBuilding,
    DisguiseAsVehicle,
    PlantBoobyTrap,
    ConvertToCarbomb,
    /// Dozer/Worker repair residual awaiting damaged structure click.
    Repair,
}

/// Evidence for retail windowed sit-through (`wnd_widget_tree_nav` /
/// interactive gameplay). Latched **only** via
/// [`CnCGameEngine::handle_mouse_button_input`] (physical winit `MouseInput` or
/// winit-equivalent inject that re-enters that path after a real gadget hit /
/// RMB release with selection).
///
/// Host control cmds must **not** call `note_menu_wnd_click` /
/// `note_gameplay_order` directly. Scripted `drive_os_wnd_*` and headless
/// soft UI cannot manufacture this evidence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct InteractivePlayabilityEvidence {
    /// A physical left click was hit-tested and consumed by a visible shell WND
    /// widget while the engine was in its menu state.
    pub(crate) menu_wnd_click: bool,
    /// That click (or a later one) hit a MainMenu → Skirmish / Start gadget,
    /// not Parent/Ruler/Options chrome.
    pub(crate) skirmish_path: bool,
    /// That menu interaction subsequently started an offline match through the
    /// normal `start_game_from_ui` authority path.
    pub(crate) match_started_from_menu_wnd: bool,
    /// A physical context-click issued an order after that match began.  Selection
    /// alone is intentionally insufficient: this proves a player can command a
    /// unit, not merely move the pointer over the HUD.
    pub(crate) gameplay_order: bool,
    /// A physical Control Bar `DozerConstruct` button was accepted for a live
    /// local dozer/worker in a visible offline match.  This is deliberately
    /// distinct from `CommandSourceType::FromUser`: injected WND input also
    /// uses that legacy source type and must not satisfy this proof.
    pub(crate) physical_control_bar_construct_armed: bool,
    /// A physical Control Bar production request was accepted after the above
    /// construct arm in the same session.  This is the narrow build-and-
    /// produce acceptance condition, not a broad runtime-host command flag.
    pub(crate) physical_build_and_produce: bool,
    /// A carrier selected by a confirmed physical Gather order subsequently
    /// deposited a positive carried-supply amount for the local player in a
    /// visible offline match. Passive income and untracked carriers cannot
    /// advance this proof.
    pub(crate) physical_gather_resources: bool,
    /// An explicit PopupSaveLoad save confirmation completed through Main's
    /// snapshot authority after a real physical WND mouse event in a visible
    /// offline match.
    pub(crate) physical_popup_save_confirmation_succeeded: bool,
    /// A physical PopupSaveLoad load confirmation completed after the physical
    /// save confirmation above.  This proves a player can save, load, and
    /// continue through the real Rust-owned popup/authority path.
    pub(crate) physical_save_load_continue: bool,
}

impl InteractivePlayabilityEvidence {
    pub(super) fn note_menu_wnd_click(
        &mut self,
        windowed: bool,
        wnd_consumed: bool,
        hit_widget: bool,
    ) {
        if windowed && wnd_consumed && hit_widget {
            self.menu_wnd_click = true;
            log::info!(
                "InteractivePlayabilityEvidence: latched menu_wnd_click (windowed={windowed} consumed={wnd_consumed} hit={hit_widget})"
            );
        } else {
            log::debug!(
                "InteractivePlayabilityEvidence: menu_wnd_click miss windowed={windowed} consumed={wnd_consumed} hit={hit_widget}"
            );
        }
    }

    pub(super) fn note_skirmish_path_gadget(&mut self, windowed: bool, gadget_name: &str) {
        if windowed
            && crate::executable_smoke::ExecutableSmokeResult::wnd_nav_gadget_is_skirmish_path(
                gadget_name,
            )
        {
            self.skirmish_path = true;
        }
    }

    pub(super) fn note_offline_match_started(&mut self, was_menu: bool, offline_mode: bool) {
        if self.menu_wnd_click && self.skirmish_path && was_menu && offline_mode {
            self.match_started_from_menu_wnd = true;
        }
    }

    pub(super) fn note_gameplay_order(&mut self, windowed: bool, had_selection: bool) {
        if windowed && self.match_started_from_menu_wnd && had_selection {
            self.gameplay_order = true;
        }
    }

    /// Record an already-validated physical DozerConstruct arm.
    ///
    /// Callers must prove the input was a real OS mouse event and that the
    /// selected source is a local live dozer/worker before reaching this
    /// method.  Keeping those authority checks outside this sticky evidence
    /// type avoids teaching it to infer gameplay state from UI labels.
    pub(super) fn note_control_bar_construct_arm(&mut self, physical: bool) {
        if physical {
            self.physical_control_bar_construct_armed = true;
        }
    }

    /// Record an already-validated physical production queue request.
    ///
    /// Ordering is intentional: a physical production request before a valid
    /// physical construct arm cannot satisfy the build-and-produce condition.
    pub(super) fn note_control_bar_production(&mut self, physical: bool) {
        if physical && self.physical_control_bar_construct_armed {
            self.physical_build_and_produce = true;
        }
    }

    /// Whether this session has completed the physical Control Bar
    /// build-and-produce proof.
    pub(super) fn build_and_produce_complete(self) -> bool {
        self.physical_build_and_produce
    }

    /// Record an already-validated physical supply drop-off.
    ///
    /// The caller must have matched the drop-off to a carrier from an accepted
    /// physical Gather command, verified a positive carried-supply amount, and
    /// verified the visible offline/local-player conditions. Keeping that
    /// gameplay authority outside this sticky evidence type prevents passive or
    /// runtime-host income from being inferred as physical input.
    pub(super) fn note_physical_gather_resources(&mut self, physical: bool) {
        if physical {
            self.physical_gather_resources = true;
        }
    }

    /// Whether this session has completed the physical Gather → drop-off proof.
    pub(super) fn gather_resources_complete(self) -> bool {
        self.physical_gather_resources
    }

    /// Record a Main-authority save success that already passed the physical
    /// Popup confirmation and visible-offline-match checks.
    pub(super) fn note_popup_save_confirmation_succeeded(&mut self, physical: bool) {
        if physical {
            self.physical_popup_save_confirmation_succeeded = true;
        }
    }

    /// Record a Main-authority load success after a physical Popup confirmation.
    ///
    /// Ordering is intentional: a physical load alone, or one following a
    /// runtime-host/injected save, cannot claim save/load continuation.
    pub(super) fn note_popup_load_confirmation_succeeded(&mut self, physical: bool) {
        if physical && self.physical_popup_save_confirmation_succeeded {
            self.physical_save_load_continue = true;
        }
    }

    /// Whether this session has completed physical PopupSaveLoad save → load
    /// continuation through Main's snapshot authority.
    pub(super) fn save_load_continue_complete(self) -> bool {
        self.physical_save_load_continue
    }

    /// The WND-navigation component of the retail claim requires the complete
    /// menu-to-match chain, rather than a broad sticky "some gadget was hovered"
    /// bit from the GUI singleton.
    pub(super) fn wnd_menu_to_match_complete(self) -> bool {
        self.menu_wnd_click && self.skirmish_path && self.match_started_from_menu_wnd
    }

    pub(super) fn gameplay_complete(self) -> bool {
        self.match_started_from_menu_wnd && self.gameplay_order
    }
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod evidence_tests;

/// Main C&C game engine with full RTS functionality - restructured to match C++ SAGE architecture
pub struct CnCGameEngine {
    pub(crate) window: Arc<Window>,
    #[allow(dead_code)] // C++ parity: stored for future command-line query access
    pub(crate) command_line: Arc<CommandLineArgs>,

    // C++ SAGE equivalent rendering subsystems
    pub(crate) graphics_system: GraphicsSystem,
    pub(crate) render_pipeline: RenderPipeline,

    // Platform message handling
    pub(crate) message_processor: WindowMessageProcessor,

    // Audio system
    #[allow(dead_code)] // C++ parity: audio stream handle kept alive to prevent drop
    pub(crate) audio_output: Option<OutputStream>,
    pub(crate) audio_handle: Option<OutputStreamHandle>,
    pub(crate) background_music: Option<Sink>,
    pub(crate) sound_effects: Vec<Sink>,
    pub(crate) ui_sound_cache: HashMap<String, Arc<[u8]>>,

    // Game state machine - matches C++ GameEngine m_quitting and state management
    pub(crate) current_state: GameState,
    pub(crate) pending_state: Option<GameState>,
    pub(crate) startup_load_state: StartupLoadState,
    pub(crate) startup_target_state: Option<GameState>,
    pub(crate) startup_start_in_menu: bool,
    pub(crate) last_loading_title_update: Option<Instant>,
    pub(crate) startup_last_reported_progress: f32,
    pub(crate) startup_loading_phase: String,
    pub(crate) startup_last_progress_change_at: Instant,
    pub(crate) startup_last_stall_warning_at: Option<Instant>,
    pub(crate) startup_stall_events: u32,
    pub(crate) startup_max_stall_duration: Duration,
    pub(crate) startup_health_summary_logged: bool,
    pub(crate) last_caustic_warmup_attempt: Option<Instant>,
    pub(crate) loading_overlay_active: bool,
    #[cfg(feature = "game_client")]
    pub(crate) active_load_screen: Option<game_client::gui::load_screen::LoadScreenKind>,
    pub(crate) shell_menu_active: bool, // C++ parity: Shell::push("Menus/MainMenu.wnd") / Shell::pop()

    // Game client — C++ parity: TheGameClient singleton, wired into Main's frame loop
    // for drawable updates and display draw. Full GameClient::update() OS-input path
    // is not used (Main owns input→commands); drawables always tick with the frame.
    #[cfg(feature = "game_client")]
    pub(crate) game_client: game_client::core::game_client::GameClient,
    /// ControlBar selection panel (portrait + health). Presentation-fed; WND load optional.
    #[cfg(feature = "game_client")]
    pub(crate) control_bar: game_client::gui::control_bar::ControlBar,

    // Game state
    pub(crate) game_logic: GameLogic,
    /// Immutable presentation feed for client/render after last logic step.
    pub(crate) last_presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Wave 842: host-owned match mode residual set at start_game_from_ui.
    /// Prefer over live GameLogic::game_mode when presentation freeze is missing.
    pub(crate) host_match_game_mode: Option<GameMode>,
    /// Wave 843: host-owned match map / local player / AI difficulty residuals.
    pub(crate) host_match_map_name: Option<String>,
    pub(crate) host_match_local_player_id: Option<u32>,
    pub(crate) host_match_ai_difficulty: Option<crate::ai::AIDifficulty>,
    /// Wave 844: host-owned sim timing residuals (prefer over live GameLogic probes).
    pub(crate) host_match_visual_speed: Option<f32>,
    pub(crate) host_match_time_frozen: Option<bool>,
    pub(crate) host_match_total_play_time: Option<f32>,
    pub(crate) host_match_logic_frame: Option<u32>,
    pub(crate) host_match_logic_steps: Option<(u32, bool, f32)>,
    pub(crate) host_match_in_replay: Option<bool>,
    /// Wave 845: host-owned shell/team residuals for presentation_or_boot peels.
    pub(crate) host_match_in_shell: Option<bool>,
    pub(crate) host_match_local_team: Option<crate::game_logic::Team>,
    /// Wave 846: host-owned diplomacy / template / sciences residuals.
    pub(crate) host_match_diplomacy_players:
        Option<Vec<crate::presentation_frame::PresentationPlayerInfo>>,
    pub(crate) host_match_known_template_names: Option<Vec<String>>,
    pub(crate) host_match_unlocked_sciences: Option<std::collections::HashMap<u32, Vec<String>>>,
    /// Wave 847: host-owned camera-follow residuals for presentation_or_boot peels.
    pub(crate) host_match_camera_follow_active: Option<bool>,
    pub(crate) host_match_camera_follow_position: Option<[f32; 3]>,
    /// Wave 913: last camera-follow object residual (skip redundant authority writes).
    pub(crate) host_match_camera_follow_id: Option<Option<crate::game_logic::ObjectId>>,
    /// Wave 848: host-owned local train producers residual (barracks / other).
    pub(crate) host_match_local_barracks_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_producer_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_unfinished_producer_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_team_sample_pos: Option<[f32; 3]>,
    /// Wave 849: host-owned match outcome residuals (victory peels).
    pub(crate) host_match_over: Option<bool>,
    pub(crate) host_match_victory_label: Option<String>,
    /// Meaningful when `host_match_over == Some(true)`: None=draw, Some(id)=winner.
    pub(crate) host_match_victory_winner: Option<Option<u32>>,
    pub(crate) host_match_victory_summary: Option<crate::game_logic::VictorySummary>,
    /// Wave 850: host-owned selection residual (peels player_selected_objects boot dual-read).
    pub(crate) host_match_selected_ids: Option<Vec<crate::game_logic::ObjectId>>,
    /// Wave 851: host-owned alive-object residual (peels object_is_alive boot dual-read).
    pub(crate) host_match_alive_object_ids: Option<std::collections::HashSet<u32>>,
    /// Wave 852: host-owned purchasable science residual per player.
    pub(crate) host_match_purchasable_sciences:
        Option<std::collections::HashMap<u32, std::collections::HashSet<String>>>,
    /// Wave 868: host-owned local science purchase points residual.
    pub(crate) host_match_local_science_purchase_points: Option<i32>,
    /// Wave 921: local supplies residual (presentation stamp; supplies floor peel).
    pub(crate) host_match_local_supplies: Option<u32>,
    /// Wave 854/857: host-owned special-power-ready object residual (unified scan stamp).
    pub(crate) host_match_special_power_ready_ids: Option<std::collections::HashSet<u32>>,
    /// Wave 855: boot victory condition residual (single evaluate stamp).
    /// None = not stamped; Some(None) = no winner yet; Some(Some(cond)) = outcome.
    pub(crate) host_match_boot_victory_condition:
        Option<Option<crate::game_logic::VictoryCondition>>,
    /// Wave 911: per-frame legal-build residual cache (construct pad scan peel).
    pub(crate) host_legal_build_cache_frame: Option<u32>,
    pub(crate) host_legal_build_cache:
        std::collections::HashMap<(crate::game_logic::Team, i32, i32, u64, u32), u32>,
    /// Wave 858: host-owned script camera default residuals.
    pub(crate) host_match_script_camera_max_height: Option<f32>,
    pub(crate) host_match_script_camera_pitch: Option<f32>,
    /// Wave 861: host-owned multiplayer residual (presentation dual-read peel).
    pub(crate) host_match_in_multiplayer: Option<bool>,
    /// Wave 862: host-owned world bounds residual (min, max).
    pub(crate) host_match_world_bounds: Option<(glam::Vec3, glam::Vec3)>,
    /// Wave 863: host-owned first-opponent residual (debug victory hotkey peel).
    pub(crate) host_match_first_opponent_id: Option<Option<u32>>,
    /// Optional GameWorld shadow session (stable ObjectId→EntityId).
    /// Production default ON (`GENERALS_GAMEWORLD_SHADOW=0` to opt out).
    /// Last-writer for HP/cash/pose/targets/move; not sole GameWorld authority yet.
    pub(crate) gameworld_shadow: Option<crate::gameworld_shadow::GameWorldShadow>,
    /// Observe-path entity count from GameWorld presentation view after coupled tick
    /// (architecture residual: GameWorld → presentation without Main dual-read).
    pub(crate) last_gameworld_presentation_entity_count: usize,
    /// Last presentation-overlaid UI state (selection health/minimap identity retained
    /// after render build so consumers are not dropped each frame).
    pub(crate) last_ui_state: Option<GameUIState>,
    pub(crate) resource_manager: ResourceManager,
    pub(crate) save_file_manager: SaveFileManager,

    // Camera system
    pub(crate) camera_position: Vec3,
    pub(crate) camera_target: Vec3,
    pub(crate) camera_zoom: f32,
    pub(crate) camera_zoom_target: Option<f32>,
    pub(crate) camera_zoom_start: f32,
    pub(crate) camera_zoom_duration: f32,
    pub(crate) camera_zoom_elapsed: f32,
    pub(crate) camera_zoom_ease_in: f32,
    pub(crate) camera_zoom_ease_out: f32,
    pub(crate) camera_orbit_distance: f32,
    pub(crate) camera_pitch_radians: f32,
    pub(crate) camera_pitch_target: Option<f32>,
    pub(crate) camera_pitch_start: f32,
    pub(crate) camera_pitch_duration: f32,
    pub(crate) camera_pitch_elapsed: f32,
    pub(crate) camera_pitch_ease_in: f32,
    pub(crate) camera_pitch_ease_out: f32,
    pub(crate) camera_yaw_radians: f32,
    pub(crate) camera_yaw_target: Option<f32>,
    pub(crate) camera_yaw_start: f32,
    pub(crate) camera_yaw_duration: f32,
    pub(crate) camera_yaw_elapsed: f32,
    pub(crate) camera_yaw_ease_in: f32,
    pub(crate) camera_yaw_ease_out: f32,
    pub(crate) camera_shake_offset: Vec3,
    pub(crate) screen_shake_intensity: f32,
    pub(crate) screen_shake_angle_cos: f32,
    pub(crate) screen_shake_angle_sin: f32,
    pub(crate) script_camera_shakers: Vec<ScriptCameraShaker>,
    pub(crate) script_fps_limit: Option<u32>,
    pub(crate) script_fps_limit_last_tick: Option<Instant>,
    pub(crate) camera_slave_mode: Option<CameraSlaveModeRequest>,
    pub(crate) view_matrix: Mat4,
    pub(crate) projection_matrix: Mat4,

    // Input state
    pub(crate) keys_pressed: HashSet<Key>,
    pub(crate) mouse_position: (f32, f32),
    pub(crate) mouse_world_position: Vec3,
    /// Last applied context cursor residual (avoid spam set_cursor).
    pub(crate) last_context_cursor: Option<&'static str>,
    /// EVA LOWPOWER residual edge counter.
    pub(crate) last_eva_low_power_count: u32,
    pub(crate) last_eva_insufficient_funds_count: u32,
    pub(crate) last_eva_base_under_attack_count: u32,
    pub(crate) last_eva_ally_under_attack_count: u32,
    /// C++ sticky waypoint mode residual (Alt hold still works; Z toggles).
    pub(crate) sticky_waypoint_mode: bool,
    /// Sticky auto-attack residual (Ctrl+Shift+A): convert plain moves to attack-move.
    pub(crate) sticky_auto_attack: bool,
    pub(crate) is_dragging: bool,
    pub(crate) selection_start: Option<Vec3>,
    /// Screen-space drag origin for selection box overlay residual.
    pub(crate) selection_start_screen: Option<(f32, f32)>,
    pub(crate) last_click_time: Option<Instant>,
    pub(crate) last_click_position: Option<Vec3>,
    pub(crate) is_windowed: bool,
    pub(crate) rmb_scroll_anchor: Option<(f32, f32)>,
    pub(crate) is_rmb_scrolling: bool,
    /// Evidence-only provenance for the active RMB gesture. A gather proof
    /// requires the press and release to both be real OS mouse input; injected
    /// press/release pairs still execute normal gameplay but cannot qualify.
    pub(crate) rmb_scroll_started_physically: bool,
    pub(crate) is_mmb_rotating: bool,
    pub(crate) mmb_anchor: Option<(f32, f32)>,

    // Game state
    pub(crate) selected_objects: Vec<ObjectId>,
    pub(crate) control_groups: HashMap<u8, Vec<ObjectId>>,
    /// Last control-group digit select (group, Instant) for double-tap camera jump residual.
    pub(crate) last_control_group_select: Option<(u8, Instant)>,
    /// Retail SAVE_VIEW1..8 / VIEW_VIEW1..8 camera bookmark residual (F1-F8).
    pub(crate) camera_view_bookmarks: [Option<Vec3>; 8],
    pub(crate) camera_rotate_left_held: bool,
    pub(crate) camera_rotate_right_held: bool,
    pub(crate) camera_zoom_in_held: bool,
    pub(crate) camera_zoom_out_held: bool,
    /// Retail TOGGLE_CAMERA_TRACKING_DRAWABLE residual.
    pub(crate) camera_tracking_selection: bool,
    /// Retail TOGGLE_FAST_FORWARD_REPLAY residual (TiVO fast mode).
    pub(crate) replay_fast_forward: bool,
    /// Retail DIPLOMACY KEY_TAB residual panel.
    pub(crate) diplomacy_panel: crate::ui::DiplomacyPanel,
    /// Retail CHAT_EVERYONE / CHAT_ALLIES residual panel.
    pub(crate) chat_panel: crate::ui::ChatPanel,
    pub(crate) current_player_id: u32,
    pub(crate) game_paused: bool,

    // UI state
    pub(crate) show_debug_info: bool,
    pub(crate) show_health_bars: bool,
    /// FPS counter residual (options game.show_fps).
    pub(crate) show_fps: bool,
    /// Draw movement path lines residual.
    pub(crate) show_move_lines: bool,
    /// Draw attack-order lines residual.
    pub(crate) show_attack_lines: bool,
    pub(crate) frame_counter: u32,
    pub(crate) fps: f32,
    pub(crate) last_frame_timing: Option<FrameTiming>,
    pub(crate) frame_clock: FrameClock,
    pub(crate) menu_loading_tick_accumulator: Duration,
    pub(crate) menu_loading_last_tick: Instant,
    pub(crate) diagnostics_overlay: Option<DiagnosticsOverlayStats>,

    // UI system
    pub(crate) ui_manager: UIManager,
    pub(crate) game_hud: GameHUD,
    /// C++ structure placement template residual (awaiting map click).
    pub(crate) pending_structure_placement: Option<String>,
    /// C++ context command awaiting map click (AttackMove/Guard/SetRally residual).
    pub(crate) pending_map_command: Option<PendingMapCommand>,
    pub(crate) active_menu_shell_hook: Option<&'static str>,
    pub(crate) runtime_host_headless: bool,
    /// True when `--runtime_host` is set (headless or windowed). Host cmds/status.
    pub(crate) runtime_host_active: bool,
    pub(crate) runtime_host_base_ui_screen: Option<String>,
    pub(crate) runtime_host_ui_screen_override: Option<String>,
    /// Sticky: open_skirmish_menu / Skirmish UI was entered this host session.
    pub(crate) runtime_host_saw_skirmish_menu: bool,
    pub(crate) runtime_host_last_gameplay_cmd: String,
    /// Main owns Rust snapshot persistence, while PopupSaveLoad owns the retail
    /// WND interaction.  This latches installation of the small typed bridge
    /// between the two so a normal mouse-driven popup never writes Common's
    /// separate GameState snapshot by accident.
    pub(crate) popup_save_load_bridge_initialized: bool,
    /// A runtime acceptance command chooses a deterministic slot/display name
    /// before it drives the real "New Save Game" confirmation.  The WND only
    /// supplies the description for that pseudo-row, so consume these once the
    /// confirmed callback reaches Main's actual save authority.
    pub(crate) pending_popup_save_slot: Option<String>,
    pub(crate) pending_popup_save_display_name: Option<String>,
    /// Real-person, windowed input evidence for the retail playable claim.
    /// Deliberately separate from `runtime_host_last_gameplay_cmd`.
    pub(crate) interactive_playability: InteractivePlayabilityEvidence,
    /// Carrier IDs admitted only from a successful physical right-click Gather
    /// command for the local player. `ReturningResources` drop-off events are
    /// matched against this set before they may latch physical evidence.
    pub(crate) physical_gather_carrier_ids: HashSet<ObjectId>,
    /// Cumulative HP damage applied this match (host_damage_log residual).
    pub(crate) match_damage_applied: f32,
    /// Cumulative destroy events from damage this match.
    pub(crate) match_kills: u32,
    /// Host asked for an immediate screenshot residual (bridge/event-loop consumes).
    pub(crate) runtime_host_pending_capture: bool,

    // Model loading state
    pub(crate) models_loaded: bool,
    pub(crate) pending_shell_model_prewarm: VecDeque<String>,
    pub(crate) menu_enter_frame: Option<u64>,
    pub(crate) shell_ui_enqueued_frame: Option<u64>,
    pub(crate) last_shell_prewarm_log: Option<Instant>,
    pub(crate) shell_prewarm_completion_logged: bool,
    /// How many Menu frames have rendered the full world scene so far.
    /// The first few Menu frames skip the world render to avoid a freeze while
    /// models/textures/terrain are loaded lazily for the first time.
    pub(crate) menu_world_frames_rendered: u32,
    pub(crate) last_slow_menu_tick_log: Option<Instant>,
    pub(crate) match_over: bool,
    pub(crate) victory_summary: Option<VictorySummary>,
}

/// C++ SAGE engine VertexFormatXYZNDUV2 equivalent - matches original vertex declarations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],    // XYZ - Position coordinates
    pub normal: [f32; 3],      // N - Normal vector
    pub diffuse: u32,          // D - Diffuse color (RGBA packed as u32, like D3D8)
    pub tex_coords0: [f32; 2], // UV - Primary texture coordinates
    pub tex_coords1: [f32; 2], // UV2 - Secondary texture coordinates for multi-stage texturing
}

impl VertexXYZNDUV2 {
    /// C++ SAGE VertexFormatXYZNDUV2 buffer layout - matches D3DVERTEXELEMENT9 declarations
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexXYZNDUV2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position (XYZ)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal (N)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Diffuse color (D) - packed RGBA like D3D8
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Unorm8x4,
                },
                // Primary texture coordinates (UV)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<u32>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Secondary texture coordinates (UV2) for multi-texturing
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<u32>()
                        + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// C++ SAGE engine equivalent uniforms - matches GlobalUniforms structure
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SAGEUniforms {
    pub(crate) view_projection: [[f32; 4]; 4],
    pub(crate) view_matrix: [[f32; 4]; 4],
    pub(crate) projection_matrix: [[f32; 4]; 4],
    pub(crate) camera_position: [f32; 4],
    pub(crate) time: f32,
    pub(crate) ambient_light: [f32; 3],
    pub(crate) sun_direction: [f32; 3],
    pub(crate) sun_color: [f32; 3],
    pub(crate) _padding: f32,
}

/// C++ SAGE VertexMaterialClass equivalent - matches original material properties
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MaterialProperties {
    pub(crate) diffuse_color: [f32; 4], // Base color reflected by lighting
    pub(crate) specular_color: [f32; 4], // Sharp reflective highlights
    pub(crate) emissive_color: [f32; 4], // Self-illumination color
    pub(crate) opacity: f32,            // Transparency (1.0 = opaque, 0.0 = transparent)
    pub(crate) shininess: f32,          // Specular power
    pub(crate) stage0_uv_scale: [f32; 2], // UV scaling for stage 0
    pub(crate) stage1_uv_scale: [f32; 2], // UV scaling for stage 1
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StartupCameraDefaults {
    pub(crate) pitch_degrees: f32,
    pub(crate) yaw_degrees: f32,
    pub(crate) camera_height: f32,
    pub(crate) max_camera_height: f32,
}

#[cfg(feature = "game_client")]
pub(super) struct RegisteredGameClientBridge {
    pub(crate) client: crate::subsystem_manager::GameClientSubsystem,
    pub(crate) active: bool,
    pub(crate) state: SubsystemState,
}

#[cfg(feature = "game_client")]
impl RegisteredGameClientBridge {
    pub(super) fn new() -> SubsystemResult<Self> {
        Ok(Self {
            client: crate::subsystem_manager::GameClientSubsystem::new(),
            active: true,
            state: SubsystemState::Uninitialized,
        })
    }
}

#[cfg(feature = "game_client")]
impl GameClientInterface for RegisteredGameClientBridge {
    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;
        self.client
            .init()
            .map_err(|err| SubsystemError::InitializationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, delta_time: std::time::Duration) -> SubsystemResult<()> {
        self.client
            .update(delta_time.as_secs_f32())
            .map_err(|err| SubsystemError::UpdateFailed(err.to_string()))
    }

    fn render(&mut self) -> SubsystemResult<()> {
        // Rendering is owned by the Main runtime event loop.
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        self.client
            .reset()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;
        self.client
            .shutdown()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.active = false;
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn get_state(&self) -> SubsystemState {
        self.state
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

#[cfg(feature = "game_client")]
pub(super) fn register_command_list_bootstrap() {
    use game_client::message_stream::command_list::get_command_list;
    use game_engine::common::message_stream::SubsystemInterface;
    register_command_list_init(|| {
        if let Ok(mut cl) = get_command_list().write() {
            let _ = cl.init();
        }
    });
}

#[cfg(feature = "game_client")]
pub(super) fn register_real_game_client_bootstrap() {
    register_command_list_bootstrap();
}

#[cfg(not(feature = "game_client"))]
pub(super) fn register_real_game_client_bootstrap() {}
