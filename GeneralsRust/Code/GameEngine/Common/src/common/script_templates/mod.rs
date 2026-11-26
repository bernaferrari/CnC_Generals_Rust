//! Authored script definitions shared by the editor and the gameplay interpreter.

mod template_metadata;
mod template_names;
mod types;

use crate::common::ini::ini_script::ScriptTemplateDefinition;
use crate::common::ini::{INI, INILoadType, INIResult};
use crate::common::name_key_generator::NameKeyNamespace;
use std::path::Path;

pub use types::{
    ActionTemplate, ConditionTemplate, ConditionType, ParameterType, ScriptActionType, Template,
};

/// One owner's action and condition templates, indexed by their C++ ordinals.
pub struct ScriptTemplateCatalog {
    actions: Vec<ActionTemplate>,
    conditions: Vec<ConditionTemplate>,
}

impl ScriptTemplateCatalog {
    /// Build the authored defaults and intern condition names before action names.
    pub fn new(names: &mut NameKeyNamespace) -> Self {
        let mut actions = vec![ActionTemplate::new(); ScriptActionType::NumItems as usize];
        let mut conditions = vec![ConditionTemplate::new(); ConditionType::NumItems as usize];
        template_metadata::initialize(&mut actions, &mut conditions);
        for (template, name) in conditions
            .iter_mut()
            .zip(template_names::CONDITION_INTERNAL_NAMES)
        {
            template.base.internal_name = name.to_owned();
            template.base.internal_name_key = names.name_to_key(name);
        }
        for (template, name) in actions
            .iter_mut()
            .zip(template_names::ACTION_INTERNAL_NAMES)
        {
            template.base.internal_name = name.to_owned();
            template.base.internal_name_key = names.name_to_key(name);
        }
        Self {
            actions,
            conditions,
        }
    }

    pub fn actions(&self) -> &[ActionTemplate] {
        &self.actions
    }

    pub fn conditions(&self) -> &[ConditionTemplate] {
        &self.conditions
    }

    /// Allow fixtures to exercise chunk mappings with adversarial name keys.
    #[cfg(feature = "test-ambient-services")]
    pub fn action_mut(&mut self, kind: ScriptActionType) -> Option<&mut ActionTemplate> {
        self.actions.get_mut(kind as usize)
    }

    /// Allow fixtures to exercise chunk mappings with adversarial name keys.
    #[cfg(feature = "test-ambient-services")]
    pub fn condition_mut(&mut self, kind: ConditionType) -> Option<&mut ConditionTemplate> {
        self.conditions.get_mut(kind as usize)
    }

    /// Apply editor labels from the shared INI file authority.
    pub fn load_labels(&mut self, filename: impl AsRef<Path>) -> INIResult<()> {
        self.read_labels(|ini| ini.load(filename, INILoadType::Overwrite))
    }

    /// Apply labels from the supplied INI source.
    pub fn parse_labels(&mut self, source: &str) -> INIResult<()> {
        self.read_labels(|ini| ini.with_inline_source(source, INI::parse_current_file))
    }

    fn read_labels(&mut self, parse: impl FnOnce(&mut INI) -> INIResult<()>) -> INIResult<()> {
        let mut ini = INI::new();
        let result = parse(&mut ini);
        // C++ applies completed blocks in file order, even before a later parse error.
        for definition in ini.take_script_template_definitions() {
            let (label, target) = match definition {
                ScriptTemplateDefinition::Action(label) => {
                    let target = self
                        .actions
                        .iter_mut()
                        .find(|template| template.base.internal_name == label.internal_name);
                    (label, target.map(|template| &mut template.base))
                }
                ScriptTemplateDefinition::Condition(label) => {
                    let target = self
                        .conditions
                        .iter_mut()
                        .find(|template| template.base.internal_name == label.internal_name);
                    (label, target.map(|template| &mut template.base))
                }
            };
            if let Some(target) = target {
                target.ui_name = label.ui_name;
                target.ui_name2 = label.ui_name2;
                target.help_text = label.help_text;
            } else {
                log::debug!(
                    "Couldn't find script template named {}",
                    label.internal_name
                );
            }
        }
        result
    }
}
