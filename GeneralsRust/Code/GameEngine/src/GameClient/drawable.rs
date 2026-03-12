// FILE: drawable.rs
// Drawable - graphical GameClient entities bound to GameLogic objects
// Ported from C++ Drawable.h and Drawable.cpp
// Author: Michael S. Booth, March 2001

use crate::Common::game_type::{Real, Bool, Int, UnsignedInt, Color, ObjectID, DrawableID, WeaponSlotType};
use crate::Common::model_state::{ModelConditionFlags, ModelConditionFlagType};
use crate::Common::coord3d::Coord3D;
use crate::WWMath::matrix3d::Matrix3D;
use crate::WWMath::vector3::Vector3;
use crate::GameClient::drawable_info::DrawableInfo;
use crate::GameClient::draw_module::{DrawModule, RGBColor, TerrainDecalType};
use crate::GameClient::tint_envelope::TintEnvelope;
use std::collections::HashMap;

/// Drawable frames per flash
pub const DRAWABLE_FRAMES_PER_FLASH: UnsignedInt = 15; // LOGICFRAMES_PER_SECOND / 2

/// Default constants for color flashing
pub const DEFAULT_HEAL_ICON_WIDTH: UnsignedInt = 32;
pub const DEFAULT_HEAL_ICON_HEIGHT: UnsignedInt = 32;

/// Tint color constants
pub const SICKLY_GREEN_POISONED_COLOR: RGBColor = RGBColor { red: -1.0, green: 1.0, blue: -1.0 };
pub const DARK_GRAY_DISABLED_COLOR: RGBColor = RGBColor { red: -0.5, green: -0.5, blue: -0.5 };
pub const RED_IRRADIATED_COLOR: RGBColor = RGBColor { red: 1.0, green: -1.0, blue: -1.0 };
pub const SUBDUAL_DAMAGE_COLOR: RGBColor = RGBColor { red: -0.2, green: -0.2, blue: 0.8 };
pub const FRENZY_COLOR: RGBColor = RGBColor { red: 0.2, green: -0.2, blue: -0.2 };
pub const FRENZY_COLOR_INFANTRY: RGBColor = RGBColor { red: 0.0, green: -0.7, blue: -0.7 };

/// Material pass opacity constants
const VERY_TRANSPARENT_MATERIAL_PASS_OPACITY: Real = 0.001;
const MATERIAL_PASS_OPACITY_FADE_SCALAR: Real = 0.8;

/// Maximum number of drawable icons
pub const MAX_ICONS: usize = 14;

/// Drawable icon types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DrawableIconType {
    Invalid = -1,
    DefaultHeal = 0,
    StructureHeal,
    VehicleHeal,
    DemoralizedObsolete,
    BombTimed,
    BombRemote,
    Disabled,
    BattleplanBombard,
    BattleplanHoldTheLine,
    BattleplanSearchAndDestroy,
    Emoticon,
    Enthusiastic,
    EnthusiasticSubliminal,
    CarBomb,
}

/// Drawable icon information
#[derive(Debug)]
pub struct DrawableIconInfo {
    /// Icons indexed by type
    icons: HashMap<DrawableIconType, Option<*mut std::ffi::c_void>>, // Anim2D pointers

    /// Frame to keep icon until
    keep_till_frame: HashMap<DrawableIconType, UnsignedInt>,
}

impl DrawableIconInfo {
    pub fn new() -> Self {
        Self {
            icons: HashMap::new(),
            keep_till_frame: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.icons.clear();
        self.keep_till_frame.clear();
    }

    pub fn kill_icon(&mut self, icon_type: DrawableIconType) {
        self.icons.remove(&icon_type);
        self.keep_till_frame.remove(&icon_type);
    }
}

impl Default for DrawableIconInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Wheel information for vehicles
#[derive(Debug, Clone, Copy)]
pub struct WheelInfo {
    /// Height offsets for tires due to suspension sway
    pub front_left_height_offset: Real,
    pub front_right_height_offset: Real,
    pub rear_left_height_offset: Real,
    pub rear_right_height_offset: Real,

    /// Wheel angle. 0 = straight, >0 left, <0 right
    pub wheel_angle: Real,

    /// Counter for frames airborne
    pub frames_airborne_counter: Int,

    /// How many frames it was in the air
    pub frames_airborne: Int,
}

impl WheelInfo {
    pub fn new() -> Self {
        Self {
            front_left_height_offset: 0.0,
            front_right_height_offset: 0.0,
            rear_left_height_offset: 0.0,
            rear_right_height_offset: 0.0,
            wheel_angle: 0.0,
            frames_airborne_counter: 0,
            frames_airborne: 0,
        }
    }
}

impl Default for WheelInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Locomotor information for drawables
#[derive(Debug, Clone)]
pub struct DrawableLocoInfo {
    /// Pitch of the entire drawable
    pub pitch: Real,
    pub pitch_rate: Real,

    /// Roll of the entire drawable
    pub roll: Real,
    pub roll_rate: Real,

    /// Yaw for entire drawable
    pub yaw: Real,

    /// Pitch due to impact/acceleration
    pub acceleration_pitch: Real,
    pub acceleration_pitch_rate: Real,

    /// Roll due to acceleration
    pub acceleration_roll: Real,
    pub acceleration_roll_rate: Real,

    /// Fake Z velocity and current height
    pub overlap_z_vel: Real,
    pub overlap_z: Real,

    /// For wobbling
    pub wobble: Real,

    /// For the swimmy soft hover of a helicopter
    pub yaw_modulator: Real,
    pub pitch_modulator: Real,

    /// Wheel offset & angle info for wheeled locomotor
    pub wheel_info: WheelInfo,
}

impl DrawableLocoInfo {
    pub fn new() -> Self {
        Self {
            pitch: 0.0,
            pitch_rate: 0.0,
            roll: 0.0,
            roll_rate: 0.0,
            yaw: 0.0,
            acceleration_pitch: 0.0,
            acceleration_pitch_rate: 0.0,
            acceleration_roll: 0.0,
            acceleration_roll_rate: 0.0,
            overlap_z_vel: 0.0,
            overlap_z: 0.0,
            wobble: 1.0,
            yaw_modulator: 0.0,
            pitch_modulator: 0.0,
            wheel_info: WheelInfo::new(),
        }
    }
}

impl Default for DrawableLocoInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Stealth look types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthLookType {
    /// Unit is not stealthed at all
    None,

    /// Unit is stealthed-but-visible due to friendly status
    VisibleFriendly,

    /// We can have units that are disguised (instead of invisible)
    DisguisedEnemy,

    /// Unit is stealthed and invisible, but a second material pass is added
    /// to reveal the invisible unit as with heat vision
    VisibleDetected,

    /// Unit is stealthed-but-visible due to being detected,
    /// and rendered in heatvision effect second material pass
    VisibleFriendlyDetected,

    /// Unit is stealthed-and-invisible
    Invisible,
}

/// Drawable status bits
#[derive(Debug, Clone, Copy)]
pub struct DrawableStatus(u32);

impl DrawableStatus {
    pub const NONE: Self = Self(0x00000000);
    pub const DRAWS_IN_MIRROR: Self = Self(0x00000001);
    pub const SHADOWS: Self = Self(0x00000002);
    pub const TINT_COLOR_LOCKED: Self = Self(0x00000004);
    pub const NO_STATE_PARTICLES: Self = Self(0x00000008);
    pub const NO_SAVE: Self = Self(0x00000010);

    pub fn is_set(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    pub fn set(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn clear(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

/// Tint status bits
#[derive(Debug, Clone, Copy)]
pub struct TintStatus(u32);

impl TintStatus {
    pub const DISABLED: Self = Self(0x00000001);
    pub const IRRADIATED: Self = Self(0x00000002);
    pub const POISONED: Self = Self(0x00000004);
    pub const GAINING_SUBDUAL_DAMAGE: Self = Self(0x00000008);
    pub const FRENZY: Self = Self(0x00000010);

    pub fn is_set(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    pub fn set(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn clear(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

/// Body damage type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyDamageType {
    /// Unit should appear in pristine condition
    Pristine,

    /// Unit has been damaged
    Damaged,

    /// Unit is extremely damaged / nearly destroyed
    ReallyDamaged,

    /// Unit has been reduced to rubble/corpse/exploded-hulk, etc
    Rubble,
}

/// Fading mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FadingMode {
    None,
    FadingIn,
    FadingOut,
}

/// Physics transform information
#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicsXformInfo {
    pub total_pitch: Real,
    pub total_roll: Real,
    pub total_yaw: Real,
    pub total_z: Real,
}

/// Main Drawable class - graphical entity associated with GameLogic objects
pub struct Drawable {
    /// Unique drawable ID
    id: DrawableID,

    /// Linked list pointers
    next_drawable: Option<Box<Drawable>>,
    prev_drawable: *mut Drawable,

    /// Bound object (if any)
    object: ObjectID,

    /// Drawable info for W3D binding
    drawable_info: DrawableInfo,

    /// Current model condition flags
    condition_state: ModelConditionFlags,

    /// Status bits
    status: DrawableStatus,

    /// Tint status bits
    tint_status: TintStatus,
    prev_tint_status: TintStatus,

    /// Draw modules
    draw_modules: Vec<Box<dyn DrawModule>>,

    /// Client update modules (optional)
    client_update_modules: Vec<*mut std::ffi::c_void>,

    /// Selection flash envelope (lazily allocated)
    selection_flash_envelope: Option<Box<TintEnvelope>>,

    /// Color tint envelope (lazily allocated)
    color_tint_envelope: Option<Box<TintEnvelope>>,

    /// Terrain decal type
    terrain_decal_type: TerrainDecalType,

    /// Opacity values
    explicit_opacity: Real,
    stealth_opacity: Real,
    effective_stealth_opacity: Real,
    second_material_pass_opacity: Real,

    /// Decal opacity fading
    decal_opacity_fade_target: Real,
    decal_opacity_fade_rate: Real,
    decal_opacity: Real,

    /// Fading mode and timing
    fade_mode: FadingMode,
    time_elapsed_fade: UnsignedInt,
    time_to_fade: UnsignedInt,

    /// Shroud clear frame
    shroud_clear_frame: UnsignedInt,

    /// Locomotor info (lazily allocated)
    loco_info: Option<Box<DrawableLocoInfo>>,

    /// Ambient sound (lazily allocated)
    ambient_sound: Option<*mut std::ffi::c_void>,
    custom_sound_ambient_info: Option<*mut std::ffi::c_void>,

    /// Stealth look
    stealth_look: StealthLookType,

    /// Flash parameters
    flash_count: Int,
    flash_color: Color,

    /// Instance matrix and scale
    instance: Matrix3D,
    instance_scale: Real,
    instance_is_identity: Bool,

    /// Display strings (lazily allocated)
    construct_display_string: Option<*mut std::ffi::c_void>,
    caption_display_string: Option<*mut std::ffi::c_void>,
    group_number: Option<*mut std::ffi::c_void>,

    /// Last construct percent displayed
    last_construct_displayed: Real,

    /// Expiration date (0 = never expires)
    expiration_date: UnsignedInt,

    /// Icon info (lazily allocated)
    icon_info: Option<Box<DrawableIconInfo>>,

    /// Boolean flags
    selected: Bool,
    hidden: Bool,
    hidden_by_stealth: Bool,
    drawable_fully_obscured_by_shroud: Bool,
    ambient_sound_enabled: Bool,
    ambient_sound_enabled_from_script: Bool,
    receives_dynamic_lights: Bool,
    is_model_dirty: Bool,

    /// Position
    position: Coord3D,
}

impl Drawable {
    /// Create a new drawable
    pub fn new(template_name: &str, status_bits: DrawableStatus) -> Self {
        Self {
            id: DrawableID::INVALID,
            next_drawable: None,
            prev_drawable: std::ptr::null_mut(),
            object: ObjectID::INVALID,
            drawable_info: DrawableInfo::new(),
            condition_state: ModelConditionFlags::new(),
            status: status_bits,
            tint_status: TintStatus(0),
            prev_tint_status: TintStatus(0),
            draw_modules: Vec::new(),
            client_update_modules: Vec::new(),
            selection_flash_envelope: None,
            color_tint_envelope: None,
            terrain_decal_type: TerrainDecalType::None,
            explicit_opacity: 1.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            second_material_pass_opacity: 0.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            decal_opacity: 0.0,
            fade_mode: FadingMode::None,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            shroud_clear_frame: 0,
            loco_info: None,
            ambient_sound: None,
            custom_sound_ambient_info: None,
            stealth_look: StealthLookType::None,
            flash_count: 0,
            flash_color: 0,
            instance: Matrix3D::identity(),
            instance_scale: 1.0,
            instance_is_identity: true,
            construct_display_string: None,
            caption_display_string: None,
            group_number: None,
            last_construct_displayed: -1.0,
            expiration_date: 0,
            icon_info: None,
            selected: false,
            hidden: false,
            hidden_by_stealth: false,
            drawable_fully_obscured_by_shroud: false,
            ambient_sound_enabled: true,
            ambient_sound_enabled_from_script: true,
            receives_dynamic_lights: true,
            is_model_dirty: true,
            position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    /// Get drawable ID
    pub fn get_id(&self) -> DrawableID {
        self.id
    }

    /// Set drawable ID
    pub fn set_id(&mut self, id: DrawableID) {
        self.id = id;
    }

    /// Get bound object ID
    pub fn get_object(&self) -> ObjectID {
        self.object
    }

    /// Bind to an object
    pub fn bind_to_object(&mut self, obj_id: ObjectID) {
        self.object = obj_id;
    }

    /// Get drawable info
    pub fn get_drawable_info(&self) -> &DrawableInfo {
        &self.drawable_info
    }

    /// Get mutable drawable info
    pub fn get_drawable_info_mut(&mut self) -> &mut DrawableInfo {
        &mut self.drawable_info
    }

    /// Set position
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.position = *pos;
    }

    /// Get position
    pub fn get_position(&self) -> &Coord3D {
        &self.position
    }

    /// Check if selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set selected (internal use only)
    pub fn friend_set_selected(&mut self) {
        if !self.selected {
            self.selected = true;
            self.on_selected();
        }
    }

    /// Clear selected (internal use only)
    pub fn friend_clear_selected(&mut self) {
        if self.selected {
            self.selected = false;
            self.on_unselected();
        }
    }

    /// On selected callback
    fn on_selected(&mut self) {
        self.flash_as_selected(None);
    }

    /// On unselected callback
    fn on_unselected(&mut self) {
        // Nothing in base implementation
    }

    /// Get model condition flags
    pub fn get_model_condition_flags(&self) -> &ModelConditionFlags {
        &self.condition_state
    }

    /// Set model condition state
    pub fn set_model_condition_state(&mut self, flag: ModelConditionFlagType) {
        self.clear_and_set_model_condition_state(ModelConditionFlagType::Invalid, flag);
    }

    /// Clear model condition state
    pub fn clear_model_condition_state(&mut self, flag: ModelConditionFlagType) {
        self.clear_and_set_model_condition_state(flag, ModelConditionFlagType::Invalid);
    }

    /// Clear and set model condition state
    pub fn clear_and_set_model_condition_state(
        &mut self,
        clear: ModelConditionFlagType,
        set: ModelConditionFlagType,
    ) {
        if clear != ModelConditionFlagType::Invalid {
            self.condition_state.clear(clear);
        }
        if set != ModelConditionFlagType::Invalid {
            self.condition_state.set(set);
        }
        self.is_model_dirty = true;
    }

    /// Replace model condition flags
    pub fn replace_model_condition_flags(&mut self, flags: &ModelConditionFlags) {
        self.condition_state = flags.clone();
        self.is_model_dirty = true;
    }

    /// Set drawable hidden
    pub fn set_drawable_hidden(&mut self, hidden: Bool) {
        self.hidden = hidden;
        self.update_hidden_status();
    }

    /// Check if effectively hidden
    pub fn is_drawable_effectively_hidden(&self) -> bool {
        self.hidden || self.hidden_by_stealth
    }

    /// Update hidden status
    fn update_hidden_status(&mut self) {
        let effective_hidden = self.is_drawable_effectively_hidden();
        for module in &mut self.draw_modules {
            if let Some(obj_draw) = module.get_object_draw_interface() {
                obj_draw.set_hidden(effective_hidden);
            }
        }
    }

    /// Set stealth look
    pub fn set_stealth_look(&mut self, look: StealthLookType) {
        if look == self.stealth_look {
            return;
        }

        self.stealth_opacity = 1.0;

        match look {
            StealthLookType::None => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }

            StealthLookType::VisibleFriendly | StealthLookType::VisibleFriendlyDetected => {
                // Would need global data access for opacity value
                self.stealth_opacity = 0.5; // Default friendly opacity
                self.hidden_by_stealth = false;

                if look == StealthLookType::VisibleFriendlyDetected {
                    self.second_material_pass_opacity = 1.0;
                } else {
                    self.second_material_pass_opacity = 0.0;
                }
            }

            StealthLookType::DisguisedEnemy => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }

            StealthLookType::VisibleDetected => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 1.0;
            }

            StealthLookType::Invisible => {
                self.hidden_by_stealth = true;
                self.second_material_pass_opacity = 0.0;
            }
        }

        self.stealth_look = look;
        self.update_hidden_status();
    }

    /// Get stealth look
    pub fn get_stealth_look(&self) -> StealthLookType {
        self.stealth_look
    }

    /// Color flash the drawable
    pub fn color_flash(
        &mut self,
        color: Option<&RGBColor>,
        decay_frames: UnsignedInt,
        attack_frames: UnsignedInt,
        sustain_at_peak: UnsignedInt,
    ) {
        if self.color_tint_envelope.is_none() {
            self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
        }

        if let Some(envelope) = &mut self.color_tint_envelope {
            if let Some(c) = color {
                envelope.play(c, attack_frames, decay_frames, sustain_at_peak);
            } else {
                let white = RGBColor::from_int(0xffffffff);
                envelope.play(&white, attack_frames, decay_frames, sustain_at_peak);
            }
        }

        // Make sure the tint color is unlocked so we "fade back down" to normal
        self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
    }

    /// Tint the drawable a specified color
    pub fn color_tint(&mut self, color: Option<&RGBColor>) {
        if let Some(c) = color {
            // Set the color via color flash
            self.color_flash(Some(c), 0, 0, 1);

            // Lock the tint color so the flash never "fades back down"
            self.status.set(DrawableStatus::TINT_COLOR_LOCKED);
        } else {
            if self.color_tint_envelope.is_none() {
                self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
            }

            // Remove the tint applied to the object
            if let Some(envelope) = &mut self.color_tint_envelope {
                envelope.rest();
            }

            // Set the tint as unlocked so we can flash and stuff again
            self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
        }
    }

    /// Flash as selected
    pub fn flash_as_selected(&mut self, color: Option<&RGBColor>) {
        if self.selection_flash_envelope.is_none() {
            self.selection_flash_envelope = Some(Box::new(TintEnvelope::new()));
        }

        if let Some(envelope) = &mut self.selection_flash_envelope {
            if let Some(c) = color {
                envelope.play(c, 0, 4, 0);
            } else {
                // Default white flash
                let white = RGBColor::from_int(0xffffffff);
                envelope.play(&white, 0, 4, 0);
            }
        }
    }

    /// Get tint color
    pub fn get_tint_color(&self) -> Option<&Vector3> {
        self.color_tint_envelope
            .as_ref()
            .and_then(|e| if e.is_effective() { Some(e.get_color()) } else { None })
    }

    /// Get selection color
    pub fn get_selection_color(&self) -> Option<&Vector3> {
        self.selection_flash_envelope
            .as_ref()
            .and_then(|e| if e.is_effective() { Some(e.get_color()) } else { None })
    }

    /// Fade out the drawable
    pub fn fade_out(&mut self, frames: UnsignedInt) {
        self.explicit_opacity = 1.0;
        self.fade_mode = FadingMode::FadingOut;
        self.time_to_fade = frames;
        self.time_elapsed_fade = 0;
    }

    /// Fade in the drawable
    pub fn fade_in(&mut self, frames: UnsignedInt) {
        self.explicit_opacity = 0.0;
        self.fade_mode = FadingMode::FadingIn;
        self.time_to_fade = frames;
        self.time_elapsed_fade = 0;
    }

    /// Set drawable opacity
    pub fn set_drawable_opacity(&mut self, value: Real) {
        self.explicit_opacity = value;
    }

    /// Get effective opacity
    pub fn get_effective_opacity(&self) -> Real {
        self.explicit_opacity * self.effective_stealth_opacity
    }

    /// Set effective opacity with pulsing
    pub fn set_effective_opacity(&mut self, pulse_factor: Real, explicit_opacity: Option<Real>) {
        if let Some(opacity) = explicit_opacity {
            self.stealth_opacity = opacity.max(0.0).min(1.0);
        }

        let pf = pulse_factor.max(0.0).min(1.0);
        let pulse_margin = 1.0 - self.stealth_opacity;
        let pulse_amount = pulse_margin * pf;

        self.effective_stealth_opacity = self.stealth_opacity + pulse_amount;
    }

    /// Imitate stealth look from another drawable
    pub fn imitate_stealth_look(&mut self, other: &Drawable) {
        self.stealth_opacity = other.stealth_opacity;
        self.explicit_opacity = other.explicit_opacity;
        self.effective_stealth_opacity = other.effective_stealth_opacity;
        self.hidden = other.is_drawable_effectively_hidden();
        self.hidden_by_stealth = other.is_drawable_effectively_hidden();
        self.stealth_look = other.stealth_look;
        self.second_material_pass_opacity = other.second_material_pass_opacity;
    }

    /// Set shadows enabled
    pub fn set_shadows_enabled(&mut self, enable: Bool) {
        if enable {
            self.status.set(DrawableStatus::SHADOWS);
        } else {
            self.status.clear(DrawableStatus::SHADOWS);
        }

        for module in &mut self.draw_modules {
            module.set_shadows_enabled(enable);
        }
    }

    /// Get shadows enabled
    pub fn get_shadows_enabled(&self) -> bool {
        self.status.is_set(DrawableStatus::SHADOWS)
    }

    /// Release shadows
    pub fn release_shadows(&mut self) {
        for module in &mut self.draw_modules {
            module.release_shadows();
        }
    }

    /// Allocate shadows
    pub fn allocate_shadows(&mut self) {
        for module in &mut self.draw_modules {
            module.allocate_shadows();
        }
    }

    /// Set fully obscured by shroud
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: Bool) {
        if self.drawable_fully_obscured_by_shroud != fully_obscured {
            for module in &mut self.draw_modules {
                module.set_fully_obscured_by_shroud(fully_obscured);
            }
            self.drawable_fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// Get fully obscured by shroud
    pub fn get_fully_obscured_by_shroud(&self) -> bool {
        self.drawable_fully_obscured_by_shroud
    }

    /// Set terrain decal
    pub fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        if self.terrain_decal_type == decal_type {
            return;
        }

        self.terrain_decal_type = decal_type;

        // Only the first draw module gets a decal to prevent stacking
        if let Some(first_module) = self.draw_modules.first_mut() {
            first_module.set_terrain_decal(decal_type);
        }
    }

    /// Set terrain decal size
    pub fn set_terrain_decal_size(&mut self, x: Real, y: Real) {
        if let Some(first_module) = self.draw_modules.first_mut() {
            first_module.set_terrain_decal_size(x, y);
        }
    }

    /// Set terrain decal fade target
    pub fn set_terrain_decal_fade_target(&mut self, target: Real, rate: Real) {
        if self.decal_opacity_fade_target != target {
            self.decal_opacity_fade_target = target;
            self.decal_opacity_fade_rate = rate;
        }
    }

    /// Get terrain decal type
    pub fn get_terrain_decal_type(&self) -> TerrainDecalType {
        self.terrain_decal_type
    }

    /// Set instance matrix
    pub fn set_instance_matrix(&mut self, instance: &Matrix3D) {
        self.instance = *instance;
        self.instance_is_identity = instance.is_identity();
    }

    /// Get instance matrix
    pub fn get_instance_matrix(&self) -> &Matrix3D {
        &self.instance
    }

    /// Check if instance is identity
    pub fn is_instance_identity(&self) -> bool {
        self.instance_is_identity
    }

    /// Set instance scale
    pub fn set_instance_scale(&mut self, scale: Real) {
        self.instance_scale = scale;
    }

    /// Get instance scale
    pub fn get_instance_scale(&self) -> Real {
        self.instance_scale
    }

    /// Get scale
    pub fn get_scale(&self) -> Real {
        self.instance_scale
    }

    /// React to body damage state change
    pub fn react_to_body_damage_state_change(&mut self, new_state: BodyDamageType) {
        let damage_map = [
            ModelConditionFlagType::Invalid,
            ModelConditionFlagType::Damaged,
            ModelConditionFlagType::ReallyDamaged,
            ModelConditionFlagType::Rubble,
        ];

        let new_damage_flag = damage_map[new_state as usize];

        // Clear all damage flags
        self.condition_state.clear(ModelConditionFlagType::Damaged);
        self.condition_state.clear(ModelConditionFlagType::ReallyDamaged);
        self.condition_state.clear(ModelConditionFlagType::Rubble);

        // Set new damage flag if valid
        if new_damage_flag != ModelConditionFlagType::Invalid {
            self.condition_state.set(new_damage_flag);
        }

        self.is_model_dirty = true;
    }

    /// Set tint status
    pub fn set_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.set(status_bits);
    }

    /// Clear tint status
    pub fn clear_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.clear(status_bits);
    }

    /// Test tint status
    pub fn test_tint_status(&self, status_bits: TintStatus) -> bool {
        self.tint_status.is_set(status_bits)
    }

    /// Update drawable (called each frame)
    pub fn update_drawable(&mut self, current_frame: UnsignedInt) {
        // Handle fading in or out
        if self.fade_mode != FadingMode::None {
            let numer = if self.fade_mode == FadingMode::FadingIn {
                self.time_elapsed_fade as Real
            } else {
                (self.time_to_fade - self.time_elapsed_fade) as Real
            };

            self.set_drawable_opacity(numer / self.time_to_fade as Real);
            self.time_elapsed_fade += 1;

            if self.time_elapsed_fade > self.time_to_fade {
                self.fade_mode = FadingMode::None;
            }
        }

        // Handle terrain decal opacity fading
        if self.terrain_decal_type != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                if let Some(first_module) = self.draw_modules.first_mut() {
                    first_module.set_terrain_decal_opacity(self.decal_opacity);
                }
                self.decal_opacity += self.decal_opacity_fade_rate;

                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.set_terrain_decal(TerrainDecalType::None);
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                    if let Some(first_module) = self.draw_modules.first_mut() {
                        first_module.set_terrain_decal_opacity(self.decal_opacity);
                    }
                }
            }
        } else {
            self.decal_opacity = 0.0;
        }

        // Handle expiration
        if self.expiration_date != 0 && current_frame >= self.expiration_date {
            // Should destroy drawable
        }

        // Handle flashing
        if self.flash_count > 0 && (current_frame % DRAWABLE_FRAMES_PER_FLASH) == 0 {
            let tmp = RGBColor::from_int(self.flash_color);
            self.color_flash(Some(&tmp), 4, 0, 0);
            self.flash_count -= 1;
        }

        // Handle tint status changes
        if self.prev_tint_status.0 != self.tint_status.0 {
            if self.test_tint_status(TintStatus::DISABLED) {
                if self.color_tint_envelope.is_none() {
                    self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
                }
                if let Some(envelope) = &mut self.color_tint_envelope {
                    envelope.play(&DARK_GRAY_DISABLED_COLOR, 30, 30, crate::GameClient::tint_envelope::SUSTAIN_INDEFINITELY);
                }
            } else if self.test_tint_status(TintStatus::GAINING_SUBDUAL_DAMAGE) {
                if self.color_tint_envelope.is_none() {
                    self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
                }
                if let Some(envelope) = &mut self.color_tint_envelope {
                    envelope.play(&SUBDUAL_DAMAGE_COLOR, 150, 150, crate::GameClient::tint_envelope::SUSTAIN_INDEFINITELY);
                }
            } else if self.test_tint_status(TintStatus::FRENZY) {
                if self.color_tint_envelope.is_none() {
                    self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
                }
                if let Some(envelope) = &mut self.color_tint_envelope {
                    // Would check KINDOF_INFANTRY here
                    envelope.play(&FRENZY_COLOR, 30, 30, crate::GameClient::tint_envelope::SUSTAIN_INDEFINITELY);
                }
            } else {
                // NO TINTING SHOULD BE PRESENT
                if self.color_tint_envelope.is_none() {
                    self.color_tint_envelope = Some(Box::new(TintEnvelope::new()));
                }
                if let Some(envelope) = &mut self.color_tint_envelope {
                    envelope.release();
                }
            }
        }

        self.prev_tint_status = self.tint_status;

        // Update envelopes
        if let Some(envelope) = &mut self.color_tint_envelope {
            envelope.update();
        }

        if let Some(envelope) = &mut self.selection_flash_envelope {
            envelope.update();
        }
    }

    /// Draw the drawable
    pub fn draw(&mut self, transform: &Matrix3D) {
        // Handle second material pass opacity fading
        if !self.test_tint_status(TintStatus::FRENZY) {
            if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }

        // Early out if hidden
        if self.hidden || self.hidden_by_stealth || self.drawable_fully_obscured_by_shroud {
            return;
        }

        // Build transform matrix
        let mut transform_mtx = *transform;

        if !self.instance_is_identity {
            transform_mtx = transform_mtx.multiply(&self.instance);
        }

        // Apply physics transform would go here

        // Draw all modules
        for module in &mut self.draw_modules {
            module.do_draw_module(&transform_mtx);
        }
    }

    /// Get locomotor info (lazily allocated)
    pub fn get_loco_info(&mut self) -> &mut DrawableLocoInfo {
        if self.loco_info.is_none() {
            self.loco_info = Some(Box::new(DrawableLocoInfo::new()));
        }
        self.loco_info.as_mut().unwrap()
    }

    /// Get icon info (lazily allocated)
    pub fn get_icon_info(&mut self) -> &mut DrawableIconInfo {
        if self.icon_info.is_none() {
            self.icon_info = Some(Box::new(DrawableIconInfo::new()));
        }
        self.icon_info.as_mut().unwrap()
    }

    /// Kill an icon
    pub fn kill_icon(&mut self, icon_type: DrawableIconType) {
        if let Some(info) = &mut self.icon_info {
            info.kill_icon(icon_type);
        }
    }

    /// Set flash count
    pub fn set_flash_count(&mut self, count: Int) {
        self.flash_count = count;
    }

    /// Get flash count
    pub fn get_flash_count(&self) -> Int {
        self.flash_count
    }

    /// Set flash color
    pub fn set_flash_color(&mut self, color: Color) {
        self.flash_color = color;
    }

    /// Saturate RGB color (utility function)
    pub fn saturate_rgb(color: &mut RGBColor, factor: Real) {
        color.red *= factor;
        color.green *= factor;
        color.blue *= factor;

        let half_factor = factor * 0.5;

        color.red -= half_factor;
        color.green -= half_factor;
        color.blue -= half_factor;
    }
}

impl Default for Drawable {
    fn default() -> Self {
        Self::new("", DrawableStatus::NONE)
    }
}
