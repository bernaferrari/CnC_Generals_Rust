// FILE: in_game_chat.rs
// GUI callbacks for the in-game chat entry
// Ported from C++ to Rust

use std::collections::VecDeque;

/// In-game chat type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InGameChatType {
    Everyone,
    Allies,
    Players,
}

/// Chat message data
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub sender_name: String,
    pub sender_player_index: i32,
    pub message: String,
    pub chat_type: InGameChatType,
    pub timestamp: u32,
    pub is_emote: bool,
}

impl ChatMessage {
    pub fn new(
        sender_name: String,
        sender_player_index: i32,
        message: String,
        chat_type: InGameChatType,
        timestamp: u32,
        is_emote: bool,
    ) -> Self {
        Self {
            sender_name,
            sender_player_index,
            message,
            chat_type,
            timestamp,
            is_emote,
        }
    }

    pub fn format_display(&self) -> String {
        if self.is_emote {
            format!("{} {}", self.sender_name, self.message)
        } else {
            format!("[{}] {}", self.sender_name, self.message)
        }
    }
}

/// In-game chat system
pub struct InGameChat {
    is_visible: bool,
    is_enabled: bool,
    current_chat_type: InGameChatType,
    current_input: String,
    saved_input: String,
    chat_history: VecDeque<ChatMessage>,
    max_history_size: usize,
}

impl InGameChat {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            is_visible: false,
            is_enabled: true,
            current_chat_type: InGameChatType::Everyone,
            current_input: String::new(),
            saved_input: String::new(),
            chat_history: VecDeque::new(),
            max_history_size,
        }
    }

    pub fn show(&mut self) {
        self.is_visible = true;
        self.is_enabled = true;
        // Restore saved input
        self.current_input = self.saved_input.clone();
        self.saved_input.clear();
    }

    pub fn hide(&mut self) {
        // Save current input
        self.saved_input = self.current_input.clone();
        self.is_visible = false;
        self.is_enabled = false;
    }

    pub fn reset(&mut self) {
        self.is_visible = false;
        self.is_enabled = true;
        self.current_input.clear();
        self.saved_input.clear();
        self.chat_history.clear();
    }

    pub fn toggle(&mut self) {
        if self.is_visible {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_visible && !self.is_hidden()
    }

    pub fn is_hidden(&self) -> bool {
        !self.is_visible
    }

    pub fn set_chat_type(&mut self, chat_type: InGameChatType) {
        self.current_chat_type = chat_type;
    }

    pub fn get_chat_type(&self) -> InGameChatType {
        self.current_chat_type
    }

    pub fn get_chat_type_text(&self, is_player_active: bool) -> &str {
        match self.current_chat_type {
            InGameChatType::Everyone => {
                if is_player_active {
                    "Chat:Everyone"
                } else {
                    "Chat:Observers"
                }
            }
            InGameChatType::Allies => "Chat:Allies",
            InGameChatType::Players => "Chat:Players",
        }
    }

    pub fn set_input_text(&mut self, text: String) {
        self.current_input = text;
    }

    pub fn get_input_text(&self) -> &str {
        &self.current_input
    }

    pub fn clear_input(&mut self) {
        self.current_input.clear();
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.chat_history.push_back(message);

        // Maintain max history size
        while self.chat_history.len() > self.max_history_size {
            self.chat_history.pop_front();
        }
    }

    pub fn get_history(&self) -> &VecDeque<ChatMessage> {
        &self.chat_history
    }

    pub fn clear_history(&mut self) {
        self.chat_history.clear();
    }
}

impl Default for InGameChat {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Slash command handler
pub struct SlashCommandHandler {
    commands: Vec<SlashCommand>,
}

#[derive(Clone)]
struct SlashCommand {
    name: String,
    handler: fn(&str) -> SlashCommandResult,
}

pub enum SlashCommandResult {
    Handled(String),
    NotHandled,
}

impl SlashCommandHandler {
    pub fn new() -> Self {
        let mut handler = Self {
            commands: Vec::new(),
        };

        // Register built-in commands
        handler.register_command("host", handle_host_command);
        handler.register_command("ally", handle_ally_command);
        handler.register_command("team", handle_team_command);

        handler
    }

    pub fn register_command(&mut self, name: &str, handler: fn(&str) -> SlashCommandResult) {
        self.commands.push(SlashCommand {
            name: name.to_lowercase(),
            handler,
        });
    }

    pub fn handle(&self, message: &str) -> SlashCommandResult {
        if !message.starts_with('/') {
            return SlashCommandResult::NotHandled;
        }

        let trimmed = message.trim_start_matches('/');
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let command_name = parts[0].to_lowercase();
        let remainder = if parts.len() > 1 { parts[1] } else { "" };

        for cmd in &self.commands {
            if cmd.name == command_name {
                return (cmd.handler)(remainder);
            }
        }

        SlashCommandResult::NotHandled
    }
}

impl Default for SlashCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in slash command handlers

fn handle_host_command(_args: &str) -> SlashCommandResult {
    // In the C++ version, this displays hosting status
    SlashCommandResult::Handled("Hosting status displayed".to_string())
}

fn handle_ally_command(_args: &str) -> SlashCommandResult {
    // Change chat mode to allies
    SlashCommandResult::Handled("Switched to ally chat".to_string())
}

fn handle_team_command(_args: &str) -> SlashCommandResult {
    // Change chat mode to team
    SlashCommandResult::Handled("Switched to team chat".to_string())
}

/// Chat message processor
pub struct ChatMessageProcessor {
    pub language_filter_enabled: bool,
    previous_message: String,
}

impl ChatMessageProcessor {
    pub fn new() -> Self {
        Self {
            language_filter_enabled: true,
            previous_message: String::new(),
        }
    }

    /// Process outgoing chat message
    /// Returns true if message should be sent, false if it should be filtered
    pub fn process_outgoing(&mut self, message: &str) -> bool {
        let trimmed = message.trim();

        // Empty messages are not sent
        if trimmed.is_empty() {
            return false;
        }

        // Filter duplicate messages (anti-spam)
        if trimmed == self.previous_message {
            return false;
        }

        self.previous_message = trimmed.to_string();
        true
    }

    /// Apply language filter to a message
    pub fn filter_language(&self, message: &mut String) {
        if !self.language_filter_enabled {
            return;
        }

        // Simple filter implementation - replace offensive words with asterisks
        // In a real implementation, this would use a more sophisticated filter
        let offensive_words = ["badword1", "badword2", "badword3"];

        for word in &offensive_words {
            let replacement = "*".repeat(word.len());
            *message = message.replace(word, &replacement);
        }
    }

    pub fn reset(&mut self) {
        self.previous_message.clear();
    }
}

impl Default for ChatMessageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Determines which players should receive a chat message
pub fn calculate_player_mask(
    chat_type: InGameChatType,
    local_player_index: i32,
    player_count: usize,
    is_allied: fn(i32, i32) -> bool,
    is_muted: fn(i32) -> bool,
) -> u32 {
    let mut mask = 0u32;

    for i in 0..player_count {
        let player_index = i as i32;

        match chat_type {
            InGameChatType::Everyone => {
                // Send to all non-muted players
                if !is_muted(player_index) {
                    mask |= 1 << i;
                }
            }
            InGameChatType::Allies => {
                // Send to allies and self
                if is_allied(local_player_index, player_index) || player_index == local_player_index {
                    mask |= 1 << i;
                }
            }
            InGameChatType::Players => {
                // Send only to self (for testing or local messages)
                if player_index == local_player_index {
                    mask |= 1 << i;
                }
            }
        }
    }

    mask
}

/// Chat color manager
pub struct ChatColorManager {
    colors: ChatColors,
}

#[derive(Clone, Copy)]
pub struct ChatColors {
    pub normal: u32,
    pub emote: u32,
    pub owner: u32,
    pub owner_emote: u32,
    pub private: u32,
    pub private_emote: u32,
    pub private_owner: u32,
    pub private_owner_emote: u32,
    pub buddy: u32,
    pub self_message: u32,
}

impl Default for ChatColors {
    fn default() -> Self {
        Self {
            normal: 0xFFFFFFFF,          // White
            emote: 0xFFFF8000,           // Orange
            owner: 0xFFFFFF00,           // Yellow
            owner_emote: 0xFF80FF00,     // Yellow-green
            private: 0xFF0000FF,         // Blue
            private_emote: 0xFF00FFFF,   // Cyan
            private_owner: 0xFFFF00FF,   // Magenta
            private_owner_emote: 0xFFFF80FF, // Light magenta
            buddy: 0xFFFF00FF,           // Magenta
            self_message: 0xFFFF0080,    // Pink
        }
    }
}

impl ChatColorManager {
    pub fn new() -> Self {
        Self {
            colors: ChatColors::default(),
        }
    }

    pub fn get_color(
        &self,
        is_buddy: bool,
        is_public: bool,
        is_action: bool,
        is_owner: bool,
    ) -> u32 {
        if is_buddy {
            return self.colors.buddy;
        }

        match (is_public, is_action, is_owner) {
            (true, true, true) => self.colors.owner_emote,
            (true, true, false) => self.colors.emote,
            (true, false, true) => self.colors.owner,
            (true, false, false) => self.colors.normal,
            (false, true, true) => self.colors.private_owner_emote,
            (false, true, false) => self.colors.private_emote,
            (false, false, true) => self.colors.private_owner,
            (false, false, false) => self.colors.private,
        }
    }

    pub fn set_color(&mut self, color_type: &str, color: u32) {
        match color_type {
            "normal" => self.colors.normal = color,
            "emote" => self.colors.emote = color,
            "owner" => self.colors.owner = color,
            "owner_emote" => self.colors.owner_emote = color,
            "private" => self.colors.private = color,
            "private_emote" => self.colors.private_emote = color,
            "private_owner" => self.colors.private_owner = color,
            "private_owner_emote" => self.colors.private_owner_emote = color,
            "buddy" => self.colors.buddy = color,
            "self" => self.colors.self_message = color,
            _ => {}
        }
    }
}

impl Default for ChatColorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_toggle() {
        let mut chat = InGameChat::new(100);
        assert!(!chat.is_active());

        chat.show();
        assert!(chat.is_active());

        chat.hide();
        assert!(!chat.is_active());
    }

    #[test]
    fn test_chat_input() {
        let mut chat = InGameChat::new(100);
        chat.set_input_text("Hello, world!".to_string());
        assert_eq!(chat.get_input_text(), "Hello, world!");

        chat.clear_input();
        assert_eq!(chat.get_input_text(), "");
    }

    #[test]
    fn test_slash_command() {
        let handler = SlashCommandHandler::new();

        match handler.handle("/host") {
            SlashCommandResult::Handled(_) => {}
            SlashCommandResult::NotHandled => panic!("Command should be handled"),
        }

        match handler.handle("not a command") {
            SlashCommandResult::NotHandled => {}
            SlashCommandResult::Handled(_) => panic!("Should not be handled"),
        }
    }

    #[test]
    fn test_message_formatting() {
        let msg = ChatMessage::new(
            "Player1".to_string(),
            0,
            "Hello!".to_string(),
            InGameChatType::Everyone,
            1000,
            false,
        );

        assert_eq!(msg.format_display(), "[Player1] Hello!");

        let emote_msg = ChatMessage::new(
            "Player1".to_string(),
            0,
            "waves".to_string(),
            InGameChatType::Everyone,
            1000,
            true,
        );

        assert_eq!(emote_msg.format_display(), "Player1 waves");
    }

    #[test]
    fn test_chat_history() {
        let mut chat = InGameChat::new(3);

        chat.add_message(ChatMessage::new(
            "P1".to_string(), 0, "M1".to_string(), InGameChatType::Everyone, 1, false
        ));
        chat.add_message(ChatMessage::new(
            "P2".to_string(), 1, "M2".to_string(), InGameChatType::Everyone, 2, false
        ));
        chat.add_message(ChatMessage::new(
            "P3".to_string(), 2, "M3".to_string(), InGameChatType::Everyone, 3, false
        ));

        assert_eq!(chat.get_history().len(), 3);

        // Adding a 4th message should remove the oldest
        chat.add_message(ChatMessage::new(
            "P4".to_string(), 3, "M4".to_string(), InGameChatType::Everyone, 4, false
        ));

        assert_eq!(chat.get_history().len(), 3);
        assert_eq!(chat.get_history().front().unwrap().sender_name, "P2");
    }

    #[test]
    fn test_player_mask() {
        let mask = calculate_player_mask(
            InGameChatType::Everyone,
            0,
            4,
            |_, _| false,
            |_| false,
        );

        // All 4 players should receive (bits 0-3 set)
        assert_eq!(mask, 0b1111);
    }
}
