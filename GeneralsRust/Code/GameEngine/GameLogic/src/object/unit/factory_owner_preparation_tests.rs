//! The factory already owns the Object; a same-ID legacy Unit cannot select it.
use super::*;
use crate::common::{DefaultThingTemplate, TemplateModuleInfo};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::{Arc, RwLock};

struct PreparedTemplate {
    inner: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
}
impl std::fmt::Debug for PreparedTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedTemplate").finish_non_exhaustive()
    }
}
impl crate::common::ThingTemplate for PreparedTemplate {
    fn is_kind_of(&self, kind: crate::common::KindOf) -> bool {
        self.inner.is_kind_of(kind)
    }
    fn get_name(&self) -> &crate::common::AsciiString {
        self.inner.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.inner.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.inner.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.inner.calc_shroud_clearing_range()
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}
fn template() -> PreparedTemplate {
    let mut data = AIUpdateModuleData::default();
    data.set_auto_acquire_enemies_when_idle(crate::object::update::AUTO_ACQUIRE_IDLE);
    PreparedTemplate {
        inner: DefaultThingTemplate::new("PreparedAiOwner".into()),
        modules: vec![TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "PreparedAi".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    }
}
struct Unregister(u32);
impl Drop for Unregister {
    fn drop(&mut self) {
        super::unregister_unit(self.0);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
        crate::ai::object_registry::unregister_legacy_object(self.0);
    }
}
fn prepare(owner: &Arc<RwLock<crate::object::Object>>) -> Arc<std::sync::Mutex<UnitAIUpdate>> {
    let id = owner.read().unwrap().get_id();
    crate::object::object_factory::factory_ai::prepare_unit_ai(owner, &template(), id)
}
#[test]
fn actual_factory_preparation_binds_exact_owner_without_legacy_unit_admission() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_23;
    assert!(super::registry::get_unit_arc(id).is_none());
    let first = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let rng = game_engine::common::random_value::get_game_logic_random_seed_state();
    let foreign = crate::system::game_logic::get_game_logic().lock().unwrap();
    let first_ai = prepare(&first);
    let second_ai = prepare(&second);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng
    );
    for (owner, ai) in [(&first, &first_ai), (&second, &second_ai)] {
        let ai = ai.lock().unwrap();
        let machine = ai
            .ai_state_machine
            .as_ref()
            .expect("factory FSM has an exact owner")
            .lock()
            .unwrap();
        assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), owner));
        assert_eq!(
            machine.get_current_state_id(),
            None,
            "preparation does not initialize the machine"
        );
        assert_eq!(ai.get_next_mood_check_time(), 0);
    }
    assert!(super::registry::get_unit_arc(id).is_none());
    drop(foreign);
}
#[test]
fn foreign_same_id_unit_cannot_select_factory_owner_or_receive_its_module_flags() {
    let _serial = crate::test_sync::lock();
    let id = 0xA1_F0_24;
    let actual = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let decoy = Arc::new(RwLock::new(crate::object::Object::new_test(id, 200.0)));
    let legacy = Arc::new(RwLock::new(
        Unit::new(
            decoy.clone(),
            &DefaultThingTemplate::new("LegacyDecoy".into()),
        )
        .unwrap(),
    ));
    super::register_unit(id, &legacy);
    let _unregister = Unregister(id);
    legacy.write().unwrap().auto_acquire_enemies = false;
    let ai = prepare(&actual);
    let ai = ai.lock().unwrap();
    let machine = ai.ai_state_machine.as_ref().unwrap().lock().unwrap();
    assert!(Arc::ptr_eq(&machine.base.get_owner().unwrap(), &actual));
    assert!(
        !legacy.read().unwrap().auto_acquire_enemies,
        "foreign Unit mirrors are untouched"
    );
    assert_eq!(
        ai.data.auto_acquire_enemies_when_idle,
        crate::object::update::AUTO_ACQUIRE_IDLE
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(id)
            .is_some_and(|published| Arc::ptr_eq(&published, &decoy))
    );
}
