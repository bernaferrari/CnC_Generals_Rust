pub mod shell;
pub mod shell_menu_scheme;

use crate::gui::source_catalog::GuiPortRecord;

pub fn records() -> Vec<&'static GuiPortRecord> {
    vec![&shell::RECORD, &shell_menu_scheme::RECORD]
}
