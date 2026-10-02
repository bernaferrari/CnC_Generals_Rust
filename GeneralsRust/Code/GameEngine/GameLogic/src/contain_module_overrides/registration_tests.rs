use super::*;
use game_engine::common::system::SubsystemInterface;
use game_engine::common::thing::module::{ModuleInterfaceType, Thing};
use game_engine::common::thing::module_factory::ModuleFactory;

#[derive(Debug)]
struct UnattachedOwner;
impl Thing for UnattachedOwner {}

fn first_body_data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(ActiveBodyModuleData {
        max_health: 100.0,
        initial_health: 100.0,
        ..ActiveBodyModuleData::default()
    })
}

fn second_body_data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(ActiveBodyModuleData {
        max_health: 200.0,
        initial_health: 200.0,
        ..ActiveBodyModuleData::default()
    })
}

#[test]
fn real_body_registration_is_explicit_and_factory_local() {
    std::thread::spawn(|| {
        let mut first = ModuleFactory::new();
        let mut second = ModuleFactory::new();
        register_module_overrides(&mut first).unwrap();
        assert!(first.has_module_data_proc("ActiveBody", ModuleType::Behavior));
        assert!(!second.has_module_data_proc("ActiveBody", ModuleType::Behavior));
        register_module_overrides(&mut second).unwrap();
        for (factory, data_proc) in [
            (
                &mut first,
                first_body_data as game_engine::common::thing::module_factory::NewModuleDataProc,
            ),
            (
                &mut second,
                second_body_data as game_engine::common::thing::module_factory::NewModuleDataProc,
            ),
        ] {
            factory
                .register_override(
                    "ActiveBody",
                    ModuleType::Behavior,
                    body::active_body_module_factory,
                    data_proc,
                )
                .unwrap();
        }
        for (factory, health) in [(&mut first, 100.0), (&mut second, 200.0)] {
            assert_eq!(
                factory.find_module_interface_mask("ActiveBody", ModuleType::Behavior),
                ModuleInterfaceType::BODY
            );
            let data = factory
                .new_module_data_from_ini(
                    None,
                    "ActiveBody",
                    ModuleType::Behavior,
                    "ModuleTag_Body",
                )
                .unwrap();
            assert_eq!(
                data.as_any()
                    .downcast_ref::<ActiveBodyModuleData>()
                    .unwrap()
                    .max_health,
                health
            );
            // Exercise the real BodyBinding constructor, without admitting a body into
            // the still-global classic ObjectRegistry or claiming callback isolation.
            let module = factory
                .new_module(
                    Arc::new(UnattachedOwner),
                    "ActiveBody",
                    data,
                    ModuleType::Behavior,
                )
                .unwrap();
            assert_eq!(
                module.get_module_name_key(),
                NameKeyGenerator::name_to_key("ActiveBody")
            );
            assert_eq!(
                module
                    .get_module_data()
                    .as_any()
                    .downcast_ref::<ActiveBodyModuleData>()
                    .unwrap()
                    .max_health,
                health
            );
        }
        second.reset().unwrap();
        first.init().unwrap();
        assert!(first.has_module_data_proc("ActiveBody", ModuleType::Behavior));
        assert!(second.has_module_data_proc("ActiveBody", ModuleType::Behavior));
    })
    .join()
    .unwrap();
}
