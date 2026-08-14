//! Direct-Xfer wire layout for persistent shroud/FOW state.
//!
//! The GameLogic snapshot types own the exact raw counters and pending reveal
//! records. This module only describes their positional Common-Xfer layout so
//! the renderer and GameLogic do not depend on the save implementation.

use super::xfer_helpers::{xfer_option, xfer_vec_default};
use super::*;
use crate::save_load::{SaveLoadResult, Xfer, XferData};
use gamelogic::system::shroud_manager::{
    ShroudCellSnapshot, ShroudGridSnapshot, ShroudPendingUndoRevealSnapshot, ShroudSnapshot,
};

impl XferData for ShroudSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ShroudSnapshot")?;
        xfer.xfer_marker_label("Grid")?;
        xfer_option(xfer, &mut self.grid, ShroudGridSnapshot::default())?;
        xfer.xfer_marker_label("PendingUndoShroudReveals")?;
        xfer_vec_default(
            xfer,
            &mut self.pending_undo_shroud_reveals,
            ShroudPendingUndoRevealSnapshot::default(),
        )?;
        xfer.xfer_marker_label("PendingFullRevealPlayers")?;
        xfer_vec_default(xfer, &mut self.pending_full_reveal_players, 0u32)?;
        xfer.xfer_marker_label("PendingPermanentRevealPlayers")?;
        xfer_vec_default(xfer, &mut self.pending_permanent_reveal_players, 0u32)?;
        Ok(())
    }
}

impl XferData for ShroudGridSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ShroudGridSnapshot")?;
        xfer.xfer_marker_label("Width")?;
        xfer.xfer_u32(&mut self.width)?;
        xfer.xfer_marker_label("Height")?;
        xfer.xfer_u32(&mut self.height)?;
        xfer.xfer_marker_label("CellSize")?;
        xfer.xfer_f32(&mut self.cell_size)?;
        xfer.xfer_marker_label("Cells")?;
        xfer_vec_default(xfer, &mut self.cells, ShroudCellSnapshot::default())
    }
}

impl XferData for ShroudCellSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ShroudCellSnapshot")?;
        xfer.xfer_marker_label("CurrentShroud")?;
        for value in &mut self.current_shroud {
            xfer.xfer_i32(value)?;
        }
        xfer.xfer_marker_label("ActiveShroudLevel")?;
        for value in &mut self.active_shroud_level {
            xfer.xfer_i32(value)?;
        }
        Ok(())
    }
}

impl XferData for ShroudPendingUndoRevealSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ShroudPendingUndoRevealSnapshot")?;
        xfer.xfer_marker_label("WherePos")?;
        for value in &mut self.where_pos {
            xfer.xfer_f32(value)?;
        }
        xfer.xfer_marker_label("HowFar")?;
        xfer.xfer_f32(&mut self.how_far)?;
        xfer.xfer_marker_label("ForWhom")?;
        xfer.xfer_u32(&mut self.for_whom)?;
        xfer.xfer_marker_label("ExpirationFrame")?;
        xfer.xfer_u32(&mut self.expiration_frame)
    }
}
