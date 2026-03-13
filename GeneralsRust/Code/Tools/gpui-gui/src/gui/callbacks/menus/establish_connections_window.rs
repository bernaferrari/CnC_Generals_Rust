use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/EstablishConnectionsWindow.cpp",
    "crate::gui::callbacks::menus::establish_connections_window",
    "Establish Connections Window",
    "Connection-establishment callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "EstablishConnectionsWindow",
    "Establish Connections",
    "Connection-establishment progress screen.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionStepPort {
    pub label: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishConnectionsWindowPort {
    pub peers_connected: u8,
    pub expected_peers: u8,
    pub steps: Vec<ConnectionStepPort>,
    pub cancel_requested: bool,
}

impl Default for EstablishConnectionsWindowPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl EstablishConnectionsWindowPort {
    pub fn sample() -> Self {
        Self {
            peers_connected: 3,
            expected_peers: 4,
            steps: vec![
                ConnectionStepPort {
                    label: "Create session".to_string(),
                    completed: true,
                },
                ConnectionStepPort {
                    label: "Resolve peers".to_string(),
                    completed: true,
                },
                ConnectionStepPort {
                    label: "Handshake player 4".to_string(),
                    completed: false,
                },
            ],
            cancel_requested: false,
        }
    }
}
