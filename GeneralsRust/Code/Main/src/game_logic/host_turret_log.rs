//! Frame-local host turret log for GameWorld SetTurret parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTurretEvent {
    pub object: ObjectId,
    pub angle_deg: f32,
    pub pitch_deg: f32,
    pub holding: bool,
    pub idle_scanning: bool,
    pub turret_turn_rate_rad: f32,
    pub turret_recenter_frames: u32,
    pub turret_hold_until_frame: u32,
    pub turret_idle_recentering: bool,
    pub turret_enabled: bool,
    pub turret_rotating: bool,
    pub turret_natural_angle_deg: f32,
    pub turret_natural_pitch_deg: f32,
    pub turret_target_host: u32,
    pub turret_force_attacking: bool,
    pub turret_mood_target: bool,
    pub turret_idle_scan_next_frame: u32,
    pub turret_idle_scan_desired_angle_deg: f32,
    pub turret_idle_scan_index: u32,
    pub turret_substate: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostTurretEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    angle_deg: f32,
    pitch_deg: f32,
    holding: bool,
    idle_scanning: bool,
    turret_turn_rate_rad: f32,
    turret_recenter_frames: u32,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_enabled: bool,
    turret_rotating: bool,
    turret_natural_angle_deg: f32,
    turret_natural_pitch_deg: f32,
    turret_target_host: u32,
    turret_force_attacking: bool,
    turret_mood_target: bool,
    turret_idle_scan_next_frame: u32,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_substate: u8,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostTurretEvent {
            object,
            angle_deg,
            pitch_deg,
            holding,
            idle_scanning,
            turret_turn_rate_rad,
            turret_recenter_frames,
            turret_hold_until_frame,
            turret_idle_recentering,
            turret_enabled,
            turret_rotating,
            turret_natural_angle_deg,
            turret_natural_pitch_deg,
            turret_target_host,
            turret_force_attacking,
            turret_mood_target,
            turret_idle_scan_next_frame,
            turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index,
            turret_substate,
        });
    });
}

pub fn drain() -> Vec<HostTurretEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
