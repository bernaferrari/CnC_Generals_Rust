use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/IMECandidate.cpp",
    "crate::gui::callbacks::ime_candidate",
    "IME Candidate",
    "Ports IME candidate list display and selection callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "IME Candidate",
    "IME candidate rendering and selection callbacks.",
);
