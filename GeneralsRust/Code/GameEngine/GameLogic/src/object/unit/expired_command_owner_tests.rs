//! Expiration must not redirect an already bound native AI to a foreign Unit.
use super::*;
#[test]
fn expired_native_command_owner_never_falls_back_to_foreign_unit() {
    if !child(concat!(
        module_path!(),
        "::expired_native_command_owner_never_falls_back_to_foreign_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_25;
    let actual = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let weak = Arc::downgrade(&actual);
    let ai = crate::object::object_factory::factory_ai::prepare_unit_ai(
        &actual,
        &crate::common::DefaultThingTemplate::new("ExpiredCommandOwner".into()),
        id,
    );
    let foreign_owner = Arc::new(RwLock::new(Object::new_test(id, 200.0)));
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            foreign_owner.clone(),
            &crate::common::DefaultThingTemplate::new("ForeignCommandOwner".into()),
        )
        .unwrap(),
    ));
    super::super::register_unit(id, &foreign);
    drop(actual);
    assert!(
        weak.upgrade().is_none(),
        "fixture must actually release native Object"
    );
    let result = ai
        .lock()
        .unwrap()
        .execute_command(&crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::Busy,
            crate::ai::CommandSourceType::FromPlayer,
        ));
    assert!(
        result.is_err(),
        "native expiration cannot become a successful foreign Unit command"
    );
    assert!(Arc::ptr_eq(
        &foreign_owner,
        &crate::object::registry::OBJECT_REGISTRY
            .get_object(id)
            .unwrap(),
    ));
}
