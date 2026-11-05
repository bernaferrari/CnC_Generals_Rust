//! Typed access to a template's authored behavior modules.
//!
//! Mirrors C++ `ThingTemplate::getBehaviorModuleInfo()` as consumed by
//! `Object::Object` / `ModuleFactory::newModule`
//! (GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingTemplate.cpp,
//! GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp): every
//! per-object behavior is selected by the module declarations the template
//! authors, never by the template's name.
//!
//! The data comes only from this session's admitted receiving definition
//! (`ThingTemplate::receiving_client_template`, falling back to the raw
//! receiving `ObjectDefinition` for class presence). A template without
//! admitted content authors no modules.

use super::ThingTemplate;

impl ThingTemplate {
    /// True when this template authors a behavior module of `class`
    /// (case-insensitive, as C++ INI module names are).
    pub(crate) fn authors_behavior_module(&self, class: &str) -> bool {
        if let Some(template) = self.receiving_client_template.as_ref() {
            if template
                .behavior_modules()
                .iter()
                .any(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            {
                return true;
            }
        }
        self.receiving_object_definition
            .as_deref()
            .is_some_and(|definition| {
                definition
                    .behavior_modules
                    .iter()
                    .any(|module| module.class_name.eq_ignore_ascii_case(class))
            })
    }

    /// First authored behavior module of `class` whose typed module data is `T`.
    pub(crate) fn authored_module_data<T: 'static>(&self, class: &str) -> Option<&T> {
        self.receiving_client_template
            .as_ref()?
            .behavior_modules()
            .iter()
            .filter(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            .find_map(|entry| entry.data.downcast_ref::<T>())
    }

    /// Every authored behavior module of `class` whose typed data is `T`,
    /// in declaration order.
    pub(crate) fn authored_modules_data<T: 'static>(&self, class: &str) -> Vec<&T> {
        let Some(template) = self.receiving_client_template.as_ref() else {
            return Vec::new();
        };
        template
            .behavior_modules()
            .iter()
            .filter(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            .filter_map(|entry| entry.data.downcast_ref::<T>())
            .collect()
    }
}
