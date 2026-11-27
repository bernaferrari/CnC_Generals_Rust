//! Compatibility services and snapshot adapter for the simulation side list.

use crate::common::{Snapshot, Xfer};
use crate::scripting::XferSnapshot;
use crate::scripting::chunk_codec::active_script_list_reader;
use crate::system::game_logic::SubsystemInterface;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkInput;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::system::XferVersion;
use generals_sim::sides::{STANDARD_SKIRMISH_SCRIPTS, read_standard_skirmish_scripts};
use std::sync::{Arc, RwLock};

pub use generals_sim::sides::{SidesInfo, SidesList, TeamsInfo, TeamsInfoRec};

pub fn prepare_legacy_skirmish_sides(sides: &mut SidesList) {
    sides.prepare_for_skirmish(NameKeyGenerator::name_to_key, || {
        let files = get_file_system();
        let data = files
            .lock()
            .expect("FileSystem mutex poisoned")
            .open_file(
                STANDARD_SKIRMISH_SCRIPTS,
                FileAccess::READ.combine(FileAccess::BINARY),
            )?
            .read_entire_and_close()
            .ok()?;
        let mut input = DataChunkInput::new(data);
        read_standard_skirmish_scripts(&mut input, active_script_list_reader())
    });
}

impl SubsystemInterface for SidesList {}

impl Snapshot for SidesList {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut version: XferVersion = 1;
        let _ = xfer.xfer_version(&mut version, 1);
        let mut v = self.get_num_sides() as i32;
        let _ = xfer.xfer_int(&mut v);
        for idx in 0..self.get_num_sides() {
            let mut v = self
                .get_side_info(idx)
                .is_some_and(|side| side.get_script_list().is_some());
            let _ = xfer.xfer_bool(&mut v);
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let mut side_count = self.get_num_sides() as i32;
        let _ = xfer.xfer_int(&mut side_count);
        if side_count != self.get_num_sides() as i32 {
            panic!("SidesList::xfer - The sides list size has changed, versioning required");
        }

        for idx in 0..side_count.max(0) as usize {
            let Some(side) = self.get_side_info_mut(idx) else {
                continue;
            };
            let mut script_list_present = side.get_script_list().is_some();
            let _ = xfer.xfer_bool(&mut script_list_present);
            if script_list_present {
                if side.get_script_list().is_none() {
                    panic!("SidesList::xfer - script list missing/present mismatch");
                }
                if let Some(list) = side.get_script_list_mut() {
                    let _ = list.xfer(xfer);
                }
            } else if side.get_script_list().is_some() {
                panic!("SidesList::xfer - script list missing/present mismatch");
            }
        }
    }

    fn load_post_process(&mut self) {}
}

pub fn get_sides_list() -> Arc<RwLock<SidesList>> {
    Arc::clone(crate::system::engine_stores::active_services().sides())
}
