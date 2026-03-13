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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureInventorySlotPort {
    pub occupant_name: String,
    pub health_pct: u8,
    pub exiting: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarStructureInventoryPort {
    pub slots: Vec<StructureInventorySlotPort>,
    pub selected_slot: Option<usize>,
}

impl Default for ControlBarStructureInventoryPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarStructureInventoryPort {
    pub fn select_slot(&mut self, index: usize) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        self.selected_slot = Some(index);
        true
    }

    pub fn sample() -> Self {
        Self {
            slots: vec![
                StructureInventorySlotPort {
                    occupant_name: "Ranger".to_string(),
                    health_pct: 100,
                    exiting: false,
                },
                StructureInventorySlotPort {
                    occupant_name: "Missile Defender".to_string(),
                    health_pct: 86,
                    exiting: false,
                },
                StructureInventorySlotPort {
                    occupant_name: "Drone".to_string(),
                    health_pct: 100,
                    exiting: true,
                },
            ],
            selected_slot: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_valid_slot_updates_state() {
        let mut inventory = ControlBarStructureInventoryPort::sample();
        assert!(inventory.select_slot(2));
        assert_eq!(inventory.selected_slot, Some(2));
    }
}
