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
    /// Display-phase admission belongs to this receiving Client, independently
    /// of any peer with the same frame number. Not serialized, like C++ static lastFrame.
    last_native_draw_frame: u32,
    // Immutable lifetime identity, retained by an outstanding cross-crate continuation.
    presentation_shell_owner: std::sync::Arc<()>,
    presentation_shell_generation: u64,
    native_particle_capability_gaps: Vec<NativeParticleCapabilityGap>,
    local_player_id: i32,
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
    letterbox_overlay_enabled: bool,
    /// Instant letterbox enable/disable flipped (C++ `m_letterBoxFadeStartTime`).
    letterbox_overlay_fade_start: Option<Instant>,
    /// Last live InGameUI postDraw / icon-UI submit residual (present path).
    last_live_ingame_hud_draw: LiveInGameHudDrawCounts,

    // Drawable management
    drawables: drawable_world::DrawableWorld,
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
