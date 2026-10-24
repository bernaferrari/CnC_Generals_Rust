//! Object scalar influence receipts; cell values stay in the receiving service.
use super::*;

impl Object {
    pub(crate) fn set_shroud_clearing_range_with_partition_owner(
        &mut self,
        range: f32,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.set_shroud_clearing_range_with_partition_context(range, Some(partition));
    }

    pub(super) fn set_shroud_clearing_range_with_partition_context(
        &mut self,
        range: f32,
        partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    ) {
        self.require_partition_owner(partition.as_deref());
        if self.shroud_clearing_range == range {
            return;
        }
        self.shroud_clearing_range = range;
        if self.get_position() != Vec3::ZERO {
            if let Some(partition) = partition {
                self.handle_partition_cell_maintenance_with_partition_owner(partition);
            }
        }
    }

    /// Detached construction never publishes scalar cells. Receiving callers
    /// must pass the same explicit owner used for their partition operation.
    pub fn stamp_partition_value_threat(&mut self) {
        self.require_partition_owner(None);
    }

    pub(crate) fn stamp_partition_value_threat_with_partition_owner(
        &mut self,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.require_partition_owner(Some(partition));
        self.unstamp_partition_value_threat_with_partition_owner(partition);
        let Some(stamp) = self.desired_partition_scalar_stamp() else {
            return;
        };
        self.partition_last_affect_applied = partition.apply_scalar_stamp(&stamp, true);
        self.partition_last_affect = Some(stamp);
    }

    fn desired_partition_scalar_stamp(
        &self,
    ) -> Option<crate::game_logic::partition_manager::HostPartitionAffectStamp> {
        // C++ Object.cpp4819: queued DESTROYED is not effectivelyDead.
        if self.status.under_construction
            || self.status.effectively_dead
            || self.shroud_clearing_range <= 0.0
        {
            return None;
        }
        let Some(player_id) = self.owner_player_id else {
            return None;
        };
        let stamp = crate::game_logic::partition_manager::HostPartitionAffectStamp {
            x: self.get_position().x,
            z: self.get_position().z,
            range: self.vision_range,
            value: self.partition_cash_value,
            threat: self.partition_threat_value,
            mask: 1u32 << player_id.min(15),
        };
        Some(stamp)
    }

    /// Receiving immutable definition projection retained for diagnostics. The
    /// C++ factory has not performed cell maintenance before Drawable creation.
    pub(crate) fn cache_initial_partition_scalar_payload(
        &mut self,
        partition: &crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.require_partition_owner(Some(partition));
        // An authored Create callback may already have maintained range/team.
        // Keep its exact receipt; factory metadata must not replace it.
        if self.partition_last_affect_applied {
            return;
        }
        self.partition_last_affect = self.desired_partition_scalar_stamp();
    }

    pub(crate) fn forget_retired_partition_scalar_receipt(&mut self) {
        self.partition_last_affect = None;
        self.partition_last_affect_applied = false;
    }

    pub fn unstamp_partition_value_threat(&mut self) {
        self.require_partition_owner(None);
        assert!(
            self.partition_last_affect.is_none(),
            "stamped actor requires explicit scalar retirement"
        );
    }

    pub(crate) fn unstamp_partition_value_threat_with_partition_owner(
        &mut self,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.require_partition_owner(Some(partition));
        if let Some(stamp) = self.partition_last_affect.take() {
            if self.partition_last_affect_applied {
                partition.apply_scalar_stamp(&stamp, false);
            }
        }
        self.partition_last_affect_applied = false;
    }

    pub fn handle_partition_cell_maintenance(&mut self) {
        self.stamp_partition_value_threat();
    }

    pub(crate) fn handle_partition_cell_maintenance_with_partition_owner(
        &mut self,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.stamp_partition_value_threat_with_partition_owner(partition);
    }
}
