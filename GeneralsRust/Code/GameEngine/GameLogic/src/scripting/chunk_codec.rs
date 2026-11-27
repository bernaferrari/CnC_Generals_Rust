//! Script archive entry points for the compatibility engine.

use super::core::new_script_group;
use super::engine::with_script_engine_ref;
pub use generals_sim::scripting::chunk_codec::{
    ScriptListReadInfo, parse_player_scripts_list_chunk,
};
#[cfg(test)]
pub(crate) use generals_sim::scripting::chunk_codec::{
    write_action_data_chunk, write_condition_data_chunk, write_parameter, write_scripts_data_chunk,
};
pub use generals_sim::scripting::program::ScriptTemplateLookup;

pub fn active_script_list_reader() -> ScriptListReadInfo {
    ScriptListReadInfo::with_template_source(
        || with_script_engine_ref(|engine| engine.script_template_lookup()).unwrap_or_default(),
        new_script_group,
    )
}
