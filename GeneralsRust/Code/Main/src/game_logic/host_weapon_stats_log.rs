//! Frame-local host weapon stats log for GameWorld SetWeaponStats parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostWeaponStatsEvent {
    pub object: ObjectId,
    pub has_weapon: bool,
    pub weapon_damage: f32,
    pub weapon_range: f32,
    pub weapon_min_range: f32,
    pub weapon_reload_time: f32,
    /// Host Weapon::last_fire_time residual (sim seconds).
    pub weapon_last_fire_time: f32,
    /// `u32::MAX` = unlimited/None ammo.
    pub weapon_ammo: u32,
    pub weapon_can_target_air: bool,
    pub weapon_can_target_ground: bool,
    pub weapon_projectile_speed: f32,
    pub has_secondary_weapon: bool,
    pub secondary_weapon_damage: f32,
    pub secondary_weapon_range: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponStatsEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: HostWeaponStatsEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

pub fn drain() -> Vec<HostWeaponStatsEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
