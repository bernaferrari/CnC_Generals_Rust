use super::{CommonXferBridge, LogicXferCrc};
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer as CommonXfer, XferStatus as CommonStatus};
use game_engine::{Xfer, XferLoad, XferSave};
use std::cell::Cell;

struct Leaf {
    value: u32,
    visits: usize,
    crc_visits: Cell<usize>,
    post_load_visits: usize,
}

impl Leaf {
    fn new(value: u32) -> Self {
        Self {
            value,
            visits: 0,
            crc_visits: Cell::new(0),
            post_load_visits: 0,
        }
    }
}

impl Snapshotable for Leaf {
    fn xfer(&mut self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
        self.visits += 1;
        xfer.xfer_unsigned_int(&mut self.value)
            .map_err(|e| e.to_string())
    }

    fn crc(&self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
        self.crc_visits.set(self.crc_visits.get() + 1);
        // Distinct bytes make an incorrect crc -> xfer dispatch observable.
        let mut value = self.value ^ 0x1357_2468;
        xfer.xfer_unsigned_int(&mut value)
            .map_err(|e| e.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.post_load_visits += 1;
        Ok(())
    }
}

struct Parent {
    leaf: Leaf,
    tail: u32,
}

impl Snapshotable for Parent {
    fn xfer(&mut self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
        xfer.xfer_snapshot(&mut self.leaf)
            .map_err(|e| format!("{e:?}"))?;
        xfer.xfer_unsigned_int(&mut self.tail)
            .map_err(|e| e.to_string())
    }

    fn crc(&self, xfer: &mut dyn CommonXfer) -> Result<(), String> {
        self.leaf.crc(xfer)?;
        let mut tail = self.tail;
        xfer.xfer_unsigned_int(&mut tail).map_err(|e| e.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct FixtureFile(std::path::PathBuf);

impl Drop for FixtureFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn nested_snapshot_bridge_preserves_save_bytes_and_loads_existing_instance() {
    // XferSave.cpp:249-263 and XferLoad.cpp:150-168 visit the supplied snapshot.
    let file = FixtureFile(std::env::temp_dir().join(format!(
        "generals_nested_snapshot_bridge_{}.bin",
        std::process::id()
    )));
    let mut source = Parent {
        leaf: Leaf::new(0x1928_3746),
        tail: 0x5566_7788,
    };
    let mut save = XferSave::new();
    save.open(file.0.to_string_lossy().into_owned()).unwrap();
    CommonXferBridge { inner: &mut save }
        .xfer_snapshot(&mut source)
        .unwrap();
    save.close().unwrap();
    let expected: Vec<_> = source
        .leaf
        .value
        .to_le_bytes()
        .into_iter()
        .chain(source.tail.to_le_bytes())
        .collect();
    assert_eq!(std::fs::read(&file.0).unwrap(), expected);
    assert_eq!(source.leaf.visits, 1);

    let mut destination = Parent {
        leaf: Leaf::new(0),
        tail: 0,
    };
    let mut load = XferLoad::new();
    load.open(file.0.to_string_lossy().into_owned()).unwrap();
    CommonXferBridge { inner: &mut load }
        .xfer_snapshot(&mut destination)
        .unwrap();
    load.close().unwrap();
    assert_eq!(destination.leaf.value, source.leaf.value);
    assert_eq!(destination.tail, source.tail);
    assert_eq!(destination.leaf.visits, 1);
    assert_eq!(destination.leaf.crc_visits.get(), 0);
    // C++ queues post-processing for later; do not run it inside this visitor.
    // Registration of that later pass remains a separate owner migration.
    assert_eq!(destination.leaf.post_load_visits, 0);
}

#[test]
fn nested_snapshot_bridge_crc_dispatches_crc_instead_of_save_payload() {
    // XferCRC.cpp:98-111 dispatches crc, unlike XferSave/Load.
    let mut leaf = Leaf::new(0x1928_3746);
    let mut actual = LogicXferCrc::new();
    CommonXferBridge { inner: &mut actual }
        .xfer_snapshot(&mut leaf)
        .unwrap();
    let mut expected = LogicXferCrc::new();
    let mut expected_value = leaf.value ^ 0x1357_2468;
    Xfer::xfer_unsigned_int(&mut expected, &mut expected_value).unwrap();
    assert_eq!(actual.get_crc(), expected.get_crc());
    assert_ne!(actual.get_crc(), LogicXferCrc::new().get_crc());
    assert_eq!(leaf.crc_visits.get(), 1);
    assert_eq!(leaf.visits, 0);
}

struct BrokenSnapshot;
impl Snapshotable for BrokenSnapshot {
    fn xfer(&mut self, _: &mut dyn CommonXfer) -> Result<(), String> {
        Err("bad payload".into())
    }
    fn crc(&self, _: &mut dyn CommonXfer) -> Result<(), String> {
        Err("bad crc".into())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        panic!("must be deferred")
    }
}

#[test]
fn nested_snapshot_bridge_propagates_mode_specific_failures() {
    let mut snapshot = BrokenSnapshot;
    let mut save = XferSave::new();
    assert_eq!(
        CommonXferBridge { inner: &mut save }.xfer_snapshot(&mut snapshot),
        Err(CommonStatus::WriteError)
    );
    let mut load = XferLoad::new();
    assert_eq!(
        CommonXferBridge { inner: &mut load }.xfer_snapshot(&mut snapshot),
        Err(CommonStatus::ReadError)
    );
    let mut crc = LogicXferCrc::new();
    assert_eq!(
        CommonXferBridge { inner: &mut crc }.xfer_snapshot(&mut snapshot),
        Err(CommonStatus::InvalidData)
    );
}
