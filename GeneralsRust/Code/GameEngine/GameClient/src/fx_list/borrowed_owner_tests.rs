//! C++ FXList.cpp794-805: dispatch the supplied Object to each object nugget.

use super::*;
use gamelogic::common::{DefaultThingTemplate, ObjectStatusMaskType};
use gamelogic::object::registry::OBJECT_REGISTRY;

struct ObjectNugget {
    calls: std::sync::mpsc::Sender<String>,
}

impl FXNugget for ObjectNugget {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("actual bridge must preserve FXNugget::doFXObj");
    }

    fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        assert!(secondary.is_none(), "upgrade FX has no secondary object");
        let owner = primary.expect("actual borrowed owner supplied to nugget");
        assert_eq!(owner.get_id(), 0x00F8_0B10);
        let name = owner.get_template().get_name().as_str().to_owned();
        // A synchronous nugget can admit another FX definition. The bridge
        // must retain the immutable FX Arc, not the catalog read guard.
        let _catalog = FX_LIST_STORE
            .get()
            .unwrap()
            .try_write()
            .expect("FX definition lookup guard must end before callback");
        self.calls.send(name).unwrap();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn actual_bridge_uses_borrowed_owner_and_never_relocks_admitted_owner() {
    const CHILD_MARKER: &str = "GENERALS_BORROWED_FX_OWNER_CHILD";
    const TEST_NAME: &str = "fx_list::borrowed_owner_tests::actual_bridge_uses_borrowed_owner_and_never_relocks_admitted_owner";
    if std::env::var_os(CHILD_MARKER).is_none() {
        run_child(TEST_NAME, CHILD_MARKER);
        return;
    }

    // The fresh child owns this unique catalog definition and all singleton
    // inputs for their entire lifetime; the parent process is never mutated.
    const OBJECT_ID: u32 = 0x00F8_0B10;
    const FX_NAME: &str = "FX_BorrowedOwnerContract";
    assert!(OBJECT_REGISTRY.get_object(OBJECT_ID).is_none());
    assert!(get_fx_list_store().find_fx_list(FX_NAME).is_none());
    gamelogic::player::player_list()
        .write()
        .unwrap()
        .set_local_player_index(0);
    let (recorded, calls) = std::sync::mpsc::channel();
    let mut observed = Vec::new();
    let mut fx = FXList::new();
    fx.add_fx_nugget(Box::new(ObjectNugget { calls: recorded }));
    get_fx_list_store_mut().add_fx_list(FX_NAME.into(), fx);
    let fx_id = NameKeyGenerator::name_to_key(FX_NAME) as FXListId;

    let foreign = Arc::new(RwLock::new(Object::new_raw(
        Arc::new(DefaultThingTemplate::new("FX_ForeignOwner".into())),
        OBJECT_ID,
        ObjectStatusMaskType::none(),
        None,
    )));
    let mut owner = gamelogic::system::game_logic::GameLogic::new();
    owner.register_object(Arc::clone(&foreign)).unwrap();
    let borrowed = Arc::new(RwLock::new(Object::new_raw(
        Arc::new(DefaultThingTemplate::new("FX_BorrowedOwner".into())),
        OBJECT_ID,
        ObjectStatusMaskType::none(),
        None,
    )));
    {
        let borrowed_guard = borrowed.write().unwrap();
        FXListManagerBridge.do_fx_for_object(fx_id, &borrowed_guard);
    }
    observed.extend(calls.try_iter());
    assert_eq!(observed, vec!["FX_BorrowedOwner"]);
    {
        // Old ID rediscovery would deadlock on this exact owner write guard.
        let foreign_guard = foreign.write().unwrap();
        FXListManagerBridge.do_fx_for_object(fx_id, &foreign_guard);
    }
    observed.extend(calls.try_iter());
    assert_eq!(observed, vec!["FX_BorrowedOwner", "FX_ForeignOwner"]);
    owner.destroy_object(OBJECT_ID);
    owner.process_destroy_list().unwrap();
    assert!(owner.find_object_by_id(OBJECT_ID).is_none());
    assert!(OBJECT_REGISTRY.get_object(OBJECT_ID).is_none());
    assert_eq!(foreign.read().unwrap().get_id(), 0);
    {
        // Explicit ownership also works after the foreign admission retires;
        // an empty global registry is not permission to discard the owner.
        let borrowed_guard = borrowed.write().unwrap();
        FXListManagerBridge.do_fx_for_object(fx_id, &borrowed_guard);
    }
    observed.extend(calls.try_iter());
    assert_eq!(
        observed,
        vec!["FX_BorrowedOwner", "FX_ForeignOwner", "FX_BorrowedOwner"]
    );
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run_child(test_name: &str, marker: &str) {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([test_name, "--exact", "--test-threads=1", "--nocapture"])
        .env(marker, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn isolated borrowed FX regression");
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut output = Vec::new();
            pipe.read_to_end(&mut output).map(|_| output)
        })
    };
    let readers = [read(Box::new(stdout)), read(Box::new(stderr))];
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().expect("reap stalled FX child");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let [stdout, stderr] = readers.map(|reader| reader.join().unwrap().unwrap());
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(!timed_out, "borrowed FX child timed out: {stdout}{stderr}");
    assert!(
        status.success(),
        "borrowed FX child failed: {stdout}{stderr}"
    );
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "exact child must run one test: {stdout}{stderr}"
    );
}
