//! Frame-local host stealth delay/camo residual for GameWorld SetStealthDelay parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostStealthDelayEvent {
    pub object: ObjectId,
    pub stealth_allowed_frame: u32,
    pub stealth_delay_pending: bool,
    pub stealth_delay_frames: u32,
    pub stealth_breaks_on_damage: bool,
    pub detection_expires_frame: u32,
    pub camo_opacity_pulse_phase: f32,
    pub camo_heat_vision_opacity: f32,
    pub camo_net_sub_object_shown: bool,
    pub camo_net_sub_object_observer_visible: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostStealthDelayEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    stealth_allowed_frame: u32,
    stealth_delay_pending: bool,
    stealth_delay_frames: u32,
    stealth_breaks_on_damage: bool,
    detection_expires_frame: u32,
    camo_opacity_pulse_phase: f32,
    camo_heat_vision_opacity: f32,
    camo_net_sub_object_shown: bool,
    camo_net_sub_object_observer_visible: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostStealthDelayEvent {
            object,
            stealth_allowed_frame,
            stealth_delay_pending,
            stealth_delay_frames,
            stealth_breaks_on_damage,
            detection_expires_frame,
            camo_opacity_pulse_phase,
            camo_heat_vision_opacity,
            camo_net_sub_object_shown,
            camo_net_sub_object_observer_visible,
        });
    });
}

pub fn drain() -> Vec<HostStealthDelayEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
