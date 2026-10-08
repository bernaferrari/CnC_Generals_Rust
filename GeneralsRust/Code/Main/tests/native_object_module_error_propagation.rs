//! Actual Object/module error propagation, deliberately limited to the runtime-free
//! AIUpdateInterface wrapper. Its complete payload is the version byte [4].
//! This does not exercise nested AI state machines, old pickup saves, the native
//! GameLogic registration bridge, or whole-load rollback. C++ Object.cpp:4264-4356
//! supplies the tagged module framing; Common/System/Xfer.cpp rejects newer versions.
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::system::{Snapshotable, Xfer, XferMode};
use game_engine::common::thing::module::{
    BaseModuleData, Module, ModuleData, ModuleInterfaceType, ModuleType, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    get_module_factory, init_module_factory, register_module_override,
};
use game_engine::common::thing::thing_factory::{
    ensure_thing_factory_exists, get_thing_factory, try_get_thing_factory,
};
use game_engine::common::thing::thing_template::ThingTemplate as EngineThingTemplate;
use gamelogic::common::{
    AsciiString, DefaultThingTemplate, GeometryInfo, KindOf, ObjectStatusMaskType, Real, Snapshot,
    TemplateModuleInfo, ThingTemplate,
};
use gamelogic::object::Object;
use gamelogic::object::armor::{TheArmorStore, load_armor_templates_from_str};
use gamelogic::object::body::active_body::ActiveBodyModuleData;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::update::ai_update_interface::{AIUpdateInterfaceModule, AIUpdateModuleData};
use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut};
use std::io::Cursor;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const ID: u32 = 0x73F0_0101;
const BODY_TAG: &str = "ModuleTag_NativeErrorBody";
const AI_TAG: &str = "ModuleTag_NativeErrorAI";
const SENTINEL_TAG: &str = "ModuleTag_NativeErrorSentinel";
const SENTINEL_NAME: &str = "NativeErrorSentinel";
const MODULE_SENTINEL: u32 = 0x62D4_A731;
const OUTER_SENTINEL: u32 = 0xE145_B92C;
const CHILD_ENV: &str = "GENERALS_NATIVE_MODULE_ERROR_CHILD";
const CATALOG_SENTINEL: &str = "NativeErrorUnusedCatalogSentinel";

#[path = "native_object_module_error_propagation/native_loader.rs"]
mod native_loader;

#[derive(Debug, Default)]
struct SentinelProbe {
    created: AtomicUsize,
    loaded: AtomicUsize,
}

#[derive(Debug)]
struct SentinelData {
    base: BaseModuleData,
    probe: Arc<SentinelProbe>,
}

impl SentinelData {
    fn new() -> Self {
        Self {
            base: BaseModuleData::new(),
            probe: Arc::new(SentinelProbe::default()),
        }
    }
}

impl ModuleData for SentinelData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: u32) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> u32 {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for SentinelData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

struct SentinelModule {
    data: Arc<dyn ModuleData>,
    value: u32,
    loads: usize,
    probe: Arc<SentinelProbe>,
}

impl Module for SentinelModule {
    fn get_module_name_key(&self) -> u32 {
        NameKeyGenerator::name_to_key(SENTINEL_NAME)
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for SentinelModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut value = self.value;
        xfer.xfer_unsigned_int(&mut value)
            .map_err(|e| e.to_string())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_unsigned_int(&mut self.value)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.loads += 1;
            self.probe.loaded.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn sentinel_factory(_: Arc<dyn ModuleThing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    let probe = data
        .as_any()
        .downcast_ref::<SentinelData>()
        .unwrap()
        .probe
        .clone();
    probe.created.fetch_add(1, Ordering::SeqCst);
    Box::new(SentinelModule {
        data,
        value: MODULE_SENTINEL,
        loads: 0,
        probe,
    })
}

fn sentinel_data(_: Option<&mut game_engine::common::ini::INI>) -> Box<dyn ModuleData> {
    Box::new(SentinelData::new())
}

struct AuthoredTemplate {
    inner: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
    catalog_sentinel: Arc<EngineThingTemplate>,
}

impl std::fmt::Debug for AuthoredTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthoredTemplate")
            .field("name", self.inner.get_name())
            .finish()
    }
}

impl ThingTemplate for AuthoredTemplate {
    fn get_name(&self) -> &AsciiString {
        self.inner.get_name()
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.inner.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> Real {
        self.inner.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> Real {
        self.inner.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.inner.is_kind_of(kind)
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}

fn authored_template() -> Arc<dyn ThingTemplate> {
    assert!(generals_main::assets::get_asset_manager().is_none());
    gamelogic::initialize_weapon_store().unwrap();
    with_weapon_store_mut(|store| {
        assert_eq!(store.get_template_count(), 0);
        store.mark_host_bootstrap_complete();
    })
    .unwrap();
    assert_eq!(
        load_armor_templates_from_str(
            "Armor NativeErrorUnusedArmor\nArmor = DEFAULT 100%\nEnd\n",
            None,
        )
        .unwrap(),
        1
    );
    gamelogic::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    register_module_override(
        SENTINEL_NAME,
        ModuleType::Behavior,
        sentinel_factory,
        sentinel_data,
    )
    .unwrap();
    init_module_factory().unwrap();
    get_module_factory()
        .unwrap()
        .as_mut()
        .unwrap()
        .add_module_internal(
            Some(sentinel_factory),
            Some(sentinel_data),
            ModuleType::Behavior,
            SENTINEL_NAME,
            ModuleInterfaceType::NONE,
        );
    // ActiveBody restore resolves even an empty last-damage template name.
    // An absent OR empty native catalog retries full runtime asset discovery.
    // Admit one explicit, unused in-memory definition before either route so
    // the real lookup stays inside this bounded catalog. Do not call init.
    assert!(try_get_thing_factory().unwrap().is_none());
    assert!(ensure_thing_factory_exists());
    let catalog_sentinel = get_thing_factory()
        .unwrap()
        .as_mut()
        .unwrap()
        .new_template(CATALOG_SENTINEL);
    let mut body = ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    body.set_module_tag_name_key(NameKeyGenerator::name_to_key(BODY_TAG));
    let mut ai = AIUpdateModuleData::default();
    ai.set_module_tag_name_key(NameKeyGenerator::name_to_key(AI_TAG));
    let mut sentinel = SentinelData::new();
    sentinel.set_module_tag_name_key(NameKeyGenerator::name_to_key(SENTINEL_TAG));
    Arc::new(AuthoredTemplate {
        inner: DefaultThingTemplate::new("NativeModuleErrorObject".into()),
        catalog_sentinel,
        modules: vec![
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: BODY_TAG.into(),
                data: Arc::new(body),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: AI_TAG.into(),
                data: Arc::new(ai),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: SENTINEL_NAME.into(),
                module_tag: SENTINEL_TAG.into(),
                data: Arc::new(sentinel),
                interface_mask: ModuleInterfaceType::NONE,
            },
        ],
    })
}

fn assert_catalog(template: &dyn ThingTemplate) {
    assert!(generals_main::assets::get_asset_manager().is_none());
    {
        let authored = template
            .as_any()
            .downcast_ref::<AuthoredTemplate>()
            .unwrap();
        let guard = try_get_thing_factory().unwrap();
        let factory = guard.as_ref().unwrap();
        let first = factory.first_template().unwrap();
        assert!(Arc::ptr_eq(first, &authored.catalog_sentinel));
        assert_eq!(first.get_name().as_str(), CATALOG_SENTINEL);
        assert_eq!(first.get_template_id(), 1);
        assert!(first.get_next_template().is_none());
        assert!(Arc::ptr_eq(
            &factory.find_template(CATALOG_SENTINEL, false).unwrap(),
            &authored.catalog_sentinel,
        ));
        for absent in ["", "GenericTracer", "GenericRope"] {
            assert!(factory.find_template(absent, false).is_none());
        }
    }
    assert_eq!(TheArmorStore::read().len(), 1);
    with_weapon_store(|store| {
        assert_eq!(store.get_template_count(), 0);
        assert!(store.host_bootstrap_is_complete());
    })
    .unwrap();
    let modules = template.get_behavior_module_info();
    assert_eq!(modules.len(), 3);
    for (entry, (name, tag)) in modules.iter().zip([
        ("ActiveBody", BODY_TAG),
        ("AIUpdateInterface", AI_TAG),
        (SENTINEL_NAME, SENTINEL_TAG),
    ]) {
        assert_eq!(entry.name.as_str(), name);
        assert_eq!(entry.module_tag.as_str(), tag);
        assert_eq!(
            NameKeyGenerator::key_to_name(entry.data.get_module_tag_name_key()).as_deref(),
            Some(tag)
        );
    }
}

fn object(template: &Arc<dyn ThingTemplate>) -> Arc<RwLock<Object>> {
    assert_catalog(template.as_ref());
    assert!(OBJECT_REGISTRY.get_object(ID).is_none());
    let object =
        Object::new_with_id(template.clone(), ID, ObjectStatusMaskType::NONE, None).unwrap();
    assert!(Arc::ptr_eq(
        &OBJECT_REGISTRY.get_object(ID).unwrap(),
        &object
    ));
    {
        let guard = object.read().unwrap();
        assert!(guard.has_ctor_helpers());
        assert!(guard.get_body_module().is_some());
        let modules = guard.behavior_modules();
        assert_eq!(
            modules.iter().map(|m| m.tag().as_str()).collect::<Vec<_>>(),
            [BODY_TAG, AI_TAG, SENTINEL_TAG]
        );
        // The production factory installs a private BodyBindingModule around
        // ActiveBody. Verify its public cached-body and module snapshot routes
        // alias the same runtime instead of downcasting the outer wrapper.
        assert!(modules[0].with_module(|module| {
            module
                .get_module_data()
                .as_any()
                .is::<ActiveBodyModuleData>()
        }));
        let body = guard.get_body_module().unwrap();
        {
            let body = body.lock().unwrap();
            assert_eq!(body.get_health(), 100.0);
            assert_eq!(body.get_max_health(), 100.0);
            assert_eq!(body.get_damage_scalar(), 1.0);
        }
        let save_body = || {
            let mut bytes = Vec::new();
            body.lock()
                .unwrap()
                .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
            bytes
        };
        let save_body_module = || {
            let mut bytes = Vec::new();
            modules[0]
                .with_module(|module| module.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1)))
                .unwrap();
            bytes
        };
        let original_body = save_body();
        assert_eq!(save_body_module(), original_body);
        body.lock().unwrap().apply_damage_scalar(2.0).unwrap();
        assert_eq!(body.lock().unwrap().get_damage_scalar(), 2.0);
        let changed_body = save_body();
        assert_ne!(changed_body, original_body);
        assert_eq!(save_body_module(), changed_body);
        body.lock().unwrap().apply_damage_scalar(0.5).unwrap();
        assert_eq!(body.lock().unwrap().get_damage_scalar(), 1.0);
        assert_eq!(save_body(), original_body);
        assert_eq!(save_body_module(), original_body);
        assert!(
            modules[1]
                .with_module_downcast::<AIUpdateInterfaceModule, _, _>(|_| ())
                .is_some()
        );
        assert!(
            modules[2]
                .with_module_downcast::<SentinelModule, _, _>(|_| ())
                .is_some()
        );
        for (module, tag) in modules.iter().zip([BODY_TAG, AI_TAG, SENTINEL_TAG]) {
            assert_eq!(
                NameKeyGenerator::key_to_name(module.module_tag_key()).as_deref(),
                Some(tag)
            );
        }
        let mut ai_bytes = Vec::new();
        modules[1]
            .with_module(|module| module.xfer(&mut XferSave::new(Cursor::new(&mut ai_bytes), 1)))
            .unwrap();
        assert_eq!(
            ai_bytes,
            [4],
            "explicit admission: no runtime AI or nested machine payload"
        );
    }
    assert_catalog(template.as_ref());
    object
}

fn save(object: &Arc<RwLock<Object>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
        Snapshot::xfer(&mut *object.write().unwrap(), &mut xfer);
        let mut sentinel = OUTER_SENTINEL;
        xfer.xfer_unsigned_int(&mut sentinel).unwrap();
    }
    assert_eq!(bytes[0], 9);
    bytes
}

// Locate only a verified length-prefixed tag + real block-size frame. The
// payload itself must exactly match the direct admitted module serialization.
fn row(bytes: &[u8], tag: &str, expected: &[u8]) -> (usize, usize) {
    let mut prefix = vec![u8::try_from(tag.len()).unwrap()];
    prefix.extend_from_slice(tag.as_bytes());
    let matches: Vec<_> = bytes
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(i, part)| (part == prefix).then_some(i))
        .collect();
    assert_eq!(matches.len(), 1, "unique authored tag frame");
    let size_at = matches[0] + prefix.len();
    assert_eq!(
        i32::from_le_bytes(bytes[size_at..size_at + 4].try_into().unwrap()),
        expected.len() as i32
    );
    let payload = size_at + 4;
    assert_eq!(&bytes[payload..payload + expected.len()], expected);
    (matches[0], payload)
}

fn direct_control(object: &Arc<RwLock<Object>>) {
    let module = object
        .read()
        .unwrap()
        .module_by_tag(&AI_TAG.into())
        .unwrap();
    for version in [4, 5] {
        let mut bytes = vec![version];
        bytes.extend_from_slice(&OUTER_SENTINEL.to_le_bytes());
        let mut cursor = Cursor::new(bytes);
        let result = {
            let mut xfer = XferLoad::new(&mut cursor, 1);
            module.with_module(|module| module.xfer(&mut xfer))
        };
        assert_eq!(cursor.position(), 1);
        if version == 4 {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "real AI wrapper must reject v5");
        }
        let mut sentinel = 0;
        XferLoad::new(&mut cursor, 1)
            .xfer_unsigned_int(&mut sentinel)
            .unwrap();
        assert_eq!(sentinel, OUTER_SENTINEL);
        assert_eq!(cursor.position(), 5);
        println!(
            "direct_wrapper version={version} result={result:?} bytes_consumed=1 sentinel={sentinel:#x}"
        );
    }
}

fn execute(case: &str) {
    let _ = env_logger::builder().is_test(true).try_init();
    if case.starts_with("native_loader::") {
        native_loader::execute(case);
        return;
    }
    let template = authored_template();
    let writer = object(&template);
    if case == "real_ai_wrapper_rejects_unsupported_version" {
        direct_control(&writer);
        return;
    }
    let valid = save(&writer);
    let (ai_row, ai_payload) = row(&valid, AI_TAG, &[4]);
    let (sentinel_row, _) = row(&valid, SENTINEL_TAG, &MODULE_SENTINEL.to_le_bytes());
    assert_eq!(
        sentinel_row,
        ai_payload + 1,
        "sentinel is the next module row"
    );
    drop(writer);
    OBJECT_REGISTRY.unregister_object(ID);
    gamelogic::ai::object_registry::unregister_legacy_object(ID);
    let reader = object(&template);
    if case == "checked_object_reports_known_helper_error" {
        let tag = "ModuleTag_SMCHelper";
        let mut prefix = vec![tag.len() as u8];
        prefix.extend_from_slice(tag.as_bytes());
        let positions: Vec<_> = valid
            .windows(prefix.len())
            .enumerate()
            .filter_map(|(i, bytes)| (bytes == prefix).then_some(i))
            .collect();
        assert_eq!(positions.len(), 1);
        let size_at = positions[0] + prefix.len();
        let size = i32::from_le_bytes(valid[size_at..size_at + 4].try_into().unwrap());
        assert!(size > 1);
        let payload = size_at + 4;
        assert_eq!(valid[payload], 1, "actual SMC helper current version");
        let mut bytes = valid.clone();
        bytes[payload] = 2;
        assert_eq!(bytes.iter().zip(&valid).filter(|(a, b)| a != b).count(), 1);
        let mut cursor = Cursor::new(&bytes);
        let error = reader
            .write()
            .unwrap()
            .xfer_checked(&mut XferLoad::new(&mut cursor, 1))
            .unwrap_err();
        assert_eq!(error.object_id, ID);
        assert_eq!(error.module_tag.as_deref(), Some(tag));
        assert_eq!(error.operation, "helper_xfer");
        assert!(error.detail.contains("ObjectSMCHelper xfer version"));
        assert_eq!(cursor.position() as usize, payload + 1);
        assert_eq!(
            reader
                .read()
                .unwrap()
                .module_by_tag(&SENTINEL_TAG.into())
                .unwrap()
                .with_module_downcast::<SentinelModule, _, _>(|m| m.loads),
            Some(0)
        );
        assert_catalog(template.as_ref());
        println!("helper_error={error} exact_cursor={}", cursor.position());
        return;
    }
    let checked = case == "checked_object_reports_module_context_and_stops_cursor";
    let unknown = case == "checked_object_skips_unknown_row_and_continues";
    let corrupted = case == "object_stops_before_following_module_on_ai_wrapper_error" || checked;
    let mut bytes = valid.clone();
    if corrupted {
        bytes[ai_payload] = 5;
        let changed: Vec<_> = bytes
            .iter()
            .zip(&valid)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert_eq!(changed, [ai_payload]);
    }
    if unknown {
        let unknown_tag = "ModuleTag_NativeErrorZZ";
        assert_eq!(unknown_tag.len(), AI_TAG.len());
        assert!(
            reader
                .read()
                .unwrap()
                .module_by_tag(&unknown_tag.into())
                .is_none()
        );
        bytes[ai_row + 1..ai_row + 1 + AI_TAG.len()].copy_from_slice(unknown_tag.as_bytes());
        bytes[ai_payload] = 5; // A known AI row would reject this same payload.
    }
    if let Some(directory) = std::env::var_os("GENERALS_NATIVE_MODULE_ERROR_EVIDENCE") {
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{case}.bin")),
            &bytes,
        )
        .unwrap();
    }
    let mut cursor = Cursor::new(&bytes);
    if checked || unknown {
        let result = reader
            .write()
            .unwrap()
            .xfer_checked(&mut XferLoad::new(&mut cursor, 1));
        if checked {
            let error =
                result.expect_err("typed checked caller must observe the actual module failure");
            assert_eq!(error.object_id, ID);
            assert_eq!(error.module_tag.as_deref(), Some(AI_TAG));
            assert_eq!(error.operation, "module_xfer");
            assert!(error.detail.contains("Unknown version '5'"));
            println!("checked_error={error}");
        } else {
            result.expect("unknown row must retain length-based skip behavior");
        }
    } else {
        Snapshot::xfer(
            &mut *reader.write().unwrap(),
            &mut XferLoad::new(&mut cursor, 1),
        );
    }
    let object_end = cursor.position();
    let mut outer = 0;
    if !corrupted {
        XferLoad::new(&mut cursor, 1)
            .xfer_unsigned_int(&mut outer)
            .unwrap();
    }
    let sentinel = reader
        .read()
        .unwrap()
        .module_by_tag(&SENTINEL_TAG.into())
        .unwrap()
        .with_module_downcast::<SentinelModule, _, _>(|module| (module.loads, module.value))
        .unwrap();
    assert_catalog(template.as_ref());
    println!(
        "object_case={case} ai_payload_offset={ai_payload} ai_payload_length=1 input_version={} following_module_loads={} following_module_value={:#x} object_end={object_end} expected_object_end={} outer_sentinel={outer:#x} total_bytes={}",
        bytes[ai_payload],
        sentinel.0,
        sentinel.1,
        bytes.len() - 4,
        bytes.len()
    );
    if corrupted {
        assert_eq!(
            object_end as usize,
            ai_payload + 1,
            "stop immediately after rejected row payload"
        );
        assert_eq!(cursor.position() as usize, ai_payload + 1);
        assert_eq!(
            sentinel.0, 0,
            "Object must stop before the following known module after AI wrapper rejection"
        );
    } else {
        assert_eq!(sentinel, (1, MODULE_SENTINEL));
        assert_eq!(object_end as usize, bytes.len() - 4);
        assert_eq!(outer, OUTER_SENTINEL);
        assert_eq!(cursor.position() as usize, bytes.len());
        assert_eq!(save(&reader), valid, "exact valid Object round-trip");
    }
}

fn bounded_child(case: &str) {
    if std::env::var(CHILD_ENV).as_deref() == Ok(case) {
        execute(case);
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", case, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, case)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated native module witness failed: {case}: {status}"
            );
            return;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("native module witness exceeded 30-second child bound: {case}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn real_ai_wrapper_rejects_unsupported_version() {
    bounded_child("real_ai_wrapper_rejects_unsupported_version");
}
#[test]
fn valid_object_preserves_exact_payload_and_sentinels() {
    bounded_child("valid_object_preserves_exact_payload_and_sentinels");
}
#[test]
fn object_stops_before_following_module_on_ai_wrapper_error() {
    bounded_child("object_stops_before_following_module_on_ai_wrapper_error");
}

#[test]
fn checked_object_reports_module_context_and_stops_cursor() {
    bounded_child("checked_object_reports_module_context_and_stops_cursor");
}

#[test]
fn checked_object_skips_unknown_row_and_continues() {
    bounded_child("checked_object_skips_unknown_row_and_continues");
}

#[test]
fn checked_object_reports_known_helper_error() {
    bounded_child("checked_object_reports_known_helper_error");
}
