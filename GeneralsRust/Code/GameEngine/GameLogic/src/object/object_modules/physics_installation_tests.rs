//! Actual authored PhysicsBehavior installation: Object.cpp:430–436 and
//! PhysicsUpdate.cpp:182–234. No early admission or injected physics runtime.

use crate::common::{AsciiString, ObjectID, ObjectStatusMaskType};
use crate::helpers::TheThingFactory;
use crate::object::{BehaviorModuleHandle, Object, PhysicsInterfaceHandle};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use glam::Vec3;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

const ID: ObjectID = 0x7B3D_0021;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_PHYSICS_INSTALLATION_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

struct Installed {
    owner: Arc<RwLock<Object>>,
    module: BehaviorModuleHandle,
    physics: PhysicsInterfaceHandle,
}

impl Installed {
    fn new(name: &str, mass: f32) -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        {
            let mut factory = get_thing_factory().unwrap();
            assert_eq!(
                factory.as_mut().unwrap().load_ini_text(&format!(
                    "Object {name}\n KindOf = INERT\n Behavior = PhysicsBehavior OwnedPhysics\n Mass = {mass}\n End\nEnd\n"
                )),
                1
            );
        }
        let template = TheThingFactory::find_template(name).unwrap();
        let owner = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        assert!(owner.read().unwrap().get_physics().is_none());
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .is_none()
        );
        Object::init_modules_for(&owner, template.as_ref()).unwrap();
        let (module, physics) = {
            let guard = owner.read().unwrap();
            (
                guard
                    .module_by_name(&AsciiString::from("PhysicsBehavior"))
                    .unwrap(),
                guard
                    .get_physics()
                    .expect("authored physics attached to its exact driving Object"),
            )
        };
        // This is a second descriptor of the existing installed entry, not a
        // separately constructed or injected physics module.
        let canonical = PhysicsInterfaceHandle::from_module(module.clone());
        assert!(physics.same_instance(&canonical));
        assert_eq!(physics.access().unwrap().get_mass(), mass);
        Self {
            owner,
            module,
            physics,
        }
    }
}

#[test]
fn authored_physics_same_id_owners_install_distinct_canonical_state() {
    if !child(concat!(
        module_path!(),
        "::authored_physics_same_id_owners_install_distinct_canonical_state"
    )) {
        return;
    }
    let first = Installed::new("PhysicsInstallFirst", 3.0);
    let second = Installed::new("PhysicsInstallSecond", 9.0);
    assert!(!first.physics.same_instance(&second.physics));
    first
        .physics
        .access()
        .unwrap()
        .set_velocity(&Vec3::new(1.0, 2.0, 3.0));
    second
        .physics
        .access()
        .unwrap()
        .set_velocity(&Vec3::new(-4.0, 0.0, 5.0));
    assert_eq!(
        first.physics.access().unwrap().get_velocity(),
        Vec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(
        second.physics.access().unwrap().get_velocity(),
        Vec3::new(-4.0, 0.0, 5.0)
    );
    let retained = first.physics.clone();
    drop(first);
    assert_eq!(
        retained.access().unwrap().get_velocity(),
        Vec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(second.physics.access().unwrap().get_mass(), 9.0);
    assert!(
        second
            .owner
            .read()
            .unwrap()
            .get_physics()
            .unwrap()
            .same_instance(&second.physics)
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn authored_physics_binding_xfer_mutates_the_installed_interface_state() {
    if !child(concat!(
        module_path!(),
        "::authored_physics_binding_xfer_mutates_the_installed_interface_state"
    )) {
        return;
    }
    let first = Installed::new("PhysicsXferFirst", 3.0);
    let second = Installed::new("PhysicsXferSecond", 9.0);
    first
        .physics
        .access()
        .unwrap()
        .set_velocity(&Vec3::new(7.0, 11.0, 13.0));
    let mut bytes = Vec::new();
    first.module.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    second.module.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap()
    });
    assert_eq!(
        second.physics.access().unwrap().get_velocity(),
        Vec3::new(7.0, 11.0, 13.0)
    );
    assert_eq!(
        second.physics.access().unwrap().get_mass(),
        3.0,
        "C++ Physics Xfer serializes mutable mass"
    );
    assert_eq!(
        first.physics.access().unwrap().get_velocity(),
        Vec3::new(7.0, 11.0, 13.0)
    );
    assert!(!first.physics.same_instance(&second.physics));
}
