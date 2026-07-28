//! Frame-local host production queue progress for GameWorld SetProductionQueue parity.

use super::ObjectId;
use std::cell::RefCell;

/// Snapshot of one production queue entry residual.
#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionQueueItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
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
    /// Host energy shortfall clamp residual (1.0 = full power).
    pub power_factor: f32,
    /// Wave 477: when true, apply power_factor only (GW sole-ticks queue/exit).
    pub power_factor_only: bool,
    /// Wave 480: when true, apply exit_delay_remaining only (post-spawn arm under sole-tick).
    pub exit_delay_only: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionProgressEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    producer: ObjectId,
    items: Vec<HostProductionQueueItem>,
    exit_delay_remaining: f32,
    power_factor: f32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionProgressEvent {
            producer,
            items,
            exit_delay_remaining: exit_delay_remaining.max(0.0),
            power_factor: power_factor.max(0.01),
            power_factor_only: false,
            exit_delay_only: false,
        });
    });
}

/// Wave 477: sole-tick residual — publish host power factor without queue stomp.
pub fn record_power_factor_only(producer: ObjectId, power_factor: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionProgressEvent {
            producer,
            items: Vec::new(),
            exit_delay_remaining: 0.0,
            power_factor: power_factor.max(0.01),
            power_factor_only: true,
            exit_delay_only: false,
        });
    });
}

/// Wave 480: sole-tick residual — arm factory exit delay after a unit exits.
pub fn record_exit_delay_only(producer: ObjectId, exit_delay_remaining: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionProgressEvent {
            producer,
            items: Vec::new(),
            exit_delay_remaining: exit_delay_remaining.max(0.0),
            power_factor: 1.0,
            power_factor_only: false,
            exit_delay_only: true,
        });
    });
}

pub fn drain() -> Vec<HostProductionProgressEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
