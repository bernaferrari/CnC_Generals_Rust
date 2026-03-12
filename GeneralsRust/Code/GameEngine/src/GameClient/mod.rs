//! Game Client Module
//!
//! C&C Generals Zero Hour Game Client implementation in Rust
//!
//! This module contains the game client subsystems including:
//! - Display and window management
//! - Audio management
//! - Video playback
//! - GUI rendering
//! - Input handling
//! - Camera control
//! - Visual effects
//! - Drawable system (visual representation of game objects)
//!
//! Architecture follows the original C++ GameClient from:
//! /GeneralsMD/Code/GameEngine/Source/GameClient/

// Display and window management modules
pub mod display;
pub mod game_window;
pub mod display_system;

pub mod audio_manager;
pub mod sound_buffer;
pub mod audio_events;
pub mod GUI;

// Drawable system modules (visual representation)
pub mod drawable_info;
pub mod draw_module;
pub mod tint_envelope;
pub mod drawable;
pub mod drawable_ui;
pub mod drawable_physics;

// Camera system modules
pub mod types;
pub mod view;
pub mod look_at_translator;

// Video playback system modules
pub mod video_buffer;
pub mod video_stream;
pub mod video_player;

// Message/Chat system modules
pub mod in_game_chat;
pub mod in_game_ui_messages;
pub mod online_chat;

// FX and Particle system modules
pub mod fx_list;
pub mod particle_sys;
pub mod particle_system;

// Client update modules (visual effects, animations, etc.)
pub mod Module;

// Re-export main types
pub use audio_manager::{
    GameAudioManager,
    AudioChannelType,
    AudioPriority,
    AudioFormat,
    AudioPosition,
    AudioVelocity,
    AudioAttenuation,
    SoundHandle,
    MusicTrack,
    GameAudioEvents,
};

pub use sound_buffer::{
    SoundBuffer,
    SoundBufferData,
    StreamingSoundBuffer,
};

pub use audio_events::{
    AudioEventDispatcher,
    UnitAudioEvent,
    BuildingAudioEvent,
    WeaponAudioEvent,
    UIAudioEvent,
};

// Re-export camera system types
pub use types::{
    Coord3D,
    Coord2D,
    ICoord2D,
    IRegion2D,
    ObjectID,
    DrawableID,
    PI_F32,
};

pub use view::{
    View,
    ViewLocation,
    CameraShakeType,
    WorldToScreenReturn,
    CameraLockType,
    DEFAULT_VIEW_WIDTH,
    DEFAULT_VIEW_HEIGHT,
    DEFAULT_VIEW_ORIGIN_X,
    DEFAULT_VIEW_ORIGIN_Y,
};

pub use look_at_translator::{
    LookAtTranslator,
    MouseMoveResult,
    MouseCursor,
};

// Re-export video system types
pub use video_buffer::{
    VideoBuffer,
    VideoBufferType,
    BaseVideoBuffer,
    RectClass,
};

pub use video_stream::{
    VideoStream,
    VideoStreamInterface,
};

pub use video_player::{
    VideoPlayer,
    VideoPlayerInterface,
    Video,
    FieldParse,
    VIDEO_FIELD_PARSE_TABLE,
    init_video_player,
    get_video_player,
    shutdown_video_player,
};

// Re-export chat system types
pub use in_game_chat::{
    InGameChat,
    InGameChatType,
    ChatMessage,
    SlashCommandHandler,
    SlashCommandResult,
    ChatMessageProcessor,
    calculate_player_mask,
    ChatColorManager,
    ChatColors,
};

pub use in_game_ui_messages::{
    InGameUiMessages,
    UiMessage,
    MilitarySubtitleManager,
    MilitarySubtitleData,
    FloatingTextManager,
    FloatingTextData,
    DisplayString,
    RgbColor,
    make_color,
    MAX_UI_MESSAGES,
    MAX_SUBTITLE_LINES,
    DEFAULT_FLOATING_TEXT_TIMEOUT,
};

pub use online_chat::{
    OnlineChatManager,
    GameSpyColor,
    GameSpyColorPalette,
    PlayerInfo,
    RoomType,
    MessageType,
    ChatDisplayInfo,
    ChatChannel,
    GameLobbyInfo,
    LobbyManager,
    PEER_FLAG_OP,
};

// Re-export drawable system types
pub use drawable_info::{
    DrawableInfo,
    ExtraRenderFlags,
};

pub use draw_module::{
    DrawModule,
    ObjectDrawInterface,
    DebrisDrawInterface,
    TracerDrawInterface,
    RopeDrawInterface,
    LaserDrawInterface,
    TerrainDecalType,
    ShadowType,
    RenderCost,
    RGBColor,
    OBBox,
    WhichTurretType,
};

pub use tint_envelope::{
    TintEnvelope,
    DEFAULT_TINT_COLOR_FADE_RATE,
    DEF_ATTACK_FRAMES,
    DEF_SUSTAIN_FRAMES,
    DEF_DECAY_FRAMES,
    SUSTAIN_INDEFINITELY,
};

pub use drawable::{
    Drawable,
    DrawableIconType,
    DrawableIconInfo,
    DrawableLocoInfo,
    WheelInfo,
    StealthLookType,
    DrawableStatus,
    TintStatus,
    BodyDamageType,
    PhysicsXformInfo,
    DRAWABLE_FRAMES_PER_FLASH,
    MAX_ICONS,
};

pub use drawable_ui::{
    DrawableUI,
    DrawableUIExt,
    IRegion2D,
    ICoord2D,
    VeterancyLevel,
};

pub use drawable_physics::{
    DrawablePhysics,
    DrawablePhysicsExt,
    LocomotorAppearance,
    LocomotorInterface,
};

// Re-export FX and Particle system types
pub use fx_list::{
    FXNugget,
    FXList,
    FXListStore,
    CameraShakeType as FXCameraShakeType,
    ScorchType,
    GameClientRandomVariable,
    init_fx_list_store,
    get_fx_list_store,
    get_fx_list_store_mut,
};

pub use particle_sys::{
    Particle,
    ParticleInfo,
    ParticlePriorityType,
    ParticleShaderType,
    ParticleType,
    EmissionVelocityType,
    EmissionVolumeType,
    WindMotion,
    Keyframe,
    RGBColorKeyframe,
    ParticleSystemID,
    INVALID_PARTICLE_SYSTEM_ID,
    MAX_KEYFRAMES,
    MAX_VOLUME_PARTICLE_DEPTH,
    DEFAULT_VOLUME_PARTICLE_DEPTH,
    OPTIMUM_VOLUME_PARTICLE_DEPTH,
    ParticleSystemManager,
    GameLogicAttachmentResolver,
};

pub use particle_system::{
    ParticleSystem,
    ParticleSystemInfo,
    ParticleSystemTemplate,
    EmissionVelocity,
    EmissionVolume,
    RandomKeyframe,
};

// Re-export display system types
pub use display::{
    Display,
    DisplaySettings,
    DisplayMode,
    ShroudLevel,
    CellShroudStatus,
    DrawImageMode,
    TimeOfDay,
    DebugDisplayCallback,
};

pub use game_window::{
    GameWindow,
    WindowConfig,
    WindowState,
    FrameTiming,
    DisplayModeInfo,
    InputEvent,
    MouseButtonType,
};

pub use display_system::{
    DisplaySystem,
    DisplayEventLoop,
    InputEventHandler,
};

// Re-export client update module types
pub use Module::{
    AnimatedParticleSysBoneClientUpdate,
    BeaconClientUpdate,
    BeaconClientUpdateModuleData,
    SwayClientUpdate,
    BreezeInfo,
    RadarInterface,
    GameLogicInterface,
    ScriptEngineInterface,
    ClientUpdateModule,
    ClientUpdateModuleData,
    XferInterface,
};
