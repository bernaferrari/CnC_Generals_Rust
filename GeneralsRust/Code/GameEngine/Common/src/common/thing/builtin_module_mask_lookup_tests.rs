//! Actual ThingTemplate lookup preserves registered masks and field fallback.

use super::*;

struct FactoryScope {
    previous: Option<ModuleFactory>,
}
impl FactoryScope {
    fn install(current: Option<ModuleFactory>) -> Self {
        let previous = {
            let mut guard = get_module_factory().expect("module factory guard");
            std::mem::replace(&mut *guard, current)
        };
        Self { previous }
    }
}
impl Drop for FactoryScope {
    fn drop(&mut self) {
        let retired = {
            let mut guard = get_module_factory().expect("module factory guard during restore");
            std::mem::replace(&mut *guard, self.previous.take())
        };
        drop(retired);
    }
}

#[test]
fn absent_catalog_uses_builtin_then_original_field_fallback() {
    let _factory = FactoryScope::install(None);
    assert_eq!(
        lookup_module_interface_mask(
            "ActiveBody",
            ModuleType::Behavior,
            ModuleInterfaceType::UPDATE
        ),
        ModuleInterfaceType::BODY,
    );
    for (name, module_type, fallback) in [
        (
            "NotRegistered",
            ModuleType::Behavior,
            ModuleInterfaceType::UPDATE,
        ),
        (
            "activebody",
            ModuleType::Behavior,
            ModuleInterfaceType::BODY,
        ),
        ("", ModuleType::Behavior, ModuleInterfaceType::UPDATE),
        ("ActiveBody", ModuleType::Draw, ModuleInterfaceType::DRAW),
        ("W3DModelDraw", ModuleType::Draw, ModuleInterfaceType::DRAW),
        (
            "SwayClientUpdate",
            ModuleType::ClientUpdate,
            ModuleInterfaceType::CLIENT_UPDATE,
        ),
        (
            "NotRegistered",
            ModuleType::Behavior,
            ModuleInterfaceType::NONE,
        ),
    ] {
        assert_eq!(
            lookup_module_interface_mask(name, module_type, fallback),
            fallback
        );
    }
}

#[test]
fn registered_nonzero_mask_wins_and_none_retains_builtin_fallback() {
    let mut factory = ModuleFactory::new();
    factory.add_module_internal(
        None,
        None,
        ModuleType::Behavior,
        "ActiveBody",
        ModuleInterfaceType::DAMAGE,
    );
    factory.add_module_internal(
        None,
        None,
        ModuleType::Draw,
        "ActiveBody",
        ModuleInterfaceType::COLLIDE,
    );
    factory.add_module_internal(
        None,
        None,
        ModuleType::Behavior,
        "NotBuiltin",
        ModuleInterfaceType::UPGRADE,
    );
    factory.add_module_internal(
        None,
        None,
        ModuleType::Behavior,
        "StructureBody",
        ModuleInterfaceType::NONE,
    );
    let _factory = FactoryScope::install(Some(factory));
    for (name, module_type, expected) in [
        (
            "ActiveBody",
            ModuleType::Behavior,
            ModuleInterfaceType::DAMAGE,
        ),
        ("ActiveBody", ModuleType::Draw, ModuleInterfaceType::COLLIDE),
        (
            "NotBuiltin",
            ModuleType::Behavior,
            ModuleInterfaceType::UPGRADE,
        ),
        (
            "StructureBody",
            ModuleType::Behavior,
            ModuleInterfaceType::BODY,
        ),
    ] {
        assert_eq!(
            lookup_module_interface_mask(name, module_type, ModuleInterfaceType::UPDATE),
            expected
        );
    }
    assert_eq!(
        lookup_module_interface_mask("NotRegistered", ModuleType::Draw, ModuleInterfaceType::DRAW),
        ModuleInterfaceType::DRAW,
    );
}
