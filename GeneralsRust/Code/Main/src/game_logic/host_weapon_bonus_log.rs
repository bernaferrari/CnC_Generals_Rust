//! Frame-local host weapon-bonus log for GameWorld SetWeaponBonus parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostWeaponBonusEvent {
    pub object: ObjectId,
    pub enthusiastic: bool,
    pub subliminal: bool,
    pub horde: bool,
    pub nationalism: bool,
    pub frenzy: bool,
    pub frenzy_level: u8,
    pub battle_plan_bombardment: bool,
    pub battle_plan_hold_the_line: bool,
    pub battle_plan_search_and_destroy: bool,
    pub frenzy_until_frame: u32,
    pub battle_plan_sight_scalar_applied: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostWeaponBonusEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: HostWeaponBonusEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

pub fn drain() -> Vec<HostWeaponBonusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
