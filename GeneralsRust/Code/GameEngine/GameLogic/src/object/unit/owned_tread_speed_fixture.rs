//! Actual Common Draw/AI/Body definitions and factory-installed W3D draw entries.
//! Body installation is a prerequisite: there is no fallback/test body.
//! The live child list is an explicit render-content fixture, not retail W3D proof.

use crate::common::{BodyDamageType, Coord3D, Matrix3D, ObjectID};
use crate::helpers::TheGameLogic;
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate};
use crate::object::Object;
use crate::object::draw::w3d_model_draw::{
    HlodLiveChildState, publish_hlod_live_child_states, take_hlod_live_child_states,
};
use crate::object::drawable::{DrawableExt, DrawableModuleHandle};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::thing::module::ModuleInterfaceType;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::sync::{Arc, RwLock};

pub(crate) fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_OWNED_TREAD_SPEED_CHILD",
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

pub(crate) struct Fixture {
    _factory: ObjectFactory,
    pub(crate) pristine: Arc<RwLock<Object>>,
    pub(crate) damaged: Arc<RwLock<Object>>,
}

impl Fixture {
    pub(crate) fn new(prefix: &str, draw_kind: &str) -> Self {
        assert!(ensure_thing_factory_exists());
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        // Real authored armor before admission; never force a loaded/readiness flag.
        let armor_name = format!("{prefix}Armor");
        assert_eq!(
            crate::object::armor::load_armor_templates_from_str(
                &format!("Armor {armor_name}\nArmor = DEFAULT 100%\nEnd\n"),
                None,
            )
            .unwrap(),
            1
        );
        let mut ini = String::new();
        for (suffix, health, speed, damaged_speed) in
            [("Pristine", 100, 40.0, 17.0), ("Damaged", 5, 60.0, 23.0)]
        {
            let name = format!("{prefix}{suffix}");
            let mut locomotor = LocomotorTemplate::new(format!("{name}Loco"));
            locomotor.max_speed = speed;
            locomotor.max_speed_damaged = damaged_speed;
            LOCOMOTOR_STORE.register_template(locomotor);
            ini.push_str(&format!(
                "Object {name}\nKindOf = VEHICLE\nArmorSet\nConditions = NONE\nArmor = {armor_name}\nDamageFX = None\nEnd\nBody = ActiveBody {name}Body\nMaxHealth = 100\nInitialHealth = {health}\nEnd\nBehavior = AIUpdateInterface {name}AI\nEnd\nBehavior = PhysicsBehavior {name}Physics\nEnd\nLocomotor = SET_NORMAL {name}Loco\nDraw = {draw_kind} ActualTreadDraw\nTreadAnimationRate = 0.25\nDefaultConditionState\nModel = OwnedTreadPristine\nEnd\nConditionState = REALLYDAMAGED\nModel = OwnedTreadDamaged\nEnd\nEnd\nEnd\n"
            ));
        }
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(&ini),
            2
        );
        let mut factory = ObjectFactory::new();
        let mut create = |suffix| {
            let id = factory
                .create_object(
                    &format!("{prefix}{suffix}"),
                    Coord3D::new(10.0, 20.0, 0.0),
                    None,
                    ObjectCreationFlags::empty(),
                )
                .unwrap();
            let instance = factory.get_object(id).unwrap();
            assert!(
                instance.is_unit(),
                "the real factory must construct its Unit variant"
            );
            let owner = instance.get_base_object().unwrap();
            assert!(Arc::ptr_eq(
                &owner,
                &TheGameLogic::find_object_by_id(id).unwrap()
            ));
            assert!(
                super::registry::get_unit_arc(id).is_none(),
                "no fake Unit admission"
            );
            assert!(
                owner.read().unwrap().get_physics().is_some(),
                "authored Physics installation"
            );
            assert!(
                owner.read().unwrap().get_ai_update_interface().is_some(),
                "actual cached AI"
            );
            owner
        };
        let pristine = create("Pristine");
        let damaged = create("Damaged");
        for (owner, expected_health, expected_condition) in [
            (&pristine, 100.0, BodyDamageType::Pristine),
            (&damaged, 5.0, BodyDamageType::ReallyDamaged),
        ] {
            let body = owner
                .read()
                .unwrap()
                .get_body_module()
                .expect("actual authored ActiveBody cache");
            let body = body.lock().unwrap();
            assert_eq!(body.get_health(), expected_health);
            assert_eq!(body.get_damage_state(), expected_condition);
            drop(body);
            // Prove the actual cached AI already owns the correct member before
            // exercising the Draw caller. A missing member is a prerequisite,
            // not the intended absent-Unit-registry speed failure.
            let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
            let condition = match expected_condition {
                BodyDamageType::Pristine => crate::locomotor::BodyDamageType::Pristine,
                BodyDamageType::ReallyDamaged => crate::locomotor::BodyDamageType::ReallyDamaged,
                _ => unreachable!("fixture uses only pristine/really-damaged bodies"),
            };
            let mut current_speed = None;
            ai.lock().unwrap().with_cur_locomotor(&mut |locomotor| {
                current_speed = Some(locomotor.get_max_speed_for_condition(condition));
            });
            assert_eq!(
                current_speed,
                Some(if expected_health == 100.0 { 40.0 } else { 23.0 }),
                "actual cached AI current locomotor prerequisite"
            );
        }
        Self {
            _factory: factory,
            pristine,
            damaged,
        }
    }

    pub(crate) fn actual_draw(
        owner: &Arc<RwLock<Object>>,
        expected_kind: &str,
    ) -> DrawableModuleHandle {
        let (id, drawable) = {
            let owner = owner.read().unwrap();
            (
                owner.get_id(),
                owner
                    .get_drawable()
                    .expect("actual factory Drawable association"),
            )
        };
        let modules = drawable
            .read()
            .unwrap()
            .modules_with_interface(ModuleInterfaceType::DRAW);
        assert_eq!(modules.len(), 1, "one authored Draw descriptor");
        let handle = modules.into_iter().next().unwrap();
        assert_eq!(handle.name().as_str(), expected_kind);
        assert_eq!(handle.tag().as_str(), "ActualTreadDraw");
        // Explicit live render-content fixture, not a manually constructed Draw
        // or evidence of loading a retail W3D asset. Actual factory state/binding
        // has completed before this host child-content publication.
        publish_hlod_live_child_states(
            id,
            vec![
                HlodLiveChildState {
                    name: "OwnedTank.TREADSL".to_string(),
                    hidden: false,
                    local_transform: Matrix3D::IDENTITY,
                    uv_animations_disabled: true,
                },
                HlodLiveChildState {
                    name: "OwnedTank.TREADSR".to_string(),
                    hidden: false,
                    local_transform: Matrix3D::IDENTITY,
                    uv_animations_disabled: true,
                },
            ],
        );
        handle
    }

    pub(crate) fn owner_id(owner: &Arc<RwLock<Object>>) -> ObjectID {
        owner.read().unwrap().get_id()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for owner in [&self.pristine, &self.damaged] {
            let _ = take_hlod_live_child_states(owner.read().unwrap().get_id());
        }
        // Factory's actual per-object retirement follows outside all owner/AI guards.
    }
}
