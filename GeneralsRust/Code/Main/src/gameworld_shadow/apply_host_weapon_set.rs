//! Host weapon-set, overcharge, contain-capacity, and hive apply.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

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
            (
                WEAPON_SLOT_MINE_CLEAR,
                obj.mine_clearing_primary_weapon.as_ref(),
            ),
        ];
        for (slot, weapon) in pairs {
            let f = facts(slot, weapon);
            if f.present {
                let _ = self.world.weapon_slots_mut().apply_slot(eid, slot, f);
            }
        }
    }

    pub fn apply_host_weapon_set_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_set_log::HostWeaponSetEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponSetFlags {
                    target: eid,
                    player_upgrade: ev.player_upgrade,
                    armed_riders: ev.armed_riders,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_overcharge_events(
        &mut self,
        events: &[crate::game_logic::host_overcharge_log::HostOverchargeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetOvercharge {
                    target: eid,
                    enabled: ev.enabled,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_contain_capacity_events(
        &mut self,
        events: &[crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetContainCapacity {
                    target: eid,
                    max_transport: ev.max_transport,
                    max_garrison: ev.max_garrison,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_hive_events(
        &mut self,
        events: &[crate::game_logic::host_hive_log::HostHiveEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetHiveSlaves {
                    target: eid,
                    slave_count: ev.slave_count,
                    slave_hp: ev.slave_hp,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }
}
