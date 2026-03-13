use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/QuitMenu.cpp",
    "crate::gui::callbacks::menus::quit_menu",
    "Quit Menu",
    "Quit confirmation callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "QuitMenu",
    "Quit Menu",
    "Quit confirmation screen.",
    "Shell",
);
