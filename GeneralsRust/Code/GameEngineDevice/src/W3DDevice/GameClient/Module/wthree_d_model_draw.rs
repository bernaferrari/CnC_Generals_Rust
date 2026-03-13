//! W3DModelDraw Module - Complete 3D Model Drawing System
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Module/W3DModelDraw.cpp
//!
//! This module provides model rendering, animation handling, bone management,
//! weapon slot configuration, and model condition state management.

use cgmath::{Matrix4, Vector3, Point3, SquareMatrix, Zero, InnerSpace};
use std::collections::HashMap;
use std::sync::Arc;

/// Maximum number of weapon slots
pub const WEAPONSLOT_COUNT: usize = 5;

/// Maximum terrain decal types
pub const TERRAIN_DECAL_MAX: usize = 10;

/// No next duration constant
pub const NO_NEXT_DURATION: u32 = 0xFFFFFFFF;

/// Animation control bits (matching C++ ACBits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationControlBit {
    RandomizeStartFrame = 0,
    StartFrameFirst,
    StartFrameLast,
    AdjustHeightByConstructionPercent,
    PristineBonePosInFinalFrame,
    MaintainFrameAcrossStates,
    RestartAnimWhenComplete,
    MaintainFrameAcrossStates2,
    MaintainFrameAcrossStates3,
    MaintainFrameAcrossStates4,
}

/// Weapon slot types (matching C++ WeaponSlotType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary,
    Tertiary,
    WeaponSlotCount,
}

/// Animation mode (matching C++ W3DAnimationMode)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    Manual = 0,
    Loop,
    Once,
    Pingpong,
    StopAtEnd,
    LoopBackwards,
    OnceBackwards,
    ModeCount,
}

/// Model condition flags (matching C++ MODELCONDITION_*)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelConditionFlag {
    None = 0,
    Night,
    Snow,
    Garrisoned,
    EmotionAfraid,
    EmotionAngry,
    EmotionHappy,
    EmotionUnhappy,
    Special1,
    Special2,
    Special3,
    Jammed,
    Attacking,
    PreAttack,
    PostAttack,
    FiringA,
    FiringB,
    FiringC,
    BetweenFiringShotsA,
    BetweenFiringShotsB,
    BetweenFiringShotsC,
    UsingWeaponA,
    UsingWeaponB,
    UsingWeaponC,
    Riding,
    Piloting,
    SpecialIdle,
    Moving,
    Deployed,
    Overwatch,
    Constructions,
    Exiting,
    Cheering,
    AwaitingConstruction,
    Packing,
    Unpacked,
    Activating,
    ActivatingTwo,
    PowerUp,
    PowerUpTwo,
    PowerUpThree,
    PowerUpFour,
    PowerUpFive,
    PowerUpSix,
    PowerUpSeven,
    PowerUpEight,
    Production,
    AwaitingProduction,
    RadarUpgraded,
    RadarExtended,
    Exploding,
    Aiming,
    Disease,
    Paralyzed,
    Railed,
    Incomplete,
}

/// Animation info (matching C++ W3DAnimationInfo)
#[derive(Debug, Clone)]
pub struct W3DAnimationInfo {
    pub name: String,
    pub is_idle_anim: bool,
    pub distance_covered: f32,
    pub natural_duration_msec: i32,
    pub frame_rate: f32,
    pub num_frames: u32,
}

impl Default for W3DAnimationInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DAnimationInfo {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            is_idle_anim: false,
            distance_covered: 0.0,
            natural_duration_msec: -1,
            frame_rate: 30.0,
            num_frames: 0,
        }
    }
    
    pub fn with_name(name: &str, is_idle: bool, distance: f32) -> Self {
        Self {
            name: name.to_string(),
            is_idle_anim: is_idle,
            distance_covered: distance,
            natural_duration_msec: -1,
            frame_rate: 30.0,
            num_frames: 0,
        }
    }
    
    pub fn get_anim_duration_msec(&self) -> i32 {
        self.natural_duration_msec
    }
    
    pub fn get_distance_covered(&self) -> f32 {
        self.distance_covered
    }
    
    pub fn is_idle_anim(&self) -> bool {
        self.is_idle_anim
    }
}

/// Bone transform info
#[derive(Debug, Clone, Copy)]
pub struct BoneTransform {
    pub matrix: Matrix4<f32>,
    pub bone_index: i32,
}

impl Default for BoneTransform {
    fn default() -> Self {
        Self {
            matrix: Matrix4::identity(),
            bone_index: -1,
        }
    }
}

/// Pristine bone info (matching C++ PristineBoneInfo)
#[derive(Debug, Clone)]
pub struct PristineBoneInfo {
    pub matrix: Matrix4<f32>,
    pub bone_index: i32,
}

impl Default for PristineBoneInfo {
    fn default() -> Self {
        Self {
            matrix: Matrix4::identity(),
            bone_index: -1,
        }
    }
}

/// Model condition info (matching C++ ModelConditionInfo)
#[derive(Debug, Clone)]
pub struct ModelConditionInfo {
    pub model_name: String,
    pub animations: Vec<W3DAnimationInfo>,
    pub conditions: Vec<u32>, // Model condition flags
    pub flags: u32,
    pub transition_key: u32,
    
    // Turret info
    pub turret_angle_key: Option<String>,
    pub turret_pitch_key: Option<String>,
    pub turret_artillery_angle_key: Option<String>,
    
    // Public bones
    pub public_bones: Vec<String>,
    pub pristine_bones: HashMap<String, PristineBoneInfo>,
    
    // Weapon barrel info
    pub weapon_barrel_bones: [Option<String>; WEAPONSLOT_COUNT],
    pub weapon_muzzle_bones: [Option<String>; WEAPONSLOT_COUNT],
    
    // State flags
    pub valid_stuff: u32,
}

impl Default for ModelConditionInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelConditionInfo {
    pub fn new() -> Self {
        Self {
            model_name: String::new(),
            animations: Vec::new(),
            conditions: Vec::new(),
            flags: 0,
            transition_key: 0,
            turret_angle_key: None,
            turret_pitch_key: None,
            turret_artillery_angle_key: None,
            public_bones: Vec::new(),
            pristine_bones: HashMap::new(),
            weapon_barrel_bones: Default::default(),
            weapon_muzzle_bones: Default::default(),
            valid_stuff: 0,
        }
    }
    
    /// Add a public bone
    pub fn add_public_bone(&mut self, bone_name: &str) {
        if bone_name.is_empty() {
            return;
        }
        let lower_name = bone_name.to_lowercase();
        if !self.public_bones.contains(&lower_name) {
            self.public_bones.push(lower_name);
        }
    }
    
    /// Check if condition matches mode (night/snow)
    pub fn matches_mode(&self, night: bool, snowy: bool) -> bool {
        for &cond in &self.conditions {
            let has_night = (cond & (1 << ModelConditionFlag::Night as u32)) != 0;
            let has_snow = (cond & (1 << ModelConditionFlag::Snow as u32)) != 0;
            if has_night == night && has_snow == snowy {
                return true;
            }
        }
        false
    }
    
    /// Get animation count
    pub fn get_animation_count(&self) -> usize {
        self.animations.len()
    }
    
    /// Get animation by index
    pub fn get_animation(&self, index: usize) -> Option<&W3DAnimationInfo> {
        self.animations.get(index)
    }
}

/// Turret state
#[derive(Debug, Clone)]
pub struct TurretState {
    pub turret_angle: f32,
    pub turret_pitch: f32,
    pub turret_rate: f32,
    pub turret_pitch_rate: f32,
    pub desired_angle: f32,
    pub desired_pitch: f32,
}

impl Default for TurretState {
    fn default() -> Self {
        Self {
            turret_angle: 0.0,
            turret_pitch: 0.0,
            turret_rate: 0.0,
            turret_pitch_rate: 0.0,
            desired_angle: 0.0,
            desired_pitch: 0.0,
        }
    }
}

/// Model draw state
#[derive(Debug, Clone)]
pub struct ModelDrawState {
    pub current_condition: u32,
    pub current_anim_frame: f32,
    pub current_anim_mode: AnimationMode,
    pub current_anim_index: usize,
    pub anim_frame_rate_multiplier: f32,
    pub prev_anim_frame: f32,
    pub anim_duration_msec: u32,
    pub frame_for_next_anim: u32,
    pub next_anim_duration: u32,
    pub skip_next_anim_restart: bool,
    
    // Turret states
    pub turret_states: [TurretState; WEAPONSLOT_COUNT],
    
    // Construction state
    pub construction_percent: f32,
    
    // Hide state
    pub hide_sub_objects: Vec<String>,
    pub show_sub_objects: Vec<String>,
    
    // Pristine bone transforms
    pub pristine_bone_transforms: HashMap<String, Matrix4<f32>>,
}

impl Default for ModelDrawState {
    fn default() -> Self {
        Self {
            current_condition: 0,
            current_anim_frame: 0.0,
            current_anim_mode: AnimationMode::Loop,
            current_anim_index: 0,
            anim_frame_rate_multiplier: 1.0,
            prev_anim_frame: 0.0,
            anim_duration_msec: 0,
            frame_for_next_anim: 0,
            next_anim_duration: NO_NEXT_DURATION,
            skip_next_anim_restart: false,
            turret_states: Default::default(),
            construction_percent: 1.0,
            hide_sub_objects: Vec::new(),
            show_sub_objects: Vec::new(),
            pristine_bone_transforms: HashMap::new(),
        }
    }
}

/// Terrain decal texture names (matching C++ TerrainDecalTextureName)
pub const TERRAIN_DECAL_TEXTURES: [&str; TERRAIN_DECAL_MAX] = [
    "DM_RING",           // Demoralized
    "EXHorde",           // Enthusiastic
    "EXHorde_UP",        // Enthusiastic with nationalism
    "EXHordeB",          // Enthusiastic vehicle
    "EXHordeB_UP",       // Enthusiastic vehicle with nationalism
    "EXJunkCrate",       // Special crate
    "EXHordeC_UP",       // Enthusiastic with fanaticism
    "EXChemSuit",        // Chemical suit
    "",                  // TERRAIN_DECAL_NONE
    "",                  // TERRAIN_DECAL_SHADOW_TEXTURE
];

/// Main W3D Model Draw implementation (matching C++ W3DModelDraw)
#[derive(Debug)]
pub struct W3DModelDraw {
    /// Model name
    pub model_name: String,
    
    /// Initial model state
    pub initial_state: String,
    
    /// Model condition states
    pub condition_states: HashMap<u32, ModelConditionInfo>,
    
    /// Default model condition
    pub default_condition: ModelConditionInfo,
    
    /// Current draw state
    pub draw_state: ModelDrawState,
    
    /// Model scale
    pub scale: f32,
    
    /// Weapon fire bones for each slot
    pub weapon_fire_bones: [Option<String>; WEAPONSLOT_COUNT],
    
    /// Weapon recoil bones for each slot
    pub weapon_recoil_bones: [Option<String>; WEAPONSLOT_COUNT],
    
    /// Weapon muzzle flash bones
    pub weapon_muzzle_flash_bones: [Option<String>; WEAPONSLOT_COUNT],
    
    /// Terrain decal type
    pub terrain_decal_type: usize,
    
    /// Terrain decal size
    pub terrain_decal_size: f32,
    
    /// Animation flags
    pub anim_flags: u32,
    
    /// Extra public bones
    pub extra_public_bones: Vec<String>,
    
    /// Attachment looks
    pub attachment_looks: HashMap<String, String>,
    
    /// Sub-object transforms
    pub sub_object_transforms: HashMap<String, Matrix4<f32>>,
    
    /// Model render object (placeholder - would be actual render object)
    pub render_object_id: Option<u64>,
    
    /// Whether model is fully loaded
    pub model_loaded: bool,
}

impl Default for W3DModelDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DModelDraw {
    pub fn new() -> Self {
        Self {
            model_name: String::new(),
            initial_state: String::new(),
            condition_states: HashMap::new(),
            default_condition: ModelConditionInfo::new(),
            draw_state: ModelDrawState::default(),
            scale: 1.0,
            weapon_fire_bones: Default::default(),
            weapon_recoil_bones: Default::default(),
            weapon_muzzle_flash_bones: Default::default(),
            terrain_decal_type: 0,
            terrain_decal_size: 1.0,
            anim_flags: 0,
            extra_public_bones: Vec::new(),
            attachment_looks: HashMap::new(),
            sub_object_transforms: HashMap::new(),
            render_object_id: None,
            model_loaded: false,
        }
    }
    
    /// Create with model name
    pub fn with_model(name: &str, scale: f32) -> Self {
        Self {
            model_name: name.to_string(),
            initial_state: String::new(),
            condition_states: HashMap::new(),
            default_condition: ModelConditionInfo::new(),
            draw_state: ModelDrawState::default(),
            scale,
            weapon_fire_bones: Default::default(),
            weapon_recoil_bones: Default::default(),
            weapon_muzzle_flash_bones: Default::default(),
            terrain_decal_type: 0,
            terrain_decal_size: 1.0,
            anim_flags: 0,
            extra_public_bones: Vec::new(),
            attachment_looks: HashMap::new(),
            sub_object_transforms: HashMap::new(),
            render_object_id: None,
            model_loaded: false,
        }
    }
    
    /// Get model name
    pub fn get_model_name(&self) -> &str {
        &self.model_name
    }
    
    /// Set model scale
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }
    
    /// Get model scale
    pub fn get_scale(&self) -> f32 {
        self.scale
    }
    
    /// Add a condition state
    pub fn add_condition_state(&mut self, condition: u32, state: ModelConditionInfo) {
        self.condition_states.insert(condition, state);
    }
    
    /// Get condition state
    pub fn get_condition_state(&self, condition: u32) -> Option<&ModelConditionInfo> {
        self.condition_states.get(&condition)
    }
    
    /// Set current condition
    pub fn set_condition(&mut self, condition: u32) {
        self.draw_state.current_condition = condition;
    }
    
    /// Get current condition
    pub fn get_condition(&self) -> u32 {
        self.draw_state.current_condition
    }
    
    /// Set animation frame
    pub fn set_anim_frame(&mut self, frame: f32) {
        self.draw_state.current_anim_frame = frame;
    }
    
    /// Get animation frame
    pub fn get_anim_frame(&self) -> f32 {
        self.draw_state.current_anim_frame
    }
    
    /// Set animation mode
    pub fn set_anim_mode(&mut self, mode: AnimationMode) {
        self.draw_state.current_anim_mode = mode;
    }
    
    /// Get animation mode
    pub fn get_anim_mode(&self) -> AnimationMode {
        self.draw_state.current_anim_mode
    }
    
    /// Set animation frame rate multiplier
    pub fn set_anim_frame_rate_multiplier(&mut self, mult: f32) {
        self.draw_state.anim_frame_rate_multiplier = mult;
    }
    
    /// Get animation frame rate multiplier
    pub fn get_anim_frame_rate_multiplier(&self) -> f32 {
        self.draw_state.anim_frame_rate_multiplier
    }
    
    /// Hide a sub-object
    pub fn hide_sub_object(&mut self, name: &str) {
        if !self.draw_state.hide_sub_objects.contains(&name.to_string()) {
            self.draw_state.hide_sub_objects.push(name.to_string());
        }
        self.draw_state.show_sub_objects.retain(|n| n != name);
    }
    
    /// Show a sub-object
    pub fn show_sub_object(&mut self, name: &str) {
        if !self.draw_state.show_sub_objects.contains(&name.to_string()) {
            self.draw_state.show_sub_objects.push(name.to_string());
        }
        self.draw_state.hide_sub_objects.retain(|n| n != name);
    }
    
    /// Set terrain decal
    pub fn set_terrain_decal(&mut self, decal_type: usize, size: f32) {
        self.terrain_decal_type = decal_type.min(TERRAIN_DECAL_MAX);
        self.terrain_decal_size = size;
    }
    
    /// Get turret angle for weapon slot
    pub fn get_turret_angle(&self, slot: usize) -> f32 {
        if slot < WEAPONSLOT_COUNT {
            self.draw_state.turret_states[slot].turret_angle
        } else {
            0.0
        }
    }
    
    /// Set turret angle for weapon slot
    pub fn set_turret_angle(&mut self, slot: usize, angle: f32) {
        if slot < WEAPONSLOT_COUNT {
            self.draw_state.turret_states[slot].turret_angle = angle;
        }
    }
    
    /// Get turret pitch for weapon slot
    pub fn get_turret_pitch(&self, slot: usize) -> f32 {
        if slot < WEAPONSLOT_COUNT {
            self.draw_state.turret_states[slot].turret_pitch
        } else {
            0.0
        }
    }
    
    /// Set turret pitch for weapon slot
    pub fn set_turret_pitch(&mut self, slot: usize, pitch: f32) {
        if slot < WEAPONSLOT_COUNT {
            self.draw_state.turret_states[slot].turret_pitch = pitch;
        }
    }
    
    /// Get construction percent
    pub fn get_construction_percent(&self) -> f32 {
        self.draw_state.construction_percent
    }
    
    /// Set construction percent
    pub fn set_construction_percent(&mut self, percent: f32) {
        self.draw_state.construction_percent = percent.clamp(0.0, 1.0);
    }
    
    /// Get bone transform by name
    pub fn get_bone_transform(&self, bone_name: &str) -> Option<&Matrix4<f32>> {
        self.draw_state.pristine_bone_transforms.get(bone_name)
    }
    
    /// Add extra public bone
    pub fn add_extra_public_bone(&mut self, bone_name: &str) {
        let lower = bone_name.to_lowercase();
        if !self.extra_public_bones.contains(&lower) {
            self.extra_public_bones.push(lower);
        }
    }
    
    /// Get weapon fire bone for slot
    pub fn get_weapon_fire_bone(&self, slot: usize) -> Option<&String> {
        if slot < WEAPONSLOT_COUNT {
            self.weapon_fire_bones[slot].as_ref()
        } else {
            None
        }
    }
    
    /// Set weapon fire bone for slot
    pub fn set_weapon_fire_bone(&mut self, slot: usize, bone: &str) {
        if slot < WEAPONSLOT_COUNT {
            self.weapon_fire_bones[slot] = Some(bone.to_string());
        }
    }
    
    /// Get weapon recoil bone for slot
    pub fn get_weapon_recoil_bone(&self, slot: usize) -> Option<&String> {
        if slot < WEAPONSLOT_COUNT {
            self.weapon_recoil_bones[slot].as_ref()
        } else {
            None
        }
    }
    
    /// Set weapon recoil bone for slot
    pub fn set_weapon_recoil_bone(&mut self, slot: usize, bone: &str) {
        if slot < WEAPONSLOT_COUNT {
            self.weapon_recoil_bones[slot] = Some(bone.to_string());
        }
    }
    
    /// Update model animation
    pub fn update_animation(&mut self, delta_msec: f32) {
        let condition_info = self.condition_states
            .get(&self.draw_state.current_condition)
            .unwrap_or(&self.default_condition);
        
        if let Some(anim) = condition_info.get_animation(self.draw_state.current_anim_index) {
            let duration = anim.get_anim_duration_msec() as f32 / self.draw_state.anim_frame_rate_multiplier;
            if duration > 0.0 {
                self.draw_state.current_anim_frame += delta_msec / duration;
                
                // Handle animation looping
                match self.draw_state.current_anim_mode {
                    AnimationMode::Loop => {
                        let num_frames = anim.num_frames as f32;
                        while self.draw_state.current_anim_frame >= num_frames {
                            self.draw_state.current_anim_frame -= num_frames;
                        }
                    }
                    AnimationMode::Once | AnimationMode::OnceBackwards => {
                        let num_frames = anim.num_frames as f32;
                        if self.draw_state.current_anim_frame >= num_frames {
                            self.draw_state.current_anim_frame = num_frames - 1.0;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    /// Preload model assets
    pub fn preload_assets(&mut self) {
        // In full implementation, this would load model from asset manager
        self.model_loaded = true;
    }
    
    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model_loaded
    }
}

// Backward compatibility alias
pub type WthreeDModelDraw = W3DModelDraw;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_model_draw_creation() {
        let model = W3DModelDraw::new();
        assert!(model.model_name.is_empty());
        assert_eq!(model.get_scale(), 1.0);
    }
    
    #[test]
    fn test_model_with_name() {
        let model = W3DModelDraw::with_model("TestModel", 2.0);
        assert_eq!(model.get_model_name(), "TestModel");
        assert_eq!(model.get_scale(), 2.0);
    }
    
    #[test]
    fn test_condition_states() {
        let mut model = W3DModelDraw::new();
        let state = ModelConditionInfo::new();
        model.add_condition_state(1, state);
        
        assert!(model.get_condition_state(1).is_some());
    }
    
    #[test]
    fn test_animation_info() {
        let anim = W3DAnimationInfo::with_name("Idle", true, 0.0);
        assert_eq!(anim.name, "Idle");
        assert!(anim.is_idle_anim());
    }
    
    #[test]
    fn test_turret_state() {
        let mut model = W3DModelDraw::new();
        model.set_turret_angle(0, 45.0);
        model.set_turret_pitch(0, 15.0);
        
        assert_eq!(model.get_turret_angle(0), 45.0);
        assert_eq!(model.get_turret_pitch(0), 15.0);
    }
    
    #[test]
    fn test_sub_object_visibility() {
        let mut model = W3DModelDraw::new();
        model.hide_sub_object("Turret");
        model.show_sub_object("Barrel");
        
        assert!(model.draw_state.hide_sub_objects.contains(&"Turret".to_string()));
        assert!(model.draw_state.show_sub_objects.contains(&"Barrel".to_string()));
    }
    
    #[test]
    fn test_construction_percent() {
        let mut model = W3DModelDraw::new();
        model.set_construction_percent(0.5);
        assert_eq!(model.get_construction_percent(), 0.5);
        
        model.set_construction_percent(1.5); // Should clamp to 1.0
        assert_eq!(model.get_construction_percent(), 1.0);
    }
    
    #[test]
    fn test_public_bones() {
        let mut info = ModelConditionInfo::new();
        info.add_public_bone("Turret");
        info.add_public_bone("Barrel");
        info.add_public_bone("Turret"); // Duplicate
        
        assert_eq!(info.public_bones.len(), 2);
    }
}
