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
