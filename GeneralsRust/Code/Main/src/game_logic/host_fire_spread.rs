//! Host FireSpreadUpdate + FlammableUpdate residual (tree/shrubbery fire chain).
//!
//! C++:
//! - `FlammableUpdate` manages AFLAME / BURNED and `tryToIgnite` / `wouldIgnite`
//! - `FireSpreadUpdate` while AFLAME: spawn OCL embers + ignite closest flammable
//!   in `SpreadTryRange`, then sleep random `[MinSpreadDelay, MaxSpreadDelay]`
//!
//! Retail peels (`NatureProp.ini` Dogwood tree):
//! - FlameDamageLimit **2**, BurnedDelay **2500**ms → **75**f
//! - AflameDuration **3500**ms → **105**f
//! - MinSpreadDelay **1000**ms → **30**f, MaxSpreadDelay **2000**ms → **60**f
//! - SpreadTryRange **50**
//! - OCLEmbers `OCL_BurningEmbers` (honesty name only; full OCL fail-closed)
//!
//! Fail-closed: not full PartitionManager 3D closest / FireWeaponCollide AFLAME
//! weapon / highlander body kill immunity / particle bones.

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const FIRE_SPREAD_LOGIC_FPS: f32 = 30.0;

pub fn fire_spread_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * FIRE_SPREAD_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail tree FlammableUpdate peels.
pub const TREE_FLAME_DAMAGE_LIMIT: f32 = 2.0;
pub const TREE_BURNED_DELAY_MS: u32 = 2_500;
pub const TREE_BURNED_DELAY_FRAMES: u32 = 75; // 2500ms
pub const TREE_AFLAME_DURATION_MS: u32 = 3_500;
pub const TREE_AFLAME_DURATION_FRAMES: u32 = 105; // 3500ms
pub const TREE_BURNING_SOUND: &str = "GenericFireMediumLoop";

/// Retail FireSpreadUpdate peels.
pub const TREE_MIN_SPREAD_DELAY_MS: u32 = 1_000;
pub const TREE_MIN_SPREAD_DELAY_FRAMES: u32 = 30;
pub const TREE_MAX_SPREAD_DELAY_MS: u32 = 2_000;
pub const TREE_MAX_SPREAD_DELAY_FRAMES: u32 = 60;
pub const TREE_SPREAD_TRY_RANGE: f32 = 50.0;
pub const TREE_OCL_EMBERS: &str = "OCL_BurningEmbers";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostFlammableState {
    Normal,
    Aflame,
    Burned,
}

impl Default for HostFlammableState {
    fn default() -> Self {
        Self::Normal
    }
}

/// Combined FlammableUpdate + FireSpreadUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFireSpreadData {
    pub state: HostFlammableState,
    pub flame_damage_accum: f32,
    pub flame_damage_limit: f32,
    pub aflame_end_frame: u32,
    pub burned_end_frame: u32,
    pub next_spread_frame: u32,
    pub min_spread_delay: u32,
    pub max_spread_delay: u32,
    pub spread_try_range: f32,
    pub aflame_duration: u32,
    pub burned_delay: u32,
    pub active: bool,
    pub spread_enabled: bool,
}

impl Default for HostFireSpreadData {
    fn default() -> Self {
        Self::tree_default()
    }
}

impl HostFireSpreadData {
    pub fn tree_default() -> Self {
        Self {
            state: HostFlammableState::Normal,
            flame_damage_accum: 0.0,
            flame_damage_limit: TREE_FLAME_DAMAGE_LIMIT,
            aflame_end_frame: 0,
            burned_end_frame: 0,
            next_spread_frame: u32::MAX,
            min_spread_delay: TREE_MIN_SPREAD_DELAY_FRAMES,
            max_spread_delay: TREE_MAX_SPREAD_DELAY_FRAMES,
            spread_try_range: TREE_SPREAD_TRY_RANGE,
            aflame_duration: TREE_AFLAME_DURATION_FRAMES,
            burned_delay: TREE_BURNED_DELAY_FRAMES,
            active: true,
            spread_enabled: true,
        }
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_fire_spread_template(template_name) {
            Some(Self::tree_default())
        } else {
            None
        }
    }

    /// C++ `wouldIgnite`: normal and not already aflame/burned.
    pub fn would_ignite(&self) -> bool {
        self.active && matches!(self.state, HostFlammableState::Normal)
    }

    /// C++ `tryToIgnite` residual.
    pub fn try_to_ignite(&mut self, current_frame: u32) -> bool {
        if !self.would_ignite() {
            return false;
        }
        self.state = HostFlammableState::Aflame;
        self.aflame_end_frame = current_frame.saturating_add(self.aflame_duration);
        self.burned_end_frame = current_frame.saturating_add(self.burned_delay.max(self.aflame_duration));
        // C++ startFireSpreading → wake with next delay.
        self.next_spread_frame = current_frame.saturating_add(self.calc_next_spread_delay(current_frame));
        true
    }

    /// Accumulate flame damage residual (limit → ignite).
    pub fn apply_flame_damage(&mut self, amount: f32, current_frame: u32) -> bool {
        if matches!(self.state, HostFlammableState::Burned | HostFlammableState::Aflame) {
            return false;
        }
        self.flame_damage_accum += amount.max(0.0);
        if self.flame_damage_accum >= self.flame_damage_limit {
            return self.try_to_ignite(current_frame);
        }
        false
    }

    /// Deterministic delay residual (midpoint of min/max; C++ uses RNG).
    pub fn calc_next_spread_delay(&self, _salt: u32) -> u32 {
        let lo = self.min_spread_delay.max(1);
        let hi = self.max_spread_delay.max(lo);
        // Midpoint residual (deterministic; RNG deferred).
        ((lo as u64 + hi as u64) / 2).max(1) as u32
    }

    pub fn is_aflame(&self) -> bool {
        matches!(self.state, HostFlammableState::Aflame)
    }

    /// Per-frame flammable status progression.
    pub fn tick_flammable(&mut self, current_frame: u32) -> FlammableTickResult {
        let mut r = FlammableTickResult::default();
        match self.state {
            HostFlammableState::Normal => {}
            HostFlammableState::Aflame => {
                r.aflame = true;
                if current_frame >= self.aflame_end_frame {
                    self.state = HostFlammableState::Burned;
                    r.became_burned = true;
                }
            }
            HostFlammableState::Burned => {
                r.burned = true;
            }
        }
        r
    }

    /// C++ FireSpreadUpdate::update while AFLAME.
    /// Returns true when a spread attempt should run (caller ignites closest).
    pub fn tick_spread(&mut self, current_frame: u32) -> SpreadTickResult {
        let mut r = SpreadTickResult::default();
        if !self.spread_enabled || !self.is_aflame() {
            return r;
        }
        if current_frame < self.next_spread_frame {
            return r;
        }
        r.try_spread = true;
        r.spawn_embers = true;
        self.next_spread_frame =
            current_frame.saturating_add(self.calc_next_spread_delay(current_frame));
        r
    }
}

#[derive(Debug, Clone, Default)]
pub struct FlammableTickResult {
    pub aflame: bool,
    pub burned: bool,
    pub became_burned: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadTickResult {
    pub try_spread: bool,
    pub spawn_embers: bool,
}

/// Nature prop / shrubbery fire-spread templates.
pub fn is_fire_spread_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("dogwood")
        || n.contains("tree") && (n.contains("pt") || n.contains("shrub") || n.contains("pine") || n.contains("oak"))
        || n.contains("shrubbery")
        || n.ends_with("tree")
        || n.contains("burnabletree")
}

/// Host residual registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFireSpreadRegistry {
    pub installed: u32,
    pub ignitions: u32,
    pub spreads: u32,
    pub embers: u32,
    pub burned: u32,
}

impl HostFireSpreadRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_ignition(&mut self) {
        self.ignitions = self.ignitions.saturating_add(1);
    }
    pub fn record_spread(&mut self) {
        self.spreads = self.spreads.saturating_add(1);
    }
    pub fn record_embers(&mut self) {
        self.embers = self.embers.saturating_add(1);
    }
    pub fn record_burned(&mut self) {
        self.burned = self.burned.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.ignitions > 0 || self.spreads > 0
    }
}

pub fn honesty_fire_spread_residual_ok() -> bool {
    fire_spread_ms_to_frames(TREE_MIN_SPREAD_DELAY_MS) == TREE_MIN_SPREAD_DELAY_FRAMES
        && fire_spread_ms_to_frames(TREE_MAX_SPREAD_DELAY_MS) == TREE_MAX_SPREAD_DELAY_FRAMES
        && fire_spread_ms_to_frames(TREE_AFLAME_DURATION_MS) == TREE_AFLAME_DURATION_FRAMES
        && fire_spread_ms_to_frames(TREE_BURNED_DELAY_MS) == TREE_BURNED_DELAY_FRAMES
        && TREE_SPREAD_TRY_RANGE == 50.0
        && TREE_FLAME_DAMAGE_LIMIT == 2.0
        && TREE_OCL_EMBERS == "OCL_BurningEmbers"
        && is_fire_spread_template("DogwoodTree")
        && is_fire_spread_template("PTDogwood01")
        && !is_fire_spread_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_fire_spread_residual_ok());
    }

    #[test]
    fn flame_damage_ignites_and_spreads() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.would_ignite());
        assert!(!d.apply_flame_damage(1.0, 0));
        assert!(d.apply_flame_damage(1.0, 0)); // limit 2
        assert!(d.is_aflame());
        assert!(!d.would_ignite());
        // Force spread due.
        d.next_spread_frame = 10;
        let s = d.tick_spread(10);
        assert!(s.try_spread);
        assert!(s.spawn_embers);
        assert!(d.next_spread_frame > 10);
    }

    #[test]
    fn aflame_expires_to_burned() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        d.aflame_end_frame = 5;
        let r = d.tick_flammable(5);
        assert!(r.became_burned);
        assert!(matches!(d.state, HostFlammableState::Burned));
    }
}
