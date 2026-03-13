use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/CreditsMenu.cpp",
    "crate::gui::callbacks::menus::credits_menu",
    "Credits Menu",
    "Credits screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "CreditsMenu",
    "Credits",
    "Scrolling credits and acknowledgements.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditsMenuPort {
    pub lines: Vec<String>,
    pub highlighted_line: usize,
    pub scroll_offset: u32,
}

impl Default for CreditsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl CreditsMenuPort {
    pub fn sample() -> Self {
        Self {
            lines: vec![
                "Engineering".to_string(),
                "Design".to_string(),
                "Audio".to_string(),
                "Quality Assurance".to_string(),
                "Community".to_string(),
            ],
            highlighted_line: 0,
            scroll_offset: 128,
        }
    }
}
