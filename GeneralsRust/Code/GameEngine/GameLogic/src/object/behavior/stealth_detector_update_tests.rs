//! Canonical authored detector coverage. StealthDetectorUpdate.cpp:64–85,
//! 131–177,400,420–432; StealthDetectorUpdate.h:29–55.
use super::*;
use crate::object::object_thing::ObjectThingHandle;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{BehaviorModuleHandle, ModuleUpdateProxy};
use crate::system::game_logic::GameLogic;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module::{ModuleInterfaceType, ModuleType};
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::update_module::UpdateModulePtr;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Exactly one admitted owner; neither an Object guard nor a Module guard
/// spans destroy/onDelete. Different fixtures own different GameLogic values.
pub(crate) struct DetectorFixture {
    pub(crate) owner: GameLogic,
    pub(crate) object: Arc<RwLock<GameObject>>,
    pub(crate) id: ObjectID,
}

impl DetectorFixture {
    pub(crate) fn new(data: StealthDetectorUpdateModuleData, id: ObjectID) -> Self {
        assert!(
            OBJECT_REGISTRY.get_object(id).is_none(),
            "detector ID already owned"
        );
        let object = Arc::new(RwLock::new(GameObject::new_test(id, 100.0)));
        let mut fixture = Self {
            owner: GameLogic::new(),
            object,
            id,
        };
        fixture
            .owner
            .register_object(fixture.object.clone())
            .expect("canonical detector admission");
        assert!(
            OBJECT_REGISTRY
                .get_object(id)
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &fixture.object))
        );

        fixture.install_detector(data, "DetectorFirst");
        fixture
    }

    fn install_detector(
        &self,
        data: StealthDetectorUpdateModuleData,
        tag: &str,
    ) -> Arc<crate::object::ModuleEntry> {
        // Use the same registered authored factory as Object::init_modules_for;
        // the already-admitted owner is resolved through ObjectThingHandle.
        let mut factory = ModuleFactory::new();
        crate::contain_module_overrides::register_module_overrides(&mut factory).unwrap();
        let data: Arc<dyn EngineModuleData> = Arc::new(data);
        let thing: Arc<dyn ModuleThing> = Arc::new(ObjectThingHandle::new(&self.object));
        let module = factory
            .new_module(
                thing,
                "StealthDetectorUpdate",
                data.clone(),
                ModuleType::Behavior,
            )
            .unwrap();
        let entry = Arc::new(crate::object::ModuleEntry::new(
            "StealthDetectorUpdate".into(),
            tag.into(),
            ModuleInterfaceType::UPDATE,
            data,
            module,
        ));
        let mut object = self.object.write().unwrap();
        let index = object.modules.len();
        object.modules.push(entry.clone());
        object.update_module_handles.push(index);
        object.rebuild_behavior_list();
        entry
    }

    pub(crate) fn module(&self) -> BehaviorModuleHandle {
        self.object
            .read()
            .unwrap()
            .find_update_module("StealthDetectorUpdate")
            .expect("installed canonical detector")
    }

    pub(crate) fn with_detector<R>(&self, f: impl FnOnce(&mut StealthDetectorUpdate) -> R) -> R {
        self.module()
            .with_module_downcast::<StealthDetectorUpdateModule, _, _>(|module| {
                f(module.behavior_mut())
            })
            .expect("canonical factory wrapper")
    }

    fn retire(&mut self) -> Result<(), String> {
        let admitted = self
            .owner
            .find_object_by_id(self.id)
            .ok_or("detector canonical admission missing")?;
        if !Arc::ptr_eq(&admitted, &self.object) {
            return Err("detector canonical identity changed".into());
        }
        self.owner.destroy_object(self.id);
        self.owner
            .process_destroy_list()
            .map_err(|e| e.to_string())?;
        if self.owner.find_object_by_id(self.id).is_some()
            || OBJECT_REGISTRY.get_object(self.id).is_some()
        {
            return Err("detector remains admitted after retirement".into());
        }
        if self
            .object
            .read()
            .map_err(|_| "retired detector object poisoned")?
            .get_id()
            != crate::common::INVALID_ID
        {
            return Err("detector finalization did not invalidate its ID".into());
        }
        Ok(())
    }
}

impl Drop for DetectorFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.retire().expect("exact detector owner retirement")
        }));
        if let Err(error) = result {
            if unwinding {
                eprintln!("detector fixture retirement failed during assertion unwind");
            } else {
                resume_unwind(error);
            }
        }
    }
}

fn exercise_retirement(unwind: bool) {
    let _guard = crate::test_sync::lock();
    let unrelated = DetectorFixture::new(Default::default(), 0x5D37_0003);
    let id = 0x5D37_0004;
    let mut pin = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let fixture = DetectorFixture::new(Default::default(), id);
        pin = Some(fixture.object.clone());
        if unwind {
            panic!("intentional detector fixture assertion unwind");
        }
    }));
    assert_eq!(result.is_err(), unwind);
    assert!(OBJECT_REGISTRY.get_object(id).is_none());
    assert_eq!(
        pin.unwrap().read().unwrap().get_id(),
        crate::common::INVALID_ID
    );
    assert!(
        OBJECT_REGISTRY
            .get_object(unrelated.id)
            .is_some_and(|admitted| Arc::ptr_eq(&admitted, &unrelated.object))
    );
    assert_eq!(unrelated.object.read().unwrap().get_id(), unrelated.id);
}

#[test]
fn installed_detector_normal_retirement_preserves_other_owner() {
    exercise_retirement(false);
}

#[test]
fn installed_detector_unwind_retires_exact_owner() {
    exercise_retirement(true);
}

#[test]
fn canonical_detector_defaults_match_cpp_module_data() {
    let data = StealthDetectorUpdateModuleData::default();
    assert_eq!(data.update_rate, 1);
    assert_eq!(data.detection_range, 0.0);
    assert!(!data.initially_disabled);
    assert!(data.ping_sound.is_none() && data.loud_ping_sound.is_none());
    assert!(data.ir_beacon_particle_sys.is_none() && data.ir_particle_sys.is_none());
    assert!(data.ir_bright_particle_sys.is_none() && data.ir_grid_particle_sys.is_none());
    assert!(data.ir_particle_sys_bone.is_empty());
    assert_eq!(
        (data.extra_detect_kindof, data.extra_detect_kindof_not),
        (0, 0)
    );
    assert!(!data.can_detect_while_garrisoned && !data.can_detect_while_transported);
}

fn parse_field(data: &mut StealthDetectorUpdateModuleData, token: &str, values: &[&str]) {
    let field = STEALTH_DETECTOR_UPDATE_FIELDS
        .iter()
        .find(|field| field.token == token)
        .expect("authored detector INI field");
    (field.parse)(&mut INI::new(), data, values).unwrap();
}

#[test]
fn canonical_detector_ini_uses_logic_frames_and_authored_fields() {
    let mut data = StealthDetectorUpdateModuleData::default();
    parse_field(&mut data, "DetectionRate", &["=", "1000"]);
    parse_field(&mut data, "DetectionRange", &["=", "123.5"]);
    parse_field(&mut data, "InitiallyDisabled", &["=", "Yes"]);
    parse_field(&mut data, "PingSound", &["=", "DetectorPing"]);
    parse_field(&mut data, "LoudPingSound", &["=", "NONE"]);
    parse_field(&mut data, "IRParticleSysName", &["=", "DetectorPulse"]);
    parse_field(&mut data, "IRParticleSysBone", &["=", "RadarBone"]);
    parse_field(
        &mut data,
        "ExtraRequiredKindOf",
        &["=", "INFANTRY", "VEHICLE"],
    );
    parse_field(&mut data, "ExtraForbiddenKindOf", &["=", "AIRCRAFT"]);
    parse_field(&mut data, "CanDetectWhileGarrisoned", &["=", "Yes"]);
    parse_field(&mut data, "CanDetectWhileContained", &["=", "Yes"]);
    assert_eq!(data.update_rate, 30);
    assert_eq!(data.detection_range, 123.5);
    assert!(data.initially_disabled);
    assert_eq!(data.ping_sound.as_deref(), Some("DetectorPing"));
    assert!(data.loud_ping_sound.is_none());
    assert_eq!(data.ir_particle_sys.as_deref(), Some("DetectorPulse"));
    assert_eq!(data.ir_particle_sys_bone, "RadarBone");
    assert!(data.can_detect_while_garrisoned && data.can_detect_while_transported);
    use game_engine::common::system::kind_of::KindOfMask;
    assert_eq!(
        data.extra_detect_kindof,
        (KindOfMask::INFANTRY | KindOfMask::VEHICLE).bits()
    );
    assert_eq!(data.extra_detect_kindof_not, KindOfMask::AIRCRAFT.bits());
}

#[test]
fn canonical_detector_requires_all_kindof_bits_and_rejects_any_forbidden_bit() {
    use game_engine::common::system::kind_of::KindOfMask;
    let required = KindOfMask::INFANTRY | KindOfMask::VEHICLE;
    let forbidden = KindOfMask::AIRCRAFT;
    assert!(stealth_detector_kindof_allows(0, 0, 0));
    assert!(!stealth_detector_kindof_allows(
        KindOfMask::INFANTRY.bits(),
        required.bits(),
        forbidden.bits()
    ));
    assert!(stealth_detector_kindof_allows(
        required.bits(),
        required.bits(),
        forbidden.bits()
    ));
    assert!(!stealth_detector_kindof_allows(
        (required | forbidden).bits(),
        required.bits(),
        forbidden.bits()
    ));
}

#[test]
fn authored_factory_exposes_one_detector_for_control_update_and_snapshot() {
    let _guard = crate::test_sync::lock();
    let now = crate::helpers::TheGameLogic::get_frame();
    let fixture = DetectorFixture::new(
        StealthDetectorUpdateModuleData {
            update_rate: 10,
            ..Default::default()
        },
        0x5D37_0010,
    );
    let installed = fixture.module();
    installed.with_module(|module| {
        assert_eq!(
            module.get_module_name_key(),
            NameKeyGenerator::name_to_key("StealthDetectorUpdate")
        );
        let wake = module
            .get_initial_wake_frame()
            .expect("authored detector initial wake");
        assert!(wake >= now.saturating_add(1) && wake <= now.saturating_add(10));
        assert_eq!(
            module
                .get_sleepy_update_interface()
                .expect("authored sleepy interface")
                .get_disabled_types_to_process(),
            DisabledMaskType::HELD
        );
        assert_eq!(
            module
                .get_update_module_interface()
                .expect("authored update interface")
                .update_simple(),
            UpdateSleepTime::Frames(10)
        );
        // This factory test queries the canonical pure control interface.
        // Registered owner/schedule timing is exercised separately below.
        module
            .get_stealth_detector_control_interface()
            .expect("authored detector control interface")
            .set_sd_enabled(false, now);
        assert_eq!(
            module
                .get_update_module_interface()
                .unwrap()
                .update_simple(),
            UpdateSleepTime::Forever
        );
        let concrete = module
            .as_any_mut()
            .downcast_mut::<StealthDetectorUpdateModule>()
            .expect("one concrete module");
        assert!(!concrete.behavior_mut().enabled);
        assert_eq!(concrete.behavior_mut().object_id, fixture.id);
        module
            .get_stealth_detector_control_interface()
            .unwrap()
            .set_sd_enabled(true, now);
        assert_eq!(
            module.get_initial_wake_frame(),
            Some(crate::helpers::TheGameLogic::get_frame().saturating_add(1))
        );
        assert_eq!(
            module
                .get_update_module_interface()
                .unwrap()
                .update_simple(),
            UpdateSleepTime::Frames(10)
        );
    });
}

#[test]
fn canonical_detector_factory_xfer_round_trips_base_wake_and_enabled() {
    let _guard = crate::test_sync::lock();
    let source = DetectorFixture::new(Default::default(), 0x5D37_0011);
    let destination = DetectorFixture::new(Default::default(), 0x5D37_0012);
    source.with_detector(|detector| {
        detector.enabled = false;
        detector.next_call_frame_and_phase = 12345;
    });
    let mut bytes = Vec::new();
    source.module().with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    destination.module().with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(&bytes), 1))
            .unwrap()
    });
    destination.with_detector(|detector| {
        assert!(!detector.enabled);
        assert_eq!(detector.next_call_frame_and_phase, 12345);
        assert_eq!(
            detector.object_id, destination.id,
            "Xfer must not rebind the admitted owner"
        );
        assert!(detector.grid_particle_ids.is_empty());
        assert!(detector.ping_particle_id.is_none() && detector.beacon_particle_id.is_none());
    });
    assert_eq!(
        bytes.len(),
        10,
        "version + four base versions + wake + enabled (CPP420–432)"
    );
}

#[test]
fn canonical_detector_dispatch_preserves_detection_rate_in_actual_sleepy_queue() {
    let _guard = crate::test_sync::lock();
    let mut fixture = DetectorFixture::new(
        StealthDetectorUpdateModuleData {
            update_rate: 10,
            ..Default::default()
        },
        0x5D37_0013,
    );
    let entry = fixture
        .object
        .read()
        .unwrap()
        .find_module_by_name("StealthDetectorUpdate")
        .unwrap();
    let proxy: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(entry, fixture.id)));
    // The real proxy dispatches the same installed factory allocation, and
    // GameLogic owns this registration. No synthetic countdown is involved.
    fixture
        .owner
        .register_sleepy_update_module(fixture.id, proxy.clone(), 1);
    assert_eq!(fixture.owner.sleepy_update_count(), 1);
    fixture.owner.update(1).unwrap();
    assert_eq!(
        fixture.owner.sleepy_entry_for(&proxy),
        Some((fixture.id, 11))
    );
    fixture.owner.update(10).unwrap();
    assert_eq!(fixture.owner.sleepy_entry_for(&proxy).unwrap().1, 11);
    fixture.owner.update(11).unwrap();
    assert_eq!(fixture.owner.sleepy_entry_for(&proxy).unwrap().1, 21);
    assert_eq!(fixture.owner.sleepy_update_count(), 1);
}

/// Test adapter over the actual owned heap, not a simulated wake recorder.
struct OwnedSchedule<'a>(&'a mut GameLogic);
impl game_engine::common::thing::update_module::UpdateScheduleContext for OwnedSchedule<'_> {
    fn frame(&self) -> u32 {
        self.0.get_frame()
    }
    fn awaken(&mut self, module: &UpdateModulePtr, wake: u32) {
        self.0.friend_awaken_update_module(module, wake);
    }
    fn unregister(&mut self, module: &UpdateModulePtr) {
        self.0
            .unregister_update_module(crate::common::INVALID_ID, module.clone());
    }
}

fn owned_detector_without_admission(id: ObjectID) -> (Arc<RwLock<GameObject>>, UpdateModulePtr) {
    let object = Arc::new(RwLock::new(GameObject::new_test(id, 100.0)));
    let data = Arc::new(StealthDetectorUpdateModuleData {
        initially_disabled: true,
        ..Default::default()
    });
    let legacy_data: Arc<dyn ModuleData> = data.clone();
    let behavior = StealthDetectorUpdate::new(object.clone(), legacy_data).unwrap();
    let module = StealthDetectorUpdateModule::new(
        behavior,
        &AsciiString::from("StealthDetectorUpdate"),
        data.clone(),
    );
    object
        .write()
        .unwrap()
        .install_update_module("StealthDetectorUpdate", Box::new(module), data);
    let entry = object
        .read()
        .unwrap()
        .find_module_by_name("StealthDetectorUpdate")
        .unwrap();
    let proxy: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(entry.clone(), id)));
    object
        .write()
        .unwrap()
        .attach_update_module_registration(proxy.clone(), Some(&entry))
        .unwrap();
    (object, proxy)
}

fn set_owned_detector(
    object: &Arc<RwLock<GameObject>>,
    enabled: bool,
    schedule: &mut dyn game_engine::common::thing::update_module::UpdateScheduleContext,
) {
    let frame = schedule.frame();
    let (wake, registrations) = object
        .read()
        .unwrap()
        .set_stealth_detector_enabled(enabled, frame)
        .expect("canonical detector control");
    // The Object read and module setter guards have both ended here.
    for registration in registrations {
        schedule.awaken(&registration, wake);
    }
}

#[test]
fn pure_detector_owner_and_real_schedule_isolate_same_id_at_different_frames() {
    let _guard = crate::test_sync::lock();
    let id = 0x5D37_0030;
    assert!(OBJECT_REGISTRY.get_object(id).is_none());
    let global_frame = crate::helpers::TheGameLogic::get_frame();
    // No canonical admission: this tests the explicit control/schedule seam,
    // not the separately global registry's unresolved equal-ID admission.
    let (object_a, proxy_a) = owned_detector_without_admission(id);
    let (object_b, proxy_b) = owned_detector_without_admission(id);
    let mut world_a = GameLogic::new();
    let mut world_b = GameLogic::new();
    world_a.set_current_frame(10);
    world_b.set_current_frame(100);
    world_a.register_sleepy_update_module(id, proxy_a.clone(), 0x3fff_ffff);
    world_b.register_sleepy_update_module(id, proxy_b.clone(), 0x3fff_ffff);
    set_owned_detector(&object_a, true, &mut OwnedSchedule(&mut world_a));
    assert_eq!(world_a.sleepy_entry_for(&proxy_a).unwrap().1, 11);
    assert_eq!(world_b.sleepy_entry_for(&proxy_b).unwrap().1, 0x3fff_ffff);
    set_owned_detector(&object_b, true, &mut OwnedSchedule(&mut world_b));
    assert_eq!(world_b.sleepy_entry_for(&proxy_b).unwrap().1, 101);
    assert_eq!(world_a.sleepy_entry_for(&proxy_a).unwrap().1, 11);
    set_owned_detector(&object_a, false, &mut OwnedSchedule(&mut world_a));
    assert_eq!(world_a.sleepy_entry_for(&proxy_a).unwrap().1, 0x3fff_ffff);
    assert_eq!(world_b.sleepy_entry_for(&proxy_b).unwrap().1, 101);
    assert_eq!(world_a.sleepy_entry_for(&proxy_a), Some((id, 0x3fff_ffff)));
    assert_eq!(world_b.sleepy_entry_for(&proxy_b), Some((id, 101)));
    // Foreign identity with equal ObjectID cannot be mistaken for this queue's entry.
    world_b.friend_awaken_update_module(&proxy_a, 200);
    assert_eq!(world_b.sleepy_entry_for(&proxy_b).unwrap().1, 101);
    assert_eq!(crate::helpers::TheGameLogic::get_frame(), global_frame);
    assert!(OBJECT_REGISTRY.get_object(id).is_none());
}

#[test]
fn detector_control_wakes_only_first_selected_authored_tag() {
    let _guard = crate::test_sync::lock();
    let mut fixture = DetectorFixture::new(Default::default(), 0x5D37_0030);
    let first = fixture
        .object
        .read()
        .unwrap()
        .find_module_by_name("StealthDetectorUpdate")
        .unwrap();
    let second = fixture.install_detector(Default::default(), "DetectorSecond");
    assert_ne!(first.tag(), second.tag());
    let first_proxy: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(
        first.clone(),
        fixture.id,
    )));
    let second_proxy: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(
        second.clone(),
        fixture.id,
    )));
    {
        let mut object = fixture.object.write().unwrap();
        object
            .attach_update_module_registration(first_proxy.clone(), Some(&first))
            .unwrap();
        object
            .attach_update_module_registration(second_proxy.clone(), Some(&second))
            .unwrap();
    }
    fixture
        .owner
        .register_sleepy_update_module(fixture.id, first_proxy.clone(), 18);
    fixture
        .owner
        .register_sleepy_update_module(fixture.id, second_proxy.clone(), 24);
    let (wake, registrations) = fixture
        .object
        .read()
        .unwrap()
        .set_stealth_detector_enabled(false, 5)
        .unwrap();
    assert_eq!(
        registrations.len(),
        1,
        "C++ findModule selects only the first class match"
    );
    assert!(Arc::ptr_eq(&registrations[0], &first_proxy));
    for registration in registrations {
        fixture
            .owner
            .friend_awaken_update_module(&registration, wake);
    }
    assert!(!fixture.with_detector(|detector| detector.is_enabled()));
    second.with_module(|module| {
        let detector = (module as &mut dyn std::any::Any)
            .downcast_mut::<StealthDetectorUpdateModule>()
            .unwrap();
        assert!(
            detector.behavior_mut().is_enabled(),
            "unselected authored detector remains enabled"
        );
    });
    assert_eq!(
        fixture.owner.sleepy_entry_for(&first_proxy),
        Some((fixture.id, 0x3fff_ffff))
    );
    assert_eq!(
        fixture.owner.sleepy_entry_for(&second_proxy),
        Some((fixture.id, 24)),
        "unselected detector keeps its actual queue wake"
    );
}
