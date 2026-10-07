//! C++ Object.cpp:424-461 exposes the installed AI before creation callbacks.
use super::*;
use crate::object::update::ai_update_interface::{AIUpdateInterfaceModule, AIUpdateModuleData};

const AI_PROBE: &str = "FactoryAiCreationProbe";

struct AiCreationProbe {
    owner_id: ObjectID,
    data: Arc<dyn ModuleData>,
    created: bool,
}

impl Module for AiCreationProbe {
    fn get_module_name_key(&self) -> u32 {
        NameKeyGenerator::name_to_key(AI_PROBE)
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
    fn on_object_created(&mut self) {
        assert!(!self.created);
        let expected = self.data.get_module_tag_name_key()
            == NameKeyGenerator::name_to_key("ExpectAiBeforeCreation");
        let owner = OBJECT_REGISTRY.get_object(self.owner_id).unwrap();
        let owner = owner.try_read().expect("creation callback releases owner");
        assert!(!owner.modules_ready);
        assert!(owner.has_ctor_helpers());
        assert_eq!(owner.modules.len(), 2);
        let ai = owner.get_ai_update_interface();
        let modules = owner.modules.clone();
        drop(owner);
        assert_eq!(ai.is_some(), expected, "AI visibility at onObjectCreated");
        if let Some(ai) = &ai {
            assert_eq!(
                ai.lock().unwrap().get_attitude(),
                crate::modules::AIAttitudeType::Aggressive,
                "C++ team attitude precedes onObjectCreated"
            );
        }
        let mut found = false;
        for entry in &modules {
            if entry.name().as_str() == "AIUpdateInterface" {
                entry.with_module(|module| {
                    let module = (module as &mut dyn Any)
                        .downcast_mut::<AIUpdateInterfaceModule>()
                        .unwrap();
                    match (module.runtime_ai_for_test(), ai.as_ref()) {
                        (Some(module_ai), Some(object_ai)) => {
                            assert!(Arc::ptr_eq(module_ai, object_ai))
                        }
                        (None, None) => {}
                        _ => panic!("Object and installed module must retain the same AI"),
                    }
                    found = true;
                });
            }
        }
        assert!(found);
        self.created = true;
    }
}
impl Snapshotable for AiCreationProbe {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
fn ai_probe_factory(thing: Arc<dyn ModuleThing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    Box::new(AiCreationProbe {
        owner_id: thing.as_object().unwrap().get_object_id(),
        data,
        created: false,
    })
}
fn ai_template(name: &str, expect_ai: bool) -> AuthoredBodyTemplate {
    let mut inner = DefaultThingTemplate::new(name.into());
    inner.add_kind_of(KindOf::Vehicle);
    let mut probe = BaseModuleData::new();
    probe.set_module_tag_name_key(NameKeyGenerator::name_to_key(if expect_ai {
        "ExpectAiBeforeCreation"
    } else {
        "ExpectNoAiBeforeCreation"
    }));
    let mut ai = AIUpdateModuleData::default();
    ModuleData::set_module_tag_name_key(&mut ai, NameKeyGenerator::name_to_key("AuthoredAi"));
    ai.set_auto_acquire_enemies_when_idle(crate::object::update::AUTO_ACQUIRE_IDLE);
    AuthoredBodyTemplate {
        inner,
        modules: vec![
            crate::common::TemplateModuleInfo {
                name: AI_PROBE.into(),
                module_tag: "AiCreationProbe".into(),
                data: Arc::new(probe),
                interface_mask: ModuleInterfaceType::NONE,
            },
            crate::common::TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "AuthoredAi".into(),
                data: Arc::new(ai),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
        ],
    }
}
fn register_ai_probe() {
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    register_module_override(
        AI_PROBE,
        ModuleType::Behavior,
        ai_probe_factory,
        probe_data_factory,
    )
    .unwrap();
    init_module_factory().unwrap();
    get_module_factory()
        .unwrap()
        .as_mut()
        .unwrap()
        .add_module_internal(
            Some(ai_probe_factory),
            Some(probe_data_factory),
            ModuleType::Behavior,
            &AsciiString::from(AI_PROBE),
            ModuleInterfaceType::NONE,
        );
}
fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_FACTORY_AI_CREATION_CHILD"
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

fn exercise_factory(no_ai: bool) {
    let _serial = crate::test_sync::lock();
    register_ai_probe();
    let mut factory = ObjectFactory::new();
    factory.next_object_id = 0xA1_F0_21;
    factory.template_cache.insert(
        "EarlyFactoryAi".into(),
        Arc::new(ai_template("EarlyFactoryAi", !no_ai)),
    );
    let team = {
        let mut teams = crate::team::get_team_factory().lock().unwrap();
        let mut prototype = crate::team::TeamPrototype::new("EarlyFactoryAiTeam".into());
        prototype.set_initial_team_attitude(crate::team::AttitudeType::Aggressive);
        teams.replace_team_prototype(prototype);
        teams.create_inactive_team("EarlyFactoryAiTeam").unwrap()
    };
    let flags = ObjectCreationFlags::NO_DRAWABLE
        | if no_ai {
            ObjectCreationFlags::NO_AI
        } else {
            ObjectCreationFlags::empty()
        };
    let id = factory
        .create_object("EarlyFactoryAi", Coord3D::default(), Some(team), flags)
        .unwrap();
    assert!(factory.get_object(id).unwrap().is_unit());
    let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
    let owner = owner.read().unwrap();
    assert!(owner.modules_ready);
    assert_eq!(owner.get_ai_update_interface().is_some(), !no_ai);
    let mut probe_found = false;
    for entry in &owner.modules {
        if entry.name().as_str() == AI_PROBE {
            entry.with_module(|module| {
                assert!(
                    (module as &mut dyn Any)
                        .downcast_mut::<AiCreationProbe>()
                        .unwrap()
                        .created
                );
                probe_found = true;
            });
        }
    }
    assert!(probe_found);
}
#[test]
fn factory_exposes_same_installed_ai_before_creation_callback() {
    if !child(concat!(
        module_path!(),
        "::factory_exposes_same_installed_ai_before_creation_callback"
    )) {
        return;
    }
    exercise_factory(false);
}
#[test]
fn no_ai_factory_keeps_module_callback_and_does_not_attach_runtime() {
    if !child(concat!(
        module_path!(),
        "::no_ai_factory_keeps_module_callback_and_does_not_attach_runtime"
    )) {
        return;
    }
    exercise_factory(true);
}
#[test]
fn direct_object_constructor_keeps_its_existing_ai_preparation_policy() {
    if !child(concat!(
        module_path!(),
        "::direct_object_constructor_keeps_its_existing_ai_preparation_policy"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    register_ai_probe();
    let template = Arc::new(ai_template("DirectObjectAiPolicy", false));
    let owner =
        Object::new_with_id(template, 0xA1_F0_22, ObjectStatusMaskType::none(), None).unwrap();
    assert!(owner.read().unwrap().get_ai_update_interface().is_none());
    OBJECT_REGISTRY.unregister_object(0xA1_F0_22);
    crate::ai::object_registry::unregister_legacy_object(0xA1_F0_22);
}
