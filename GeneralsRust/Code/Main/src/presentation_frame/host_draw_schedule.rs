//! Host present-path draw schedule.
//!
//! Mirrors `GameClient` `display/client_draw_schedule.rs` for the Main
//! PresentationFrame path. C++ `W3DDisplay::draw` (W3DDisplay.cpp:1730-1835):
//! freeze → WW3D::Sync → updateViews/`Drawable::draw` gated by
//! `Get_Frame_Time()!=0` → `ParticleSystemManager::update` → drawViews GPU.
//!
//! Dual-world GameClient owns the live `OBJECT_REGISTRY` path. This module is
//! the host/present equivalent: one loco step per presented frame with elapsed
//! visual time, particles after transforms.

use std::collections::HashMap;

use glam::Mat4;

use crate::game_logic::ObjectId;

/// Same 33 ms visual quantum as `client_draw_schedule::W3D_FRAME_LENGTH_MS`.
pub const HOST_VISUAL_FRAME_MS: u32 = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPresentPhase {
    Freeze,
    PhysicsLoco,
    Particles,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostPresentVisualInput {
    pub visual_dt_ms: u32,
    pub frozen: bool,
}

#[derive(Debug)]
pub(crate) struct HostDrawSchedule {
    epoch: u64,
    visual_dt_ms: u32,
    frozen: bool,
    loco_done: HashMap<u32, Mat4>,
    particles_advanced: bool,
    particle_visual_ms: u32,
    phases: Vec<HostPresentPhase>,
}

impl HostDrawSchedule {
    pub(crate) fn new() -> Self {
        Self {
            epoch: 0,
            visual_dt_ms: HOST_VISUAL_FRAME_MS,
            frozen: false,
            loco_done: HashMap::new(),
            particles_advanced: false,
            particle_visual_ms: 0,
            phases: Vec::new(),
        }
    }
}

/// C++ `Get_Frame_Time()!=0` (W3DDisplay.cpp:1824).
#[must_use]
pub const fn should_advance_visuals(visual_dt_ms: u32) -> bool {
    visual_dt_ms != 0
}

impl HostDrawSchedule {
    pub fn begin_presented_frame(&mut self, input: HostPresentVisualInput) {
        let state = self;
        state.epoch = state.epoch.saturating_add(1);
        state.visual_dt_ms = input.visual_dt_ms;
        state.frozen = input.frozen;
        state.loco_done.clear();
        state.particles_advanced = false;
        state.phases.clear();
        state.phases.push(HostPresentPhase::Freeze);
    }

    #[must_use]
    pub fn present_epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub fn visual_time_permits_loco(&self) -> bool {
        let state = self;
        if state.epoch == 0 {
            return true;
        }
        !state.frozen && should_advance_visuals(state.visual_dt_ms)
    }

    #[must_use]
    pub fn cached_applied_matrix(&self, id: ObjectId) -> Option<Mat4> {
        self.loco_done.get(&id.0).copied()
    }

    pub fn note_loco_applied(&mut self, id: ObjectId, matrix: Mat4) {
        let state = self;
        if state.epoch == 0 {
            return;
        }
        let first = state.loco_done.is_empty();
        state.loco_done.insert(id.0, matrix);
        if first {
            state.phases.push(HostPresentPhase::PhysicsLoco);
        }
    }

    #[must_use]
    pub fn should_calc_loco(&self, id: ObjectId) -> bool {
        let state = self;
        if state.epoch == 0 {
            return true;
        }
        if state.frozen || !should_advance_visuals(state.visual_dt_ms) {
            return false;
        }
        !state.loco_done.contains_key(&id.0)
    }

    pub fn advance_particles_after_transforms(&mut self) -> u32 {
        let state = self;
        if state.particles_advanced {
            return state.particle_visual_ms;
        }
        if state.epoch != 0 && (state.frozen || !should_advance_visuals(state.visual_dt_ms)) {
            state.particles_advanced = true;
            state.phases.push(HostPresentPhase::Particles);
            return state.particle_visual_ms;
        }
        let dt = if state.epoch == 0 {
            HOST_VISUAL_FRAME_MS
        } else {
            state.visual_dt_ms
        };
        state.particle_visual_ms = state.particle_visual_ms.saturating_add(dt);
        state.particles_advanced = true;
        state.phases.push(HostPresentPhase::Particles);
        state.particle_visual_ms
    }

    #[must_use]
    pub fn particle_visual_ms(&self) -> u32 {
        self.particle_visual_ms
    }

    pub fn note_gpu_phase(&mut self) {
        let state = self;
        if !state.phases.contains(&HostPresentPhase::Gpu) {
            state.phases.push(HostPresentPhase::Gpu);
        }
    }

    #[must_use]
    pub fn phase_log(&self) -> Vec<HostPresentPhase> {
        self.phases.clone()
    }
}

impl Default for HostDrawSchedule {
    fn default() -> Self {
        Self::new()
    }
}
