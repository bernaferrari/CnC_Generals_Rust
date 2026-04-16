//! Stub game_network module for builds without the `network` feature.
//!
//! This provides the minimal API surface used by the Common crate so it can
//! compile without the real game_network crate (avoids cyclic dependency).

pub const PLAYERTEMPLATE_MIN: i32 = 0;
pub const PLAYERTEMPLATE_OBSERVER: i32 = 1;
pub const PLAYERTEMPLATE_RANDOM: i32 = 2;

pub mod config {
    pub const MAX_FRAMES_AHEAD: u32 = 0;
    pub const MIN_RUNAHEAD: u32 = 0;
    pub const TARGET_FPS: u32 = 30;
}

pub mod nat {
    #[derive(Clone, Default)]
    pub struct NatConfig;
}

pub mod security {
    pub mod firewall {
        #[derive(Clone, Default)]
        pub struct FirewallConfig;
    }
}

#[derive(Clone, Default)]
pub struct NetworkConfig {
    pub player_id: u32,
    pub max_frames_ahead: u32,
    pub min_runahead: u32,
    pub max_run_ahead: u32,
    pub target_frame_rate: u32,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub debug_mode: bool,
    pub nat: nat::NatConfig,
    pub firewall: security::firewall::FirewallConfig,
}

#[derive(Default)]
pub struct NetworkInterface;

impl NetworkInterface {
    pub async fn new(_config: NetworkConfig) -> Result<Self, String> {
        Ok(Self)
    }

    pub async fn update_concurrent(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub async fn is_frame_data_ready(&self) -> bool {
        true
    }
}

pub mod gamespy {
    pub mod peer_defs {
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        pub struct GamespyInfo;

        impl GamespyInfo {
            pub fn get_local_profile_id(&self) -> u32 {
                0
            }
        }

        pub fn get_gamespy_info() -> Option<Arc<Mutex<GamespyInfo>>> {
            None
        }
    }
}

pub mod game_info {
    use std::sync::Arc;

    use super::PLAYERTEMPLATE_RANDOM;

    #[derive(Clone, Debug, Default)]
    pub struct MultiplayerSettingsView {
        pub show_random_player_template: bool,
        pub show_random_start_pos: bool,
        pub show_random_color: bool,
        pub observer_color: Option<i32>,
        pub random_color: Option<i32>,
        pub color_values: Vec<i32>,
    }

    pub mod serialization {
        use super::GameInfo;

        pub fn game_info_to_ascii_string(_info: &GameInfo) -> String {
            String::new()
        }

        pub fn parse_ascii_string_to_game_info(_s: &str, _info: &mut GameInfo) -> bool {
            true
        }
    }

    pub type MapPlayersProvider = Arc<dyn Fn(&str) -> Option<i32> + Send + Sync>;
    pub type MultiplayerSettingsProvider = Arc<dyn Fn() -> MultiplayerSettingsView + Send + Sync>;
    pub type GameTextProvider = Arc<dyn Fn(&str) -> String + Send + Sync>;
    pub type PlayerTemplateDisplayNameProvider = Arc<dyn Fn(i32) -> Option<String> + Send + Sync>;

    pub fn set_map_players_provider(_provider: MapPlayersProvider) -> Result<(), ()> {
        Ok(())
    }

    pub fn set_multiplayer_settings_provider(
        _provider: MultiplayerSettingsProvider,
    ) -> Result<(), ()> {
        Ok(())
    }

    pub fn set_game_text_provider(_provider: GameTextProvider) -> Result<(), ()> {
        Ok(())
    }

    pub fn set_player_template_display_name_provider(
        _provider: PlayerTemplateDisplayNameProvider,
    ) -> Result<(), ()> {
        Ok(())
    }

    #[derive(Clone, Debug, Default)]
    pub struct GameSlot {
        name: String,
        ip: u32,
        is_human: bool,
        is_occupied: bool,
        player_template: i32,
    }

    impl GameSlot {
        pub fn get_name(&self) -> &str {
            &self.name
        }

        pub fn get_ip(&self) -> u32 {
            self.ip
        }

        pub fn is_human(&self) -> bool {
            self.is_human
        }

        pub fn is_occupied(&self) -> bool {
            self.is_occupied
        }

        pub fn get_player_template(&self) -> i32 {
            self.player_template
        }

        pub fn set_player_template(&mut self, template: i32) {
            self.player_template = template;
        }

        pub fn set_state(&mut self, state: SlotState, name: String, ip: u32) {
            match state {
                SlotState::Closed => {
                    self.is_human = false;
                    self.is_occupied = false;
                    self.player_template = PLAYERTEMPLATE_RANDOM;
                }
                SlotState::Player => {
                    self.is_human = true;
                    self.is_occupied = true;
                    self.player_template = PLAYERTEMPLATE_RANDOM;
                }
                SlotState::MedAI => {
                    self.is_human = false;
                    self.is_occupied = true;
                    self.player_template = PLAYERTEMPLATE_RANDOM;
                }
            }
            self.name = name;
            self.ip = ip;
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub enum SlotState {
        Closed,
        Player,
        MedAI,
    }

    #[derive(Clone, Debug)]
    pub struct GameInfo {
        map: String,
        seed: i32,
        crc_interval: i32,
        local_ip: u32,
        slots: Vec<GameSlot>,
    }

    impl Default for GameInfo {
        fn default() -> Self {
            Self::new()
        }
    }

    impl GameInfo {
        pub fn new() -> Self {
            Self {
                map: String::new(),
                seed: 0,
                crc_interval: 0,
                local_ip: 0,
                slots: vec![GameSlot::default(); 8],
            }
        }

        pub fn get_map(&self) -> &str {
            &self.map
        }

        pub fn set_map(&mut self, map: String) {
            self.map = map;
        }

        pub fn get_seed(&self) -> i32 {
            self.seed
        }

        pub fn set_seed(&mut self, seed: i32) {
            self.seed = seed;
        }

        pub fn get_crc_interval(&self) -> i32 {
            self.crc_interval
        }

        pub fn set_crc_interval(&mut self, interval: i32) {
            self.crc_interval = interval;
        }

        pub fn set_local_ip(&mut self, ip: u32) {
            self.local_ip = ip;
        }

        pub fn get_local_ip(&self) -> u32 {
            self.local_ip
        }

        pub fn get_slot(&self, index: usize) -> Option<&GameSlot> {
            self.slots.get(index)
        }

        pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut GameSlot> {
            self.slots.get_mut(index)
        }
    }
}
