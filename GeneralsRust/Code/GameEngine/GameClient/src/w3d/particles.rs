/*
**  Command & Conquer Generals Zero Hour™
**  Copyright 2025 Electronic Arts Inc.
*/

//! W3D Particle System Bridge
//!
//! Connects the C++-parity particle system (effects::particle_manager, effects::particle_system)
//! to the WGPU rendering pipeline (effects::particle_renderer).
//!
//! PARITY_NOTE: The C++ W3DParticleSystemManager (W3DParticleSys.h/.cpp) manages
//! render buffers (position, RGBA, size, angle) and dispatches rendering via
//! PointGroupClass for particles and StreakLineClass for streaks. This bridge
//! performs the equivalent data flow: collecting particle data from active
//! ParticleSystem instances and submitting it to the ParticleRenderer.

use std::sync::Arc;

use crate::effects::particle_manager::ParticleSystemManager;
use crate::effects::particle_renderer::{ParticleRenderer, ParticleUniforms};

/// W3D Particle System Bridge
///
/// Manages the per-frame data flow from particle system simulation to GPU rendering.
/// In C++, this is W3DParticleSystemManager::doParticles() + queueParticleRender().
pub struct W3DParticleSystemBridge {
    /// Whether the particle system is ready to render (set by queueParticleRender)
    ready_to_render: bool,
    /// Last frame's on-screen particle count (from C++ getOnScreenParticleCount)
    on_screen_particle_count: i32,
}

impl W3DParticleSystemBridge {
    /// Create a new W3D particle system bridge.
    pub fn new() -> Self {
        Self {
            ready_to_render: false,
            on_screen_particle_count: 0,
        }
    }

    /// Queue particle rendering (matches C++ W3DParticleSystemManager::queueParticleRender)
    ///
    /// Called from the flush/render pipeline to signal that particle data should
    /// be collected and submitted for rendering this frame.
    pub fn queue_particle_render(&mut self) {
        self.ready_to_render = true;
    }

    /// Execute particle rendering (matches C++ W3DParticleSystemManager::doParticles)
    ///
    /// Collects particle data from all active systems and submits it to the
    /// particle renderer. In C++, this:
    /// 1. Iterates all particle systems (skip DRAWABLE type)
    /// 2. For smudge systems: collect into SmudgeSet, render at end
    /// 3. For streak systems: fill pos/size/RGBA buffers, render via StreakLineClass
    /// 4. For particle/volume systems: fill buffers, render via PointGroupClass
    /// 5. Apply frustum culling
    ///
    /// PARITY_NOTE: Full rendering requires:
    /// - Streak rendering via line-strip geometry (not yet implemented — uses quads as fallback)
    /// - Volume particle rendering via depth-based layering (not yet implemented — uses regular quads)
    /// - Smudge/heat-distortion rendering via post-process (not yet implemented)
    pub fn do_particles(
        &mut self,
        particle_manager: &ParticleSystemManager,
        renderer: &mut ParticleRenderer,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        uniforms: &ParticleUniforms,
    ) {
        if !self.ready_to_render {
            return;
        }
        self.ready_to_render = false;

        // Collect all active particle systems for rendering
        let systems: Vec<_> = particle_manager.all_particle_systems().collect();

        // Render all particle systems
        renderer.render_particles(encoder, view, depth_view, &systems, uniforms);

        // Track on-screen particle count
        self.on_screen_particle_count = renderer.stats.particles_rendered as i32;
    }

    /// Get the number of particles rendered last frame (matches C++ getOnScreenParticleCount)
    pub fn get_on_screen_particle_count(&self) -> i32 {
        self.on_screen_particle_count
    }

    /// Check if ready to render (matches C++ m_readyToRender)
    pub fn is_ready_to_render(&self) -> bool {
        self.ready_to_render
    }
}

impl Default for W3DParticleSystemBridge {
    fn default() -> Self {
        Self::new()
    }
}
