//! Per-slot weapon facts owned by GameWorld (Entity stays primary-only).
//!
//! C++ `WeaponSet` slots are PRIMARY / SECONDARY / TERTIARY
//! (`WeaponSet.h` WeaponSlotType). Mine-clearing swaps the live primary
//! (`WeaponSet.cpp` WEAPONSET_MINECLEARING_DETAIL) — stored as slot 3 so it
//! never aliases slot 0.

use super::entities::EntityId;
use std::collections::HashMap;

pub const WEAPON_SLOT_PRIMARY: u8 = 0;
pub const WEAPON_SLOT_SECONDARY: u8 = 1;
pub const WEAPON_SLOT_TERTIARY: u8 = 2;
pub const WEAPON_SLOT_MINE_CLEAR: u8 = 3;
pub const WEAPON_SLOT_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSlotFacts {
    pub present: bool,
    pub clip_size: u32,
    pub ammo: u32,
    pub reload_time: f32,
    pub last_fire_time: f32,
    pub barrel_cursor: u8,
    pub barrel_count: u8,
    pub lock_type: u8,
}

impl Default for WeaponSlotFacts {
    fn default() -> Self {
        Self {
            present: false,
            clip_size: 0,
            ammo: u32::MAX,
            reload_time: 0.0,
            last_fire_time: 0.0,
            barrel_cursor: 0,
            barrel_count: 0,
            lock_type: 0,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct GameWorldWeaponSlots {
    by_entity: HashMap<u32, [WeaponSlotFacts; WEAPON_SLOT_COUNT]>,
}

impl GameWorldWeaponSlots {
    pub fn clear(&mut self) {
        self.by_entity.clear();
    }

    pub fn remove(&mut self, id: EntityId) {
        self.by_entity.remove(&id.get());
    }

    pub fn slot(&self, id: EntityId, slot: u8) -> Option<WeaponSlotFacts> {
        let idx = slot as usize;
        if idx >= WEAPON_SLOT_COUNT {
            return None;
        }
        self.by_entity.get(&id.get()).map(|slots| slots[idx])
    }

    pub fn slots(&self, id: EntityId) -> Option<&[WeaponSlotFacts; WEAPON_SLOT_COUNT]> {
        self.by_entity.get(&id.get())
    }

    /// Fail-closed: a missing host source (`present == false` and no prior row)
    /// leaves the table untouched.
    pub fn apply_slot(&mut self, id: EntityId, slot: u8, facts: WeaponSlotFacts) -> bool {
        let idx = slot as usize;
        if idx >= WEAPON_SLOT_COUNT {
            return false;
        }
        if !facts.present && !self.by_entity.contains_key(&id.get()) {
            return false;
        }
        let row = self
            .by_entity
            .entry(id.get())
            .or_insert_with(|| [WeaponSlotFacts::default(); WEAPON_SLOT_COUNT]);
        if !facts.present && !row[idx].present {
            return false;
        }
        row[idx] = facts;
        true
    }
}
