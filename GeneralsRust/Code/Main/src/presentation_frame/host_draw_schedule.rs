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
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use glam::Mat4;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

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

#[derive(Debug, Clone, Copy)]
pub struct HostPresentVisualInput {
    pub visual_dt_ms: u32,
    pub frozen: bool,
}

struct PresentState {
    epoch: u64,
    visual_dt_ms: u32,
    frozen: bool,
    loco_done: HashMap<u32, Mat4>,
    particles_advanced: bool,
    particle_visual_ms: u32,
    phases: Vec<HostPresentPhase>,
}

impl PresentState {
    fn new() -> Self {
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

static STATE: Lazy<Mutex<PresentState>> = Lazy::new(|| Mutex::new(PresentState::new()));
static ACTIVE: AtomicU64 = AtomicU64::new(0);
static PARTICLE_MS: AtomicU32 = AtomicU32::new(0);

/// C++ `Get_Frame_Time()!=0` (W3DDisplay.cpp:1824).
#[must_use]
pub const fn should_advance_visuals(visual_dt_ms: u32) -> bool {
    visual_dt_ms != 0
}

pub fn begin_presented_frame(input: HostPresentVisualInput) {
    let mut state = STATE.lock();
    state.epoch = state.epoch.saturating_add(1);
    state.visual_dt_ms = input.visual_dt_ms;
    state.frozen = input.frozen;
    state.loco_done.clear();
    state.particles_advanced = false;
    state.phases.clear();
    state.phases.push(HostPresentPhase::Freeze);
    ACTIVE.store(state.epoch, Ordering::Release);
}

#[must_use]
pub fn present_epoch() -> u64 {
    ACTIVE.load(Ordering::Acquire)
}

#[must_use]
pub fn visual_time_permits_loco() -> bool {
    let state = STATE.lock();
    if state.epoch == 0 {
        return true;
    }
    !state.frozen && should_advance_visuals(state.visual_dt_ms)
}

#[must_use]
pub fn cached_applied_matrix(id: ObjectId) -> Option<Mat4> {
    STATE.lock().loco_done.get(&id.0).copied()
}

pub fn note_loco_applied(id: ObjectId, matrix: Mat4) {
    let mut state = STATE.lock();
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
pub fn should_calc_loco(id: ObjectId) -> bool {
    let state = STATE.lock();
    if state.epoch == 0 {
        return true;
    }
    if state.frozen || !should_advance_visuals(state.visual_dt_ms) {
        return false;
    }
    !state.loco_done.contains_key(&id.0)
}

pub fn advance_particles_after_transforms() -> u32 {
    let mut state = STATE.lock();
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
    PARTICLE_MS.store(state.particle_visual_ms, Ordering::Release);
    state.particle_visual_ms
}

#[must_use]
pub fn particle_visual_ms() -> u32 {
    PARTICLE_MS.load(Ordering::Acquire)
}

pub fn note_gpu_phase() {
    let mut state = STATE.lock();
    if !state.phases.contains(&HostPresentPhase::Gpu) {
        state.phases.push(HostPresentPhase::Gpu);
    }
}

#[must_use]
pub fn phase_log() -> Vec<HostPresentPhase> {
    STATE.lock().phases.clone()
}

pub fn reset_host_present_schedule() {
    *STATE.lock() = PresentState::new();
    ACTIVE.store(0, Ordering::Release);
    PARTICLE_MS.store(0, Ordering::Release);
}

/// Test/host helper: freeze → loco (once per id) → particles → GPU.
pub fn run_host_present_visual_phases(
    input: HostPresentVisualInput,
    apply_loco: impl FnOnce(),
) -> Vec<HostPresentPhase> {
    begin_presented_frame(input);
    apply_loco();
    let _ = advance_particles_after_transforms();
    note_gpu_phase();
    phase_log()
}
