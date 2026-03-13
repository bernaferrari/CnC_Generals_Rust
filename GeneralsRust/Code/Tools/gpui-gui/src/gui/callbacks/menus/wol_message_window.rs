use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLMessageWindow.cpp",
    "crate::gui::callbacks::menus::wol_message_window",
    "WOL Message Window",
    "WOL message callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLMessageWindow",
    "WOL Messages",
    "Online messages and inbox screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolMessagePort {
    pub from: String,
    pub subject: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolMessageWindowPort {
    pub messages: Vec<WolMessagePort>,
    pub selected_message: Option<usize>,
}

impl Default for WolMessageWindowPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolMessageWindowPort {
    pub fn sample() -> Self {
        Self {
            messages: vec![
                WolMessagePort {
                    from: "Tournament Admin".to_string(),
                    subject: "Quarterfinal bracket posted".to_string(),
                },
                WolMessagePort {
                    from: "Buddy: ZeroHourAce".to_string(),
                    subject: "Join custom lobby?".to_string(),
                },
            ],
            selected_message: Some(0),
        }
    }
}
