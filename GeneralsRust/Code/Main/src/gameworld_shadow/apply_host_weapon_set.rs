//! Host weapon-set, overcharge, contain-capacity, and hive apply.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
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
