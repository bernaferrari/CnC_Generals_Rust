use crate::gui::callbacks::menus::main_menu::GameDifficultyPort;
use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

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
}
