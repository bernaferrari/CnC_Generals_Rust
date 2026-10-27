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
    /// Capture this explicitly selected definition, never an implicitly chosen
    /// compatibility final override. The original seed and its links stay intact.
    fn detached_receiving_clone(&self) -> Result<Self, String> {
        if self.get_next_override().is_some() {
            return Err(
                "Receiving admission requires an explicitly selected definition, not a linked compatibility template"
                    .into(),
            );
        }
        let mut template = self.clone();
        template.next_override = TemplateOverrideLink::Frozen;
        Ok(template)
    }

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
        let mut template = self.detached_receiving_clone()?;
        template.name_string = AsciiString::from(target_name);
        // This prepares a new definition in source history, including C++
        // ThingFactory::newOverride (copies and marks inherited entries).
        // Low-level in-place parsing remains the compatibility parser API.
        template.set_copied_from_default();
        let mut module_factory = module_factory.validation_catalog();
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

/// Runnable typed content, obtainable only by explicit namespace admission.
/// Unlike ReceivingTemplateCandidate, its module tags belong to the supplied
/// receiving catalog. The namespace itself remains with that catalog owner.
#[derive(Debug, Clone)]
pub struct AdmittedReceivingTemplate {
    template: Arc<ThingTemplate>,
}
impl AdmittedReceivingTemplate {
    pub fn name(&self) -> &str {
        self.template.get_name().as_str()
    }
    /// C++ GameLogic tests the admitted template's `IsBridge` flag.
    pub fn is_bridge(&self) -> bool {
        self.template.is_bridge()
    }
    /// ActiveBody reads these events from this exact admitted template.
    pub fn damage_sounds(&self) -> [Option<&str>; 2] {
        [
            self.template.get_sound_on_damaged(),
            self.template.get_sound_on_really_damaged(),
        ]
        .map(|event| event.map(|event| event.get_event_name()).filter(|name| !name.is_empty()))
    }
    pub fn command_set_name(&self) -> &str {
        self.template.get_command_set_string().as_str()
    }
    pub fn buildable_status(&self) -> u32 {
        self.template.get_buildable() as u32
    }
    pub fn template_id(&self) -> u16 {
        self.template.get_template_id()
    }
    /// Original immutable ThingTemplate::getThreatValue, including authored zero.
    pub fn occlusion_delay_frames(&self) -> u32 {
        self.template.get_occlusion_delay()
    }
    pub fn threat_value(&self) -> u16 {
        self.template.get_threat_value()
    }
    pub fn behavior_modules(&self) -> &ModuleInfo {
        self.template.get_behavior_module_info()
    }
    pub fn draw_modules(&self) -> &ModuleInfo {
        self.template.get_draw_module_info()
    }
    pub fn client_update_modules(&self) -> &ModuleInfo {
        self.template.get_client_update_module_info()
    }
    pub fn production_prerequisites(&self) -> &[crate::common::rts::ProductionPrerequisite] {
        self.template.get_prereqs()
    }
    pub fn build_variations(&self) -> impl Iterator<Item = &str> {
        self.template
            .get_build_variations()
            .iter()
            .map(|name| name.as_str())
    }
    pub fn reskin_name(&self) -> Option<&str> {
        self.template
            .get_reskinned_from()
            .map(|parent| parent.get_name().as_str())
    }
    pub fn is_shrubbery(&self) -> bool {
        self.template
            .is_kind_of_mask(crate::common::system::kind_of::KindOfMask::SHRUBBERY.bits())
    }
    pub fn audio_event(&self, audio_type: ThingTemplateAudioType) -> Option<&AudioEventRts> {
        self.template.audio_event(audio_type)
    }
    pub fn per_unit_sound_event(&self, name: &str) -> Option<&str> {
        self.template
            .get_per_unit_sound(&name.into())
            .map(|event| event.get_event_name())
    }
    pub(crate) fn template(&self) -> &ThingTemplate {
        &self.template
    }

    pub(crate) fn template_arc(&self) -> &Arc<ThingTemplate> {
        &self.template
    }

    pub(crate) fn from_parsed_template(template: ThingTemplate) -> Result<Self, String> {
        if !matches!(&template.next_override, TemplateOverrideLink::Frozen) {
            return Err("Admitted receiving template retains a compatibility override link".into());
        }
        Ok(Self {
            template: Arc::new(template),
        })
    }
}
impl ThingTemplate {
    /// Parse the same ordered grammar directly into the actual receiving
    /// catalog. Detached validation data is neither extracted nor promoted.
    pub fn admit_named_receiving_module_source(
        &self,
        target_name: &str,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
        module_factory: &mut ModuleFactory,
    ) -> Result<AdmittedReceivingTemplate, String> {
        if target_name.is_empty() {
            return Err("Receiving template name cannot be empty".into());
        }
        if module_factory.receiving_name_keys_mut().is_none() {
            return Err("Runtime module admission requires an owned name-key namespace".into());
        }
        if kind == ReceivingTemplateKind::Reskin {
            module_fields::validate_receiving_reskin_fields(properties)?;
        }
        // The factory/namespace fork is committed only after the complete
        // declaration succeeds. A rejected candidate cannot consume live IDs.
        let mut admitted_factory = module_factory.clone();
        let mut template = self.detached_receiving_clone()?;
        template.name_string = AsciiString::from(target_name);
        template.set_copied_from_default();
        template.parse_receiving_fields(properties, load_type, kind, &mut admitted_factory)?;
        let admitted = AdmittedReceivingTemplate::from_parsed_template(template)?;
        *module_factory = admitted_factory;
        Ok(admitted)
    }
    /// Actual ordered content producer uses the original scalar parser once as
    /// well as strict typed modules. Existing definition-service dependencies
    /// remain explicit compatibility scope until their separate migrations.
    pub(crate) fn parse_receiving_definition_fields(
        &mut self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
        modules: &mut ModuleFactory,
    ) -> Result<(), String> {
        if kind == ReceivingTemplateKind::Reskin {
            module_fields::validate_receiving_reskin_fields(properties)?;
        }
        self.parse_object_fields_with_scalar_policy(
            properties,
            load_type,
            Some(modules),
            true,
            true,
        )
    }

    pub(crate) fn parse_receiving_fields(
        &mut self,
        properties: &HashMap<String, String>,
        load_type: crate::common::ini::INILoadType,
        kind: ReceivingTemplateKind,
        module_factory: &mut ModuleFactory,
    ) -> Result<(), String> {
        if kind == ReceivingTemplateKind::Reskin {
            module_fields::validate_receiving_reskin_fields(properties)?;
        }
        self.parse_object_fields_with_policy(properties, load_type, Some(module_factory), true)?;
        for field in ObjectIniFields::new(properties).declarations() {
            if field.name().eq_ignore_ascii_case("UnitSpecificSounds") {
                self.load_per_unit_sounds_from_properties(properties, field.key());
            }
        }
        Ok(())
    }
}
