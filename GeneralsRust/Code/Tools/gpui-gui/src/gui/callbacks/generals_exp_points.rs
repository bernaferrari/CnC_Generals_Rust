use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/GeneralsExpPoints.cpp",
    "crate::gui::callbacks::generals_exp_points",
    "Generals Experience Points",
    "Ports general-points presentation and rank progression callback logic.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "General Points",
    "General points and promotion callback logic.",
);
