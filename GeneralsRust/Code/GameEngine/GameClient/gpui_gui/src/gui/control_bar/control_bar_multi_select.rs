use super::control_bar_command_processing::CommandAvailabilityPort;
use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};
use crate::model::{CommandOption, GuiCommandType, LegacyCommandButton};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarMultiSelect.cpp",
    "crate::gui::control_bar::control_bar_multi_select",
    "Control Bar Multi Select",
    "Ports merged command-set presentation for multi-selection contexts.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Multi Select",
    "Merged command grid for multiple selected units.",
);

#[derive(Clone, Debug, PartialEq)]
pub struct SelectedUnitPort {
    pub template_name: String,
    pub portrait_name: String,
    pub ignored_in_gui: bool,
    pub sold: bool,
    pub commands: Vec<Option<LegacyCommandButton>>,
    pub availabilities: Vec<CommandAvailabilityPort>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiSelectPort {
    pub common_commands: Vec<Option<LegacyCommandButton>>,
    pub portrait_name: Option<String>,
    pub selected_units: usize,
    pub actionable_units: usize,
    pub objects_that_can_do_command: Vec<usize>,
}

impl MultiSelectPort {
    pub fn populate(units: &[SelectedUnitPort]) -> Self {
        let slot_count = units
            .iter()
            .map(|unit| unit.commands.len())
            .max()
            .unwrap_or(0);
        let mut common_commands = vec![None; slot_count];
        let mut portrait_name: Option<String> = None;
        let mut first_unit = true;
        let mut actionable_units = 0;

        for unit in units
            .iter()
            .filter(|unit| !unit.ignored_in_gui && !unit.sold)
        {
            actionable_units += 1;

            if first_unit {
                for (index, command) in unit.commands.iter().enumerate() {
                    if let Some(command) = command {
                        if command.options.contains(CommandOption::OK_FOR_MULTI_SELECT) {
                            common_commands[index] = Some(command.clone());
                        }
                    }
                }
                portrait_name = Some(unit.portrait_name.clone());
                first_unit = false;
                continue;
            }

            if portrait_name.as_deref() != Some(unit.portrait_name.as_str()) {
                portrait_name = None;
            }

            for index in 0..slot_count {
                let command = unit.commands.get(index).and_then(|command| command.clone());
                let existing = common_commands[index].clone();
                let attack_move = existing
                    .as_ref()
                    .map(|command| command.command == GuiCommandType::AttackMove)
                    .unwrap_or(false)
                    || command
                        .as_ref()
                        .map(|command| command.command == GuiCommandType::AttackMove)
                        .unwrap_or(false);

                if attack_move && existing.is_none() {
                    common_commands[index] = command;
                } else if !attack_move && !same_command_slot(existing.as_ref(), command.as_ref()) {
                    common_commands[index] = None;
                }
            }
        }

        let mut objects_that_can_do_command = vec![0; slot_count];
        for unit in units
            .iter()
            .filter(|unit| !unit.ignored_in_gui && !unit.sold)
        {
            for index in 0..slot_count {
                if common_commands[index].is_none() {
                    continue;
                }

                let availability = unit
                    .availabilities
                    .get(index)
                    .copied()
                    .unwrap_or(CommandAvailabilityPort::Available);

                if matches!(
                    availability,
                    CommandAvailabilityPort::Available | CommandAvailabilityPort::Active
                ) {
                    objects_that_can_do_command[index] += 1;
                }
            }
        }

        Self {
            common_commands,
            portrait_name,
            selected_units: units.len(),
            actionable_units,
            objects_that_can_do_command,
        }
    }

    pub fn visible_commands(&self) -> Vec<LegacyCommandButton> {
        self.common_commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                command.clone().map(|mut command| {
                    command.enabled = self
                        .objects_that_can_do_command
                        .get(index)
                        .copied()
                        .unwrap_or_default()
                        > 0;
                    command
                })
            })
            .collect()
    }

    pub fn sample() -> Self {
        let guard = LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };
        let attack = LegacyCommandButton {
            label: "Attack",
            command: GuiCommandType::AttackMove,
            options: CommandOption::OK_FOR_MULTI_SELECT | CommandOption::NEED_TARGET_POS,
            progress: 0.0,
            enabled: true,
        };
        let stop = LegacyCommandButton {
            label: "Stop",
            command: GuiCommandType::Stop,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };

        Self::populate(&[
            SelectedUnitPort {
                template_name: "AmericaTankCrusader".to_string(),
                portrait_name: "Portrait_Crusader".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![
                    Some(attack.clone()),
                    Some(guard.clone()),
                    Some(stop.clone()),
                ],
                availabilities: vec![
                    CommandAvailabilityPort::Available,
                    CommandAvailabilityPort::Available,
                    CommandAvailabilityPort::Available,
                ],
            },
            SelectedUnitPort {
                template_name: "AmericaVehicleAmbulance".to_string(),
                portrait_name: "Portrait_Crusader".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![None, Some(guard.clone()), Some(stop.clone())],
                availabilities: vec![
                    CommandAvailabilityPort::Restricted,
                    CommandAvailabilityPort::Available,
                    CommandAvailabilityPort::Available,
                ],
            },
            SelectedUnitPort {
                template_name: "AmericaVehicleDozer".to_string(),
                portrait_name: "Portrait_Dozer".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![None, None, Some(stop)],
                availabilities: vec![
                    CommandAvailabilityPort::Restricted,
                    CommandAvailabilityPort::Restricted,
                    CommandAvailabilityPort::Available,
                ],
            },
        ])
    }
}

fn same_command_slot(
    left: Option<&LegacyCommandButton>,
    right: Option<&LegacyCommandButton>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.label == right.label
                && left.command == right.command
                && left.options == right.options
        }
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_move_is_retained_when_only_one_unit_has_it() {
        let attack = LegacyCommandButton {
            label: "Attack",
            command: GuiCommandType::AttackMove,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };
        let guard = LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };

        let multi_select = MultiSelectPort::populate(&[
            SelectedUnitPort {
                template_name: "Tank".to_string(),
                portrait_name: "Tank".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![Some(attack.clone()), Some(guard.clone())],
                availabilities: vec![
                    CommandAvailabilityPort::Available,
                    CommandAvailabilityPort::Available,
                ],
            },
            SelectedUnitPort {
                template_name: "Dozer".to_string(),
                portrait_name: "Dozer".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![None, Some(guard)],
                availabilities: vec![
                    CommandAvailabilityPort::Restricted,
                    CommandAvailabilityPort::Available,
                ],
            },
        ]);

        assert_eq!(
            multi_select.common_commands[0]
                .as_ref()
                .map(|command| command.command),
            Some(GuiCommandType::AttackMove)
        );
    }

    #[test]
    fn mismatched_non_attack_commands_are_removed() {
        let guard = LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };
        let stop = LegacyCommandButton {
            label: "Stop",
            command: GuiCommandType::Stop,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };

        let multi_select = MultiSelectPort::populate(&[
            SelectedUnitPort {
                template_name: "Tank".to_string(),
                portrait_name: "Tank".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![Some(guard)],
                availabilities: vec![CommandAvailabilityPort::Available],
            },
            SelectedUnitPort {
                template_name: "Dozer".to_string(),
                portrait_name: "Dozer".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![Some(stop)],
                availabilities: vec![CommandAvailabilityPort::Available],
            },
        ]);

        assert!(multi_select.common_commands[0].is_none());
    }

    #[test]
    fn counts_only_available_and_active_commands() {
        let guard = LegacyCommandButton {
            label: "Guard",
            command: GuiCommandType::Guard,
            options: CommandOption::OK_FOR_MULTI_SELECT,
            progress: 0.0,
            enabled: true,
        };

        let multi_select = MultiSelectPort::populate(&[
            SelectedUnitPort {
                template_name: "Tank".to_string(),
                portrait_name: "Tank".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![Some(guard.clone())],
                availabilities: vec![CommandAvailabilityPort::Available],
            },
            SelectedUnitPort {
                template_name: "Ambulance".to_string(),
                portrait_name: "Tank".to_string(),
                ignored_in_gui: false,
                sold: false,
                commands: vec![Some(guard)],
                availabilities: vec![CommandAvailabilityPort::Restricted],
            },
        ]);

        assert_eq!(multi_select.objects_that_can_do_command[0], 1);
        assert!(multi_select.visible_commands()[0].enabled);
    }
}
