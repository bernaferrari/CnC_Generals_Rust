use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarUnderConstruction.cpp",
    "crate::gui::control_bar::control_bar_under_construction",
    "Control Bar Under Construction",
    "Ports building-under-construction progress and option locking.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Under Construction",
    "Construction progress and locked command presentation.",
);

#[derive(Clone, Debug, PartialEq)]
pub struct UnderConstructionPort {
    pub object_name: String,
    pub construction_percent: i32,
    pub displayed_construct_percent: i32,
    pub description_text: String,
    pub cancel_command: LegacyCommandButton,
    pub rally_point_visible: bool,
    pub completed: bool,
}

impl Default for UnderConstructionPort {
    fn default() -> Self {
        Self::populate("Strategy Center", 66, true)
    }
}

impl UnderConstructionPort {
    pub fn populate(
        object_name: impl Into<String>,
        construction_percent: i32,
        has_rally_point: bool,
    ) -> Self {
        let object_name = object_name.into();
        let mut state = Self {
            object_name,
            construction_percent,
            displayed_construct_percent: -1,
            description_text: String::new(),
            cancel_command: LegacyCommandButton {
                label: "Cancel",
                command: GuiCommandType::DozerConstructCancel,
                options: CommandOption::empty(),
                progress: 0.0,
                enabled: true,
            },
            rally_point_visible: has_rally_point,
            completed: false,
        };
        state.update_construction_text_display(construction_percent);
        state
    }

    pub fn update_context(&mut self, still_under_construction: bool, current_percent: i32) -> bool {
        if !still_under_construction {
            self.completed = true;
            return true;
        }

        self.completed = false;
        if self.displayed_construct_percent != current_percent {
            self.update_construction_text_display(current_percent);
        }
        false
    }

    fn update_construction_text_display(&mut self, current_percent: i32) {
        self.construction_percent = current_percent.clamp(0, 100);
        self.displayed_construct_percent = self.construction_percent;
        self.description_text = format!(
            "{} is under construction ({}%)",
            self.object_name, self.construction_percent
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_percent_refreshes_description() {
        let mut state = UnderConstructionPort::populate("War Factory", 10, false);

        state.update_context(true, 45);

        assert_eq!(state.displayed_construct_percent, 45);
        assert!(state.description_text.contains("45%"));
    }

    #[test]
    fn completion_switches_context() {
        let mut state = UnderConstructionPort::default();

        let switched = state.update_context(false, 100);

        assert!(switched);
        assert!(state.completed);
    }
}
