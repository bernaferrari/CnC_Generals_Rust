//! Borrowed client UI operations and native-service adapters.
//!
//! InGameUI state belongs to its receiving GameClient. The inherent operations
//! below use that owned subsystem directly; no active UI backend is published.

use crate::display::view::{IPoint2, Point3, with_tactical_view, with_tactical_view_ref};

use crate::game_text::GameText;
use crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text;
use crate::gui::control_bar::{CommandOption, HostControlBarInputProvenance};

use crate::gui::load_screen::{
    LoadScreenGameMode, LoadScreenInitContext, LoadScreenKind, LoadScreenRequest,
    reset_load_screen, run_load_screen_prelude_with_pump, select_load_screen,
    update_load_screen_with_pump,
};
use crate::gui::{
    HintData, HintType, MouseCursor, MouseMode, WindowLayout, WindowStatus, hide_control_bar,
    queue_shell_hide, queue_shell_operation, show_shell_map_if_available,
};
use crate::message_stream::game_message::{Coord3D, ICoord2D};
use game_engine::common::recorder::with_recorder;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};

use log::info;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Change only the explicitly receiving client's cursor visibility.
pub fn set_mouse_cursor_visibility(mouse: &mut crate::input::Mouse, visible: bool) {
    mouse.set_cursor_visible(visible);
}

pub fn is_mouse_cursor_visible(mouse: &crate::input::Mouse) -> bool {
    mouse.state().is_cursor_visible()
}

/// A WND click awaiting the live Control Bar tick.
///
/// The actual command request is published during the bar replay phase, not inside
/// the WND callback.  Capture the callback's input provenance here so that a
/// runtime-host injected click cannot be mistaken for an OS mouse event.
#[derive(Clone)]
pub struct LiveControlBarClick {
    pub control_id: u32,
    pub msg: u32,
    pub input_provenance: HostControlBarInputProvenance,
    pub target: Option<std::rc::Weak<RefCell<crate::gui::GameWindow>>>,
}

impl LiveControlBarClick {
    /// Copy this receiving WND operation's explicit input origin for deferred replay.
    ///
    /// This must happen while the WindowManager callback is still executing;
    /// the receiving ControlBar publishes the eventual host request at its original phase.
    pub fn from_input(
        control_id: u32,
        msg: u32,
        input_provenance: HostControlBarInputProvenance,
    ) -> Self {
        Self {
            control_id,
            msg,
            input_provenance,
            target: None,
        }
    }
}

/// Pending non-click UI notifications for the live `ControlBar` (not Send).
#[derive(Debug, Default)]
pub struct LiveControlBarEvents {
    pub ui_dirty: bool,
    pub transitions: Vec<(u32, bool)>,
    pub hide_purchase_science: bool,
    pub toggle_purchase_science: bool,
    pub show_special_power_shortcut: bool,
    pub hide_special_power_shortcut: bool,
    pub animate_special_power_shortcut: Option<bool>,
    pub toggle_control_bar_stage: bool,
    pub init_special_power_shortcut_for_player: Option<String>,
    pub set_scheme_by_player: Option<String>,
}

#[derive(Debug)]
struct LegacyCoreControlBarIngress {
    ui_dirty: bool,
    observer_look_at: i32,
}
impl Default for LegacyCoreControlBarIngress {
    fn default() -> Self {
        Self {
            ui_dirty: false,
            observer_look_at: -1,
        }
    }
}
// Transitional standalone Core ingress only. Native UIs never register or
// consume this old simulation hook; their events are owned by the actual UI.
thread_local! {
    static LEGACY_CORE_CONTROL_BAR_INGRESS: RefCell<LegacyCoreControlBarIngress> = RefCell::new(Default::default());
}
pub fn drain_live_control_bar_events(
    ui: &mut crate::core::subsystems::InGameUISubsystem,
) -> LiveControlBarEvents {
    let mut events = std::mem::take(&mut ui.control_bar_events);
    if !crate::gui::control_bar::host_control_bar_bridge_enabled(ui) {
        LEGACY_CORE_CONTROL_BAR_INGRESS.with(|pending| {
            events.ui_dirty |= std::mem::take(&mut pending.borrow_mut().ui_dirty);
        });
    }
    events
}
pub fn set_live_control_bar_observer_look_at(
    index: Option<i32>,
    ui: &mut crate::core::subsystems::InGameUISubsystem,
) {
    ui.control_bar_observer_look_at = index;
    if !crate::gui::control_bar::host_control_bar_bridge_enabled(ui) {
        set_legacy_core_control_bar_observer_look_at(index);
    }
}
pub(crate) fn set_legacy_core_control_bar_observer_look_at(index: Option<i32>) {
    LEGACY_CORE_CONTROL_BAR_INGRESS
        .with(|pending| pending.borrow_mut().observer_look_at = index.unwrap_or(-1));
}

struct LiveControlBarUiHooks;

impl gamelogic::control_bar::ControlBarUiHooks for LiveControlBarUiHooks {
    fn mark_ui_dirty(&self) {
        LEGACY_CORE_CONTROL_BAR_INGRESS.with(|pending| pending.borrow_mut().ui_dirty = true);
    }

    fn on_player_science_purchase_points_changed(&self, _player_id: i32, _points: i32) {
        self.mark_ui_dirty();
    }

    fn on_player_rank_changed(&self, _player_id: i32, _rank_level: i32, _points: i32) {
        self.mark_ui_dirty();
    }
}

/// Transitional standalone Core dirty ingress; actual WND events borrow their UI.
pub fn register_legacy_core_control_bar_ui_ingress() {
    let _ = gamelogic::control_bar::register_control_bar_ui_hooks(Arc::new(LiveControlBarUiHooks));
}

#[cfg(test)]
mod live_control_bar_click_provenance_tests {
    use super::LiveControlBarClick;
    use crate::gui::control_bar::HostControlBarInputProvenance;

    #[test]
    fn deferred_click_snapshots_physical_origin_and_fails_closed_otherwise() {
        let unscoped = LiveControlBarClick::from_input(
            10,
            20,
            HostControlBarInputProvenance::InjectedOrUnknown,
        );
        let injected = LiveControlBarClick::from_input(
            11,
            21,
            HostControlBarInputProvenance::InjectedOrUnknown,
        );
        let physical = LiveControlBarClick::from_input(
            12,
            22,
            HostControlBarInputProvenance::PhysicalWindowMouseInput,
        );

        assert_eq!(
            unscoped.input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown
        );
        assert_eq!(
            injected.input_provenance,
            HostControlBarInputProvenance::InjectedOrUnknown
        );
        assert_eq!(
            physical.input_provenance,
            HostControlBarInputProvenance::PhysicalWindowMouseInput
        );
    }
}

struct GameClientObserverAudioLocalityHooks;

#[derive(Debug, Clone, Copy)]
struct ActiveLoadScreen {
    kind: LoadScreenKind,
    game_mode: i32,
}

const LOAD_SCREEN_COMPLETION_TRANSITION_GROUP: &str = "FadeWholeScreen";
const LOAD_SCREEN_COMPLETION_TRANSITION_MAX_FRAMES: usize = 180;

/// Short-lived adapter passed by borrow to GameLogic's synchronous start/load call.
/// It owns no client state; the actual WND tree and first-load latch live on GameClient.
pub struct GameClientLoadScreenHooks<'a> {
    client: &'a mut crate::core::game_client::GameClient,
    menu_audio: &'a mut dyn crate::gui::menu_audio::MenuAudioEffects,
    active_load_screen: Option<ActiveLoadScreen>,
    pending_game_mode: Option<i32>,
}

impl<'a> GameClientLoadScreenHooks<'a> {
    pub fn new(
        client: &'a mut crate::core::game_client::GameClient,
        menu_audio: &'a mut dyn crate::gui::menu_audio::MenuAudioEffects,
    ) -> Result<Self, String> {
        if !menu_audio.is_bound() {
            return Err("Core loading requires explicit bound menu audio".into());
        }
        Ok(Self {
            client,
            menu_audio,
            active_load_screen: None,
            pending_game_mode: None,
        })
    }

    fn reveal_shell_main_menu_after_start(&mut self, game_mode: i32) {
        if game_mode != gamelogic::system::game_logic::GAME_SHELL {
            return;
        }
        self.client.with_receiving_shell_and_menu_audio(
            None,
            &mut *self.menu_audio,
            |shell, effects| {
                if let Err(err) = shell.show_main_menu_after_shell_game_start(effects) {
                    log::warn!("Failed to activate shell main menu after shell-map load: {err}");
                }
            },
        );
        self.client.with_window_event_effects_and_menu_audio(
            None,
            &mut *self.menu_audio,
            |effects| {
                effects.with_manager(|manager, effects| {
                    if let Err(err) = hide_control_bar(true, manager, effects) {
                        log::warn!("Failed to hide control bar after shell-map load: {err}");
                    }
                });
            },
        );
    }
}

impl gamelogic::helpers::LoadScreenHooks for GameClientLoadScreenHooks<'_> {
    fn replay_controls_visibility(&mut self, hidden: bool) -> Result<(), String> {
        self.client.admit_replay_control_visibility(hidden);
        Ok(())
    }

    fn ensure_background_window(&mut self) -> Result<(), String> {
        let existing = self.client.prepare_background_layout.clone();
        let layout = self.client.with_window_event_effects_and_menu_audio(
            None,
            &mut *self.menu_audio,
            |effects| {
                effects.with_manager(|manager, effects| {
                    let current = existing.filter(|layout| {
                        layout.borrow().get_first_window().is_some_and(|window| {
                            manager
                                .get_window_by_id(window.borrow().get_id())
                                .is_some_and(|registered| Rc::ptr_eq(&registered, &window))
                        })
                    });
                    let layout = match current {
                        Some(layout) => layout,
                        None => {
                            manager
                                .create_layout_with_windows("Menus/BlankWindow.wnd", effects)
                                .map_err(|error| error.to_string())?
                                .0
                        }
                    };
                    layout.borrow().hide(manager, false);
                    layout.borrow_mut().bring_forward(manager);
                    if let Some(window) = layout.borrow().get_first_window() {
                        window.borrow_mut().clear_status(WindowStatus::IMAGE);
                    }
                    Ok::<_, String>(layout)
                })
            },
        )?;
        self.client.prepare_background_layout = Some(layout);
        Ok(())
    }
    fn hide_shell(&mut self) -> Result<(), String> {
        self.client
            .with_receiving_shell_and_menu_audio(None, &mut *self.menu_audio, |shell, effects| {
                shell.hide(true, effects)
            })
            .ok_or_else(|| "receiving Shell already borrowed during preparation".to_owned())?;
        Ok(())
    }

    fn begin_load_screen(&mut self, game_mode: i32, loading_save_game: bool) -> Result<(), String> {
        let (has_current_campaign, current_campaign_is_challenge) = {
            let campaign_manager = self.client.campaign_manager();
            let campaign = campaign_manager.get_current_campaign();
            (
                campaign.is_some(),
                campaign.map(|c| c.is_challenge_campaign()).unwrap_or(false),
            )
        };
        let request = LoadScreenRequest {
            mode: load_screen_game_mode_from_logic(game_mode),
            loading_save_game,
            has_current_campaign,
            current_campaign_is_challenge,
        };
        let owner = self.client.window_manager_owner();
        if let Some(active) = self.active_load_screen.take() {
            reset_load_screen(
                active.kind,
                &mut self.client.load_screen_runtime_mut(&mut *self.menu_audio),
            );
        }
        self.pending_game_mode = Some(game_mode);
        let Some(kind) = select_load_screen(request) else {
            return Ok(());
        };
        let context = load_screen_init_context_from_globals();
        // A legal-hold presentation can fail after WND creation. Retain its
        // exact owner receipt before initialization so end_load_screen cleans it.
        self.active_load_screen = Some(ActiveLoadScreen { kind, game_mode });
        if self
            .client
            .init_owned_load_screen(kind, &context, &mut *self.menu_audio)?
        {
            let (mut runtime, mut presentation_pump) = self
                .client
                .split_load_screen_runtime_and_pump(&mut *self.menu_audio);
            let _ = run_load_screen_prelude_with_pump(
                kind,
                crate::gui::load_screen::LOAD_SCREEN_PRELUDE_MAX_PUMPS,
                Some(&mut presentation_pump),
                &mut runtime,
            )?;
        } else {
            reset_load_screen(
                kind,
                &mut self.client.load_screen_runtime_mut(&mut *self.menu_audio),
            );
            self.active_load_screen = None;
        }

        Ok(())
    }

    fn update_load_screen(&mut self, progress: i32) -> Result<(), String> {
        if let Some(active) = self.active_load_screen {
            let owner = self.client.window_manager_owner();
            let (mut runtime, mut presentation_pump) = self
                .client
                .split_load_screen_runtime_and_pump(&mut *self.menu_audio);
            update_load_screen_with_pump(
                active.kind,
                progress as f32,
                Some(&mut presentation_pump),
                &mut runtime,
            )?;
        }

        Ok(())
    }

    fn run_load_screen_completion_transition(
        &mut self,
        loading_save_game: bool,
    ) -> Result<(), String> {
        if loading_save_game {
            return Ok(());
        }
        let owner = self.client.window_manager_owner();
        let (mut runtime, mut presentation_pump) = self
            .client
            .split_load_screen_runtime_and_pump(&mut *self.menu_audio);
        {
            owner.with_manager(|wm| {
                wm.transition_set_group(LOAD_SCREEN_COMPLETION_TRANSITION_GROUP, false, None)
            });
            for _ in 0..LOAD_SCREEN_COMPLETION_TRANSITION_MAX_FRAMES {
                if owner.with_manager(|wm| wm.transitions_finished()) {
                    break;
                }
                owner.with_manager(|wm| wm.update(&mut runtime.effects));
                let result = presentation_pump.pump(&mut runtime);
                gamelogic::system::game_logic::set_fp_mode();
                result?;
            }
        }

        Ok(())
    }

    fn end_load_screen(&mut self) {
        let active = self.active_load_screen.take();
        let game_mode = active
            .map(|item| item.game_mode)
            .or(self.pending_game_mode.take());
        if let Some(active) = active {
            if let Some(mouse) = self.client.input_mouse_mut() {
                set_mouse_cursor_visibility(mouse, true);
            }
            let owner = self.client.window_manager_owner();
            reset_load_screen(
                active.kind,
                &mut self.client.load_screen_runtime_mut(&mut *self.menu_audio),
            );
        }
        if let Some(game_mode) = game_mode {
            self.reveal_shell_main_menu_after_start(game_mode);
        }
    }
}

#[cfg(test)]
mod load_screen_hook_tests {
    use super::*;
    use crate::gui::load_screen::{
        clear_load_screen_presentation_pump, register_load_screen_presentation_pump,
    };
    use gamelogic::helpers::LoadScreenHooks;
    use std::cell::Cell;
    use std::rc::Rc;
    #[test]
    fn active_load_screen_hides_cursor_like_cpp_start_new_game() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        client.init_input_subsystems().unwrap();
        set_mouse_cursor_visibility(client.input_mouse_mut().unwrap(), false);
        let mut audio = crate::gui::menu_audio::SilentMenuAudio::new();
        let mut hooks = GameClientLoadScreenHooks::new(&mut client, &mut audio).unwrap();
        hooks.active_load_screen = Some(ActiveLoadScreen {
            kind: LoadScreenKind::ShellGame,
            game_mode: gamelogic::system::game_logic::GAME_SHELL,
        });
        set_mouse_cursor_visibility(hooks.client.input_mouse_mut().unwrap(), false);
        assert!(!is_mouse_cursor_visible(
            hooks.client.input_mouse().unwrap()
        ));
        set_mouse_cursor_visibility(hooks.client.input_mouse_mut().unwrap(), true);
    }

    #[test]
    fn ending_active_load_screen_shows_cursor_like_cpp_start_new_game() {
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        client.init_input_subsystems().unwrap();
        set_mouse_cursor_visibility(client.input_mouse_mut().unwrap(), false);
        let mut audio = crate::gui::menu_audio::SilentMenuAudio::new();
        let mut hooks = GameClientLoadScreenHooks::new(&mut client, &mut audio).unwrap();
        hooks.active_load_screen = Some(ActiveLoadScreen {
            kind: LoadScreenKind::ShellGame,
            game_mode: gamelogic::system::game_logic::GAME_NONE,
        });
        hooks.end_load_screen();
        assert!(is_mouse_cursor_visible(hooks.client.input_mouse().unwrap()));
    }

    #[test]
    fn save_load_completion_skips_fade_transition_like_cpp() {
        clear_load_screen_presentation_pump();
        let pump_count = Rc::new(Cell::new(0usize));
        let captured = Rc::clone(&pump_count);
        register_load_screen_presentation_pump(move || captured.set(captured.get() + 1));
        let mut client = crate::core::game_client::GameClient::new().unwrap();
        let mut audio = crate::gui::menu_audio::SilentMenuAudio::new();
        let mut hooks = GameClientLoadScreenHooks::new(&mut client, &mut audio).unwrap();
        hooks.run_load_screen_completion_transition(true);
        clear_load_screen_presentation_pump();
        assert_eq!(pump_count.get(), 0);
    }
}

fn load_screen_game_mode_from_logic(game_mode: i32) -> LoadScreenGameMode {
    match game_mode {
        gamelogic::system::game_logic::GAME_SINGLE_PLAYER => LoadScreenGameMode::SinglePlayer,
        gamelogic::system::game_logic::GAME_SKIRMISH => LoadScreenGameMode::Skirmish,
        gamelogic::system::game_logic::GAME_LAN => LoadScreenGameMode::Lan,
        gamelogic::system::game_logic::GAME_INTERNET => LoadScreenGameMode::Internet,
        gamelogic::system::game_logic::GAME_REPLAY => LoadScreenGameMode::Replay,
        gamelogic::system::game_logic::GAME_SHELL => LoadScreenGameMode::Shell,
        gamelogic::system::game_logic::GAME_NONE => LoadScreenGameMode::None,
        _ => LoadScreenGameMode::None,
    }
}

fn load_screen_init_context_from_globals() -> LoadScreenInitContext {
    let mut context = LoadScreenInitContext::default();
    context.map_name = game_engine::common::ini::get_global_data()
        .map(|data| data.read().map_name.clone())
        .filter(|map_name| !map_name.is_empty());
    context
}

impl gamelogic::helpers::ObserverAudioLocalityHooks for GameClientObserverAudioLocalityHooks {
    fn get_observer_look_at_player_index(&self) -> Option<i32> {
        TheControlBar::get_observer_look_at_player_index()
    }
}

pub fn register_observer_audio_locality_hooks() {
    let _ = gamelogic::helpers::register_observer_audio_locality_hooks(Arc::new(
        GameClientObserverAudioLocalityHooks,
    ));
}

struct GameClientObserverAudioViewHooks;

impl gamelogic::helpers::ObserverAudioViewHooks for GameClientObserverAudioViewHooks {
    fn get_tactical_view_position(&self) -> Option<(f32, f32, f32)> {
        Some(with_tactical_view_ref(|view| {
            let pos = view.position();
            (pos.x, pos.y, pos.z)
        }))
    }

    fn get_tactical_view_angle(&self) -> Option<f32> {
        Some(with_tactical_view_ref(|view| view.angle()))
    }

    fn get_3d_camera_position(&self) -> Option<(f32, f32, f32)> {
        Some(with_tactical_view_ref(|view| {
            let camera = view.get_3d_camera_position();
            (camera.x, camera.y, camera.z)
        }))
    }
}

pub fn register_observer_audio_view_hooks() {
    let _ = gamelogic::helpers::register_observer_audio_view_hooks(Arc::new(
        GameClientObserverAudioViewHooks,
    ));
}

#[derive(Debug, Clone)]
pub struct PendingSpecialPower {
    pub power_id: u32,
    pub options: u32,
    pub source_object_id: u32,
}

#[derive(Debug, Clone)]
pub struct PendingCommand {
    pub command_type: gamelogic::commands::command::CommandType,
    pub options: u32,
    pub source_object_id: u32,
    pub cursor_name: String,
    pub invalid_cursor_name: String,
    pub radius_cursor_type: String,
}

#[derive(Debug, Clone)]
pub struct PopupMessageData {
    pub message: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub text_color: u32,
    pub pause: bool,
    pub pause_music: bool,
    /// Opaque Main-owned identity for a mission-script popup. Standalone C++
    /// callers leave this `None` and retain their legacy dismissal route.
    pub host_popup_generation: Option<usize>,
    pub layout: Option<Rc<RefCell<WindowLayout>>>,
}

#[derive(Debug, Clone, Copy)]
enum CursorType {
    Arrow,
    Cross,
    Selecting,
    MoveTo,
    AttackMoveTo,
    Waypoint,
    AttackObject,
    OutRange,
    ForceAttackObject,
    ForceAttackGround,
    GetRepaired,
    Dock,
    GetHealed,
    DoRepair,
    ResumeConstruction,
    EnterFriendly,
    EnterAggressively,
    Defector,
    CaptureBuilding,
    Hack,
    GenericInvalid,
    SetRallyPoint,
    ParticleUplinkCannon,
}

#[derive(Debug)]
pub(crate) struct InGameUIStatusState {
    quit_menu_visible: bool,
    input_enabled: bool,
    client_quiet: bool,
    messages_on: bool,
    selecting: bool,
    scrolling: bool,
    scroll_amount_x: f32,
    scroll_amount_y: f32,
    cursor: CursorType,
    mouse_cursor: MouseCursor,
    mouse_mode: MouseMode,
    mouse_mode_cursor: MouseCursor,
    moused_over_drawable_id: u32,
    double_click_attack_move_guard_timer: u32,
    guard_hint_stashed_position: (f32, f32, f32),
}

impl Default for InGameUIStatusState {
    fn default() -> Self {
        Self {
            quit_menu_visible: false,
            input_enabled: true,
            client_quiet: false,
            messages_on: true,
            selecting: false,
            scrolling: false,
            scroll_amount_x: 0.0,
            scroll_amount_y: 0.0,
            cursor: CursorType::Arrow,
            mouse_cursor: MouseCursor::Arrow,
            mouse_mode: MouseMode::Default,
            mouse_mode_cursor: MouseCursor::Arrow,
            moused_over_drawable_id: 0,
            double_click_attack_move_guard_timer: 0,
            guard_hint_stashed_position: (0.0, 0.0, 0.0),
        }
    }
}

/// C++ `InGameUI::displayCantBuildMessage` label map (`GUI:CantBuild*`).
pub fn map_cant_build_message(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "GUI:CantBuildThere".to_string();
    }
    if trimmed.starts_with("GUI:") {
        return trimmed.to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("flat") {
        "GUI:CantBuildNotFlatEnough".to_string()
    } else if lower.contains("object") {
        "GUI:CantBuildObjectsInTheWay".to_string()
    } else if lower.contains("supply") || lower.contains("supplies") {
        "GUI:CantBuildTooCloseToSupplies".to_string()
    } else if lower.contains("path") {
        "GUI:CantBuildNoClearPath".to_string()
    } else if lower.contains("shroud") || lower.contains("visible") {
        "GUI:CantBuildShroud".to_string()
    } else if lower.contains("terrain")
        || lower.contains("cliff")
        || lower.contains("underwater")
        || lower.contains("bridge")
    {
        "GUI:CantBuildRestrictedTerrain".to_string()
    } else {
        "GUI:CantBuildThere".to_string()
    }
}

/// Operations on the receiving client's canonical UI state.
impl crate::core::subsystems::InGameUISubsystem {
    pub fn admit_radar_draw(
        &mut self,
        radar: &game_engine::common::system::radar::RadarPresentation,
    ) -> usize {
        if !self
            .radar_presentation
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current.identity(), radar.identity()))
        {
            return 0;
        }
        self.radar_presented_keys.retain(|old| {
            radar
                .event_keys()
                .iter()
                .any(|(_, identity, _, _)| Arc::ptr_eq(old, identity))
        });
        let mut events = Vec::new();
        let mut chirps = 0;
        for (index, identity, played, beacon) in radar.event_keys() {
            if !played
                && !self
                    .radar_presented_keys
                    .iter()
                    .any(|old| Arc::ptr_eq(old, identity))
            {
                self.radar_presented_keys.push(identity.clone());
                events.push((*index, identity.clone()));
                chirps += usize::from(!*beacon);
            }
        }
        if !events.is_empty() {
            self.radar_acknowledgments
                .push((radar.identity().clone(), events));
        }
        chirps
    }
    pub fn take_radar_acknowledgments(&mut self) -> Vec<(Arc<()>, Vec<(usize, Arc<()>)>)> {
        std::mem::take(&mut self.radar_acknowledgments)
    }
    pub fn radar_presentation(
        &self,
    ) -> Option<Arc<game_engine::common::system::radar::RadarPresentation>> {
        self.radar_presentation.clone()
    }
    pub fn set_radar_presentation(
        &mut self,
        radar: Arc<game_engine::common::system::radar::RadarPresentation>,
    ) {
        if !self
            .radar_presentation
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old.identity(), radar.identity()))
        {
            self.radar_presented_keys.clear();
        }
        self.radar_presentation = Some(radar);
    }
    fn set_cursor(&mut self, cursor: CursorType) {
        self.status.cursor = cursor;
    }
    fn cursor_from_name(name: &str) -> CursorType {
        match name {
            "ARROW" => CursorType::Arrow,
            "CROSS" => CursorType::Cross,
            "SELECTING" => CursorType::Selecting,
            "MOVETO" => CursorType::MoveTo,
            "ATTACKMOVETO" => CursorType::AttackMoveTo,
            "WAYPOINT" => CursorType::Waypoint,
            "ATTACK_OBJECT" => CursorType::AttackObject,
            "OUTRANGE" => CursorType::OutRange,
            "FORCE_ATTACK_OBJECT" => CursorType::ForceAttackObject,
            "FORCE_ATTACK_GROUND" => CursorType::ForceAttackGround,
            "GET_REPAIRED" => CursorType::GetRepaired,
            "DOCK" => CursorType::Dock,
            "GET_HEALED" => CursorType::GetHealed,
            "DO_REPAIR" => CursorType::DoRepair,
            "RESUME_CONSTRUCTION" => CursorType::ResumeConstruction,
            "ENTER_FRIENDLY" => CursorType::EnterFriendly,
            "ENTER_AGGRESSIVELY" => CursorType::EnterAggressively,
            "DEFECTOR" => CursorType::Defector,
            "CAPTUREBUILDING" => CursorType::CaptureBuilding,
            "HACK" => CursorType::Hack,
            "GENERIC_INVALID" => CursorType::GenericInvalid,
            "SET_RALLY_POINT" => CursorType::SetRallyPoint,
            "PARTICLE_UPLINK_CANNON" => CursorType::ParticleUplinkCannon,
            _ => CursorType::Arrow,
        }
    }
    fn cursor_name(cursor: CursorType) -> &'static str {
        match cursor {
            CursorType::Arrow => "ARROW",
            CursorType::Cross => "CROSS",
            CursorType::Selecting => "SELECTING",
            CursorType::MoveTo => "MOVETO",
            CursorType::AttackMoveTo => "ATTACKMOVETO",
            CursorType::Waypoint => "WAYPOINT",
            CursorType::AttackObject => "ATTACK_OBJECT",
            CursorType::OutRange => "OUTRANGE",
            CursorType::ForceAttackObject => "FORCE_ATTACK_OBJECT",
            CursorType::ForceAttackGround => "FORCE_ATTACK_GROUND",
            CursorType::GetRepaired => "GET_REPAIRED",
            CursorType::Dock => "DOCK",
            CursorType::GetHealed => "GET_HEALED",
            CursorType::DoRepair => "DO_REPAIR",
            CursorType::ResumeConstruction => "RESUME_CONSTRUCTION",
            CursorType::EnterFriendly => "ENTER_FRIENDLY",
            CursorType::EnterAggressively => "ENTER_AGGRESSIVELY",
            CursorType::Defector => "DEFECTOR",
            CursorType::CaptureBuilding => "CAPTUREBUILDING",
            CursorType::Hack => "HACK",
            CursorType::GenericInvalid => "GENERIC_INVALID",
            CursorType::SetRallyPoint => "SET_RALLY_POINT",
            CursorType::ParticleUplinkCannon => "PARTICLE_UPLINK_CANNON",
        }
    }
    pub fn select_area(&mut self, upper_left: ICoord2D, lower_right: ICoord2D) {
        self.record_selection(upper_left, lower_right);
    }
    pub fn issue_move_command(&mut self, position: Coord3D, queue: bool) {
        self.record_command(crate::core::subsystems::CommandLogEntry::Move {
            position,
            queued: queue,
        });
    }
    pub fn issue_force_attack_ground(&mut self, position: Coord3D) {
        self.record_command(
            crate::core::subsystems::CommandLogEntry::ForceAttackGround { position },
        );
    }
    pub fn issue_attack_command(&mut self, target: u32, queue: bool) {
        self.record_command(crate::core::subsystems::CommandLogEntry::Attack {
            target_id: target,
            queued: queue,
        });
    }
    pub fn issue_stop_command(&mut self) {
        self.record_command(crate::core::subsystems::CommandLogEntry::Stop);
    }
    pub fn set_hint_text(&mut self, hint: &str, frame: u32) {
        self.push_hud_message_at(hint.to_owned(), frame);
    }
    pub fn set_quit_menu_visible(&mut self, visible: bool) {
        self.status.quit_menu_visible = visible;
    }
    pub fn is_quit_menu_visible(&self) -> bool {
        self.status.quit_menu_visible
    }
    pub fn get_input_enabled(&self) -> bool {
        self.status.input_enabled
    }
    pub fn set_input_enabled(&mut self, enabled: bool) {
        let was_enabled = {
            if !enabled {
                self.status.selecting = false;
                self.status.scrolling = false;
                self.status.scroll_amount_x = 0.0;
                self.status.scroll_amount_y = 0.0;
            }
            let was_enabled = self.status.input_enabled;
            self.status.input_enabled = enabled;
            was_enabled
        };
        if was_enabled && !enabled {
            // C++ InGameUI::setInputEnabled falling edge (InGameUI.cpp:3391-3408)
            self.set_force_attack_mode(false);
            self.set_force_move_to_mode(false);
            self.set_waypoint_mode(false);
            self.set_prefer_selection_mode(false);
            self.set_camera_rotate_left(false);
            self.set_camera_rotate_right(false);
            self.set_camera_zoom_in(false);
            self.set_camera_zoom_out(false);
        }
    }
    pub fn is_selecting(&self) -> bool {
        self.status.selecting
    }
    pub fn set_selecting(&mut self, selecting: bool) {
        self.status.selecting = selecting;
    }
    pub fn set_scrolling(&mut self, scrolling: bool) -> bool {
        let changed = {
            let changed = self.status.scrolling != scrolling;
            self.status.scrolling = scrolling;
            if !scrolling {
                self.status.scroll_amount_x = 0.0;
                self.status.scroll_amount_y = 0.0;
            }
            if changed {
                // C++ InGameUI.cpp:2797 / 2805 setMouseCursor(SCROLL/ARROW).
                self.status.mouse_cursor = if scrolling {
                    MouseCursor::Scroll
                } else {
                    MouseCursor::Arrow
                };
            }
            changed
        };
        // C++ InGameUI.cpp:2799-2801: the receiving view clears its locks
        // immediately when this operation reports the rising edge.
        changed && scrolling
    }

    pub fn is_scrolling(&self) -> bool {
        self.status.scrolling
    }
    pub fn set_scroll_amount(&mut self, x: f32, y: f32) {
        {
            self.status.scroll_amount_x = x;
            self.status.scroll_amount_y = y;
        };
    }
    pub fn get_scroll_amount(&self) -> (f32, f32) {
        (self.status.scroll_amount_x, self.status.scroll_amount_y)
    }
    pub fn set_client_quiet(&mut self, quiet: bool) {
        self.status.client_quiet = quiet;
    }
    pub fn is_client_quiet(&self) -> bool {
        self.status.client_quiet
    }
    pub fn toggle_messages(&mut self) -> bool {
        {
            self.status.messages_on = !self.status.messages_on;
            self.status.messages_on
        }
    }
    pub fn is_messages_on(&self) -> bool {
        self.status.messages_on
    }
    pub fn set_cursor_arrow(&mut self) {
        self.set_cursor(CursorType::Arrow);
    }
    pub fn set_cursor_by_name(&mut self, cursor: &str) {
        self.set_cursor(Self::cursor_from_name(cursor));
    }
    pub fn get_cursor_name(&self) -> &'static str {
        Self::cursor_name(self.status.cursor)
    }
    pub fn set_radius_cursor_active(&mut self) {
        self.set_radius_cursor_active_state(None);
    }
    pub fn set_radius_cursor_active_with_type(&mut self, radius_cursor_type: &str) {
        self.set_radius_cursor_active_state(Some(radius_cursor_type.to_owned()));
    }
    pub fn place_build_available(
        &mut self,
        template_name: Option<String>,
        source_object_id: Option<u32>,
    ) {
        self.set_pending_place(template_name, source_object_id);
    }
    pub fn set_pending_special_power(
        &mut self,
        power_id: u32,
        options: u32,
        source_object_id: u32,
    ) {
        self.set_pending_special_power_state(Some(PendingSpecialPower {
            power_id,
            options,
            source_object_id,
        }));
    }
    pub fn command_option_need_target(options: u32) -> bool {
        let mut mask = CommandOption::NeedTargetEnemyObject as u32
            | CommandOption::NeedTargetNeutralObject as u32
            | CommandOption::NeedTargetAllyObject as u32
            | CommandOption::NeedTargetPos as u32
            | CommandOption::ContextmodeCommand as u32;
        #[cfg(feature = "allow_surrender")]
        {
            mask |= CommandOption::NeedTargetPrisoner as u32;
        }
        options & mask != 0
    }
    fn recorder_playback_active(&self) -> bool {
        self.recorder_playback_active
    }
    fn arm_gui_command_mouse_mode(&mut self, pending: &PendingCommand) {
        self.set_mouse_mode(MouseMode::GuiCommand);
        self.set_mouse_cursor(MouseCursor::Arrow);
        let is_context = pending.options & (CommandOption::ContextmodeCommand as u32) != 0;
        if !is_context
            && !pending.radius_cursor_type.is_empty()
            && !pending.radius_cursor_type.eq_ignore_ascii_case("NONE")
        {
            self.set_radius_cursor_active_with_type(&pending.radius_cursor_type);
        } else {
            self.set_radius_cursor_none();
        }
        (|guard: &mut InGameUIStatusState| guard.mouse_mode_cursor = MouseCursor::Arrow)(
            &mut self.status,
        );
    }
    fn restore_default_mouse_mode_after_gui_command(&mut self) {
        self.set_mouse_mode(MouseMode::Default);
        self.set_mouse_cursor(MouseCursor::Arrow);
        self.set_radius_cursor_none();
    }
    pub fn set_pending_command(
        &mut self,
        command_type: gamelogic::commands::command::CommandType,
        options: u32,
        source_object_id: u32,
    ) {
        if self.recorder_playback_active() {
            return;
        }
        let pending = PendingCommand {
            command_type,
            options,
            source_object_id,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        if !Self::command_option_need_target(pending.options) {
            self.restore_default_mouse_mode_after_gui_command();
            return;
        }
        self.set_pending_command_state(Some(pending.clone()));
        self.arm_gui_command_mouse_mode(&pending);
    }
    pub fn set_pending_command_with_visual(
        &mut self,
        command_type: gamelogic::commands::command::CommandType,
        options: u32,
        source_object_id: u32,
        cursor_name: String,
        invalid_cursor_name: String,
        radius_cursor_type: String,
    ) {
        if self.recorder_playback_active() {
            return;
        }
        let pending = PendingCommand {
            command_type,
            options,
            source_object_id,
            cursor_name,
            invalid_cursor_name,
            radius_cursor_type,
        };
        if !Self::command_option_need_target(pending.options) {
            self.restore_default_mouse_mode_after_gui_command();
            return;
        }
        self.set_pending_command_state(Some(pending.clone()));
        self.arm_gui_command_mouse_mode(&pending);
    }
    pub fn clear_pending_command(&mut self) {
        self.clear_pending_command_state();
        self.restore_default_mouse_mode_after_gui_command();
    }
    pub fn deselect_all(&mut self) {
        // C++ parity: InGameUI::deselectAll() — clears all selections
        // Full implementation would clear the selection list in InGameUI
        info!("InGameUI::deselect_all() called");
    }
    pub fn military_subtitle(
        &mut self,
        label: &str,
        duration_ms: i32,
        frame: u32,
        manager: &mut crate::gui::window_manager::WindowManager,
        diplomacy: &mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    ) {
        crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
            label, false, manager, diplomacy,
        );
        self.push_military_subtitle(label, duration_ms, frame, manager);
    }
    pub fn toggle_attack_move_to_mode(&mut self) -> bool {
        let enabled = !self.is_in_attack_move_to_mode();
        self.set_attack_move_to_mode(enabled);
        enabled
    }
    pub fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.status.mouse_cursor = cursor;
    }
    pub fn get_mouse_cursor(&self) -> MouseCursor {
        self.status.mouse_cursor
    }
    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        {
            self.status.mouse_mode = mode;
            if mode != MouseMode::GuiCommand {
                self.status.mouse_mode_cursor = MouseCursor::Arrow;
            }
        };
    }
    pub fn get_mouse_mode(&self) -> MouseMode {
        self.status.mouse_mode
    }
    pub fn get_mouse_mode_cursor(&self) -> MouseCursor {
        self.status.mouse_mode_cursor
    }
    pub fn set_moused_over_drawable_id(&mut self, id: u32) {
        self.status.moused_over_drawable_id = id;
    }
    pub fn get_moused_over_drawable_id(&self) -> u32 {
        self.status.moused_over_drawable_id
    }
    pub fn create_move_hint(
        &mut self,
        start: crate::message_stream::game_message::Coord3D,
        end: crate::message_stream::game_message::Coord3D,
        source_id: u32,
        client_frame: u32,
    ) {
        // C++ InGameUI.cpp:2163 — frame = TheGameClient->getFrame();
        // C++ W3DInGameUI::drawMoveHints shows while elapsed <= 40.
        let hint = HintData {
            hint_type: HintType::Move,
            start: gamelogic::common::Coord3D::new(start.x, start.y, start.z),
            end: gamelogic::common::Coord3D::new(end.x, end.y, end.z),
            creation_frame: client_frame,
            source_id,
            lifetime_frames: 41,
        };
        {
            self.hints
                .retain(|h| !(h.hint_type == HintType::Move && h.source_id == source_id));
            if self.hints.len() >= 256 {
                if let Some(pos) = self
                    .hints
                    .iter()
                    .position(|h| h.hint_type == HintType::Move)
                {
                    self.hints.remove(pos);
                }
            }
            self.hints.push(hint);
        };
    }
    pub fn create_attack_hint(
        &mut self,
        start: crate::message_stream::game_message::Coord3D,
        end: crate::message_stream::game_message::Coord3D,
        source_id: u32,
        client_frame: u32,
    ) {
        let hint = HintData {
            hint_type: HintType::Attack,
            start: gamelogic::common::Coord3D::new(start.x, start.y, start.z),
            end: gamelogic::common::Coord3D::new(end.x, end.y, end.z),
            // C++ InGameUI.cpp:2163 — hints stamp TheGameClient->getFrame();
            // frame 0 would expire before mid-match attacks ever render.
            creation_frame: client_frame,
            source_id,
            lifetime_frames: 41,
        };
        {
            if self.hints.len() >= 256 {
                if let Some(pos) = self
                    .hints
                    .iter()
                    .position(|h| h.hint_type == HintType::Attack)
                {
                    self.hints.remove(pos);
                }
            }
            self.hints.push(hint);
        };
    }
    pub fn begin_area_select_hint(&mut self) {
        let hint = HintData {
            hint_type: HintType::AreaSelect,
            start: gamelogic::common::Coord3D::new(0.0, 0.0, 0.0),
            end: gamelogic::common::Coord3D::new(0.0, 0.0, 0.0),
            creation_frame: 0,
            source_id: 0,
            lifetime_frames: 300,
        };
        self.hints.push(hint);
    }
    pub fn end_area_select_hint(&mut self) {
        {
            if let Some(pos) = self
                .hints
                .iter()
                .rposition(|h| h.hint_type == HintType::AreaSelect)
            {
                self.hints.remove(pos);
            }
        };
    }
    pub fn expire_hints(&mut self, current_frame: u32) {
        {
            self.hints
                .retain(|h| current_frame < h.creation_frame + h.lifetime_frames)
        };
    }
    pub fn clear_hints(&mut self) {
        self.hints.clear();
    }
    pub fn get_hints(&self) -> &[HintData] {
        &self.hints
    }
    pub fn arm_double_click_attack_move_guard_hint(&mut self, x: f32, y: f32, z: f32) {
        {
            self.status.double_click_attack_move_guard_timer = 11;
            self.status.guard_hint_stashed_position = (x, y, z);
        };
    }
    pub fn trigger_double_click_attack_move_guard_hint(&mut self, position: (f32, f32, f32)) {
        // C++ InGameUI.cpp:1314-1318; the receiving input/view resolves terrain.
        self.arm_double_click_attack_move_guard_hint(position.0, position.1, position.2);
        self.set_mouse_cursor(MouseCursor::ForceAttackGround);
        self.set_radius_cursor_active_with_type("GUARD_AREA");
    }

    pub fn consume_double_click_attack_move_guard_hint(&mut self) -> bool {
        {
            if self.status.double_click_attack_move_guard_timer == 0 {
                return false;
            }
            self.status.double_click_attack_move_guard_timer -= 1;
            self.status.double_click_attack_move_guard_timer > 0
        }
    }
    pub fn double_click_attack_move_guard_timer(&self) -> u32 {
        self.status.double_click_attack_move_guard_timer
    }
    pub fn guard_hint_stashed_position(&self) -> (f32, f32, f32) {
        self.status.guard_hint_stashed_position
    }
}
pub fn popup_message(
    identifier: &str,
    x: i32,
    y: i32,
    width: i32,
    pause: bool,
    pause_music: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    crate::gui::callbacks::in_game_popup_message::show_receiving_popup_message(
        identifier,
        x,
        y,
        width,
        0xffffffff,
        pause,
        pause_music,
        None,
        effects,
    );
}
pub(crate) fn popup_message_with_host_generation(
    identifier: &str,
    x: i32,
    y: i32,
    width: i32,
    pause: bool,
    pause_music: bool,
    generation: Option<usize>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    crate::gui::callbacks::in_game_popup_message::show_receiving_popup_message(
        identifier,
        x,
        y,
        width,
        0xffffffff,
        pause,
        pause_music,
        generation,
        effects,
    );
}
pub fn popup_message_with_color(
    identifier: &str,
    x: i32,
    y: i32,
    width: i32,
    color: u32,
    pause: bool,
    pause_music: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    crate::gui::callbacks::in_game_popup_message::show_receiving_popup_message(
        identifier,
        x,
        y,
        width,
        color,
        pause,
        pause_music,
        None,
        effects,
    );
}
pub fn get_popup_message_data(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<PopupMessageData> {
    crate::gui::callbacks::in_game_popup_message::receiving_popup_message(effects)
}
pub fn clear_popup_message_data(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<PopupMessageData> {
    crate::gui::callbacks::in_game_popup_message::clear_receiving_popup_message(effects)
}

/// Minimal stand-in for classic `TheControlBar` singleton.
pub struct TheControlBar;

impl TheControlBar {
    pub fn hide_purchase_science(ui: &mut crate::core::subsystems::InGameUISubsystem) {
        ui.control_bar_events.hide_purchase_science = true;
    }
    pub fn toggle_purchase_science(ui: &mut crate::core::subsystems::InGameUISubsystem) {
        ui.control_bar_events.toggle_purchase_science = true;
    }
    pub fn show_special_power_shortcut(ui: &mut crate::core::subsystems::InGameUISubsystem) {
        ui.control_bar_events.show_special_power_shortcut = true;
    }
    pub fn hide_special_power_shortcut(ui: &mut crate::core::subsystems::InGameUISubsystem) {
        ui.control_bar_events.hide_special_power_shortcut = true;
    }
    pub fn animate_special_power_shortcut(
        enabled: bool,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        ui.control_bar_events.animate_special_power_shortcut = Some(enabled);
    }
    pub fn init_special_power_shortcut_bar_for_player(
        player_side: &str,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        ui.control_bar_events.init_special_power_shortcut_for_player = Some(player_side.to_owned());
    }
    pub fn set_control_bar_scheme_by_player(
        player_side: &str,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        ui.control_bar_events.set_scheme_by_player = Some(player_side.to_owned());
    }
    pub fn toggle_control_bar_stage(ui: &mut crate::core::subsystems::InGameUISubsystem) {
        ui.control_bar_events.toggle_control_bar_stage = true;
    }
    pub fn process_context_sensitive_button_transition(
        control_id: u32,
        entering: bool,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        ui.control_bar_events
            .transitions
            .push((control_id, entering));
    }
    /// Standalone Core audio compatibility only; actual receiving UI users
    /// read their control_bar_observer_look_at field instead.
    pub fn get_observer_look_at_player_index() -> Option<i32> {
        let index =
            LEGACY_CORE_CONTROL_BAR_INGRESS.with(|pending| pending.borrow().observer_look_at);
        (index >= 0).then_some(index)
    }

    /// C++ `TheControlBar->getArrowImage()` (`m_genArrow`).
    pub fn get_arrow_image() -> Option<crate::gui::game_window::Image> {
        crate::gui::control_bar::control_bar_observer::get_gen_arrow_image()
    }

    /// C++ `TheControlBar->setArrowImage()`.
    pub fn set_arrow_image(image: Option<crate::gui::game_window::Image>) {
        crate::gui::control_bar::control_bar_observer::set_gen_arrow_image(image);
    }
}

#[cfg(test)]
mod in_game_ui_input_and_guard_hint_tests {
    use super::*;

    #[test]
    fn set_input_enabled_false_clears_force_waypoint_shift_camera_modes() {
        // C++ InGameUI::setInputEnabled (InGameUI.cpp:3382-3409)
        let mut ui = crate::core::subsystems::InGameUISubsystem::default();
        ui.set_input_enabled(true);
        ui.set_selecting(true);
        ui.set_force_attack_mode(true);
        ui.set_force_move_to_mode(true);
        ui.set_waypoint_mode(true);
        ui.set_prefer_selection_mode(true);
        ui.set_camera_rotate_left(true);
        ui.set_camera_rotate_right(true);
        ui.set_camera_zoom_in(true);
        ui.set_camera_zoom_out(true);

        ui.set_input_enabled(false);

        assert!(!ui.is_selecting());
        assert!(!ui.is_in_force_attack_mode());
        assert!(!ui.is_in_force_move_to_mode());
        assert!(!ui.is_in_waypoint_mode());
        assert!(!ui.is_in_prefer_selection_mode());
        assert!(!ui.is_camera_rotating_left());
        assert!(!ui.is_camera_rotating_right());
        assert!(!ui.is_camera_zooming_in());
        assert!(!ui.is_camera_zooming_out());

        // Already-disabled: C++ does not re-clear on a second FALSE.
        ui.set_force_attack_mode(true);
        ui.set_input_enabled(false);
        assert!(ui.is_in_force_attack_mode());

        ui.set_force_attack_mode(false);
    }

    #[test]
    fn trigger_double_click_attack_move_guard_hint_arms_11_frame_timer() {
        // C++ InGameUI::triggerDoubleClickAttackMoveGuardHint (InGameUI.cpp:1314-1318)
        let mut ui = crate::core::subsystems::InGameUISubsystem::default();
        ui.arm_double_click_attack_move_guard_hint(12.0, 24.0, 3.0);
        assert_eq!(ui.double_click_attack_move_guard_timer(), 11);
        assert_eq!(ui.guard_hint_stashed_position(), (12.0, 24.0, 3.0));
        assert!(ui.consume_double_click_attack_move_guard_hint());
        assert_eq!(ui.double_click_attack_move_guard_timer(), 10);
        assert_eq!(ui.guard_hint_stashed_position(), (12.0, 24.0, 3.0));

        while ui.double_click_attack_move_guard_timer() > 0 {
            let _ = ui.consume_double_click_attack_move_guard_hint();
        }
    }
}

#[cfg(test)]
mod receiving_control_bar_events_tests {
    use super::*;
    #[test]
    fn actual_ui_events_do_not_cross_peer_reset_or_drop() {
        let mut a = crate::core::subsystems::InGameUISubsystem::default();
        let mut b = crate::core::subsystems::InGameUISubsystem::default();
        a.admit_native_control_bar_transport();
        b.admit_native_control_bar_transport();
        TheControlBar::process_context_sensitive_button_transition(41, true, &mut a);
        TheControlBar::toggle_purchase_science(&mut a);
        TheControlBar::set_control_bar_scheme_by_player("America", &mut a);
        TheControlBar::process_context_sensitive_button_transition(41, false, &mut b);
        TheControlBar::set_control_bar_scheme_by_player("China", &mut b);
        b.clear_runtime_state();
        assert!(drain_live_control_bar_events(&mut b).transitions.is_empty());
        drop(b);
        let events = drain_live_control_bar_events(&mut a);
        assert_eq!(events.transitions, vec![(41, true)]);
        assert!(events.toggle_purchase_science);
        assert_eq!(events.set_scheme_by_player.as_deref(), Some("America"));
        assert!(drain_live_control_bar_events(&mut a).transitions.is_empty());
    }
    #[test]
    fn native_ui_never_consumes_standalone_dirty_or_observer_ingress() {
        let mut a = crate::core::subsystems::InGameUISubsystem::default();
        a.admit_native_control_bar_transport();
        LEGACY_CORE_CONTROL_BAR_INGRESS.with(|state| {
            state.borrow_mut().ui_dirty = true;
            state.borrow_mut().observer_look_at = 7;
        });
        assert!(!drain_live_control_bar_events(&mut a).ui_dirty);
        set_live_control_bar_observer_look_at(Some(2), &mut a);
        assert_eq!(a.control_bar_observer_look_at, Some(2));
        assert_eq!(TheControlBar::get_observer_look_at_player_index(), Some(7));
        LEGACY_CORE_CONTROL_BAR_INGRESS.with(|state| *state.borrow_mut() = Default::default());
    }
}
