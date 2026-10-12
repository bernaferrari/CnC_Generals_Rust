//! C++ AIUpdate.cpp5000–5003, UpdateModule.cpp89–139.
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::update_module::SleepyUpdatePhase;
use std::io::Cursor;
#[test]
fn ai_module_saves_original_base_versions_and_encoded_clock() {
    let mut module = AIUpdateInterfaceModule::new(0, Arc::new(AIUpdateModuleData::default()));
    module.record_scheduled_wake(217, SleepyUpdatePhase::Initial);
    let mut bytes = Cursor::new(Vec::new());
    module.xfer(&mut XferSave::new(&mut bytes, 1)).unwrap();
    let mut expected = vec![4, 1, 1, 1, 1];
    expected.extend_from_slice(&(217u32 << 2).to_le_bytes());
    assert_eq!(bytes.into_inner(), expected);
}
#[test]
fn ai_module_load_normalizes_legacy_phase_and_preserves_saved_frame() {
    let mut module = AIUpdateInterfaceModule::new(0, Arc::new(AIUpdateModuleData::default()));
    let mut bytes = vec![4, 1, 1, 1, 1];
    bytes.extend_from_slice(&((223u32 << 2) | 3).to_le_bytes());
    module
        .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    assert_eq!(module.get_initial_wake_frame(), Some(223));
    assert_eq!(module.next_call_frame_and_phase, 223 << 2);
}
