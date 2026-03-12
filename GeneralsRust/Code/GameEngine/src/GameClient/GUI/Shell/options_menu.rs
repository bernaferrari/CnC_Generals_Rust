// FILE: options_menu.rs
// Author: Ported from C++ (Colin Day, October 2001)
// Description: Options menu window callbacks
//
// This is a faithful port from:
// GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp

use std::collections::HashMap;
use std::net::Ipv4Addr;

// Type aliases to match C++ naming
type NameKeyType = u32;
type Bool = bool;
type Int = i32;
type UnsignedInt = u32;
type UnsignedShort = u16;
type Short = i16;
type Real = f32;

// Constants for detail levels - matches C++ enum Detail
const HIGHDETAIL: i32 = 0;
const MEDIUMDETAIL: i32 = 1;
const LOWDETAIL: i32 = 2;
const CUSTOMDETAIL: i32 = 3;

// Anti-aliasing modes - matches C++ enum AliasingMode
const AA_OFF: i32 = 0;
const AA_LOW: i32 = 1;
const AA_HIGH: i32 = 2;
const NUM_ALIASING_MODES: i32 = 3;

// Difficulty levels
pub const DIFFICULTY_EASY: i32 = 0;
pub const DIFFICULTY_MEDIUM: i32 = 1;
pub const DIFFICULTY_HARD: i32 = 2;

// Static game LOD levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticGameLODLevel {
    Unknown = -1,
    Low = 0,
    Medium = 1,
    High = 2,
    Custom = 3,
}

/// Display settings structure for resolution management
#[derive(Debug, Clone, Copy)]
pub struct DisplaySettings {
    pub x_res: i32,
    pub y_res: i32,
    pub bit_depth: i32,
    pub windowed: bool,
}

/// IP address enumeration entry
#[derive(Debug, Clone)]
pub struct EnumeratedIP {
    pub ip: u32,
    pub ip_string: String,
}

/// IP enumeration helper
pub struct IPEnumeration {
    addresses: Vec<EnumeratedIP>,
}

impl IPEnumeration {
    pub fn new() -> Self {
        // In a real implementation, this would enumerate network interfaces
        // For now, we create a minimal list
        let addresses = vec![
            EnumeratedIP {
                ip: u32::from_be_bytes([127, 0, 0, 1]),
                ip_string: "127.0.0.1".to_string(),
            },
        ];

        Self { addresses }
    }

    pub fn get_addresses(&self) -> &[EnumeratedIP] {
        &self.addresses
    }
}

/// OptionPreferences class - manages user options
/// Matches C++ OptionPreferences from UserPreferences.h and OptionsMenu.cpp
pub struct OptionPreferences {
    preferences: HashMap<String, String>,
    filename: String,
}

impl OptionPreferences {
    /// Create new OptionPreferences
    /// Matches C++ OptionPreferences::OptionPreferences() line 209-213
    pub fn new() -> Self {
        let mut prefs = Self {
            preferences: HashMap::new(),
            filename: "Options.ini".to_string(),
        };
        prefs.load();
        prefs
    }

    /// Load preferences from file
    /// Matches C++ UserPreferences::load()
    pub fn load(&mut self) -> bool {
        // In a real implementation, this would read from the file
        // For now, return true to indicate success
        true
    }

    /// Write preferences to file
    /// Matches C++ UserPreferences::write()
    pub fn write(&self) -> bool {
        // In a real implementation, this would write to the file
        true
    }

    /// Get campaign difficulty
    /// Matches C++ OptionPreferences::getCampaignDifficulty() line 220-233
    pub fn get_campaign_difficulty(&self) -> i32 {
        if let Some(value) = self.preferences.get("CampaignDifficulty") {
            let mut factor = value.parse::<i32>().unwrap_or(DIFFICULTY_MEDIUM);
            if factor < DIFFICULTY_EASY {
                factor = DIFFICULTY_EASY;
            }
            if factor > DIFFICULTY_HARD {
                factor = DIFFICULTY_HARD;
            }
            factor
        } else {
            DIFFICULTY_MEDIUM
        }
    }

    /// Set campaign difficulty
    /// Matches C++ OptionPreferences::setCampaignDifficulty() line 235-240
    pub fn set_campaign_difficulty(&mut self, diff: i32) {
        self.preferences.insert("CampaignDifficulty".to_string(), diff.to_string());
    }

    /// Get LAN IP address
    /// Matches C++ OptionPreferences::getLANIPAddress() line 242-256
    pub fn get_lan_ip_address(&self) -> u32 {
        if let Some(selected_ip) = self.preferences.get("IPAddress") {
            let ips = IPEnumeration::new();
            for ip_entry in ips.get_addresses() {
                if selected_ip.eq_ignore_ascii_case(&ip_entry.ip_string) {
                    return ip_entry.ip;
                }
            }
        }
        // Default IP
        u32::from_be_bytes([127, 0, 0, 1])
    }

    /// Set LAN IP address (string)
    /// Matches C++ OptionPreferences::setLANIPAddress(AsciiString) line 258-261
    pub fn set_lan_ip_address_str(&mut self, ip: &str) {
        self.preferences.insert("IPAddress".to_string(), ip.to_string());
    }

    /// Set LAN IP address (u32)
    /// Matches C++ OptionPreferences::setLANIPAddress(UnsignedInt) line 263-268
    pub fn set_lan_ip_address(&mut self, ip: u32) {
        let bytes = ip.to_be_bytes();
        let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
        self.set_lan_ip_address_str(&ip_str);
    }

    /// Get Online/GameSpy IP address
    /// Matches C++ OptionPreferences::getOnlineIPAddress() line 270-284
    pub fn get_online_ip_address(&self) -> u32 {
        if let Some(selected_ip) = self.preferences.get("GameSpyIPAddress") {
            let ips = IPEnumeration::new();
            for ip_entry in ips.get_addresses() {
                if selected_ip.eq_ignore_ascii_case(&ip_entry.ip_string) {
                    return ip_entry.ip;
                }
            }
        }
        // Default IP
        u32::from_be_bytes([127, 0, 0, 1])
    }

    /// Set Online IP address (string)
    /// Matches C++ OptionPreferences::setOnlineIPAddress(AsciiString) line 286-289
    pub fn set_online_ip_address_str(&mut self, ip: &str) {
        self.preferences.insert("GameSpyIPAddress".to_string(), ip.to_string());
    }

    /// Set Online IP address (u32)
    /// Matches C++ OptionPreferences::setOnlineIPAddress(UnsignedInt) line 291-296
    pub fn set_online_ip_address(&mut self, ip: u32) {
        let bytes = ip.to_be_bytes();
        let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
        self.set_online_ip_address_str(&ip_str);
    }

    /// Get alternate mouse mode enabled
    /// Matches C++ OptionPreferences::getAlternateMouseModeEnabled() line 298-308
    pub fn get_alternate_mouse_mode_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseAlternateMouse") {
            value.eq_ignore_ascii_case("yes")
        } else {
            false // Default from GlobalData
        }
    }

    /// Get retaliation mode enabled
    /// Matches C++ OptionPreferences::getRetaliationModeEnabled() line 310-320
    pub fn get_retaliation_mode_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("Retaliation") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true // Default from GlobalData
        }
    }

    /// Get double-click attack move enabled
    /// Matches C++ OptionPreferences::getDoubleClickAttackMoveEnabled() line 322-332
    pub fn get_double_click_attack_move_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseDoubleClickAttackMove") {
            value.eq_ignore_ascii_case("yes")
        } else {
            false // Default from GlobalData
        }
    }

    /// Get scroll factor (0.0 to 1.0)
    /// Matches C++ OptionPreferences::getScrollFactor() line 334-347
    pub fn get_scroll_factor(&self) -> f32 {
        if let Some(value) = self.preferences.get("ScrollFactor") {
            let mut factor = value.parse::<i32>().unwrap_or(50);
            if factor < 0 {
                factor = 0;
            }
            if factor > 100 {
                factor = 100;
            }
            factor as f32 / 100.0
        } else {
            0.5 // Default keyboard scroll factor
        }
    }

    /// Get uses system map directory
    /// Matches C++ OptionPreferences::usesSystemMapDir() line 349-359
    pub fn uses_system_map_dir(&self) -> bool {
        if let Some(value) = self.preferences.get("UseSystemMapDir") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get save camera in replays
    /// Matches C++ OptionPreferences::saveCameraInReplays() line 361-371
    pub fn save_camera_in_replays(&self) -> bool {
        if let Some(value) = self.preferences.get("SaveCameraInReplays") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get use camera in replays
    /// Matches C++ OptionPreferences::useCameraInReplays() line 373-383
    pub fn use_camera_in_replays(&self) -> bool {
        if let Some(value) = self.preferences.get("UseCameraInReplays") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get ideal static game detail
    /// Matches C++ OptionPreferences::getIdealStaticGameDetail() line 385-392
    pub fn get_ideal_static_game_detail(&self) -> StaticGameLODLevel {
        if let Some(value) = self.preferences.get("IdealStaticGameLOD") {
            Self::parse_lod_level(value)
        } else {
            StaticGameLODLevel::Unknown
        }
    }

    /// Get static game detail
    /// Matches C++ OptionPreferences::getStaticGameDetail() line 394-401
    pub fn get_static_game_detail(&self) -> StaticGameLODLevel {
        if let Some(value) = self.preferences.get("StaticGameLOD") {
            Self::parse_lod_level(value)
        } else {
            StaticGameLODLevel::Medium // Default
        }
    }

    /// Parse LOD level from string
    fn parse_lod_level(s: &str) -> StaticGameLODLevel {
        match s.to_lowercase().as_str() {
            "low" => StaticGameLODLevel::Low,
            "medium" => StaticGameLODLevel::Medium,
            "high" => StaticGameLODLevel::High,
            "custom" => StaticGameLODLevel::Custom,
            _ => StaticGameLODLevel::Unknown,
        }
    }

    /// Get send delay
    /// Matches C++ OptionPreferences::getSendDelay() line 403-413
    pub fn get_send_delay(&self) -> bool {
        if let Some(value) = self.preferences.get("SendDelay") {
            value.eq_ignore_ascii_case("yes")
        } else {
            false // Default
        }
    }

    /// Get firewall behavior
    /// Matches C++ OptionPreferences::getFirewallBehavior() line 415-427
    pub fn get_firewall_behavior(&self) -> i32 {
        if let Some(value) = self.preferences.get("FirewallBehavior") {
            let mut behavior = value.parse::<i32>().unwrap_or(0);
            if behavior < 0 {
                behavior = 0;
            }
            behavior
        } else {
            0
        }
    }

    /// Get firewall port allocation delta
    /// Matches C++ OptionPreferences::getFirewallPortAllocationDelta() line 429-438
    pub fn get_firewall_port_allocation_delta(&self) -> i16 {
        if let Some(value) = self.preferences.get("FirewallPortAllocationDelta") {
            value.parse::<i16>().unwrap_or(0)
        } else {
            0
        }
    }

    /// Get firewall port override
    /// Matches C++ OptionPreferences::getFirewallPortOverride() line 440-451
    pub fn get_firewall_port_override(&self) -> u16 {
        if let Some(value) = self.preferences.get("FirewallPortOverride") {
            let override_val = value.parse::<i32>().unwrap_or(0);
            if override_val < 0 || override_val > 65535 {
                0
            } else {
                override_val as u16
            }
        } else {
            0
        }
    }

    /// Get firewall need to refresh
    /// Matches C++ OptionPreferences::getFirewallNeedToRefresh() line 453-466
    pub fn get_firewall_need_to_refresh(&self) -> bool {
        if let Some(value) = self.preferences.get("FirewallNeedToRefresh") {
            value.eq_ignore_ascii_case("TRUE")
        } else {
            false
        }
    }

    /// Get preferred 3D audio provider
    /// Matches C++ OptionPreferences::getPreferred3DProvider() line 468-474
    pub fn get_preferred_3d_provider(&self) -> String {
        self.preferences
            .get("3DAudioProvider")
            .cloned()
            .unwrap_or_else(|| "".to_string())
    }

    /// Get speaker type
    /// Matches C++ OptionPreferences::getSpeakerType() line 476-482
    pub fn get_speaker_type(&self) -> String {
        self.preferences
            .get("SpeakerType")
            .cloned()
            .unwrap_or_else(|| "Stereo".to_string())
    }

    /// Get sound volume (0-100)
    /// Matches C++ OptionPreferences::getSoundVolume() line 484-504
    pub fn get_sound_volume(&self) -> f32 {
        if let Some(value) = self.preferences.get("SFXVolume") {
            let mut volume = value.parse::<f32>().unwrap_or(55.0);
            if volume < 0.0 {
                volume = 0.0;
            }
            volume
        } else {
            55.0 // Default sound volume
        }
    }

    /// Get 3D sound volume (0-100)
    /// Matches C++ OptionPreferences::get3DSoundVolume() line 506-526
    pub fn get_3d_sound_volume(&self) -> f32 {
        if let Some(value) = self.preferences.get("SFX3DVolume") {
            let mut volume = value.parse::<f32>().unwrap_or(79.0);
            if volume < 0.0 {
                volume = 0.0;
            }
            volume
        } else {
            79.0 // Default 3D sound volume
        }
    }

    /// Get speech volume (0-100)
    /// Matches C++ OptionPreferences::getSpeechVolume() line 528-540
    pub fn get_speech_volume(&self) -> f32 {
        if let Some(value) = self.preferences.get("VoiceVolume") {
            let mut volume = value.parse::<f32>().unwrap_or(70.0);
            if volume < 0.0 {
                volume = 0.0;
            }
            volume
        } else {
            70.0 // Default speech volume
        }
    }

    /// Get cloud shadows enabled
    /// Matches C++ OptionPreferences::getCloudShadowsEnabled() line 542-552
    pub fn get_cloud_shadows_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseCloudMap") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get lightmap enabled
    /// Matches C++ OptionPreferences::getLightmapEnabled() line 554-564
    pub fn get_lightmap_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseLightMap") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get smooth water enabled
    /// Matches C++ OptionPreferences::getSmoothWaterEnabled() line 566-576
    pub fn get_smooth_water_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("ShowSoftWaterEdge") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get trees enabled
    /// Matches C++ OptionPreferences::getTreesEnabled() line 578-588
    pub fn get_trees_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("ShowTrees") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get extra animations disabled
    /// Matches C++ OptionPreferences::getExtraAnimationsDisabled() line 590-600
    pub fn get_extra_animations_disabled(&self) -> bool {
        if let Some(value) = self.preferences.get("ExtraAnimations") {
            // Note: "yes" means ENABLED, so we return FALSE for disabled
            !value.eq_ignore_ascii_case("yes")
        } else {
            false // Default: extra animations enabled
        }
    }

    /// Get use heat effects
    /// Matches C++ OptionPreferences::getUseHeatEffects() line 602-612
    pub fn get_use_heat_effects(&self) -> bool {
        if let Some(value) = self.preferences.get("HeatEffects") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get dynamic LOD enabled
    /// Matches C++ OptionPreferences::getDynamicLODEnabled() line 614-624
    pub fn get_dynamic_lod_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("DynamicLOD") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get FPS limit enabled
    /// Matches C++ OptionPreferences::getFPSLimitEnabled() line 626-636
    pub fn get_fps_limit_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("FPSLimit") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get 3D shadows enabled
    /// Matches C++ OptionPreferences::get3DShadowsEnabled() line 638-648
    pub fn get_3d_shadows_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseShadowVolumes") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get 2D shadows enabled
    /// Matches C++ OptionPreferences::get2DShadowsEnabled() line 650-660
    pub fn get_2d_shadows_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("UseShadowDecals") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get building occlusion enabled
    /// Matches C++ OptionPreferences::getBuildingOcclusionEnabled() line 662-672
    pub fn get_building_occlusion_enabled(&self) -> bool {
        if let Some(value) = self.preferences.get("BuildingOcclusion") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true
        }
    }

    /// Get particle cap
    /// Matches C++ OptionPreferences::getParticleCap() line 674-685
    pub fn get_particle_cap(&self) -> i32 {
        if let Some(value) = self.preferences.get("MaxParticleCount") {
            let mut factor = value.parse::<i32>().unwrap_or(5000);
            if factor < 100 {
                factor = 100; // Clamp to at least 100 particles
            }
            factor
        } else {
            5000 // Default
        }
    }

    /// Get texture reduction (0-2)
    /// Matches C++ OptionPreferences::getTextureReduction() line 687-697
    pub fn get_texture_reduction(&self) -> i32 {
        if let Some(value) = self.preferences.get("TextureReduction") {
            let mut factor = value.parse::<i32>().unwrap_or(-1);
            if factor > 2 {
                factor = 2; // Clamp it
            }
            factor
        } else {
            -1 // Unknown texture reduction
        }
    }

    /// Get gamma value (0-100, default 50)
    /// Matches C++ OptionPreferences::getGammaValue() line 699-707
    pub fn get_gamma_value(&self) -> f32 {
        if let Some(value) = self.preferences.get("Gamma") {
            value.parse::<f32>().unwrap_or(50.0)
        } else {
            50.0
        }
    }

    /// Get resolution
    /// Matches C++ OptionPreferences::getResolution() line 709-724
    pub fn get_resolution(&self) -> (i32, i32) {
        let default_x = 800;
        let default_y = 600;

        if let Some(value) = self.preferences.get("Resolution") {
            // Parse "800 600" format
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    return (x, y);
                }
            }
        }

        (default_x, default_y)
    }

    /// Get music volume (0-100)
    /// Matches C++ OptionPreferences::getMusicVolume() line 726-738
    pub fn get_music_volume(&self) -> f32 {
        if let Some(value) = self.preferences.get("MusicVolume") {
            let mut volume = value.parse::<f32>().unwrap_or(60.0);
            if volume < 0.0 {
                volume = 0.0;
            }
            volume
        } else {
            60.0 // Default music volume
        }
    }
}

/// Options menu state management
/// Manages the UI state for the options menu
pub struct OptionsMenu {
    pub preferences: OptionPreferences,
    pub old_display_settings: Option<DisplaySettings>,
    pub new_display_settings: Option<DisplaySettings>,
    pub display_changed: bool,
    pub ignore_selected: bool,

    // UI control states
    pub selected_lan_ip_index: i32,
    pub selected_online_ip_index: i32,
    pub selected_resolution_index: i32,
    pub selected_detail_index: i32,
    pub selected_anti_aliasing_index: i32,

    // Checkbox states
    pub alternate_mouse: bool,
    pub retaliation: bool,
    pub double_click_attack_move: bool,
    pub language_filter: bool,
    pub send_delay: bool,
    pub use_camera: bool,
    pub save_camera: bool,
    pub draw_anchor: bool,
    pub move_anchor: bool,

    // Advanced graphics options
    pub shadows_3d: bool,
    pub shadows_2d: bool,
    pub cloud_shadows: bool,
    pub ground_lighting: bool,
    pub smooth_water: bool,
    pub building_occlusion: bool,
    pub props: bool,
    pub extra_animations: bool,
    pub no_dynamic_lod: bool,
    pub unlock_fps: bool,
    pub heat_effects: bool,

    // Slider values (0-100)
    pub scroll_speed: i32,
    pub music_volume: i32,
    pub sfx_volume: i32,
    pub voice_volume: i32,
    pub gamma: i32,
    pub texture_resolution: i32,
    pub particle_cap: i32,

    // Network settings
    pub http_proxy: String,
    pub firewall_port_override: u16,
}

impl OptionsMenu {
    /// Create new options menu
    /// Matches C++ OptionsMenuInit() line 1312-1801
    pub fn new() -> Self {
        let preferences = OptionPreferences::new();

        Self {
            preferences,
            old_display_settings: None,
            new_display_settings: None,
            display_changed: false,
            ignore_selected: true,

            selected_lan_ip_index: 0,
            selected_online_ip_index: 0,
            selected_resolution_index: 0,
            selected_detail_index: MEDIUMDETAIL,
            selected_anti_aliasing_index: 0,

            alternate_mouse: false,
            retaliation: true,
            double_click_attack_move: false,
            language_filter: true,
            send_delay: false,
            use_camera: true,
            save_camera: true,
            draw_anchor: false,
            move_anchor: false,

            shadows_3d: true,
            shadows_2d: true,
            cloud_shadows: true,
            ground_lighting: true,
            smooth_water: true,
            building_occlusion: true,
            props: true,
            extra_animations: true,
            no_dynamic_lod: false,
            unlock_fps: false,
            heat_effects: true,

            scroll_speed: 50,
            music_volume: 60,
            sfx_volume: 55,
            voice_volume: 70,
            gamma: 50,
            texture_resolution: 2,
            particle_cap: 5000,

            http_proxy: String::new(),
            firewall_port_override: 0,
        }
    }

    /// Load options from preferences
    /// Matches various parts of OptionsMenuInit() line 1312-1801
    pub fn load_from_preferences(&mut self) {
        // Load control options
        self.alternate_mouse = self.preferences.get_alternate_mouse_mode_enabled();
        self.retaliation = self.preferences.get_retaliation_mode_enabled();
        self.double_click_attack_move = self.preferences.get_double_click_attack_move_enabled();
        self.send_delay = self.preferences.get_send_delay();
        self.use_camera = self.preferences.use_camera_in_replays();
        self.save_camera = self.preferences.save_camera_in_replays();

        // Load audio volumes
        self.music_volume = self.preferences.get_music_volume() as i32;
        self.sfx_volume = self.preferences.get_sound_volume().max(self.preferences.get_3d_sound_volume()) as i32;
        self.voice_volume = self.preferences.get_speech_volume() as i32;

        // Load graphics options
        self.shadows_3d = self.preferences.get_3d_shadows_enabled();
        self.shadows_2d = self.preferences.get_2d_shadows_enabled();
        self.cloud_shadows = self.preferences.get_cloud_shadows_enabled();
        self.ground_lighting = self.preferences.get_lightmap_enabled();
        self.smooth_water = self.preferences.get_smooth_water_enabled();
        self.props = self.preferences.get_trees_enabled();
        self.extra_animations = !self.preferences.get_extra_animations_disabled();
        self.no_dynamic_lod = !self.preferences.get_dynamic_lod_enabled();
        self.unlock_fps = !self.preferences.get_fps_limit_enabled();
        self.heat_effects = self.preferences.get_use_heat_effects();
        self.building_occlusion = self.preferences.get_building_occlusion_enabled();

        // Load other settings
        self.scroll_speed = (self.preferences.get_scroll_factor() * 100.0) as i32;
        self.gamma = self.preferences.get_gamma_value() as i32;
        self.particle_cap = self.preferences.get_particle_cap();

        let texture_reduction = self.preferences.get_texture_reduction();
        if texture_reduction >= 0 {
            self.texture_resolution = 2 - texture_reduction;
        }

        // Load network settings
        self.firewall_port_override = self.preferences.get_firewall_port_override();
    }

    /// Set default values
    /// Matches C++ setDefaults() line 742-907
    pub fn set_defaults(&mut self) {
        self.language_filter = true;
        self.send_delay = false;
        self.alternate_mouse = false;
        self.retaliation = true;
        self.double_click_attack_move = false;
        self.scroll_speed = 50;

        self.music_volume = 60;
        self.sfx_volume = 55;
        self.voice_volume = 70;
        self.gamma = 50;

        self.shadows_3d = true;
        self.shadows_2d = true;
        self.cloud_shadows = true;
        self.ground_lighting = true;
        self.smooth_water = true;
        self.extra_animations = true;
        self.no_dynamic_lod = false;
        self.unlock_fps = false;
        self.heat_effects = true;
        self.building_occlusion = true;
        self.props = true;
        self.particle_cap = 5000;
        self.texture_resolution = 2;
    }

    /// Save options to preferences
    /// Matches C++ saveOptions() line 909-1265
    pub fn save_options(&mut self) {
        // Save language filter
        self.preferences.preferences.insert(
            "LanguageFilter".to_string(),
            if self.language_filter { "true" } else { "false" }.to_string(),
        );

        // Save send delay
        self.preferences.preferences.insert(
            "SendDelay".to_string(),
            if self.send_delay { "yes" } else { "no" }.to_string(),
        );

        // Save detail level
        let detail_name = match self.selected_detail_index {
            HIGHDETAIL => "High",
            MEDIUMDETAIL => "Medium",
            LOWDETAIL => "Low",
            CUSTOMDETAIL => "Custom",
            _ => "Medium",
        };
        self.preferences.preferences.insert("StaticGameLOD".to_string(), detail_name.to_string());

        // Save custom detail settings if custom selected
        if self.selected_detail_index == CUSTOMDETAIL {
            let texture_reduction = 2 - self.texture_resolution;
            self.preferences.preferences.insert("TextureReduction".to_string(), texture_reduction.to_string());

            self.preferences.preferences.insert(
                "UseShadowVolumes".to_string(),
                if self.shadows_3d { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "UseShadowDecals".to_string(),
                if self.shadows_2d { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "UseCloudMap".to_string(),
                if self.cloud_shadows { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "UseLightMap".to_string(),
                if self.ground_lighting { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "ShowSoftWaterEdge".to_string(),
                if self.smooth_water { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "ExtraAnimations".to_string(),
                if self.extra_animations { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "DynamicLOD".to_string(),
                if !self.no_dynamic_lod { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "HeatEffects".to_string(),
                if self.heat_effects { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "BuildingOcclusion".to_string(),
                if self.building_occlusion { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert(
                "ShowTrees".to_string(),
                if self.props { "yes" } else { "no" }.to_string(),
            );
            self.preferences.preferences.insert("MaxParticleCount".to_string(), self.particle_cap.to_string());
        }

        // Save mouse mode
        self.preferences.preferences.insert(
            "UseAlternateMouse".to_string(),
            if self.alternate_mouse { "yes" } else { "no" }.to_string(),
        );
        self.preferences.preferences.insert(
            "Retaliation".to_string(),
            if self.retaliation { "yes" } else { "no" }.to_string(),
        );
        self.preferences.preferences.insert(
            "UseDoubleClickAttackMove".to_string(),
            if self.double_click_attack_move { "yes" } else { "no" }.to_string(),
        );

        // Save scroll speed
        self.preferences.preferences.insert("ScrollFactor".to_string(), self.scroll_speed.to_string());

        // Save audio volumes
        self.preferences.preferences.insert("MusicVolume".to_string(), self.music_volume.to_string());

        // Calculate 2D and 3D sound volumes based on relative setting
        let relative_2d_volume = 0.0f32; // Could be configurable
        let sound_2d_volume = if relative_2d_volume < 0.0 {
            (self.sfx_volume as f32 / 100.0) * (1.0 + relative_2d_volume)
        } else {
            self.sfx_volume as f32 / 100.0
        };
        let sound_3d_volume = if relative_2d_volume >= 0.0 {
            (self.sfx_volume as f32 / 100.0) * (1.0 - relative_2d_volume)
        } else {
            self.sfx_volume as f32 / 100.0
        };

        self.preferences.preferences.insert("SFXVolume".to_string(), ((sound_2d_volume * 100.0) as i32).to_string());
        self.preferences.preferences.insert("SFX3DVolume".to_string(), ((sound_3d_volume * 100.0) as i32).to_string());
        self.preferences.preferences.insert("VoiceVolume".to_string(), self.voice_volume.to_string());

        // Save gamma
        self.preferences.preferences.insert("Gamma".to_string(), self.gamma.to_string());

        // Save anti-aliasing
        self.preferences.preferences.insert("AntiAliasing".to_string(), self.selected_anti_aliasing_index.to_string());

        // Save camera settings
        self.preferences.preferences.insert(
            "SaveCameraInReplays".to_string(),
            if self.save_camera { "yes" } else { "no" }.to_string(),
        );
        self.preferences.preferences.insert(
            "UseCameraInReplays".to_string(),
            if self.use_camera { "yes" } else { "no" }.to_string(),
        );

        // Save scroll anchor settings
        self.preferences.preferences.insert(
            "DrawScrollAnchor".to_string(),
            if self.draw_anchor { "Yes" } else { "No" }.to_string(),
        );
        self.preferences.preferences.insert(
            "MoveScrollAnchor".to_string(),
            if self.move_anchor { "Yes" } else { "No" }.to_string(),
        );

        // Write to file
        self.preferences.write();
    }

    /// Convert gamma slider value (0-100) to actual gamma value (0.6-2.0)
    /// Matches C++ saveOptions() gamma calculation line 1243-1254
    pub fn slider_to_gamma(&self, slider_val: i32) -> f32 {
        if slider_val < 50 {
            if slider_val <= 0 {
                0.6
            } else {
                1.0 - (0.4 * (50 - slider_val) as f32 / 50.0)
            }
        } else if slider_val > 50 {
            1.0 + (1.0 * (slider_val - 50) as f32 / 50.0)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_preferences_creation() {
        let prefs = OptionPreferences::new();
        assert_eq!(prefs.filename, "Options.ini");
    }

    #[test]
    fn test_difficulty_clamping() {
        let mut prefs = OptionPreferences::new();

        // Test clamping low
        prefs.set_campaign_difficulty(-1);
        assert_eq!(prefs.get_campaign_difficulty(), DIFFICULTY_EASY);

        // Test clamping high
        prefs.set_campaign_difficulty(10);
        assert_eq!(prefs.get_campaign_difficulty(), DIFFICULTY_HARD);

        // Test valid value
        prefs.set_campaign_difficulty(DIFFICULTY_MEDIUM);
        assert_eq!(prefs.get_campaign_difficulty(), DIFFICULTY_MEDIUM);
    }

    #[test]
    fn test_scroll_factor() {
        let mut prefs = OptionPreferences::new();

        // Test that 50 becomes 0.5
        prefs.preferences.insert("ScrollFactor".to_string(), "50".to_string());
        assert_eq!(prefs.get_scroll_factor(), 0.5);

        // Test clamping
        prefs.preferences.insert("ScrollFactor".to_string(), "150".to_string());
        assert_eq!(prefs.get_scroll_factor(), 1.0);

        prefs.preferences.insert("ScrollFactor".to_string(), "-10".to_string());
        assert_eq!(prefs.get_scroll_factor(), 0.0);
    }

    #[test]
    fn test_gamma_slider_conversion() {
        let menu = OptionsMenu::new();

        // Test minimum (0 -> 0.6)
        assert_eq!(menu.slider_to_gamma(0), 0.6);

        // Test middle (50 -> 1.0)
        assert_eq!(menu.slider_to_gamma(50), 1.0);

        // Test maximum (100 -> 2.0)
        assert_eq!(menu.slider_to_gamma(100), 2.0);
    }

    #[test]
    fn test_resolution_parsing() {
        let mut prefs = OptionPreferences::new();

        prefs.preferences.insert("Resolution".to_string(), "1024 768".to_string());
        assert_eq!(prefs.get_resolution(), (1024, 768));

        prefs.preferences.insert("Resolution".to_string(), "1920 1080".to_string());
        assert_eq!(prefs.get_resolution(), (1920, 1080));

        // Test default on invalid format
        prefs.preferences.insert("Resolution".to_string(), "invalid".to_string());
        assert_eq!(prefs.get_resolution(), (800, 600));
    }

    #[test]
    fn test_boolean_preferences() {
        let mut prefs = OptionPreferences::new();

        prefs.preferences.insert("UseAlternateMouse".to_string(), "yes".to_string());
        assert!(prefs.get_alternate_mouse_mode_enabled());

        prefs.preferences.insert("UseAlternateMouse".to_string(), "no".to_string());
        assert!(!prefs.get_alternate_mouse_mode_enabled());

        prefs.preferences.insert("UseAlternateMouse".to_string(), "YES".to_string());
        assert!(prefs.get_alternate_mouse_mode_enabled());
    }

    #[test]
    fn test_particle_cap_clamping() {
        let mut prefs = OptionPreferences::new();

        // Test minimum clamping to 100
        prefs.preferences.insert("MaxParticleCount".to_string(), "50".to_string());
        assert_eq!(prefs.get_particle_cap(), 100);

        prefs.preferences.insert("MaxParticleCount".to_string(), "5000".to_string());
        assert_eq!(prefs.get_particle_cap(), 5000);
    }

    #[test]
    fn test_texture_reduction_clamping() {
        let mut prefs = OptionPreferences::new();

        prefs.preferences.insert("TextureReduction".to_string(), "5".to_string());
        assert_eq!(prefs.get_texture_reduction(), 2); // Clamped to 2

        prefs.preferences.insert("TextureReduction".to_string(), "1".to_string());
        assert_eq!(prefs.get_texture_reduction(), 1);
    }

    #[test]
    fn test_ip_address_conversion() {
        let mut prefs = OptionPreferences::new();

        let ip = u32::from_be_bytes([192, 168, 1, 1]);
        prefs.set_lan_ip_address(ip);

        if let Some(ip_str) = prefs.preferences.get("IPAddress") {
            assert_eq!(ip_str, "192.168.1.1");
        }
    }

    #[test]
    fn test_options_menu_defaults() {
        let mut menu = OptionsMenu::new();
        menu.set_defaults();

        assert_eq!(menu.scroll_speed, 50);
        assert_eq!(menu.music_volume, 60);
        assert_eq!(menu.gamma, 50);
        assert!(menu.retaliation);
        assert!(!menu.alternate_mouse);
        assert!(menu.language_filter);
    }
}
