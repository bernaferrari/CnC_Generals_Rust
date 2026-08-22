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

/// The main GameClient struct - central hub for all client operations
pub struct GameClient {
    // Core state
    frame: u32,
    last_visual_time_frame: u32,
    next_drawable_id: DrawableId,
    local_player_id: i32,
    /// Last presentation military caption applied (avoid per-frame re-push).
    last_applied_military_caption: Option<String>,
    /// Last presentation cinematic text applied (avoid per-frame re-push).
    last_applied_cinematic_text: Option<String>,
    /// Last presentation remaining_ms (detect leftover re-fire / re-arm).
    last_applied_cinematic_remaining_ms: Option<i32>,
    /// C++ `Display::m_cinematicFont` residual (script font name).
    cinematic_overlay_font: Option<String>,
    /// C++ `Display::m_cinematicTextFrames` — decremented per rendered frame.
    cinematic_overlay_frames: u32,
    /// Live letterbox enable residual (mirrors Display, for overlay fade).
    letterbox_overlay_enabled: bool,
    /// Instant letterbox enable/disable flipped (C++ `m_letterBoxFadeStartTime`).
    letterbox_overlay_fade_start: Option<Instant>,
    /// Last live InGameUI postDraw / icon-UI submit residual (present path).
    last_live_ingame_hud_draw: LiveInGameHudDrawCounts,


    // Drawable management
    drawable_map: std::collections::HashMap<DrawableId, Box<dyn Drawable>>,
    drawable_object_map: std::collections::HashMap<ObjectID, DrawableId>,
    /// Runtime-only identity and visual-template metadata for Main's direct
    /// host-object drawables.  Do not add this to Xfer: C++ reconstructs these
    /// client bindings around the loaded Drawable instances.
    presentation_direct_drawable_bindings:
        std::collections::HashMap<DrawableId, PresentationDirectDrawableBinding>,
    /// Never-zero generation allocator for direct visual binding lifetimes.
    /// It deliberately survives world invalidation so an object id/drawable id
    /// reuse cannot look like the same visual binding.
    next_presentation_direct_binding_generation: u64,
    drawable_toc: Vec<DrawableTOCEntry>,
    text_bearing_drawables: Vec<DrawableId>,
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
