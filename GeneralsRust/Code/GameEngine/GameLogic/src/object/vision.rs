//! Object vision / shroud / map-boundary helpers (C++ Object.cpp).
//!
//! Split from `object_vision.rs` so new C++-parity logic does not grow that
//! file and `object.rs` only needs `mod vision`.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;
use crate::common::RadarPriorityType;

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

    /// C++ `Object::friend_prepareForMapBoundaryAdjust` forgets the
    /// receipts before the owning partition is reset. Radar rows are
    /// cleared together by the matching new-map phase.
    pub(crate) fn detach_map_boundary_observations(&mut self) {
        self.partition_last_look.reset();
        self.partition_reveal_all_last_look.reset();
        self.partition_last_shroud.reset();
        self.partition_last_threat.reset();
        self.partition_last_value.reset();
    }

    pub(crate) fn restore_map_boundary_radar(
        &mut self,
        radar: &mut game_engine::common::system::radar::RadarSystem,
        local_player: Option<&Arc<RwLock<Player>>>,
    ) {
        use game_engine::common::system::radar as radar_types;

        let mut row = self.radar_data.as_deref().cloned().unwrap_or_else(|| {
            let mut row = RadarObject::new(self.id);
            row.priority = match self.get_radar_priority() {
                RadarPriorityType::Invalid => radar_types::RadarPriorityType::Invalid,
                RadarPriorityType::NotOnRadar => radar_types::RadarPriorityType::NotOnRadar,
                RadarPriorityType::Structure => radar_types::RadarPriorityType::Structure,
                RadarPriorityType::Unit => radar_types::RadarPriorityType::Unit,
                RadarPriorityType::LocalUnitOnly => radar_types::RadarPriorityType::LocalUnitOnly,
            };
            row
        });
        let pos = self.get_position();
        row.world_pos = radar_types::Coord3D::new(pos.x, pos.y, pos.z);
        self.populate_radar_object_for_player(&mut row, local_player);
        if let Some(existing) = self.radar_data.as_deref_mut() {
            *existing = row.clone();
        }
        radar.add_object(row);
    }

    /// The saved C++ private bit is updated at transform and boundary phases.
    pub(crate) fn refresh_off_map_status(&mut self, extent: &crate::common::Region3D) {
        if extent.is_in_region_no_z(self.get_position()) {
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
        if let Some(data) = &self.partition_data {
            return data.get_shrouded_status(player_index, self);
        }
        ObjectShroudStatus::Clear
    }

    /// C++ live-parent PartitionData query using the driving match's cells.
    pub(crate) fn get_shrouded_status_with_partition(
        &self,
        player_index: i32,
        partition: &crate::system::game_logic::PartitionManager,
    ) -> ObjectShroudStatus {
        if self.is_kind_of(KindOf::AlwaysVisible) || self.get_container_id().is_some() {
            return ObjectShroudStatus::Clear;
        }
        match &self.partition_data {
            Some(data) => data.get_shrouded_status_with_partition(player_index, self, partition),
            None => ObjectShroudStatus::Clear,
        }
    }

    pub(crate) fn get_shrouded_status_with_native_partition(
        &self,
        player_index: i32,
        partition: &crate::object::collide::PartitionManager,
        players: &crate::player::PlayerList,
    ) -> ObjectShroudStatus {
        if self.is_kind_of(KindOf::AlwaysVisible) || self.get_container_id().is_some() {
            return ObjectShroudStatus::Clear;
        }
        match &self.partition_data {
            Some(data) => data.get_shrouded_status_with_native_partition(
                player_index,
                self,
                partition,
                players,
            ),
            None => ObjectShroudStatus::Clear,
        }
    }

    pub(crate) fn clear_owned_partition_registration(&mut self) {
        self.partition_data = None;
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
