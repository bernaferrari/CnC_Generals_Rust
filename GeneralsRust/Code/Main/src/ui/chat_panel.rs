//! In-Game Chat System
//!
//! Provides a text chat overlay for multiplayer and single-player games.
//! Press Enter to open the chat input, type a message, then press Enter to
//! send or Escape to cancel.  Chat messages are stored per-game and displayed
//! in a scrollable, fading message log area.
//!
//! Message types:
//! - Player chat (All / Allies / specific player)
//! - System notifications (player disconnected, low power, etc.)
//! - EVA notifications (voice-line text equivalents)

use super::{
    layout, utils, Interactive, KeyCode, MouseButton, Renderable, UIRenderContext,
};
use crate::localization;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Who the chat message is directed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTarget {
    All,
    Allies,
    Player(u8), // player index 0-7
}

/// Semantic category of a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMessageType {
    Player,
    System,
    Eva,
}

// ---------------------------------------------------------------------------
// Chat message
// ---------------------------------------------------------------------------

/// A single chat/log message displayed in the message area.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub text: String,
    /// Sender name (empty for system / EVA messages).
    pub sender: String,
    pub message_type: ChatMessageType,
    pub target: ChatTarget,
    /// Seconds since game start when the message was posted.
    pub spawn_time: f32,
    /// True while the message should be rendered at full opacity.
    pub visible: bool,
}

// ---------------------------------------------------------------------------
// Chat panel state
// ---------------------------------------------------------------------------

/// Overall state of the chat input overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatState {
    /// Chat input is hidden; game receives keyboard events normally.
    Closed,
    /// Chat input is open and routing keyboard events to the text field.
    Open,
}

/// Events emitted by the chat panel that higher-level systems should act on.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// The player sent a chat message.
    MessageSent {
        text: String,
        target: ChatTarget,
    },
    /// The player pressed Enter to open chat (consumed, no game command).
    ChatOpened,
    /// The player pressed Escape or Enter-with-empty-text to close.
    ChatClosed,
}

// ---------------------------------------------------------------------------
// Chat panel
// ---------------------------------------------------------------------------

/// Maximum number of messages retained in the log.
const MAX_MESSAGES: usize = 50;
/// Maximum length of a chat message (characters).
const MAX_MESSAGE_LENGTH: usize = 200;
/// Seconds before a message starts to fade.
const MESSAGE_FADE_DELAY: f32 = 12.0;
/// Seconds over which a message fades out after the delay.
const MESSAGE_FADE_DURATION: f32 = 8.0;
/// After this many seconds a message is removed entirely.
const MESSAGE_LIFETIME: f32 = MESSAGE_FADE_DELAY + MESSAGE_FADE_DURATION;

/// Main chat panel component.
pub struct ChatPanel {
    /// Current overlay state.
    state: ChatState,
    /// Text the player has typed so far.
    input_text: String,
    /// Cursor position within `input_text`.
    cursor_pos: usize,
    /// Selected chat target (cycles with Tab).
    target: ChatTarget,
    /// Stored messages.
    messages: Vec<ChatMessage>,
    /// Scroll offset (0 = newest visible at bottom).
    scroll_offset: usize,
    /// Screen dimensions for layout.
    screen_size: (u32, u32),
    /// Current game time (seconds).
    game_time: f32,
    /// Events to be drained by the owner each frame.
    pending_events: Vec<ChatEvent>,
    /// Name of the local player for display.
    local_player_name: String,
    /// Whether the local player is currently muted.
    muted: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatPanel {
    /// Create a new chat panel.
    pub fn new() -> Self {
        Self {
            state: ChatState::Closed,
            input_text: String::new(),
            cursor_pos: 0,
            target: ChatTarget::All,
            messages: Vec::new(),
            scroll_offset: 0,
            screen_size: (1024, 768),
            game_time: 0.0,
            pending_events: Vec::new(),
            local_player_name: "Player".to_string(),
            muted: false,
        }
    }

    // -- public query API ---------------------------------------------------

    pub fn is_open(&self) -> bool {
        self.state == ChatState::Open
    }

    pub fn state(&self) -> ChatState {
        self.state
    }

    pub fn target(&self) -> ChatTarget {
        self.target
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    // -- event drain --------------------------------------------------------

    /// Drain all pending `ChatEvent`s.  Call once per frame.
    pub fn drain_events(&mut self) -> Vec<ChatEvent> {
        std::mem::take(&mut self.pending_events)
    }

    // -- mutation API -------------------------------------------------------

    /// Set the local player display name.
    pub fn set_local_player_name(&mut self, name: &str) {
        self.local_player_name = name.to_string();
    }

    /// Resize the panel (call on window resize).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    /// Open the chat input.  Returns `true` if the state actually changed.
    pub fn open(&mut self) -> bool {
        if self.state == ChatState::Open {
            return false;
        }
        self.state = ChatState::Open;
        self.input_text.clear();
        self.cursor_pos = 0;
        self.scroll_offset = 0;
        self.pending_events.push(ChatEvent::ChatOpened);
        true
    }

    /// Close the chat input.  Returns `true` if the state actually changed.
    pub fn close(&mut self) -> bool {
        if self.state == ChatState::Closed {
            return false;
        }
        self.state = ChatState::Closed;
        self.input_text.clear();
        self.cursor_pos = 0;
        self.pending_events.push(ChatEvent::ChatClosed);
        true
    }

    /// Toggle chat open/closed.
    pub fn toggle(&mut self) -> bool {
        if self.is_open() {
            self.close()
        } else {
            self.open()
        }
    }

    /// Post a player chat message (from network or locally).
    pub fn add_player_message(&mut self, sender: &str, text: &str, target: ChatTarget) {
        self.push_message(ChatMessage {
            text: text.to_string(),
            sender: sender.to_string(),
            message_type: ChatMessageType::Player,
            target,
            spawn_time: self.game_time,
            visible: true,
        });
    }

    /// Post a system notification.
    pub fn add_system_message(&mut self, text: &str) {
        self.push_message(ChatMessage {
            text: text.to_string(),
            sender: String::new(),
            message_type: ChatMessageType::System,
            target: ChatTarget::All,
            spawn_time: self.game_time,
            visible: true,
        });
    }

    /// Post an EVA notification.
    pub fn add_eva_message(&mut self, text: &str) {
        self.push_message(ChatMessage {
            text: text.to_string(),
            sender: "EVA".to_string(),
            message_type: ChatMessageType::Eva,
            target: ChatTarget::All,
            spawn_time: self.game_time,
            visible: true,
        });
    }

    /// Set whether the local player is muted.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    // -- per-frame update ---------------------------------------------------

    /// Advance game time and expire old messages.
    pub fn update(&mut self, dt: f32) {
        self.game_time += dt;

        // Expire old messages
        self.messages
            .retain(|msg| self.game_time - msg.spawn_time < MESSAGE_LIFETIME);

        // Fade messages past the delay
        for msg in &mut self.messages {
            let age = self.game_time - msg.spawn_time;
            msg.visible = age < MESSAGE_FADE_DELAY + MESSAGE_FADE_DURATION;
        }

        // Clamp scroll offset
        if self.messages.is_empty() {
            self.scroll_offset = 0;
        } else if self.scroll_offset >= self.messages.len() {
            self.scroll_offset = self.messages.len() - 1;
        }
    }

    /// Calculate alpha for a message based on its age.
    pub fn message_alpha(&self, msg: &ChatMessage) -> f32 {
        let age = self.game_time - msg.spawn_time;
        if age < MESSAGE_FADE_DELAY {
            1.0
        } else {
            let fade_progress = (age - MESSAGE_FADE_DELAY) / MESSAGE_FADE_DURATION;
            (1.0 - fade_progress.clamp(0.0, 1.0)).max(0.15)
        }
    }

    // -- target label -------------------------------------------------------

    /// Human-readable label for the current chat target.
    pub fn target_label(&self) -> &'static str {
        match self.target {
            ChatTarget::All => "[All]",
            ChatTarget::Allies => "[Allies]",
            ChatTarget::Player(_idx) => "[Whisper]",
        }
    }

    /// Cycle to the next chat target (Tab while chat is open).
    pub fn cycle_target(&mut self) {
        self.target = match self.target {
            ChatTarget::All => ChatTarget::Allies,
            ChatTarget::Allies => ChatTarget::Player(0),
            ChatTarget::Player(idx) => {
                if idx < 7 {
                    ChatTarget::Player(idx + 1)
                } else {
                    ChatTarget::All
                }
            }
        };
    }

    // -- internal -----------------------------------------------------------

    fn push_message(&mut self, msg: ChatMessage) {
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.remove(0);
        }
        self.messages.push(msg);
    }

    /// Send the current input text as a message and close the input.
    fn send_message(&mut self) {
        let text = self.input_text.trim().to_string();
        if text.is_empty() {
            self.close();
            return;
        }
        if self.muted {
            self.add_system_message(&localization::localize(
                "chat.muted_warning",
                "You are muted and cannot send messages.",
            ));
            self.close();
            return;
        }
        let target = self.target;
        let sender = self.local_player_name.clone();
        self.pending_events.push(ChatEvent::MessageSent {
            text: text.clone(),
            target,
        });
        // Echo locally
        self.add_player_message(&sender, &text, target);
        self.close();
    }

    /// Insert a character at the cursor.
    fn insert_char(&mut self, ch: char) {
        if self.input_text.len() >= MAX_MESSAGE_LENGTH {
            return;
        }
        self.input_text.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Delete the character before the cursor (Backspace).
    fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            // Find the previous char boundary
            let prev = self.input_text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_text.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    /// Delete the character at the cursor (Delete).
    fn delete(&mut self) {
        if self.cursor_pos < self.input_text.len() {
            let next = self.input_text[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input_text.len());
            self.input_text.drain(self.cursor_pos..next);
        }
    }

    /// Move cursor left.
    fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.input_text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    fn cursor_right(&mut self) {
        if self.cursor_pos < self.input_text.len() {
            self.cursor_pos = self.input_text[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input_text.len());
        }
    }

    /// Move cursor to start of input.
    fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end of input.
    fn cursor_end(&mut self) {
        self.cursor_pos = self.input_text.len();
    }

    /// Layout: bounding rectangle for the message log area (above the input).
    pub fn message_log_rect(&self) -> (i32, i32, u32, u32) {
        let log_height: u32 = 200;
        let log_width: u32 = 500;
        let x = 10i32;
        let y = self.screen_size.1 as i32 - layout::HUD_PANEL_HEIGHT as i32 - log_height as i32 - 40;
        (x, y, log_width, log_height)
    }

    /// Layout: bounding rectangle for the text input field.
    pub fn input_field_rect(&self) -> (i32, i32, u32, u32) {
        let input_height: u32 = 28;
        let input_width: u32 = 500;
        let x = 10i32;
        let y = self.screen_size.1 as i32 - layout::HUD_PANEL_HEIGHT as i32 - 38;
        (x, y, input_width, input_height)
    }
}

// ---------------------------------------------------------------------------
// Interactive trait (keyboard / mouse)
// ---------------------------------------------------------------------------

impl Interactive for ChatPanel {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false // Chat panel does not track mouse hover.
    }

    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        if self.state != ChatState::Open {
            return false;
        }

        match key {
            KeyCode::Enter => {
                self.send_message();
                true
            }
            KeyCode::Escape => {
                self.close();
                true
            }
            KeyCode::Backspace => {
                self.backspace();
                true
            }
            KeyCode::Delete => {
                self.delete();
                true
            }
            KeyCode::Left => {
                self.cursor_left();
                true
            }
            KeyCode::Right => {
                self.cursor_right();
                true
            }
            KeyCode::Tab => {
                self.cycle_target();
                true
            }
            KeyCode::A => {
                // Ctrl+A selects all (handled outside). Plain A is text input.
                false // Let handle_text_input deal with it
            }
            KeyCode::C => {
                // Ctrl+C could copy; for now let text input handle it.
                false
            }
            KeyCode::V => {
                // Ctrl+V could paste; for now let text input handle it.
                false
            }
            KeyCode::Home => {
                self.cursor_home();
                true
            }
            KeyCode::End => {
                self.cursor_end();
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, text: &str) -> bool {
        if self.state != ChatState::Open {
            return false;
        }
        for ch in text.chars() {
            // Skip control characters
            if ch.is_control() {
                continue;
            }
            self.insert_char(ch);
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Renderable trait
// ---------------------------------------------------------------------------

impl Renderable for ChatPanel {
    fn render(&self, _context: &mut UIRenderContext) {
        if self.state == ChatState::Closed && self.messages.is_empty() {
            return;
        }

        let chat_header = localization::localize("chat.header", "=== Chat ===");
        println!("{chat_header}");

        // Render messages
        if !self.messages.is_empty() {
            let messages_label = localization::localize("chat.messages", "Messages:");
            println!("{messages_label}");
            for msg in self.messages.iter().rev().take(8) {
                let alpha = self.message_alpha(msg);
                let prefix = match msg.message_type {
                    ChatMessageType::Player => {
                        let target_tag = match msg.target {
                            ChatTarget::All => "[All]",
                            ChatTarget::Allies => "[Allies]",
                            ChatTarget::Player(_) => "[Whisper]",
                        };
                        if msg.sender.is_empty() {
                            format!("{} ", target_tag)
                        } else {
                            format!("{} {}: ", target_tag, msg.sender)
                        }
                    }
                    ChatMessageType::System => "[System] ".to_string(),
                    ChatMessageType::Eva => "[EVA] ".to_string(),
                };
                let alpha_str = format!("{:.0}%", alpha * 100.0);
                println!("  {}{} (opacity {})", prefix, msg.text, alpha_str);
            }
        }

        // Render input field
        if self.state == ChatState::Open {
            let target_label = self.target_label();
            let prompt = localization::localize_with_args(
                "chat.prompt",
                "{target} {name}: {text}|",
                &[
                    ("target", target_label),
                    ("name", &self.local_player_name),
                    ("text", &self.input_text),
                ],
            );
            println!("{prompt}");

            if self.muted {
                println!(
                    "  {}",
                    localization::localize("chat.muted_notice", "(You are muted)")
                );
            }

            let tab_hint = localization::localize(
                "chat.hint.tab_target",
                "Tab - Cycle target (All / Allies / Whisper)",
            );
            let esc_hint = localization::localize("chat.hint.esc_cancel", "Esc - Cancel");
            println!("  {tab_hint}");
            println!("  {esc_hint}");
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        // The chat panel occupies the lower-left area of the screen.
        let log_rect = self.message_log_rect();
        let input_rect = self.input_field_rect();
        let x = log_rect.0;
        let y = log_rect.1;
        let w = log_rect.2.max(input_rect.2);
        let h = (input_rect.1 + input_rect.3 as i32) - y;
        (x, y, w, h as u32)
    }

    fn is_visible(&self) -> bool {
        self.state == ChatState::Open || !self.messages.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_panel_creation() {
        let panel = ChatPanel::new();
        assert_eq!(panel.state(), ChatState::Closed);
        assert!(!panel.is_open());
        assert!(panel.messages().is_empty());
    }

    #[test]
    fn test_open_close_toggle() {
        let mut panel = ChatPanel::new();

        assert!(panel.open());
        assert!(panel.is_open());

        // Open again should be a no-op
        assert!(!panel.open());
        assert!(panel.is_open());

        assert!(panel.close());
        assert!(!panel.is_open());

        assert!(!panel.close());
    }

    #[test]
    fn test_toggle() {
        let mut panel = ChatPanel::new();
        assert!(panel.toggle());
        assert!(panel.is_open());
        assert!(panel.toggle());
        assert!(!panel.is_open());
    }

    #[test]
    fn test_text_input() {
        let mut panel = ChatPanel::new();
        panel.open();

        panel.handle_text_input("Hello");
        assert_eq!(panel.input_text, "Hello");
        assert_eq!(panel.cursor_pos, 5);

        panel.handle_text_input(" World");
        assert_eq!(panel.input_text, "Hello World");
        assert_eq!(panel.cursor_pos, 11);
    }

    #[test]
    fn test_backspace() {
        let mut panel = ChatPanel::new();
        panel.open();
        panel.handle_text_input("ABC");

        panel.backspace();
        assert_eq!(panel.input_text, "AB");
        assert_eq!(panel.cursor_pos, 2);

        panel.backspace();
        panel.backspace();
        assert_eq!(panel.input_text, "");
        assert_eq!(panel.cursor_pos, 0);

        // Backspace at start is a no-op
        panel.backspace();
        assert_eq!(panel.input_text, "");
    }

    #[test]
    fn test_cursor_movement() {
        let mut panel = ChatPanel::new();
        panel.open();
        panel.handle_text_input("ABCD");

        assert_eq!(panel.cursor_pos, 4);

        panel.cursor_left();
        assert_eq!(panel.cursor_pos, 3);

        panel.cursor_left();
        panel.cursor_left();
        assert_eq!(panel.cursor_pos, 1);

        panel.cursor_right();
        assert_eq!(panel.cursor_pos, 2);

        panel.cursor_home();
        assert_eq!(panel.cursor_pos, 0);

        panel.cursor_end();
        assert_eq!(panel.cursor_pos, 4);
    }

    #[test]
    fn test_send_message() {
        let mut panel = ChatPanel::new();
        panel.open();
        panel.handle_text_input("Hello world");

        let events = panel.drain_events();
        assert!(events.is_empty()); // No events yet

        panel.send_message();

        let events = panel.drain_events();
        assert_eq!(events.len(), 2); // ChatOpened + MessageSent
        assert!(matches!(events[0], ChatEvent::ChatOpened));
        assert!(matches!(&events[1], ChatEvent::MessageSent { text, .. } if text == "Hello world"));

        // Verify message appears in log
        assert_eq!(panel.messages().len(), 1);
        assert_eq!(panel.messages()[0].text, "Hello world");
        assert_eq!(panel.messages()[0].sender, "Player");

        // Panel should be closed after send
        assert!(!panel.is_open());
    }

    #[test]
    fn test_send_empty_message_closes() {
        let mut panel = ChatPanel::new();
        panel.open();

        panel.send_message();
        assert!(!panel.is_open());
    }

    #[test]
    fn test_message_types() {
        let mut panel = ChatPanel::new();

        panel.add_player_message("Alice", "Hi there", ChatTarget::All);
        panel.add_system_message("Player 2 has left the game");
        panel.add_eva_message("Our base is under attack");

        assert_eq!(panel.messages().len(), 3);
        assert_eq!(panel.messages()[0].message_type, ChatMessageType::Player);
        assert_eq!(panel.messages()[1].message_type, ChatMessageType::System);
        assert_eq!(panel.messages()[2].message_type, ChatMessageType::Eva);
    }

    #[test]
    fn test_target_cycling() {
        let mut panel = ChatPanel::new();

        assert_eq!(panel.target(), ChatTarget::All);
        panel.cycle_target();
        assert_eq!(panel.target(), ChatTarget::Allies);
        panel.cycle_target();
        assert_eq!(panel.target(), ChatTarget::Player(0));
        panel.cycle_target();
        assert_eq!(panel.target(), ChatTarget::Player(1));
    }

    #[test]
    fn test_message_expiry() {
        let mut panel = ChatPanel::new();
        panel.game_time = 0.0;
        panel.add_player_message("Test", "msg", ChatTarget::All);

        // Before fade delay
        panel.game_time = MESSAGE_FADE_DELAY - 1.0;
        assert_eq!(panel.message_alpha(&panel.messages()[0]), 1.0);

        // During fade
        panel.game_time = MESSAGE_FADE_DELAY + MESSAGE_FADE_DURATION / 2.0;
        let alpha = panel.message_alpha(&panel.messages()[0]);
        assert!(alpha > 0.3 && alpha < 0.8);

        // After lifetime
        panel.game_time = MESSAGE_LIFETIME + 1.0;
        panel.update(0.0);
        assert!(panel.messages().is_empty());
    }

    #[test]
    fn test_max_message_length() {
        let mut panel = ChatPanel::new();
        panel.open();

        let long_text = "A".repeat(MAX_MESSAGE_LENGTH + 50);
        panel.handle_text_input(&long_text);
        assert_eq!(panel.input_text.len(), MAX_MESSAGE_LENGTH);
    }

    #[test]
    fn test_muted_player() {
        let mut panel = ChatPanel::new();
        panel.set_muted(true);
        panel.open();
        panel.handle_text_input("test");
        panel.send_message();

        // Should have a system warning message
        let system_msgs: Vec<_> = panel
            .messages()
            .iter()
            .filter(|m| m.message_type == ChatMessageType::System)
            .collect();
        assert_eq!(system_msgs.len(), 1);
    }

    #[test]
    fn test_message_cap() {
        let mut panel = ChatPanel::new();
        for i in 0..(MAX_MESSAGES + 10) {
            panel.add_player_message("Bot", &format!("msg {}", i), ChatTarget::All);
        }
        assert_eq!(panel.messages().len(), MAX_MESSAGES);
    }

    #[test]
    fn test_key_press_consumed_when_open() {
        let mut panel = ChatPanel::new();
        panel.open();

        // These keys should be consumed when chat is open
        assert!(panel.handle_key_press(KeyCode::Enter));
        assert!(panel.handle_key_press(KeyCode::Escape));
        assert!(panel.handle_key_press(KeyCode::Backspace));
        assert!(panel.handle_key_press(KeyCode::Left));
        assert!(panel.handle_key_press(KeyCode::Right));
        assert!(panel.handle_key_press(KeyCode::Tab));
        assert!(panel.handle_key_press(KeyCode::Home));
        assert!(panel.handle_key_press(KeyCode::End));
    }

    #[test]
    fn test_key_press_ignored_when_closed() {
        let mut panel = ChatPanel::new();
        assert!(!panel.handle_key_press(KeyCode::Enter));
        assert!(!panel.handle_key_press(KeyCode::Escape));
    }

    #[test]
    fn test_text_input_ignored_when_closed() {
        let mut panel = ChatPanel::new();
        assert!(!panel.handle_text_input("hello"));
        assert!(panel.input_text.is_empty());
    }
}
