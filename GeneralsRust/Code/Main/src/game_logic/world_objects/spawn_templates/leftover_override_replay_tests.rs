//! hq-0v6gq: Common `record_object_create_override` already overlays the live
//! catalog once (C++ ThingFactory::parseObjectDefinition -> newOverride, once
//! per parse). Host pending/all retrieval must read that result, not replay
//! AddModule onto the already-overlaid definition.
use crate::game_logic::GameLogic;
use game_engine::common::ini::{INI, INILoadType};
use game_engine::common::thing::thing_factory::ThingFactory;

fn module_count(name: &str) -> usize {
    let manager = crate::assets::manager::get_asset_manager().unwrap();
    let manager = manager.lock().unwrap();
    let definition = manager.resolve_object_definition(name, None).unwrap();
    definition.behavior_modules.len()
}

#[test]
fn pending_leftover_override_does_not_replay_add_module() {
    let _main = crate::main_services::MainServicesScope::new();
    let _common = game_engine::common::common_services::CommonServicesScope::new();
    *crate::main_services::main_services()
        .asset_manager
        .lock()
        .unwrap() = Some(std::sync::Arc::new(std::sync::Mutex::new(
        crate::assets::manager::AssetManager::new().unwrap(),
    )));
    let mut logic = GameLogic::new();
    GameLogic::register_leftover_object_create_overrides_overlay();

    let name = "ReplayAddModuleProbe";
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        source.path(),
        "AddModule ModuleTag_Probe01\n  Behavior = SlowDeathBehavior ModuleTag_Probe02\n  End\nEnd\nEnd\n",
    )
    .unwrap();
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    let mut result = Ok(());
    ini.with_file_source(source.path(), INILoadType::CreateOverrides, |ini| {
        result = factory.parse_object_definition(ini, name, "");
        Ok(())
    })
    .unwrap();
    result.unwrap();

    let after_parse = module_count(name);
    assert!(after_parse >= 1, "Common record must overlay AddModule once");
    logic.apply_pending_leftover_object_override(name);
    logic.apply_pending_leftover_object_override(name);
    logic.apply_all_leftover_object_create_overrides();
    assert_eq!(module_count(name), after_parse, "host lookup replayed AddModule");
}
