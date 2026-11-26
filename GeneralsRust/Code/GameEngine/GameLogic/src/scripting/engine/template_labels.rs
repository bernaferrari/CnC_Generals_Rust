use super::ScriptEngine;
use game_engine::common::ini::INIResult;

impl ScriptEngine {
    /// Apply prepared INI labels to this interpreter's owned catalog.
    pub fn parse_template_labels(&mut self, source: &str) -> INIResult<()> {
        self.inner.get_mut().templates.parse_labels(source)
    }
}

#[cfg(test)]
#[path = "tests/template_label_tests.rs"]
mod tests;
