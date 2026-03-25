use crate::gui::callbacks::menus::main_menu::GameDifficultyPort;
use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
use std::collections::HashMap;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/OptionsMenu.cpp",
    "crate::gui::callbacks::menus::options_menu",
    "Options Menu",
    "Options shell callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "OptionsMenu",
    "Options",
    "Audio, video, gameplay, and control options.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsTabPort {
    Audio,
    Video,
    Gameplay,
    Controls,
    AdvancedDisplay,
}

impl OptionsTabPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Video => "Video",
            Self::Gameplay => "Gameplay",
            Self::Controls => "Controls",
            Self::AdvancedDisplay => "Advanced",
        }
    }
}

const HIGHDETAIL: i32 = 0;
const MEDIUMDETAIL: i32 = 1;
const LOWDETAIL: i32 = 2;
const CUSTOMDETAIL: i32 = 3;

const DIFFICULTY_EASY: i32 = 0;
const DIFFICULTY_MEDIUM: i32 = 1;
const DIFFICULTY_HARD: i32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetailLevel {
    High = 0,
    Medium = 1,
    Low = 2,
    Custom = 3,
}

impl DetailLevel {
    pub fn from_index(index: i32) -> Self {
        match index {
            HIGHDETAIL => Self::High,
            MEDIUMDETAIL => Self::Medium,
            LOWDETAIL => Self::Low,
            CUSTOMDETAIL => Self::Custom,
            _ => Self::Medium,
        }
    }

    pub fn to_index(self) -> i32 {
        self as i32
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsButtonId {
    Back,
    Accept,
    Defaults,
    KeyboardOptions,
    AdvancedAccept,
    AdvancedCancel,
    FirewallRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsCheckboxId {
    AlternateMouse,
    Retaliation,
    DoubleClickAttackMove,
    LanguageFilter,
    SendDelay,
    SaveCamera,
    UseCamera,
    DrawAnchor,
    MoveAnchor,
    Shadows3D,
    Shadows2D,
    CloudShadows,
    GroundLighting,
    SmoothWater,
    BuildingOcclusion,
    ExtraAnimations,
    NoDynamicLod,
    UnlockFps,
    HeatEffects,
    Props,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsComboBoxId {
    Resolution,
    Detail,
    AntiAliasing,
    LanIP,
    OnlineIP,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsSliderId {
    ScrollSpeed,
    MusicVolume,
    SFXVolume,
    VoiceVolume,
    Gamma,
    TextureResolution,
    ParticleCap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptionsMenuPort {
    pub active_tab: OptionsTabPort,
    pub online_ip: String,
    pub lan_ip: String,
    pub anti_aliasing: u8,
    pub resolution: (u16, u16),
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub voice_volume: f32,
    pub scroll_speed: f32,
    pub gamma: f32,
    pub alternate_mouse: bool,
    pub retaliation_mode: bool,
    pub double_click_attack_move: bool,
    pub language_filter: bool,
    pub use_camera_in_replays: bool,
    pub save_camera_in_replays: bool,
    pub draw_scroll_anchor: bool,
    pub move_scroll_anchor: bool,
    pub cloud_shadows: bool,
    pub ground_lighting: bool,
    pub smooth_water: bool,
    pub building_occlusion: bool,
    pub extra_animations: bool,
    pub dynamic_lod: bool,
    pub unlock_fps: bool,
    pub heat_effects: bool,
    pub use_3d_shadows: bool,
    pub use_2d_shadows: bool,
    pub particle_cap: u32,
    pub texture_reduction: u8,
    pub campaign_difficulty: GameDifficultyPort,
}

impl Default for OptionsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl OptionsMenuPort {
    pub fn sample() -> Self {
        Self {
            active_tab: OptionsTabPort::Video,
            online_ip: "203.0.113.24".to_string(),
            lan_ip: "192.168.1.20".to_string(),
            anti_aliasing: 2,
            resolution: (1920, 1080),
            music_volume: 0.68,
            sfx_volume: 0.82,
            voice_volume: 0.74,
            scroll_speed: 0.55,
            gamma: 0.50,
            alternate_mouse: false,
            retaliation_mode: true,
            double_click_attack_move: false,
            language_filter: true,
            use_camera_in_replays: true,
            save_camera_in_replays: true,
            draw_scroll_anchor: true,
            move_scroll_anchor: false,
            cloud_shadows: true,
            ground_lighting: true,
            smooth_water: true,
            building_occlusion: true,
            extra_animations: true,
            dynamic_lod: true,
            unlock_fps: false,
            heat_effects: true,
            use_3d_shadows: true,
            use_2d_shadows: true,
            particle_cap: 5000,
            texture_reduction: 1,
            campaign_difficulty: GameDifficultyPort::Normal,
        }
    }

    pub fn set_resolution(&mut self, x: u16, y: u16) {
        self.resolution = (x, y);
    }

    pub fn set_audio_levels(&mut self, music: f32, sfx: f32, voice: f32) {
        self.music_volume = music.clamp(0.0, 1.0);
        self.sfx_volume = sfx.clamp(0.0, 1.0);
        self.voice_volume = voice.clamp(0.0, 1.0);
    }

    pub fn set_scroll_speed(&mut self, scroll_speed: f32) {
        self.scroll_speed = scroll_speed.clamp(0.0, 1.0);
    }

    pub fn set_campaign_difficulty(&mut self, difficulty: GameDifficultyPort) {
        self.campaign_difficulty = difficulty;
    }
}

#[derive(Clone, Debug)]
pub struct DisplaySettings {
    pub x_res: i32,
    pub y_res: i32,
    pub bit_depth: i32,
    pub windowed: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            x_res: 800,
            y_res: 600,
            bit_depth: 32,
            windowed: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OptionPreferences {
    pub preferences: HashMap<String, String>,
    pub filename: String,
}

impl Default for OptionPreferences {
    fn default() -> Self {
        Self {
            preferences: HashMap::new(),
            filename: "Options.ini".to_string(),
        }
    }
}

impl OptionPreferences {
    pub fn new() -> Self {
        let mut prefs = Self::default();
        prefs.load();
        prefs
    }

    pub fn load(&mut self) -> bool {
        true
    }

    pub fn write(&self) -> bool {
        true
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.preferences.get(key)
    }

    pub fn set(&mut self, key: &str, value: String) {
        self.preferences.insert(key.to_string(), value);
    }

    pub fn get_bool_yes(&self, key: &str, default: bool) -> bool {
        if let Some(value) = self.preferences.get(key) {
            value.eq_ignore_ascii_case("yes") || value.eq_ignore_ascii_case("true")
        } else {
            default
        }
    }

    pub fn get_int_clamped(&self, key: &str, default: i32, min: i32, max: i32) -> i32 {
        if let Some(value) = self.preferences.get(key) {
            let mut v = value.parse::<i32>().unwrap_or(default);
            v = v.max(min).min(max);
            v
        } else {
            default
        }
    }

    pub fn get_real_clamped(&self, key: &str, default: f32, min: f32) -> f32 {
        if let Some(value) = self.preferences.get(key) {
            let mut v = value.parse::<f32>().unwrap_or(default);
            if v < min {
                v = min;
            }
            v
        } else {
            default
        }
    }

    pub fn get_campaign_difficulty(&self) -> i32 {
        self.get_int_clamped(
            "CampaignDifficulty",
            DIFFICULTY_MEDIUM,
            DIFFICULTY_EASY,
            DIFFICULTY_HARD,
        )
    }

    pub fn set_campaign_difficulty(&mut self, diff: i32) {
        self.preferences
            .insert("CampaignDifficulty".to_string(), diff.to_string());
    }

    pub fn get_alternate_mouse_mode_enabled(&self) -> bool {
        self.get_bool_yes("UseAlternateMouse", false)
    }

    pub fn get_retaliation_mode_enabled(&self) -> bool {
        self.get_bool_yes("Retaliation", true)
    }

    pub fn get_double_click_attack_move_enabled(&self) -> bool {
        self.get_bool_yes("UseDoubleClickAttackMove", false)
    }

    pub fn get_scroll_factor(&self) -> f32 {
        let factor = self.get_int_clamped("ScrollFactor", 50, 0, 100);
        factor as f32 / 100.0
    }

    pub fn save_camera_in_replays(&self) -> bool {
        self.get_bool_yes("SaveCameraInReplays", true)
    }

    pub fn use_camera_in_replays(&self) -> bool {
        self.get_bool_yes("UseCameraInReplays", true)
    }

    pub fn get_send_delay(&self) -> bool {
        self.get_bool_yes("SendDelay", false)
    }

    pub fn get_static_game_detail(&self) -> DetailLevel {
        if let Some(value) = self.preferences.get("StaticGameLOD") {
            match value.to_lowercase().as_str() {
                "low" => DetailLevel::Low,
                "medium" => DetailLevel::Medium,
                "high" => DetailLevel::High,
                "custom" => DetailLevel::Custom,
                _ => DetailLevel::Medium,
            }
        } else {
            DetailLevel::Medium
        }
    }

    pub fn get_cloud_shadows_enabled(&self) -> bool {
        self.get_bool_yes("UseCloudMap", true)
    }

    pub fn get_lightmap_enabled(&self) -> bool {
        self.get_bool_yes("UseLightMap", true)
    }

    pub fn get_smooth_water_enabled(&self) -> bool {
        self.get_bool_yes("ShowSoftWaterEdge", true)
    }

    pub fn get_trees_enabled(&self) -> bool {
        self.get_bool_yes("ShowTrees", true)
    }

    pub fn get_extra_animations_disabled(&self) -> bool {
        if let Some(value) = self.preferences.get("ExtraAnimations") {
            !value.eq_ignore_ascii_case("yes")
        } else {
            false
        }
    }

    pub fn get_use_heat_effects(&self) -> bool {
        self.get_bool_yes("HeatEffects", true)
    }

    pub fn get_dynamic_lod_enabled(&self) -> bool {
        self.get_bool_yes("DynamicLOD", true)
    }

    pub fn get_fps_limit_enabled(&self) -> bool {
        self.get_bool_yes("FPSLimit", true)
    }

    pub fn get_3d_shadows_enabled(&self) -> bool {
        self.get_bool_yes("UseShadowVolumes", true)
    }

    pub fn get_2d_shadows_enabled(&self) -> bool {
        self.get_bool_yes("UseShadowDecals", true)
    }

    pub fn get_building_occlusion_enabled(&self) -> bool {
        self.get_bool_yes("BuildingOcclusion", true)
    }

    pub fn get_particle_cap(&self) -> i32 {
        self.get_int_clamped("MaxParticleCount", 5000, 100, i32::MAX)
    }

    pub fn get_texture_reduction(&self) -> i32 {
        if let Some(value) = self.preferences.get("TextureReduction") {
            let mut factor = value.parse::<i32>().unwrap_or(-1);
            if factor > 2 {
                factor = 2;
            }
            factor
        } else {
            -1
        }
    }

    pub fn get_gamma_value(&self) -> f32 {
        self.get_int_clamped("Gamma", 50, 0, 100) as f32
    }

    pub fn get_resolution(&self) -> (i32, i32) {
        if let Some(value) = self.preferences.get("Resolution") {
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    return (x, y);
                }
            }
        }
        (800, 600)
    }

    pub fn get_music_volume(&self) -> f32 {
        self.get_real_clamped("MusicVolume", 60.0, 0.0)
    }

    pub fn get_sound_volume(&self) -> f32 {
        self.get_real_clamped("SFXVolume", 55.0, 0.0)
    }

    pub fn get_3d_sound_volume(&self) -> f32 {
        self.get_real_clamped("SFX3DVolume", 55.0, 0.0)
    }

    pub fn get_speech_volume(&self) -> f32 {
        self.get_real_clamped("VoiceVolume", 70.0, 0.0)
    }

    pub fn get_firewall_port_override(&self) -> u16 {
        let val = self.get_int_clamped("FirewallPortOverride", 0, 0, 65535);
        val as u16
    }

    pub fn get_language_filter(&self) -> bool {
        if let Some(value) = self.preferences.get("LanguageFilter") {
            value.eq_ignore_ascii_case("true")
        } else {
            true
        }
    }

    pub fn set_lan_ip_address(&mut self, ip: u32) {
        let bytes = ip.to_be_bytes();
        let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
        self.preferences.insert("IPAddress".to_string(), ip_str);
    }

    pub fn set_online_ip_address(&mut self, ip: u32) {
        let bytes = ip.to_be_bytes();
        let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
        self.preferences
            .insert("GameSpyIPAddress".to_string(), ip_str);
    }
}

pub struct OptionsMenuController {
    preferences: OptionPreferences,
    snapshot: OptionMenuSnapshot,
    detail_level: DetailLevel,
    selected_resolution_index: i32,
    selected_anti_aliasing_index: i32,
    old_display_settings: DisplaySettings,
    new_display_settings: DisplaySettings,
    display_changed: bool,
    ignore_selected: bool,
    advanced_display_visible: bool,
    scroll_speed: i32,
    music_volume: i32,
    sfx_volume: i32,
    voice_volume: i32,
    gamma: i32,
    texture_resolution: i32,
    particle_cap: i32,
    language_filter: bool,
    send_delay: bool,
    use_camera: bool,
    save_camera: bool,
    draw_anchor: bool,
    move_anchor: bool,
    alternate_mouse: bool,
    retaliation: bool,
    double_click_attack_move: bool,
    shadows_3d: bool,
    shadows_2d: bool,
    cloud_shadows: bool,
    ground_lighting: bool,
    smooth_water: bool,
    building_occlusion: bool,
    props: bool,
    extra_animations: bool,
    no_dynamic_lod: bool,
    unlock_fps: bool,
    heat_effects: bool,
    in_game: bool,
}

#[derive(Clone, Debug)]
struct OptionMenuSnapshot {
    scroll_speed: i32,
    music_volume: i32,
    sfx_volume: i32,
    voice_volume: i32,
    gamma: i32,
    texture_resolution: i32,
    particle_cap: i32,
    language_filter: bool,
    send_delay: bool,
    use_camera: bool,
    save_camera: bool,
    draw_anchor: bool,
    move_anchor: bool,
    alternate_mouse: bool,
    retaliation: bool,
    double_click_attack_move: bool,
    shadows_3d: bool,
    shadows_2d: bool,
    cloud_shadows: bool,
    ground_lighting: bool,
    smooth_water: bool,
    building_occlusion: bool,
    props: bool,
    extra_animations: bool,
    no_dynamic_lod: bool,
    unlock_fps: bool,
    heat_effects: bool,
    detail_level: DetailLevel,
    selected_resolution_index: i32,
    selected_anti_aliasing_index: i32,
}

impl OptionsMenuController {
    pub fn new() -> Self {
        let preferences = OptionPreferences::new();
        let mut controller = Self {
            snapshot: OptionMenuSnapshot {
                scroll_speed: 50,
                music_volume: 60,
                sfx_volume: 55,
                voice_volume: 70,
                gamma: 50,
                texture_resolution: 2,
                particle_cap: 5000,
                language_filter: true,
                send_delay: false,
                use_camera: true,
                save_camera: true,
                draw_anchor: true,
                move_anchor: false,
                alternate_mouse: false,
                retaliation: true,
                double_click_attack_move: false,
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
                detail_level: DetailLevel::Medium,
                selected_resolution_index: 0,
                selected_anti_aliasing_index: 0,
            },
            detail_level: DetailLevel::Medium,
            selected_resolution_index: 0,
            selected_anti_aliasing_index: 0,
            old_display_settings: DisplaySettings::default(),
            new_display_settings: DisplaySettings::default(),
            display_changed: false,
            ignore_selected: true,
            advanced_display_visible: false,
            scroll_speed: 50,
            music_volume: 60,
            sfx_volume: 55,
            voice_volume: 70,
            gamma: 50,
            texture_resolution: 2,
            particle_cap: 5000,
            language_filter: true,
            send_delay: false,
            use_camera: true,
            save_camera: true,
            draw_anchor: true,
            move_anchor: false,
            alternate_mouse: false,
            retaliation: true,
            double_click_attack_move: false,
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
            in_game: false,
            preferences,
        };
        controller.load_from_preferences();
        controller.snapshot = controller.take_snapshot();
        controller.ignore_selected = false;
        controller
    }

    fn take_snapshot(&self) -> OptionMenuSnapshot {
        OptionMenuSnapshot {
            scroll_speed: self.scroll_speed,
            music_volume: self.music_volume,
            sfx_volume: self.sfx_volume,
            voice_volume: self.voice_volume,
            gamma: self.gamma,
            texture_resolution: self.texture_resolution,
            particle_cap: self.particle_cap,
            language_filter: self.language_filter,
            send_delay: self.send_delay,
            use_camera: self.use_camera,
            save_camera: self.save_camera,
            draw_anchor: self.draw_anchor,
            move_anchor: self.move_anchor,
            alternate_mouse: self.alternate_mouse,
            retaliation: self.retaliation,
            double_click_attack_move: self.double_click_attack_move,
            shadows_3d: self.shadows_3d,
            shadows_2d: self.shadows_2d,
            cloud_shadows: self.cloud_shadows,
            ground_lighting: self.ground_lighting,
            smooth_water: self.smooth_water,
            building_occlusion: self.building_occlusion,
            props: self.props,
            extra_animations: self.extra_animations,
            no_dynamic_lod: self.no_dynamic_lod,
            unlock_fps: self.unlock_fps,
            heat_effects: self.heat_effects,
            detail_level: self.detail_level,
            selected_resolution_index: self.selected_resolution_index,
            selected_anti_aliasing_index: self.selected_anti_aliasing_index,
        }
    }

    fn restore_snapshot(&mut self) {
        self.scroll_speed = self.snapshot.scroll_speed;
        self.music_volume = self.snapshot.music_volume;
        self.sfx_volume = self.snapshot.sfx_volume;
        self.voice_volume = self.snapshot.voice_volume;
        self.gamma = self.snapshot.gamma;
        self.texture_resolution = self.snapshot.texture_resolution;
        self.particle_cap = self.snapshot.particle_cap;
        self.language_filter = self.snapshot.language_filter;
        self.send_delay = self.snapshot.send_delay;
        self.use_camera = self.snapshot.use_camera;
        self.save_camera = self.snapshot.save_camera;
        self.draw_anchor = self.snapshot.draw_anchor;
        self.move_anchor = self.snapshot.move_anchor;
        self.alternate_mouse = self.snapshot.alternate_mouse;
        self.retaliation = self.snapshot.retaliation;
        self.double_click_attack_move = self.snapshot.double_click_attack_move;
        self.shadows_3d = self.snapshot.shadows_3d;
        self.shadows_2d = self.snapshot.shadows_2d;
        self.cloud_shadows = self.snapshot.cloud_shadows;
        self.ground_lighting = self.snapshot.ground_lighting;
        self.smooth_water = self.snapshot.smooth_water;
        self.building_occlusion = self.snapshot.building_occlusion;
        self.props = self.snapshot.props;
        self.extra_animations = self.snapshot.extra_animations;
        self.no_dynamic_lod = self.snapshot.no_dynamic_lod;
        self.unlock_fps = self.snapshot.unlock_fps;
        self.heat_effects = self.snapshot.heat_effects;
        self.detail_level = self.snapshot.detail_level;
        self.selected_resolution_index = self.snapshot.selected_resolution_index;
        self.selected_anti_aliasing_index = self.snapshot.selected_anti_aliasing_index;
    }

    pub fn load_from_preferences(&mut self) {
        self.alternate_mouse = self.preferences.get_alternate_mouse_mode_enabled();
        self.retaliation = self.preferences.get_retaliation_mode_enabled();
        self.double_click_attack_move = self.preferences.get_double_click_attack_move_enabled();
        self.send_delay = self.preferences.get_send_delay();
        self.use_camera = self.preferences.use_camera_in_replays();
        self.save_camera = self.preferences.save_camera_in_replays();
        self.language_filter = self.preferences.get_language_filter();

        self.music_volume = self.preferences.get_music_volume() as i32;
        let max_volume = self
            .preferences
            .get_sound_volume()
            .max(self.preferences.get_3d_sound_volume());
        self.sfx_volume = max_volume as i32;
        self.voice_volume = self.preferences.get_speech_volume() as i32;

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

        self.scroll_speed = (self.preferences.get_scroll_factor() * 100.0) as i32;
        self.gamma = self.preferences.get_gamma_value() as i32;
        self.particle_cap = self.preferences.get_particle_cap() as u32;

        let texture_reduction = self.preferences.get_texture_reduction();
        if texture_reduction >= 0 {
            self.texture_resolution = 2 - texture_reduction;
        } else {
            self.texture_resolution = 2;
        }

        self.detail_level = self.preferences.get_static_game_detail();

        let (xres, yres) = self.preferences.get_resolution();
        self.selected_resolution_index = Self::find_resolution_index(xres, yres);

        self.selected_anti_aliasing_index =
            self.preferences.get_int_clamped("AntiAliasing", 0, 0, 2);
    }

    fn find_resolution_index(xres: i32, yres: i32) -> i32 {
        0
    }

    pub fn handle_button(&mut self, button: OptionsButtonId) -> OptionsMenuAction {
        if self.ignore_selected {
            return OptionsMenuAction::None;
        }

        match button {
            OptionsButtonId::Back => OptionsMenuAction::Close,
            OptionsButtonId::Accept => {
                self.save_options();
                OptionsMenuAction::Accept {
                    display_changed: self.display_changed,
                }
            }
            OptionsButtonId::Defaults => {
                self.set_defaults();
                OptionsMenuAction::None
            }
            OptionsButtonId::KeyboardOptions => {
                OptionsMenuAction::PushScreen("Menus/KeyboardOptionsMenu.wnd".to_string())
            }
            OptionsButtonId::AdvancedAccept => {
                self.advanced_display_visible = false;
                OptionsMenuAction::None
            }
            OptionsButtonId::AdvancedCancel => {
                self.cancel_advanced_options();
                OptionsMenuAction::None
            }
            OptionsButtonId::FirewallRefresh => {
                self.preferences.set("FirewallBehavior", "0".to_string());
                OptionsMenuAction::None
            }
        }
    }

    pub fn handle_checkbox(&mut self, checkbox: OptionsCheckboxId, checked: bool) {
        if self.ignore_selected {
            return;
        }

        match checkbox {
            OptionsCheckboxId::AlternateMouse => {
                self.alternate_mouse = checked;
            }
            OptionsCheckboxId::Retaliation => {
                self.retaliation = checked;
            }
            OptionsCheckboxId::DoubleClickAttackMove => {
                self.double_click_attack_move = checked;
            }
            OptionsCheckboxId::LanguageFilter => {
                self.language_filter = checked;
            }
            OptionsCheckboxId::SendDelay => {
                self.send_delay = checked;
            }
            OptionsCheckboxId::SaveCamera => {
                self.save_camera = checked;
            }
            OptionsCheckboxId::UseCamera => {
                self.use_camera = checked;
            }
            OptionsCheckboxId::DrawAnchor => {
                self.draw_anchor = checked;
            }
            OptionsCheckboxId::MoveAnchor => {
                self.move_anchor = checked;
            }
            OptionsCheckboxId::Shadows3D => {
                self.shadows_3d = checked;
            }
            OptionsCheckboxId::Shadows2D => {
                self.shadows_2d = checked;
            }
            OptionsCheckboxId::CloudShadows => {
                self.cloud_shadows = checked;
            }
            OptionsCheckboxId::GroundLighting => {
                self.ground_lighting = checked;
            }
            OptionsCheckboxId::SmoothWater => {
                self.smooth_water = checked;
            }
            OptionsCheckboxId::BuildingOcclusion => {
                self.building_occlusion = checked;
            }
            OptionsCheckboxId::ExtraAnimations => {
                self.extra_animations = checked;
            }
            OptionsCheckboxId::NoDynamicLod => {
                self.no_dynamic_lod = checked;
            }
            OptionsCheckboxId::UnlockFps => {
                self.unlock_fps = checked;
            }
            OptionsCheckboxId::HeatEffects => {
                self.heat_effects = checked;
            }
            OptionsCheckboxId::Props => {
                self.props = checked;
            }
        }
    }

    pub fn handle_slider(&mut self, slider: OptionsSliderId, value: i32) {
        if self.ignore_selected {
            return;
        }

        let clamped = value.max(0).min(100);
        match slider {
            OptionsSliderId::ScrollSpeed => {
                self.scroll_speed = clamped;
            }
            OptionsSliderId::MusicVolume => {
                self.music_volume = clamped;
            }
            OptionsSliderId::SFXVolume => {
                self.sfx_volume = clamped;
            }
            OptionsSliderId::VoiceVolume => {
                self.voice_volume = clamped;
            }
            OptionsSliderId::Gamma => {
                self.gamma = clamped;
            }
            OptionsSliderId::TextureResolution => {
                self.texture_resolution = clamped.max(0).min(2);
            }
            OptionsSliderId::ParticleCap => {
                self.particle_cap = (value as u32).max(100);
            }
        }
    }

    pub fn handle_combo_box(&mut self, combo: OptionsComboBoxId, index: i32) {
        if self.ignore_selected {
            return;
        }

        match combo {
            OptionsComboBoxId::Resolution => {
                self.selected_resolution_index = index;
            }
            OptionsComboBoxId::Detail => {
                let level = DetailLevel::from_index(index);
                if level == DetailLevel::Custom {
                    self.advanced_display_visible = true;
                }
                self.detail_level = level;
            }
            OptionsComboBoxId::AntiAliasing => {
                self.selected_anti_aliasing_index = index;
            }
            OptionsComboBoxId::LanIP => {
                self.selected_lan_ip_index(index);
            }
            OptionsComboBoxId::OnlineIP => {
                self.selected_online_ip_index(index);
            }
        }
    }

    fn selected_lan_ip_index(&mut self, _index: i32) {}

    fn selected_online_ip_index(&mut self, _index: i32) {}

    pub fn set_detail_level(&mut self, level: DetailLevel) {
        self.detail_level = level;
        if level == DetailLevel::Custom {
            self.advanced_display_visible = true;
        }
    }

    pub fn set_resolution_from_display_mode(
        &mut self,
        index: i32,
        xres: i32,
        yres: i32,
        bit_depth: i32,
        current_x: i32,
        current_y: i32,
    ) -> bool {
        self.selected_resolution_index = index;
        if current_x != xres || current_y != yres {
            self.old_display_settings = DisplaySettings {
                x_res: current_x,
                y_res: current_y,
                bit_depth: 32,
                windowed: false,
            };
            self.new_display_settings = DisplaySettings {
                x_res: xres,
                y_res: yres,
                bit_depth,
                windowed: false,
            };
            self.display_changed = true;
            return true;
        }
        false
    }

    fn set_defaults(&mut self) {
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

    fn cancel_advanced_options(&mut self) {
        self.detail_level = self.snapshot.detail_level;
        self.advanced_display_visible = false;
    }

    pub fn save_options(&mut self) {
        let prefs = &mut self.preferences;

        prefs.set(
            "LanguageFilter".to_string(),
            if self.language_filter {
                "true".to_string()
            } else {
                "false".to_string()
            },
        );

        prefs.set(
            "SendDelay".to_string(),
            if self.send_delay {
                "yes".to_string()
            } else {
                "no".to_string()
            },
        );

        if self.detail_level == DetailLevel::Custom {
            let texture_reduction = 2 - self.texture_resolution;
            prefs.set(
                "TextureReduction".to_string(),
                texture_reduction.to_string(),
            );

            prefs.set("UseShadowVolumes".to_string(), yes_no(self.shadows_3d));
            prefs.set("UseShadowDecals".to_string(), yes_no(self.shadows_2d));
            prefs.set("UseCloudMap".to_string(), yes_no(self.cloud_shadows));
            prefs.set("UseLightMap".to_string(), yes_no(self.ground_lighting));
            prefs.set("ShowSoftWaterEdge".to_string(), yes_no(self.smooth_water));
            prefs.set("ExtraAnimations".to_string(), yes_no(self.extra_animations));
            prefs.set("DynamicLOD".to_string(), yes_no(!self.no_dynamic_lod));
            prefs.set("HeatEffects".to_string(), yes_no(self.heat_effects));
            prefs.set(
                "BuildingOcclusion".to_string(),
                yes_no(self.building_occlusion),
            );
            prefs.set("ShowTrees".to_string(), yes_no(self.props));
            prefs.set(
                "MaxParticleCount".to_string(),
                self.particle_cap.to_string(),
            );
        }

        prefs.set(
            "StaticGameLOD".to_string(),
            self.detail_level.label().to_string(),
        );

        prefs.set(
            "Resolution".to_string(),
            format!(
                "{} {}",
                self.new_display_settings.x_res, self.new_display_settings.y_res
            ),
        );

        prefs.set(
            "UseAlternateMouse".to_string(),
            yes_no(self.alternate_mouse),
        );
        prefs.set("Retaliation".to_string(), yes_no(self.retaliation));
        prefs.set(
            "UseDoubleClickAttackMove".to_string(),
            yes_no(self.double_click_attack_move),
        );

        prefs.set("ScrollFactor".to_string(), self.scroll_speed.to_string());

        prefs.set("MusicVolume".to_string(), self.music_volume.to_string());

        let sfx_val = self.sfx_volume as f32 / 100.0;
        let relative_2d_volume: f32 = 0.0;
        let sound_2d_volume = if relative_2d_volume < 0.0 {
            sfx_val * (1.0 + relative_2d_volume)
        } else {
            sfx_val
        };
        let sound_3d_volume = if relative_2d_volume >= 0.0 {
            sfx_val * (1.0 - relative_2d_volume)
        } else {
            sfx_val
        };

        prefs.set("SFXVolume".to_string(), (sound_2d_volume * 100.0) as i32);
        prefs.set("SFX3DVolume".to_string(), (sound_3d_volume * 100.0) as i32);
        prefs.set("VoiceVolume".to_string(), self.voice_volume.to_string());

        prefs.set("Gamma".to_string(), self.gamma.to_string());

        prefs.set(
            "AntiAliasing".to_string(),
            self.selected_anti_aliasing_index.to_string(),
        );

        prefs.set("SaveCameraInReplays".to_string(), yes_no(self.save_camera));
        prefs.set("UseCameraInReplays".to_string(), yes_no(self.use_camera));

        prefs.set(
            "DrawScrollAnchor".to_string(),
            if self.draw_anchor {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        );
        prefs.set(
            "MoveScrollAnchor".to_string(),
            if self.move_anchor {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        );

        prefs.write();
    }

    pub fn slider_to_gamma(slider_val: i32) -> f32 {
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

    pub fn detail_level(&self) -> DetailLevel {
        self.detail_level
    }

    pub fn is_advanced_display_visible(&self) -> bool {
        self.advanced_display_visible
    }

    pub fn show_advanced_display(&mut self) {
        self.advanced_display_visible = true;
    }

    pub fn get_display_changed(&self) -> bool {
        self.display_changed
    }

    pub fn get_new_display_settings(&self) -> &DisplaySettings {
        &self.new_display_settings
    }

    pub fn get_music_volume(&self) -> i32 {
        self.music_volume
    }

    pub fn get_sfx_volume(&self) -> i32 {
        self.sfx_volume
    }

    pub fn get_voice_volume(&self) -> i32 {
        self.voice_volume
    }

    pub fn get_scroll_speed(&self) -> i32 {
        self.scroll_speed
    }

    pub fn get_gamma(&self) -> i32 {
        self.gamma
    }

    pub fn get_resolution_index(&self) -> i32 {
        self.selected_resolution_index
    }

    pub fn get_anti_aliasing_index(&self) -> i32 {
        self.selected_anti_aliasing_index
    }

    pub fn get_particle_cap(&self) -> u32 {
        self.particle_cap
    }

    pub fn get_texture_resolution(&self) -> i32 {
        self.texture_resolution
    }

    pub fn is_shadows_3d(&self) -> bool {
        self.shadows_3d
    }

    pub fn is_shadows_2d(&self) -> bool {
        self.shadows_2d
    }

    pub fn is_cloud_shadows(&self) -> bool {
        self.cloud_shadows
    }

    pub fn is_ground_lighting(&self) -> bool {
        self.ground_lighting
    }

    pub fn is_smooth_water(&self) -> bool {
        self.smooth_water
    }

    pub fn is_building_occlusion(&self) -> bool {
        self.building_occlusion
    }

    pub fn is_extra_animations(&self) -> bool {
        self.extra_animations
    }

    pub fn is_no_dynamic_lod(&self) -> bool {
        self.no_dynamic_lod
    }

    pub fn is_unlock_fps(&self) -> bool {
        self.unlock_fps
    }

    pub fn is_heat_effects(&self) -> bool {
        self.heat_effects
    }

    pub fn is_props(&self) -> bool {
        self.props
    }

    pub fn is_alternate_mouse(&self) -> bool {
        self.alternate_mouse
    }

    pub fn is_retaliation(&self) -> bool {
        self.retaliation
    }

    pub fn is_double_click_attack_move(&self) -> bool {
        self.double_click_attack_move
    }

    pub fn is_language_filter(&self) -> bool {
        self.language_filter
    }

    pub fn is_send_delay(&self) -> bool {
        self.send_delay
    }

    pub fn is_save_camera(&self) -> bool {
        self.save_camera
    }

    pub fn is_use_camera(&self) -> bool {
        self.use_camera
    }

    pub fn is_draw_anchor(&self) -> bool {
        self.draw_anchor
    }

    pub fn is_move_anchor(&self) -> bool {
        self.move_anchor
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptionsMenuAction {
    None,
    Close,
    Accept { display_changed: bool },
    PushScreen(String),
}

fn yes_no(value: bool) -> String {
    if value {
        "yes".to_string()
    } else {
        "no".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_levels_are_clamped() {
        let mut options = OptionsMenuPort::sample();
        options.set_audio_levels(1.4, -0.2, 0.5);

        assert_eq!(options.music_volume, 1.0);
        assert_eq!(options.sfx_volume, 0.0);
        assert_eq!(options.voice_volume, 0.5);
    }

    #[test]
    fn resolution_is_updated_explicitly() {
        let mut options = OptionsMenuPort::sample();
        options.set_resolution(1600, 900);

        assert_eq!(options.resolution, (1600, 900));
    }

    #[test]
    fn controller_loads_preferences() {
        let controller = OptionsMenuController::new();
        assert_eq!(controller.get_music_volume(), 60);
        assert_eq!(controller.get_gamma(), 50);
        assert!(controller.is_retaliation());
        assert!(!controller.is_alternate_mouse());
    }

    #[test]
    fn controller_defaults_restores_values() {
        let mut controller = OptionsMenuController::new();
        controller.set_defaults();

        assert_eq!(controller.get_scroll_speed(), 50);
        assert_eq!(controller.get_music_volume(), 60);
        assert_eq!(controller.get_gamma(), 50);
        assert!(controller.is_retaliation());
        assert!(!controller.is_alternate_mouse());
        assert!(controller.is_language_filter());
    }

    #[test]
    fn gamma_slider_conversion() {
        assert!((OptionsMenuController::slider_to_gamma(0) - 0.6).abs() < f32::EPSILON);
        assert!((OptionsMenuController::slider_to_gamma(50) - 1.0).abs() < f32::EPSILON);
        assert!((OptionsMenuController::slider_to_gamma(100) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn handle_button_accept_saves() {
        let mut controller = OptionsMenuController::new();
        controller.handle_button(OptionsButtonId::Accept);
    }

    #[test]
    fn handle_button_back_returns_close() {
        let mut controller = OptionsMenuController::new();
        let action = controller.handle_button(OptionsButtonId::Back);
        assert_eq!(action, OptionsMenuAction::Close);
    }

    #[test]
    fn handle_button_defaults_resets() {
        let mut controller = OptionsMenuController::new();
        controller.music_volume = 100;
        controller.handle_button(OptionsButtonId::Defaults);
        assert_eq!(controller.get_music_volume(), 60);
    }

    #[test]
    fn handle_checkbox_updates_state() {
        let mut controller = OptionsMenuController::new();
        controller.handle_checkbox(OptionsCheckboxId::AlternateMouse, true);
        assert!(controller.is_alternate_mouse());

        controller.handle_checkbox(OptionsCheckboxId::LanguageFilter, false);
        assert!(!controller.is_language_filter());
    }

    #[test]
    fn handle_slider_clamps_values() {
        let mut controller = OptionsMenuController::new();
        controller.handle_slider(OptionsSliderId::MusicVolume, 150);
        assert_eq!(controller.get_music_volume(), 100);

        controller.handle_slider(OptionsSliderId::MusicVolume, -10);
        assert_eq!(controller.get_music_volume(), 0);
    }

    #[test]
    fn handle_slider_particle_cap_minimum() {
        let mut controller = OptionsMenuController::new();
        controller.handle_slider(OptionsSliderId::ParticleCap, 50);
        assert_eq!(controller.get_particle_cap(), 100);
    }

    #[test]
    fn handle_combo_detail_custom_shows_advanced() {
        let mut controller = OptionsMenuController::new();
        controller.handle_combo_box(OptionsComboBoxId::Detail, CUSTOMDETAIL);
        assert!(controller.is_advanced_display_visible());
    }

    #[test]
    fn cancel_advanced_restores_detail() {
        let mut controller = OptionsMenuController::new();
        controller.handle_combo_box(OptionsComboBoxId::Detail, CUSTOMDETAIL);
        assert!(controller.is_advanced_display_visible());

        controller.handle_button(OptionsButtonId::AdvancedCancel);
        assert!(!controller.is_advanced_display_visible());
        assert_eq!(controller.detail_level(), DetailLevel::Medium);
    }

    #[test]
    fn resolution_change_tracks_display_changed() {
        let mut controller = OptionsMenuController::new();
        let changed = controller.set_resolution_from_display_mode(0, 1920, 1080, 32, 800, 600);
        assert!(changed);
        assert!(controller.get_display_changed());

        let settings = controller.get_new_display_settings();
        assert_eq!(settings.x_res, 1920);
        assert_eq!(settings.y_res, 1080);
    }

    #[test]
    fn same_resolution_no_display_changed() {
        let mut controller = OptionsMenuController::new();
        let changed = controller.set_resolution_from_display_mode(0, 800, 600, 32, 800, 600);
        assert!(!changed);
        assert!(!controller.get_display_changed());
    }

    #[test]
    fn preferences_bool_yes_parsing() {
        let mut prefs = OptionPreferences::default();
        assert_eq!(prefs.get_bool_yes("Missing", true), true);
        assert_eq!(prefs.get_bool_yes("Missing", false), false);

        prefs
            .preferences
            .insert("Key1".to_string(), "yes".to_string());
        assert!(prefs.get_bool_yes("Key1", false));

        prefs
            .preferences
            .insert("Key2".to_string(), "YES".to_string());
        assert!(prefs.get_bool_yes("Key2", false));

        prefs
            .preferences
            .insert("Key3".to_string(), "no".to_string());
        assert!(!prefs.get_bool_yes("Key3", true));
    }

    #[test]
    fn preferences_resolution_parsing() {
        let mut prefs = OptionPreferences::default();
        assert_eq!(prefs.get_resolution(), (800, 600));

        prefs
            .preferences
            .insert("Resolution".to_string(), "1920 1080".to_string());
        assert_eq!(prefs.get_resolution(), (1920, 1080));

        prefs
            .preferences
            .insert("Resolution".to_string(), "invalid".to_string());
        assert_eq!(prefs.get_resolution(), (800, 600));
    }

    #[test]
    fn detail_level_round_trip() {
        assert_eq!(DetailLevel::from_index(0), DetailLevel::High);
        assert_eq!(DetailLevel::from_index(1), DetailLevel::Medium);
        assert_eq!(DetailLevel::from_index(2), DetailLevel::Low);
        assert_eq!(DetailLevel::from_index(3), DetailLevel::Custom);
        assert_eq!(DetailLevel::from_index(99), DetailLevel::Medium);

        assert_eq!(DetailLevel::High.to_index(), 0);
        assert_eq!(DetailLevel::Custom.to_index(), 3);
    }

    #[test]
    fn yes_no_helper() {
        assert_eq!(yes_no(true), "yes");
        assert_eq!(yes_no(false), "no");
    }
}
