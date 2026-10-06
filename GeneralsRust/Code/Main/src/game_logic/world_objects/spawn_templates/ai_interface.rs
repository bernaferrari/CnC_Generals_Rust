use super::*;

impl GameLogic {
    /// C++ Object.cpp:419–426 caches the actual declared AI interface.
    /// Auto-acquire flags and locomotor presence cannot establish that fact.
    pub(super) fn apply_authored_ai_interface_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use game_engine::common::thing::module_factory::builtin_behavior_has_ai_update_interface;

        let mut presence = Some(false);
        for module in &definition.behavior_modules {
            match builtin_behavior_has_ai_update_interface(&module.class_name) {
                Some(true) => {
                    presence = Some(true);
                    break;
                }
                Some(false) => {}
                None => presence = None,
            }
        }
        template.set_authored_ai_update_interface(presence);
    }
}
