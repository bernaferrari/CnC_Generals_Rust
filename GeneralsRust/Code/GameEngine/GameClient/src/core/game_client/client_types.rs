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
    pub(crate) script_runtime: crate::core::script_action_runtime::ScriptActionRuntime,
    pub(crate) draw_schedule: crate::display::client_draw_schedule::ClientDrawScheduleState,
    pub(crate) main_menu: crate::gui::shell::main_menu::MainMenu,
    pub(crate) menu_manager: crate::gui::callbacks::menu_callbacks::MenuManager,
    message_stream: crate::message_stream::message_stream::MessageStream,
    /// Parsed CommandMap / MetaEvent key bindings (C++ TheMetaMap).
    pub(crate) meta_map: crate::message_stream::meta_event::MetaMap,
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
    receiving_ui_images: Option<crate::display::image::ImageCollection>,
    mapped_image_collection: Option<std::sync::Arc<parking_lot::RwLock<crate::display::image::ImageCollection>>>,
    fx_list_store: Option<std::sync::Arc<std::sync::RwLock<crate::fx_list::FXListStore>>>,
    weather_setting: Option<std::sync::Arc<std::sync::RwLock<crate::snow::WeatherSetting>>>,
    snow_manager: Option<std::sync::Arc<std::sync::Mutex<crate::snow::SnowManager>>>,
    draw_group_info: Option<std::sync::Arc<std::sync::RwLock<crate::draw_group_info::DrawGroupInfo>>>,
    game_text_store: Option<std::sync::Arc<std::sync::RwLock<crate::game_text::GameText>>>,
    global_language_data: Option<std::sync::Arc<std::sync::RwLock<crate::global_language::GlobalLanguageData>>>,
    language_filter: Option<std::sync::Arc<std::sync::RwLock<crate::language_filter::LanguageFilter>>>,
    smudge_manager: Option<std::sync::Arc<std::sync::Mutex<crate::system::smudge::SmudgeManager>>>,
    header_template_manager: Option<std::sync::Arc<std::sync::Mutex<crate::gui::header_template::HeaderTemplateManager>>>,
    translator_presentation_residual: Option<std::sync::Arc<std::sync::RwLock<crate::presentation_translator_residual::TranslatorPresentationResidual>>>,
    mouse: Option<std::sync::Arc<std::sync::Mutex<crate::input::mouse::Mouse>>>,
    host_command_authority: Option<std::sync::Arc<dyn Fn(&[crate::message_stream::game_message::GameMessage]) + Send + Sync>>,
    font_library: Option<std::sync::Arc<std::sync::Mutex<crate::gui::font::FontLibrary>>>,
    /// C++ `TheTacticalView` — owned per receiving client.
    pub(crate) tactical_view: crate::display::view::View,
    /// Move/locator hint drawable cache for this client's tactical view.
    pub(crate) hint_model_cache: crate::display::view::HintModelCache,
    /// C++ `TheDisplay->isLetterBoxed()` as seen by W3DView::buildCameraTransform.
    pub(crate) display_letter_boxed: bool,
    /// Per-frame unit shadow casters + GUI-thread GPU pipelines for this client.
    pub(crate) shadow_pass: crate::display::shadow_pass::ShadowPassState,
    /// Resolved terrain texture path caches for this client.
    pub(crate) terrain_texture_caches: crate::terrain::textures::TerrainTextureCaches,
    /// C++ `TheTerrainVisual` GPU impl — owned per receiving client (was process lazy_static).
    pub(crate) terrain_visual_gpu: std::sync::Arc<
        std::sync::Mutex<Option<crate::terrain::terrain_visual::TerrainVisualImpl>>,
    >,
    fx_next_id: std::sync::atomic::AtomicU64,
    render_bridge_next_line_id: std::sync::atomic::AtomicU64,
    win_draw_image_commands: std::sync::atomic::AtomicUsize,
    shipped_ui_draw_commands: std::sync::atomic::AtomicUsize,
    text_entry_draw_cnt: std::sync::atomic::AtomicU8,
    asset_id_counter: std::sync::atomic::AtomicU64,
    device_id_counter: std::sync::atomic::AtomicU64,
    supply_and_tech_images: Option<std::sync::Arc<std::sync::Mutex<crate::map_util::TechAndSupplyImages>>>,
    map_cache_manager: Option<std::sync::Arc<std::sync::Mutex<crate::map_util::MapCache>>>,
    video_player_slot: Option<std::sync::Arc<std::sync::Mutex<Option<crate::video_player::VideoPlayer>>>>,
    video_stream_provider_slot: Option<std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<dyn crate::video_player::VideoStreamProvider>>>>>,
    startup_shell_image_reported: std::collections::HashSet<String>,
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
    /// Wave 984 host residual: flash contained presentation drawables on select.
    host_contained_flash_queue: Vec<u32>,
    /// C++ TheAnim2DCollection residual.
    client_anim2d_instances: std::sync::Arc<parking_lot::Mutex<crate::system::anim2_d::Anim2DCollection>>,

    /// Stable identity of this client's live-slot registration (0 = none). It
    /// travels with the value across moves, unlike its address.
    pub(crate) live_slot_token: u64,

    // Drawable management
    pub(crate) drawables: drawable_world::DrawableWorld,
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
