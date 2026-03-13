use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarResizer.cpp",
    "crate::gui::control_bar::control_bar_resizer",
    "Control Bar Resizer",
    "Ports resolution-aware control bar anchoring and resize behavior.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Resizer",
    "Resolution-aware anchoring and resizing rules.",
);

#[derive(Clone, Debug, PartialEq)]
pub struct ControlBarResizerPort {
    pub screen_width: i32,
    pub screen_height: i32,
    pub default_x: i32,
    pub default_y: i32,
    pub current_x: i32,
    pub current_y: i32,
}

impl Default for ControlBarResizerPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarResizerPort {
    pub fn apply_stage_offset(&mut self, x_offset: i32, y_offset: i32) {
        self.current_x = self.default_x + x_offset;
        self.current_y = self.default_y + y_offset;
    }

    pub fn sample() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
            default_x: 0,
            default_y: 768,
            current_x: 0,
            current_y: 768,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_stage_offset_moves_control_bar() {
        let mut resizer = ControlBarResizerPort::sample();
        resizer.apply_stage_offset(24, -32);

        assert_eq!((resizer.current_x, resizer.current_y), (24, 736));
    }
}
