use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarBeacon.cpp",
    "crate::gui::control_bar::control_bar_beacon",
    "Control Bar Beacon",
    "Ports beacon placement, deletion, and beacon-specific command presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Beacon Controls",
    "Beacon-specific buttons and targeting flow.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarBeaconPort {
    pub targeting_active: bool,
    pub beacon_count: u8,
    pub selected_beacon: Option<String>,
}

impl Default for ControlBarBeaconPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarBeaconPort {
    pub fn place_beacon(&mut self, name: impl Into<String>) {
        self.targeting_active = false;
        self.beacon_count = self.beacon_count.saturating_add(1);
        self.selected_beacon = Some(name.into());
    }

    pub fn arm_targeting(&mut self) {
        self.targeting_active = true;
    }

    pub fn delete_selected(&mut self) -> bool {
        let had_selection = self.selected_beacon.take().is_some();
        if had_selection && self.beacon_count > 0 {
            self.beacon_count -= 1;
        }
        had_selection
    }

    pub fn sample() -> Self {
        Self {
            targeting_active: true,
            beacon_count: 2,
            selected_beacon: Some("North Ridge".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_selected_beacon_reduces_count() {
        let mut beacon = ControlBarBeaconPort::sample();
        assert!(beacon.delete_selected());
        assert_eq!(beacon.beacon_count, 1);
    }
}
