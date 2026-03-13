use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarCommandProcessing.cpp",
    "crate::gui::control_bar::control_bar_command_processing",
    "Control Bar Command Processing",
    "Ports command dispatch, contextual targeting, and queueability checks.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Command Processing",
    "Context-sensitive command dispatch and target gating.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandAvailabilityPort {
    Available,
    Active,
    Hidden,
    Restricted,
    NotReady,
    CantAfford,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandDispatchStatusPort {
    Used,
    NotUsed,
    EnterTargetMode,
    PlaceBuildPreview,
    MessageQueued,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDispatchPort {
    pub current_context: &'static str,
    pub queue_mode: bool,
    pub targeting_mode: bool,
    pub flashing: bool,
    pub last_message: Option<String>,
    pub last_status: CommandDispatchStatusPort,
}

impl Default for CommandDispatchPort {
    fn default() -> Self {
        Self {
            current_context: "None",
            queue_mode: false,
            targeting_mode: false,
            flashing: false,
            last_message: None,
            last_status: CommandDispatchStatusPort::NotUsed,
        }
    }
}

impl CommandDispatchPort {
    pub fn availability_for(
        can_execute: bool,
        ready: bool,
        can_afford: bool,
        hidden: bool,
        active: bool,
    ) -> CommandAvailabilityPort {
        if hidden {
            CommandAvailabilityPort::Hidden
        } else if !can_execute {
            CommandAvailabilityPort::Restricted
        } else if !ready {
            CommandAvailabilityPort::NotReady
        } else if !can_afford {
            CommandAvailabilityPort::CantAfford
        } else if active {
            CommandAvailabilityPort::Active
        } else {
            CommandAvailabilityPort::Available
        }
    }

    pub fn process_transition_ui(
        &mut self,
        selection_present: bool,
        multi_select: bool,
    ) -> CommandDispatchStatusPort {
        if !multi_select && !selection_present {
            self.current_context = "None";
            self.last_status = CommandDispatchStatusPort::NotUsed;
            return self.last_status;
        }

        self.last_status = CommandDispatchStatusPort::Used;
        self.last_status
    }

    pub fn process_command_ui(
        &mut self,
        button: &LegacyCommandButton,
        selection_present: bool,
        multi_select: bool,
        can_afford: bool,
        queue_full: bool,
        parking_full: bool,
        maxed_out: bool,
    ) -> CommandDispatchStatusPort {
        self.last_message = None;
        self.targeting_mode = false;
        self.flashing = false;

        if !multi_select && command_requires_selection(button) && !selection_present {
            self.current_context = "None";
            self.last_status = CommandDispatchStatusPort::NotUsed;
            return self.last_status;
        }

        if command_needs_target(button) {
            self.targeting_mode = true;
            self.current_context = "Targeting";
            self.last_status = CommandDispatchStatusPort::EnterTargetMode;
            return self.last_status;
        }

        match button.command {
            GuiCommandType::DozerConstruct
            | GuiCommandType::SpecialPowerConstruct
            | GuiCommandType::SpecialPowerConstructFromShortcut => {
                if !can_afford {
                    self.last_message = Some("GUI:NotEnoughMoneyToBuild".to_string());
                    self.last_status = CommandDispatchStatusPort::MessageQueued;
                } else if queue_full {
                    self.last_message = Some("GUI:ProductionQueueFull".to_string());
                    self.last_status = CommandDispatchStatusPort::MessageQueued;
                } else if parking_full {
                    self.last_message = Some("GUI:ParkingPlacesFull".to_string());
                    self.last_status = CommandDispatchStatusPort::MessageQueued;
                } else if maxed_out {
                    self.last_message = Some("GUI:UnitMaxedOut".to_string());
                    self.last_status = CommandDispatchStatusPort::MessageQueued;
                } else {
                    self.current_context = "BuildPlacement";
                    self.last_status = CommandDispatchStatusPort::PlaceBuildPreview;
                }
            }
            _ => {
                self.current_context = if self.queue_mode { "Queued" } else { "Issued" };
                self.last_status = CommandDispatchStatusPort::Used;
            }
        }

        self.last_status
    }
}

fn command_requires_selection(button: &LegacyCommandButton) -> bool {
    !matches!(
        button.command,
        GuiCommandType::PurchaseScience
            | GuiCommandType::SpecialPowerFromShortcut
            | GuiCommandType::SpecialPowerConstructFromShortcut
            | GuiCommandType::SelectAllUnitsOfType
    )
}

fn command_needs_target(button: &LegacyCommandButton) -> bool {
    button.options.intersects(
        CommandOption::NEED_TARGET_ENEMY_OBJECT
            | CommandOption::NEED_TARGET_NEUTRAL_OBJECT
            | CommandOption::NEED_TARGET_ALLY_OBJECT
            | CommandOption::NEED_TARGET_POS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_commands_enter_target_mode() {
        let mut dispatch = CommandDispatchPort::default();
        let button = LegacyCommandButton {
            label: "Attack",
            command: GuiCommandType::AttackMove,
            options: CommandOption::NEED_TARGET_POS,
            progress: 0.0,
            enabled: true,
        };

        let status = dispatch.process_command_ui(&button, true, false, true, false, false, false);

        assert_eq!(status, CommandDispatchStatusPort::EnterTargetMode);
        assert!(dispatch.targeting_mode);
    }

    #[test]
    fn missing_selection_blocks_normal_commands() {
        let mut dispatch = CommandDispatchPort::default();
        let button = LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::empty(),
            progress: 0.0,
            enabled: true,
        };

        let status = dispatch.process_command_ui(&button, false, false, true, false, false, false);

        assert_eq!(status, CommandDispatchStatusPort::NotUsed);
    }

    #[test]
    fn construct_command_reports_insufficient_funds() {
        let mut dispatch = CommandDispatchPort::default();
        let button = LegacyCommandButton {
            label: "Build Strategy Center",
            command: GuiCommandType::DozerConstruct,
            options: CommandOption::empty(),
            progress: 0.0,
            enabled: true,
        };

        let status = dispatch.process_command_ui(&button, true, false, false, false, false, false);

        assert_eq!(status, CommandDispatchStatusPort::MessageQueued);
        assert_eq!(
            dispatch.last_message.as_deref(),
            Some("GUI:NotEnoughMoneyToBuild")
        );
    }
}
