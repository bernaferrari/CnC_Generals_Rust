use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "EstablishConnectionsMenu/EstablishConnectionsMenu.cpp",
    "crate::gui::establish_connections_menu",
    "Establish Connections Menu",
    "Owns the connection-establishment overlay and status messaging flow.",
);

#[derive(Clone, Debug)]
pub struct EstablishConnectionsPort {
    pub stage: String,
    pub progress: f32,
    pub peers_connected: usize,
}

impl Default for EstablishConnectionsPort {
    fn default() -> Self {
        Self {
            stage: "Negotiating Session".to_string(),
            progress: 0.25,
            peers_connected: 1,
        }
    }
}

impl EstablishConnectionsPort {
    pub fn advance(&mut self, stage: impl Into<String>, progress: f32, peers_connected: usize) {
        self.stage = stage.into();
        self.progress = progress.clamp(0.0, 1.0);
        self.peers_connected = peers_connected;
    }
}
