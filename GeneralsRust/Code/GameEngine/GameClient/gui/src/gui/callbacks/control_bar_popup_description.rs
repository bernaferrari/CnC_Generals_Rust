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
pub enum CanMakeStatus {
    Ok,
    NoPrereq,
    NoMoney,
    FactoryDisabled,
    QueueFull,
    ParkingPlacesFull,
    MaxedOutForPlayer,
}

impl CanMakeStatus {
    fn status_message(&self, is_structure: bool) -> Option<&'static str> {
        match self {
            CanMakeStatus::Ok => None,
            CanMakeStatus::NoMoney => Some("Not enough money to build"),
            CanMakeStatus::QueueFull => Some("Cannot purchase because build queue is full"),
            CanMakeStatus::ParkingPlacesFull => Some("Cannot build unit because parking is full"),
            CanMakeStatus::MaxedOutForPlayer => {
                if is_structure {
                    Some("Cannot build building because maximum number reached")
                } else {
                    Some("Cannot build unit because maximum number reached")
                }
            }
            CanMakeStatus::NoPrereq | CanMakeStatus::FactoryDisabled => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScienceGateInfo {
    pub science_valid: bool,
    pub fire_science_button: bool,
    pub missing_science: bool,
    pub science_name: Option<String>,
    pub science_description: Option<String>,
    pub science_cost: u32,
}

impl Default for ScienceGateInfo {
    fn default() -> Self {
        Self {
            science_valid: false,
            fire_science_button: false,
            missing_science: false,
            science_name: None,
            science_description: None,
            science_cost: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThingTemplateTooltipInput {
    pub name: String,
    pub description: String,
    pub cost_to_build: u32,
    pub unsatisfied_prerequisites: Vec<String>,
    pub can_make_status: CanMakeStatus,
    pub is_structure: bool,
    pub science_gate: ScienceGateInfo,
}

#[derive(Clone, Debug)]
pub struct UpgradeTooltipInput {
    pub name: String,
    pub description: String,
    pub cost_to_build: u32,
    pub already_has_upgrade: bool,
    pub has_conflicting_upgrade: bool,
    pub missing_science: bool,
    pub purchased_label: Option<String>,
    pub conflicting_label: Option<String>,
    pub queue_full: bool,
    pub cannot_afford: bool,
}

#[derive(Clone, Debug)]
pub struct SciencePurchaseTooltipInput {
    pub name: String,
    pub description: String,
    pub cost_to_build: u32,
    pub unsatisfied_prerequisites: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum BuildTooltipInput {
    ThingTemplate(ThingTemplateTooltipInput),
    Upgrade(UpgradeTooltipInput),
    SciencePurchase(SciencePurchaseTooltipInput),
}

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
                name: String::new(),
                cost: None,
                description: String::new(),
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

    pub fn populate_build_tooltip(&mut self, input: BuildTooltipInput) {
        match input {
            BuildTooltipInput::ThingTemplate(ref tt) => self.populate_thing_template_tooltip(tt),
            BuildTooltipInput::Upgrade(ref upg) => self.populate_upgrade_tooltip(upg),
            BuildTooltipInput::SciencePurchase(ref sci) => {
                self.populate_science_purchase_tooltip(sci)
            }
        }
        self.recalculate_panel_height();
    }

    pub fn populate_power_tooltip(&mut self, production: i32, consumption: i32) {
        let description = if production != 0 || consumption != 0 {
            format!("Production: {production}\nConsumption: {consumption}")
        } else {
            "Production: 0\nConsumption: 0".to_string()
        };
        self.content = TooltipContentPort {
            name: "Power".to_string(),
            cost: None,
            description,
        };
        self.recalculate_panel_height();
    }

    pub fn populate_generals_exp_tooltip(&mut self) {
        self.content = TooltipContentPort {
            name: "General's Experience".to_string(),
            cost: None,
            description: "Tracks promotion progress and unlockable science points.".to_string(),
        };
        self.recalculate_panel_height();
    }

    pub fn populate_money_tooltip(&mut self) {
        self.content = TooltipContentPort {
            name: "Money".to_string(),
            cost: None,
            description: "Displays your current credits and spending capacity.".to_string(),
        };
        self.recalculate_panel_height();
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
        match subject {
            TooltipSubjectPort::MoneyDisplay => self.populate_money_tooltip(),
            TooltipSubjectPort::PowerWindow => {
                self.populate_power_tooltip(power_production, power_consumption)
            }
            TooltipSubjectPort::GeneralsExp => self.populate_generals_exp_tooltip(),
            TooltipSubjectPort::CommandButton => {}
        }
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

    fn populate_thing_template_tooltip(&mut self, tt: &ThingTemplateTooltipInput) {
        let mut description = tt.description.clone();

        if let Some(status_msg) = tt.can_make_status.status_message(tt.is_structure) {
            if !description.is_empty() {
                description.push_str("\n\n");
            }
            description.push_str(status_msg);
        }

        let cost = if tt.cost_to_build > 0 {
            Some(format!("${}", tt.cost_to_build))
        } else {
            None
        };

        if !tt.unsatisfied_prerequisites.is_empty() {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str("Requirements: ");
            description.push_str(&tt.unsatisfied_prerequisites.join(", "));
        }

        self.content = TooltipContentPort {
            name: tt.name.clone(),
            cost,
            description,
        };
    }

    fn populate_upgrade_tooltip(&mut self, upg: &UpgradeTooltipInput) {
        let mut description;
        let cost;

        if upg.has_conflicting_upgrade && !upg.already_has_upgrade {
            description = upg
                .conflicting_label
                .clone()
                .unwrap_or_else(|| "Has conflicting upgrade".to_string());
            cost = None;
        } else if upg.already_has_upgrade {
            description = upg
                .purchased_label
                .clone()
                .unwrap_or_else(|| "Already upgraded".to_string());
            cost = None;
        } else {
            description = upg.description.clone();
            cost = if upg.cost_to_build > 0 {
                Some(format!("${}", upg.cost_to_build))
            } else {
                None
            };

            if upg.queue_full {
                if !description.is_empty() {
                    description.push_str("\n\n");
                }
                description.push_str("Cannot purchase because build queue is full");
            } else if upg.cannot_afford {
                if !description.is_empty() {
                    description.push_str("\n\n");
                }
                description.push_str("Not enough money to build");
            }

            if upg.missing_science {
                if !description.is_empty() {
                    description.push('\n');
                }
                description.push_str("Requirements: General's Promotion");
            }
        }

        self.content = TooltipContentPort {
            name: upg.name.clone(),
            cost,
            description,
        };
    }

    fn populate_science_purchase_tooltip(&mut self, sci: &SciencePurchaseTooltipInput) {
        let mut description = sci.description.clone();

        let cost = if sci.cost_to_build > 0 {
            Some(format!("${}", sci.cost_to_build))
        } else {
            None
        };

        if !sci.unsatisfied_prerequisites.is_empty() {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str("Requirements: ");
            description.push_str(&sci.unsatisfied_prerequisites.join(", "));
        }

        self.content = TooltipContentPort {
            name: sci.name.clone(),
            cost,
            description,
        };
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

    #[test]
    fn thing_template_tooltip_with_no_money_status() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::ThingTemplate(
            ThingTemplateTooltipInput {
                name: "Scorpion Tank".to_string(),
                description: "Fast anti-armor unit.".to_string(),
                cost_to_build: 600,
                unsatisfied_prerequisites: vec!["Arms Dealer".to_string()],
                can_make_status: CanMakeStatus::NoMoney,
                is_structure: false,
                science_gate: ScienceGateInfo::default(),
            },
        ));

        assert_eq!(tooltip.content.name, "Scorpion Tank");
        assert_eq!(tooltip.content.cost.as_deref(), Some("$600"));
        assert!(tooltip
            .content
            .description
            .contains("Not enough money to build"));
        assert!(tooltip
            .content
            .description
            .contains("Requirements: Arms Dealer"));
    }

    #[test]
    fn thing_template_tooltip_with_maxed_structure() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::ThingTemplate(
            ThingTemplateTooltipInput {
                name: "Supply Center".to_string(),
                description: "Generates supply dropzone.".to_string(),
                cost_to_build: 2000,
                unsatisfied_prerequisites: vec![],
                can_make_status: CanMakeStatus::MaxedOutForPlayer,
                is_structure: true,
                science_gate: ScienceGateInfo::default(),
            },
        ));

        assert!(tooltip
            .content
            .description
            .contains("Cannot build building because maximum number reached"));
    }

    #[test]
    fn thing_template_tooltip_ok_no_status_appended() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::ThingTemplate(
            ThingTemplateTooltipInput {
                name: "Ranger".to_string(),
                description: "Basic infantry unit.".to_string(),
                cost_to_build: 200,
                unsatisfied_prerequisites: vec![],
                can_make_status: CanMakeStatus::Ok,
                is_structure: false,
                science_gate: ScienceGateInfo::default(),
            },
        ));

        assert!(!tooltip.content.description.contains("Not enough"));
        assert!(!tooltip.content.description.contains("Requirements:"));
    }

    #[test]
    fn thing_template_tooltip_queue_full() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::ThingTemplate(
            ThingTemplateTooltipInput {
                name: "Humvee".to_string(),
                description: "Fast recon vehicle.".to_string(),
                cost_to_build: 700,
                unsatisfied_prerequisites: vec![],
                can_make_status: CanMakeStatus::QueueFull,
                is_structure: false,
                science_gate: ScienceGateInfo::default(),
            },
        ));

        assert!(tooltip
            .content
            .description
            .contains("Cannot purchase because build queue is full"));
    }

    #[test]
    fn thing_template_tooltip_parking_full() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::ThingTemplate(
            ThingTemplateTooltipInput {
                name: "Comanche".to_string(),
                description: "Attack helicopter.".to_string(),
                cost_to_build: 1500,
                unsatisfied_prerequisites: vec![],
                can_make_status: CanMakeStatus::ParkingPlacesFull,
                is_structure: false,
                science_gate: ScienceGateInfo::default(),
            },
        ));

        assert!(tooltip
            .content
            .description
            .contains("Cannot build unit because parking is full"));
    }

    #[test]
    fn upgrade_tooltip_already_purchased() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Composite Armor".to_string(),
            description: "Improved armor plating.".to_string(),
            cost_to_build: 2000,
            already_has_upgrade: true,
            has_conflicting_upgrade: false,
            missing_science: false,
            purchased_label: Some("Composite Armor already applied".to_string()),
            conflicting_label: None,
            queue_full: false,
            cannot_afford: false,
        }));

        assert_eq!(
            tooltip.content.description,
            "Composite Armor already applied"
        );
        assert!(tooltip.content.cost.is_none());
    }

    #[test]
    fn upgrade_tooltip_already_purchased_default_label() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Composite Armor".to_string(),
            description: "Improved armor plating.".to_string(),
            cost_to_build: 2000,
            already_has_upgrade: true,
            has_conflicting_upgrade: false,
            missing_science: false,
            purchased_label: None,
            conflicting_label: None,
            queue_full: false,
            cannot_afford: false,
        }));

        assert_eq!(tooltip.content.description, "Already upgraded");
    }

    #[test]
    fn upgrade_tooltip_conflicting() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Drone Armor".to_string(),
            description: "Upgrades drone defenses.".to_string(),
            cost_to_build: 1500,
            already_has_upgrade: false,
            has_conflicting_upgrade: true,
            missing_science: false,
            purchased_label: None,
            conflicting_label: Some("Conflicts with active Buggy Armor upgrade.".to_string()),
            queue_full: false,
            cannot_afford: false,
        }));

        assert!(tooltip.content.cost.is_none());
        assert!(tooltip
            .content
            .description
            .contains("Conflicts with active Buggy Armor upgrade"));
    }

    #[test]
    fn upgrade_tooltip_conflicting_default_label() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Drone Armor".to_string(),
            description: "Upgrades drone defenses.".to_string(),
            cost_to_build: 1500,
            already_has_upgrade: false,
            has_conflicting_upgrade: true,
            missing_science: false,
            purchased_label: None,
            conflicting_label: None,
            queue_full: false,
            cannot_afford: false,
        }));

        assert_eq!(tooltip.content.description, "Has conflicting upgrade");
    }

    #[test]
    fn upgrade_tooltip_missing_science_shows_requirement() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Satellite Hack".to_string(),
            description: "Reveals enemy positions.".to_string(),
            cost_to_build: 3000,
            already_has_upgrade: false,
            has_conflicting_upgrade: false,
            missing_science: true,
            purchased_label: None,
            conflicting_label: None,
            queue_full: false,
            cannot_afford: false,
        }));

        assert_eq!(tooltip.content.cost.as_deref(), Some("$3000"));
        assert!(tooltip
            .content
            .description
            .contains("Requirements: General's Promotion"));
    }

    #[test]
    fn upgrade_tooltip_queue_full() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Mines".to_string(),
            description: "Lays minefield.".to_string(),
            cost_to_build: 800,
            already_has_upgrade: false,
            has_conflicting_upgrade: false,
            missing_science: false,
            purchased_label: None,
            conflicting_label: None,
            queue_full: true,
            cannot_afford: false,
        }));

        assert!(tooltip
            .content
            .description
            .contains("Cannot purchase because build queue is full"));
    }

    #[test]
    fn upgrade_tooltip_cannot_afford() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Advanced Training".to_string(),
            description: "Veterancy upgrade.".to_string(),
            cost_to_build: 5000,
            already_has_upgrade: false,
            has_conflicting_upgrade: false,
            missing_science: false,
            purchased_label: None,
            conflicting_label: None,
            queue_full: false,
            cannot_afford: true,
        }));

        assert!(tooltip
            .content
            .description
            .contains("Not enough money to build"));
    }

    #[test]
    fn upgrade_tooltip_queue_full_takes_precedence_over_cannot_afford() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::Upgrade(UpgradeTooltipInput {
            name: "Mines".to_string(),
            description: "Lays minefield.".to_string(),
            cost_to_build: 800,
            already_has_upgrade: false,
            has_conflicting_upgrade: false,
            missing_science: false,
            purchased_label: None,
            conflicting_label: None,
            queue_full: true,
            cannot_afford: true,
        }));

        assert!(tooltip
            .content
            .description
            .contains("Cannot purchase because build queue is full"));
        assert!(!tooltip.content.description.contains("Not enough money"));
    }

    #[test]
    fn science_purchase_tooltip_with_prerequisites() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::SciencePurchase(
            SciencePurchaseTooltipInput {
                name: "Anti-Aircraft Missiles".to_string(),
                description: "Unlocks avenger missile upgrade.".to_string(),
                cost_to_build: 1,
                unsatisfied_prerequisites: vec!["Rank 3".to_string()],
            },
        ));

        assert_eq!(tooltip.content.name, "Anti-Aircraft Missiles");
        assert_eq!(tooltip.content.cost.as_deref(), Some("$1"));
        assert!(tooltip.content.description.contains("Requirements: Rank 3"));
    }

    #[test]
    fn science_purchase_tooltip_no_cost() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_build_tooltip(BuildTooltipInput::SciencePurchase(
            SciencePurchaseTooltipInput {
                name: "Free Science".to_string(),
                description: "A free science.".to_string(),
                cost_to_build: 0,
                unsatisfied_prerequisites: vec![],
            },
        ));

        assert!(tooltip.content.cost.is_none());
    }

    #[test]
    fn power_tooltip_formats_production_and_consumption() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_power_tooltip(15, 12);

        assert_eq!(tooltip.content.name, "Power");
        assert!(tooltip.content.cost.is_none());
        assert!(tooltip.content.description.contains("Production: 15"));
        assert!(tooltip.content.description.contains("Consumption: 12"));
    }

    #[test]
    fn power_tooltip_shows_zero_when_no_energy() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_power_tooltip(0, 0);

        assert!(tooltip.content.description.contains("Production: 0"));
        assert!(tooltip.content.description.contains("Consumption: 0"));
    }

    #[test]
    fn generals_exp_tooltip() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_generals_exp_tooltip();

        assert_eq!(tooltip.content.name, "General's Experience");
        assert!(tooltip.content.cost.is_none());
        assert!(!tooltip.content.description.is_empty());
    }

    #[test]
    fn money_tooltip() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();
        tooltip.populate_money_tooltip();

        assert_eq!(tooltip.content.name, "Money");
        assert!(tooltip.content.cost.is_none());
    }

    #[test]
    fn generic_tooltip_delegates_to_specialized() {
        let mut tooltip = ControlBarPopupDescriptionPort::default();

        tooltip.populate_generic_tooltip(TooltipSubjectPort::PowerWindow, 20, 18);
        assert_eq!(tooltip.content.name, "Power");

        tooltip.populate_generic_tooltip(TooltipSubjectPort::GeneralsExp, 0, 0);
        assert_eq!(tooltip.content.name, "General's Experience");

        tooltip.populate_generic_tooltip(TooltipSubjectPort::MoneyDisplay, 0, 0);
        assert_eq!(tooltip.content.name, "Money");
    }

    #[test]
    fn can_make_status_no_prereq_produces_no_message() {
        assert!(CanMakeStatus::NoPrereq.status_message(false).is_none());
        assert!(CanMakeStatus::FactoryDisabled
            .status_message(false)
            .is_none());
    }

    #[test]
    fn default_content_is_empty() {
        let tooltip = ControlBarPopupDescriptionPort::default();
        assert!(tooltip.content.name.is_empty());
        assert!(tooltip.content.cost.is_none());
        assert!(tooltip.content.description.is_empty());
    }
}
