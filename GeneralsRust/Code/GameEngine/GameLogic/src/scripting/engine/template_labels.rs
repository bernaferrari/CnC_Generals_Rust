//! ScriptEngine.cpp:340-416: editor labels override the first exact-name match.
use super::{ScriptEngine, Template};
use game_engine::common::ini::ini_script::{ScriptTemplate, ScriptTemplateDefinition};
use game_engine::common::ini::{INI, INILoadType, INIResult};
use std::path::Path;

impl ScriptEngine {
    /// WorldBuilder loads Scripts.ini after the authored templates are initialized.
    /// Use the shared INI file authority for loose files and mounted archives.
    pub fn load_template_labels(&mut self, filename: impl AsRef<Path>) -> INIResult<()> {
        self.read_template_labels(|ini| ini.load(filename, INILoadType::Overwrite))
    }

    /// Apply already prepared INI bytes to this engine, without selecting a global engine.
    pub fn parse_template_labels(&mut self, source: &str) -> INIResult<()> {
        self.read_template_labels(|ini| {
            ini.with_inline_source(source, |ini| ini.parse_current_file())
        })
    }

    fn read_template_labels(
        &mut self,
        parse: impl FnOnce(&mut INI) -> INIResult<()>,
    ) -> INIResult<()> {
        let mut ini = INI::new();
        let result = parse(&mut ini);
        // C++ commits completed blocks before encountering a later bad block.
        // The failing, incomplete block is never included in this ordered output.
        let inner = self.inner.get_mut();
        for definition in ini.take_script_template_definitions() {
            let (label, target) = match definition {
                ScriptTemplateDefinition::Action(label) => {
                    let target = inner
                        .action_templates
                        .iter_mut()
                        .find(|t| t.base.internal_name == label.internal_name);
                    (label, target.map(|t| &mut t.base))
                }
                ScriptTemplateDefinition::Condition(label) => {
                    let target = inner
                        .condition_templates
                        .iter_mut()
                        .find(|t| t.base.internal_name == label.internal_name);
                    (label, target.map(|t| &mut t.base))
                }
            };
            if let Some(target) = target {
                replace_labels(target, label);
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

fn replace_labels(target: &mut Template, label: ScriptTemplate) {
    target.ui_name = label.ui_name;
    target.ui_name2 = label.ui_name2;
    target.help_text = label.help_text;
}

#[cfg(test)]
#[path = "tests/template_label_tests.rs"]
mod tests;
