use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanLobbyMenu.cpp",
    "crate::gui::callbacks::menus::lan_lobby_menu",
    "LAN Lobby Menu",
    "LAN lobby callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanLobbyMenu",
    "LAN Lobby",
    "LAN lobby player list and chat.",
    "LAN",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanLobbyPlayerPort {
    pub name: String,
    pub faction: String,
    pub color: String,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanLobbyMenuPort {
    pub visible: bool,
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub socket_error_detected: bool,
    pub just_entered: bool,
    pub initial_gadget_delay: u16,
    pub player_name: String,
    pub chat_entry: String,
    pub chat_history: Vec<String>,
    pub players: Vec<LanLobbyPlayerPort>,
    pub selected_player: Option<usize>,
    pub next_screen: Option<String>,
    pub active_transition_group: Option<String>,
}

impl Default for LanLobbyMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl LanLobbyMenuPort {
    pub fn init(player_name: impl Into<String>, players: Vec<LanLobbyPlayerPort>) -> Self {
        Self {
            visible: true,
            is_shutting_down: false,
            button_pushed: false,
            socket_error_detected: false,
            just_entered: true,
            initial_gadget_delay: 2,
            player_name: player_name.into(),
            chat_entry: String::new(),
            chat_history: vec!["System: Welcome to the LAN lobby.".to_string()],
            selected_player: (!players.is_empty()).then_some(0),
            players,
            next_screen: None,
            active_transition_group: None,
        }
    }

    pub fn update(&mut self) {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.active_transition_group = Some("LanLobbyMenuFade".to_string());
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay = self.initial_gadget_delay.saturating_sub(1);
            }
        }
    }

    pub fn select_player(&mut self, index: usize) -> bool {
        if index >= self.players.len() {
            return false;
        }
        self.selected_player = Some(index);
        true
    }

    pub fn send_chat(&mut self) -> bool {
        let message = self.chat_entry.trim();
        if message.is_empty() {
            return false;
        }
        self.chat_history
            .push(format!("{}: {}", self.player_name, message));
        self.chat_entry.clear();
        true
    }

    pub fn host_game(&mut self) {
        self.button_pushed = true;
        self.next_screen = Some("Menus/LanGameOptionsMenu.wnd".to_string());
    }

    pub fn direct_connect(&mut self) {
        self.button_pushed = true;
        self.next_screen = Some("Menus/NetworkDirectConnect.wnd".to_string());
    }

    pub fn back(&mut self) {
        self.button_pushed = true;
        self.is_shutting_down = true;
    }

    pub fn sample() -> Self {
        Self::init(
            "bernardo",
            vec![
                LanLobbyPlayerPort {
                    name: "bernardo".to_string(),
                    faction: "USA".to_string(),
                    color: "Blue".to_string(),
                    ready: true,
                },
                LanLobbyPlayerPort {
                    name: "guest42".to_string(),
                    faction: "GLA".to_string(),
                    color: "Green".to_string(),
                    ready: false,
                },
                LanLobbyPlayerPort {
                    name: "ai_hard_2".to_string(),
                    faction: "China".to_string(),
                    color: "Red".to_string(),
                    ready: true,
                },
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sending_chat_appends_message_and_clears_entry() {
        let mut lobby = LanLobbyMenuPort::sample();
        lobby.chat_entry = "ready up".to_string();

        assert!(lobby.send_chat());
        assert_eq!(
            lobby.chat_history.last().map(String::as_str),
            Some("bernardo: ready up")
        );
        assert!(lobby.chat_entry.is_empty());
    }

    #[test]
    fn update_arms_entry_transition_after_delay() {
        let mut lobby = LanLobbyMenuPort::sample();
        lobby.update();
        lobby.update();

        assert_eq!(
            lobby.active_transition_group.as_deref(),
            Some("LanLobbyMenuFade")
        );
    }
}
