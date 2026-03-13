use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarObserver.cpp",
    "crate::gui::control_bar::control_bar_observer",
    "Control Bar Observer",
    "Ports observer-mode overlays and passive HUD presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "Observer Mode",
    "Observer-specific HUD composition and restrictions.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverPlayerPort {
    pub name: String,
    pub units: u16,
    pub buildings: u16,
    pub units_killed: u16,
    pub units_lost: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarObserverPort {
    pub players: Vec<ObserverPlayerPort>,
    pub look_at_player: Option<usize>,
}

impl Default for ControlBarObserverPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarObserverPort {
    pub fn set_look_at_player(&mut self, index: Option<usize>) {
        self.look_at_player = index.filter(|index| *index < self.players.len());
    }

    pub fn current_player(&self) -> Option<&ObserverPlayerPort> {
        self.look_at_player
            .and_then(|index| self.players.get(index))
    }

    pub fn sample() -> Self {
        Self {
            players: vec![
                ObserverPlayerPort {
                    name: "USA".to_string(),
                    units: 32,
                    buildings: 11,
                    units_killed: 18,
                    units_lost: 9,
                },
                ObserverPlayerPort {
                    name: "China".to_string(),
                    units: 27,
                    buildings: 13,
                    units_killed: 22,
                    units_lost: 14,
                },
            ],
            look_at_player: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_invalid_player_clears_observer_target() {
        let mut observer = ControlBarObserverPort::sample();
        observer.set_look_at_player(Some(99));

        assert!(observer.current_player().is_none());
    }
}
