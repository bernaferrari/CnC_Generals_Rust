use crate::{ObjectId, ObjectiveDisplay, Team};
use serde::{Deserialize, Serialize};

const fn default_presentation_alliance_team() -> i32 {
    -1
}

/// Frozen player roster residual for defeat/alliance UI and radar team identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPlayerInfo {
    pub id: u32,
    pub name: String,
    pub team: Team,
    #[serde(default = "default_presentation_alliance_team")]
    pub alliance_team: i32,
    pub is_alive: bool,
    pub is_local: bool,
    pub is_ai: bool,
    pub color_rgb: (u8, u8, u8),
}

/// Frozen script popup residual (C++ ScriptPopupMessageRequest parity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationPopupMessage {
    pub message: String,
    pub x_percent: i32,
    pub y_percent: i32,
    pub width: i32,
    pub pause: bool,
    pub pause_music: bool,
}

/// Frozen InGameUI PublicTimer superweapon countdown residual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSuperweaponTimer {
    pub name: String,
    pub template_name: String,
    pub icon: String,
    pub recharge_time: f32,
    pub remaining: f32,
    pub unlocked: bool,
    pub ready: bool,
    pub power_key: String,
}

/// A focused borrowed HUD read view over a completed presentation frame.
/// The source `PresentationFrame` stays flat to preserve its existing serde layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationHudFrame<'a> {
    pub total_play_time_seconds: f32,
    pub local_player_id: u32,
    pub local_team: Team,
    pub players: &'a [PresentationPlayerInfo],
    pub selected: &'a [ObjectId],
    pub local_supplies: u32,
    pub local_power: i32,
    pub local_power_produced: i32,
    pub local_power_consumed: i32,
    pub local_color_rgb: (u8, u8, u8),
    pub local_is_alive: bool,
    pub local_radar_count: i32,
    pub local_radar_disabled: bool,
    pub radar_ui_enabled: bool,
    pub radar_forced: bool,
    pub superweapon_timers: &'a [PresentationSuperweaponTimer],
    pub objectives: &'a [ObjectiveDisplay],
    pub pending_popup_messages: &'a [PresentationPopupMessage],
}

impl<'a> PresentationHudFrame<'a> {
    /// Look up a roster entry frozen for this presentation frame.
    #[inline]
    pub fn player_info(&self, id: u32) -> Option<&'a PresentationPlayerInfo> {
        self.players.iter().find(|player| player.id == id)
    }
}
