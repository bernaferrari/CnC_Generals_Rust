use super::{HostPartitionAffectStamp, PartitionManager};
use gamelogic::system::shroud_manager::ShroudManager;
use std::sync::{Arc, Mutex};

/// A dependency handle to the existing service, not another scalar store.
pub(super) struct ScalarOwner(Arc<Mutex<ShroudManager>>);
impl std::fmt::Debug for ScalarOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReceivingScalarOwner")
    }
}

impl PartitionManager {
    /// Explicit service dependency on the existing driving world's scalar grid.
    /// Construction only retains identity; it does not stamp or initialize cells.
    pub(crate) fn with_scalar_owner(owner: &Arc<Mutex<ShroudManager>>) -> Self {
        Self {
            scalar_owner: Some(ScalarOwner(Arc::clone(owner))),
            ..Self::new()
        }
    }

    pub(crate) fn capture_scalar_grid(
        &self,
    ) -> Result<Option<gamelogic::system::shroud_manager::HostScalarGridSnapshot>, String> {
        self.scalar_owner
            .as_ref()
            .expect("receiving snapshot scalar owner")
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .capture_host_scalar_grid()
    }
    pub(crate) fn restore_scalar_grid(
        &mut self,
        snapshot: &gamelogic::system::shroud_manager::HostScalarGridSnapshot,
    ) -> Result<(), String> {
        self.scalar_owner
            .as_ref()
            .expect("receiving snapshot scalar owner")
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .restore_host_scalar_grid(snapshot)
    }

    pub(crate) fn apply_scalar_stamp(
        &mut self,
        stamp: &HostPartitionAffectStamp,
        add: bool,
    ) -> bool {
        let owner = self
            .scalar_owner
            .as_ref()
            .expect("receiving scalar operation requires its driving World's shroud service");
        let mut grid = owner.0.lock().unwrap_or_else(|error| error.into_inner());
        if !grid.has_shroud_grid() {
            return false;
        }
        let center = gamelogic::common::Coord3D::new(stamp.x, stamp.z, 0.0);
        if add {
            grid.do_value_affect(&center, stamp.range, stamp.value, stamp.mask);
            grid.do_threat_affect(&center, stamp.range, stamp.threat, stamp.mask);
        } else {
            grid.undo_value_affect(&center, stamp.range, stamp.value, stamp.mask);
            grid.undo_threat_affect(&center, stamp.range, stamp.threat, stamp.mask);
        }
        // The guard ends before the Object operation resumes any authored callback.
        true
    }
}

impl PartitionManager {
    pub(in crate::game_logic) fn scalar_cell_observation(
        &self,
        id: u32,
        x: f32,
        z: f32,
    ) -> (bool, Option<(i32, i32)>) {
        let cell = self.receiving_cell_coords(x, z);
        let cell = match self.grid {
            Some(grid)
                if cell.0 < 0
                    || cell.1 < 0
                    || cell.0 as usize >= grid.dimensions[0]
                    || cell.1 as usize >= grid.dimensions[1] =>
            {
                None
            }
            _ => Some(cell),
        };
        (
            self.object_cells
                .get(&id)
                .expect("attached cell callback")
                .last_cell
                != cell,
            cell,
        )
    }
    pub(in crate::game_logic) fn finish_scalar_cell_observation(
        &mut self,
        id: u32,
        cell: Option<(i32, i32)>,
    ) {
        self.object_cells
            .get_mut(&id)
            .expect("cell maintenance preserves attached module")
            .last_cell = cell;
    }
}

impl crate::game_logic::GameLogic {
    pub(crate) fn maintain_owned_partition_scalars(&mut self, id: crate::game_logic::ObjectId) {
        if let Some(object) = self.objects.get_mut(&id) {
            object.handle_partition_cell_maintenance_with_partition_owner(
                &mut self.partition_manager,
            );
        }
    }
}
