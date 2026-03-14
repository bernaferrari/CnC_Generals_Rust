use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/InGameChat.cpp",
    "crate::gui::callbacks::in_game_chat",
    "In-Game Chat",
    "Ports chat overlay, entry, and history behavior for active gameplay.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "In-Game Chat",
    "Chat overlay and message-entry callbacks.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InGameChatTypePort {
    Everyone,
    Allies,
    Players,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatParticipantPort {
    pub slot: usize,
    pub active: bool,
    pub muted: bool,
    pub allied_with_local: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatDispatchPort {
    pub filtered_message: String,
    pub player_mask: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InGameChatPort {
    pub visible: bool,
    pub enabled: bool,
    pub saved_chat: String,
    pub current_text: String,
    pub chat_type: InGameChatTypePort,
    pub chat_type_label: String,
    pub sent_messages: Vec<ChatDispatchPort>,
}

impl Default for InGameChatPort {
    fn default() -> Self {
        let mut chat = Self {
            visible: false,
            enabled: true,
            saved_chat: String::new(),
            current_text: String::new(),
            chat_type: InGameChatTypePort::Everyone,
            chat_type_label: String::new(),
            sent_messages: Vec::new(),
        };
        chat.set_chat_type(InGameChatTypePort::Everyone, true);
        chat
    }
}

impl InGameChatPort {
    pub fn show(
        &mut self,
        replay_game: bool,
        quit_menu_visible: bool,
        disconnect_visible: bool,
    ) -> bool {
        if replay_game || quit_menu_visible || disconnect_visible {
            return false;
        }
        self.visible = true;
        self.enabled = true;
        self.current_text = std::mem::take(&mut self.saved_chat);
        self.set_chat_type(InGameChatTypePort::Everyone, true);
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn hide(&mut self) {
        self.saved_chat = self.current_text.clone();
        self.visible = false;
        self.enabled = false;
    }

    pub fn set_chat_type(&mut self, chat_type: InGameChatTypePort, local_player_active: bool) {
        self.chat_type = chat_type;
        self.chat_type_label = match (chat_type, local_player_active) {
            (InGameChatTypePort::Everyone, true) => "Everyone".to_string(),
            (InGameChatTypePort::Everyone, false) => "Observers".to_string(),
            (InGameChatTypePort::Allies, _) => "Allies".to_string(),
            (InGameChatTypePort::Players, _) => "Players".to_string(),
        };
    }

    pub fn handle_slash_command(
        &self,
        text: &str,
        hosting_status: i32,
        thread_hosting: i32,
    ) -> Option<String> {
        let trimmed = text.trim();
        if !trimmed.starts_with('/') {
            return None;
        }
        let command = trimmed[1..]
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match command.as_str() {
            "host" => Some(format!(
                "Hosting qr2:{hosting_status} thread:{thread_hosting}"
            )),
            _ => None,
        }
    }

    pub fn submit_message(
        &mut self,
        participants: &[ChatParticipantPort],
        local_slot: usize,
        local_player_active: bool,
        hosting_status: i32,
        thread_hosting: i32,
    ) -> Option<ChatDispatchPort> {
        let message = self.current_text.trim().to_string();
        if message.is_empty() {
            self.current_text.clear();
            self.hide();
            return None;
        }

        if self
            .handle_slash_command(&message, hosting_status, thread_hosting)
            .is_some()
        {
            self.current_text.clear();
            self.hide();
            return None;
        }

        self.set_chat_type(self.chat_type, local_player_active);
        let filtered_message = filter_language(&message);
        let player_mask = participants.iter().fold(0_u32, |mask, participant| {
            let include = match self.chat_type {
                InGameChatTypePort::Everyone => !participant.muted,
                InGameChatTypePort::Allies => {
                    participant.slot == local_slot || participant.allied_with_local
                }
                InGameChatTypePort::Players => participant.slot == local_slot,
            };
            if include {
                mask | (1_u32 << participant.slot)
            } else {
                mask
            }
        });

        let dispatch = ChatDispatchPort {
            filtered_message,
            player_mask,
        };
        self.sent_messages.push(dispatch.clone());
        self.current_text.clear();
        self.hide();
        Some(dispatch)
    }

    pub fn clear_entry(&mut self) {
        self.current_text.clear();
        self.saved_chat.clear();
    }
}

fn filter_language(message: &str) -> String {
    message.replace("damn", "d***")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_is_blocked_when_disconnect_menu_is_visible() {
        let mut chat = InGameChatPort::default();

        assert!(!chat.show(false, false, true));
        assert!(!chat.visible);
    }

    #[test]
    fn allies_chat_targets_only_local_and_allies() {
        let mut chat = InGameChatPort::default();
        chat.show(false, false, false);
        chat.set_chat_type(InGameChatTypePort::Allies, true);
        chat.current_text = "hold the left flank".to_string();

        let dispatch = chat
            .submit_message(
                &[
                    ChatParticipantPort {
                        slot: 0,
                        active: true,
                        muted: false,
                        allied_with_local: true,
                    },
                    ChatParticipantPort {
                        slot: 1,
                        active: true,
                        muted: false,
                        allied_with_local: true,
                    },
                    ChatParticipantPort {
                        slot: 2,
                        active: true,
                        muted: false,
                        allied_with_local: false,
                    },
                ],
                0,
                true,
                0,
                0,
            )
            .unwrap();

        assert_eq!(dispatch.player_mask, 0b011);
    }

    #[test]
    fn slash_host_command_is_not_sent_to_network() {
        let mut chat = InGameChatPort::default();
        chat.show(false, false, false);
        chat.current_text = "/host".to_string();

        let dispatch = chat.submit_message(&[], 0, true, 3, 1);

        assert!(dispatch.is_none());
        assert!(chat.sent_messages.is_empty());
        assert!(!chat.visible);
    }
}
