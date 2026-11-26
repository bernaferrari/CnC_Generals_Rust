use serde::{Deserialize, Serialize};

pub use generals_game_domain::script_opcodes::{ConditionType, ParameterType, ScriptActionType};

/// Template class matching C++ Template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub ui_name: String,
    pub ui_name2: String,
    pub internal_name: String,
    pub internal_name_key: u32,
    pub num_ui_strings: usize,
    pub ui_strings: Vec<String>,
    pub num_parameters: usize,
    pub parameters: Vec<ParameterType>,
    pub help_text: String,
    #[cfg(feature = "count_script_usage")]
    pub num_times_used: i32,
    #[cfg(feature = "count_script_usage")]
    pub first_map_used: String,
}

impl Template {
    pub fn new() -> Self {
        Self {
            ui_name: "UNUSED/(placeholder)/placeholder".to_owned(),
            ui_name2: String::new(),
            internal_name: String::new(),
            internal_name_key: 0,
            num_ui_strings: 0,
            ui_strings: Vec::new(),
            num_parameters: 0,
            parameters: Vec::new(),
            help_text: String::new(),
            #[cfg(feature = "count_script_usage")]
            num_times_used: 0,
            #[cfg(feature = "count_script_usage")]
            first_map_used: String::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.ui_name
    }

    pub fn get_name2(&self) -> &str {
        &self.ui_name2
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parameters
    }

    pub fn get_parameter_type(&self, index: usize) -> Option<ParameterType> {
        self.parameters.get(index).copied()
    }
}

/// Condition template matching C++ ConditionTemplate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionTemplate {
    pub base: Template,
}

impl ConditionTemplate {
    pub fn new() -> Self {
        Self {
            base: Template::new(),
        }
    }

    pub fn qualify(
        &mut self,
        _qualifier: &str,
        _player_template_name: &str,
        _new_player_name: &str,
    ) {
    }
}

/// Action template matching C++ ActionTemplate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    pub base: Template,
}

impl ActionTemplate {
    pub fn new() -> Self {
        Self {
            base: Template::new(),
        }
    }
}
