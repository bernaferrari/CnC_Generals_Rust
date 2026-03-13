use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLocaleSelectPopup.cpp",
    "crate::gui::callbacks::menus::wol_locale_select_popup",
    "WOL Locale Select Popup",
    "Locale popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLocaleSelect",
    "WOL Locale",
    "Locale selection popup.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolLocaleSelectPopupPort {
    pub locales: Vec<String>,
    pub selected_index: usize,
    pub route_region: String,
}

impl Default for WolLocaleSelectPopupPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolLocaleSelectPopupPort {
    pub fn selected_locale(&self) -> Option<&str> {
        self.locales.get(self.selected_index).map(String::as_str)
    }

    pub fn sample() -> Self {
        Self {
            locales: vec![
                "English".to_string(),
                "German".to_string(),
                "French".to_string(),
            ],
            selected_index: 0,
            route_region: "North America".to_string(),
        }
    }
}
