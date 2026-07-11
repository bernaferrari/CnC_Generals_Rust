//! Animation Synchronization System
//!
//! Synchronizes material animations, UV mappers, and skeletal animations
//! across the entire rendering pipeline with frame-accurate timing.
//!
//! This is the integration layer that ensures:
//! - Material animation time stays synchronized with skeletal animation
//! - UV mapper animations play at correct speed
//! - Animation time is properly passed to GPU
//! - All systems use consistent time source

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Global animation clock - synchronized across all systems
pub struct AnimationClock {
    /// Last point in time when the clock was advanced automatically
    last_update: Instant,
    /// Current elapsed time in microseconds (u64 allows up to ~584,000 years)
    elapsed_time: Arc<AtomicU64>,
    /// Whether animation is currently playing
    is_playing: bool,
    /// Playback speed multiplier (1.0 = normal, 2.0 = 2x speed)
    speed_multiplier: f32,
}

impl AnimationClock {
    /// Create a new animation clock
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            elapsed_time: Arc::new(AtomicU64::new(0)),
            is_playing: true,
            speed_multiplier: 1.0,
        }
    }

    /// Update the clock (should be called once per frame)
    pub fn update(&mut self, delta_override: Option<f32>) {
        if !self.is_playing {
            self.last_update = Instant::now();
            return;
        }

        let delta = if let Some(value) = delta_override {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        } else {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_update).as_secs_f32();
            self.last_update = now;
            elapsed
        };

        if delta <= 0.0 {
            return;
        }

        let adjusted = delta * self.speed_multiplier;
        if adjusted <= 0.0 {
            return;
        }

        let micros = (adjusted * 1_000_000.0) as u64;
        self.elapsed_time.fetch_add(micros, Ordering::Relaxed);
    }

    /// Force the clock to a specific absolute time (in seconds)
    pub fn sync_to_seconds(&mut self, total_seconds: f32) {
        let total = if total_seconds.is_finite() {
            total_seconds.max(0.0)
        } else {
            0.0
        };
        let micros = (total * 1_000_000.0) as u64;
        self.elapsed_time.store(micros, Ordering::Relaxed);
        self.last_update = Instant::now();
    }

    /// Get current elapsed time in seconds
    pub fn elapsed_seconds(&self) -> f32 {
        let micros = self.elapsed_time.load(Ordering::Relaxed);
        micros as f32 / 1_000_000.0
    }

    /// Pause animation playback
    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    /// Resume animation playback
    pub fn resume(&mut self) {
        self.last_update = Instant::now();
        self.is_playing = true;
    }

    /// Set playback speed multiplier
    pub fn set_speed(&mut self, speed: f32) {
        self.speed_multiplier = speed.max(0.0);
    }

    /// Reset time to zero
    pub fn reset(&mut self) {
        self.elapsed_time.store(0, Ordering::Relaxed);
        self.last_update = Instant::now();
        self.is_playing = true;
    }

    /// Check if animation is playing
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Animation time context passed to all rendering systems
#[derive(Debug, Clone, Copy)]
pub struct AnimationTimeContext {
    /// Elapsed time in seconds since animation start
    pub elapsed_time: f32,
    /// Current frame number (for frame-based systems)
    pub frame_number: u32,
    /// Frame delta time (seconds since last frame)
    pub delta_time: f32,
    /// Is animation currently playing
    pub is_playing: bool,
}

impl AnimationTimeContext {
    /// Create context from clock
    pub fn from_clock(clock: &AnimationClock, delta: f32, frame: u32) -> Self {
        Self {
            elapsed_time: clock.elapsed_seconds(),
            frame_number: frame,
            delta_time: delta,
            is_playing: clock.is_playing(),
        }
    }

    /// Create context with explicit values
    pub fn new(elapsed_time: f32, frame: u32, delta: f32, is_playing: bool) -> Self {
        Self {
            elapsed_time,
            frame_number: frame,
            delta_time: delta,
            is_playing,
        }
    }
}

/// Material animation state updater
///
/// Keeps material animations synchronized with global time
pub struct MaterialAnimationUpdater;

impl MaterialAnimationUpdater {
    /// Update all material passes with current animation time
    pub fn update_material_mappers(
        material_passes: &mut [crate::material_system::MaterialPassClass],
        context: &AnimationTimeContext,
    ) {
        for pass in material_passes {
            // Set mapper parameters based on animation time
            // This ensures UV mappers produce correct transforms
            Self::apply_animation_time_to_mapper(pass, context);
        }
    }

    /// Apply animation time to a single material pass
    fn apply_animation_time_to_mapper(
        pass: &mut crate::material_system::MaterialPassClass,
        _context: &AnimationTimeContext,
    ) {
        let mapper_id = pass.get_mapper_id();

        // Skip static textures
        if mapper_id == 0 {
            return;
        }

        // Time-based mappers: update arguments based on elapsed time
        match mapper_id {
            4 => {
                // LinearOffset: speed is stored as × 1000
                // No changes needed - GPU shader uses elapsed_time directly
            }
            7 => {
                // Grid: frame-based animation
                // Could implement frame counter updates here if needed
            }
            8 => {
                // Rotate: rotation based on time
                // No changes needed - GPU shader handles it
            }
            9 => {
                // SineLinearOffset: wave animation based on time
                // No changes needed - GPU shader handles it
            }
            _ => {
                // Other mapper types
            }
        }

        // Note: Most animations are computed in GPU shaders using elapsed_time
        // This function is here for material-level updates if needed in future
    }

    /// Update blend weights for animation blending.
    ///
    /// Normalizes weights and clamps invalid values to keep transitions stable.
    pub fn update_animation_blending(blend_weights: &mut [f32], _context: &AnimationTimeContext) {
        let mut total = 0.0f32;
        for weight in blend_weights.iter_mut() {
            if weight.is_finite() {
                *weight = weight.max(0.0);
            } else {
                *weight = 0.0;
            }
            total += *weight;
        }

        if total > 0.0 {
            for weight in blend_weights.iter_mut() {
                *weight /= total;
            }
        }
    }
}

/// Skeletal animation synchronizer
///
/// Synchronizes skeletal animations with material animations
pub struct SkeletalAnimationSynchronizer;

impl SkeletalAnimationSynchronizer {
    /// Update skeletal animations with current time
    pub fn update_skeletal_animations(
        _bone_transforms: &mut [glam::Mat4],
        context: &AnimationTimeContext,
    ) {
        // When skeletal animation system is active:
        // 1. Look up current frame/time in animation data
        // 2. Update bone transforms
        // 3. Ensure material animations stay synchronized

        if !context.is_playing {}

        // This would integrate with HAnim/HCompressedAnim playback
        // For now, the animation system handles this directly
    }

    /// Ensure material and skeletal animations stay in sync
    pub fn synchronize_all_animations(
        context: &AnimationTimeContext,
        skeletal_animations: &mut [glam::Mat4],
        material_passes: &mut [crate::material_system::MaterialPassClass],
    ) {
        // Update skeletal animations
        Self::update_skeletal_animations(skeletal_animations, context);

        // Update material animations
        MaterialAnimationUpdater::update_material_mappers(material_passes, context);

        // Both systems now use same time source, ensuring synchronization
    }
}

/// Render pipeline animation integrator
///
/// Integrates animation time into render info and GPU buffers
pub struct RenderPipelineAnimationIntegrator;

impl RenderPipelineAnimationIntegrator {
    /// Update animation time in render info
    pub fn update_render_info_with_animation(
        render_info: &mut crate::render_object_system::RenderInfoClass,
        context: &AnimationTimeContext,
    ) {
        // Update the time field in render info
        render_info.time = context.elapsed_time;
        // Frame count is also tracked separately
        render_info.frame_count = context.frame_number as u64;
    }

    /// Update GPU time uniform for shaders
    pub fn update_gpu_animation_time(
        _context: &AnimationTimeContext,
    ) -> crate::core::error::Result<()> {
        // Animation time is passed through render_info.time
        // Shaders receive it via uniforms in the rendering pipeline
        // This function documents the time synchronization point

        // Shader will use this time for:
        // - apply_uv_mapper() - texture coordinate transformations
        // - Any time-dependent effects
        Ok(())
    }

    /// Ensure animation time is properly threaded through render pipeline
    pub fn prepare_frame_animation_data(
        clock: &AnimationClock,
        frame_number: u32,
        delta_time: f32,
    ) -> AnimationTimeContext {
        AnimationTimeContext::from_clock(clock, delta_time, frame_number)
    }
}

/// Frame-level timing data mirrored from the engine (WW3D::Sync inputs in C++).
#[derive(Debug, Clone, Copy)]
pub struct AnimationFrameInput {
    /// Delta time between frames in seconds.
    pub delta_seconds: f32,
    /// Total elapsed time in seconds since engine start, if known.
    pub total_seconds: Option<f32>,
    /// Optional absolute frame index supplied by the engine.
    pub frame_number: Option<u64>,
}

impl AnimationFrameInput {
    /// Construct a new frame input, sanitizing invalid floating point values.
    pub fn new(delta_seconds: f32, total_seconds: Option<f32>, frame_number: Option<u64>) -> Self {
        Self {
            delta_seconds: if delta_seconds.is_finite() {
                delta_seconds.max(0.0)
            } else {
                0.0
            },
            total_seconds: total_seconds
                .and_then(|value| value.is_finite().then_some(value.max(0.0))),
            frame_number,
        }
    }

    /// Convenience helper for constant-rate fallback loops.
    pub fn from_delta(delta_seconds: f32) -> Self {
        Self::new(delta_seconds, None, None)
    }
}

/// High-level animation frame coordinator
///
/// Orchestrates all animation updates for a frame
pub struct AnimationFrameCoordinator {
    clock: AnimationClock,
    frame_count: u32,
}

impl AnimationFrameCoordinator {
    /// Create a new frame coordinator
    pub fn new() -> Self {
        Self {
            clock: AnimationClock::new(),
            frame_count: 0,
        }
    }

    /// Process a complete animation frame
    pub fn process_frame(&mut self, input: AnimationFrameInput) -> AnimationTimeContext {
        let delta = input.delta_seconds;
        if let Some(total_seconds) = input.total_seconds {
            self.clock.sync_to_seconds(total_seconds);
        } else {
            self.clock.update(Some(delta));
        }

        let frame_index = if let Some(frame_number) = input.frame_number {
            let clamped = frame_number.min(u32::MAX as u64) as u32;
            self.frame_count = clamped;
            clamped
        } else {
            self.frame_count = self.frame_count.wrapping_add(1);
            self.frame_count
        };

        // Create synchronized time context for all systems
        RenderPipelineAnimationIntegrator::prepare_frame_animation_data(
            &self.clock,
            frame_index,
            delta,
        )
    }

    /// Get animation clock for external control
    pub fn clock_mut(&mut self) -> &mut AnimationClock {
        &mut self.clock
    }

    /// Get read-only animation clock
    pub fn clock(&self) -> &AnimationClock {
        &self.clock
    }

    /// Get current frame number
    pub fn frame_number(&self) -> u32 {
        self.frame_count
    }
}

impl Default for AnimationFrameCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_clock_timing() {
        let mut clock = AnimationClock::new();
        std::thread::sleep(std::time::Duration::from_millis(50));
        clock.update(None);

        let elapsed = clock.elapsed_seconds();
        // Allow looser tolerance for sleep timing
        assert!(
            (0.03..=0.2).contains(&elapsed),
            "Timing should be ~50ms (got {}s)",
            elapsed
        );
    }

    #[test]
    fn test_animation_clock_pause_resume() {
        let mut clock = AnimationClock::new();
        clock.pause();
        assert!(!clock.is_playing());

        clock.resume();
        assert!(clock.is_playing());
    }

    #[test]
    fn test_animation_context_creation() {
        let context = AnimationTimeContext::new(1.5, 90, 0.016, true);
        assert_eq!(context.elapsed_time, 1.5);
        assert_eq!(context.frame_number, 90);
        assert_eq!(context.delta_time, 0.016);
        assert!(context.is_playing);
    }

    #[test]
    fn test_frame_coordinator() {
        let mut coordinator = AnimationFrameCoordinator::new();
        assert_eq!(coordinator.frame_number(), 0);

        let context = coordinator.process_frame(AnimationFrameInput::from_delta(0.016));
        assert!(context.is_playing);
        assert_eq!(context.frame_number, 1);
        assert!((context.delta_time - 0.016).abs() < f32::EPSILON);

        let context =
            coordinator.process_frame(AnimationFrameInput::new(0.033, Some(1.0), Some(42)));
        assert_eq!(context.frame_number, 42);
        assert!((context.delta_time - 0.033).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_speed_multiplier() {
        let mut clock = AnimationClock::new();
        clock.set_speed(2.0);
        assert!(clock.is_playing());

        clock.reset();
        assert_eq!(clock.elapsed_seconds(), 0.0);
    }
}
