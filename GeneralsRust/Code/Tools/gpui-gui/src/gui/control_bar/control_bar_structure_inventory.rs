use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarStructureInventory.cpp",
    "crate::gui::control_bar::control_bar_structure_inventory",
    "Control Bar Structure Inventory",
    "Ports container and structure inventory slots shown through the command bar.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Structure Inventory",
    "Inventory and passenger slots for structures and transports.",
);
