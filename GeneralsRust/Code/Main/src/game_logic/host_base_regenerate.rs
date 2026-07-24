//! Host BaseRegenerateUpdate residual (structure auto-heal after combat).
//!
//! C++: `BaseRegenerateUpdate` (source file misspelled `BaseRenerateUpdate.cpp`)
//! - Sleep forever while at full health or sold
//! - On non-healing damage: wake after `BaseRegenDelay`
//! - Heal every **3** frames: `3 * maxHealth * percentPerSecond / 30`
//! - Skip while `OBJECT_STATUS_UNDER_CONSTRUCTION`
//! - Also processes while `DISABLED_UNDERPOWERED`
//!
//! Retail `GameData.ini`:
//! - `BaseRegenHealthPercentPerSecond = 0.3%` → **0.003** of max health / second
//! - `BaseRegenDelay = 3000` ms → **90** frames
//!
//! Module is attached to faction / tech structures with empty INI body.
//!
//! Fail-closed: not full DamageModule onHealing / underpowered disable mask
//! integration beyond allowing heal while underpowered / GlobalData runtime override.

use serde::{Deserialize, Serialize};

pub const BASE_REGEN_LOGIC_FPS: f32 = 30.0;

/// Retail BaseRegenHealthPercentPerSecond = 0.3% → fraction of max health per second.
pub const BASE_REGEN_HEALTH_PERCENT_PER_SECOND: f32 = 0.003;
/// Retail BaseRegenDelay = 3000 ms.
pub const BASE_REGEN_DELAY_MS: u32 = 3_000;
pub const BASE_REGEN_DELAY_FRAMES: u32 = 90;
/// C++ HEAL_RATE residual.
pub const BASE_REGEN_HEAL_RATE_FRAMES: u32 = 3;

pub fn base_regen_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * BASE_REGEN_LOGIC_FPS / 1000.0).round() as u32
}

/// Heal amount applied every HEAL_RATE frames.
pub fn base_regen_heal_amount(max_health: f32) -> f32 {
    BASE_REGEN_HEAL_RATE_FRAMES as f32 * (max_health * BASE_REGEN_HEALTH_PERCENT_PER_SECOND)
        / BASE_REGEN_LOGIC_FPS
}

/// Per-object BaseRegenerateUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBaseRegenerateData {
    /// Frame when regen may resume after last non-heal damage.
    pub wake_frame: u32,
    /// Active residual (percent > 0).
    pub active: bool,
    pub done_sold: bool,
    /// Set by Object damage path; consumed on next regen tick.
    pub pending_damage: bool,
}

impl Default for HostBaseRegenerateData {
    fn default() -> Self {
        Self {
            // C++ ctor: if percent > 0 wake immediately (UPDATE_SLEEP_NONE).
            wake_frame: 0,
            active: BASE_REGEN_HEALTH_PERCENT_PER_SECOND > 0.0,
            done_sold: false,
            pending_damage: false,
        }
    }
}

impl HostBaseRegenerateData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_structure_template(template_name: &str, is_structure: bool) -> Option<Self> {
        if is_structure || is_base_regenerate_template(template_name) {
            Some(Self::new())
        } else {
            None
        }
    }

    /// C++ onDamage: non-healing damage → sleep BaseRegenDelay.
    pub fn on_damage(&mut self, current_frame: u32, is_healing_damage: bool) {
        if !self.active || is_healing_damage {
            return;
        }
        self.wake_frame = current_frame.saturating_add(BASE_REGEN_DELAY_FRAMES);
        self.pending_damage = false;
    }

    pub fn mark_damaged(&mut self) {
        if self.active {
            self.pending_damage = true;
        }
    }

    /// Returns heal amount this frame, or 0 if sleeping / full / sold / constructing.
    pub fn tick_heal_amount(
        &mut self,
        current_frame: u32,
        current_health: f32,
        max_health: f32,
        under_construction: bool,
        sold: bool,
    ) -> f32 {
        if !self.active || self.done_sold {
            return 0.0;
        }
        if self.pending_damage {
            self.on_damage(current_frame, false);
        }
        if sold {
            self.done_sold = true;
            return 0.0;
        }
        if under_construction {
            return 0.0;
        }
        if max_health <= 0.0 || current_health >= max_health - f32::EPSILON {
            // Sleep until damaged again (wake_frame stays; on_damage reopens).
            return 0.0;
        }
        if current_frame < self.wake_frame {
            return 0.0;
        }
        // Heal only every HEAL_RATE frames after wake (align to wake).
        let elapsed = current_frame.saturating_sub(self.wake_frame);
        if elapsed % BASE_REGEN_HEAL_RATE_FRAMES != 0 {
            return 0.0;
        }
        base_regen_heal_amount(max_health).max(0.0)
    }
}

/// Explicit template peels that always get the module even if KindOf missing in tests.
pub fn is_base_regenerate_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("commandcenter")
        || n.contains("warfactory")
        || n.contains("barracks")
        || n.contains("supplycenter")
        || n.contains("powerplant")
        || n.contains("airfield")
        || n.contains("strategycenter")
        || n.contains("techhospital")
        || n.contains("techoil")
        || n.contains("techartillery")
        || n.contains("techrepair")
        || n.contains("stargate")
        || n.contains("scudstorm")
        || n.contains("particlecannon")
        || n.contains("nuclearmissile")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBaseRegenerateRegistry {
    pub installed: u32,
    pub damage_delays: u32,
    pub heal_ticks: u32,
    pub total_healed: f32,
}

impl HostBaseRegenerateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_damage_delay(&mut self) {
        self.damage_delays = self.damage_delays.saturating_add(1);
    }
    pub fn record_heal(&mut self, amount: f32) {
        self.heal_ticks = self.heal_ticks.saturating_add(1);
        self.total_healed += amount;
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.heal_ticks > 0 || self.damage_delays > 0
    }
}

pub fn honesty_base_regenerate_residual_ok() -> bool {
    base_regen_ms_to_frames(BASE_REGEN_DELAY_MS) == BASE_REGEN_DELAY_FRAMES
        && BASE_REGEN_HEAL_RATE_FRAMES == 3
        && (BASE_REGEN_HEALTH_PERCENT_PER_SECOND - 0.003).abs() < 1.0e-6
        && {
            let amt = base_regen_heal_amount(1000.0);
            // 3 * 1000 * 0.003 / 30 = 0.3
            (amt - 0.3).abs() < 1.0e-5
        }
        && is_base_regenerate_template("AmericaCommandCenter")
        && !is_base_regenerate_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_heal_math() {
        assert!(honesty_base_regenerate_residual_ok());
    }

    #[test]
    fn damage_delays_then_heals() {
        let mut d = HostBaseRegenerateData::new();
        d.on_damage(10, false);
        assert_eq!(d.wake_frame, 10 + BASE_REGEN_DELAY_FRAMES);
        // Before wake: no heal.
        assert_eq!(
            d.tick_heal_amount(10 + BASE_REGEN_DELAY_FRAMES - 1, 500.0, 1000.0, false, false),
            0.0
        );
        // At wake: heal.
        let amt = d.tick_heal_amount(10 + BASE_REGEN_DELAY_FRAMES, 500.0, 1000.0, false, false);
        assert!((amt - 0.3).abs() < 1.0e-5);
        // Full health: no heal.
        assert_eq!(
            d.tick_heal_amount(10 + BASE_REGEN_DELAY_FRAMES, 1000.0, 1000.0, false, false),
            0.0
        );
        // Construction blocks.
        assert_eq!(
            d.tick_heal_amount(10 + BASE_REGEN_DELAY_FRAMES, 500.0, 1000.0, true, false),
            0.0
        );
        // Sold forever.
        assert_eq!(
            d.tick_heal_amount(10 + BASE_REGEN_DELAY_FRAMES, 500.0, 1000.0, false, true),
            0.0
        );
        assert!(d.done_sold);
    }
}
