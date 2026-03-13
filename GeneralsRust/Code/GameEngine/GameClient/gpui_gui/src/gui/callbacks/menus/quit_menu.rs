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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuitFocusPort {
    Confirm,
    Cancel,
    SaveThenQuit,
}

impl QuitFocusPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Confirm => "Confirm",
            Self::Cancel => "Cancel",
            Self::SaveThenQuit => "Save Then Quit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuitMenuPort {
    pub in_match: bool,
    pub has_unsaved_progress: bool,
    pub confirmation_text: String,
    pub default_focus: QuitFocusPort,
}

impl Default for QuitMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl QuitMenuPort {
    pub fn sample() -> Self {
        Self {
            in_match: true,
            has_unsaved_progress: true,
            confirmation_text: "Leaving now will abandon the current match.".to_string(),
            default_focus: QuitFocusPort::SaveThenQuit,
        }
    }
}
