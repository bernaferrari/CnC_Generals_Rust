//! Compatibility for legacy script, AI, and Xfer callers.

use crate::common::{Coord3D, Snapshot};
use crate::helpers::TheTerrainLogic;
use game_engine::common::system::{Snapshotable, Xfer};

pub use game_engine::polygon_trigger::{
    PolygonTrigger, PolygonTriggerId, PolygonTriggerList, WaterHandle,
};

pub(crate) fn terrain_center(trigger: &PolygonTrigger) -> Coord3D {
    trigger.get_center_point(|x, y| {
        TheTerrainLogic::get().map_or(0.0, |terrain| terrain.get_ground_height(x, y, None))
    })
}

impl Snapshot for PolygonTrigger {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let _ = Snapshotable::crc(self, xfer);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let _ = Snapshotable::xfer(self, xfer);
    }

    fn load_post_process(&mut self) {
        let _ = Snapshotable::load_post_process(self);
    }
}
