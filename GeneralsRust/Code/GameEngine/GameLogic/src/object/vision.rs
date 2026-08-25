//! Object vision / shroud / map-boundary helpers (C++ Object.cpp).
//!
//! Split from `object_vision.rs` so new C++-parity logic does not grow that
//! file and `object.rs` only needs `mod vision`.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `Object::getShroudClearingRange` (Object.cpp:5128-5156).
    pub fn get_shroud_clearing_range(&self) -> Real {
        if self.test_status(ObjectStatusTypes::UnderConstruction) {
            return self.get_geometry_info().get_bounding_circle_radius();
        }
        self.shroud_clearing_range
    }

    /// C++ `Object::setShroudClearingRange` (Object.cpp:5159-5191).
    pub fn set_shroud_clearing_range(&mut self, range: Real) {
        let range = range.max(0.0);
        if range == self.shroud_clearing_range {
            return;
        }
        self.shroud_clearing_range = range;
        let pos = *self.get_position();
        if pos.x != 0.0 || pos.y != 0.0 || pos.z != 0.0 {
            self.handle_partition_cell_maintenance();
        }
    }

    /// C++ `Object::friend_prepareForMapBoundaryAdjust` (Object.cpp:2777-2794).
    pub fn friend_prepare_for_map_boundary_adjust(&mut self) {
        if self.radar_data.is_some() {
            let radar = game_engine::common::system::radar::get_radar_system();
            if let Ok(mut radar_guard) = radar.write() {
                radar_guard.remove_object(self.id);
            }
        }
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.unregister_object(self.id);
        }
        self.partition_last_look.reset();
        self.partition_reveal_all_last_look.reset();
        self.partition_last_shroud.reset();
        self.partition_last_threat.reset();
        self.partition_last_value.reset();
    }

    /// C++ `Object::friend_notifyOfNewMapBoundary` (Object.cpp:2799-2813).
    pub fn friend_notify_of_new_map_boundary(&mut self) {
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            partition.register_object_at(self.id, *self.get_position());
        }
        if self.radar_data.is_some() {
            self.refresh_radar_object_from_state();
        } else {
            let radar = game_engine::common::system::radar::get_radar_system();
            if let Ok(mut radar_guard) = radar.write() {
                let mut radar_obj = crate::object::RadarObject::new(self.id);
                let pos = self.get_position();
                radar_obj.world_pos =
                    game_engine::common::system::radar::Coord3D::new(pos.x, pos.y, pos.z);
                radar_obj.priority = match self.get_radar_priority() {
                    crate::common::RadarPriorityType::Invalid => {
                        game_engine::common::system::radar::RadarPriorityType::Invalid
                    }
                    crate::common::RadarPriorityType::NotOnRadar => {
                        game_engine::common::system::radar::RadarPriorityType::NotOnRadar
                    }
                    crate::common::RadarPriorityType::Structure => {
                        game_engine::common::system::radar::RadarPriorityType::Structure
                    }
                    crate::common::RadarPriorityType::Unit => {
                        game_engine::common::system::radar::RadarPriorityType::Unit
                    }
                    crate::common::RadarPriorityType::LocalUnitOnly => {
                        game_engine::common::system::radar::RadarPriorityType::LocalUnitOnly
                    }
                };
                self.populate_radar_object_from_state(&mut radar_obj);
                radar_guard.add_object(radar_obj);
            }
        }
        self.add_self_to_pathfind_map();
        self.handle_partition_cell_maintenance();

        let in_playable = crate::helpers::TheTerrainLogic::get()
            .map(|terrain| {
                let extent = terrain.get_extent();
                let pos = self.get_position();
                pos.x >= extent.lo.x
                    && pos.x <= extent.hi.x
                    && pos.y >= extent.lo.y
                    && pos.y <= extent.hi.y
            })
            .unwrap_or(true);
        if in_playable {
            self.private_status &= !(ObjectPrivateStatusBits::OffMap as u8);
        } else {
            self.private_status |= ObjectPrivateStatusBits::OffMap as u8;
        }
    }

    /// C++ `Object::getShroudedStatus` (Object.cpp:1778-1788).
    pub fn get_shrouded_status(&self, player_index: i32) -> ObjectShroudStatus {
        if self.is_kind_of(KindOf::AlwaysVisible) {
            return ObjectShroudStatus::Clear;
        }
        // C++ garrisoned / unregistered objects have no PartitionData → CLEAR.
        if self.get_container_id().is_some() {
            return ObjectShroudStatus::Clear;
        }
        if let Some(partition_data) = &self.partition_data {
            if let Ok(mut data) = partition_data.lock() {
                return data.get_shrouded_status(player_index, self);
            }
        }
        ObjectShroudStatus::Clear
    }

    /// C++ `TheAI->pathfinder()->addObjectToPathfindMap(this)`.
    pub(super) fn add_self_to_pathfind_map(&self) {
        let pos = *self.get_position();
        let footprint = crate::ai::object_footprint_positions(self).unwrap_or_else(|| vec![pos]);
        if let Ok(ai) = crate::ai::THE_AI.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(mut pf) = pf.write() {
                    pf.add_object_to_map(self.id, &footprint, false);
                }
            }
        }
    }

    /// C++ `getControllingPlayer()->getRelationship(currentPlayer->getDefaultTeam())`.
    pub(super) fn controller_relationship_to_player_default_team(
        controller: &Player,
        current_player: &Player,
    ) -> Relationship {
        match current_player.get_default_team() {
            Some(team) => match team.read() {
                Ok(team_guard) => controller.get_relationship_with_team(&team_guard),
                Err(_) => Relationship::Neutral,
            },
            None => Relationship::Neutral,
        }
    }
}
