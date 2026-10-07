//! Held-installed-AI coverage for the real Guard state callbacks.
//! This deliberately stops at AIGuardMachine: canonical UnitAI state-machine
//! admission still requires hq-lmpdw and is not fabricated by this fixture.
use super::{AIGuardMachine, GuardStateType};
use crate::common::Coord3D;
use crate::helpers::TheGameLogic;
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate};
use crate::modules::AIUpdateInterface;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, Mutex, RwLock};

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_AIGUARD_HELD_AI_CHILD",
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

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let mut loco = LocomotorTemplate::new("AIGuardHeldLoco".to_string());
    loco.preferred_height = 18.0;
    LOCOMOTOR_STORE.register_template(loco);
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object AIGuardHeldUnit\n KindOf = VEHICLE\n Behavior = AIUpdateInterface AIGuardHeldAI\n End\n Locomotor = SET_NORMAL AIGuardHeldLoco\nEnd\n"
        ),
        1
    );
}

struct ActualUnit {
    _factory: ObjectFactory,
    owner: Arc<RwLock<crate::object::Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
}

impl ActualUnit {
    fn new() -> Self {
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "AIGuardHeldUnit",
                Coord3D::new(20.0, 30.0, 0.0),
                None,
                ObjectCreationFlags::empty(),
            )
            .expect("create authored mobile unit");
        let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
        assert_eq!(
            ai.lock()
                .unwrap()
                .get_locomotor_set_clone()
                .unwrap()
                .active_name(),
            Some("AIGuardHeldLoco")
        );
        Self {
            _factory: factory,
            owner,
            ai,
        }
    }
}

#[test]
fn guard_machine_callbacks_reuse_the_held_installed_ai() {
    if !child(concat!(
        module_path!(),
        "::guard_machine_callbacks_reuse_the_held_installed_ai"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let unit = ActualUnit::new();
    let position = *unit.owner.read().unwrap().get_position();

    // This is the actual AIUpdateInterface instance installed on the authored
    // object. OLD Return entry/update re-locks this same module through the owner.
    let mut ai = unit.ai.lock().expect("installed AIUpdateInterface handle");
    let mut machine = AIGuardMachine::new(Arc::downgrade(&unit.owner));
    machine.set_target_position_to_guard(&position);
    machine.set_guard_mode(crate::ai::GuardMode::Normal);
    // This factory-created owner is present in GameLogic, but this isolated
    // fixture deliberately has no UNIT_REGISTRY admission. Return enters the
    // real movement helper and reports its existing "unit no longer available"
    // failure instead of manufacturing a scheduler-visible Unit registration.
    let transition = machine.set_state_with_ai(GuardStateType::Return, &mut *ai);
    assert!(transition.is_failure());
}
