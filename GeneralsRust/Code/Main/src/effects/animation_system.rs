use glam::{Mat4, Quat, Vec3};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;

use crate::game_logic::ObjectId;

/// Animation types matching C&C categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationType {
    // Unit animations
    UnitIdle,
    UnitMove,
    UnitAttack,
    UnitDie,
    UnitDamaged,
    UnitSelected,

    // Vehicle-specific animations
    TankTurretRotate,
    TankTreads,
    HelicopterRotor,
    HelicopterTilt,

    // Building animations
    BuildingConstruct,
    BuildingIdle,
    BuildingDamaged,
    BuildingDestroy,
    RadarDishRotate,
    ConstructionCrane,
    PowerPlantGlow,

    // Weapon animations
    WeaponFire,
    WeaponReload,
    WeaponRecoil,
    MuzzleFlash,
    ShellEject,

    // Effect animations
    ExplosionShockwave,
    SmokeRising,
    FireFlicker,
    WaterRipple,
    LightFlicker,
}

/// Animation interpolation types
#[derive(Debug, Clone, Copy)]
pub enum InterpolationType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    Elastic,
    BackEase,
    Constant, // No interpolation, instant change
}

/// Animation loop behavior
#[derive(Debug, Clone, Copy)]
pub enum LoopBehavior {
    Once,       // Play once and stop
    Loop,       // Loop continuously
    PingPong,   // Play forward then backward repeatedly
    ClampToEnd, // Play once and hold final value
}

/// Keyframe data for different property types
#[derive(Debug, Clone)]
pub enum KeyframeValue {
    Float(f32),
    Vec3(Vec3),
    Quaternion(Quat),
    Color(Vec3), // RGB color
    Bool(bool),
}

/// Individual animation keyframe
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f32, // Time in seconds
    pub value: KeyframeValue,
    pub interpolation: InterpolationType,
}

/// Animation track for a specific property
#[derive(Debug, Clone)]
pub struct AnimationTrack {
    pub property_name: String, // e.g., "position", "rotation", "scale", "turret_angle"
    pub keyframes: Vec<Keyframe>,
    pub default_value: KeyframeValue,
}

impl AnimationTrack {
    pub fn evaluate(&self, time: f32) -> KeyframeValue {
        if self.keyframes.is_empty() {
            return self.default_value.clone();
        }

        // Find surrounding keyframes
        if time <= self.keyframes[0].time {
            return self.keyframes[0].value.clone();
        }

        if time >= self.keyframes.last().unwrap().time {
            return self.keyframes.last().unwrap().value.clone();
        }

        // Find interpolation range
        for i in 0..self.keyframes.len() - 1 {
            let current = &self.keyframes[i];
            let next = &self.keyframes[i + 1];

            if time >= current.time && time <= next.time {
                let t = (time - current.time) / (next.time - current.time);
                return self.interpolate(&current.value, &next.value, t, next.interpolation);
            }
        }

        self.default_value.clone()
    }

    fn interpolate(
        &self,
        from: &KeyframeValue,
        to: &KeyframeValue,
        t: f32,
        interp_type: InterpolationType,
    ) -> KeyframeValue {
        let adjusted_t = match interp_type {
            InterpolationType::Linear => t,
            InterpolationType::EaseIn => t * t,
            InterpolationType::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            InterpolationType::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            InterpolationType::Bounce => {
                let bounce = |t: f32| {
                    if t < 1.0 / 2.75 {
                        7.5625 * t * t
                    } else if t < 2.0 / 2.75 {
                        let t = t - 1.5 / 2.75;
                        7.5625 * t * t + 0.75
                    } else if t < 2.5 / 2.75 {
                        let t = t - 2.25 / 2.75;
                        7.5625 * t * t + 0.9375
                    } else {
                        let t = t - 2.625 / 2.75;
                        7.5625 * t * t + 0.984375
                    }
                };
                bounce(t)
            }
            InterpolationType::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let p = 0.3;
                    let s = p / 4.0;
                    -(2.0f32.powf(10.0 * (t - 1.0)))
                        * ((t - 1.0 - s) * (2.0 * std::f32::consts::PI) / p).sin()
                }
            }
            InterpolationType::BackEase => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }
            InterpolationType::Constant => return from.clone(),
        };

        match (from, to) {
            (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
                KeyframeValue::Float(a + (b - a) * adjusted_t)
            }
            (KeyframeValue::Vec3(a), KeyframeValue::Vec3(b)) => {
                KeyframeValue::Vec3(a.lerp(*b, adjusted_t))
            }
            (KeyframeValue::Quaternion(a), KeyframeValue::Quaternion(b)) => {
                KeyframeValue::Quaternion(a.slerp(*b, adjusted_t))
            }
            (KeyframeValue::Color(a), KeyframeValue::Color(b)) => {
                KeyframeValue::Color(a.lerp(*b, adjusted_t))
            }
            (KeyframeValue::Bool(a), KeyframeValue::Bool(_)) => {
                // Boolean values don't interpolate, use threshold
                if adjusted_t < 0.5 {
                    KeyframeValue::Bool(*a)
                } else {
                    KeyframeValue::Bool(!*a)
                }
            }
            _ => from.clone(), // Mismatched types, return original
        }
    }
}

/// Complete animation definition
#[derive(Debug, Clone)]
pub struct Animation {
    pub name: String,
    pub animation_type: AnimationType,
    pub duration: f32, // Total duration in seconds
    pub tracks: Vec<AnimationTrack>,
    pub loop_behavior: LoopBehavior,
    pub priority: u32,       // Higher priority animations override lower ones
    pub blend_in_time: f32,  // Time to blend in when starting
    pub blend_out_time: f32, // Time to blend out when ending
    pub is_additive: bool,   // Whether this animation adds to base pose or replaces it
}

impl Animation {
    /// Create a new animation
    pub fn new(name: String, animation_type: AnimationType, duration: f32) -> Self {
        Self {
            name,
            animation_type,
            duration,
            tracks: Vec::new(),
            loop_behavior: LoopBehavior::Loop,
            priority: 0,
            blend_in_time: 0.2,
            blend_out_time: 0.2,
            is_additive: false,
        }
    }

    /// Add a track to this animation
    pub fn add_track(&mut self, track: AnimationTrack) {
        self.tracks.push(track);
    }

    /// Evaluate all tracks at a given time
    pub fn evaluate(&self, time: f32) -> HashMap<String, KeyframeValue> {
        let mut results = HashMap::new();

        // Apply loop behavior
        let wrapped_time = match self.loop_behavior {
            LoopBehavior::Once | LoopBehavior::ClampToEnd => time.min(self.duration),
            LoopBehavior::Loop => time % self.duration,
            LoopBehavior::PingPong => {
                let cycle_time = time % (self.duration * 2.0);
                if cycle_time <= self.duration {
                    cycle_time
                } else {
                    self.duration * 2.0 - cycle_time
                }
            }
        };

        for track in &self.tracks {
            let value = track.evaluate(wrapped_time);
            results.insert(track.property_name.clone(), value);
        }

        results
    }

    /// Check if animation is finished (for non-looping animations)
    pub fn is_finished(&self, time: f32) -> bool {
        match self.loop_behavior {
            LoopBehavior::Once => time >= self.duration,
            LoopBehavior::ClampToEnd => false, // Never truly "finished"
            LoopBehavior::Loop | LoopBehavior::PingPong => false, // Never finished
        }
    }
}

/// Animation state for a single animation instance
#[derive(Debug)]
pub struct AnimationState {
    pub animation: Arc<Animation>,
    pub current_time: f32,
    pub play_rate: f32,
    pub weight: f32,        // Blend weight (0.0 to 1.0)
    pub target_weight: f32, // Target weight for blending
    pub is_playing: bool,
    pub is_paused: bool,
    pub start_time: f32, // When this animation started (for sync)
}

impl AnimationState {
    pub fn new(animation: Arc<Animation>) -> Self {
        Self {
            animation,
            current_time: 0.0,
            play_rate: 1.0,
            weight: 1.0,
            target_weight: 1.0,
            is_playing: true,
            is_paused: false,
            start_time: 0.0,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        if !self.is_playing || self.is_paused {
            return;
        }

        // Update time
        self.current_time += delta_time * self.play_rate;

        // Update weight blending
        let blend_speed = 1.0 / self.animation.blend_in_time.max(0.1);
        if self.weight < self.target_weight {
            self.weight = (self.weight + blend_speed * delta_time).min(self.target_weight);
        } else if self.weight > self.target_weight {
            self.weight = (self.weight - blend_speed * delta_time).max(self.target_weight);
        }

        // Check if finished
        if self.animation.is_finished(self.current_time) {
            self.is_playing = false;
        }
    }

    pub fn reset(&mut self) {
        self.current_time = 0.0;
        self.is_playing = true;
        self.is_paused = false;
    }

    pub fn stop(&mut self) {
        self.target_weight = 0.0;
        // Will actually stop when weight reaches 0
    }
}

/// Animation instance for a specific object
#[derive(Debug)]
pub struct ObjectAnimator {
    pub object_id: ObjectId,
    pub active_animations: Vec<AnimationState>,
    pub base_transform: Mat4,
    pub current_values: HashMap<String, KeyframeValue>,
}

impl ObjectAnimator {
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            object_id,
            active_animations: Vec::new(),
            base_transform: Mat4::IDENTITY,
            current_values: HashMap::new(),
        }
    }

    pub fn play_animation(
        &mut self,
        animation: Arc<Animation>,
        replace_same_type: bool,
        start_time_seconds: f32,
    ) {
        // Remove existing animations of same type if requested
        if replace_same_type {
            self.active_animations
                .retain(|state| state.animation.animation_type != animation.animation_type);
        }

        let mut state = AnimationState::new(animation);
        state.start_time = start_time_seconds.max(0.0);
        self.active_animations.push(state);
    }

    pub fn stop_animation(&mut self, animation_type: AnimationType) {
        for state in &mut self.active_animations {
            if state.animation.animation_type == animation_type {
                state.stop();
            }
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update all animation states
        for state in &mut self.active_animations {
            state.update(delta_time);
        }

        // Remove finished animations with zero weight
        self.active_animations
            .retain(|state| state.is_playing || state.weight > 0.001);

        // Blend all animations
        self.blend_animations();
    }

    fn blend_animations(&mut self) {
        self.current_values.clear();

        // Sort by priority (lower priority first)
        self.active_animations
            .sort_by_key(|state| state.animation.priority);

        for state in &self.active_animations {
            if state.weight <= 0.001 {
                continue;
            }

            let animation_values = state.animation.evaluate(state.current_time);

            for (property, value) in animation_values {
                if state.animation.is_additive {
                    // Additive blending
                    if let Some(existing) = self.current_values.get(&property) {
                        let blended = self.add_values(existing, &value, state.weight);
                        self.current_values.insert(property, blended);
                    } else {
                        let scaled = self.scale_value(&value, state.weight);
                        self.current_values.insert(property, scaled);
                    }
                } else {
                    // Replace blending
                    if let Some(existing) = self.current_values.get(&property) {
                        let blended = self.blend_values(existing, &value, state.weight);
                        self.current_values.insert(property, blended);
                    } else {
                        self.current_values.insert(property, value);
                    }
                }
            }
        }
    }

    fn blend_values(&self, from: &KeyframeValue, to: &KeyframeValue, weight: f32) -> KeyframeValue {
        match (from, to) {
            (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
                KeyframeValue::Float(a + (b - a) * weight)
            }
            (KeyframeValue::Vec3(a), KeyframeValue::Vec3(b)) => {
                KeyframeValue::Vec3(a.lerp(*b, weight))
            }
            (KeyframeValue::Quaternion(a), KeyframeValue::Quaternion(b)) => {
                KeyframeValue::Quaternion(a.slerp(*b, weight))
            }
            (KeyframeValue::Color(a), KeyframeValue::Color(b)) => {
                KeyframeValue::Color(a.lerp(*b, weight))
            }
            (KeyframeValue::Bool(a), KeyframeValue::Bool(b)) => {
                if weight < 0.5 {
                    KeyframeValue::Bool(*a)
                } else {
                    KeyframeValue::Bool(*b)
                }
            }
            _ => to.clone(),
        }
    }

    fn add_values(
        &self,
        base: &KeyframeValue,
        additive: &KeyframeValue,
        weight: f32,
    ) -> KeyframeValue {
        match (base, additive) {
            (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
                KeyframeValue::Float(a + b * weight)
            }
            (KeyframeValue::Vec3(a), KeyframeValue::Vec3(b)) => {
                KeyframeValue::Vec3(*a + *b * weight)
            }
            (KeyframeValue::Quaternion(a), KeyframeValue::Quaternion(b)) => {
                let scaled_rotation =
                    Quat::from_axis_angle(b.xyz().normalize_or_zero(), b.w * weight);
                KeyframeValue::Quaternion(*a * scaled_rotation)
            }
            (KeyframeValue::Color(a), KeyframeValue::Color(b)) => {
                KeyframeValue::Color(*a + *b * weight)
            }
            _ => base.clone(),
        }
    }

    fn scale_value(&self, value: &KeyframeValue, scale: f32) -> KeyframeValue {
        match value {
            KeyframeValue::Float(v) => KeyframeValue::Float(v * scale),
            KeyframeValue::Vec3(v) => KeyframeValue::Vec3(*v * scale),
            KeyframeValue::Quaternion(q) => {
                let scaled_angle = q.w * scale;
                KeyframeValue::Quaternion(Quat::from_axis_angle(
                    q.xyz().normalize_or_zero(),
                    scaled_angle,
                ))
            }
            KeyframeValue::Color(c) => KeyframeValue::Color(*c * scale),
            KeyframeValue::Bool(b) => KeyframeValue::Bool(*b),
        }
    }

    pub fn get_transform(&self) -> Mat4 {
        let mut transform = self.base_transform;

        if let Some(KeyframeValue::Vec3(pos)) = self.current_values.get("position") {
            transform = Mat4::from_translation(*pos) * transform;
        }

        if let Some(KeyframeValue::Quaternion(rot)) = self.current_values.get("rotation") {
            transform = Mat4::from_quat(*rot) * transform;
        }

        if let Some(KeyframeValue::Vec3(scale)) = self.current_values.get("scale") {
            transform = Mat4::from_scale(*scale) * transform;
        }

        transform
    }

    pub fn get_property_value(&self, property: &str) -> Option<&KeyframeValue> {
        self.current_values.get(property)
    }
}

/// Animation manager for the entire game
pub struct AnimationManager {
    animations: HashMap<String, Arc<Animation>>,
    object_animators: HashMap<ObjectId, ObjectAnimator>,
    current_time_seconds: f32,
}

impl AnimationManager {
    pub fn new() -> Self {
        let mut manager = Self {
            animations: HashMap::new(),
            object_animators: HashMap::new(),
            current_time_seconds: 0.0,
        };

        manager.load_default_animations();
        manager
    }

    pub fn register_animation(&mut self, animation: Animation) {
        let name = animation.name.clone();
        self.animations.insert(name, Arc::new(animation));
    }

    pub fn play_animation(
        &mut self,
        object_id: ObjectId,
        animation_name: &str,
        replace_same_type: bool,
    ) {
        if let Some(animation) = self.animations.get(animation_name) {
            let animator = self
                .object_animators
                .entry(object_id)
                .or_insert_with(|| ObjectAnimator::new(object_id));

            animator.play_animation(
                animation.clone(),
                replace_same_type,
                self.current_time_seconds,
            );
            debug!(
                "Playing animation '{}' on object {:?}",
                animation_name, object_id
            );
        } else {
            warn!("Animation '{}' not found", animation_name);
        }
    }

    pub fn stop_animation(&mut self, object_id: ObjectId, animation_type: AnimationType) {
        if let Some(animator) = self.object_animators.get_mut(&object_id) {
            animator.stop_animation(animation_type);
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.current_time_seconds += delta_time.max(0.0);
        for animator in self.object_animators.values_mut() {
            animator.update(delta_time);
        }

        // Remove empty animators
        self.object_animators
            .retain(|_, animator| !animator.active_animations.is_empty());
    }

    pub fn get_object_transform(&self, object_id: ObjectId) -> Option<Mat4> {
        self.object_animators
            .get(&object_id)
            .map(|animator| animator.get_transform())
    }

    pub fn get_object_property(
        &self,
        object_id: ObjectId,
        property: &str,
    ) -> Option<&KeyframeValue> {
        self.object_animators
            .get(&object_id)?
            .get_property_value(property)
    }

    fn load_default_animations(&mut self) {
        // Tank turret rotation animation
        let mut tank_turret = Animation::new(
            "TankTurretRotate".to_string(),
            AnimationType::TankTurretRotate,
            2.0,
        );
        let turret_track = AnimationTrack {
            property_name: "turret_angle".to_string(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Float(0.0),
                    interpolation: InterpolationType::Linear,
                },
                Keyframe {
                    time: 2.0,
                    value: KeyframeValue::Float(std::f32::consts::PI * 2.0),
                    interpolation: InterpolationType::Linear,
                },
            ],
            default_value: KeyframeValue::Float(0.0),
        };
        tank_turret.add_track(turret_track);
        tank_turret.loop_behavior = LoopBehavior::Loop;
        tank_turret.priority = 10;
        self.register_animation(tank_turret);

        // Helicopter rotor animation
        let mut heli_rotor = Animation::new(
            "HelicopterRotor".to_string(),
            AnimationType::HelicopterRotor,
            0.1,
        );
        let rotor_track = AnimationTrack {
            property_name: "rotor_rotation".to_string(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Float(0.0),
                    interpolation: InterpolationType::Linear,
                },
                Keyframe {
                    time: 0.1,
                    value: KeyframeValue::Float(std::f32::consts::PI * 2.0),
                    interpolation: InterpolationType::Linear,
                },
            ],
            default_value: KeyframeValue::Float(0.0),
        };
        heli_rotor.add_track(rotor_track);
        heli_rotor.loop_behavior = LoopBehavior::Loop;
        heli_rotor.priority = 15;
        self.register_animation(heli_rotor);

        // Muzzle flash animation
        let mut muzzle_flash =
            Animation::new("MuzzleFlash".to_string(), AnimationType::MuzzleFlash, 0.2);
        let flash_scale_track = AnimationTrack {
            property_name: "muzzle_scale".to_string(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Vec3(Vec3::ZERO),
                    interpolation: InterpolationType::EaseOut,
                },
                Keyframe {
                    time: 0.05,
                    value: KeyframeValue::Vec3(Vec3::ONE * 2.0),
                    interpolation: InterpolationType::EaseIn,
                },
                Keyframe {
                    time: 0.2,
                    value: KeyframeValue::Vec3(Vec3::ZERO),
                    interpolation: InterpolationType::EaseIn,
                },
            ],
            default_value: KeyframeValue::Vec3(Vec3::ZERO),
        };
        let flash_color_track = AnimationTrack {
            property_name: "muzzle_color".to_string(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Color(Vec3::new(1.0, 1.0, 0.8)),
                    interpolation: InterpolationType::Linear,
                },
                Keyframe {
                    time: 0.1,
                    value: KeyframeValue::Color(Vec3::new(1.0, 0.5, 0.0)),
                    interpolation: InterpolationType::Linear,
                },
                Keyframe {
                    time: 0.2,
                    value: KeyframeValue::Color(Vec3::new(0.3, 0.0, 0.0)),
                    interpolation: InterpolationType::Linear,
                },
            ],
            default_value: KeyframeValue::Color(Vec3::ZERO),
        };
        muzzle_flash.add_track(flash_scale_track);
        muzzle_flash.add_track(flash_color_track);
        muzzle_flash.loop_behavior = LoopBehavior::Once;
        muzzle_flash.priority = 20;
        self.register_animation(muzzle_flash);

        // Building construction animation
        let mut construction = Animation::new(
            "BuildingConstruct".to_string(),
            AnimationType::BuildingConstruct,
            5.0,
        );
        let construct_scale_track = AnimationTrack {
            property_name: "construction_progress".to_string(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Float(0.0),
                    interpolation: InterpolationType::EaseOut,
                },
                Keyframe {
                    time: 5.0,
                    value: KeyframeValue::Float(1.0),
                    interpolation: InterpolationType::EaseOut,
                },
            ],
            default_value: KeyframeValue::Float(0.0),
        };
        construction.add_track(construct_scale_track);
        construction.loop_behavior = LoopBehavior::ClampToEnd;
        construction.priority = 5;
        self.register_animation(construction);

        info!("Loaded {} default animations", self.animations.len());
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}
