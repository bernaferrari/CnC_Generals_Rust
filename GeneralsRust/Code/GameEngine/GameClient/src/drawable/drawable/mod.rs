//! Core Drawable trait and implementations.
//!
//! Live `crate::drawable::drawable` module (C++ Drawable.cpp / Drawable.h).
//! Split by domain from the former `drawable/drawable.rs` god-file. Public
//! names stay identical so `crate::drawable::drawable::*` keeps working.

use std::any::Any;
use std::sync::atomic::AtomicU8;

use crate::drawable::DrawableShroudClearState;
use crate::drawable_info::DrawableInfo;
use game_engine::common::audio::audio_event_rts::AudioEventRts;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::bit_flags::{ModelConditionBitFlags, create_model_condition_flags};
use gamelogic::object::registry::OBJECT_REGISTRY;

mod basic_core;
mod basic_drawable_trait;
mod basic_modules;
mod basic_overlay;
mod basic_visual;
mod condition_apply;
mod draw_module;
mod drawable_trait;
mod hidden_status;
mod icons;
mod leftover;
mod selection_flash;
mod snapshot;
mod tint_envelope;
mod types;
mod xfer;

#[cfg(test)]
mod tests;

pub(crate) use types::{DEFAULT_STEALTH_FRIENDLY_OPACITY, xfer_vector3};

pub use types::{
    Color, DARK_GRAY_DISABLED_COLOR, DrawableId, DrawableOverlayData, DrawableStatus,
    DrawableXferVisualSnapshot, FRENZY_COLOR, FRENZY_COLOR_INFANTRY, ICoord2D, INVALID_DRAWABLE_ID,
    IRegion2D, Matrix4, RED_IRRADIATED_COLOR, SICKLY_GREEN_POISONED_COLOR, SUBDUAL_DAMAGE_COLOR,
    StealthLook, TintStatus, Vector3, format_under_construction_desc, health_bar_colors,
};

pub use icons::{Anim2DIcon, Icon, IconInfo, IconType};

pub use tint_envelope::{
    DEF_ATTACK_FRAMES, DEF_DECAY_FRAMES, DEF_SUSTAIN_FRAMES, DEFAULT_TINT_COLOR_FADE_RATE,
    DRAWABLE_FRAMES_PER_FLASH, EnvelopeState, FadingMode, LocoInfo,
    MATERIAL_PASS_OPACITY_FADE_SCALAR, SUSTAIN_INDEFINITELY, TintEnvelope,
    VERY_TRANSPARENT_MATERIAL_PASS_OPACITY, WheelInfo,
};
pub(crate) use tint_envelope::{envelope_state_from_u8, envelope_state_to_u8, snap_denorm};

pub use draw_module::{
    BoneData, DrawModule, FXListRef, LogicDrawModuleSnapshotAdapter, TerrainDecalType,
};
pub use drawable_trait::{Drawable, DrawableDowncast, DrawableExt};
pub use leftover::DrawableType;
pub(crate) use xfer::*;

/// Wave 270: host-only path has no dual-world factory objects.
#[inline]
pub(crate) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Basic drawable implementation
#[derive(Debug)]
pub struct BasicDrawable {
    id: DrawableId,
    object_id: Option<u32>,
    /// C++ `m_drawableInfo` — W3D user-data binding (IDs, not raw Drawable*).
    drawable_info: DrawableInfo,
    template_name: Option<String>,
    position: Vector3,
    instance_transform: Matrix4,
    instance_scale: f32,
    status: DrawableStatus,
    tint_status: TintStatus,
    prev_tint_status: TintStatus,
    visible: bool,
    hidden: bool,
    hidden_by_stealth: bool,
    selected: bool,
    selectable: bool,
    opacity: f32,
    explicit_opacity: f32,
    stealth_opacity: f32,
    effective_stealth_opacity: f32,
    stealth_look: StealthLook,
    tint_color: Vector3,
    tint_envelope: Option<TintEnvelope>,
    selection_flash_envelope: Option<TintEnvelope>,
    icon_info: Option<IconInfo>,
    loco_info: Option<LocoInfo>,
    receives_dynamic_lights: bool,
    terrain_decal_type: TerrainDecalType,
    terrain_decal_size: Vector3,
    decal_opacity: f32,
    decal_opacity_fade_target: f32,
    decal_opacity_fade_rate: f32,
    terrain_decal_handle: Option<crate::radius_decal::ShadowHandle>,
    fade_mode: FadingMode,
    time_to_fade: u32,
    time_elapsed_fade: u32,
    second_material_pass_opacity: f32,
    flash_count: u32,
    flash_color: Vector3,
    expiration_frame: Option<u32>,
    ambient_sound_enabled: bool,
    ambient_sound_enabled_from_script: bool,
    custom_sound_ambient_off: bool,
    custom_sound_ambient_base_name: Option<String>,
    custom_sound_ambient_dynamic_info: Option<DynamicAudioEventInfo>,
    /// Live C++ `m_ambientSound` event after startAmbientSound.
    ambient_sound_event: Option<AudioEventRts>,
    /// C++ `setTimeOfDay` is invoked via `&self` iterate; applied on next `update()`.
    pending_time_of_day: AtomicU8,

    current_frame: u32,
    /// C++ `m_isModelDirty` (`DIRTY_CONDITION_FLAGS`).
    is_model_dirty: bool,
    /// Model condition flags for animation state (matches C++ m_conditionState)
    model_condition_flags: ModelConditionBitFlags,
    /// Wave 965: presentation KindOf Debug names (host empty dual-world).
    presentation_kind_names: Vec<String>,
    /// Wave 965: presentation team indicator residual.
    presentation_indicator_color: Option<(u8, u8, u8)>,
    /// Wave 965: presentation stealth residual.
    presentation_effectively_stealthed: bool,
    /// Wave 1055: host control-group residual (0..9, -1 = none).
    presentation_hotkey_group: i8,
    /// Wave 1058: formation id residual (0 = none).
    presentation_formation_id: u32,
    /// Wave 965: presentation health fraction 0..1.
    presentation_health_pct: f32,
    /// Wave 965: presentation selected residual.
    presentation_selected: bool,
    /// Wave 980: presentation orientation residual (radians).
    presentation_orientation: f32,
    /// Wave 970: presentation veterancy residual.
    presentation_veterancy_level: u8,
    /// Wave 970: presentation construction residual.
    presentation_under_construction: bool,
    presentation_construction_percent: f32,
    /// Wave 1115: C++ OBJECT_STATUS_SOLD residual for construct-percent fail-closed.
    presentation_sold: bool,
    /// Wave 972: icon-pip residual.
    presentation_ammo_pip_total: u8,
    presentation_ammo_pip_full: u8,
    presentation_occupant_count: u8,
    presentation_max_garrison: u8,
    presentation_disabled: bool,
    presentation_is_carbomb: bool,
    /// C++ drawBombed sticky type residual: 0 none, 1 timed, 2 remote.
    presentation_bomb_type: u8,
    /// C++ StickyBombUpdate countdown residual in whole seconds.
    presentation_bomb_timer_seconds: u32,
    presentation_weapon_bonus_enthusiastic: bool,
    /// Wave 983: host healing icon residual.
    presentation_show_healing: bool,
    presentation_healing_icon_type: u8,
    /// Wave 984: garrisoned unit object ids for host contained-flash residual.
    presentation_garrisoned_ids: Vec<u32>,
    /// C++ getHealthBoxDimensions width (0 = default 20 fallback).
    presentation_health_box_width: f32,
    /// C++ getHealthBoxPosition height lift (0 = default +10).
    presentation_health_box_z: f32,

    /// Animation loop duration in frames setAnimationLoopDuration)
    animation_loop_duration: u32,
    /// Animation completion time in frames (matches C++ setAnimationCompletionTime)
    animation_completion_time: u32,
    /// 2D icon overlay data computed each frame (health bar, veterancy, construction, caption).
    /// Replaces C++ direct TheDisplay calls in drawIconUI/drawHealthBar/drawVeterancy/etc.
    pub overlay_data: DrawableOverlayData,
    /// Caption text displayed above the drawable (C++ m_captionDisplayString).
    caption_text: Option<String>,
    /// Team/indicator color propagated to draw modules (C++ setIndicatorColor -> replaceIndicatorColor).
    /// Stored as (r, g, b) where each component is 0-255.
    indicator_color: Option<(u8, u8, u8)>,
    /// Static image initialization flag (C++ s_staticImagesInited).
    static_images_inited: bool,
    /// Volatile C++ `Drawable::m_shroudClearFrame`.
    ///
    /// Direct W3D scene dispatch owns writes to this timestamp.  GameClient's
    /// frozen direct-status pass only reads it to choose fully-obscured state.
    /// It is intentionally not part of the Drawable Xfer layout.
    shroud_clear_state: DrawableShroudClearState,
    /// C++ parity: Drawable::m_drawableFullyObscuredByShroud.
    /// When true, the drawable is completely hidden by fog-of-war and should not render.
    drawable_fully_obscured_by_shroud: bool,
    /// Draw modules attached to this drawable.
    /// C++ parity: `m_modules[MODULETYPE_DRAW - FIRST_DRAWABLE_MODULE_TYPE]`.
    /// Iterated for render dispatch, bone queries, FX, and barrel counts.
    draw_modules: Vec<Box<dyn DrawModule>>,
    /// Bone data for modules without W3D bone systems.
    /// PARITY_NOTE: In C++, this data lives in W3D RenderObjClass → HTreeClass.
    /// Here it's stored inline as a fallback when no W3D draw module is present.
    bone_data: Option<BoneData>,
}

impl DrawableDowncast for BasicDrawable {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl BasicDrawable {
    pub fn new(id: DrawableId) -> Self {
        Self {
            id,
            object_id: None,
            // C++ Drawable.cpp: m_drawableInfo.m_drawable = this; m_ghostObject = NULL;
            drawable_info: DrawableInfo::for_drawable(id.0),
            template_name: None,
            position: Vector3::zero(),
            instance_transform: Matrix4::identity(),
            instance_scale: 1.0,
            // C++ creates with DRAWABLE_STATUS_NONE; first Drawable::draw() then
            // setShadowsEnabled for living non-stealth-detected objects. Seed SHADOWS
            // here so shadow enable is observable after create without requiring a full
            // render pass / GPU shadow mesh (matches GameLogic Drawable residual).
            status: DrawableStatus::SHADOWS,
            tint_status: TintStatus::NONE,
            prev_tint_status: TintStatus::NONE,
            visible: true,
            hidden: false,
            hidden_by_stealth: false,
            selected: false,
            selectable: true,
            opacity: 1.0,
            explicit_opacity: 1.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            stealth_look: StealthLook::None,
            tint_color: Vector3::zero(),
            tint_envelope: None,
            selection_flash_envelope: None,
            icon_info: None,
            loco_info: None,
            receives_dynamic_lights: true,
            terrain_decal_type: TerrainDecalType::None,
            terrain_decal_size: Vector3::zero(),
            decal_opacity: 0.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            terrain_decal_handle: None,
            fade_mode: FadingMode::None,
            time_to_fade: 0,
            time_elapsed_fade: 0,
            second_material_pass_opacity: 0.0,
            flash_count: 0,
            flash_color: Vector3::zero(),
            expiration_frame: None,
            ambient_sound_enabled: true,
            ambient_sound_enabled_from_script: true,
            custom_sound_ambient_off: false,
            custom_sound_ambient_base_name: None,
            custom_sound_ambient_dynamic_info: None,
            ambient_sound_event: None,
            pending_time_of_day: AtomicU8::new(0),

            current_frame: 0,
            is_model_dirty: true,
            model_condition_flags: create_model_condition_flags(),
            presentation_kind_names: Vec::new(),
            presentation_indicator_color: None,
            presentation_effectively_stealthed: false,
            presentation_hotkey_group: -1,
            presentation_formation_id: 0,
            presentation_health_pct: 0.0,
            presentation_selected: false,
            presentation_orientation: 0.0,
            presentation_veterancy_level: 0,
            presentation_under_construction: false,
            presentation_construction_percent: 0.0,
            presentation_sold: false,
            presentation_ammo_pip_total: 0,
            presentation_ammo_pip_full: 0,
            presentation_occupant_count: 0,
            presentation_max_garrison: 0,
            presentation_disabled: false,
            presentation_is_carbomb: false,
            presentation_bomb_type: 0,
            presentation_bomb_timer_seconds: 0,
            presentation_weapon_bonus_enthusiastic: false,
            presentation_show_healing: false,
            presentation_healing_icon_type: 0,
            presentation_garrisoned_ids: Vec::new(),
            presentation_health_box_width: 0.0,
            presentation_health_box_z: 0.0,

            animation_loop_duration: 0,
            animation_completion_time: 0,
            overlay_data: DrawableOverlayData::default(),
            caption_text: None,
            indicator_color: None,
            static_images_inited: false,
            shroud_clear_state: DrawableShroudClearState::default(),
            drawable_fully_obscured_by_shroud: false,
            draw_modules: Vec::new(),
            bone_data: None,
        }
    }
}

/// Concatenated live sources for residual `include_str!` scans.
pub const DRAWABLE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("icons.rs"),
    include_str!("tint_envelope.rs"),
    include_str!("draw_module.rs"),
    include_str!("drawable_trait.rs"),
    include_str!("xfer.rs"),
    include_str!("leftover.rs"),
    include_str!("basic_core.rs"),
    include_str!("basic_visual.rs"),
    include_str!("basic_overlay.rs"),
    include_str!("basic_modules.rs"),
    include_str!("condition_apply.rs"),
    include_str!("hidden_status.rs"),
    include_str!("selection_flash.rs"),
    include_str!("basic_drawable_trait.rs"),
    include_str!("snapshot.rs"),
);
