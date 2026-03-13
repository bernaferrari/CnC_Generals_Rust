use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ControlBarPopupDescription.cpp",
    "crate::gui::callbacks::control_bar_popup_description",
    "Control Bar Popup Description",
    "Builds tooltip and popup-description content for control bar buttons.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Popup Description",
    "Tooltip and popup-description callback logic.",
);
