//! Transactional, explicitly catalog-owned receiving content validation.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceivingTemplateKind {
    Object,
    Reskin,
}

/// Detached candidate. Neither input template nor catalog was modified.
/// This is validation capability only: numeric name identity has not been
/// admitted to a runtime owner, so typed data/catalog cannot be extracted.
pub struct ReceivingTemplateCandidate {
    template: ThingTemplate,
    module_factory: ModuleFactory,
}

/// Immutable validation evidence without runnable module data or a factory.
#[derive(Debug, Clone)]
pub struct ReceivingTemplateSummary {
    pub name: String,
    pub behavior: Vec<ModuleDescriptor>,
    pub draw: Vec<ModuleDescriptor>,
    pub client_update: Vec<ModuleDescriptor>,
}

impl ReceivingTemplateCandidate {
    pub fn summary(&self) -> ReceivingTemplateSummary {
        ReceivingTemplateSummary {
            name: self.template.get_name().to_string(),
            behavior: self
                .template
                .get_behavior_module_info()
                .iter()
                .map(|entry| entry.to_descriptor())
                .collect(),
            draw: self
                .template
                .get_draw_module_info()
                .iter()
                .map(|entry| entry.to_descriptor())
                .collect(),
            client_update: self
                .template
                .get_client_update_module_info()
                .iter()
                .map(|entry| entry.to_descriptor())
                .collect(),
        }
    }

    /// Continue a detached source history without exposing unkeyed data.
    pub fn prepare_named_receiving_candidate(
        &self,
        target_name: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
    ) -> Result<Self, String> {
        self.template.prepare_named_receiving_candidate(
            target_name,
            properties,
            load_type,
            kind,
            &self.module_factory,
        )
    }

    #[cfg(test)]
    fn template(&self) -> &ThingTemplate {
        &self.template
    }

    #[cfg(test)]
    fn into_parts(self) -> (ThingTemplate, ModuleFactory) {
        (self.template, self.module_factory)
    }
}

impl ThingTemplate {
    /// Validate module grammar, typed module bodies, and KindOf/IsTrainable
    /// context. Other Object scalar domains are intentionally not parsed here:
    /// their compatibility readers may depend on ambient content services.
    pub fn prepare_receiving_candidate(
        &self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
        module_factory: &ModuleFactory,
    ) -> Result<ReceivingTemplateCandidate, String> {
        self.prepare_named_receiving_candidate(
            self.get_name().as_str(),
            properties,
            load_type,
            kind,
            module_factory,
        )
    }

    /// Child/default copies retain receiving identity before declaration parsing.
    pub fn prepare_named_receiving_candidate(
        &self,
        target_name: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
        module_factory: &ModuleFactory,
    ) -> Result<ReceivingTemplateCandidate, String> {
        if target_name.is_empty() {
            return Err("Receiving template name cannot be empty".to_owned());
        }
        if kind == ReceivingTemplateKind::Reskin {
            module_fields::validate_receiving_reskin_fields(properties)?;
        }
        let mut template = self.clone();
        template.name_string = AsciiString::from(target_name);
        // This prepares a new definition in source history, including C++
        // ThingFactory::newOverride (copies and marks inherited entries).
        // Low-level in-place parsing remains the compatibility parser API.
        template.set_copied_from_default();
        let mut module_factory = module_factory.clone();
        template.parse_object_fields_with_policy(
            properties,
            load_type,
            Some(&mut module_factory),
            true,
        )?;
        Ok(ReceivingTemplateCandidate {
            template,
            module_factory,
        })
    }
}

#[cfg(test)]
#[path = "receiving_candidate_tests.rs"]
mod tests;
