//! Target/build/enter legality helpers.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    radar_notifications::RadarKind, AIState, GameLogic, KindOf, ObjectId, ObjectType,
    PendingSpecialAbility, Resources, Team,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::common::AsciiString;
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
        location.x >= min_x && location.x <= max_x && location.z >= min_z && location.z <= max_z
    }

    /// Minimal `canEnterObject`/`canDockAt` legality mirror for Main command execution.
    pub(super) fn can_issue_enter_or_dock(&self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        if unit_id == target_id {
            return false;
        }

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        let Some(target) = self.game_logic.host_object(target_id) else {
            return false;
        };

        if !unit.is_alive()
            || !target.is_alive()
            || unit.status.under_construction
            || target.status.under_construction
        {
            return false;
        }

        // Tunnel network residual: units already in the shared pool may transfer
        // to another allied tunnel without can_move (Garrisoned).
        let unit_in_tunnel = self
            .game_logic
            .tunnel_network_residual()
            .team_holding_unit(unit_id)
            .is_some();
        if unit.is_kind_of(KindOf::Structure) {
            return false;
        }
        if !unit.can_move() && !unit_in_tunnel {
            return false;
        }

        // USA Pilot residual: pilots may Enter unmanned ground vehicles for recrew
        // even when the vehicle is not a residual transport container.
        let pilot_recrew = crate::game_logic::host_usa_pilot::should_recrew_on_enter(
            crate::game_logic::host_usa_pilot::is_pilot_template(&unit.template_name),
            crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_unmanned(),
                target.status.under_construction,
                target.is_worker() || target.template_name.to_ascii_lowercase().contains("dozer"),
            ),
        );
        if pilot_recrew {
            return true;
        }

        if !target.can_contain() {
            return false;
        }

        // Residual garrison / Overlord BattleBunker / Battle Bus: infantry (and heroes)
        // only. C++ AllowInsideKindOf = INFANTRY. Generic transports still accept any
        // mobile unit. Combat Chinook allows INFANTRY + VEHICLE (rejects AIRCRAFT).
        // Tunnel Network: all units except aircraft (C++ TunnelTracker residual).
        // Fail-closed vs full C++ garrison filters.
        if target.is_tunnel_network_style_container() {
            if unit.is_kind_of(KindOf::Aircraft) {
                return false;
            }
            // Shared MaxTunnelCapacity=10 residual (team pool).
            let in_pool = self
                .game_logic
                .tunnel_network_residual()
                .is_in_network(unit.team, unit_id);
            if !in_pool
                && !self
                    .game_logic
                    .tunnel_network_residual()
                    .has_capacity(unit.team)
            {
                return false;
            }
            // Ally tunnels only for residual enter (not enemy capture residual).
            if target.team != unit.team && target.team != Team::Neutral {
                return false;
            }
            return true;
        }

        let infantry_only_container = target.is_kind_of(KindOf::Structure)
            || (target.is_overlord_style_container() && target.overlord_bunker_slot_capacity() > 0)
            || target.is_battle_bus_style_container()
            || target.is_listening_outpost_style_container()
            || target.is_troop_crawler_style_container();
        if infantry_only_container && !unit.is_kind_of(KindOf::Infantry) && !unit.is_hero() {
            return false;
        }
        // Combat Chinook ForbidInsideKindOf = AIRCRAFT residual.
        if target.is_combat_chinook_style_container() && unit.is_kind_of(KindOf::Aircraft) {
            return false;
        }

        let target_contains_unit = target.contained_units().contains(&unit_id);
        let target_has_space = target.has_capacity_for(1);
        if !target_contains_unit && !target_has_space {
            return false;
        }

        if target.team != unit.team && target.team != Team::Neutral {
            let target_has_occupants = !target.contained_units().is_empty();
            if target.is_faction_structure() || target_has_occupants {
                return false;
            }
        }

        true
    }

}
