//! Short observations of the receiving Player ledger and admitted definition.
use crate::common::system::upgrade::UpgradeTemplate;
use std::sync::Arc;

pub struct UpgradeTooltipObservation {
    pub command: Option<crate::common::ini::ini_command_button::CommandButton>,
    pub template: Option<Arc<UpgradeTemplate>>,
    pub completed: bool,
    pub object_completed: bool,
    pub object_affected: Option<bool>,
}
#[derive(Clone, Copy)]
pub enum UpgradeTooltipSource<'a> {
    Receiving(&'a dyn Fn(&str, &str) -> UpgradeTooltipObservation),
    Standalone,
}

impl UpgradeTooltipSource<'_> {
    /// None is the explicitly selected standalone adapter only. A missing
    /// receiving definition is authoritative false, never a fallback request.
    pub fn completed_upgrade(self, name: &str) -> Option<bool> {
        match self {
            Self::Standalone => None,
            Self::Receiving(observe) => {
                let observation = observe("", name);
                Some(observation.template.is_some() && observation.completed)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiving_shortcut_upgrade_gate_never_uses_standalone_completion_on_missing() {
        let name = "Upgrade_ObservedShortcut";
        let template = Arc::new(UpgradeTemplate::new(name.into()));
        let owner = |_: &str, _: &str| UpgradeTooltipObservation {
            object_completed: false,
            object_affected: None,
            command: None,
            template: Some(Arc::clone(&template)),
            completed: true,
        };
        let peer = |_: &str, _: &str| UpgradeTooltipObservation {
            object_completed: false,
            object_affected: None,
            command: None,
            template: Some(Arc::clone(&template)),
            completed: false,
        };
        let missing = |_: &str, _: &str| UpgradeTooltipObservation {
            object_completed: false,
            object_affected: None,
            command: None,
            template: None,
            completed: true,
        };
        assert_eq!(
            UpgradeTooltipSource::Receiving(&owner).completed_upgrade(name),
            Some(true)
        );
        assert_eq!(
            UpgradeTooltipSource::Receiving(&peer).completed_upgrade(name),
            Some(false)
        );
        assert_eq!(
            UpgradeTooltipSource::Receiving(&missing).completed_upgrade(name),
            Some(false)
        );
        assert_eq!(
            UpgradeTooltipSource::Standalone.completed_upgrade(name),
            None
        );
    }
}
