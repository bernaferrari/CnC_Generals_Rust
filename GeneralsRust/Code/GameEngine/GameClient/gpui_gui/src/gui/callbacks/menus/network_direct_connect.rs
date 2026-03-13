use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/NetworkDirectConnect.cpp",
    "crate::gui::callbacks::menus::network_direct_connect",
    "Network Direct Connect",
    "Direct-connect callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "NetworkDirectConnect",
    "Direct Connect",
    "Manual network direct-connect flow.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkDirectConnectPort {
    pub host_ip: String,
    pub port: u16,
    pub nickname: String,
    pub status_message: String,
}

impl Default for NetworkDirectConnectPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl NetworkDirectConnectPort {
    pub fn sample() -> Self {
        Self {
            host_ip: "192.168.1.50".to_string(),
            port: 8088,
            nickname: "bernardo".to_string(),
            status_message: "Waiting for remote session advertisement.".to_string(),
        }
    }
}
