//! Target/build/enter legality helpers.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    pub(super) fn validate_target_exists(&self, target_id: ObjectId) -> bool {
        self.game_logic.host_object(target_id).is_some()
    }

    pub(super) fn validate_build_location(&self, location: Vec3) -> bool {
        if !location.x.is_finite() || !location.z.is_finite() {
            
            return false;
        }
        // Use loaded map world bounds when available (Lone Eagle bases can sit
        // near edges beyond the old hard-coded ±1000 host box). Fall back to a
        // generous host default for synthetic/no-map worlds.
        let (min, max) = self.game_logic.world_bounds();
        let pad = 50.0;
        let min_x = min.x.min(-1000.0) - pad;
        let max_x = max.x.max(1000.0) + pad;
        let min_z = min.z.min(-1000.0) - pad;
        let max_z = max.z.max(1000.0) + pad;
        let ok = location.x >= min_x
            && location.x <= max_x
            && location.z >= min_z
            && location.z <= max_z;
        ok
    }

    /// C++ `canEnterObject` legality mirror.  Dock uses the distinct
    /// `can_issue_dock` path below; a SupplyCenter/Warehouse is never treated
    /// as a generic container.
    pub(super) fn can_issue_enter(&self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        if unit_id == target_id {
            return false;
        }

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        let Some(target) = self.game_logic.host_object(target_id) else {
            return false;
        };
        // C++ canEnterObject applies isObjectShroudedForAction before unmanned
        // / pilot recrew special cases (`ActionManager.cpp:519-560`).
        if self.game_logic.is_object_shrouded_for_action(unit, target) {
            return false;
        }

        if !unit.is_alive()
            || !target.is_alive()
            || unit.status.under_construction
            || target.status.under_construction
            || target.status.sold
            || target.is_subdued_disabled()
            || unit.is_kind_of(KindOf::IgnoredInGui)
            || target.is_kind_of(KindOf::IgnoredInGui)
        {
            return false;
        }

        // Tunnel network residual: units already in the shared pool may transfer
        // to another allied tunnel without can_move (Garrisoned).
        let unit_in_tunnel = self
            .game_logic
            .tunnel_network_residual()
            .player_holding_unit(unit_id)
            .is_some();
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        if !unit.can_move() && !unit_in_tunnel {
            return false;
        }

        // USA Pilot residual: pilots may Enter unmanned ground vehicles for recrew
        // even when the vehicle is not a residual transport container.
        let pilot_recrew = self.game_logic.can_execute_pilot_recrew(unit_id, target_id);
        if pilot_recrew {
            return true;
        }
        // C++ canEnterObject: generic infantry steals DISABLED_UNMANNED husks.
        if self
            .game_logic
            .can_execute_infantry_unmanned_recrew(unit_id, target_id)
        {
            return true;
        }

        // Keep the executor as the final authority.  This central helper is
        // also used by boot classification and Enter arrival, so a frozen RMB
        // hint cannot bypass exact-controller ownership or rider-weighted
        // TransportContain capacity while the order is in flight.
        self.game_logic
            .can_unit_enter_normal_target(unit_id, target_id)
    }

    /// C++ `ActionManager::canDockAt` subset for the exact DockUpdate modules
    /// retained by Main.  In particular, it does *not* require `can_contain`
    /// or spare capacity before a railed dock command is issued.
    pub(super) fn can_issue_dock(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> Option<crate::game_logic::DockKind> {
        use crate::game_logic::DockKind;
        use gamelogic::common::Relationship;

        if unit_id == target_id {
            return None;
        }
        let unit = self.game_logic.host_object(unit_id)?;
        let target = self.game_logic.host_object(target_id)?;
        if !unit.is_alive()
            || !target.is_alive()
            || unit.status.under_construction
            || target.status.under_construction
            || target.status.sold
            || unit.is_kind_of(KindOf::Structure)
            || !unit.can_move()
        {
            return None;
        }

        // `ActionManager::canTransferSuppliesAt` has two deliberately
        // different ownership rules.  A SupplyCenter is a deposit owned by
        // one *controlling player*, not merely an ally/faction.  A warehouse
        // is a map supply source and only refuses an enemy relationship.
        //
        // Some legacy/map saves have no player provenance at all.  Retain
        // the old faction fallback only for a wholly ownerless, unambiguous
        // host world; mixing a player-owned object with an ownerless one must
        // not make a player-specific SupplyCenter look shared.
        let legacy_ownerless_world = self.game_logic.uses_legacy_team_ownership_fallback();
        let same_supply_center_controller = match (unit.owner_player_id, target.owner_player_id) {
            (Some(unit_owner), Some(target_owner)) => unit_owner == target_owner,
            (None, None) => {
                legacy_ownerless_world
                    && unit.team == target.team
                    && self
                        .game_logic
                        .unique_player_id_for_team(unit.team)
                        .is_some()
            }
            _ => false,
        };
        let warehouse_is_not_enemy = match (unit.owner_player_id, target.owner_player_id) {
            (Some(_), Some(_)) => {
                // Skirmish reality behind `ActionManager::canTransferSuppliesAt`:
                // two distinct controlling players are Allies only through an
                // explicit alliance (shared `alliance_team` / diplomacy map).
                // Faction equality alone is ENEMIES-or-NEUTRAL, and a Neutral
                // label must not open another player's warehouse.
                self.game_logic.object_relationship(target, unit)
                    == Relationship::Allies
            }
            // `Object::getRelationship` treats an ownerless map object as
            // neutral. Do not manufacture hostility from its faction label.
            (Some(_), None) | (None, Some(_)) => true,
            // Only old map/synthetic objects with no ownership provenance at
            // all retain the legacy faction/Neutral fallback.
            (None, None) => {
                legacy_ownerless_world
                    && (target.team == Team::Neutral
                        || (target.team == unit.team
                            && self
                                .game_logic
                                .unique_player_id_for_team(unit.team)
                                .is_some()))
            }
        };

        match target.thing.template.dock_kind {
            DockKind::SupplyCenter
                if unit.is_resource_collector()
                    && unit.stored_resources.supplies > 0
                    && same_supply_center_controller =>
            {
                Some(DockKind::SupplyCenter)
            }
            DockKind::SupplyWarehouse
                if unit.is_resource_collector()
                    && target.stored_resources.supplies > 0
                    && warehouse_is_not_enemy =>
            {
                Some(DockKind::SupplyWarehouse)
            }
            DockKind::RailedTransport
                if unit.is_kind_of(KindOf::Vehicle) || unit.is_kind_of(KindOf::Infantry) =>
            {
                Some(DockKind::RailedTransport)
            }
            _ => None,
        }
    }
}
#[cfg(test)]
#[path = "leftover_dispatch_tests.rs"]
mod leftover_dispatch_tests;

#[cfg(test)]
#[path = "can_use_special_power_caster_filter_tests.rs"]
mod can_use_special_power_caster_filter_tests;

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
