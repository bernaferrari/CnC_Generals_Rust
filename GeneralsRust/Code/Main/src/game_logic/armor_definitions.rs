//! Immutable receiving armor definitions. C++ ArmorStore::reset is a no-op.
pub(crate) use gamelogic::object::armor::ArmorDefinitions;
use std::sync::Arc;

impl super::GameLogic {
    /// Admit at the original content phase before factory callbacks. Rebinding
    /// existing definition views also preserves C++ named ArmorTemplate identity
    /// when a Map.ini block replaces a row in the current session.
    pub(crate) fn admit_armor_definitions(&mut self, catalog: Arc<ArmorDefinitions>) {
        self.armor_definitions = Some(Arc::clone(&catalog));
        for template in self.templates.values_mut() {
            template.armor_definitions = Some(Arc::clone(&catalog));
        }
        for object in self.objects.values_mut() {
            object.template_mut().armor_definitions = Some(Arc::clone(&catalog));
        }
    }
    pub(crate) fn current_armor_definitions(&self) -> Option<&Arc<ArmorDefinitions>> {
        self.armor_definitions.as_ref()
    }
}

#[cfg(test)]
#[path = "armor_definition_owner_tests.rs"]
mod tests;
