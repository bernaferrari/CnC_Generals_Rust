//! Immutable defaults from ScriptEngine.cpp:564-5242.
//! INI labels may replace names/help later; counts and active fields stay authored.
use super::{
    ActionTemplate, ConditionTemplate, ConditionType, ParameterType, ScriptActionType, Template,
};

mod actions;
mod conditions;

struct Definition {
    ui_name: &'static str,
    parameters: &'static [ParameterType],
    ui_strings: &'static [&'static str],
}

impl Definition {
    const fn new(
        ui_name: &'static str,
        parameters: &'static [ParameterType],
        ui_strings: &'static [&'static str],
    ) -> Self {
        Self {
            ui_name,
            parameters,
            ui_strings,
        }
    }

    fn apply(&self, template: &mut Template, ordinal: usize) {
        // C++ appends the ordinal even to unused placeholders (5230-5242).
        template.ui_name = format!("{}[{ordinal}]", self.ui_name);
        template.num_parameters = self.parameters.len();
        template.parameters = self.parameters.to_vec();
        template.num_ui_strings = self.ui_strings.len();
        template.ui_strings = self.ui_strings.iter().map(|s| (*s).to_owned()).collect();
    }
}

pub(super) fn initialize(actions: &mut [ActionTemplate], conditions: &mut [ConditionTemplate]) {
    for (ordinal, (template, definition)) in
        actions.iter_mut().zip(&actions::DEFINITIONS).enumerate()
    {
        definition.apply(&mut template.base, ordinal);
    }
    for (ordinal, (template, definition)) in conditions
        .iter_mut()
        .zip(&conditions::DEFINITIONS)
        .enumerate()
    {
        definition.apply(&mut template.base, ordinal);
    }
}
