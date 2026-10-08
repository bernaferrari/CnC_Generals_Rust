//! Local receiving GameLogic + actual file-backed System XferLoad. The registered
//! service bridge's global mutex/liveness is deliberately outside this fixture.
use super::*;
use game_engine::{
    Snapshot as NativeSnapshot, Xfer as NativeXfer, XferLoad as NativeLoad, XferSave as NativeSave,
    XferStatus as NativeStatus,
};
use gamelogic::system::game_logic::GameLogic as NativeLogic;
use std::path::PathBuf;

const NATIVE_NAME: &str = "NativeModuleErrorReceivingFixture";
const SECOND_ID: u32 = ID + 1;

// Supplies only the trait-object boundary required by the real System loader.
// Both this adapter and the production registered bridge call the same facade.
struct LocalSnapshot<'a>(&'a mut NativeLogic);

impl NativeSnapshot for LocalSnapshot<'_> {
    fn crc(&mut self, _: &mut dyn NativeXfer) -> Result<(), NativeStatus> {
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn NativeXfer) -> Result<(), NativeStatus> {
        self.0.xfer_native_snapshot(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), NativeStatus> {
        panic!("fixture observes deferred registration, not execution of native postprocessing")
    }
}

fn path(case: &str, suffix: &str) -> PathBuf {
    let directory = std::env::var_os("GENERALS_NATIVE_MODULE_ERROR_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    directory.join(format!("{case}-{}-{suffix}.bin", std::process::id()))
}

fn assert_catalog_pair(
    base: &dyn ThingTemplate,
    native: &Arc<EngineThingTemplate>,
    head: &Arc<EngineThingTemplate>,
) {
    let authored = base.as_any().downcast_ref::<AuthoredTemplate>().unwrap();
    let guard = try_get_thing_factory().unwrap();
    let factory = guard.as_ref().unwrap();
    // The parser's current copy-on-write path replaces the map entry but
    // retains its initial linked-list shell. Pin both real identities; do not
    // repair the production catalog merely to satisfy this fixture.
    assert!(Arc::ptr_eq(factory.first_template().unwrap(), head));
    assert!(!Arc::ptr_eq(head, native));
    assert_eq!(head.get_name().as_str(), NATIVE_NAME);
    assert_eq!(head.get_template_id(), 2);
    assert!(Arc::ptr_eq(
        head.get_next_template().as_ref().unwrap(),
        &authored.catalog_sentinel,
    ));
    assert_eq!(native.get_name().as_str(), NATIVE_NAME);
    assert_eq!(native.get_template_id(), 2);
    let marker = native.get_next_template().as_ref().unwrap();
    assert!(Arc::ptr_eq(marker, &authored.catalog_sentinel));
    assert_eq!(marker.get_name().as_str(), CATALOG_SENTINEL);
    assert!(marker.get_next_template().is_none());
    assert!(Arc::ptr_eq(
        &factory.find_template(NATIVE_NAME, false).unwrap(),
        native
    ));
    assert!(factory.find_template("", false).is_none());
    assert!(factory.find_template("GenericTracer", false).is_none());
    assert!(factory.find_template("GenericRope", false).is_none());
    assert!(generals_main::assets::get_asset_manager().is_none());
    assert_eq!(TheArmorStore::read().len(), 1);
    with_weapon_store(|store| {
        assert_eq!(store.get_template_count(), 0);
        assert!(store.host_bootstrap_is_complete());
    })
    .unwrap();
}

fn assert_admitted(object: &Arc<RwLock<Object>>, expected_id: u32) {
    let object = object.read().unwrap();
    assert_eq!(object.get_id(), expected_id);
    assert_eq!(object.get_template().get_name().as_str(), NATIVE_NAME);
    assert!(object.has_ctor_helpers());
    let body = object.get_body_module().unwrap();
    assert_eq!(body.lock().unwrap().get_health(), 100.0);
    let modules = object.behavior_modules();
    assert_eq!(
        modules.iter().map(|m| m.tag().as_str()).collect::<Vec<_>>(),
        [BODY_TAG, AI_TAG, SENTINEL_TAG]
    );
    assert!(modules[0].with_module(|m| m.get_module_data().as_any().is::<ActiveBodyModuleData>()));
    assert!(
        modules[1]
            .with_module_downcast::<AIUpdateInterfaceModule, _, _>(|_| ())
            .is_some()
    );
    let mut ai = Vec::new();
    modules[1]
        .with_module(|m| m.xfer(&mut XferSave::new(Cursor::new(&mut ai), 1)))
        .unwrap();
    assert_eq!(
        ai,
        [4],
        "no-neutral-team fixture requires matching runtime-free AI envelopes"
    );
    assert!(
        modules[2]
            .with_module_downcast::<SentinelModule, _, _>(|_| ())
            .is_some()
    );
}

fn occurrences(bytes: &[u8], pattern: &[u8]) -> Vec<usize> {
    bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(i, part)| (part == pattern).then_some(i))
        .collect()
}

fn module_payloads(object: &Arc<RwLock<Object>>) -> Vec<(String, Vec<u8>)> {
    object
        .read()
        .unwrap()
        .behavior_modules()
        .into_iter()
        .map(|module| {
            let mut bytes = Vec::new();
            module
                .with_module(|value| value.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1)))
                .unwrap();
            (module.tag().to_string(), bytes)
        })
        .collect()
}

pub(super) fn execute(case: &str) {
    let base = authored_template();
    let ini = format!(
        "Object {NATIVE_NAME}\n KindOf = INERT\n Body = ActiveBody {BODY_TAG}\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface {AI_TAG}\n End\n Behavior = {SENTINEL_NAME} {SENTINEL_TAG}\n End\nEnd\n"
    );
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&ini),
        1
    );
    let native = get_thing_factory()
        .unwrap()
        .as_ref()
        .unwrap()
        .find_template(NATIVE_NAME, false)
        .unwrap();
    let head = get_thing_factory()
        .unwrap()
        .as_ref()
        .unwrap()
        .first_template()
        .unwrap()
        .clone();
    assert_catalog_pair(base.as_ref(), &native, &head);
    // Match the actual loader branch instead of taking a template-miss skip or
    // invoking the team/factory route with different AI reconstruction policy.
    assert!(
        gamelogic::player::player_list()
            .read()
            .unwrap()
            .get_neutral_player()
            .and_then(|p| p.read().unwrap().get_default_team())
            .is_none()
    );
    let template = gamelogic::helpers::TheThingFactory::find_template(NATIVE_NAME).unwrap();
    let definitions = template.get_behavior_module_info();
    assert_eq!(
        definitions
            .iter()
            .map(|m| m.module_tag.as_str())
            .collect::<Vec<_>>(),
        [BODY_TAG, AI_TAG, SENTINEL_TAG]
    );
    let probe = definitions[2]
        .data
        .as_any()
        .downcast_ref::<SentinelData>()
        .unwrap()
        .probe
        .clone();
    assert_eq!(probe.created.load(Ordering::SeqCst), 0);
    let first =
        Object::new_with_id(template.clone(), ID, ObjectStatusMaskType::NONE, None).unwrap();
    let second =
        Object::new_with_id(template, SECOND_ID, ObjectStatusMaskType::NONE, None).unwrap();
    assert_admitted(&first, ID);
    assert_admitted(&second, SECOND_ID);
    let mut writer = NativeLogic::new();
    // Native registration prepends; ID must be the first serialized record.
    writer.register_object(second.clone()).unwrap();
    writer.register_object(first.clone()).unwrap();
    assert_eq!(writer.get_object_count(), 2);
    let first_saved = super::save(&first);
    let second_saved = super::save(&second);
    let first_modules = module_payloads(&first);
    let second_modules = module_payloads(&second);
    let valid_path = path(case, "valid");
    {
        let mut save = NativeSave::new();
        save.open(valid_path.to_string_lossy().into_owned())
            .unwrap();
        writer.xfer_native_snapshot(&mut save).unwrap();
        let mut outer = OUTER_SENTINEL;
        save.xfer_unsigned_int(&mut outer).unwrap();
        save.close().unwrap();
    }
    let valid = std::fs::read(&valid_path).unwrap();
    assert_eq!(valid[0], 10, "actual native GameLogic current version");
    let mut ai_prefix = vec![AI_TAG.len() as u8];
    ai_prefix.extend_from_slice(AI_TAG.as_bytes());
    let ai_rows = occurrences(&valid, &ai_prefix);
    assert_eq!(ai_rows.len(), 2);
    let payloads: Vec<_> = ai_rows
        .iter()
        .map(|row| {
            let size = row + ai_prefix.len();
            assert_eq!(&valid[size..size + 4], &1i32.to_le_bytes());
            assert_eq!(valid[size + 4], 4);
            size + 4
        })
        .collect();
    let object_header = |id: u32| {
        let mut bytes = vec![9];
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes
    };
    let first_positions = occurrences(&valid, &object_header(ID));
    let second_positions = occurrences(&valid, &object_header(SECOND_ID));
    assert_eq!(first_positions.len(), 1);
    assert_eq!(second_positions.len(), 1);
    assert!(first_positions[0] < payloads[0] && payloads[0] < second_positions[0]);
    assert!(second_positions[0] < payloads[1]);
    let malformed = case.ends_with("native_loader_returns_error_before_receiver_registration");
    let mut input = valid.clone();
    if malformed {
        input[payloads[0]] = 5;
        assert_eq!(
            input
                .iter()
                .zip(&valid)
                .enumerate()
                .filter_map(|(i, (a, b))| (a != b).then_some(i))
                .collect::<Vec<_>>(),
            [payloads[0]]
        );
    }
    let input_path = path(case, "input");
    std::fs::write(&input_path, &input).unwrap();
    drop(writer);
    drop(first);
    drop(second);
    for id in [ID, SECOND_ID] {
        OBJECT_REGISTRY.unregister_object(id);
        gamelogic::ai::object_registry::unregister_legacy_object(id);
    }
    let created_before = probe.created.load(Ordering::SeqCst);
    let loaded_before = probe.loaded.load(Ordering::SeqCst);
    assert_eq!(created_before, 2);
    assert_eq!(loaded_before, 0);
    let mut receiver = NativeLogic::new();
    assert_eq!(receiver.get_object_count(), 0);
    assert!(
        gamelogic::player::player_list()
            .read()
            .unwrap()
            .get_neutral_player()
            .and_then(|p| p.read().unwrap().get_default_team())
            .is_none(),
        "verify the actual receiving constructor branch immediately before load"
    );
    let queued = Arc::new(AtomicUsize::new(0));
    let mut load = NativeLoad::new();
    load.open(input_path.to_string_lossy().into_owned())
        .unwrap();
    let queued_callback = queued.clone();
    load.set_post_process_snapshot_callback(Some(Box::new(move || {
        queued_callback.fetch_add(1, Ordering::SeqCst);
    })));
    let result = load.xfer_snapshot(&mut LocalSnapshot(&mut receiver));
    if malformed {
        assert_eq!(result, Err(NativeStatus::ReadError));
        assert_eq!(
            receiver.get_object_count(),
            0,
            "only this receiving store is claimed empty"
        );
        assert!(receiver.find_object_by_id(ID).is_none());
        assert!(receiver.find_object_by_id(SECOND_ID).is_none());
        assert_eq!(
            probe.created.load(Ordering::SeqCst) - created_before,
            1,
            "actual first destination constructed; second not reached"
        );
        assert_eq!(
            probe.loaded.load(Ordering::SeqCst) - loaded_before,
            0,
            "following module not transferred"
        );
        assert_eq!(
            queued.load(Ordering::SeqCst),
            0,
            "real System loader must not queue failed snapshot postprocessing"
        );
        // The real file reader has no public position getter. Exact remaining
        // bytes prove its cursor without substituting any reader/helper shim.
        let mut remaining = Vec::new();
        loop {
            let mut byte = 0;
            match load.xfer_unsigned_byte(&mut byte) {
                Ok(()) => remaining.push(byte),
                Err(status) => {
                    assert!(matches!(
                        status,
                        NativeStatus::Eof | NativeStatus::ReadError
                    ));
                    break;
                }
            }
        }
        assert_eq!(remaining, input[payloads[0] + 1..]);
        println!(
            "native_error={result:?} exact_cursor={} receiver_objects=0 destination_constructions=1 following_module_loads=0 queued_postprocess=0 unread_bytes={}",
            payloads[0] + 1,
            remaining.len()
        );
    } else {
        result.unwrap();
        assert_eq!(receiver.get_object_count(), 2);
        for id in [ID, SECOND_ID] {
            assert_admitted(&receiver.find_object_by_id(id).unwrap(), id);
        }
        assert_eq!(probe.created.load(Ordering::SeqCst) - created_before, 2);
        assert_eq!(probe.loaded.load(Ordering::SeqCst) - loaded_before, 2);
        assert_eq!(queued.load(Ordering::SeqCst), 1);
        let mut outer = 0;
        load.xfer_unsigned_int(&mut outer).unwrap();
        assert_eq!(outer, OUTER_SENTINEL);
        let mut byte = 0;
        assert!(load.xfer_unsigned_byte(&mut byte).is_err());
        let resave_path = path(case, "resave");
        let mut save = NativeSave::new();
        save.open(resave_path.to_string_lossy().into_owned())
            .unwrap();
        receiver.xfer_native_snapshot(&mut save).unwrap();
        save.xfer_unsigned_int(&mut outer).unwrap();
        save.close().unwrap();
        // Preserve exact module fields. Registration creates new companion
        // drawables, so whole-Object/native-container resave identity is outside
        // this control. Record any byte differences without repairing them.
        let loaded_first = receiver.find_object_by_id(ID).unwrap();
        let loaded_second = receiver.find_object_by_id(SECOND_ID).unwrap();
        assert_eq!(module_payloads(&loaded_first), first_modules);
        assert_eq!(module_payloads(&loaded_second), second_modules);
        for (id, before, loaded) in [
            (ID, first_saved, loaded_first),
            (SECOND_ID, second_saved, loaded_second),
        ] {
            let after = super::save(&loaded);
            let offsets: Vec<_> = before
                .iter()
                .zip(&after)
                .enumerate()
                .filter_map(|(i, (a, b))| (a != b).then_some(i))
                .collect();
            std::fs::write(path(case, &format!("object-{id}-before")), &before).unwrap();
            std::fs::write(path(case, &format!("object-{id}-after")), &after).unwrap();
            println!(
                "native_object_resave_observation id={id} before_bytes={} after_bytes={} differing_offsets={offsets:?}; companion identity equivalence is not asserted",
                before.len(),
                after.len()
            );
        }
        println!(
            "native_valid receiver_objects=2 destination_constructions=2 following_module_loads=2 queued_postprocess=1 outer_sentinel={outer:#x} input_bytes={}",
            input.len()
        );
    }
    load.close().unwrap();
    assert_catalog_pair(base.as_ref(), &native, &head);
}

#[test]
fn native_loader_returns_error_before_receiver_registration() {
    bounded_child("native_loader::native_loader_returns_error_before_receiver_registration");
}

#[test]
fn native_loader_accepts_matching_current_object_records() {
    bounded_child("native_loader::native_loader_accepts_matching_current_object_records");
}
