//! W3DModelDraw - Main 3D model drawing module
//!
//! Port of C++ W3DModelDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h
//!
//! This is the primary draw module for rendering 3D models with:
//! - Model condition-based state switching
//! - Skeletal animation
//! - Weapon fire effects
//! - Particle systems
//! - Turret positioning
//! - Weapon recoil
//! - Shadows and decals

use super::draw_module::*;
use crate::common::*;
use crate::helpers::{
    game_client_random_value, game_client_random_value_real, BoneOverrideState,
    MeshUvOverrideState, ModelDrawState, SubObjectVisibilityState, TheGameClient, TheGameLogic,
    TheParticleSystemManager,
};
use crate::upgrade::modules::model_condition::parse_model_condition_flag;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, ModuleType, TimeOfDay,
};
use log::warn;
use std::any::Any;
use std::collections::HashMap;

/// Animation information for a model
#[derive(Debug, Clone)]
pub struct W3DAnimationInfo {
    /// Name of animation
    pub name: AsciiString,

    /// Distance covered by a single loop (for movement animations)
    pub distance_covered: Real,

    /// Natural duration in milliseconds
    pub natural_duration_ms: Real,

    /// Whether this is an idle animation (picks random anim when complete)
    pub is_idle_anim: bool,
}

impl W3DAnimationInfo {
    pub fn new(name: AsciiString, is_idle: bool, distance_covered: Real) -> Self {
        Self {
            name,
            distance_covered,
            natural_duration_ms: 0.0, // Calculated from animation data
            is_idle_anim: is_idle,
        }
    }
}

/// Animation mode for render objects
///
/// Reference: RenderObjClass::AnimMode in W3DModelDraw.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimMode {
    Manual,        // Manual control
    Loop,          // Loop continuously
    Once,          // Play once
    LoopPingPong,  // Loop with reverse playback
    LoopBackwards, // Loop playing backwards
    OnceBackwards, // Play once backwards
}

/// Particle system attachment to bone
#[derive(Debug, Clone)]
pub struct ParticleSysBoneInfo {
    /// Name of bone to attach to
    pub bone_name: AsciiString,

    /// Particle system template
    pub particle_system: AsciiString, // Reference to particle template
}

/// Pristine bone information (default pose)
#[derive(Debug, Clone)]
pub struct PristineBoneInfo {
    /// Transform matrix in default pose
    pub transform: Matrix3D,

    /// Bone index in skeleton
    pub bone_index: i32,
}

/// Turret information for model condition
#[derive(Debug, Clone)]
pub struct TurretInfo {
    /// Name key for turret angle bone
    pub turret_angle_name_key: NameKeyType,

    /// Name key for turret pitch bone
    pub turret_pitch_name_key: NameKeyType,

    /// Art-defined turret angle offset
    pub turret_art_angle: Real,

    /// Art-defined turret pitch offset
    pub turret_art_pitch: Real,

    /// Calculated bone index for angle
    pub turret_angle_bone: i32,

    /// Calculated bone index for pitch
    pub turret_pitch_bone: i32,
}

impl TurretInfo {
    pub fn new() -> Self {
        Self {
            turret_angle_name_key: 0,
            turret_pitch_name_key: 0,
            turret_art_angle: 0.0,
            turret_art_pitch: 0.0,
            turret_angle_bone: 0,
            turret_pitch_bone: 0,
        }
    }
}

impl Default for TurretInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Weapon barrel information
#[derive(Debug, Clone)]
pub struct WeaponBarrelInfo {
    /// Recoil bone index
    pub recoil_bone: i32,

    /// FX bone index
    pub fx_bone: i32,

    /// Muzzle flash bone index
    pub muzzle_flash_bone: i32,

    /// Projectile launch offset matrix
    pub projectile_offset_mtx: Matrix3D,
}

impl WeaponBarrelInfo {
    pub fn new() -> Self {
        Self {
            recoil_bone: 0,
            fx_bone: 0,
            muzzle_flash_bone: 0,
            projectile_offset_mtx: Matrix3D::IDENTITY,
        }
    }
}

impl Default for WeaponBarrelInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Hide/show sub-object directive
#[derive(Debug, Clone)]
pub struct HideShowSubObjInfo {
    /// Name of sub-object
    pub sub_obj_name: AsciiString,

    /// True to hide, false to show
    pub hide: bool,
}

/// Model condition state information
///
/// Defines which model and animations to use for a given set of model conditions.
///
/// Reference: ModelConditionInfo in W3DModelDraw.h
#[derive(Debug, Clone)]
pub struct ModelConditionInfo {
    /// Condition flags this state matches
    pub conditions_yes: Vec<ModelConditionFlags>,

    /// Model name to use
    pub model_name: AsciiString,

    /// Sub-objects to hide/show
    pub hide_show_list: Vec<HideShowSubObjInfo>,

    /// Public bones (accessible to code)
    pub public_bones: Vec<AsciiString>,

    /// Weapon fire FX bone names
    pub weapon_fire_fx_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon recoil bone names
    pub weapon_recoil_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon muzzle flash bone names
    pub weapon_muzzle_flash: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon projectile launch bone names
    pub weapon_projectile_launch_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon projectile hide/show bone names.
    pub weapon_projectile_hide_show_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Animations for this state
    pub animations: Vec<W3DAnimationInfo>,

    /// Transition key
    pub transition_key: NameKeyType,

    /// Allow to finish key
    pub allow_to_finish_key: NameKeyType,

    /// Bit flags from INI `Flags`.
    pub flags: u32,

    /// Parse-time flags used to preserve C++ default-state animation behavior.
    pub ini_read_flags: u32,

    /// Animation mode
    pub anim_mode: AnimMode,

    /// Particle systems attached to bones
    pub particle_sys_bones: Vec<ParticleSysBoneInfo>,

    /// Animation speed randomization (min factor)
    pub anim_min_speed_factor: Real,

    /// Animation speed randomization (max factor)
    pub anim_max_speed_factor: Real,

    /// Transition source condition key.
    pub transition_from_key: NameKeyType,

    /// Transition destination condition key.
    pub transition_to_key: NameKeyType,

    /// Pristine bone transforms
    pub pristine_bones: HashMap<NameKeyType, PristineBoneInfo>,

    /// Turret information (up to MAX_TURRETS)
    pub turrets: Vec<TurretInfo>,

    /// Weapon barrel information per slot
    pub weapon_barrels: Vec<Vec<WeaponBarrelInfo>>,

    /// Runtime validation flags mirroring C++ `m_validStuff`.
    valid_stuff: u8,
}

impl ModelConditionInfo {
    pub fn new() -> Self {
        Self {
            conditions_yes: Vec::new(),
            model_name: AsciiString::new(),
            hide_show_list: Vec::new(),
            public_bones: Vec::new(),
            weapon_fire_fx_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_recoil_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_muzzle_flash: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_projectile_launch_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_projectile_hide_show_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            animations: Vec::new(),
            transition_key: 0,
            allow_to_finish_key: 0,
            flags: 0,
            ini_read_flags: 0,
            anim_mode: AnimMode::Once,
            particle_sys_bones: Vec::new(),
            anim_min_speed_factor: 1.0,
            anim_max_speed_factor: 1.0,
            transition_from_key: 0,
            transition_to_key: 0,
            pristine_bones: HashMap::new(),
            turrets: Vec::new(),
            weapon_barrels: vec![Vec::new(); WEAPONSLOT_COUNT],
            valid_stuff: 0,
        }
    }

    fn find_pristine_bone(&self, bone_key: NameKeyType) -> Option<&PristineBoneInfo> {
        if bone_key == NAMEKEY_INVALID {
            return None;
        }
        self.pristine_bones.get(&bone_key)
    }

    fn find_pristine_bone_by_name(
        &self,
        bone_name: &str,
    ) -> Option<(NameKeyType, &PristineBoneInfo)> {
        if bone_name.is_empty() {
            return None;
        }
        let bone_key = name_key_generate(bone_name);
        self.find_pristine_bone(bone_key)
            .map(|info| (bone_key, info))
    }

    fn pristine_bone_index_by_name(&self, bone_name: &str) -> i32 {
        self.find_pristine_bone_by_name(bone_name)
            .map(|(_, info)| info.bone_index)
            .unwrap_or(0)
    }

    fn validate_turret_info(&mut self) {
        if (self.valid_stuff & MODEL_CONDITION_TURRETS_VALID) != 0 {
            return;
        }
        if self.pristine_bones.is_empty() {
            return;
        }

        let angle_keys: Vec<NameKeyType> = self
            .turrets
            .iter()
            .map(|turret| turret.turret_angle_name_key)
            .collect();
        let pitch_keys: Vec<NameKeyType> = self
            .turrets
            .iter()
            .map(|turret| turret.turret_pitch_name_key)
            .collect();

        let angle_bones: Vec<i32> = angle_keys
            .iter()
            .map(|key| {
                self.find_pristine_bone(*key)
                    .map(|bone| bone.bone_index)
                    .unwrap_or(0)
            })
            .collect();
        let pitch_bones: Vec<i32> = pitch_keys
            .iter()
            .map(|key| {
                self.find_pristine_bone(*key)
                    .map(|bone| bone.bone_index)
                    .unwrap_or(0)
            })
            .collect();

        for (index, turret) in self.turrets.iter_mut().enumerate() {
            turret.turret_angle_bone = angle_bones.get(index).copied().unwrap_or(0);
            turret.turret_pitch_bone = pitch_bones.get(index).copied().unwrap_or(0);
        }
        self.valid_stuff |= MODEL_CONDITION_TURRETS_VALID;
    }

    fn validate_weapon_barrel_info(&mut self) {
        if (self.valid_stuff & MODEL_CONDITION_BARRELS_VALID) != 0 {
            return;
        }
        let has_requested_weapon_bones = (0..WEAPONSLOT_COUNT).any(|slot| {
            !self.weapon_fire_fx_bone[slot].is_empty()
                || !self.weapon_recoil_bone[slot].is_empty()
                || !self.weapon_muzzle_flash[slot].is_empty()
                || !self.weapon_projectile_launch_bone[slot].is_empty()
        });
        if has_requested_weapon_bones && self.pristine_bones.is_empty() {
            return;
        }

        let mut validated_barrels = vec![Vec::new(); WEAPONSLOT_COUNT];

        for (slot, barrels) in validated_barrels.iter_mut().enumerate() {
            let fx_bone_name = self.weapon_fire_fx_bone[slot].as_str();
            let recoil_bone_name = self.weapon_recoil_bone[slot].as_str();
            let muzzle_flash_name = self.weapon_muzzle_flash[slot].as_str();
            let projectile_launch_name = self.weapon_projectile_launch_bone[slot].as_str();

            if fx_bone_name.is_empty()
                && recoil_bone_name.is_empty()
                && muzzle_flash_name.is_empty()
                && projectile_launch_name.is_empty()
            {
                continue;
            }

            let mut prev_fx_bone = 0;
            for index in 1..=99 {
                let mut info = WeaponBarrelInfo::new();

                if !recoil_bone_name.is_empty() {
                    let name = format!("{recoil_bone_name}{index:02}");
                    info.recoil_bone = self.pristine_bone_index_by_name(&name);
                }

                if !muzzle_flash_name.is_empty() {
                    let name = format!("{muzzle_flash_name}{index:02}");
                    info.muzzle_flash_bone = self.pristine_bone_index_by_name(&name);
                }

                if !fx_bone_name.is_empty() {
                    let name = format!("{fx_bone_name}{index:02}");
                    info.fx_bone = self.pristine_bone_index_by_name(&name);
                    if info.fx_bone == 0 && info.muzzle_flash_bone != 0 {
                        info.fx_bone = prev_fx_bone;
                    }
                }

                let mut projectile_launch_bone = 0;
                if !projectile_launch_name.is_empty() {
                    let name = format!("{projectile_launch_name}{index:02}");
                    if let Some((_, bone)) = self.find_pristine_bone_by_name(&name) {
                        projectile_launch_bone = bone.bone_index;
                        info.projectile_offset_mtx = bone.transform;
                    }
                }

                if info.fx_bone == 0
                    && info.recoil_bone == 0
                    && info.muzzle_flash_bone == 0
                    && projectile_launch_bone == 0
                {
                    break;
                }

                prev_fx_bone = info.fx_bone;
                barrels.push(info);
            }

            if barrels.is_empty() {
                let mut info = WeaponBarrelInfo::new();

                if !recoil_bone_name.is_empty() {
                    info.recoil_bone = self.pristine_bone_index_by_name(recoil_bone_name);
                }

                if !muzzle_flash_name.is_empty() {
                    info.muzzle_flash_bone = self.pristine_bone_index_by_name(muzzle_flash_name);
                }

                if !projectile_launch_name.is_empty() {
                    if let Some((_, bone)) = self.find_pristine_bone_by_name(projectile_launch_name)
                    {
                        info.projectile_offset_mtx = bone.transform;
                    }
                }

                if !fx_bone_name.is_empty() {
                    info.fx_bone = self.pristine_bone_index_by_name(fx_bone_name);
                }

                if info.fx_bone != 0
                    || info.recoil_bone != 0
                    || info.muzzle_flash_bone != 0
                    || info.projectile_offset_mtx != Matrix3D::IDENTITY
                {
                    barrels.push(info);
                }
            }
        }

        self.weapon_barrels = validated_barrels;
        self.valid_stuff |= MODEL_CONDITION_BARRELS_VALID;
    }

    fn validate_public_bones(&mut self, extra_public_bones: &[AsciiString]) {
        if (self.valid_stuff & MODEL_CONDITION_PUBLIC_BONES_VALID) != 0 {
            return;
        }
        for bone in extra_public_bones {
            add_public_bone(&mut self.public_bones, bone.as_str());
        }
        self.valid_stuff |= MODEL_CONDITION_PUBLIC_BONES_VALID;
    }

    fn validate_runtime_caches(&mut self, extra_public_bones: &[AsciiString]) {
        self.validate_public_bones(extra_public_bones);
        if !self.pristine_bones.is_empty() {
            self.valid_stuff |= MODEL_CONDITION_PRISTINE_BONES_VALID;
        }
        self.validate_turret_info();
        self.validate_weapon_barrel_info();
    }

    fn refresh_projectile_valid_bit(&mut self) {
        self.valid_stuff &= !MODEL_CONDITION_HAS_PROJECTILE_BONES;
        if self
            .weapon_projectile_launch_bone
            .iter()
            .any(|name| !name.is_empty())
        {
            self.valid_stuff |= MODEL_CONDITION_HAS_PROJECTILE_BONES;
        }
    }
}

impl Default for ModelConditionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// W3DModelDraw module data
///
/// Reference: W3DModelDrawModuleData in W3DModelDraw.h
#[derive(Debug, Clone)]
pub struct W3DModelDrawModuleData {
    /// Module tag name key
    pub module_tag_name_key: NameKeyType,

    /// Model condition states
    pub condition_states: Vec<ModelConditionInfo>,

    /// Transition states (`TransitionState` in INI), keyed at runtime by from/to pair.
    pub transition_states: Vec<ModelConditionInfo>,

    /// Track file for leaving marks on terrain
    pub track_file: AsciiString,

    /// Bone to attach this drawable to (on parent)
    pub attach_to_drawable_bone: AsciiString,

    /// Cached attach bone offset
    pub attach_to_drawable_bone_offset: Coord3D,

    /// Default state index
    pub default_state: i32,

    /// Which weapon slots have projectile bone feedback enabled
    pub projectile_bone_feedback_enabled_slots: u32,

    /// Initial recoil amount
    pub initial_recoil: Real,

    /// Maximum recoil distance
    pub max_recoil: Real,

    /// Recoil damping factor
    pub recoil_damping: Real,

    /// Recoil settle speed
    pub recoil_settle: Real,

    /// Minimum LOD level required
    pub min_lod_required: i32,

    /// Model conditions to ignore
    pub ignore_condition_states: ModelConditionFlags,

    /// Whether model color can be changed
    pub ok_to_change_model_color: bool,

    /// Whether animations require power
    pub animations_require_power: bool,

    /// Whether particles are attached to animated bones
    pub particles_attached_to_animated_bones: bool,

    /// Whether object receives dynamic lights
    pub receives_dynamic_lights: bool,

    /// Extra public bones
    pub extra_public_bones: Vec<AsciiString>,
}

impl W3DModelDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            condition_states: Vec::new(),
            transition_states: Vec::new(),
            track_file: AsciiString::new(),
            attach_to_drawable_bone: AsciiString::new(),
            attach_to_drawable_bone_offset: Coord3D::origin(),
            default_state: -1,
            projectile_bone_feedback_enabled_slots: 0,
            initial_recoil: 2.0,
            max_recoil: 3.0,
            recoil_damping: 0.4,
            recoil_settle: 0.065,
            min_lod_required: 0,
            ignore_condition_states: ModelConditionFlags::empty(),
            ok_to_change_model_color: false,
            animations_require_power: true,
            particles_attached_to_animated_bones: false,
            receives_dynamic_lights: true,
            extra_public_bones: Vec::new(),
        }
    }

    /// Parse module data from an INI block.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        parse_model_draw_module_data_block(ini, self)
    }

    /// Parse a single key/value field in this module block.
    pub(crate) fn parse_ini_field(
        &mut self,
        ini: &mut INI,
        key: &str,
        tokens: &[&str],
    ) -> Result<bool, INIError> {
        match key.to_ascii_uppercase().as_str() {
            "INITIALRECOILSPEED" => {
                self.initial_recoil = INI::parse_velocity_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "MAXRECOILDISTANCE" => {
                self.max_recoil = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "RECOILDAMPING" => {
                self.recoil_damping = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "RECOILSETTLESPEED" => {
                self.recoil_settle = INI::parse_velocity_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "OKTOCHANGEMODELCOLOR" => {
                self.ok_to_change_model_color = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "ANIMATIONSREQUIREPOWER" => {
                self.animations_require_power = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "PARTICLESATTACHEDTOANIMATEDBONES" => {
                self.particles_attached_to_animated_bones =
                    INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "MINLODREQUIRED" => {
                self.min_lod_required = parse_static_game_lod_level(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "PROJECTILEBONEFEEDBACKENABLEDSLOTS" => {
                self.projectile_bone_feedback_enabled_slots = parse_weapon_slot_mask(tokens);
                Ok(true)
            }
            "DEFAULTCONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Default)?;
                Ok(true)
            }
            "CONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Normal)?;
                Ok(true)
            }
            "ALIASCONDITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Alias)?;
                Ok(true)
            }
            "TRANSITIONSTATE" => {
                self.parse_condition_state(ini, tokens, ParseCondStateType::Transition)?;
                Ok(true)
            }
            "TRACKMARKS" => {
                let track = parse_ascii_lower(parse_required_value(tokens)?)?;
                self.track_file = AsciiString::from(track.as_str());
                Ok(true)
            }
            "EXTRAPUBLICBONE" => {
                for token in tokens {
                    let value = INI::parse_ascii_string(token)?;
                    if value.is_empty() {
                        continue;
                    }
                    self.extra_public_bones
                        .push(AsciiString::from(value.as_str()));
                }
                Ok(true)
            }
            "ATTACHTOBONEINANOTHERMODULE" => {
                let bone = parse_ascii_lower(parse_required_value(tokens)?)?;
                self.attach_to_drawable_bone = AsciiString::from(bone.as_str());
                Ok(true)
            }
            "IGNORECONDITIONSTATES" => {
                self.ignore_condition_states = parse_model_condition_flags_tokens(tokens);
                Ok(true)
            }
            "RECEIVESDYNAMICLIGHTS" => {
                self.receives_dynamic_lights = INI::parse_bool(parse_required_value(tokens)?)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn parse_condition_state(
        &mut self,
        ini: &mut INI,
        tokens: &[&str],
        state_type: ParseCondStateType,
    ) -> Result<(), INIError> {
        match state_type {
            ParseCondStateType::Alias => {
                if self.condition_states.is_empty() {
                    return Err(INIError::InvalidData);
                }
                let conditions_yes = parse_model_condition_flags_tokens(tokens);
                if conditions_yes.intersects(self.ignore_condition_states) {
                    return Err(INIError::InvalidData);
                }
                if does_state_exist(&self.condition_states, conditions_yes) {
                    return Err(INIError::InvalidData);
                }
                if conditions_yes.is_empty() && self.default_state >= 0 {
                    return Err(INIError::InvalidData);
                }
                if let Some(last) = self.condition_states.last_mut() {
                    last.conditions_yes.push(conditions_yes);
                    return Ok(());
                }
                Err(INIError::InvalidData)
            }
            _ => {
                let mut info = ModelConditionInfo::new();
                match state_type {
                    ParseCondStateType::Default => {
                        if self.default_state >= 0
                            || !tokens.is_empty()
                            || !self.condition_states.is_empty()
                        {
                            return Err(INIError::InvalidData);
                        }
                        self.default_state = self.condition_states.len() as i32;
                        info.conditions_yes.push(ModelConditionFlags::empty());
                    }
                    ParseCondStateType::Transition => {
                        let from_name = parse_ascii_lower(parse_required_value(tokens)?)?;
                        let to_name = parse_ascii_lower(
                            tokens
                                .iter()
                                .copied()
                                .skip(1)
                                .find(|token| !token.is_empty())
                                .ok_or(INIError::InvalidData)?,
                        )?;
                        if from_name == to_name {
                            return Err(INIError::InvalidData);
                        }
                        if self.default_state >= 0 {
                            let idx = self.default_state as usize;
                            if let Some(default_state) = self.condition_states.get(idx) {
                                info = default_state.clone();
                                info.ini_read_flags |= INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT;
                                info.transition_key = NAMEKEY_INVALID;
                                info.allow_to_finish_key = NAMEKEY_INVALID;
                            }
                        }
                        info.transition_from_key = if from_name.is_empty() || from_name == "none" {
                            NAMEKEY_INVALID
                        } else {
                            name_key_generate(&from_name)
                        };
                        info.transition_to_key = if to_name.is_empty() || to_name == "none" {
                            NAMEKEY_INVALID
                        } else {
                            name_key_generate(&to_name)
                        };
                    }
                    ParseCondStateType::Normal => {
                        if self.default_state >= 0 {
                            let idx = self.default_state as usize;
                            if let Some(default_state) = self.condition_states.get(idx) {
                                info = default_state.clone();
                                info.ini_read_flags |= INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT;
                                info.conditions_yes.clear();
                            }
                        }
                        let conditions_yes = parse_model_condition_flags_tokens(tokens);
                        if conditions_yes.intersects(self.ignore_condition_states) {
                            return Err(INIError::InvalidData);
                        }
                        if self.default_state < 0
                            && self.condition_states.is_empty()
                            && !conditions_yes.is_empty()
                        {
                            return Err(INIError::InvalidData);
                        }
                        if conditions_yes.is_empty() && self.default_state >= 0 {
                            return Err(INIError::InvalidData);
                        }
                        if does_state_exist(&self.condition_states, conditions_yes) {
                            return Err(INIError::InvalidData);
                        }
                        info.conditions_yes.push(conditions_yes);
                    }
                    ParseCondStateType::Alias => unreachable!(),
                }

                parse_model_condition_info_block(ini, &mut info)?;

                if info.model_name.is_empty() {
                    return Err(INIError::InvalidData);
                }
                if info.model_name.is_none() {
                    info.model_name.clear();
                }
                if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0
                    && (info.ini_read_flags & INI_READ_FLAG_GOT_NONIDLE_ANIMS) != 0
                {
                    return Err(INIError::InvalidData);
                }
                if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0
                    && info.anim_mode != AnimMode::Once
                    && info.anim_mode != AnimMode::OnceBackwards
                {
                    return Err(INIError::InvalidData);
                }
                info.refresh_projectile_valid_bit();

                if state_type == ParseCondStateType::Transition {
                    if (info.ini_read_flags & INI_READ_FLAG_GOT_IDLE_ANIMS) != 0 {
                        return Err(INIError::InvalidData);
                    }
                    if info.anim_mode != AnimMode::Once && info.anim_mode != AnimMode::OnceBackwards
                    {
                        return Err(INIError::InvalidData);
                    }
                    if info.transition_key != NAMEKEY_INVALID
                        || info.allow_to_finish_key != NAMEKEY_INVALID
                    {
                        return Err(INIError::InvalidData);
                    }
                    self.transition_states.push(info);
                } else {
                    self.condition_states.push(info);
                }

                Ok(())
            }
        }
    }

    /// Find best model condition info matching given conditions
    ///
    /// Implements the sparse matching algorithm from C++ SparseMatchFinder.h
    /// Reference: /GeneralsMD/Code/GameEngine/Include/Common/SparseMatchFinder.h:99-162
    ///
    /// The algorithm finds the ModelConditionInfo that best matches the given conditions by:
    /// 1. Maximizing the number of matching "yes" bits
    /// 2. Minimizing extraneous "yes" bits (bits set in the state but not in the query)
    pub fn find_best_info(&self, conditions: &ModelConditionFlags) -> Option<&ModelConditionInfo> {
        let filtered_conditions = *conditions & !self.ignore_condition_states;
        let mut best_match: Option<&ModelConditionInfo> = None;
        let mut best_yes_match = 0;
        let mut best_yes_extraneous_bits = i32::MAX;

        // Iterate through all condition states
        for state in &self.condition_states {
            // Each state can have multiple condition flag combinations (conditions_yes)
            for yes_flags in &state.conditions_yes {
                // Count how many bits match between query and state
                let yes_match = (filtered_conditions.bits() & yes_flags.bits()).count_ones() as i32;

                // Count extraneous bits: bits set in state but not in query
                let yes_extraneous_bits =
                    (yes_flags.bits() & !filtered_conditions.bits()).count_ones() as i32;

                // Select best match:
                // - Prefer more matching bits
                // - If tied, prefer fewer extraneous bits
                // Reference: W3DModelDraw.cpp:133-143
                if yes_match > best_yes_match
                    || (yes_match == best_yes_match
                        && yes_extraneous_bits < best_yes_extraneous_bits)
                {
                    best_match = Some(state);
                    best_yes_match = yes_match;
                    best_yes_extraneous_bits = yes_extraneous_bits;
                }
            }
        }

        // If no match found, return default state or first state
        best_match.or_else(|| {
            if self.default_state >= 0
                && (self.default_state as usize) < self.condition_states.len()
            {
                self.condition_states.get(self.default_state as usize)
            } else {
                self.condition_states.first()
            }
        })
    }
}

impl Default for W3DModelDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DModelDrawModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl DrawModuleData for W3DModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DModelDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut clone = self.clone();
        clone.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DModelDrawModuleData::xfer (version 1) persists validated
        // runtime caches (pristine bones/turret bones/barrel launch matrices).
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        for state in &mut self.condition_states {
            let mut valid_stuff = model_condition_valid_stuff(state) as i8;
            xfer.xfer_byte(&mut valid_stuff)
                .map_err(|e| e.to_string())?;
            if xfer.is_reading() {
                state.valid_stuff = valid_stuff as u8;
            }

            if valid_stuff == 0 {
                continue;
            }

            let mut pristine_keys: Vec<NameKeyType> =
                state.pristine_bones.keys().copied().collect();
            pristine_keys.sort_unstable();
            for key in pristine_keys {
                if let Some(bone) = state.pristine_bones.get_mut(&key) {
                    xfer.xfer_int(&mut bone.bone_index)
                        .map_err(|e| e.to_string())?;
                    xfer_matrix3d_values(xfer, &mut bone.transform)?;
                }
            }

            for turret_index in 0..MAX_TURRETS {
                let mut turret_angle_bone = state
                    .turrets
                    .get(turret_index)
                    .map(|turret| turret.turret_angle_bone)
                    .unwrap_or(0);
                let mut turret_pitch_bone = state
                    .turrets
                    .get(turret_index)
                    .map(|turret| turret.turret_pitch_bone)
                    .unwrap_or(0);
                xfer.xfer_int(&mut turret_angle_bone)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut turret_pitch_bone)
                    .map_err(|e| e.to_string())?;
                if xfer.is_reading() {
                    if state.turrets.len() <= turret_index {
                        state.turrets.resize_with(turret_index + 1, TurretInfo::new);
                    }
                    if let Some(turret) = state.turrets.get_mut(turret_index) {
                        turret.turret_angle_bone = turret_angle_bone;
                        turret.turret_pitch_bone = turret_pitch_bone;
                    }
                }
            }

            for barrels in &mut state.weapon_barrels {
                for barrel in barrels {
                    xfer_matrix3d_values(xfer, &mut barrel.projectile_offset_mtx)?;
                }
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Weapon recoil state
#[derive(Debug, Clone, Copy)]
enum RecoilState {
    Idle,
    RecoilStart,
    Recoil,
    Settle,
}

fn recoil_state_to_i32(state: RecoilState) -> i32 {
    match state {
        RecoilState::Idle => 0,
        RecoilState::RecoilStart => 1,
        RecoilState::Recoil => 2,
        RecoilState::Settle => 3,
    }
}

fn recoil_state_from_i32(value: i32) -> RecoilState {
    match value {
        1 => RecoilState::RecoilStart,
        2 => RecoilState::Recoil,
        3 => RecoilState::Settle,
        _ => RecoilState::Idle,
    }
}

fn xfer_matrix3d_values(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) -> Result<(), String> {
    let cols = matrix.to_cols_array();
    let mut row0 = [cols[0], cols[4], cols[8], cols[12]];
    let mut row1 = [cols[1], cols[5], cols[9], cols[13]];
    let mut row2 = [cols[2], cols[6], cols[10], cols[14]];

    for value in row0
        .iter_mut()
        .chain(row1.iter_mut())
        .chain(row2.iter_mut())
    {
        xfer.xfer_real(value).map_err(|e| e.to_string())?;
    }

    let rebuilt_cols = [
        row0[0], row1[0], row2[0], 0.0, row0[1], row1[1], row2[1], 0.0, row0[2], row1[2], row2[2],
        0.0, row0[3], row1[3], row2[3], 1.0,
    ];
    *matrix = Matrix3D::from_cols_array(&rebuilt_cols);
    Ok(())
}

/// Weapon recoil information
#[derive(Debug, Clone)]
struct WeaponRecoilInfo {
    /// Current recoil state
    state: RecoilState,

    /// Current shift amount
    shift: Real,

    /// Recoil rate
    recoil_rate: Real,
}

impl WeaponRecoilInfo {
    fn new() -> Self {
        Self {
            state: RecoilState::Idle,
            shift: 0.0,
            recoil_rate: 0.0,
        }
    }
}

/// Animation override settings
///
/// Used to override animation behavior (duration, frame, etc.)
#[derive(Debug, Clone)]
struct AnimationOverride {
    /// Override for animation loop duration (in frames)
    duration_frames: Option<u32>,

    /// Override for animation completion time (in frames, for ONCE animations)
    completion_frames: Option<u32>,

    /// Manual frame override
    manual_frame: Option<i32>,
}

impl AnimationOverride {
    fn new() -> Self {
        Self {
            duration_frames: None,
            completion_frames: None,
            manual_frame: None,
        }
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.duration_frames = None;
        self.completion_frames = None;
        self.manual_frame = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveModelState {
    Condition(usize),
    Transition(usize),
}

/// W3DModelDraw module instance
///
/// Reference: W3DModelDraw in W3DModelDraw.h
#[allow(dead_code)]
pub struct W3DModelDraw {
    /// Module data
    data: W3DModelDrawModuleData,

    /// Current model condition state
    cur_state: Option<ActiveModelState>,

    /// Next state to transition to
    next_state: Option<usize>,

    /// Animation loop duration for next state
    next_state_anim_loop_duration: u32,

    /// Current hex color
    #[allow(dead_code)]
    hex_color: i32,

    /// Index of currently playing animation in current state
    which_anim_in_cur_state: i32,

    /// Weapon recoil info per slot
    weapon_recoil_info: Vec<Vec<WeaponRecoilInfo>>,

    /// Whether bone particle systems need recalculation
    need_recalc_bone_particle_systems: bool,

    /// Whether fully obscured by shroud
    fully_obscured_by_shroud: bool,
    /// Explicit hidden state propagated by Drawable::update_hidden_status.
    hidden: bool,

    /// Whether shadows are enabled
    shadow_enabled: bool,

    /// Whether headlights are hidden
    hide_headlights: bool,

    /// Whether animation is paused
    pause_animation: bool,

    /// Current animation mode
    animation_mode: i32,

    /// Current animation frame index tracked by the logic-side draw runtime.
    current_anim_frame: i32,

    /// Current animation frame count used for completion checks.
    current_anim_num_frames: i32,

    /// Current animation completion state.
    current_anim_complete: bool,

    /// Cached animation speed factor selected at animation start.
    current_anim_speed_factor: Real,

    /// Sub-objects to hide/show
    sub_object_vec: Vec<HideShowSubObjInfo>,

    /// Whether sub-object visibility needs to be pushed to renderer.
    sub_objects_dirty: bool,

    /// Current terrain decal type for this draw module.
    terrain_decal: TerrainDecalType,
    /// Optional terrain decal size override (width, height).
    terrain_decal_size: Option<(Real, Real)>,
    /// Optional terrain decal opacity override.
    terrain_decal_opacity: Option<Real>,

    /// Particle system IDs currently active
    particle_system_ids: Vec<u32>,

    /// Animation override state
    animation_override: AnimationOverride,

    /// Last model conditions (for detecting state changes)
    last_model_conditions: ModelConditionFlags,

    /// Owning object ID (used for turret aiming).
    owner_id: Option<ObjectID>,
}

impl W3DModelDraw {
    pub fn new(data: W3DModelDrawModuleData) -> Self {
        let weapon_recoil_info = vec![Vec::new(); WEAPONSLOT_COUNT];

        Self {
            data,
            cur_state: None,
            next_state: None,
            next_state_anim_loop_duration: NO_NEXT_DURATION,
            hex_color: 0,
            which_anim_in_cur_state: -1,
            weapon_recoil_info,
            need_recalc_bone_particle_systems: false,
            fully_obscured_by_shroud: false,
            hidden: false,
            shadow_enabled: true,
            hide_headlights: false,
            pause_animation: false,
            animation_mode: 0,
            current_anim_frame: 0,
            current_anim_num_frames: DEFAULT_ANIMATION_FRAMES,
            current_anim_complete: true,
            current_anim_speed_factor: 1.0,
            sub_object_vec: Vec::new(),
            sub_objects_dirty: false,
            terrain_decal: TerrainDecalType::None,
            terrain_decal_size: None,
            terrain_decal_opacity: None,
            particle_system_ids: Vec::new(),
            animation_override: AnimationOverride::new(),
            last_model_conditions: ModelConditionFlags::empty(),
            owner_id: None,
        }
    }

    fn rebuild_weapon_recoil_info(&mut self, state_ref: Option<ActiveModelState>) {
        let mut target_counts = [0usize; WEAPONSLOT_COUNT];
        if let Some(state_ref) = state_ref {
            if let Some(state) = self.resolve_state(state_ref) {
                for (slot, count) in target_counts.iter_mut().enumerate() {
                    *count = state.weapon_barrels[slot].len();
                }
            }
        }

        for (slot, target_count) in target_counts.iter().copied().enumerate() {
            if let Some(recoils) = self.weapon_recoil_info.get_mut(slot) {
                recoils.resize_with(target_count, WeaponRecoilInfo::new);
                for recoil in recoils.iter_mut() {
                    recoil.state = RecoilState::Idle;
                    recoil.shift = 0.0;
                    recoil.recoil_rate = 0.0;
                }
            }
        }
    }

    pub fn has_any_turrets(&self) -> bool {
        self.data
            .condition_states
            .iter()
            .any(|state| !state.turrets.is_empty())
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.owner_id
    }

    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.fully_obscured_by_shroud
    }

    fn with_owner_drawable<R>(
        &self,
        func: impl FnOnce(&crate::object::drawable::Drawable) -> R,
    ) -> Option<R> {
        let owner_id = self.owner_id?;
        let object = TheGameLogic::find_object_by_id(owner_id)?;
        let drawable = {
            let obj_guard = object.read().ok()?;
            obj_guard.get_drawable()?
        };
        let drawable_guard = drawable.read().ok()?;
        Some(func(&drawable_guard))
    }

    fn resolve_state(&self, state_ref: ActiveModelState) -> Option<&ModelConditionInfo> {
        match state_ref {
            ActiveModelState::Condition(index) => self.data.condition_states.get(index),
            ActiveModelState::Transition(index) => self.data.transition_states.get(index),
        }
    }

    fn resolve_state_mut(
        &mut self,
        state_ref: ActiveModelState,
    ) -> Option<&mut ModelConditionInfo> {
        match state_ref {
            ActiveModelState::Condition(index) => self.data.condition_states.get_mut(index),
            ActiveModelState::Transition(index) => self.data.transition_states.get_mut(index),
        }
    }

    fn current_state(&self) -> Option<&ModelConditionInfo> {
        self.cur_state
            .and_then(|state_ref| self.resolve_state(state_ref))
    }

    fn is_current_transition_state(&self) -> bool {
        matches!(self.cur_state, Some(ActiveModelState::Transition(_)))
    }

    fn find_best_state_index(&self, conditions: &ModelConditionFlags) -> Option<usize> {
        let best_info = self.data.find_best_info(conditions)?;
        self.data
            .condition_states
            .iter()
            .position(|state| std::ptr::eq(state, best_info))
    }

    fn find_transition_state_index(
        &self,
        from_key: NameKeyType,
        to_key: NameKeyType,
    ) -> Option<usize> {
        self.data.transition_states.iter().position(|state| {
            state.transition_from_key == from_key && state.transition_to_key == to_key
        })
    }

    fn get_current_anim_fraction(&self) -> Real {
        let Some(state) = self.current_state() else {
            return -1.0;
        };
        if !is_any_maintain_frame_flag_set(state.flags) {
            return -1.0;
        }
        if self.current_anim_num_frames <= 1 {
            return 0.0;
        }
        let denom = (self.current_anim_num_frames - 1) as Real;
        if denom <= 0.0 {
            return 0.0;
        }
        let frame = self
            .current_anim_frame
            .clamp(0, self.current_anim_num_frames - 1) as Real;
        (frame / denom).clamp(0.0, 1.0)
    }

    fn current_animation_complete(&self) -> bool {
        self.current_anim_complete
    }

    fn animation_total_frames(&self, state: &ModelConditionInfo) -> i32 {
        if let Some(frames) = self.animation_override.duration_frames {
            return frames.max(1) as i32;
        }
        if self.which_anim_in_cur_state >= 0
            && (self.which_anim_in_cur_state as usize) < state.animations.len()
        {
            let anim = &state.animations[self.which_anim_in_cur_state as usize];
            if anim.natural_duration_ms > 0.0 {
                let frames = (anim.natural_duration_ms / MSEC_PER_LOGICFRAME_REAL).round() as i32;
                return frames.max(1);
            }
        }
        DEFAULT_ANIMATION_FRAMES
    }

    fn ensure_animation_duration_loaded(&mut self, state_ref: ActiveModelState, anim_index: usize) {
        let Some(state) = self.resolve_state(state_ref) else {
            return;
        };
        let Some(anim) = state.animations.get(anim_index) else {
            return;
        };
        if anim.natural_duration_ms > 0.0 || anim.name.is_empty() {
            return;
        }

        let Some(client) = TheGameClient::get() else {
            return;
        };
        let Some(duration_ms) = client.get_animation_duration_ms(anim.name.as_str()) else {
            return;
        };
        if duration_ms <= 0.0 {
            return;
        }

        if let Some(state) = self.resolve_state_mut(state_ref) {
            if let Some(anim) = state.animations.get_mut(anim_index) {
                if anim.natural_duration_ms <= 0.0 {
                    anim.natural_duration_ms = duration_ms;
                }
            }
        }
    }

    pub fn update_bones_for_client_particle_systems(&mut self) -> bool {
        let Some(state) = self.current_state() else {
            return false;
        };
        let particle_sys_bones = state.particle_sys_bones.clone();
        if particle_sys_bones.is_empty() {
            return false;
        }

        let Some(owner_id) = self.owner_id else {
            return false;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return false;
        };
        let Ok(obj_guard) = object.read() else {
            return false;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return false;
        };
        let Ok(drawable_guard) = drawable.read() else {
            return false;
        };

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return false;
        };

        let mut created_or_recalculated = false;
        if self.need_recalc_bone_particle_systems
            || self.particle_system_ids.len() != particle_sys_bones.len()
        {
            for system_id in self.particle_system_ids.drain(..) {
                ps_manager.destroy_particle_system(system_id);
            }
            self.need_recalc_bone_particle_systems = false;
            created_or_recalculated = true;
        }

        for (idx, info) in particle_sys_bones.iter().enumerate() {
            if info.particle_system.is_empty() {
                continue;
            }

            let system_id = if idx < self.particle_system_ids.len() {
                self.particle_system_ids[idx]
            } else {
                let system_id =
                    match ps_manager.create_particle_system(Some(info.particle_system.as_str())) {
                        Some(id) => id,
                        None => continue,
                    };
                ps_manager.attach_particle_system_to_drawable(system_id, owner_id);
                self.particle_system_ids.push(system_id);
                created_or_recalculated = true;
                system_id
            };

            if !info.bone_name.is_empty() {
                if let Some(transform) = drawable_guard
                    .get_current_worldspace_client_bone_positions(info.bone_name.as_str())
                {
                    ps_manager.set_particle_system_transform(system_id, &transform);
                } else {
                    ps_manager
                        .set_particle_system_position(system_id, &drawable_guard.get_position());
                }
            } else {
                ps_manager.set_particle_system_position(system_id, &drawable_guard.get_position());
            }
        }

        if created_or_recalculated {
            self.do_start_or_stop_particle_sys();
        }

        true
    }

    fn do_start_or_stop_particle_sys(&self) {
        let hidden = self.hidden || self.fully_obscured_by_shroud;
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        for system_id in &self.particle_system_ids {
            if hidden {
                ps_manager.stop_particle_system(*system_id);
            } else {
                ps_manager.start_particle_system(*system_id);
            }
        }
    }

    fn adjust_anim_speed_to_movement_speed(&mut self) {
        let Some(state) = self.current_state() else {
            return;
        };
        if self.which_anim_in_cur_state < 0 {
            return;
        }
        let anim_index = self.which_anim_in_cur_state as usize;
        let Some(anim) = state.animations.get(anim_index) else {
            return;
        };
        let distance_covered = anim.distance_covered;
        if distance_covered <= 0.0 {
            return;
        }

        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        let Some(physics) = obj_guard.get_physics() else {
            return;
        };
        let Ok(physics_guard) = physics.lock() else {
            return;
        };
        let speed = physics_guard.get_velocity().length();
        if speed <= 0.0 {
            return;
        }

        // C++ parity: distance-covered animations scale loop duration to unit speed.
        let desired_duration_ms = distance_covered / speed * MSEC_PER_LOGICFRAME_REAL;
        self.set_cur_anim_duration_in_msec(desired_duration_ms);
    }

    /// Show or hide a named sub-object.
    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        let normalized_name = name.to_ascii_lowercase();
        if normalized_name.is_empty() {
            return;
        }
        let hide = !show;
        if let Some(entry) = self.sub_object_vec.iter_mut().find(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(&normalized_name)
        }) {
            entry.hide = hide;
        } else {
            self.sub_object_vec.push(HideShowSubObjInfo {
                sub_obj_name: AsciiString::from(normalized_name.as_str()),
                hide,
            });
        }
        self.sub_objects_dirty = true;
    }

    fn normalize_sub_object_entries(&mut self) {
        let mut normalized: Vec<HideShowSubObjInfo> = Vec::new();

        for entry in self.sub_object_vec.drain(..) {
            let key = entry.sub_obj_name.as_str().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }

            if let Some(existing) = normalized
                .iter_mut()
                .find(|existing| existing.sub_obj_name.as_str().eq_ignore_ascii_case(&key))
            {
                // Last writer wins, matching repeated show/hide call behavior.
                existing.hide = entry.hide;
            } else {
                normalized.push(HideShowSubObjInfo {
                    sub_obj_name: AsciiString::from(key.as_str()),
                    hide: entry.hide,
                });
            }
        }

        self.sub_object_vec = normalized;
    }

    /// Apply pending sub-object visibility updates.
    pub fn update_sub_objects(&mut self) {
        self.normalize_sub_object_entries();
        self.sub_objects_dirty = false;
    }

    /// Set current model state
    fn set_model_state(&mut self, state_index: usize) {
        if state_index >= self.data.condition_states.len() {
            return;
        }
        let extra_public_bones = self.data.extra_public_bones.clone();
        self.data.condition_states[state_index].validate_runtime_caches(&extra_public_bones);

        let mut new_state_ref = ActiveModelState::Condition(state_index);
        let mut pending_next_state: Option<usize> = None;

        if let Some(cur_state_ref) = self.cur_state {
            if (cur_state_ref == new_state_ref && self.next_state.is_none())
                || self.next_state == Some(state_index)
            {
                return;
            }

            let cur_transition_key = self
                .resolve_state(cur_state_ref)
                .map(|state| state.transition_key)
                .unwrap_or(NAMEKEY_INVALID);
            let requested_state = &self.data.condition_states[state_index];

            if new_state_ref != cur_state_ref
                && requested_state.allow_to_finish_key != NAMEKEY_INVALID
                && requested_state.allow_to_finish_key == cur_transition_key
                && !self.current_animation_complete()
            {
                self.next_state = Some(state_index);
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                return;
            }

            if new_state_ref != cur_state_ref
                && cur_transition_key != NAMEKEY_INVALID
                && requested_state.transition_key != NAMEKEY_INVALID
            {
                if let Some(transition_index) = self
                    .find_transition_state_index(cur_transition_key, requested_state.transition_key)
                {
                    new_state_ref = ActiveModelState::Transition(transition_index);
                    pending_next_state = Some(state_index);
                }
            }
        }

        if let Some(state) = self.resolve_state_mut(new_state_ref) {
            state.validate_runtime_caches(&extra_public_bones);
        }

        let prev_state = self.cur_state;
        let prev_anim_fraction = self.get_current_anim_fraction();

        self.need_recalc_bone_particle_systems = true;
        self.sub_object_vec = self
            .resolve_state(new_state_ref)
            .map(|state| state.hide_show_list.clone())
            .unwrap_or_default();
        self.sub_objects_dirty = true;
        self.rebuild_weapon_recoil_info(Some(new_state_ref));

        self.cur_state = Some(new_state_ref);
        self.next_state = pending_next_state;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        self.adjust_animation(prev_state, prev_anim_fraction);
    }

    fn adjust_animation(&mut self, prev_state: Option<ActiveModelState>, prev_anim_fraction: Real) {
        let Some(cur_state_ref) = self.cur_state else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };
        let Some(cur_state) = self.resolve_state(cur_state_ref) else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };

        let num_anims = cur_state.animations.len();
        if num_anims == 0 {
            self.which_anim_in_cur_state = -1;
            self.current_anim_frame = 0;
            self.current_anim_num_frames = DEFAULT_ANIMATION_FRAMES;
            self.current_anim_complete = true;
            return;
        }

        if num_anims == 1 {
            self.which_anim_in_cur_state = 0;
        } else if prev_state == Some(cur_state_ref) {
            let anim_to_avoid = self.which_anim_in_cur_state;
            while self.which_anim_in_cur_state == anim_to_avoid {
                self.which_anim_in_cur_state = game_client_random_value(0, num_anims as i32 - 1);
            }
        } else {
            self.which_anim_in_cur_state = game_client_random_value(0, num_anims as i32 - 1);
        }

        if self.which_anim_in_cur_state >= 0 {
            self.ensure_animation_duration_loaded(
                cur_state_ref,
                self.which_anim_in_cur_state as usize,
            );
        }

        let Some(cur_state) = self.resolve_state(cur_state_ref).cloned() else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };

        let total_frames = self.animation_total_frames(&cur_state).max(1);
        let mut start_frame = if cur_state.anim_mode == AnimMode::OnceBackwards
            || cur_state.anim_mode == AnimMode::LoopBackwards
        {
            total_frames - 1
        } else {
            0
        };

        if test_flag_bit(cur_state.flags, ACBIT_RANDOMSTART) {
            start_frame = game_client_random_value(0, total_frames - 1);
        } else if test_flag_bit(cur_state.flags, ACBIT_START_FRAME_FIRST) {
            start_frame = 0;
        } else if test_flag_bit(cur_state.flags, ACBIT_START_FRAME_LAST) {
            start_frame = total_frames - 1;
        } else if is_any_maintain_frame_flag_set(cur_state.flags)
            && prev_state.is_some()
            && prev_state != Some(cur_state_ref)
            && prev_state
                .and_then(|state_ref| self.resolve_state(state_ref))
                .map(|state| {
                    is_any_maintain_frame_flag_set(state.flags)
                        && is_common_maintain_frame_flag_set(cur_state.flags, state.flags)
                })
                .unwrap_or(false)
            && prev_anim_fraction >= 0.0
        {
            let target = prev_anim_fraction * (total_frames - 1) as Real;
            start_frame = target.round() as i32;
        }

        self.current_anim_num_frames = total_frames.max(1);
        self.current_anim_frame = start_frame.clamp(0, self.current_anim_num_frames - 1);
        self.current_anim_speed_factor =
            if cur_state.anim_min_speed_factor <= cur_state.anim_max_speed_factor {
                game_client_random_value_real(
                    cur_state.anim_min_speed_factor,
                    cur_state.anim_max_speed_factor,
                )
            } else {
                1.0
            };
        self.current_anim_complete = false;
    }

    fn tick_animation_state(&mut self) {
        if self.pause_animation {
            return;
        }
        let Some(cur_state) = self.current_state().cloned() else {
            self.current_anim_complete = true;
            return;
        };
        if self.which_anim_in_cur_state < 0 || cur_state.animations.is_empty() {
            self.current_anim_complete = true;
            return;
        }

        self.current_anim_num_frames = self.animation_total_frames(&cur_state).max(1);
        let last_frame = self.current_anim_num_frames.saturating_sub(1);
        match cur_state.anim_mode {
            AnimMode::Loop | AnimMode::LoopPingPong => {
                self.current_anim_frame =
                    (self.current_anim_frame + 1).rem_euclid(self.current_anim_num_frames);
                self.current_anim_complete = false;
            }
            AnimMode::LoopBackwards => {
                self.current_anim_frame -= 1;
                if self.current_anim_frame < 0 {
                    self.current_anim_frame = last_frame;
                }
                self.current_anim_complete = false;
            }
            AnimMode::Manual => {
                self.current_anim_complete = false;
            }
            AnimMode::Once => {
                if self.current_anim_frame < last_frame {
                    self.current_anim_frame += 1;
                    self.current_anim_complete = false;
                } else {
                    self.current_anim_complete = true;
                }
            }
            AnimMode::OnceBackwards => {
                if self.current_anim_frame > 0 {
                    self.current_anim_frame -= 1;
                    self.current_anim_complete = false;
                } else {
                    self.current_anim_complete = true;
                }
            }
        }

        if let Some(frame) = self.animation_override.manual_frame {
            self.current_anim_frame = frame.clamp(0, last_frame);
        }
    }

    /// Handle client-side turret positioning
    ///
    /// Updates turret bone rotations based on object's current turret angles.
    /// Reference: C++ W3DModelDraw.cpp:2391-2442
    fn handle_client_turret_positioning(&mut self) {
        let Some(state) = self.current_state() else {
            return;
        };

        // Process each turret slot (up to MAX_TURRETS)
        for (index, turret) in state.turrets.iter().enumerate() {
            if turret.turret_angle_bone == 0 && turret.turret_pitch_bone == 0 {
                continue;
            }

            let mut turret_angle = 0.0;
            let mut turret_pitch = 0.0;
            if let Some(owner_id) = self.owner_id {
                if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) {
                    if let Ok(obj_guard) = obj.read() {
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                let turret_type = if index == 0 {
                                    TurretType::Primary
                                } else {
                                    TurretType::Secondary
                                };
                                if let Some((angle, pitch)) =
                                    ai_guard.get_turret_rot_and_pitch(turret_type)
                                {
                                    turret_angle = angle;
                                    turret_pitch = pitch;
                                }
                            }
                        }
                    }
                }
            }

            // Apply turret angle bone rotation
            if turret.turret_angle_bone != 0 {
                // Add art-defined offset to turret angle
                turret_angle += turret.turret_art_angle;

                // Create rotation matrix around Z axis
                // Reference: W3DModelDraw.cpp:2416-2421
                let turret_transform = Matrix3D::from_rotation_z(turret_angle);

                // When render object system is implemented:
                // - Capture the bone to allow manual control
                // - Apply the rotation transform to the bone
                // Reference: C++ W3DModelDraw.cpp:2416-2421
                // m_renderObject->Capture_Bone(turret.turret_angle_bone);
                // m_renderObject->Control_Bone(turret.turret_angle_bone, turretXfrm);
                let _ = turret_transform;
            }

            // Apply turret pitch bone rotation
            if turret.turret_pitch_bone != 0 {
                // Add art-defined offset to turret pitch
                turret_pitch += turret.turret_art_pitch;

                // Create rotation matrix around Y axis
                // Reference: W3DModelDraw.cpp:2427-2432
                let pitch_transform = Matrix3D::from_rotation_y(turret_pitch);

                // When render object system is implemented:
                // Reference: C++ W3DModelDraw.cpp:2427-2432
                // m_renderObject->Capture_Bone(turret.turret_pitch_bone);
                // m_renderObject->Control_Bone(turret.turret_pitch_bone, pitchXfrm);
                let _ = pitch_transform;
            }
        }
    }

    /// Handle client-side weapon recoil
    fn handle_client_recoil(&mut self) {
        const TINY_RECOIL: Real = 0.01;
        let Some(state) = self.current_state().cloned() else {
            return;
        };

        for wslot in 0..WEAPONSLOT_COUNT {
            let barrels = &state.weapon_barrels[wslot];
            let Some(recoils) = self.weapon_recoil_info.get_mut(wslot) else {
                continue;
            };
            let count = barrels.len().min(recoils.len());
            for i in 0..count {
                if barrels[i].recoil_bone == 0 {
                    recoils[i].state = RecoilState::Idle;
                    continue;
                }

                match recoils[i].state {
                    RecoilState::Idle => {}
                    RecoilState::RecoilStart | RecoilState::Recoil => {
                        recoils[i].shift += recoils[i].recoil_rate;
                        recoils[i].recoil_rate *= self.data.recoil_damping;
                        if recoils[i].shift >= self.data.max_recoil {
                            recoils[i].shift = self.data.max_recoil;
                            recoils[i].state = RecoilState::Settle;
                        } else if recoils[i].recoil_rate.abs() < TINY_RECOIL {
                            recoils[i].state = RecoilState::Settle;
                        } else {
                            recoils[i].state = RecoilState::Recoil;
                        }
                    }
                    RecoilState::Settle => {
                        recoils[i].shift -= self.data.recoil_settle;
                        if recoils[i].shift <= 0.0 {
                            recoils[i].shift = 0.0;
                            recoils[i].state = RecoilState::Idle;
                        }
                    }
                }
            }
        }
    }

    /// Update model condition state based on current conditions
    ///
    /// Finds the best matching ModelConditionInfo and switches to it if different
    pub fn update_model_condition_state(&mut self, conditions: ModelConditionFlags) {
        // Skip if conditions haven't changed
        if conditions == self.last_model_conditions {
            return;
        }

        self.last_model_conditions = conditions;

        if let Some(state_index) = self.find_best_state_index(&conditions) {
            self.set_model_state(state_index);
        }
    }

    /// Set animation to loop in N frames
    ///
    /// This call says, "I want the current animation (if any) to take n frames to complete a single cycle".
    /// If it's a looping anim, each loop will take n frames.
    /// Note that you must call this AFTER setting the condition codes.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3748 - setAnimationLoopDuration
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.animation_override.duration_frames = Some(num_frames);
        self.animation_override.completion_frames = None;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        let desired_duration_ms = (num_frames as Real * MSEC_PER_LOGICFRAME_REAL).ceil();
        self.set_cur_anim_duration_in_msec(desired_duration_ms);
    }

    /// Set animation completion time
    ///
    /// Similar to setAnimationLoopDuration, but assumes that the current state is a "ONCE",
    /// and is smart about transition states... if there is a transition state "inbetween",
    /// it is included in the completion time.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3774 - setAnimationCompletionTime
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        self.animation_override.completion_frames = Some(num_frames);
        self.animation_override.duration_frames = None;

        if self.is_current_transition_state() {
            let Some(cur_state_ref) = self.cur_state else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            self.ensure_animation_duration_loaded(cur_state_ref, 0);

            let Some(next_state_index) = self.next_state else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            self.ensure_animation_duration_loaded(ActiveModelState::Condition(next_state_index), 0);

            let Some(cur_state) = self.current_state() else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            let Some(next_state) = self.data.condition_states.get(next_state_index) else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            if !cur_state.animations.is_empty() && !next_state.animations.is_empty() {
                let t1 = cur_state.animations[0].natural_duration_ms.max(1.0);
                let t2 = next_state.animations[0].natural_duration_ms.max(1.0);
                let numerator = num_frames as Real * t1;
                let trans_time = (numerator / (t1 + t2)).floor().max(1.0) as u32;
                self.set_animation_loop_duration(trans_time);
                self.next_state_anim_loop_duration = num_frames.saturating_sub(trans_time);
                return;
            }
        }

        self.set_animation_loop_duration(num_frames);
    }

    /// Set animation frame manually
    ///
    /// Manually set a drawable's current animation to a specific frame.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3797 - setAnimationFrame
    pub fn set_animation_frame(&mut self, frame: i32) {
        self.animation_override.manual_frame = Some(frame);
        if self.current_anim_num_frames > 0 {
            self.current_anim_frame = frame.clamp(0, self.current_anim_num_frames - 1);
        }
    }

    /// Set current animation duration in milliseconds
    ///
    /// Internal helper that applies duration override to the render object
    /// Reference: C++ W3DModelDraw.cpp:3716-3745
    fn set_cur_anim_duration_in_msec(&mut self, duration_ms: Real) {
        if duration_ms > 0.0 {
            let frames = (duration_ms / MSEC_PER_LOGICFRAME_REAL).round() as i32;
            self.current_anim_num_frames = frames.max(1);
            self.current_anim_frame = self
                .current_anim_frame
                .clamp(0, self.current_anim_num_frames - 1);
            self.current_anim_complete = false;
        }
    }

    /// Build and submit a `ModelDrawState` to the rendering bridge.
    ///
    /// This is the primary rendering submission path that connects the
    /// GameLogic draw module to the GameClient rendering pipeline. The
    /// GameClient device layer reads the `ModelDrawState` from the shared
    /// `DRAWABLE_STATE` map and converts it into a
    /// `render_bridge::DrawSubmission` for the WWVegas renderer.
    ///
    /// Reference: C++ W3DModelDraw::doDrawModule() lines 2016-2088
    ///
    /// ## C++ parity behaviors
    ///
    /// 1. **Condition-state model selection**: Selects the correct model
    ///    name based on the current condition flags (Default, Damaged,
    ///    ReallyDamaged, Rubble, Night, Snow, etc.). This mirrors the C++
    ///    behavior where `setModelState()` swaps the W3D render object.
    ///
    /// 2. **Bone overrides**: Collects turret rotation, turret pitch, and
    ///    weapon recoil bone transforms into a single list. In C++, these
    ///    are applied via `Capture_Bone`/`Control_Bone` on the render
    ///    object. Here we pass them as a `Vec<BoneOverrideState>`.
    ///
    /// 3. **Animation state**: Passes the current animation name, mode,
    ///    and time fraction. In C++, `Set_Animation()` is called on the
    ///    HLod render object.
    ///
    /// 4. **Mesh UV overrides**: For tread/track animations, the C++ code
    ///    adjusts UV offsets on specific mesh sub-objects. We pass these
    ///    as `MeshUvOverrideState` entries.
    ///
    /// 5. **Sub-object visibility**: In C++, `doHideShowSubObjs()` is
    ///    called to show/hide sub-objects. The bridge converts
    ///    `sub_object_vec` into render-state visibility directives.
    ///
    /// 6. **Instance scaling**: C++ applies `getDrawable()->getInstanceScale()`
    ///    to the world transform before rendering. We include the scaled
    ///    transform.
    fn submit_draw_to_bridge(&mut self, transform_mtx: &Matrix3D) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };

        // Phase 1: Resolve the model name from the current condition state.
        //
        // C++ parity: W3DModelDraw::setModelState() swaps m_renderObject to the
        // model defined by the matching ModelConditionInfo. The model name
        // comes from ModelConditionInfo::m_modelName.
        let model_name = self
            .current_state()
            .map(|s| s.model_name.to_string())
            .unwrap_or_default();

        // Phase 2: Resolve animation state.
        //
        // C++ parity: W3DModelDraw::adjustAnimation() calls
        // m_renderObject->Set_Animation(animHandle, startFrame, mode).
        // The consumer maps anim_name + anim_time + anim_mode to the
        // WWVegas AnimationController.
        let anim_name = self.current_state().and_then(|state| {
            let idx = self.which_anim_in_cur_state;
            if idx >= 0 && (idx as usize) < state.animations.len() {
                Some(state.animations[idx as usize].name.to_string())
            } else {
                None
            }
        });

        let anim_mode = self
            .current_state()
            .map(|s| match s.anim_mode {
                AnimMode::Manual => 0,
                AnimMode::Loop => 1,
                AnimMode::Once => 2,
                AnimMode::LoopPingPong => 3,
                AnimMode::LoopBackwards => 4,
                AnimMode::OnceBackwards => 5,
            })
            .unwrap_or(0);

        let anim_time = self.get_current_anim_fraction().clamp(0.0, 1.0);

        // Phase 3: Collect bone overrides (turret + recoil).
        //
        // C++ parity: handleClientTurretPositioning() and handleClientRecoil()
        // each call Capture_Bone/Control_Bone on the render object. We collect
        // all overrides into a single list.
        let bone_overrides = self.collect_bone_overrides();

        // Phase 4: Collect mesh UV overrides for tread animations.
        //
        // C++ parity: W3DTankDraw::doDrawModule() adjusts UV offsets on
        // TREADSL/TREADSR mesh sub-objects. Other draw modules (truck, etc.)
        // do similar UV scrolling on moving parts.
        let mesh_uv_overrides = self.collect_mesh_uv_overrides();
        let sub_object_visibility = self
            .sub_object_vec
            .iter()
            .map(|entry| SubObjectVisibilityState {
                sub_object_name: entry.sub_obj_name.to_string(),
                hidden: entry.hide,
            })
            .collect();

        // Phase 5: Apply instance scaling to the world transform.
        //
        // C++ parity: doDrawModule() applies getDrawable()->getInstanceScale()
        // before setting the render object transform.
        let world_transform = self.apply_instance_scale(transform_mtx);

        // Phase 6: Build the model draw state with all collected data.
        //
        // The consumer (GameClient device layer) maps condition_flags_bits to
        // render_bridge::RenderConditionFlags, which controls damage overlays,
        // night/snow maps, construction visibility, etc.
        let state = ModelDrawState {
            model_name,
            world_transform,
            condition_flags_bits: self.last_model_conditions.bits(),
            bone_overrides,
            animation_name: anim_name,
            animation_time: anim_time,
            animation_mode: anim_mode,
            mesh_uv_overrides,
            sub_object_visibility,
        };

        client.set_drawable_model_draw(owner_id, state);
    }

    /// Collect mesh UV overrides for tread/track animations.
    ///
    /// In C++, W3DTankDraw and similar subclasses adjust UV offsets on
    /// specific mesh sub-objects (e.g., "TREADSL", "TREADSR") based on
    /// the object's velocity and distance traveled. The base W3DModelDraw
    /// doesn't generate UV overrides itself, but the architecture allows
    /// subclass overrides to contribute UV scrolling via this method.
    ///
    /// Returns an empty vec for the base W3DModelDraw. Subclasses like
    /// W3DTankDraw override this to provide tread UV scrolling.
    fn collect_mesh_uv_overrides(&self) -> Vec<MeshUvOverrideState> {
        Vec::new()
    }

    /// Apply instance scaling to the world transform.
    ///
    /// C++ parity: doDrawModule() checks getDrawable()->getInstanceScale()
    /// and scales the transform matrix if != 1.0. Also calls
    /// m_renderObject->Set_ObjectScale() for proper LOD calculations.
    fn apply_instance_scale(&self, transform_mtx: &Matrix3D) -> Matrix3D {
        let instance_scale = self
            .with_owner_drawable(|drawable| drawable.get_world_scale().x)
            .unwrap_or(1.0);

        if (instance_scale - 1.0).abs() < f32::EPSILON {
            *transform_mtx
        } else {
            let scale_mtx = Matrix3D::from_scale(Coord3D::splat(instance_scale));
            *transform_mtx * scale_mtx
        }
    }

    fn collect_bone_overrides(&self) -> Vec<BoneOverrideState> {
        let mut overrides = Vec::new();
        let Some(state) = self.current_state() else {
            return overrides;
        };

        for (index, turret) in state.turrets.iter().enumerate() {
            let (turret_angle, turret_pitch) = self.get_turret_angles(index);

            if turret.turret_angle_bone != 0 {
                let angle = turret_angle + turret.turret_art_angle;
                overrides.push(BoneOverrideState {
                    bone_index: turret.turret_angle_bone,
                    transform: Matrix3D::from_rotation_z(angle),
                });
            }

            if turret.turret_pitch_bone != 0 {
                let pitch = turret_pitch + turret.turret_art_pitch;
                overrides.push(BoneOverrideState {
                    bone_index: turret.turret_pitch_bone,
                    transform: Matrix3D::from_rotation_y(-pitch),
                });
            }
        }

        for wslot in 0..WEAPONSLOT_COUNT {
            let barrels = &state.weapon_barrels[wslot];
            let Some(recoils) = self.weapon_recoil_info.get(wslot) else {
                continue;
            };
            let count = barrels.len().min(recoils.len());
            for i in 0..count {
                let shift = recoils[i].shift;
                if barrels[i].recoil_bone != 0 && shift.abs() > 0.001 {
                    overrides.push(BoneOverrideState {
                        bone_index: barrels[i].recoil_bone,
                        transform: Matrix3D::from_translation(glam::Vec3::new(-shift, 0.0, 0.0)),
                    });
                }
            }
        }

        overrides
    }

    fn get_turret_angles(&self, turret_index: usize) -> (Real, Real) {
        let mut angle = 0.0;
        let mut pitch = 0.0;
        let Some(owner_id) = self.owner_id else {
            return (angle, pitch);
        };
        let Some(obj) = TheGameLogic::find_object_by_id(owner_id) else {
            return (angle, pitch);
        };
        let Ok(obj_guard) = obj.read() else {
            return (angle, pitch);
        };
        let Some(ai) = obj_guard.get_ai_update_interface() else {
            return (angle, pitch);
        };
        let Ok(ai_guard) = ai.lock() else {
            return (angle, pitch);
        };
        let turret_type = if turret_index == 0 {
            TurretType::Primary
        } else {
            TurretType::Secondary
        };
        if let Some((a, p)) = ai_guard.get_turret_rot_and_pitch(turret_type) {
            angle = a;
            pitch = p;
        }
        (angle, pitch)
    }
}

impl Module for W3DModelDraw {
    fn on_drawable_bound_to_object(&mut self) {
        if self.data.default_state >= 0 {
            self.set_model_state(self.data.default_state as usize);
        } else if let Some(state_index) = self.find_best_state_index(&ModelConditionFlags::empty())
        {
            self.set_model_state(state_index);
        }
    }

    fn on_delete(&mut self) {
        self.particle_system_ids.clear();
    }

    fn get_module_name_key(&self) -> NameKeyType {
        NameKeyGenerator::name_to_key("W3DModelDraw")
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DModelDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        if self.fully_obscured_by_shroud || self.hidden {
            return;
        }

        self.tick_animation_state();
        if self.current_animation_complete() {
            if let Some(next_state_index) = self.next_state {
                let next_duration = self.next_state_anim_loop_duration;
                self.next_state = None;
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                self.set_model_state(next_state_index);
                if next_duration != NO_NEXT_DURATION {
                    self.set_animation_loop_duration(next_duration);
                }
            }

            if let Some(state) = self.current_state() {
                let anim_index = self.which_anim_in_cur_state;
                if anim_index >= 0 && (anim_index as usize) < state.animations.len() {
                    let should_restart = state.animations[anim_index as usize].is_idle_anim
                        || test_flag_bit(state.flags, ACBIT_RESTART_ANIM_WHEN_COMPLETE);
                    if should_restart {
                        let cur_ref = self.cur_state;
                        self.adjust_animation(cur_ref, -1.0);
                    }
                }
            }
        }

        self.adjust_anim_speed_to_movement_speed();

        // Update client-side effects
        // Reference: C++ W3DModelDraw.cpp:2075-2088

        // Update turret rotations (for tanks, etc.)
        self.handle_client_turret_positioning();

        if self.sub_objects_dirty {
            self.update_sub_objects();
        }

        // C++ parity: recalc bone particle systems and keep animated-bone systems in sync.
        if self.need_recalc_bone_particle_systems || self.data.particles_attached_to_animated_bones
        {
            let _ = self.update_bones_for_client_particle_systems();
        }

        // Update weapon recoil animations
        self.handle_client_recoil();

        self.submit_draw_to_bridge(transform_mtx);
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }

    fn release_shadows(&mut self) {
        // When shadow system is implemented, release shadow resources
        // Reference: C++ W3DModelDraw.cpp - shadow management
        // - Destroy shadow render objects
        // - Clear shadow texture/decal references
    }

    fn allocate_shadows(&mut self) {
        // When shadow system is implemented, allocate shadow resources
        // Reference: C++ W3DModelDraw.cpp - shadow management
        // - Create shadow render objects based on model bounds
        // - Set up shadow textures/decals
    }

    fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        self.terrain_decal = decal_type;
        // When terrain decal rendering is implemented, this will update the render state.
    }

    fn set_terrain_decal_size(&mut self, x: Real, y: Real) {
        self.terrain_decal_size = Some((x, y));
    }

    fn set_terrain_decal_opacity(&mut self, opacity: Real) {
        self.terrain_decal_opacity = Some(opacity);
    }

    fn set_hidden(&mut self, hidden: bool) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.do_start_or_stop_particle_sys();
        }
    }

    fn update_bones_for_client_particle_systems(&mut self) -> bool {
        W3DModelDraw::update_bones_for_client_particle_systems(self)
    }

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.fully_obscured_by_shroud != fully_obscured {
            self.fully_obscured_by_shroud = fully_obscured;
            self.do_start_or_stop_particle_sys();
        }
    }

    fn is_visible(&self) -> bool {
        !self.fully_obscured_by_shroud && !self.hidden
    }

    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        // C++ updates render-object and track-object transforms here; those runtime systems
        // are still client-side only in this port.
    }

    fn react_to_geometry_change(&mut self) {
        // C++ W3DModelDraw declares reactToGeometryChange() as a no-op.
    }

    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(self)
    }

    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(self)
    }
}

impl ObjectDrawInterface for W3DModelDraw {
    fn client_only_get_render_obj_info(
        &self,
        pos: &mut Coord3D,
        bounding_sphere_radius: &mut Real,
        transform: &mut Matrix3D,
    ) -> bool {
        let Some((position, radius, world_transform)) = self.with_owner_drawable(|drawable| {
            (
                drawable.get_position(),
                drawable.get_bounding_sphere_radius(),
                drawable.get_transform_matrix(),
            )
        }) else {
            return false;
        };

        *pos = position;
        *bounding_sphere_radius = radius;
        *transform = world_transform;
        true
    }

    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut BoundingBox) -> bool {
        let Some((min, max)) = self.with_owner_drawable(|drawable| {
            let world_box = drawable.get_bounding_box();
            (world_box.min, world_box.max)
        }) else {
            return false;
        };
        boundbox.center = (min + max) * 0.5;
        boundbox.extents = (max - min) * 0.5;
        boundbox.rotation = Matrix3D::IDENTITY;
        true
    }

    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &AsciiString,
        transform: &mut Matrix3D,
    ) -> bool {
        let Some(world_bone) =
            self.with_owner_drawable(|drawable| drawable.get_bone_transform(bone_name.as_str()))
        else {
            return false;
        };

        if let Some(world_bone) = world_bone {
            *transform = world_bone;
            true
        } else {
            *transform = Matrix3D::IDENTITY;
            false
        }
    }

    fn get_pristine_bone_positions(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        let Some(state) = self.data.find_best_info(condition) else {
            return 0;
        };

        let mut matches: Vec<(i32, &PristineBoneInfo)> = Vec::new();

        for (key, info) in &state.pristine_bones {
            let Some(name) = NameKeyGenerator::key_to_name(*key) else {
                continue;
            };

            if start_index == 0 {
                if name == bone_name_prefix {
                    matches.push((0, info));
                }
                continue;
            }

            if !name.starts_with(bone_name_prefix) {
                continue;
            }

            let suffix = &name[bone_name_prefix.len()..];
            if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if let Ok(index) = suffix.parse::<i32>() {
                if index >= start_index {
                    matches.push((index, info));
                }
            }
        }

        matches.sort_by_key(|(index, _)| *index);

        let limit = max_bones.min(positions.len()).min(transforms.len());
        let mut count = 0usize;
        for (_, info) in matches.into_iter().take(limit) {
            transforms[count] = info.transform;
            let (_, _, translation) = info.transform.to_scale_rotation_translation();
            positions[count] = translation;
            count += 1;
        }

        count
    }

    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        let limit = max_bones.min(positions.len()).min(transforms.len()).min(64);
        if limit == 0 {
            return 0;
        }

        let start = start_index.max(0);
        let Some((to_model_space, world_bones)) = self.with_owner_drawable(|drawable| {
            let inverse = drawable.get_transform_matrix().inverse();
            let inverse = if inverse.is_finite() {
                inverse
            } else {
                Matrix3D::IDENTITY
            };
            let uniform_scale = drawable.get_world_scale().x;
            let to_model_space = inverse * Matrix3D::from_scale(Coord3D::splat(uniform_scale));

            let mut world_bones = Vec::new();
            let end_index = if start == 0 { 0 } else { 99 };
            for idx in start..=end_index {
                let bone_name = if idx == 0 {
                    bone_name_prefix.to_string()
                } else {
                    format!("{bone_name_prefix}{idx:02}")
                };

                let Some(world_bone) = drawable.get_bone_transform(&bone_name) else {
                    break;
                };
                world_bones.push(world_bone);
                if world_bones.len() >= limit {
                    break;
                }
            }

            (to_model_space, world_bones)
        }) else {
            return 0;
        };

        let mut count = 0usize;
        for world_bone in world_bones {
            let local_bone = to_model_space * world_bone;
            transforms[count] = local_bone;
            positions[count] = local_bone.w_axis.truncate();
            count += 1;
        }

        count
    }

    fn get_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        launch_pos: &mut Matrix3D,
        turret_type: TurretType,
        turret_rot_pos: &mut Coord3D,
        turret_pitch_pos: &mut Coord3D,
    ) -> bool {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return false;
        }

        *turret_rot_pos = Coord3D::origin();
        *turret_pitch_pos = Coord3D::origin();

        let Some(state) = self.data.find_best_info(condition) else {
            return false;
        };

        let drawable_arc = self.owner_id.and_then(|id| {
            TheGameLogic::find_object_by_id(id)
                .and_then(|obj_arc| obj_arc.read().ok().and_then(|guard| guard.get_drawable()))
        });
        let owner_orientation = self
            .owner_id
            .and_then(TheGameLogic::find_object_by_id)
            .and_then(|obj_arc| obj_arc.read().ok().map(|guard| guard.get_orientation()))
            .unwrap_or(0.0);

        let resolve_pivot_transform = |name_key: NameKeyType| -> Option<Matrix3D> {
            if name_key == 0 {
                return None;
            }

            if let Some(info) = state.pristine_bones.get(&name_key) {
                return Some(info.transform);
            }

            let Some(name) = NameKeyGenerator::key_to_name(name_key) else {
                return None;
            };

            let Some(drawable) = &drawable_arc else {
                return None;
            };

            let Ok(draw_guard) = drawable.read() else {
                return None;
            };

            draw_guard.get_bone_local_transform(&name)
        };

        let mut tech_offset = Coord3D::origin();
        if !self.data.attach_to_drawable_bone.is_empty() {
            let attach_key =
                NameKeyGenerator::name_to_key(self.data.attach_to_drawable_bone.as_str());
            if let Some(pivot) = resolve_pivot_transform(attach_key) {
                let rotated = Matrix3D::from_rotation_z(owner_orientation) * pivot;
                tech_offset = rotated.w_axis.truncate();
            }
        }

        if turret_type != TurretType::Invalid {
            let turret_index = match turret_type {
                TurretType::Primary => Some(0),
                TurretType::Secondary => Some(1),
                TurretType::Invalid => None,
            };

            if let Some(index) = turret_index {
                if let Some(turret) = state.turrets.get(index) {
                    if let Some(rot) = resolve_pivot_transform(turret.turret_angle_name_key) {
                        *turret_rot_pos = rot.w_axis.truncate();
                    }

                    if let Some(pitch) = resolve_pivot_transform(turret.turret_pitch_name_key) {
                        *turret_pitch_pos = pitch.w_axis.truncate();
                    }
                }
            }
        }

        let barrels = &state.weapon_barrels[weapon_slot];
        if barrels.is_empty() {
            return false;
        }

        let mut selected_barrel = barrel_index;
        if selected_barrel < 0 || (selected_barrel as usize) >= barrels.len() {
            selected_barrel = 0;
        }

        let Some(barrel) = barrels.get(selected_barrel as usize) else {
            return false;
        };
        *launch_pos = barrel.projectile_offset_mtx;

        if turret_type != TurretType::Invalid {
            let turret_index = match turret_type {
                TurretType::Primary => Some(0),
                TurretType::Secondary => Some(1),
                TurretType::Invalid => None,
            };

            if let Some(index) = turret_index {
                if let Some(turret) = state.turrets.get(index) {
                    *launch_pos = Matrix3D::from_rotation_z(turret.turret_art_angle) * *launch_pos;
                    *launch_pos = Matrix3D::from_rotation_y(-turret.turret_art_pitch) * *launch_pos;
                }
            }
        }

        launch_pos.w_axis.x += tech_offset.x;
        launch_pos.w_axis.y += tech_offset.y;
        launch_pos.w_axis.z += tech_offset.z;

        true
    }

    fn update_projectile_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: usize,
    ) {
        if weapon_slot >= WEAPONSLOT_COUNT || max_shots < shots_remaining {
            return;
        }

        if (self.data.projectile_bone_feedback_enabled_slots & (1u32 << weapon_slot)) == 0 {
            return;
        }

        let Some(state) = self.current_state() else {
            return;
        };
        let feedback_prefix = {
            let override_prefix = state.weapon_projectile_hide_show_bone[weapon_slot].as_str();
            if !override_prefix.is_empty() {
                override_prefix
            } else {
                state.weapon_projectile_launch_bone[weapon_slot].as_str()
            }
            .to_string()
        };
        if feedback_prefix.is_empty() {
            return;
        }

        // C++ parity: hide first (max-shown) projectile subobjects named PREFIX01, PREFIX02, ...
        let hide_count = max_shots - shots_remaining;
        for projectile_index in 0..max_shots {
            let sub_obj_name = format!("{}{:02}", feedback_prefix, projectile_index + 1);
            let hide = projectile_index < hide_count;
            self.show_sub_object(sub_obj_name.as_str(), !hide);
        }
        self.update_sub_objects();
    }

    fn update_supply_status(&mut self, _max_supply: i32, current_supply: i32) {
        // C++ parity target is Drawable::set/clear MODELCONDITION_CARRYING.
        // In Rust this callback runs under the drawable lock, so update this draw module's
        // condition view directly to avoid object->drawable lock recursion.
        let mut conditions = self.last_model_conditions;
        if current_supply > 0 {
            conditions.insert(ModelConditionFlags::CARRYING);
        } else {
            conditions.remove(ModelConditionFlags::CARRYING);
        }
        self.last_model_conditions = conditions;
        self.replace_model_condition_state(&conditions);
    }

    fn set_hidden(&mut self, hidden: bool) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.do_start_or_stop_particle_sys();
        }
    }

    fn notify_draw_module_dependency_cleared(&mut self) {
        self.update_sub_objects();
    }

    fn replace_model_condition_state(&mut self, condition: &ModelConditionFlags) {
        self.hide_headlights = !condition.contains(ModelConditionFlags::NIGHT);
        if let Some(state_index) = self.find_best_state_index(condition) {
            self.set_model_state(state_index);
        }
    }

    fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: usize,
        barrel_index: i32,
        _victim_pos: &Coord3D,
    ) -> bool {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return false;
        }

        let (selected_barrel, barrel_info) = {
            let Some(state) = self.current_state() else {
                return false;
            };
            let barrels = &state.weapon_barrels[weapon_slot];
            if barrels.is_empty() {
                return false;
            }

            let mut selected_barrel = barrel_index;
            if selected_barrel < 0 || (selected_barrel as usize) >= barrels.len() {
                selected_barrel = 0;
            }

            (
                selected_barrel as usize,
                barrels[selected_barrel as usize].clone(),
            )
        };

        // Start recoil animation.
        if selected_barrel < self.weapon_recoil_info[weapon_slot].len() {
            self.weapon_recoil_info[weapon_slot][selected_barrel].state = RecoilState::RecoilStart;
            self.weapon_recoil_info[weapon_slot][selected_barrel].recoil_rate =
                self.data.initial_recoil;
        }

        if barrel_info.muzzle_flash_bone != 0 {
            // Muzzle-flash sub-object visibility depends on the render-object path.
            // Keep recoil behavior active while render object integration is pending.
        }

        if barrel_info.fx_bone != 0 {
            // Weapon fire FX attachment to FX bone depends on render-object transform lookups.
            // C++ only reports handled after actually invoking FXList::doFXPos from this path.
        }

        false
    }

    fn get_barrel_count(&self, weapon_slot: usize) -> i32 {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return 0;
        }

        if let Some(state) = self.current_state() {
            return state.weapon_barrels[weapon_slot].len() as i32;
        }

        0
    }
}

impl Snapshotable for W3DModelDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        // C++ W3DModelDraw::crc only extends DrawModule::crc; it does not hash
        // the versioned W3DModelDraw::xfer payload.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DModelDraw::xfer (version 2) serializes recoil vectors,
        // sub-object visibility list, and optional animation frame payload.
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        for slot in 0..WEAPONSLOT_COUNT {
            let mut recoil_info_count = self
                .weapon_recoil_info
                .get(slot)
                .map(|entries| entries.len())
                .unwrap_or_default()
                .min(u8::MAX as usize) as u8;
            xfer.xfer_unsigned_byte(&mut recoil_info_count)
                .map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                if let Some(entries) = self.weapon_recoil_info.get(slot) {
                    for entry in entries.iter().take(recoil_info_count as usize) {
                        let mut state_value = recoil_state_to_i32(entry.state);
                        let mut shift = entry.shift;
                        let mut recoil_rate = entry.recoil_rate;
                        xfer.xfer_int(&mut state_value).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut shift).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut recoil_rate)
                            .map_err(|e| e.to_string())?;
                    }
                }
            } else {
                if let Some(entries) = self.weapon_recoil_info.get_mut(slot) {
                    entries.clear();
                    for _ in 0..recoil_info_count {
                        let mut state_value = 0i32;
                        let mut shift = 0.0f32;
                        let mut recoil_rate = 0.0f32;
                        xfer.xfer_int(&mut state_value).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut shift).map_err(|e| e.to_string())?;
                        xfer.xfer_real(&mut recoil_rate)
                            .map_err(|e| e.to_string())?;
                        entries.push(WeaponRecoilInfo {
                            state: recoil_state_from_i32(state_value),
                            shift,
                            recoil_rate,
                        });
                    }
                }
            }
        }

        let mut sub_object_count = self.sub_object_vec.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut sub_object_count)
            .map_err(|e| e.to_string())?;
        if xfer.is_writing() {
            for sub_obj in self.sub_object_vec.iter().take(sub_object_count as usize) {
                let mut sub_obj_name = sub_obj.sub_obj_name.as_str().to_string();
                let mut hide = sub_obj.hide;
                xfer.xfer_ascii_string(&mut sub_obj_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hide).map_err(|e| e.to_string())?;
            }
        } else {
            self.sub_object_vec.clear();
            for _ in 0..sub_object_count {
                let mut sub_obj_name = String::new();
                let mut hide = false;
                xfer.xfer_ascii_string(&mut sub_obj_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hide).map_err(|e| e.to_string())?;
                self.sub_object_vec.push(HideShowSubObjInfo {
                    sub_obj_name: AsciiString::from(sub_obj_name.as_str()),
                    hide,
                });
            }
        }

        if version >= 2 {
            if xfer.is_writing() {
                let mut animation_payload_present =
                    self.which_anim_in_cur_state >= 0 && !self.is_current_transition_state();
                xfer.xfer_bool(&mut animation_payload_present)
                    .map_err(|e| e.to_string())?;
                if animation_payload_present {
                    let mut mode = self
                        .current_state()
                        .map(|state| anim_mode_to_i32(state.anim_mode))
                        .unwrap_or(0);
                    xfer.xfer_int(&mut mode).map_err(|e| e.to_string())?;

                    let mut percent = if self.current_anim_num_frames > 1 {
                        self.current_anim_frame as Real / (self.current_anim_num_frames - 1) as Real
                    } else {
                        0.0
                    };
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                }
            } else {
                let mut animation_payload_present = false;
                xfer.xfer_bool(&mut animation_payload_present)
                    .map_err(|e| e.to_string())?;
                if animation_payload_present {
                    let mut ignored_mode = 0i32;
                    xfer.xfer_int(&mut ignored_mode)
                        .map_err(|e| e.to_string())?;
                    let mut percent = 0.0f32;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    if self.current_anim_num_frames > 1 {
                        let frame =
                            (percent * (self.current_anim_num_frames - 1) as Real).round() as i32;
                        self.current_anim_frame = frame.clamp(0, self.current_anim_num_frames - 1);
                    } else {
                        self.current_anim_frame = 0;
                    }
                    self.current_anim_complete = false;
                }
            }
        }

        if xfer.is_reading() && !self.sub_object_vec.is_empty() {
            self.update_sub_objects();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if !self.sub_object_vec.is_empty() {
            self.update_sub_objects();
        }
        Ok(())
    }
}

fn parse_model_draw_module_data_block(
    ini: &mut INI,
    data: &mut W3DModelDrawModuleData,
) -> Result<(), INIError> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini
            .get_line_tokens()
            .into_iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        let Some(key) = tokens.first().cloned() else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }

        let value_tokens = tokens
            .iter()
            .map(String::as_str)
            .skip(1)
            .filter(|token| *token != "=")
            .collect::<Vec<_>>();
        if !data.parse_ini_field(ini, key.as_str(), &value_tokens)? {
            return Err(INIError::UnknownToken);
        }
    }
    Ok(())
}

fn parse_model_condition_info_block(
    ini: &mut INI,
    info: &mut ModelConditionInfo,
) -> Result<(), INIError> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(key) = tokens.first().copied() else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let value_tokens: Vec<&str> = tokens
            .iter()
            .copied()
            .skip(1)
            .filter(|token| *token != "=")
            .collect();
        if !parse_model_condition_info_field(info, key, &value_tokens)? {
            return Err(INIError::UnknownToken);
        }
    }
    Ok(())
}

fn parse_model_condition_info_field(
    info: &mut ModelConditionInfo,
    key: &str,
    tokens: &[&str],
) -> Result<bool, INIError> {
    match key.to_ascii_uppercase().as_str() {
        "MODEL" => {
            let model = parse_ascii_lower(parse_required_value(tokens)?)?;
            info.model_name = AsciiString::from(model.as_str());
            Ok(true)
        }
        "TURRET" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 0);
            turret.turret_angle_name_key = bone_key;
            Ok(true)
        }
        "TURRETARTANGLE" => {
            let turret = ensure_turret_slot(info, 0);
            turret.turret_art_angle = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "TURRETPITCH" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 0);
            turret.turret_pitch_name_key = bone_key;
            Ok(true)
        }
        "TURRETARTPITCH" => {
            let turret = ensure_turret_slot(info, 0);
            turret.turret_art_pitch = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "ALTTURRET" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 1);
            turret.turret_angle_name_key = bone_key;
            Ok(true)
        }
        "ALTTURRETARTANGLE" => {
            let turret = ensure_turret_slot(info, 1);
            turret.turret_art_angle = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "ALTTURRETPITCH" => {
            let bone_key = parse_bone_name_key(&mut info.public_bones, tokens)?;
            let turret = ensure_turret_slot(info, 1);
            turret.turret_pitch_name_key = bone_key;
            Ok(true)
        }
        "ALTTURRETARTPITCH" => {
            let turret = ensure_turret_slot(info, 1);
            turret.turret_art_pitch = INI::parse_angle_real(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "SHOWSUBOBJECT" => {
            parse_show_hide_sub_objects(info, tokens, false)?;
            Ok(true)
        }
        "HIDESUBOBJECT" => {
            parse_show_hide_sub_objects(info, tokens, true)?;
            Ok(true)
        }
        "WEAPONFIREFXBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_fire_fx_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONRECOILBONE" => {
            parse_weapon_bone(tokens, &mut info.weapon_recoil_bone, &mut info.public_bones)?;
            Ok(true)
        }
        "WEAPONMUZZLEFLASH" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_muzzle_flash,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONLAUNCHBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_projectile_launch_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "WEAPONHIDESHOWBONE" => {
            parse_weapon_bone(
                tokens,
                &mut info.weapon_projectile_hide_show_bone,
                &mut info.public_bones,
            )?;
            Ok(true)
        }
        "ANIMATION" => {
            parse_animation(info, tokens, false)?;
            Ok(true)
        }
        "IDLEANIMATION" => {
            parse_animation(info, tokens, true)?;
            Ok(true)
        }
        "ANIMATIONMODE" => {
            info.anim_mode = parse_anim_mode(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "TRANSITIONKEY" => {
            info.transition_key = parse_name_key_value(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "WAITFORSTATETOFINISHIFPOSSIBLE" => {
            info.allow_to_finish_key = parse_name_key_value(parse_required_value(tokens)?)?;
            Ok(true)
        }
        "FLAGS" => {
            info.flags = parse_ac_bits_flags(tokens)?;
            Ok(true)
        }
        "PARTICLESYSBONE" => {
            let bone_name = parse_ascii_lower(parse_required_value(tokens)?)?;
            let particle_system = tokens
                .iter()
                .copied()
                .skip(1)
                .find(|token| !token.is_empty())
                .map(INI::parse_ascii_string)
                .transpose()?
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            info.particle_sys_bones.push(ParticleSysBoneInfo {
                bone_name: AsciiString::from(bone_name.as_str()),
                particle_system: AsciiString::from(particle_system.as_str()),
            });
            Ok(true)
        }
        "ANIMATIONSPEEDFACTORRANGE" => {
            let min_token = parse_required_value(tokens)?;
            let max_token = tokens
                .iter()
                .copied()
                .skip(1)
                .find(|token| !token.is_empty())
                .ok_or(INIError::InvalidData)?;
            info.anim_min_speed_factor = INI::parse_real(min_token)?;
            info.anim_max_speed_factor = INI::parse_real(max_token)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)
}

fn parse_ascii_lower(token: &str) -> Result<String, INIError> {
    Ok(INI::parse_ascii_string(token)?.to_ascii_lowercase())
}

fn parse_static_game_lod_level(token: &str) -> Result<i32, INIError> {
    let value = token.trim().to_ascii_uppercase();
    match value.as_str() {
        "LOW" => Ok(0),
        "MEDIUM" => Ok(1),
        "HIGH" => Ok(2),
        _ => INI::parse_int(token),
    }
}

fn parse_name_key_value(token: &str) -> Result<NameKeyType, INIError> {
    let value = parse_ascii_lower(token)?;
    if value.is_empty() || value == "none" {
        return Ok(NAMEKEY_INVALID);
    }
    Ok(name_key_generate(&value))
}

fn parse_weapon_slot_mask(tokens: &[&str]) -> u32 {
    let mut mask = 0u32;
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };
            if let Some(idx) = parse_weapon_slot_index(value) {
                let bit = 1u32 << idx;
                if clear {
                    mask &= !bit;
                } else {
                    mask |= bit;
                }
            } else {
                warn!("Unknown weapon slot token '{}'", value);
            }
        }
    }
    mask
}

fn parse_weapon_slot_index(token: &str) -> Option<usize> {
    let mut upper = token.trim().to_ascii_uppercase();
    if let Some(stripped) = upper.strip_prefix("WEAPONSLOT_") {
        upper = stripped.to_string();
    }
    match upper.as_str() {
        "PRIMARY" | "A" => Some(0),
        "SECONDARY" | "B" => Some(1),
        "TERTIARY" | "C" => Some(2),
        _ => None,
    }
}

fn parse_model_condition_flags_tokens(tokens: &[&str]) -> ModelConditionFlags {
    let mut flags = ModelConditionFlags::empty();
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };

            let normalized = value
                .trim()
                .to_ascii_uppercase()
                .trim_start_matches("MODELCONDITION_")
                .to_string();
            if normalized == "NONE" || normalized == "INVALID" {
                if clear {
                    continue;
                }
                flags = ModelConditionFlags::empty();
                continue;
            }

            match parse_model_condition_flag(value) {
                Some(flag) if clear => flags.remove(flag),
                Some(flag) => flags.insert(flag),
                None => warn!("Unknown model condition token '{}'", value),
            }
        }
    }
    flags
}

fn does_state_exist(states: &[ModelConditionInfo], flags: ModelConditionFlags) -> bool {
    states.iter().any(|state| {
        state
            .conditions_yes
            .iter()
            .any(|existing| *existing == flags)
    })
}

fn ensure_turret_slot(info: &mut ModelConditionInfo, index: usize) -> &mut TurretInfo {
    if info.turrets.len() <= index {
        info.turrets.resize_with(index + 1, TurretInfo::new);
    }
    &mut info.turrets[index]
}

fn add_public_bone(public_bones: &mut Vec<AsciiString>, bone_name: &str) {
    if bone_name.is_empty() || bone_name.eq_ignore_ascii_case("none") {
        return;
    }
    if public_bones
        .iter()
        .any(|bone| bone.as_str().eq_ignore_ascii_case(bone_name))
    {
        return;
    }
    public_bones.push(AsciiString::from(bone_name));
}

fn parse_bone_name_key(
    public_bones: &mut Vec<AsciiString>,
    tokens: &[&str],
) -> Result<NameKeyType, INIError> {
    let value = parse_ascii_lower(parse_required_value(tokens)?)?;
    add_public_bone(public_bones, &value);
    if value.is_empty() || value == "none" {
        return Ok(NAMEKEY_INVALID);
    }
    Ok(name_key_generate(&value))
}

fn parse_show_hide_sub_objects(
    info: &mut ModelConditionInfo,
    tokens: &[&str],
    hide: bool,
) -> Result<(), INIError> {
    let mut values = tokens
        .iter()
        .copied()
        .map(INI::parse_ascii_string)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if values.len() == 1 && values[0].eq_ignore_ascii_case("none") {
        info.hide_show_list.clear();
        return Ok(());
    }

    for sub_object in values.drain(..) {
        if let Some(existing) = info.hide_show_list.iter_mut().find(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(&sub_object)
        }) {
            existing.hide = hide;
            continue;
        }
        info.hide_show_list.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from(sub_object.as_str()),
            hide,
        });
    }

    Ok(())
}

fn parse_weapon_bone(
    tokens: &[&str],
    target: &mut [AsciiString; WEAPONSLOT_COUNT],
    public_bones: &mut Vec<AsciiString>,
) -> Result<(), INIError> {
    let slot_token = parse_required_value(tokens)?;
    let slot_index = parse_weapon_slot_index(slot_token).ok_or(INIError::InvalidData)?;
    let bone_token = tokens
        .iter()
        .copied()
        .skip(1)
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)?;
    let bone_name = parse_ascii_lower(bone_token)?;
    if bone_name == "none" {
        target[slot_index] = AsciiString::new();
        return Ok(());
    }
    target[slot_index] = AsciiString::from(bone_name.as_str());
    add_public_bone(public_bones, &bone_name);
    Ok(())
}

fn parse_animation(
    info: &mut ModelConditionInfo,
    tokens: &[&str],
    idle: bool,
) -> Result<(), INIError> {
    let anim_name = parse_ascii_lower(parse_required_value(tokens)?)?;
    let distance_covered = tokens
        .iter()
        .copied()
        .skip(1)
        .find(|token| !token.is_empty())
        .map(INI::parse_real)
        .transpose()?
        .unwrap_or(0.0);
    let repeat_count = tokens
        .iter()
        .copied()
        .skip(2)
        .find(|token| !token.is_empty())
        .map(INI::parse_int)
        .transpose()?
        .unwrap_or(1)
        .max(1) as usize;

    if (info.ini_read_flags & INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT) != 0 {
        info.ini_read_flags &= !(INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT
            | INI_READ_FLAG_GOT_IDLE_ANIMS
            | INI_READ_FLAG_GOT_NONIDLE_ANIMS);
        info.animations.clear();
    }

    if idle {
        info.ini_read_flags |= INI_READ_FLAG_GOT_IDLE_ANIMS;
    } else {
        info.ini_read_flags |= INI_READ_FLAG_GOT_NONIDLE_ANIMS;
    }

    if anim_name.is_empty() || anim_name.eq_ignore_ascii_case("none") {
        return Ok(());
    }

    for _ in 0..repeat_count {
        info.animations.push(W3DAnimationInfo::new(
            AsciiString::from(anim_name.as_str()),
            idle,
            distance_covered,
        ));
    }

    Ok(())
}

fn parse_anim_mode(token: &str) -> Result<AnimMode, INIError> {
    let value = token.trim().to_ascii_uppercase();
    match value.as_str() {
        "MANUAL" => Ok(AnimMode::Manual),
        "LOOP" => Ok(AnimMode::Loop),
        "ONCE" => Ok(AnimMode::Once),
        "LOOP_PING_PONG" | "LOOPPINGPONG" => Ok(AnimMode::LoopPingPong),
        "LOOP_BACKWARDS" | "LOOPBACKWARDS" => Ok(AnimMode::LoopBackwards),
        "ONCE_BACKWARDS" | "ONCEBACKWARDS" => Ok(AnimMode::OnceBackwards),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_ac_bits_flags(tokens: &[&str]) -> Result<u32, INIError> {
    let mut bits = 0u32;
    for raw in tokens {
        for part in raw.split(|ch| ch == ',' || ch == '|') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (clear, value) = if let Some(stripped) = part.strip_prefix('-') {
                (true, stripped)
            } else if let Some(stripped) = part.strip_prefix('+') {
                (false, stripped)
            } else {
                (false, part)
            };
            let index = AC_BITS_NAMES
                .iter()
                .position(|name| name.eq_ignore_ascii_case(value))
                .ok_or(INIError::InvalidData)?;
            let mask = 1u32 << index;
            if clear {
                bits &= !mask;
            } else {
                bits |= mask;
            }
        }
    }
    Ok(bits)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseCondStateType {
    Normal,
    Default,
    Transition,
    Alias,
}

const INI_READ_FLAG_ANIMS_COPIED_FROM_DEFAULT: u32 = 1 << 0;
const INI_READ_FLAG_GOT_NONIDLE_ANIMS: u32 = 1 << 1;
const INI_READ_FLAG_GOT_IDLE_ANIMS: u32 = 1 << 2;
const NAMEKEY_INVALID: NameKeyType = 0;
const AC_BITS_NAMES: &[&str] = &[
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];
const ACBIT_RANDOMSTART: u32 = 0;
const ACBIT_START_FRAME_FIRST: u32 = 1;
const ACBIT_START_FRAME_LAST: u32 = 2;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES: u32 = 5;
const ACBIT_RESTART_ANIM_WHEN_COMPLETE: u32 = 6;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES2: u32 = 7;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES3: u32 = 8;
const ACBIT_MAINTAIN_FRAME_ACROSS_STATES4: u32 = 9;
const ALL_MAINTAIN_FRAME_FLAGS: u32 = (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES2)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES3)
    | (1u32 << ACBIT_MAINTAIN_FRAME_ACROSS_STATES4);
const NO_NEXT_DURATION: u32 = u32::MAX;
const DEFAULT_ANIMATION_FRAMES: i32 = 30;
const MSEC_PER_LOGICFRAME_REAL: Real = 1000.0 / LOGICFRAMES_PER_SECOND as Real;

fn test_flag_bit(flags: u32, bit: u32) -> bool {
    (flags & (1u32 << bit)) != 0
}

fn is_any_maintain_frame_flag_set(flags: u32) -> bool {
    (flags & ALL_MAINTAIN_FRAME_FLAGS) != 0
}

fn is_common_maintain_frame_flag_set(a: u32, b: u32) -> bool {
    (a & ALL_MAINTAIN_FRAME_FLAGS & b & ALL_MAINTAIN_FRAME_FLAGS) != 0
}

fn anim_mode_to_i32(mode: AnimMode) -> i32 {
    match mode {
        AnimMode::Manual => 0,
        AnimMode::Loop => 1,
        AnimMode::Once => 2,
        AnimMode::LoopPingPong => 3,
        AnimMode::LoopBackwards => 4,
        AnimMode::OnceBackwards => 5,
    }
}

fn model_condition_valid_stuff(state: &ModelConditionInfo) -> u8 {
    let mut valid = state.valid_stuff;
    if !state.pristine_bones.is_empty() {
        valid |= MODEL_CONDITION_PRISTINE_BONES_VALID;
    }
    if !state.turrets.is_empty() {
        valid |= MODEL_CONDITION_TURRETS_VALID;
    }
    if state
        .weapon_projectile_launch_bone
        .iter()
        .any(|name| !name.is_empty())
    {
        valid |= MODEL_CONDITION_HAS_PROJECTILE_BONES;
    }
    if state
        .weapon_barrels
        .iter()
        .any(|barrels| !barrels.is_empty())
    {
        valid |= MODEL_CONDITION_BARRELS_VALID;
    }
    if !state.public_bones.is_empty() {
        valid |= MODEL_CONDITION_PUBLIC_BONES_VALID;
    }
    valid
}

// Constants
const WEAPONSLOT_COUNT: usize = 3;
const MAX_TURRETS: usize = 2;
const MODEL_CONDITION_PRISTINE_BONES_VALID: u8 = 0x01;
const MODEL_CONDITION_TURRETS_VALID: u8 = 0x02;
const MODEL_CONDITION_HAS_PROJECTILE_BONES: u8 = 0x04;
const MODEL_CONDITION_BARRELS_VALID: u8 = 0x08;
const MODEL_CONDITION_PUBLIC_BONES_VALID: u8 = 0x10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_sub_object_is_case_insensitive() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.show_sub_object("Gun_Barrel", true);
        draw.show_sub_object("gun_barrel", false);
        draw.update_sub_objects();

        assert_eq!(draw.sub_object_vec.len(), 1);
        assert_eq!(draw.sub_object_vec[0].sub_obj_name.as_str(), "gun_barrel");
        assert!(draw.sub_object_vec[0].hide);
    }

    #[test]
    fn update_sub_objects_deduplicates_by_normalized_name() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.sub_object_vec.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from("Wheel_L"),
            hide: false,
        });
        draw.sub_object_vec.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from("wheel_l"),
            hide: true,
        });
        draw.sub_objects_dirty = true;

        draw.update_sub_objects();

        assert_eq!(draw.sub_object_vec.len(), 1);
        assert_eq!(draw.sub_object_vec[0].sub_obj_name.as_str(), "wheel_l");
        assert!(draw.sub_object_vec[0].hide);
        assert!(!draw.sub_objects_dirty);
    }

    #[test]
    fn module_name_key_is_model_draw() {
        let draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DModelDraw")
        );
    }

    #[test]
    fn react_to_geometry_change_is_noop_like_cpp() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.need_recalc_bone_particle_systems = false;

        draw.react_to_geometry_change();

        assert!(!draw.need_recalc_bone_particle_systems);
    }

    #[test]
    fn model_condition_valid_stuff_preserves_cpp_bit_layout() {
        let mut state = ModelConditionInfo::new();
        state.pristine_bones.insert(
            1,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 7,
            },
        );
        state.turrets.push(TurretInfo::new());
        state.weapon_projectile_launch_bone[0] = AsciiString::from("MUZZLE");
        state.weapon_barrels[0].push(WeaponBarrelInfo::new());
        state.public_bones.push(AsciiString::from("HEAD"));

        assert_eq!(
            model_condition_valid_stuff(&state),
            MODEL_CONDITION_PRISTINE_BONES_VALID
                | MODEL_CONDITION_TURRETS_VALID
                | MODEL_CONDITION_HAS_PROJECTILE_BONES
                | MODEL_CONDITION_BARRELS_VALID
                | MODEL_CONDITION_PUBLIC_BONES_VALID
        );
    }

    #[test]
    fn projectile_launch_bone_updates_stored_valid_stuff_bit() {
        let mut state = ModelConditionInfo::new();
        state.weapon_projectile_launch_bone[0] = AsciiString::from("muzzle");

        state.refresh_projectile_valid_bit();

        assert_eq!(
            state.valid_stuff & MODEL_CONDITION_HAS_PROJECTILE_BONES,
            MODEL_CONDITION_HAS_PROJECTILE_BONES
        );
    }

    #[test]
    fn runtime_validation_merges_extra_public_bones_once() {
        let mut state = ModelConditionInfo::new();
        let extra = [AsciiString::from("extra_bone")];

        state.validate_runtime_caches(&extra);
        state.validate_runtime_caches(&extra);

        assert_eq!(
            state.valid_stuff & MODEL_CONDITION_PUBLIC_BONES_VALID,
            MODEL_CONDITION_PUBLIC_BONES_VALID
        );
        assert_eq!(
            state
                .public_bones
                .iter()
                .filter(|bone| bone.as_str() == "extra_bone")
                .count(),
            1
        );
    }

    #[test]
    fn matrix3d_user_xfer_uses_cpp_raw_3x4_row_layout() {
        use game_engine::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut matrix = Matrix3D::from_cols_array(&[
            1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0, 3.0, 7.0, 11.0, 15.0, 4.0, 8.0, 12.0, 16.0,
        ]);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("w3d_model_draw_matrix3d_user").unwrap();
            xfer_matrix3d_values(&mut save, &mut matrix).unwrap();
            save.close().unwrap();
        }

        assert_eq!(bytes.len(), 12 * std::mem::size_of::<f32>());
        for (index, expected) in (1..=12).map(|value| value as f32).enumerate() {
            let start = index * std::mem::size_of::<f32>();
            assert_eq!(&bytes[start..start + 4], &expected.to_le_bytes());
        }
    }

    #[test]
    fn matrix3d_user_xfer_load_restores_glam_bottom_row() {
        use game_engine::common::system::xfer_load::XferLoad;
        use std::io::Cursor;

        let bytes = (1..=12)
            .flat_map(|value| (value as f32).to_le_bytes())
            .collect::<Vec<_>>();
        let mut matrix = Matrix3D::from_cols_array(&[99.0; 16]);
        let mut load = XferLoad::new(Cursor::new(bytes), 1);

        load.open("w3d_model_draw_matrix3d_user").unwrap();
        xfer_matrix3d_values(&mut load, &mut matrix).unwrap();
        load.close().unwrap();

        assert_eq!(
            matrix.to_cols_array(),
            [1.0, 5.0, 9.0, 0.0, 2.0, 6.0, 10.0, 0.0, 3.0, 7.0, 11.0, 0.0, 4.0, 8.0, 12.0, 1.0]
        );
    }

    #[test]
    fn exposes_object_draw_interface_from_draw_module() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());

        assert!(DrawModule::get_object_draw_interface(&draw).is_some());
        assert!(DrawModule::get_object_draw_interface_mut(&mut draw).is_some());
    }

    #[test]
    fn bound_model_draw_initializes_best_empty_condition_state() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);

        draw.on_drawable_bound_to_object();

        assert_eq!(draw.cur_state, Some(ActiveModelState::Condition(0)));
    }

    #[test]
    fn bone_overrides_use_cpp_pitch_and_recoil_signs() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.turrets.push(TurretInfo {
            turret_pitch_bone: 11,
            turret_art_pitch: 0.25,
            ..TurretInfo::new()
        });
        state.weapon_barrels[0].push(WeaponBarrelInfo {
            recoil_bone: 12,
            ..WeaponBarrelInfo::new()
        });
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));
        draw.weapon_recoil_info[0].push(WeaponRecoilInfo {
            state: RecoilState::Recoil,
            shift: 0.5,
            recoil_rate: 0.0,
        });

        let overrides = draw.collect_bone_overrides();

        let pitch = overrides
            .iter()
            .find(|override_state| override_state.bone_index == 11)
            .expect("pitch override");
        let recoil = overrides
            .iter()
            .find(|override_state| override_state.bone_index == 12)
            .expect("recoil override");

        let pitch_cols = pitch.transform.to_cols_array();
        assert!(pitch_cols[2] < 0.0);
        assert_eq!(recoil.transform.w_axis.x, -0.5);
    }

    #[test]
    fn weapon_fire_fx_starts_recoil_but_does_not_fabricate_handled_fx() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_barrels[0].push(WeaponBarrelInfo {
            fx_bone: 7,
            recoil_bone: 12,
            muzzle_flash_bone: 13,
            ..WeaponBarrelInfo::new()
        });
        data.condition_states.push(state);
        data.initial_recoil = 1.75;

        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));
        draw.weapon_recoil_info[0].push(WeaponRecoilInfo::new());

        let handled = ObjectDrawInterface::handle_weapon_fire_fx(
            &mut draw,
            0,
            0,
            &Coord3D::new(1.0, 2.0, 3.0),
        );

        assert!(!handled);
        assert!(matches!(
            draw.weapon_recoil_info[0][0].state,
            RecoilState::RecoilStart
        ));
        assert_eq!(draw.weapon_recoil_info[0][0].recoil_rate, 1.75);
    }

    #[test]
    fn state_activation_populates_turret_bones_from_pristine_bones() {
        let turret_key = name_key_generate("turret");
        let pitch_key = name_key_generate("pitch");

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.turrets.push(TurretInfo {
            turret_angle_name_key: turret_key,
            turret_pitch_name_key: pitch_key,
            ..TurretInfo::new()
        });
        state.pristine_bones.insert(
            turret_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 21,
            },
        );
        state.pristine_bones.insert(
            pitch_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 22,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let state = draw.current_state().expect("active state");
        assert_eq!(state.turrets[0].turret_angle_bone, 21);
        assert_eq!(state.turrets[0].turret_pitch_bone, 22);
    }

    #[test]
    fn state_activation_populates_numbered_weapon_barrels_like_cpp() {
        let fx01_key = name_key_generate("fx01");
        let recoil01_key = name_key_generate("recoil01");
        let muzzle01_key = name_key_generate("muzzle01");
        let muzzle02_key = name_key_generate("muzzle02");
        let projectile01_key = name_key_generate("projectile01");
        let projectile02_key = name_key_generate("projectile02");
        let projectile01 = Matrix3D::from_translation(glam::Vec3::new(1.0, 0.0, 0.0));
        let projectile02 = Matrix3D::from_translation(glam::Vec3::new(2.0, 0.0, 0.0));

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_fire_fx_bone[0] = AsciiString::from("fx");
        state.weapon_recoil_bone[0] = AsciiString::from("recoil");
        state.weapon_muzzle_flash[0] = AsciiString::from("muzzle");
        state.weapon_projectile_launch_bone[0] = AsciiString::from("projectile");
        state.pristine_bones.insert(
            fx01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 31,
            },
        );
        state.pristine_bones.insert(
            recoil01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 32,
            },
        );
        state.pristine_bones.insert(
            muzzle01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 33,
            },
        );
        state.pristine_bones.insert(
            muzzle02_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 34,
            },
        );
        state.pristine_bones.insert(
            projectile01_key,
            PristineBoneInfo {
                transform: projectile01,
                bone_index: 35,
            },
        );
        state.pristine_bones.insert(
            projectile02_key,
            PristineBoneInfo {
                transform: projectile02,
                bone_index: 36,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let barrels = &draw.current_state().expect("active state").weapon_barrels[0];
        assert_eq!(barrels.len(), 2);
        assert_eq!(barrels[0].fx_bone, 31);
        assert_eq!(barrels[0].recoil_bone, 32);
        assert_eq!(barrels[0].muzzle_flash_bone, 33);
        assert_eq!(barrels[0].projectile_offset_mtx.w_axis.x, 1.0);
        assert_eq!(barrels[1].fx_bone, 31);
        assert_eq!(barrels[1].muzzle_flash_bone, 34);
        assert_eq!(barrels[1].projectile_offset_mtx.w_axis.x, 2.0);
    }

    #[test]
    fn state_activation_uses_unadorned_weapon_bone_fallback() {
        let fx_key = name_key_generate("fx");
        let projectile_key = name_key_generate("projectile");
        let projectile = Matrix3D::from_translation(glam::Vec3::new(3.0, 0.0, 0.0));

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_fire_fx_bone[0] = AsciiString::from("fx");
        state.weapon_projectile_launch_bone[0] = AsciiString::from("projectile");
        state.pristine_bones.insert(
            fx_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 41,
            },
        );
        state.pristine_bones.insert(
            projectile_key,
            PristineBoneInfo {
                transform: projectile,
                bone_index: 42,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let barrels = &draw.current_state().expect("active state").weapon_barrels[0];
        assert_eq!(barrels.len(), 1);
        assert_eq!(barrels[0].fx_bone, 41);
        assert_eq!(barrels[0].projectile_offset_mtx.w_axis.x, 3.0);
    }
}
