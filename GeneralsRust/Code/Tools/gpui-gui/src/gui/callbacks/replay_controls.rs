use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ReplayControls.cpp",
    "crate::gui::callbacks::replay_controls",
    "Replay Controls",
    "Ports replay playback controls and time navigation callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Replay Controls",
    "Replay playback and timeline callbacks.",
);
