use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ControlBarPopupDescription.cpp",
    "crate::gui::callbacks::control_bar_popup_description",
    "Control Bar Popup Description",
    "Builds tooltip and popup-description content for control bar buttons.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Popup Description",
    "Tooltip and popup-description callback logic.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TooltipSubjectPort {
    CommandButton,
    MoneyDisplay,
    PowerWindow,
    GeneralsExp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TooltipContentPort {
    pub name: String,
    pub cost: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarPopupDescriptionPort {
    pub visible: bool,
    pub show_requested: bool,
    pub wait_started_at_ms: Option<u64>,
    pub wait_delay_ms: u64,
    pub previous_window_id: Option<i32>,
    pub animation_reversed: bool,
    pub deleted: bool,
    pub use_animation: bool,
    pub content: TooltipContentPort,
    pub panel_height: i32,
}

impl Default for ControlBarPopupDescriptionPort {
    fn default() -> Self {
        Self {
            visible: false,
            show_requested: false,
            wait_started_at_ms: None,
            wait_delay_ms: 500,
            previous_window_id: None,
            animation_reversed: false,
            deleted: false,
            use_animation: true,
            content: TooltipContentPort {
                name: "Scorpion Tank".to_string(),
                cost: Some("$600".to_string()),
                description: "Fast anti-armor unit.\nRequires Arms Dealer.".to_string(),
            },
            panel_height: 102,
        }
    }
}

impl ControlBarPopupDescriptionPort {
    pub fn show_build_tooltip_layout(
        &mut self,
        window_id: i32,
        tooltip_delay_ms: u64,
        now_ms: u64,
    ) -> bool {
        self.deleted = false;
        self.show_requested = true;
        self.wait_delay_ms = tooltip_delay_ms;

        if self.previous_window_id == Some(window_id) {
            if let Some(wait_started_at_ms) = self.wait_started_at_ms {
                if wait_started_at_ms + tooltip_delay_ms > now_ms {
                    return false;
                }
            }
        } else if self.visible {
            if self.use_animation && !self.animation_reversed {
                self.animation_reversed = true;
            } else if !self.use_animation {
                self.delete_build_tooltip_layout();
            }
            self.previous_window_id = Some(window_id);
            self.wait_started_at_ms = Some(now_ms);
            return false;
        } else {
            self.previous_window_id = Some(window_id);
            self.wait_started_at_ms = Some(now_ms);
            return false;
        }

        self.visible = true;
        self.animation_reversed = false;
        true
    }

    pub fn populate_command_tooltip(
        &mut self,
        name: impl Into<String>,
        cost: Option<u32>,
        description: impl Into<String>,
        requirements: &[&str],
        status_note: Option<&str>,
    ) {
        let mut description = description.into();
        if let Some(status_note) = status_note {
            if !description.is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(status_note);
        }
        if !requirements.is_empty() {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str("Requirements: ");
            description.push_str(&requirements.join(", "));
        }

        self.content = TooltipContentPort {
            name: name.into(),
            cost: cost.map(|cost| format!("${cost}")),
            description,
        };
        self.recalculate_panel_height();
    }

    pub fn populate_generic_tooltip(
        &mut self,
        subject: TooltipSubjectPort,
        power_production: i32,
        power_consumption: i32,
    ) {
        self.content = match subject {
            TooltipSubjectPort::MoneyDisplay => TooltipContentPort {
                name: "Money".to_string(),
                cost: None,
                description: "Displays your current credits and spending capacity.".to_string(),
            },
            TooltipSubjectPort::PowerWindow => TooltipContentPort {
                name: "Power".to_string(),
                cost: None,
                description: format!(
                    "Current power balance.\nProduction: {power_production}\nConsumption: {power_consumption}"
                ),
            },
            TooltipSubjectPort::GeneralsExp => TooltipContentPort {
                name: "General's Experience".to_string(),
                cost: None,
                description: "Tracks promotion progress and unlockable science points."
                    .to_string(),
            },
            TooltipSubjectPort::CommandButton => self.content.clone(),
        };
        self.recalculate_panel_height();
    }

    pub fn update(&mut self, game_ending: bool, animate_windows_enabled: bool) {
        if game_ending {
            self.hide_build_tooltip_layout(animate_windows_enabled);
        }

        if self.animation_reversed && !self.show_requested {
            self.delete_build_tooltip_layout();
        }
    }

    pub fn repopulate(&mut self) -> bool {
        self.previous_window_id.is_some() && !self.deleted
    }

    pub fn hide_build_tooltip_layout(&mut self, animate_windows_enabled: bool) {
        self.show_requested = false;
        if self.animation_reversed {
            return;
        }
        if self.use_animation && animate_windows_enabled {
            self.animation_reversed = true;
        } else {
            self.delete_build_tooltip_layout();
        }
    }

    pub fn delete_build_tooltip_layout(&mut self) {
        self.visible = false;
        self.show_requested = false;
        self.previous_window_id = None;
        self.wait_started_at_ms = None;
        self.animation_reversed = false;
        self.deleted = true;
    }

    fn recalculate_panel_height(&mut self) {
        let lines = self.content.description.lines().count().max(1) as i32;
        self.panel_height = (102).max(78 + lines * 14);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_waits_before_showing_same_window() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();

        assert!(!tooltip.show_build_tooltip_layout(10, 500, 1000));
        assert!(!tooltip.show_build_tooltip_layout(10, 500, 1200));
        assert!(tooltip.show_build_tooltip_layout(10, 500, 1600));
        assert!(tooltip.visible);
    }

    #[test]
    fn populating_command_appends_requirements_and_status() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_command_tooltip(
            "Strategy Center",
            Some(2500),
            "Coordinates battle plans.",
            &["Command Center", "Power"],
            Some("Not enough money to build"),
        );

        assert_eq!(tooltip.content.cost.as_deref(), Some("$2500"));
        assert!(tooltip.content.description.contains("Not enough money"));
        assert!(tooltip
            .content
            .description
            .contains("Requirements: Command Center, Power"));
        assert!(tooltip.panel_height >= 102);
    }

    #[test]
    fn hide_without_animation_deletes_immediately() {
        let mut tooltip = ControlBarPopupDescriptionPort {
            visible: true,
            use_animation: false,
            ..Default::default()
        };

        tooltip.hide_build_tooltip_layout(false);

        assert!(tooltip.deleted);
        assert!(!tooltip.visible);
    }
}
