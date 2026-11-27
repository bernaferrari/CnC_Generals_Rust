//! Save trait adapter for the simulation build-list entry.

use crate::common::{Snapshot, Xfer};

pub use generals_sim::build_list::BuildListInfo;

impl Snapshot for BuildListInfo {
    fn crc(&self, xfer: &mut dyn Xfer) {
        self.crc_xfer(xfer);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        self.transfer_xfer(xfer);
    }

    fn load_post_process(&mut self) {}
}
