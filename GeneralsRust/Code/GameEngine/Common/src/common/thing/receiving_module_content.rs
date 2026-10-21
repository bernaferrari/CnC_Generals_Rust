//! Ordered receiving Object module definitions and their one name-key/data owner.
//! ThingFactory.cpp106-170,348-407,185-217 are the lifecycle authority.
//! Effective versions retain immutable base lineage, never mutable override links.
use super::{
    module_factory::ModuleFactory,
    thing_factory::{strip_ini_comment, try_parse_object_block_properties},
    thing_template::{AdmittedReceivingTemplate, ReceivingTemplateKind, ThingTemplate},
};
use crate::common::ini::INILoadType;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceivingDefinitionKind {
    Object,
    ObjectReskin,
    ChildObject,
}

#[derive(Clone)]
struct TemplateVersion {
    // None means this definition was introduced only for the current map.
    base: Option<Arc<AdmittedReceivingTemplate>>,
    current: Arc<AdmittedReceivingTemplate>,
}

#[derive(Clone)]
pub struct ReceivingModuleContent {
    templates: HashMap<String, TemplateVersion>,
    declaration_order: Vec<String>,
    next_template_id: u16,
    modules: ModuleFactory,
}

impl ReceivingModuleContent {
    /// Construction neither registers classes nor publishes a mutable owner.
    pub fn new(modules: ModuleFactory) -> Self {
        Self {
            templates: HashMap::new(),
            declaration_order: Vec::new(),
            next_template_id: 1,
            modules,
        }
    }
    pub fn template_name_for_id(&self, id: u16) -> Option<&str> {
        self.templates
            .values()
            .find(|entry| entry.current.template_id() == id)
            .map(|entry| entry.current.name())
    }
    pub fn science_type(&self, name: &str) -> Option<crate::common::rts::ScienceType> {
        self.modules.receiving_science_names().science(name)
    }
    pub fn science_name(&self, science: crate::common::rts::ScienceType) -> Option<&str> {
        self.modules.receiving_science_names().name(science)
    }
    /// Boot forward names resolve only after all ordered source files are loaded.
    pub fn validate_production_prerequisites(&self) -> Result<(), String> {
        for (_, plan) in self.admitted_templates() {
            self.validate_prerequisites_for(plan, None)?;
        }
        Ok(())
    }
    fn validate_prerequisites_for(
        &self,
        plan: &AdmittedReceivingTemplate,
        current: Option<(&str, u16)>,
    ) -> Result<(), String> {
        for prerequisite in plan.production_prerequisites() {
            let mut prerequisite = prerequisite.clone();
            prerequisite.resolve_names_with(|name| {
                let id = current
                    .filter(|(current, _)| *current == name)
                    .map(|(_, id)| id)
                    .or_else(|| {
                        self.admitted_template(name)
                            .map(|template| template.template_id())
                    })
                    .ok_or_else(|| {
                        format!(
                            "Missing receiving prerequisite '{name}' for '{}'",
                            plan.name()
                        )
                    })?;
                Ok(Some(crate::common::rts::ThingTemplateHandle::new(
                    id as u32,
                )))
            })?;
        }
        Ok(())
    }
    pub fn module_factory_mut(&mut self) -> &mut ModuleFactory {
        &mut self.modules
    }
    pub fn admitted_template(&self, name: &str) -> Option<Arc<AdmittedReceivingTemplate>> {
        self.templates
            .get(name)
            .map(|entry| Arc::clone(&entry.current))
    }
    /// Original master-list order is newest first; overrides do not add nodes.
    pub fn admitted_templates(
        &self,
    ) -> impl Iterator<Item = (&str, &Arc<AdmittedReceivingTemplate>)> {
        self.declaration_order.iter().rev().filter_map(|name| {
            self.templates
                .get(name)
                .map(|entry| (name.as_str(), &entry.current))
        })
    }
    /// Existing prereq/build-variation readers may observe this detached view at
    /// explicit accepted startup. They never provide the receiving native plan.
    pub fn compatibility_template_view(&self) -> super::thing_factory::ThingFactory {
        super::thing_factory::ThingFactory::from_receiving_versions(
            self.declaration_order.iter().filter_map(|name| {
                self.templates
                    .get(name)
                    .map(|entry| entry.current.template())
            }),
            self.next_template_id,
        )
    }

    /// Parse each actual declaration once, in supplied source order. File admission
    /// is atomic: a failed tail cannot mutate the live namespace, data list or plans.
    pub fn load_ini_text(
        &mut self,
        content: &str,
        load_type: INILoadType,
    ) -> Result<usize, String> {
        let mut candidate = self.clone();
        let lines = content.lines().collect::<Vec<_>>();
        let mut index = 0;
        let mut loaded = 0;
        while index < lines.len() {
            let line = strip_ini_comment(lines[index]).trim();
            let tokens = line.split_whitespace().collect::<Vec<_>>();
            let Some(header) = tokens.first() else {
                index += 1;
                continue;
            };
            // An Object = field in another block is not an Object declaration.
            if tokens.get(1) == Some(&"=") {
                index += 1;
                continue;
            }
            let kind = if header.eq_ignore_ascii_case("Object") {
                ReceivingDefinitionKind::Object
            } else if header.eq_ignore_ascii_case("ObjectReskin") {
                ReceivingDefinitionKind::ObjectReskin
            } else if header.eq_ignore_ascii_case("ChildObject") {
                ReceivingDefinitionKind::ChildObject
            } else {
                index += 1;
                continue;
            };
            let name = *tokens
                .get(1)
                .ok_or_else(|| format!("Missing receiving Object name at line {}", index + 1))?;
            let parent = tokens.get(2).copied();
            let (properties, next) = try_parse_object_block_properties(&lines, index + 1)?;
            candidate.admit_source_in_place(name, parent, kind, &properties, load_type)?;
            loaded += 1;
            index = next;
        }
        *self = candidate;
        Ok(loaded)
    }
    /// Map/Solo callers with already-captured properties admit just that actual
    /// declaration. Parents come from this catalog, never replayed source history.
    pub fn admit_source_definition(
        &mut self,
        name: &str,
        parent: Option<&str>,
        kind: ReceivingDefinitionKind,
        properties: &HashMap<String, String>,
        load_type: INILoadType,
    ) -> Result<Arc<AdmittedReceivingTemplate>, String> {
        let mut candidate = self.clone();
        let admitted =
            candidate.admit_source_in_place(name, parent, kind, properties, load_type)?;
        *self = candidate;
        Ok(admitted)
    }
    fn admit_source_in_place(
        &mut self,
        name: &str,
        parent: Option<&str>,
        kind: ReceivingDefinitionKind,
        properties: &HashMap<String, String>,
        load_type: INILoadType,
    ) -> Result<Arc<AdmittedReceivingTemplate>, String> {
        if name.is_empty() {
            return Err("Receiving Object name is empty".into());
        }
        if self.modules.receiving_name_keys_mut().is_none() {
            return Err("Receiving content requires its originating name-key namespace".into());
        }
        if load_type == INILoadType::Invalid {
            return Err("Invalid receiving Object load type".into());
        }
        let previous = self.templates.get(name).cloned();
        let override_load = load_type == INILoadType::CreateOverrides;
        // This catalog owns base/current versions. No compatibility override
        // slot is allocated; copy_from preserves this frozen destination mode.
        let mut template = ThingTemplate::new_receiving();
        let id = if let Some(entry) = &previous {
            entry.current.template().get_template_id()
        } else {
            let id = self.next_template_id;
            self.next_template_id = id
                .checked_add(1)
                .ok_or("Receiving template identity exhausted")?;
            id
        };
        template.set_template_id(id);
        template.set_template_name(name.into());
        if let Some(entry) = &previous {
            let source = if override_load {
                &entry.current
            } else {
                entry.base.as_ref().unwrap_or(&entry.current)
            };
            template.copy_from(source.template());
            if override_load {
                template.set_copied_from_default();
            }
        } else if let Some(default) = self.admitted_template("DefaultThingTemplate") {
            template.copy_from(default.template());
            template.set_copied_from_default();
        }
        if override_load {
            template.mark_as_override();
        }
        if kind != ReceivingDefinitionKind::Object {
            let parent = parent.ok_or("Receiving inherited Object has no parent")?;
            let source = self.admitted_template(parent).ok_or_else(|| {
                format!("Receiving parent '{parent}' must be admitted before '{name}'")
            })?;
            template.copy_from(source.template());
            template.set_copied_from_default();
            if kind == ReceivingDefinitionKind::ObjectReskin {
                template.set_reskinned_from(Arc::clone(source.template_arc()));
            }
            // ChildObject is the existing Rust content extension; the selected
            // original Generals source defines Object/ObjectReskin only.
        }
        template.parse_receiving_definition_fields(
            properties,
            load_type,
            if kind == ReceivingDefinitionKind::ObjectReskin {
                ReceivingTemplateKind::Reskin
            } else {
                ReceivingTemplateKind::Object
            },
            &mut self.modules,
        )?;
        template.validate();
        let admitted = Arc::new(AdmittedReceivingTemplate::from_parsed_template(template)?);
        // ThingFactory.cpp406 resolves override names at this declaration,
        // not after a later Object or Solo file can introduce a missing row.
        if override_load {
            self.validate_prerequisites_for(&admitted, Some((name, id)))?;
        }

        let base = if override_load {
            previous.as_ref().and_then(|entry| entry.base.clone())
        } else if previous.as_ref().is_some_and(|entry| entry.base.is_none()) {
            None
        } else {
            Some(Arc::clone(&admitted))
        };
        if previous.is_none() {
            self.declaration_order.push(name.to_owned());
        }
        self.templates.insert(
            name.to_owned(),
            TemplateVersion {
                base,
                current: Arc::clone(&admitted),
            },
        );
        Ok(admitted)
    }
    /// ThingFactory::reset removes override versions/map-only roots. ModuleFactory
    /// reset is explicitly a no-op, so its data list and namespace remain intact.
    pub fn reset_map_overrides(&mut self) {
        self.templates.retain(|_, entry| {
            if let Some(base) = &entry.base {
                entry.current = Arc::clone(base);
                true
            } else {
                false
            }
        });
        self.declaration_order
            .retain(|name| self.templates.contains_key(name));
    }
}

#[cfg(test)]
#[path = "receiving_module_content_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "receiving_audio_owner_tests.rs"]
mod audio_owner_tests;
