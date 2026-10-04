//! C++ Module.h:102-104 leaves ModuleData snapshots empty. These are
//! definition hooks; the separate Module runtime keeps its version headers.
use super::*;
use crate::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[test]
fn base_definition_save_writes_no_version_or_tag() {
    let mut data = BaseModuleData::new();
    data.set_module_tag_name_key(0x1234_5678);
    let mut bytes = Vec::new();
    data.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    assert!(bytes.is_empty(), "C++ ModuleData has no snapshot payload");
    assert_eq!(data.get_module_tag_name_key(), 0x1234_5678);
}

#[test]
fn base_definition_load_leaves_the_next_record_unread() {
    let mut data = BaseModuleData::new();
    data.set_module_tag_name_key(37);
    let sentinel = 0x7856_3401_u32;
    let mut xfer = XferLoad::new(Cursor::new(sentinel.to_le_bytes()), 1);
    data.xfer(&mut xfer).unwrap();
    assert_eq!(xfer.bytes_read(), 0, "C++ ModuleData reads no version");
    data.load_post_process().unwrap();
    assert_eq!(data.get_module_tag_name_key(), 37);
    let mut next = 0;
    xfer.xfer_unsigned_int(&mut next).unwrap();
    assert_eq!(next, sentinel);
}
