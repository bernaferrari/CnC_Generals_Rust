//! Copy canonical Main weapon facts into the presentation observation.
use super::*;
use gamelogic::world::entities::EntityId;
impl GameWorldShadow {
    pub(super) fn sync_weapon_slots_from_host(
        &mut self,
        eid: EntityId,
        obj: &crate::game_logic::Object,
    ) {
        use gamelogic::world::{
            WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_PRIMARY, WEAPON_SLOT_SECONDARY,
            WEAPON_SLOT_TERTIARY, WeaponSlotFacts,
        };
        // Most objects have never carried a weapon. An absent observation
        // only needs a write when it clears a previously present slot.
        let previous = self
            .world
            .weapon_slots()
            .slots(eid)
            .map(|slots| slots.each_ref().map(|facts| facts.present));
        if previous.is_none()
            && obj.weapon.is_none()
            && obj.secondary_weapon.is_none()
            && obj.tertiary_weapon.is_none()
        {
            return;
        }
        let lock_slot = obj.weapon_lock_slot;
        let lock_ty = obj.weapon_lock_type as u8;
        let facts = |slot: u8, weapon: Option<&crate::game_logic::Weapon>| -> WeaponSlotFacts {
            let Some(w) = weapon else {
                return WeaponSlotFacts::default();
            };
            let barrel = obj
                .weapon_barrel_states
                .get(slot as usize)
                .map(|b| (b.current_barrel, b.barrel_count))
                .unwrap_or((0, 0));
            WeaponSlotFacts {
                present: true,
                clip_size: w.clip_size,
                ammo: w.ammo.unwrap_or(u32::MAX),
                reload_time: w.reload_time,
                last_fire_time: w.last_fire_time,
                barrel_cursor: barrel.0,
                barrel_count: barrel.1,
                lock_type: if lock_slot == slot { lock_ty } else { 0 },
            }
        };
        let pairs = [
            (WEAPON_SLOT_PRIMARY, obj.weapon.as_ref()),
            (WEAPON_SLOT_SECONDARY, obj.secondary_weapon.as_ref()),
            (WEAPON_SLOT_TERTIARY, obj.tertiary_weapon.as_ref()),
            (WEAPON_SLOT_MINE_CLEAR, None),
        ];
        for (slot, weapon) in pairs {
            if weapon.is_some() || previous.is_some_and(|row| row[slot as usize]) {
                let _ = self
                    .world
                    .weapon_slots_mut()
                    .apply_slot(eid, slot, facts(slot, weapon));
            }
        }
    }
}
