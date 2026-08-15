//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Stale override factory tests.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bits_upgrade_parses_status_lists() {
        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("set mask parsed");
        data.set_status_to_clear_from_tokens(&["+MASKED"])
            .expect("clear mask parsed");

        let set_mask = data.status_to_set();
        assert!(set_mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(set_mask.contains(ObjectStatusMaskType::DETECTED));
        let clear_mask = data.status_to_clear();
        assert!(clear_mask.contains(ObjectStatusMaskType::MASKED));
    }

    #[test]
    fn status_bits_upgrade_data_factory_sets_defaults() {
        let data = status_bits_upgrade_module_data_factory(None);
        let typed = data
            .as_ref()
            .downcast_ref::<StatusBitsUpgradeModuleData>()
            .unwrap();
        assert!(typed.status_to_set().is_empty());
        assert!(typed.status_to_clear().is_empty());
    }

    #[test]
    fn module_factory_uses_status_bits_override() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module_factory::ModuleFactory;

        install_module_overrides().expect("install overrides");

        let mut factory = ModuleFactory::new();
        let name = AsciiString::from("StatusBitsUpgrade");
        factory.add_module_internal(
            None,
            None,
            ModuleType::Behavior,
            &name,
            ModuleInterfaceType::UPGRADE.0 as i32,
        );

        let module_tag = AsciiString::from("TagStatusBits");
        let data = factory
            .new_module_data_from_ini(None, &name, ModuleType::Behavior, &module_tag)
            .expect("module data via override");

        #[derive(Debug)]
        struct StubThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> = Arc::new(StubThing { id: 99 });

        let module = factory
            .new_module(thing, &name, data.clone(), ModuleType::Behavior)
            .expect("module via override");

        assert!(data.as_ref().as_any().is::<StatusBitsUpgradeModuleData>());
        assert!(module
            .get_module_data()
            .as_any()
            .is::<StatusBitsUpgradeModuleData>());
    }

    #[test]
    fn stealth_update_data_parses_status_tokens() {
        let mut data = StealthUpdateModuleData::default();
        data.set_hint_detectable_states_from_tokens(&["STEALTHED", "DETECTED"])
            .expect("hint detectable parsed");
        data.set_required_status_from_tokens(&["CAN_STEALTH"])
            .expect("required parsed");
        data.set_forbidden_status_from_tokens(&["+MASKED"])
            .expect("forbidden parsed");

        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::STEALTHED));
        assert!(data
            .hint_detectable_states()
            .contains(ObjectStatusMaskType::DETECTED));
        assert!(data
            .required_status()
            .contains(ObjectStatusMaskType::CAN_STEALTH));
        assert!(data
            .forbidden_status()
            .contains(ObjectStatusMaskType::MASKED));
    }

    #[test]
    fn status_bits_upgrade_factory_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;

        #[derive(Debug)]
        struct StubThing;

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                0
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED"])
            .expect("status to set parsed");
        let module = status_bits_upgrade_module_factory(
            Arc::new(StubThing) as Arc<dyn ModuleThing>,
            Arc::new(data) as Arc<dyn ModuleData>,
        );

        let typed_data = module
            .get_module_data()
            .as_any()
            .downcast_ref::<StatusBitsUpgradeModuleData>()
            .expect("upgrade module data downcasts");
        assert!(typed_data
            .status_to_set()
            .contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn stealth_update_factory_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;

        #[derive(Debug)]
        struct StubThing;

        impl ModuleObjectTrait for StubThing {
            fn get_object_id(&self) -> ObjectID {
                0
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                None
            }
        }

        impl ModuleThing for StubThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let data = Arc::new(StealthUpdateModuleData::default());
        let module = stealth_update_module_factory(
            Arc::new(StubThing) as Arc<dyn ModuleThing>,
            data.clone() as Arc<dyn ModuleData>,
        );

        let typed_data = module
            .get_module_data()
            .as_any()
            .downcast_ref::<StealthUpdateModuleData>()
            .expect("stealth module data downcasts");
        assert_eq!(typed_data.required_status(), ObjectStatusMaskType::none());
    }

    #[test]
    fn auto_heal_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubHealThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubHealThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubHealThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubHealThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubHealThing { id: 777 }) as Arc<dyn ModuleThing>;

        let data_box = auto_heal_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = auto_heal_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<AutoHealBehaviorModuleData>()
                .is_some(),
            "AutoHeal override should return typed module data"
        );
    }

    #[test]
    fn dumb_projectile_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubProjectileThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubProjectileThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubProjectileThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubProjectileThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubProjectileThing { id: 456 }) as Arc<dyn ModuleThing>;

        let data_box = dumb_projectile_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = dumb_projectile_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<DumbProjectileBehaviorModuleData>()
                .is_some(),
            "DumbProjectile override should return typed module data"
        );
    }

    #[test]
    fn countermeasures_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubCounterThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubCounterThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubCounterThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubCounterThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubCounterThing { id: 123 }) as Arc<dyn ModuleThing>;

        let data_box = countermeasures_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = countermeasures_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<CountermeasuresBehaviorModuleData>()
                .is_some(),
            "Countermeasures override should return typed module data"
        );
    }

    #[test]
    fn bunker_buster_override_produces_concrete_module() {
        use crate::common::ObjectID;
        use game_engine::common::thing::module::ModuleData;
        use std::sync::RwLock;

        #[derive(Debug, Clone)]
        struct StubBunkerThing {
            id: ObjectID,
        }

        impl ModuleObjectTrait for StubBunkerThing {
            fn get_object_id(&self) -> ObjectID {
                self.id
            }

            fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
                let arc: Arc<RwLock<StubBunkerThing>> = Arc::new(RwLock::new(self.clone()));
                Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
            }
        }

        impl ModuleThing for StubBunkerThing {
            fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
                Some(self)
            }

            fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
                None
            }
        }

        let thing: Arc<dyn ModuleThing> =
            Arc::new(StubBunkerThing { id: 456 }) as Arc<dyn ModuleThing>;

        let data_box = bunker_buster_behavior_module_data_factory(None);
        let module_data: Arc<dyn ModuleData> = data_box.into();

        let module = bunker_buster_behavior_module_factory(thing, module_data);
        assert!(
            module
                .get_module_data()
                .as_any()
                .downcast_ref::<BunkerBusterBehaviorModuleData>()
                .is_some(),
            "BunkerBuster override should return typed module data"
        );
    }
}
#[test]
fn battle_bus_slow_death_override_produces_concrete_module() {
    use crate::common::ObjectID;
    use game_engine::common::thing::module::ModuleData;
    use std::sync::RwLock;

    #[derive(Debug, Clone)]
    struct StubBusThing {
        id: ObjectID,
    }

    impl ModuleObjectTrait for StubBusThing {
        fn get_object_id(&self) -> ObjectID {
            self.id
        }

        fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObjectTrait>>> {
            let arc: Arc<RwLock<StubBusThing>> = Arc::new(RwLock::new(self.clone()));
            Some(arc as Arc<RwLock<dyn ModuleObjectTrait>>)
        }
    }

    impl ModuleThing for StubBusThing {
        fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
            Some(self)
        }

        fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
            None
        }
    }

    let thing: Arc<dyn ModuleThing> = Arc::new(StubBusThing { id: 321 }) as Arc<dyn ModuleThing>;

    let data_box = battle_bus_slow_death_behavior_module_data_factory(None);
    let module_data: Arc<dyn ModuleData> = data_box.into();

    let module = battle_bus_slow_death_behavior_module_factory(thing, module_data);
    assert!(
        module
            .get_module_data()
            .as_any()
            .downcast_ref::<BattleBusSlowDeathBehaviorModuleData>()
            .is_some(),
        "BattleBusSlowDeath override should return typed module data"
    );
}
