// FILE: online_chat.rs
// Online chat system for multiplayer lobbies (GameSpy-style)
// Ported from C++ to Rust

use std::collections::{HashMap, HashSet};

/// GameSpy color definitions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum GameSpyColor {
    Default = 0,
    CurrentRoom,
    Room,
    Game,
    GameFull,
    GameCrcMismatch,
    PlayerNormal,
    PlayerOwner,
    PlayerBuddy,
    PlayerSelf,
    PlayerIgnored,
    ChatNormal,
    ChatEmote,
    ChatOwner,
    ChatOwnerEmote,
    ChatPrivate,
    ChatPrivateEmote,
    ChatPrivateOwner,
    ChatPrivateOwnerEmote,
    ChatBuddy,
    ChatSelf,
    AcceptTrue,
    AcceptFalse,
    MapSelected,
    MapUnselected,
    Motd,
    MotdHeading,
    Max,
}

/// Color palette for GameSpy interface
pub struct GameSpyColorPalette {
    colors: [u32; GameSpyColor::Max as usize],
}

impl Default for GameSpyColorPalette {
    fn default() -> Self {
        let mut colors = [0u32; GameSpyColor::Max as usize];

        colors[GameSpyColor::Default as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::CurrentRoom as usize] = make_color(255, 255, 0, 255);
        colors[GameSpyColor::Room as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::Game as usize] = make_color(128, 128, 0, 255);
        colors[GameSpyColor::GameFull as usize] = make_color(128, 128, 128, 255);
        colors[GameSpyColor::GameCrcMismatch as usize] = make_color(128, 128, 128, 255);
        colors[GameSpyColor::PlayerNormal as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::PlayerOwner as usize] = make_color(255, 0, 255, 255);
        colors[GameSpyColor::PlayerBuddy as usize] = make_color(255, 0, 128, 255);
        colors[GameSpyColor::PlayerSelf as usize] = make_color(255, 0, 0, 255);
        colors[GameSpyColor::PlayerIgnored as usize] = make_color(128, 128, 128, 255);
        colors[GameSpyColor::ChatNormal as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::ChatEmote as usize] = make_color(255, 128, 0, 255);
        colors[GameSpyColor::ChatOwner as usize] = make_color(255, 255, 0, 255);
        colors[GameSpyColor::ChatOwnerEmote as usize] = make_color(128, 255, 0, 255);
        colors[GameSpyColor::ChatPrivate as usize] = make_color(0, 0, 255, 255);
        colors[GameSpyColor::ChatPrivateEmote as usize] = make_color(0, 255, 255, 255);
        colors[GameSpyColor::ChatPrivateOwner as usize] = make_color(255, 0, 255, 255);
        colors[GameSpyColor::ChatPrivateOwnerEmote as usize] = make_color(255, 128, 255, 255);
        colors[GameSpyColor::ChatBuddy as usize] = make_color(255, 0, 255, 255);
        colors[GameSpyColor::ChatSelf as usize] = make_color(255, 0, 128, 255);
        colors[GameSpyColor::AcceptTrue as usize] = make_color(0, 255, 0, 255);
        colors[GameSpyColor::AcceptFalse as usize] = make_color(255, 0, 0, 255);
        colors[GameSpyColor::MapSelected as usize] = make_color(255, 255, 0, 255);
        colors[GameSpyColor::MapUnselected as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::Motd as usize] = make_color(255, 255, 255, 255);
        colors[GameSpyColor::MotdHeading as usize] = make_color(255, 255, 0, 255);

        Self { colors }
    }
}

impl GameSpyColorPalette {
    pub fn get_color(&self, color_type: GameSpyColor) -> u32 {
        self.colors[color_type as usize]
    }

    pub fn set_color(&mut self, color_type: GameSpyColor, color: u32) {
        self.colors[color_type as usize] = color;
    }
}

fn make_color(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Player information
#[derive(Clone, Debug)]
pub struct PlayerInfo {
    pub name: String,
    pub profile_id: i32,
    pub flags: u32,
}

impl PlayerInfo {
    pub fn new(name: String, profile_id: i32, flags: u32) -> Self {
        Self {
            name,
            profile_id,
            flags,
        }
    }
}

/// Peer flags
pub const PEER_FLAG_OP: u32 = 0x01; // Operator/Owner flag

/// Room types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomType {
    StagingRoom,
    GroupRoom,
}

/// Message types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageType {
    Normal,
    Emote,
    Private,
}

/// Online chat manager
pub struct OnlineChatManager {
    local_player_name: String,
    local_profile_id: i32,
    player_info_map: HashMap<String, PlayerInfo>,
    buddy_map: HashMap<i32, String>,
    ignored_users: HashSet<String>,
    ignored_profile_ids: HashSet<i32>,
    color_palette: GameSpyColorPalette,
    disallow_asian_text: bool,
    disallow_non_asian_text: bool,
    previous_message: String,
}

impl OnlineChatManager {
    pub fn new() -> Self {
        Self {
            local_player_name: String::new(),
            local_profile_id: 0,
            player_info_map: HashMap::new(),
            buddy_map: HashMap::new(),
            ignored_users: HashSet::new(),
            ignored_profile_ids: HashSet::new(),
            color_palette: GameSpyColorPalette::default(),
            disallow_asian_text: false,
            disallow_non_asian_text: false,
            previous_message: String::new(),
        }
    }

    pub fn set_local_player(&mut self, name: String, profile_id: i32) {
        self.local_player_name = name;
        self.local_profile_id = profile_id;
    }

    pub fn get_local_name(&self) -> &str {
        &self.local_player_name
    }

    pub fn add_player(&mut self, name: String, profile_id: i32, flags: u32) {
        let player_info = PlayerInfo::new(name.clone(), profile_id, flags);
        self.player_info_map.insert(name, player_info);
    }

    pub fn remove_player(&mut self, name: &str) {
        self.player_info_map.remove(name);
    }

    pub fn get_player_info(&self, name: &str) -> Option<&PlayerInfo> {
        self.player_info_map.get(name)
    }

    pub fn add_buddy(&mut self, profile_id: i32, name: String) {
        self.buddy_map.insert(profile_id, name);
    }

    pub fn remove_buddy(&mut self, profile_id: i32) {
        self.buddy_map.remove(&profile_id);
    }

    pub fn is_buddy(&self, profile_id: i32) -> bool {
        self.buddy_map.contains_key(&profile_id)
    }

    pub fn add_ignored(&mut self, name: String, profile_id: i32) {
        self.ignored_users.insert(name);
        self.ignored_profile_ids.insert(profile_id);
    }

    pub fn remove_ignored(&mut self, name: &str, profile_id: i32) {
        self.ignored_users.remove(name);
        self.ignored_profile_ids.remove(&profile_id);
    }

    pub fn is_ignored(&self, name: &str) -> bool {
        self.ignored_users.contains(name)
    }

    pub fn is_ignored_by_id(&self, profile_id: i32) -> bool {
        self.ignored_profile_ids.contains(&profile_id)
    }

    /// Send a chat message
    pub fn send_chat(
        &mut self,
        message: String,
        is_action: bool,
        recipients: Option<Vec<String>>,
    ) -> bool {
        let trimmed = message.trim();

        if trimmed.is_empty() {
            return false;
        }

        // Anti-spam: don't send duplicate messages
        if !is_action && trimmed == self.previous_message {
            return false;
        }

        self.previous_message = trimmed.to_string();

        // In a real implementation, this would send the message over the network
        // For now, we just return success
        true
    }

    /// Add a chat message to the display
    pub fn add_chat(
        &self,
        player_info: &PlayerInfo,
        message: String,
        is_public: bool,
        is_action: bool,
    ) -> Option<ChatDisplayInfo> {
        // Check if player is ignored
        if self.is_ignored(&player_info.name) || self.is_ignored_by_id(player_info.profile_id) {
            return None;
        }

        let is_me = player_info.name == self.local_player_name;

        // Filter text based on settings
        if !is_me {
            if self.disallow_asian_text && contains_asian_characters(&message) {
                return None;
            }

            if self.disallow_non_asian_text && !contains_asian_characters(&message) {
                return None;
            }
        }

        let is_owner = (player_info.flags & PEER_FLAG_OP) != 0;
        let is_buddy = self.is_buddy(player_info.profile_id);

        // Determine color style
        let color = self.get_chat_color(is_buddy, is_public, is_action, is_owner);

        // Format the message
        let full_message = if is_action {
            format!("{} {}", player_info.name, message)
        } else {
            format!("[{}] {}", player_info.name, message)
        };

        Some(ChatDisplayInfo {
            message: full_message,
            color,
            profile_id: player_info.profile_id,
        })
    }

    fn get_chat_color(&self, is_buddy: bool, is_public: bool, is_action: bool, is_owner: bool) -> u32 {
        if is_buddy {
            return self.color_palette.get_color(GameSpyColor::ChatBuddy);
        }

        let color_type = match (is_public, is_action, is_owner) {
            (true, true, true) => GameSpyColor::ChatOwnerEmote,
            (true, true, false) => GameSpyColor::ChatEmote,
            (true, false, true) => GameSpyColor::ChatOwner,
            (true, false, false) => GameSpyColor::ChatNormal,
            (false, true, true) => GameSpyColor::ChatPrivateOwnerEmote,
            (false, true, false) => GameSpyColor::ChatPrivateEmote,
            (false, false, true) => GameSpyColor::ChatPrivateOwner,
            (false, false, false) => GameSpyColor::ChatPrivate,
        };

        self.color_palette.get_color(color_type)
    }

    pub fn set_text_filter(&mut self, disallow_asian: bool, disallow_non_asian: bool) {
        self.disallow_asian_text = disallow_asian;
        self.disallow_non_asian_text = disallow_non_asian;
    }

    pub fn get_color_palette(&self) -> &GameSpyColorPalette {
        &self.color_palette
    }

    pub fn get_color_palette_mut(&mut self) -> &mut GameSpyColorPalette {
        &mut self.color_palette
    }
}

impl Default for OnlineChatManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Information needed to display a chat message
pub struct ChatDisplayInfo {
    pub message: String,
    pub color: u32,
    pub profile_id: i32,
}

/// Check if text contains Asian (Unicode) characters
fn contains_asian_characters(text: &str) -> bool {
    text.chars().any(|c| (c as u32) >= 256)
}

/// Chat channel information
#[derive(Clone, Debug)]
pub struct ChatChannel {
    pub name: String,
    pub topic: String,
    pub user_count: usize,
}

impl ChatChannel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            topic: String::new(),
            user_count: 0,
        }
    }
}

/// Game lobby information
#[derive(Clone, Debug)]
pub struct GameLobbyInfo {
    pub name: String,
    pub host: String,
    pub player_count: usize,
    pub max_players: usize,
    pub is_full: bool,
    pub has_password: bool,
}

impl GameLobbyInfo {
    pub fn new(name: String, host: String, max_players: usize) -> Self {
        Self {
            name,
            host,
            player_count: 0,
            max_players,
            is_full: false,
            has_password: false,
        }
    }

    pub fn is_joinable(&self) -> bool {
        !self.is_full && self.player_count < self.max_players
    }
}

/// Lobby manager
pub struct LobbyManager {
    current_channel: Option<ChatChannel>,
    available_channels: Vec<ChatChannel>,
    available_games: Vec<GameLobbyInfo>,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self {
            current_channel: None,
            available_channels: Vec::new(),
            available_games: Vec::new(),
        }
    }

    pub fn join_channel(&mut self, channel: ChatChannel) {
        self.current_channel = Some(channel);
    }

    pub fn leave_channel(&mut self) {
        self.current_channel = None;
    }

    pub fn get_current_channel(&self) -> Option<&ChatChannel> {
        self.current_channel.as_ref()
    }

    pub fn add_channel(&mut self, channel: ChatChannel) {
        self.available_channels.push(channel);
    }

    pub fn remove_channel(&mut self, name: &str) {
        self.available_channels.retain(|c| c.name != name);
    }

    pub fn get_channels(&self) -> &[ChatChannel] {
        &self.available_channels
    }

    pub fn add_game(&mut self, game: GameLobbyInfo) {
        self.available_games.push(game);
    }

    pub fn remove_game(&mut self, name: &str) {
        self.available_games.retain(|g| g.name != name);
    }

    pub fn get_games(&self) -> &[GameLobbyInfo] {
        &self.available_games
    }

    pub fn update_game(&mut self, name: &str, player_count: usize) {
        if let Some(game) = self.available_games.iter_mut().find(|g| g.name == name) {
            game.player_count = player_count;
            game.is_full = player_count >= game.max_players;
        }
    }
}

impl Default for LobbyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_management() {
        let mut chat = OnlineChatManager::new();
        chat.set_local_player("Player1".to_string(), 1);

        chat.add_player("Player2".to_string(), 2, 0);
        assert!(chat.get_player_info("Player2").is_some());

        chat.remove_player("Player2");
        assert!(chat.get_player_info("Player2").is_none());
    }

    #[test]
    fn test_buddy_system() {
        let mut chat = OnlineChatManager::new();

        chat.add_buddy(123, "Buddy1".to_string());
        assert!(chat.is_buddy(123));

        chat.remove_buddy(123);
        assert!(!chat.is_buddy(123));
    }

    #[test]
    fn test_ignore_system() {
        let mut chat = OnlineChatManager::new();

        chat.add_ignored("BadPlayer".to_string(), 999);
        assert!(chat.is_ignored("BadPlayer"));
        assert!(chat.is_ignored_by_id(999));

        chat.remove_ignored("BadPlayer", 999);
        assert!(!chat.is_ignored("BadPlayer"));
        assert!(!chat.is_ignored_by_id(999));
    }

    #[test]
    fn test_send_chat_anti_spam() {
        let mut chat = OnlineChatManager::new();

        assert!(chat.send_chat("Hello".to_string(), false, None));
        assert!(!chat.send_chat("Hello".to_string(), false, None)); // Duplicate
        assert!(chat.send_chat("World".to_string(), false, None)); // Different message
    }

    #[test]
    fn test_chat_formatting() {
        let chat = OnlineChatManager::new();
        let player = PlayerInfo::new("TestPlayer".to_string(), 1, 0);

        let info = chat.add_chat(&player, "Hello!".to_string(), true, false);
        assert!(info.is_some());

        let display = info.unwrap();
        assert_eq!(display.message, "[TestPlayer] Hello!");

        let emote_info = chat.add_chat(&player, "waves".to_string(), true, true);
        let emote_display = emote_info.unwrap();
        assert_eq!(emote_display.message, "TestPlayer waves");
    }

    #[test]
    fn test_asian_character_detection() {
        assert!(!contains_asian_characters("Hello"));
        assert!(contains_asian_characters("你好"));
        assert!(contains_asian_characters("Hello 世界"));
    }

    #[test]
    fn test_lobby_manager() {
        let mut lobby = LobbyManager::new();

        let channel = ChatChannel::new("General".to_string());
        lobby.add_channel(channel.clone());
        assert_eq!(lobby.get_channels().len(), 1);

        lobby.join_channel(channel);
        assert!(lobby.get_current_channel().is_some());

        lobby.leave_channel();
        assert!(lobby.get_current_channel().is_none());
    }

    #[test]
    fn test_game_lobby() {
        let mut game = GameLobbyInfo::new("Test Game".to_string(), "Host".to_string(), 8);

        assert!(game.is_joinable());

        game.player_count = 8;
        game.is_full = true;
        assert!(!game.is_joinable());
    }

    #[test]
    fn test_color_palette() {
        let palette = GameSpyColorPalette::default();

        let normal_color = palette.get_color(GameSpyColor::ChatNormal);
        assert_eq!(normal_color, make_color(255, 255, 255, 255));
    }
}
