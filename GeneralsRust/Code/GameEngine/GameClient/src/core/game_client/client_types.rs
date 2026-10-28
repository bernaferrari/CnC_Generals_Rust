// GameClient struct, TOC entry, loaded-map handle, and translator IDs.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Drawable table of contents entry for save/load operations
#[derive(Debug, Clone)]
struct DrawableTOCEntry {
    name: String,
    id: u16,
}

/// Tracks the currently loaded map asset
#[derive(Debug, Clone)]
struct LoadedMap {
    name: String,
    handle: AssetHandle,
}

/// Message translator ID type
pub type TranslatorId = u32;
pub const TRANSLATOR_ID_INVALID: TranslatorId = 0;

/// Which receiving runtime owns player-camera input. Main handles native raw
/// events and held-key steps directly; attaching Core LookAt as well would
/// execute the same camera action twice against the native world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClientInputAuthority {
    #[default]
    StandaloneCore,
    NativeHosted,
}
impl ClientInputAuthority {
    const fn runs_core_look_at(self) -> bool {
        matches!(self, Self::StandaloneCore)
    }
}

/// The main GameClient struct - central hub for all client operations
pub struct GameClient {
    // Core state
    ui_renderer: Option<crate::gui::UIRenderer>,
    /// Composition, target and candidate state belong to this client only.
    pub(crate) ime: crate::gui::ime_manager::ImeManager,
    /// The single native WND tree owned for this client's lifetime.
    window_manager_owner: crate::gui::window_manager::WindowManagerOwner,
    pub(crate) prepare_background_layout:
        Option<std::rc::Rc<std::cell::RefCell<crate::gui::WindowLayout>>>,
    /// This client's ordered message pipeline and its translator-local state.
    pub(crate) campaign_manager: crate::gui::campaign_manager::CampaignManager,
    pub(crate) menu_host: crate::gui::campaign_launch_host_bridge::MenuHostMailbox,
    pub(crate) script_display: crate::core::script_action_handler::ScriptDisplayReceiver,
    pub(crate) main_menu: crate::gui::shell::main_menu::MainMenu,
    pub(crate) menu_manager: crate::gui::callbacks::menu_callbacks::MenuManager,
    message_stream: crate::message_stream::message_stream::MessageStream,
    /// Fixed at inert construction; never inferred from a global registry.
    input_authority: ClientInputAuthority,
    /// C++ shell-game load-screen first-load latch for this receiving client.
    shell_game_first_load: bool,
    window_videos: crate::gui::window_video_manager::ClientWindowVideos,
    load_screen_runtime: crate::gui::load_screen::LoadScreenRuntime,
    load_screen_messages: Vec<crate::message_stream::game_message::GameMessage>,
    frame: u32,
    /// Current lighting facts copied from the GameLogic that drives this client.
    pub(crate) session_time_of_day: i32,
    pub(crate) session_weather_is_snowy: bool,
    pub(crate) session_force_models_to_follow_time_of_day: bool,
    pub(crate) session_force_models_to_follow_weather: bool,
    /// Effective lighting admitted by the driving game instance.
    session_lighting: game_engine::common::ini::ini_game_data::LightingContentRules,
    command_hints: crate::gui::command_hints::CommandHintState,
    owned_world_animations: OwnedWorldAnimations,
    /// DrawIconUI admitted from the driving Main session before native draw.
    receiving_draw_icon_ui: bool,
    snow: crate::snow::SnowManager,
    snow_definition: crate::snow::WeatherSetting,
    snow_base_definition: crate::snow::WeatherSetting,
    last_visual_time_frame: u32,
    /// Display-phase admission belongs to this receiving Client, independently
    /// of any peer with the same frame number. Not serialized, like C++ static lastFrame.
    last_native_draw_frame: u32,
    // Immutable lifetime identity, retained by an outstanding cross-crate continuation.
    presentation_shell_owner: std::sync::Arc<()>,
    presentation_shell_generation: u64,
    native_particle_capability_gaps: Vec<NativeParticleCapabilityGap>,
    local_player_id: i32,
    /// Application audio service handed in by the native host. Drawable
    /// retirement appends stops to this service's existing request queue.
    pub(crate) native_audio_device:
        Option<std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>>,
    /// Presentation companion for the driving GameLogic instance. Bound by
    /// the engine only when a world becomes active, never by construction.
    visual_world: Option<gamelogic::helpers::ClientVisualHandle>,
    /// Last presentation military caption applied (avoid per-frame re-push).
    last_applied_military_caption: Option<String>,
    /// Last presentation remaining_ms (detect leftover re-fire / re-arm).
    last_applied_military_caption_remaining_ms: Option<i32>,
    /// Last presentation cinematic text applied (avoid per-frame re-push).
    last_applied_cinematic_text: Option<String>,
    /// Last presentation remaining_ms (detect leftover re-fire / re-arm).
    last_applied_cinematic_remaining_ms: Option<i32>,
    /// C++ `Display::m_cinematicFont` residual (script font name).
    cinematic_overlay_font: Option<String>,
    /// C++ `Display::m_cinematicTextFrames` — decremented per rendered frame.
    cinematic_overlay_frames: u32,
    /// Live letterbox enable residual (mirrors Display, for overlay fade).
    /// Instant letterbox enable/disable flipped (C++ `m_letterBoxFadeStartTime`).
    /// Last live InGameUI postDraw / icon-UI submit residual (present path).
    last_live_ingame_hud_draw: LiveInGameHudDrawCounts,

    /// Renderer/cache state belongs to this client, never an active process slot.
    render_bridge: crate::render_bridge::RenderBridge,

    /// Drawable inventory for this receiving GameClient. The process-global
    /// manager remains only for standalone compatibility callers.
    drawable_manager: crate::drawable::DrawableManager,

    // Drawable management
    drawables: drawable_world::DrawableWorld,
    /// Drawable snapshot IDs in successful native Xfer visitation order.
    /// `None` means no completed restore is awaiting Drawable postprocessing.
    pub(crate) pending_drawable_post_process_order:
        Option<Vec<crate::drawable::drawable::DrawableId>>,
    drawable_toc: Vec<DrawableTOCEntry>,
    loaded_map: Option<LoadedMap>,

    // Message system
    translators: [TranslatorId; super::MAX_CLIENT_TRANSLATORS],
    num_translators: usize,
    command_translator: Option<Arc<dyn CommandTranslator>>,
    message_dispatcher: Arc<GameClientMessageDispatcher>,
    network_bridge: Option<NetworkBridgeHandle>,

    // Subsystems
    subsystem_manager: SubsystemManager,

    audio_event_queue: Option<AudioEventQueue>,
    music_system: Option<MusicSystem>,
    speech_system: Option<SpeechSystem>,
    audio_engine: Option<AudioEngine>,

    // Shadow system — mirrors C++ ShadowManager per-object shadow table
    shadow_map: std::collections::HashMap<ObjectID, Shadow>,
    shadows_enabled: bool,

    // Performance tracking
    rendered_object_count: u32,
    last_update_time: Instant,

    // Timing
    target_frame_duration: Duration,

    // Runtime flags
    startup_sizzle_pending: bool,
    initialized: bool,
}

/// Borrowed native window-input driver. The receiving client supplies its identities.
pub type NativeWindowInputDriver<'a> = dyn FnMut(
        &crate::gui::window_manager::WindowManagerOwner,
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::shell::main_menu::MainMenu,
        &mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox,
        &mut crate::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'_>,
        &mut crate::core::script_action_handler::ScriptDisplayReceiver,
        &mut crate::gui::window_video_manager::ClientWindowVideos,
        &mut crate::gui::ime_manager::ImeManager,
        &mut crate::core::subsystems::InGameUISubsystem,
        Option<&mut crate::input::Mouse>,
        game_engine::common::message_stream::WindowInputDispatch,
        &mut Vec<crate::message_stream::game_message::GameMessage>,
    ) -> Option<game_engine::common::message_stream::WindowInputDispatchResult>
    + 'a;
