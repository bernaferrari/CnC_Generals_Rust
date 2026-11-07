//! Test diagnostics for production queue state already committed to BuildingData.
//! This module is excluded from production; completion delivery uses ProductionEvents.

// This file is mounted below `host_mods_logs_c`, not directly below
// `game_logic`; use the public root paths so the Queue exit runtime state is
// available regardless of that packing module's imports.
use crate::game_logic::{ObjectId, ProductionExitRuntimeState};
use std::cell::RefCell;

/// Snapshot of one production queue entry residual.
#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionQueueItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    /// C++ ProductionEntry::m_framesUnderConstruction mirrored across the
    /// host/GameWorld boundary.  Float `progress` is presentation only.
    pub construction_frames: u32,
    pub cost_supplies: u32,
    pub is_upgrade: bool,
    /// C++ ProductionEntry::m_productionQuantityTotal residual (Wave 463).
    pub quantity_total: u32,
    /// C++ ProductionEntry::m_productionQuantityProduced residual (Wave 463).
    pub quantity_produced: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionProgressEvent {
    pub producer: ObjectId,
    pub items: Vec<HostProductionQueueItem>,
    /// C++ QueueProductionExitUpdate residual (seconds).
    pub exit_delay_remaining: f32,
    /// C++ QueueProductionExitUpdate::m_currentDelay in logic frames.  The
    /// float above is retained only for pre-frame-counter snapshots and UI.
    pub exit_delay_remaining_frames: u32,
    /// C++ QueueProductionExitUpdate::m_currentBurstCount.
    pub exit_burst_remaining: u32,
    /// Whether these two counters are source-backed Queue runtime authority.
    pub queue_exit_state_initialized: bool,
    /// Host energy shortfall clamp residual (1.0 = full power).
    pub power_factor: f32,
    /// Wave 477: when true, apply power_factor only (GW sole-ticks queue/exit).
    pub power_factor_only: bool,
    /// Wave 480: when true, apply exit_delay_remaining only (post-spawn arm under sole-tick).
    pub exit_delay_only: bool,
}




fn with_log_mut<R>(f: impl FnOnce(&mut Vec<HostProductionProgressEvent>) -> R) -> R {
    let services = crate::main_services::main_services();
    let mut log = services
        .host_production_progress_log
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    f(&mut log)
}

pub fn record(
    producer: ObjectId,
    items: Vec<HostProductionQueueItem>,
    exit_delay_remaining: f32,
    power_factor: f32,
) {
    record_with_exit_state(
        producer,
        items,
        exit_delay_remaining,
        ProductionExitRuntimeState {
            delay_frames: 0,
            burst_remaining: 0,
            initialized: false,
        },
        power_factor,
    );
}

/// Record a full queue snapshot together with authoritative parsed Queue exit
/// state.  Existing callers retain `record` so old float-only paths remain
/// backwards compatible rather than accidentally acquiring Queue authority.
pub fn record_with_exit_state(
    producer: ObjectId,
    items: Vec<HostProductionQueueItem>,
    exit_delay_remaining: f32,
    exit_state: ProductionExitRuntimeState,
    power_factor: f32,
) {
    with_log_mut(|log| {
        log.push(HostProductionProgressEvent {
            producer,
            items,
            exit_delay_remaining: exit_delay_remaining.max(0.0),
            exit_delay_remaining_frames: exit_state.delay_frames,
            exit_burst_remaining: exit_state.burst_remaining,
            queue_exit_state_initialized: exit_state.initialized,
            power_factor: power_factor.max(0.01),
            power_factor_only: false,
            exit_delay_only: false,
        });
    });
}

/// Wave 477: sole-tick residual — publish host power factor without queue stomp.
pub fn record_power_factor_only(producer: ObjectId, power_factor: f32) {
    with_log_mut(|log| {
        log.push(HostProductionProgressEvent {
            producer,
            items: Vec::new(),
            exit_delay_remaining: 0.0,
            exit_delay_remaining_frames: 0,
            exit_burst_remaining: 0,
            queue_exit_state_initialized: false,
            power_factor: power_factor.max(0.01),
            power_factor_only: true,
            exit_delay_only: false,
        });
    });
}


pub fn drain() -> Vec<HostProductionProgressEvent> {
    with_log_mut(std::mem::take)
}

pub fn clear() {
    with_log_mut(|log| log.clear());
}

pub fn len() -> usize {
    with_log_mut(|log| log.len())
}
