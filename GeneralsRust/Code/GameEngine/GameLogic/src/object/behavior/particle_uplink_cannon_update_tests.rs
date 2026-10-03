use super::*;
use crate::common::{
    DefaultThingTemplate, GeometryInfo, KindOf, ObjectStatusMaskType, TemplateModuleInfo,
    ThingTemplate,
};
use crate::object::Object;
use crate::object::object_thing::ObjectThingHandle;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_types::SpecialPowerType;
use crate::system::game_logic::get_game_logic;
use game_engine::common::thing::module::{ModuleInterfaceType, ModuleType, Thing as ModuleThing};
use game_engine::common::thing::module_factory::ModuleFactory;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Holds exactly one canonical owner. Constructor fixtures stay outside the
/// GameLogic object list until teardown; their module scheduler registrations
/// and object lookup are retired by that same GameLogic, without a world reset.
struct UplinkOwner {
    object: Arc<RwLock<GameObject>>,
    id: ObjectID,
}

impl UplinkOwner {
    fn admitted(object: Arc<RwLock<GameObject>>) -> Self {
        let id = object.read().unwrap().get_id();
        get_game_logic()
            .lock()
            .unwrap()
            .register_object(object.clone())
            .unwrap();
        Self { object, id }
    }

    fn from_template(id: ObjectID, template: Arc<dyn ThingTemplate>) -> Self {
        let object = Object::new_with_id(template, id, ObjectStatusMaskType::NONE, None).unwrap();
        Self { object, id }
    }

    fn before_admission(id: ObjectID) -> Self {
        Self::from_template(
            id,
            Arc::new(DefaultThingTemplate::new(format!("UplinkOwner{id}"))),
        )
    }

    fn retire(&self) -> Result<(), String> {
        let mut logic = get_game_logic().lock().map_err(|e| e.to_string())?;
        if let Some(existing) = logic.find_object_by_id(self.id) {
            if !Arc::ptr_eq(&existing, &self.object) {
                return Err(format!(
                    "fixture object {} changed canonical identity",
                    self.id
                ));
            }
        } else {
            logic
                .register_object(self.object.clone())
                .map_err(|e| e.to_string())?;
        }
        logic.destroy_object(self.id);
        logic.process_destroy_list().map_err(|e| e.to_string())?;
        if logic.find_object_by_id(self.id).is_some() {
            return Err(format!("fixture object {} was not retired", self.id));
        }
        Ok(())
    }
}

impl Drop for UplinkOwner {
    fn drop(&mut self) {
        let already_panicking = std::thread::panicking();
        match catch_unwind(AssertUnwindSafe(|| self.retire())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) if already_panicking => eprintln!("Uplink fixture retirement: {error}"),
            Ok(Err(error)) => panic!("Uplink fixture retirement: {error}"),
            Err(_) if already_panicking => eprintln!("Uplink fixture retirement panicked"),
            Err(payload) => resume_unwind(payload),
        }
    }
}

fn authored_data() -> Arc<ParticleUplinkCannonUpdateModuleData> {
    Arc::new(ParticleUplinkCannonUpdateModuleData {
        special_power_template: Some(Arc::new(SpecialPowerTemplate::new(
            "SPECIAL_PARTICLE_UPLINK_CANNON".to_string(),
            SpecialPowerType::ParticleUplinkCannon as u32,
        ))),
        powerup_sound_name: AsciiString::from("PowerUpLoop"),
        unpack_to_ready_sound_name: AsciiString::from("UnpackLoop"),
        firing_to_idle_sound_name: AsciiString::from("PackLoop"),
        annihilation_sound_name: AsciiString::from("AnnihilationLoop"),
        ..ParticleUplinkCannonUpdateModuleData::default()
    })
}

fn factory_module(
    owner: &UplinkOwner,
    data: Arc<ParticleUplinkCannonUpdateModuleData>,
) -> Box<dyn Module> {
    // Install the genuine GameLogic constructors on an independent rules owner.
    // Do not reset the process factory or import another thread's staged overrides.
    let mut factory = ModuleFactory::new();
    crate::contain_module_overrides::register_module_overrides(&mut factory).unwrap();
    let thing: Arc<dyn ModuleThing> = Arc::new(ObjectThingHandle::new(&owner.object));
    factory
        .new_module(
            thing,
            "ParticleUplinkCannonUpdate",
            data,
            ModuleType::Behavior,
        )
        .unwrap()
}

fn installed_entry(owner: &UplinkOwner) -> Arc<crate::object::ModuleEntry> {
    owner
        .object
        .read()
        .unwrap()
        .find_module_by_name("ParticleUplinkCannonUpdate")
        .unwrap()
}

#[derive(Debug)]
struct CreationState {
    invalid_settings: bool,
    connector: Coord3D,
    origin: Coord3D,
    sounds: [String; 4],
}

fn installed_creation_state(owner: &UplinkOwner) -> CreationState {
    installed_entry(owner).with_module(|module| {
        let wrapper = module
            .as_any_mut()
            .downcast_mut::<ParticleUplinkCannonUpdateModule>()
            .unwrap();
        let behavior = &wrapper.behavior;
        CreationState {
            invalid_settings: behavior.invalid_settings,
            connector: behavior.connector_node_position,
            origin: behavior.laser_origin_position,
            sounds: [
                behavior.powerup_sound.get_event_name().to_string(),
                behavior.unpack_to_ready_sound.get_event_name().to_string(),
                behavior.firing_to_idle_sound.get_event_name().to_string(),
                behavior.annihilation_sound.get_event_name().to_string(),
            ],
        }
    })
}

fn assert_creation_state(state: &CreationState, position: Coord3D) {
    // Assert outside ModuleEntry's guard so the expected RED cannot poison it
    // and cascade through canonical fixture retirement.
    assert!(!state.invalid_settings);
    assert_eq!(state.connector, position);
    assert_eq!(state.origin, position);
    assert_eq!(
        state.sounds,
        ["PowerUpLoop", "UnpackLoop", "PackLoop", "AnnihilationLoop"]
    );
}

fn test_object_at(position: Coord3D) -> Arc<RwLock<GameObject>> {
    let object = Arc::new(RwLock::new(Object::new_test(98_001, 100.0)));
    object
        .write()
        .unwrap()
        .set_position(&position)
        .expect("test position is valid");
    object
}

#[test]
fn constructor_defers_invalid_settings_until_object_created() {
    let _lock = crate::test_sync::lock();
    let object = test_object_at(Coord3D::new(10.0, 20.0, 3.0));
    let data = Arc::new(ParticleUplinkCannonUpdateModuleData::default());

    let behavior = ParticleUplinkCannonUpdate::new_with_data(Arc::clone(&object), data).unwrap();

    assert!(!behavior.invalid_settings);
    assert_eq!(behavior.connector_node_position, Coord3D::ZERO);
    assert_eq!(behavior.laser_origin_position, Coord3D::ZERO);
}

#[test]
fn object_created_validates_missing_template_like_cpp() {
    let _lock = crate::test_sync::lock();
    let object = test_object_at(Coord3D::new(10.0, 20.0, 3.0));
    let data = Arc::new(ParticleUplinkCannonUpdateModuleData::default());
    let mut behavior =
        ParticleUplinkCannonUpdate::new_with_data(Arc::clone(&object), data).unwrap();

    BehaviorModuleInterface::on_object_created(&mut behavior).unwrap();

    assert!(behavior.invalid_settings);
    assert_eq!(behavior.connector_node_position, Coord3D::ZERO);
    assert_eq!(behavior.laser_origin_position, Coord3D::ZERO);
}

#[test]
fn object_created_captures_origin_position_and_audio_names() {
    let _lock = crate::test_sync::lock();
    let position = Coord3D::new(-12.5, 44.0, 6.25);
    let object = test_object_at(position);
    let _owner = UplinkOwner::admitted(object.clone());
    let mut data = ParticleUplinkCannonUpdateModuleData::default();
    data.special_power_template = Some(Arc::new(SpecialPowerTemplate::new(
        "SPECIAL_PARTICLE_UPLINK_CANNON".to_string(),
        SpecialPowerType::ParticleUplinkCannon as u32,
    )));
    data.powerup_sound_name = AsciiString::from("PowerUpLoop");
    data.unpack_to_ready_sound_name = AsciiString::from("UnpackLoop");
    data.firing_to_idle_sound_name = AsciiString::from("PackLoop");
    data.annihilation_sound_name = AsciiString::from("AnnihilationLoop");
    let mut behavior =
        ParticleUplinkCannonUpdate::new_with_data(Arc::clone(&object), Arc::new(data)).unwrap();

    BehaviorModuleInterface::on_object_created(&mut behavior).unwrap();

    assert!(!behavior.invalid_settings);
    assert_eq!(behavior.connector_node_position, position);
    assert_eq!(behavior.laser_origin_position, position);
    assert_eq!(behavior.powerup_sound.get_event_name(), "PowerUpLoop");
    assert_eq!(
        behavior.unpack_to_ready_sound.get_event_name(),
        "UnpackLoop"
    );
    assert_eq!(behavior.firing_to_idle_sound.get_event_name(), "PackLoop");
    assert_eq!(
        behavior.annihilation_sound.get_event_name(),
        "AnnihilationLoop"
    );
}

#[test]
fn remove_all_effects_preserves_orbit_to_target_beam() {
    let _lock = crate::test_sync::lock();
    // C++ ParticleUplinkCannonUpdate::removeAllEffects
    // (ParticleUplinkCannonUpdate.cpp:996-1031) destroys outer FX, connector
    // lasers, and the ground-to-orbit beam — not m_orbitToTargetBeamID.
    // LaserStatus owns that beam (cpp:453-467); killEverything (cpp:187-200)
    // is the destructor-only teardown.
    let object = test_object_at(Coord3D::new(0.0, 0.0, 0.0));
    let data = Arc::new(ParticleUplinkCannonUpdateModuleData::default());
    let mut behavior =
        ParticleUplinkCannonUpdate::new_with_data(Arc::clone(&object), data).unwrap();
    behavior.orbit_to_target_beam_id = 42;
    behavior.laser_status = LaserStatus::Born;
    behavior.ground_to_orbit_beam_id = 7;
    behavior.laser_beam_ids = vec![9];

    behavior.remove_all_effects();

    assert_eq!(behavior.orbit_to_target_beam_id, 42);
    assert_eq!(behavior.laser_status, LaserStatus::Born);
    assert_eq!(behavior.ground_to_orbit_beam_id, INVALID_DRAWABLE_ID);
    assert_eq!(behavior.laser_beam_ids, vec![INVALID_DRAWABLE_ID]);

    behavior.kill_everything();
    assert_eq!(behavior.orbit_to_target_beam_id, INVALID_DRAWABLE_ID);
}

struct AuthoredUplinkTemplate {
    inner: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
}

impl std::fmt::Debug for AuthoredUplinkTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthoredUplinkTemplate")
            .field("name", self.inner.get_name())
            .finish()
    }
}

impl ThingTemplate for AuthoredUplinkTemplate {
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

#[test]
fn authored_template_creation_validates_missing_power_before_game_logic_admission() {
    let _lock = crate::test_sync::lock();
    let template = AuthoredUplinkTemplate {
        inner: DefaultThingTemplate::new("AuthoredInvalidUplink".to_string()),
        modules: vec![TemplateModuleInfo {
            name: "ParticleUplinkCannonUpdate".into(),
            module_tag: "ModuleTag_UplinkCreation".into(),
            data: Arc::new(ParticleUplinkCannonUpdateModuleData::default()),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    };
    let owner = UplinkOwner::from_template(98_011, Arc::new(template));
    assert!(
        get_game_logic()
            .lock()
            .unwrap()
            .find_object_by_id(owner.id)
            .is_none()
    );
    assert!(Arc::ptr_eq(
        &OBJECT_REGISTRY.get_object(owner.id).unwrap(),
        &owner.object
    ));
    assert!(owner.object.read().unwrap().modules_ready);

    // Object.cpp:458-471 calls this actual installed Module wrapper before
    // modulesReady, without requiring GameLogic admission first.
    let state = installed_creation_state(&owner);
    assert!(state.invalid_settings);
    assert_eq!(state.connector, Coord3D::ZERO);
    assert_eq!(state.origin, Coord3D::ZERO);
}

#[test]
fn installed_factory_creation_uses_moved_owner_position_before_game_logic_admission() {
    let _lock = crate::test_sync::lock();
    let owner = UplinkOwner::before_admission(98_012);
    let original = Coord3D::new(12.0, 34.0, 5.0);
    owner
        .object
        .write()
        .unwrap()
        .set_position(&original)
        .unwrap();
    let data = authored_data();
    let mut module = factory_module(&owner, data.clone());
    {
        let wrapper = module
            .as_any_mut()
            .downcast_mut::<ParticleUplinkCannonUpdateModule>()
            .unwrap();
        assert!(!wrapper.behavior.invalid_settings);
        assert_eq!(wrapper.behavior.status, PUCStatus::Idle);
        assert_eq!(wrapper.behavior.laser_status, LaserStatus::None);
        assert_eq!(wrapper.behavior.connector_node_position, Coord3D::ZERO);
        assert_eq!(wrapper.behavior.laser_origin_position, Coord3D::ZERO);
    }
    owner
        .object
        .write()
        .unwrap()
        .install_update_module("ParticleUplinkCannonUpdate", module, data);
    let position = Coord3D::new(-12.5, 44.0, 6.25);
    owner
        .object
        .write()
        .unwrap()
        .set_position(&position)
        .unwrap();
    assert!(
        get_game_logic()
            .lock()
            .unwrap()
            .find_object_by_id(owner.id)
            .is_none()
    );

    // The callback must resolve this canonical owner, use its current position,
    // and enter the same wrapper only after the owner's installation guard ends.
    Object::invoke_on_object_created_after_install(&owner.object).unwrap();
    assert_creation_state(&installed_creation_state(&owner), position);
}

#[test]
fn installed_factory_creation_origin_survives_version_three_xfer() {
    use game_engine::system::xfer_load::XferLoad;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    let _lock = crate::test_sync::lock();
    let saved_owner = UplinkOwner::before_admission(98_013);
    let data = authored_data();
    let module = factory_module(&saved_owner, data.clone());
    saved_owner.object.write().unwrap().install_update_module(
        "ParticleUplinkCannonUpdate",
        module,
        data.clone(),
    );
    let position = Coord3D::new(71.0, -24.0, 9.0);
    saved_owner
        .object
        .write()
        .unwrap()
        .set_position(&position)
        .unwrap();
    Object::invoke_on_object_created_after_install(&saved_owner.object).unwrap();
    assert_creation_state(&installed_creation_state(&saved_owner), position);
    let mut bytes = Vec::new();
    installed_entry(&saved_owner).with_module(|module| {
        let wrapper = module
            .as_any_mut()
            .downcast_mut::<ParticleUplinkCannonUpdateModule>()
            .unwrap();
        wrapper
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
    });
    assert_eq!(bytes[0], 3, "C++ ParticleUplinkCannonUpdate xfer version");

    let restored_owner = UplinkOwner::before_admission(98_014);
    let module = factory_module(&restored_owner, data.clone());
    restored_owner
        .object
        .write()
        .unwrap()
        .install_update_module("ParticleUplinkCannonUpdate", module, data);
    let different_position = Coord3D::new(-70.0, 240.0, 19.0);
    restored_owner
        .object
        .write()
        .unwrap()
        .set_position(&different_position)
        .unwrap();
    Object::invoke_on_object_created_after_install(&restored_owner.object).unwrap();
    assert_creation_state(
        &installed_creation_state(&restored_owner),
        different_position,
    );
    let restored_object_id = installed_entry(&restored_owner).with_module(|module| {
        let wrapper = module
            .as_any_mut()
            .downcast_mut::<ParticleUplinkCannonUpdateModule>()
            .unwrap();
        // C++ xfer v3 (cpp:1326-1437) transfers origins, not the owner ID or
        // cached special-power pointer. Restore must retain the new owner identity.
        wrapper
            .xfer(&mut XferLoad::new(Cursor::new(&bytes), 1))
            .unwrap();
        wrapper.behavior.object_id
    });
    assert_eq!(restored_object_id, restored_owner.id);
    assert_creation_state(&installed_creation_state(&restored_owner), position);
}
