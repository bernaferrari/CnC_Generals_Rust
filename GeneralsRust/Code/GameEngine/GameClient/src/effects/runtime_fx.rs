//! # Runtime FX System
//!
//! Core real-time visual effect types used by the game client renderer.
//! These are the runtime representations that the FXList template system
//! (INI-driven FXNuggets) creates and manages at runtime.
//!
//! ## Types
//!
//! - [`FXPoint`] — Simple billboard particle (position, color, size, lifetime).
//!   Used for muzzle flashes, impact sparks, small explosions.
//!
//! - [`FXLine`] — Line effect between two 3D points. Used for laser beams,
//!   projectile trails, energy discharges.
//!
//! - [`FXRing`] — Expanding ring effect. Used for shockwave rings,
//!   selection indicators, area-of-effect visuals.
//!
//! - [`FXShader`] — Shader-driven effect with animated custom parameters.
//!   Used for custom visual effects that don't fit the other categories.
//!
//! - [`FXList`] — Container managing all active effects. Provides `update()`,
//!   `draw()`, and factory methods to create and track effects.
//!
//! ## C++ Parity Notes
//!
//! In the C++ engine, these effect types were rendered through the W3D
//! draw-module pipeline (TracerDrawModule, ParticleSys, etc.). The Rust
//! port uses WGPU, so we provide a unified runtime representation that
//! the render bridge can consume directly.
//!
//! PARITY_NOTE: The FXList here is a *runtime container* of live effect
//! instances, distinct from the INI-parsed `fx_list::FXList` which holds
//! template `FXNugget`s. The two are related: when an INI FXList fires,
//! its nuggets create runtime FX instances that live in this container.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use game_engine::common::global_data;
use nalgebra::{Point3, Vector3};

// ---------------------------------------------------------------------------
// Unique IDs
// ---------------------------------------------------------------------------

/// Unique identifier for a live runtime effect.
pub type FXId = u64;

/// Global counter for generating unique IDs.
static FX_NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_fx_id() -> FXId {
    FX_NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Shared FX data
// ---------------------------------------------------------------------------

/// Common fields shared by all runtime FX types.
#[derive(Debug, Clone)]
pub struct FXBase {
    /// Unique ID assigned at creation time.
    pub id: FXId,
    /// World-space position of the effect origin.
    pub position: Point3<f32>,
    /// RGBA color.
    pub color: [f32; 4],
    /// Current opacity (0.0–1.0), animated over lifetime.
    pub alpha: f32,
    /// Total lifetime in seconds. 0.0 means persistent (never expires).
    pub lifetime_secs: f32,
    /// Elapsed time since creation.
    pub age_secs: f32,
    /// Whether this effect is still alive.
    pub active: bool,
}

impl FXBase {
    /// Create a new base with the given position and lifetime.
    fn new(position: Point3<f32>, color: [f32; 4], lifetime_secs: f32) -> Self {
        Self {
            id: next_fx_id(),
            position,
            color,
            alpha: color[3],
            lifetime_secs,
            age_secs: 0.0,
            active: true,
        }
    }

    /// Returns the fraction of lifetime elapsed (0.0–1.0).
    /// For persistent effects (lifetime 0.0), always returns 0.0.
    fn life_fraction(&self) -> f32 {
        if self.lifetime_secs <= 0.0 {
            return 0.0;
        }
        (self.age_secs / self.lifetime_secs).clamp(0.0, 1.0)
    }

    /// Advance the base state by `dt` seconds.
    fn tick(&mut self, dt: f32) {
        if !self.active {
            return;
        }
        self.age_secs += dt;
        if self.lifetime_secs > 0.0 && self.age_secs >= self.lifetime_secs {
            self.active = false;
        }
    }
}

// ---------------------------------------------------------------------------
// FXPoint — simple billboard particle
// ---------------------------------------------------------------------------

/// A simple point/billboard particle effect.
///
/// Update behavior (matches C++ single-particle tracer/spark logic):
/// - Move along `velocity` each frame.
/// - Shrink `size` linearly from `start_size` to 0 over lifetime.
/// - Fade `alpha` from initial value to 0 over lifetime.
///
/// Used for: muzzle flashes, impact sparks, small explosion particles.
#[derive(Debug, Clone)]
pub struct FXPoint {
    pub base: FXBase,
    /// Current velocity (world units / second).
    pub velocity: Vector3<f32>,
    /// Current rendered size (world units).
    pub size: f32,
    /// Size at creation.
    pub start_size: f32,
    /// Gravity acceleration applied each frame (world units / sec²).
    pub gravity: f32,
    /// Drag factor (0.0 = no drag, 1.0 = full stop instantly).
    pub drag: f32,
}

impl FXPoint {
    /// Create a new point effect.
    pub fn new(
        position: Point3<f32>,
        color: [f32; 4],
        velocity: Vector3<f32>,
        size: f32,
        lifetime_secs: f32,
    ) -> Self {
        let base = FXBase::new(position, color, lifetime_secs);
        Self {
            base,
            velocity,
            size,
            start_size: size,
            gravity: global_data::read_safe()
                .map(|data| data.gravity)
                .unwrap_or(-1.0),
            drag: 0.0,
        }
    }

    /// Create with explicit gravity and drag.
    pub fn with_physics(
        position: Point3<f32>,
        color: [f32; 4],
        velocity: Vector3<f32>,
        size: f32,
        lifetime_secs: f32,
        gravity: f32,
        drag: f32,
    ) -> Self {
        let base = FXBase::new(position, color, lifetime_secs);
        Self {
            base,
            velocity,
            size,
            start_size: size,
            gravity,
            drag,
        }
    }

    /// Update the point effect by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.base.tick(dt);
        if !self.base.active {
            return;
        }

        // Apply gravity
        self.velocity.z += self.gravity * dt;

        // Apply drag
        if self.drag > 0.0 {
            let drag_factor = 1.0 - self.drag * dt;
            self.velocity *= drag_factor.max(0.0);
        }

        // Move
        self.base.position += self.velocity * dt;

        // Shrink and fade over lifetime
        let t = self.base.life_fraction();
        self.size = self.start_size * (1.0 - t);
        self.base.alpha = self.base.color[3] * (1.0 - t);
    }

    /// Check if the effect is still alive.
    pub fn is_alive(&self) -> bool {
        self.base.active
    }
}

// ---------------------------------------------------------------------------
// FXLine — line between two points
// ---------------------------------------------------------------------------

/// A line effect drawn between two 3D points.
///
/// Update behavior:
/// - Extend/shrink the line over its lifetime (`progress` 0→1).
/// - Fade alpha over lifetime.
/// - Optionally track moving endpoints via `velocity_start` / `velocity_end`.
///
/// Used for: laser beams, projectile trails, energy discharges.
#[derive(Debug, Clone)]
pub struct FXLine {
    pub base: FXBase,
    /// Line start point.
    pub start: Point3<f32>,
    /// Line end point.
    pub end: Point3<f32>,
    /// Original start (for progress-based extension).
    pub start_origin: Point3<f32>,
    /// Original end (for progress-based extension).
    pub end_origin: Point3<f32>,
    /// Line width in world units.
    pub width: f32,
    /// How far along the line has been drawn (0.0–1.0).
    pub progress: f32,
    /// Velocity applied to the start point each frame.
    pub velocity_start: Vector3<f32>,
    /// Velocity applied to the end point each frame.
    pub velocity_end: Vector3<f32>,
}

impl FXLine {
    /// Create a new line effect from `start` to `end`.
    pub fn new(
        start: Point3<f32>,
        end: Point3<f32>,
        color: [f32; 4],
        width: f32,
        lifetime_secs: f32,
    ) -> Self {
        let mid = start + (end - start) * 0.5;
        let base = FXBase::new(mid, color, lifetime_secs);
        Self {
            base,
            start,
            end,
            start_origin: start,
            end_origin: end,
            width,
            progress: 0.0,
            velocity_start: Vector3::zeros(),
            velocity_end: Vector3::zeros(),
        }
    }

    /// Create a line that extends from start to end over its lifetime.
    pub fn extending(
        start: Point3<f32>,
        end: Point3<f32>,
        color: [f32; 4],
        width: f32,
        lifetime_secs: f32,
    ) -> Self {
        // Start with zero-length line at start position
        let mut fx = Self::new(start, start, color, width, lifetime_secs);
        fx.start_origin = start;
        fx.end_origin = end;
        fx
    }

    /// Set endpoint velocities for tracking moving targets.
    pub fn with_velocities(mut self, vel_start: Vector3<f32>, vel_end: Vector3<f32>) -> Self {
        self.velocity_start = vel_start;
        self.velocity_end = vel_end;
        self
    }

    /// Update the line effect by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.base.tick(dt);
        if !self.base.active {
            return;
        }

        let t = self.base.life_fraction();

        // Move endpoints if velocities are set
        if self.velocity_start.norm() > 1e-6 {
            self.start_origin += self.velocity_start * dt;
        }
        if self.velocity_end.norm() > 1e-6 {
            self.end_origin += self.velocity_end * dt;
        }

        // Extend progress from 0→1 over lifetime
        self.progress = t.clamp(0.0, 1.0);

        // Interpolate the visible end point
        let full_end = self.end_origin;
        self.end = self.start_origin + (full_end - self.start_origin) * self.progress;
        self.start = self.start_origin;

        // Fade alpha over lifetime
        self.base.alpha = self.base.color[3] * (1.0 - t);

        // Update position to midpoint for LOD calculations
        self.base.position = self.start + (self.end - self.start) * 0.5;
    }

    /// Check if the effect is still alive.
    pub fn is_alive(&self) -> bool {
        self.base.active
    }

    /// Current line length.
    pub fn length(&self) -> f32 {
        (self.end - self.start).norm()
    }
}

// ---------------------------------------------------------------------------
// FXRing — expanding ring
// ---------------------------------------------------------------------------

/// An expanding ring effect (circle on the XZ plane by default).
///
/// Update behavior:
/// - Grow `radius` from `start_radius` to `end_radius` over lifetime.
/// - Fade `alpha` from initial to 0.
///
/// Used for: shockwave rings, selection indicators, area-of-effect visuals.
#[derive(Debug, Clone)]
pub struct FXRing {
    pub base: FXBase,
    /// Current ring radius.
    pub radius: f32,
    /// Radius at creation.
    pub start_radius: f32,
    /// Target radius at end of lifetime.
    pub end_radius: f32,
    /// Ring thickness (world units).
    pub thickness: f32,
    /// Normal vector defining the ring plane (default: +Z).
    pub normal: Vector3<f32>,
}

impl FXRing {
    /// Create a new expanding ring effect.
    pub fn new(
        position: Point3<f32>,
        color: [f32; 4],
        start_radius: f32,
        end_radius: f32,
        thickness: f32,
        lifetime_secs: f32,
    ) -> Self {
        let base = FXBase::new(position, color, lifetime_secs);
        Self {
            base,
            radius: start_radius,
            start_radius,
            end_radius,
            thickness,
            normal: Vector3::new(0.0, 0.0, 1.0),
        }
    }

    /// Create with a custom plane normal.
    pub fn with_normal(mut self, normal: Vector3<f32>) -> Self {
        self.normal = normal;
        self
    }

    /// Update the ring effect by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.base.tick(dt);
        if !self.base.active {
            return;
        }

        let t = self.base.life_fraction();

        // Expand radius
        self.radius = self.start_radius + (self.end_radius - self.start_radius) * t;

        // Fade alpha over lifetime
        self.base.alpha = self.base.color[3] * (1.0 - t);
    }

    /// Check if the effect is still alive.
    pub fn is_alive(&self) -> bool {
        self.base.active
    }
}

// ---------------------------------------------------------------------------
// FXShader — shader-driven effect
// ---------------------------------------------------------------------------

/// A shader-driven effect with custom animated parameters.
///
/// Unlike the other FX types which have fixed update logic, FXShader
/// delegates visual behavior to a named shader and animates its parameters
/// over time using simple interpolation or wave functions.
///
/// Used for: custom visual effects that don't fit point/line/ring categories
/// (e.g., screen-space overlays, distortion fields, pulsing glows).
#[derive(Debug, Clone)]
pub struct FXShader {
    pub base: FXBase,
    /// Name of the shader to use (matched against shader registry).
    pub shader_name: String,
    /// Custom parameters keyed by name. Values are animated over lifetime.
    pub params: HashMap<String, FXShaderParam>,
}

/// A single shader parameter with animation settings.
#[derive(Debug, Clone)]
pub struct FXShaderParam {
    /// Current value (float — shader params are typically float or vec).
    pub value: f32,
    /// Value at start of lifetime.
    pub start_value: f32,
    /// Value at end of lifetime.
    pub end_value: f32,
    /// Animation mode.
    pub mode: FXShaderParamMode,
}

/// How a shader parameter is animated over the effect lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FXShaderParamMode {
    /// Linear interpolation from start to end.
    Linear,
    /// Sine-wave oscillation between start and end.
    SineWave,
    /// Constant value (no animation).
    Constant,
}

impl FXShaderParam {
    /// Create a new parameter.
    pub fn new(start_value: f32, end_value: f32, mode: FXShaderParamMode) -> Self {
        Self {
            value: start_value,
            start_value,
            end_value,
            mode,
        }
    }

    /// Create a constant parameter.
    pub fn constant(value: f32) -> Self {
        Self {
            value,
            start_value: value,
            end_value: value,
            mode: FXShaderParamMode::Constant,
        }
    }

    /// Update the parameter value based on lifetime fraction `t` (0.0–1.0).
    pub fn update(&mut self, t: f32) {
        match self.mode {
            FXShaderParamMode::Linear => {
                self.value = self.start_value + (self.end_value - self.start_value) * t;
            }
            FXShaderParamMode::SineWave => {
                let mid = (self.start_value + self.end_value) * 0.5;
                let amp = (self.end_value - self.start_value) * 0.5;
                self.value = mid + amp * (t * std::f32::consts::PI * 2.0).sin();
            }
            FXShaderParamMode::Constant => {
                // No change
            }
        }
    }
}

impl FXShader {
    /// Create a new shader effect.
    pub fn new(
        position: Point3<f32>,
        color: [f32; 4],
        shader_name: String,
        lifetime_secs: f32,
    ) -> Self {
        let base = FXBase::new(position, color, lifetime_secs);
        Self {
            base,
            shader_name,
            params: HashMap::new(),
        }
    }

    /// Add a parameter to the shader effect.
    pub fn with_param(mut self, name: &str, param: FXShaderParam) -> Self {
        self.params.insert(name.to_string(), param);
        self
    }

    /// Set a parameter value by name (for dynamic updates from game logic).
    pub fn set_param(&mut self, name: &str, value: f32) {
        if let Some(param) = self.params.get_mut(name) {
            param.value = value;
        }
    }

    /// Get a parameter value by name.
    pub fn get_param(&self, name: &str) -> Option<f32> {
        self.params.get(name).map(|p| p.value)
    }

    /// Update the shader effect by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.base.tick(dt);
        if !self.base.active {
            return;
        }

        let t = self.base.life_fraction();

        // Animate all parameters
        for param in self.params.values_mut() {
            param.update(t);
        }

        // Fade alpha
        self.base.alpha = self.base.color[3] * (1.0 - t);
    }

    /// Check if the effect is still alive.
    pub fn is_alive(&self) -> bool {
        self.base.active
    }
}

// ---------------------------------------------------------------------------
// FXHandle — tagged union of all effect types
// ---------------------------------------------------------------------------

/// Runtime representation of any live effect. Used by [`FXList`] to store
/// heterogeneous effects in a single collection.
#[derive(Debug)]
pub enum FXHandle {
    Point(FXPoint),
    Line(FXLine),
    Ring(FXRing),
    Shader(FXShader),
}

impl FXHandle {
    /// Get the shared base fields.
    pub fn base(&self) -> &FXBase {
        match self {
            FXHandle::Point(fx) => &fx.base,
            FXHandle::Line(fx) => &fx.base,
            FXHandle::Ring(fx) => &fx.base,
            FXHandle::Shader(fx) => &fx.base,
        }
    }

    /// Update the effect by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        match self {
            FXHandle::Point(fx) => fx.update(dt),
            FXHandle::Line(fx) => fx.update(dt),
            FXHandle::Ring(fx) => fx.update(dt),
            FXHandle::Shader(fx) => fx.update(dt),
        }
    }

    /// Check if the effect is still alive.
    pub fn is_alive(&self) -> bool {
        match self {
            FXHandle::Point(fx) => fx.is_alive(),
            FXHandle::Line(fx) => fx.is_alive(),
            FXHandle::Ring(fx) => fx.is_alive(),
            FXHandle::Shader(fx) => fx.is_alive(),
        }
    }

    /// Get the effect's unique ID.
    pub fn id(&self) -> FXId {
        self.base().id
    }

    /// Get the effect's world position.
    pub fn position(&self) -> Point3<f32> {
        self.base().position
    }
}

// ---------------------------------------------------------------------------
// FXList — runtime container for all active effects
// ---------------------------------------------------------------------------

/// Container for all active runtime effects. Provides `update()` and
/// `draw()` integration, plus factory methods for creating effects.
///
/// PARITY_NOTE: In C++, there is no single "runtime FXList" container.
/// The C++ engine managed live effects through separate systems:
/// - Particles: `ParticleSystemManager`
/// - Tracers: `ThingFactory` created `Drawable` tracers
/// - Ray effects: `TheGameClient->createRayEffectByTemplate()`
/// - Light pulses: `TheDisplay->createLightPulse()`
/// This Rust FXList consolidates lightweight runtime effects into one
/// update/draw pass for WGPU efficiency.
#[derive(Debug)]
pub struct FXList {
    /// All active effects indexed by ID.
    effects: HashMap<FXId, FXHandle>,
    /// Global time accumulator (seconds).
    time: f32,
    /// Maximum number of concurrent effects (0 = unlimited).
    max_effects: usize,
}

impl FXList {
    /// Create a new empty FXList.
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            time: 0.0,
            max_effects: 0,
        }
    }

    /// Create with a maximum effect count.
    pub fn with_capacity(max_effects: usize) -> Self {
        Self {
            effects: HashMap::new(),
            time: 0.0,
            max_effects,
        }
    }

    // ---- Factory methods ----

    /// Add a point effect and return its ID.
    pub fn add_point(&mut self, point: FXPoint) -> FXId {
        let id = point.base.id;
        self.effects.insert(id, FXHandle::Point(point));
        id
    }

    /// Add a line effect and return its ID.
    pub fn add_line(&mut self, line: FXLine) -> FXId {
        let id = line.base.id;
        self.effects.insert(id, FXHandle::Line(line));
        id
    }

    /// Add a ring effect and return its ID.
    pub fn add_ring(&mut self, ring: FXRing) -> FXId {
        let id = ring.base.id;
        self.effects.insert(id, FXHandle::Ring(ring));
        id
    }

    /// Add a shader effect and return its ID.
    pub fn add_shader(&mut self, shader: FXShader) -> FXId {
        let id = shader.base.id;
        self.effects.insert(id, FXHandle::Shader(shader));
        id
    }

    /// Add a generic FXHandle and return its ID.
    pub fn add_fx(&mut self, handle: FXHandle) -> FXId {
        let id = handle.id();
        self.effects.insert(id, handle);
        id
    }

    // ---- Update / draw ----

    /// Update all effects by `dt` seconds. Removes dead effects.
    /// Matches C++ per-frame update pattern in `GameClient::update()`.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;

        // Update all effects
        for effect in self.effects.values_mut() {
            effect.update(dt);
        }

        // Remove dead effects
        self.effects.retain(|_, effect| effect.is_alive());
    }

    /// Get the current global time (seconds since creation).
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Get number of active effects.
    pub fn count(&self) -> usize {
        self.effects.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Remove a specific effect by ID.
    pub fn remove(&mut self, id: FXId) -> bool {
        self.effects.remove(&id).is_some()
    }

    /// Get a reference to an effect by ID.
    pub fn get(&self, id: FXId) -> Option<&FXHandle> {
        self.effects.get(&id)
    }

    /// Get a mutable reference to an effect by ID.
    pub fn get_mut(&mut self, id: FXId) -> Option<&mut FXHandle> {
        self.effects.get_mut(&id)
    }

    /// Clear all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Iterate over all active effects (read-only).
    pub fn iter(&self) -> impl Iterator<Item = &FXHandle> {
        self.effects.values()
    }

    /// Iterate mutably over all active effects.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut FXHandle> {
        self.effects.values_mut()
    }

    /// Collect render data for all active point effects.
    /// Returns vertex-like data suitable for GPU upload.
    pub fn collect_point_render_data(&self) -> Vec<FXPointRenderData> {
        self.effects
            .values()
            .filter_map(|handle| {
                if let FXHandle::Point(point) = handle {
                    Some(FXPointRenderData::from_point(point))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect render data for all active line effects.
    pub fn collect_line_render_data(&self) -> Vec<FXLineRenderData> {
        self.effects
            .values()
            .filter_map(|handle| {
                if let FXHandle::Line(line) = handle {
                    Some(FXLineRenderData::from_line(line))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect render data for all active ring effects.
    pub fn collect_ring_render_data(&self) -> Vec<FXRingRenderData> {
        self.effects
            .values()
            .filter_map(|handle| {
                if let FXHandle::Ring(ring) = handle {
                    Some(FXRingRenderData::from_ring(ring))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect render data for all active shader effects.
    pub fn collect_shader_render_data(&self) -> Vec<FXShaderRenderData> {
        self.effects
            .values()
            .filter_map(|handle| {
                if let FXHandle::Shader(shader) = handle {
                    Some(FXShaderRenderData::from_shader(shader))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for FXList {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GPU render data structs
// ---------------------------------------------------------------------------

/// GPU-ready render data for a point effect.
/// Layout is compatible with the existing particle vertex format for
/// WGPU instanced rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FXPointRenderData {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub alpha: f32,
    pub _padding: [f32; 3],
}

impl FXPointRenderData {
    fn from_point(point: &FXPoint) -> Self {
        Self {
            position: [
                point.base.position.x,
                point.base.position.y,
                point.base.position.z,
            ],
            size: point.size,
            color: point.base.color,
            alpha: point.base.alpha,
            _padding: [0.0; 3],
        }
    }
}

/// GPU-ready render data for a line effect.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FXLineRenderData {
    pub start: [f32; 3],
    pub width: f32,
    pub end: [f32; 3],
    pub alpha: f32,
    pub color: [f32; 4],
    pub _padding: f32,
}

impl FXLineRenderData {
    fn from_line(line: &FXLine) -> Self {
        Self {
            start: [line.start.x, line.start.y, line.start.z],
            width: line.width,
            end: [line.end.x, line.end.y, line.end.z],
            alpha: line.base.alpha,
            color: line.base.color,
            _padding: 0.0,
        }
    }
}

/// GPU-ready render data for a ring effect.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FXRingRenderData {
    pub center: [f32; 3],
    pub radius: f32,
    pub normal: [f32; 3],
    pub thickness: f32,
    pub color: [f32; 4],
}

impl FXRingRenderData {
    fn from_ring(ring: &FXRing) -> Self {
        Self {
            center: [
                ring.base.position.x,
                ring.base.position.y,
                ring.base.position.z,
            ],
            radius: ring.radius,
            normal: [ring.normal.x, ring.normal.y, ring.normal.z],
            thickness: ring.thickness,
            color: [
                ring.base.color[0],
                ring.base.color[1],
                ring.base.color[2],
                ring.base.alpha,
            ],
        }
    }
}

/// GPU-ready render data for a shader effect.
/// The actual shader name and parameter lookup happen on the CPU side;
/// this struct carries the position, color, and a flat array of up to
/// 8 parameter values for the GPU uniform buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FXShaderRenderData {
    pub position: [f32; 3],
    pub param_count: u32,
    pub color: [f32; 4],
    pub params: [f32; 8],
}

impl FXShaderRenderData {
    fn from_shader(shader: &FXShader) -> Self {
        let mut params = [0.0f32; 8];
        let mut param_count = 0u32;
        for (i, (_, p)) in shader.params.iter().enumerate() {
            if i >= 8 {
                break;
            }
            params[i] = p.value;
            param_count += 1;
        }
        Self {
            position: [
                shader.base.position.x,
                shader.base.position.y,
                shader.base.position.z,
            ],
            param_count,
            color: [
                shader.base.color[0],
                shader.base.color[1],
                shader.base.color[2],
                shader.base.alpha,
            ],
            params,
        }
    }
}

// ---------------------------------------------------------------------------
// FX creation hooks — convenience methods for game logic
// ---------------------------------------------------------------------------

/// Convenience hooks for creating common FX from game logic.
///
/// PARITY_NOTE: In C++, these effects were created by FXNugget implementations
/// (e.g., `TracerFXNugget::doFXPos`, `ParticleSystemFXNugget::reallyDoFX`).
/// These hooks provide the same entry points for the Rust game logic.
pub struct FXFactory;

impl FXFactory {
    /// Create a muzzle flash point effect.
    pub fn muzzle_flash(position: Point3<f32>, direction: Vector3<f32>) -> FXPoint {
        let speed = 2.0;
        let velocity = direction.normalize() * speed;
        FXPoint::with_physics(
            position,
            [1.0, 0.9, 0.5, 1.0], // Warm yellow
            velocity,
            3.0,  // size
            0.15, // 150ms lifetime
            0.0,  // no gravity
            5.0,  // high drag for quick stop
        )
    }

    /// Create an impact spark effect.
    pub fn impact_spark(position: Point3<f32>, normal: Vector3<f32>) -> FXPoint {
        let spread = 5.0;
        let up = Vector3::new(0.0, 0.0, 1.0);
        let rand_dir = if normal.norm() > 0.01 {
            normal.normalize()
        } else {
            up
        };
        // Scatter sparks around the normal
        let angle = rand::random::<f32>() * std::f32::consts::TAU;
        let scatter = Vector3::new(angle.cos(), angle.sin(), 0.5);
        let velocity = (rand_dir + scatter * 0.5).normalize() * spread;

        FXPoint::with_physics(
            position,
            [1.0, 0.7, 0.3, 1.0], // Orange spark
            velocity,
            1.5,
            0.3 + rand::random::<f32>() * 0.2, // 300-500ms
            -15.0,                             // heavy gravity
            2.0,
        )
    }

    /// Create an explosion flash (bright point that fades quickly).
    pub fn explosion_flash(position: Point3<f32>, size: f32) -> FXPoint {
        FXPoint::new(
            position,
            [1.0, 1.0, 0.8, 1.0], // Bright white-yellow
            Vector3::zeros(),     // Stationary
            size,
            0.2, // 200ms flash
        )
    }

    /// Create a laser beam line effect.
    pub fn laser_beam(start: Point3<f32>, end: Point3<f32>) -> FXLine {
        FXLine::new(
            start,
            end,
            [1.0, 0.1, 0.1, 0.9], // Red laser
            0.15,
            0.3, // 300ms
        )
    }

    /// Create a projectile trail (extending line).
    pub fn projectile_trail(start: Point3<f32>, end: Point3<f32>, color: [f32; 4]) -> FXLine {
        FXLine::extending(
            start, end, color, 0.08, // thin trail
            0.5,  // 500ms
        )
    }

    /// Create a shockwave ring effect.
    pub fn shockwave_ring(center: Point3<f32>, max_radius: f32) -> FXRing {
        FXRing::new(
            center,
            [0.8, 0.8, 0.8, 0.7], // White-grey
            1.0,                  // start small
            max_radius,
            2.0, // thickness
            0.6, // 600ms expansion
        )
    }

    /// Create a selection ring indicator.
    pub fn selection_ring(center: Point3<f32>, radius: f32, color: [f32; 4]) -> FXRing {
        FXRing::new(
            center, color, radius, radius, // No expansion
            0.3,    // Thin ring
            0.0,    // Persistent
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_point_update() {
        let mut point = FXPoint::new(
            Point3::new(0.0, 0.0, 0.0),
            [1.0, 0.0, 0.0, 1.0],
            Vector3::new(1.0, 0.0, 0.0),
            5.0,
            1.0,
        );

        assert!(point.is_alive());
        assert!((point.size - 5.0).abs() < 0.01);

        point.update(0.5);
        assert!(point.is_alive());
        assert!(point.base.position.x > 0.0); // Moved
        assert!(point.size < 5.0); // Shrunk
        assert!(point.base.alpha < 1.0); // Faded

        point.update(0.6); // Total: 1.1s > 1.0s lifetime
        assert!(!point.is_alive());
    }

    #[test]
    fn fx_point_default_gravity_tracks_global_data_like_cpp() {
        let old_gravity = {
            let mut global = global_data::write();
            let old = global.gravity;
            global.gravity = -2.5;
            old
        };

        let point = FXPoint::new(
            Point3::new(0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0, 1.0],
            Vector3::zeros(),
            1.0,
            1.0,
        );

        global_data::write().gravity = old_gravity;

        assert_eq!(point.gravity, -2.5);
    }

    #[test]
    fn test_fx_line_update() {
        let start = Point3::new(0.0, 0.0, 0.0);
        let end = Point3::new(10.0, 0.0, 0.0);
        let mut line = FXLine::extending(start, end, [1.0, 1.0, 0.0, 1.0], 0.2, 1.0);

        assert!(line.is_alive());
        assert!((line.progress - 0.0).abs() < 0.01);

        line.update(0.5);
        assert!(line.progress > 0.0);
        assert!(line.length() < 10.0); // Not fully extended yet

        line.update(0.6);
        assert!(!line.is_alive());
    }

    #[test]
    fn test_fx_ring_update() {
        let mut ring = FXRing::new(
            Point3::new(0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0, 0.8],
            1.0,
            20.0,
            2.0,
            1.0,
        );

        assert!(ring.is_alive());
        assert!((ring.radius - 1.0).abs() < 0.01);

        ring.update(0.5);
        assert!(ring.radius > 1.0); // Expanded
        assert!(ring.radius < 20.0); // Not fully expanded
        assert!(ring.base.alpha < 0.8); // Faded
    }

    #[test]
    fn test_fx_shader_update() {
        let mut shader = FXShader::new(
            Point3::new(5.0, 5.0, 5.0),
            [0.5, 0.5, 1.0, 1.0],
            "pulse_glow".to_string(),
            2.0,
        )
        .with_param(
            "intensity",
            FXShaderParam::new(0.0, 1.0, FXShaderParamMode::Linear),
        )
        .with_param("frequency", FXShaderParam::constant(60.0));

        assert!(shader.is_alive());
        assert!((shader.get_param("intensity").unwrap() - 0.0).abs() < 0.01);

        shader.update(1.0); // Halfway through lifetime
        assert!((shader.get_param("intensity").unwrap() - 0.5).abs() < 0.01);
        assert!((shader.get_param("frequency").unwrap() - 60.0).abs() < 0.01); // Constant unchanged

        shader.update(1.5); // Past lifetime
        assert!(!shader.is_alive());
    }

    #[test]
    fn test_fx_list_update_and_cleanup() {
        let mut list = FXList::new();

        let p1 = FXPoint::new(Point3::origin(), [1.0; 4], Vector3::zeros(), 1.0, 0.1);
        let p2 = FXPoint::new(Point3::origin(), [1.0; 4], Vector3::zeros(), 1.0, 2.0);
        let id1 = list.add_point(p1);
        let id2 = list.add_point(p2);

        assert_eq!(list.count(), 2);

        // Update past p1's lifetime
        list.update(0.15);
        assert_eq!(list.count(), 1); // p1 removed
        assert!(list.get(id2).is_some());
        assert!(list.get(id1).is_none());
    }

    #[test]
    fn test_fx_list_render_data_collection() {
        let mut list = FXList::new();

        list.add_point(FXPoint::new(
            Point3::new(1.0, 2.0, 3.0),
            [1.0, 0.0, 0.0, 1.0],
            Vector3::new(1.0, 0.0, 0.0),
            4.0,
            1.0,
        ));
        list.add_line(FXLine::new(
            Point3::origin(),
            Point3::new(10.0, 0.0, 0.0),
            [0.0, 1.0, 0.0, 1.0],
            0.5,
            1.0,
        ));
        list.add_ring(FXRing::new(
            Point3::new(5.0, 5.0, 0.0),
            [0.0, 0.0, 1.0, 1.0],
            1.0,
            10.0,
            1.0,
            1.0,
        ));

        let points = list.collect_point_render_data();
        let lines = list.collect_line_render_data();
        let rings = list.collect_ring_render_data();

        assert_eq!(points.len(), 1);
        assert_eq!(lines.len(), 1);
        assert_eq!(rings.len(), 1);

        assert!((points[0].position[0] - 1.0).abs() < 0.01);
        assert!((lines[0].start[0] - 0.0).abs() < 0.01);
        assert!((rings[0].center[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_fx_factory_muzzle_flash() {
        let flash =
            FXFactory::muzzle_flash(Point3::new(10.0, 20.0, 5.0), Vector3::new(1.0, 0.0, 0.0));
        assert!(flash.is_alive());
        assert!((flash.base.position.x - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_fx_factory_shockwave() {
        let ring = FXFactory::shockwave_ring(Point3::origin(), 30.0);
        assert!(ring.is_alive());
        assert!((ring.end_radius - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_persistent_effect() {
        // Lifetime 0.0 = never expires
        let mut ring = FXRing::new(
            Point3::origin(),
            [1.0; 4],
            10.0,
            10.0,
            1.0,
            0.0, // Persistent
        );

        ring.update(100.0); // Lots of time
        assert!(ring.is_alive());
        assert!((ring.base.alpha - 1.0).abs() < 0.01); // No fade
    }
}
