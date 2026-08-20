//! Host FireSpreadUpdate + FlammableUpdate (trees and flammable buildings).
//!
//! C++:
//! - `FlammableUpdate` manages AFLAME / BURNED and `tryToIgnite` / `wouldIgnite`
//! - `FireSpreadUpdate` while AFLAME: spawn OCL embers + ignite closest flammable
//!   in `SpreadTryRange`, then sleep random `[MinSpreadDelay, MaxSpreadDelay]`
//!
//! Retail peels (`NatureProp.ini` Dogwood tree):
//! - FlameDamageLimit **2**, FlameDamageExpiration **2000**ms → **60**f
//! - BurnedDelay **2500**ms → **75**f, AflameDuration **3500**ms → **105**f
//! - MinSpreadDelay **1000**ms → **30**f, MaxSpreadDelay **2000**ms → **60**f
//! - SpreadTryRange **50**, OCLEmbers `OCL_BurningEmbers`
//!
//! Buildings with FlammableUpdate (oil derrick / black market / PUC) ignite
//! but do not carry FireSpreadUpdate unless the template is a tree.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const FIRE_SPREAD_LOGIC_FPS: f32 = 30.0;

pub fn fire_spread_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * FIRE_SPREAD_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail tree FlammableUpdate peels.
pub const TREE_FLAME_DAMAGE_LIMIT: f32 = 2.0;
/// C++ default FlameDamageExpiration = 2s → 60f (`LOGICFRAMES_PER_SECOND * 2`).
pub const TREE_FLAME_DAMAGE_EXPIRATION_MS: u32 = 2_000;
pub const TREE_FLAME_DAMAGE_EXPIRATION_FRAMES: u32 = 60;
pub const TREE_BURNED_DELAY_MS: u32 = 2_500;
pub const TREE_BURNED_DELAY_FRAMES: u32 = 75; // 2500ms
pub const TREE_AFLAME_DURATION_MS: u32 = 3_500;
pub const TREE_AFLAME_DURATION_FRAMES: u32 = 105; // 3500ms
/// Retail tree FlammableUpdate AflameDamageAmount residual.
pub const TREE_AFLAME_DAMAGE_AMOUNT: f32 = 5.0;
/// Retail tree FlammableUpdate AflameDamageDelay residual (msec).
pub const TREE_AFLAME_DAMAGE_DELAY_MS: u32 = 500;
/// AflameDamageDelay 500ms → 15 frames @ 30 FPS.
pub const TREE_AFLAME_DAMAGE_DELAY_FRAMES: u32 = 15;
pub const TREE_BURNING_SOUND: &str = "GenericFireMediumLoop";
/// C++ GameData AutoAflameParticleSystem residual name.
pub const AUTO_AFLAME_PARTICLE: &str = "AutoAflame";

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
    /// Remaining flame threshold (C++ `m_flameDamageLimit`).
    pub flame_damage_limit: f32,
    /// Authored FlameDamageLimit (C++ `m_flameDamageLimitData`).
    #[serde(default = "default_flame_damage_limit_data")]
    pub flame_damage_limit_data: f32,
    /// FlameDamageExpiration in frames (C++ default 60).
    #[serde(default = "default_flame_damage_expiration")]
    pub flame_damage_expiration: u32,
    /// C++ `m_lastFlameDamageDealt`.
    #[serde(default)]
    pub last_flame_damage_dealt: u32,
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
    /// C++ FlammableUpdateModuleData::m_aflameDamageAmount.
    #[serde(default = "default_aflame_damage_amount")]
    pub aflame_damage_amount: f32,
    /// C++ FlammableUpdateModuleData::m_aflameDamageDelay (frames).
    #[serde(default = "default_aflame_damage_delay")]
    pub aflame_damage_delay: u32,
    /// C++ FlammableUpdate::m_damageEndFrame.
    #[serde(default)]
    pub damage_end_frame: u32,
    /// C++ BurningSoundName / startBurningSound.
    #[serde(default = "default_burning_sound_name")]
    pub burning_sound_name: String,
    /// C++ FireSpreadUpdateModuleData::m_oclEmbers.
    #[serde(default = "default_ocl_embers")]
    pub ocl_embers: String,
    /// OBJECT_STATUS_BURNED + MODELCONDITION_SMOLDERING while still AFLAME.
    #[serde(default)]
    pub smoldering: bool,
    /// C++ BodyModule::setAflame.
    #[serde(default)]
    pub body_aflame: bool,
    /// C++ `m_audioHandle` live: loop already started.
    #[serde(default)]
    pub burning_sound_active: bool,
}

fn default_aflame_damage_amount() -> f32 {
    TREE_AFLAME_DAMAGE_AMOUNT
}

fn default_aflame_damage_delay() -> u32 {
    TREE_AFLAME_DAMAGE_DELAY_FRAMES
}

fn default_flame_damage_limit_data() -> f32 {
    TREE_FLAME_DAMAGE_LIMIT
}

fn default_flame_damage_expiration() -> u32 {
    TREE_FLAME_DAMAGE_EXPIRATION_FRAMES
}

fn default_burning_sound_name() -> String {
    TREE_BURNING_SOUND.to_string()
}

fn default_ocl_embers() -> String {
    TREE_OCL_EMBERS.to_string()
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
            flame_damage_limit_data: TREE_FLAME_DAMAGE_LIMIT,
            flame_damage_expiration: TREE_FLAME_DAMAGE_EXPIRATION_FRAMES,
            last_flame_damage_dealt: 0,
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
            aflame_damage_amount: TREE_AFLAME_DAMAGE_AMOUNT,
            aflame_damage_delay: TREE_AFLAME_DAMAGE_DELAY_FRAMES,
            damage_end_frame: 0,
            burning_sound_name: TREE_BURNING_SOUND.to_string(),
            ocl_embers: TREE_OCL_EMBERS.to_string(),
            smoldering: false,
            body_aflame: false,
            burning_sound_active: false,
        }
    }

    /// Oil derrick FlammableUpdate residual (no FireSpreadUpdate).
    pub fn oil_derrick_default() -> Self {
        use crate::game_logic::host_oil_derrick::{
            OIL_DERRICK_AFLAME_DAMAGE_AMOUNT, OIL_DERRICK_AFLAME_DAMAGE_DELAY_FRAMES,
            OIL_DERRICK_AFLAME_DURATION_FRAMES, OIL_DERRICK_FLAME_DAMAGE_EXPIRATION_FRAMES,
            OIL_DERRICK_FLAME_DAMAGE_LIMIT,
        };
        Self {
            flame_damage_limit: OIL_DERRICK_FLAME_DAMAGE_LIMIT,
            flame_damage_limit_data: OIL_DERRICK_FLAME_DAMAGE_LIMIT,
            flame_damage_expiration: OIL_DERRICK_FLAME_DAMAGE_EXPIRATION_FRAMES,
            aflame_duration: OIL_DERRICK_AFLAME_DURATION_FRAMES,
            burned_delay: 0,
            spread_enabled: false,
            spread_try_range: 0.0,
            ocl_embers: String::new(),
            aflame_damage_amount: OIL_DERRICK_AFLAME_DAMAGE_AMOUNT,
            aflame_damage_delay: OIL_DERRICK_AFLAME_DAMAGE_DELAY_FRAMES,
            ..Self::tree_default()
        }
    }

    /// Black market FlammableUpdate residual (no FireSpreadUpdate).
    pub fn black_market_default() -> Self {
        use crate::game_logic::host_black_market::{
            BLACK_MARKET_AFLAME_DAMAGE_AMOUNT, BLACK_MARKET_AFLAME_DAMAGE_DELAY_FRAMES,
            BLACK_MARKET_AFLAME_DURATION_FRAMES,
        };
        Self {
            flame_damage_limit: 20.0,
            flame_damage_limit_data: 20.0,
            flame_damage_expiration: TREE_FLAME_DAMAGE_EXPIRATION_FRAMES,
            aflame_duration: BLACK_MARKET_AFLAME_DURATION_FRAMES,
            burned_delay: 0,
            spread_enabled: false,
            spread_try_range: 0.0,
            ocl_embers: String::new(),
            aflame_damage_amount: BLACK_MARKET_AFLAME_DAMAGE_AMOUNT,
            aflame_damage_delay: BLACK_MARKET_AFLAME_DAMAGE_DELAY_FRAMES,
            ..Self::tree_default()
        }
    }

    /// Particle Uplink FlammableUpdate residual (no FireSpreadUpdate).
    pub fn particle_uplink_default() -> Self {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_UPLINK_AFLAME_DAMAGE_AMOUNT, PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_FRAMES,
            PARTICLE_UPLINK_AFLAME_DURATION_FRAMES,
        };
        Self {
            flame_damage_limit: 20.0,
            flame_damage_limit_data: 20.0,
            flame_damage_expiration: TREE_FLAME_DAMAGE_EXPIRATION_FRAMES,
            aflame_duration: PARTICLE_UPLINK_AFLAME_DURATION_FRAMES,
            burned_delay: 0,
            spread_enabled: false,
            spread_try_range: 0.0,
            ocl_embers: String::new(),
            aflame_damage_amount: PARTICLE_UPLINK_AFLAME_DAMAGE_AMOUNT,
            aflame_damage_delay: PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_FRAMES,
            ..Self::tree_default()
        }
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if crate::game_logic::host_oil_derrick::is_oil_derrick_template(template_name) {
            Some(Self::oil_derrick_default())
        } else if crate::game_logic::host_black_market::is_black_market_template(template_name) {
            Some(Self::black_market_default())
        } else if is_particle_uplink_flammable_template(template_name) {
            Some(Self::particle_uplink_default())
        } else if is_fire_spread_template(template_name) {
            Some(Self::tree_default())
        } else {
            None
        }
    }

    /// C++ `wouldIgnite`: normal and not already aflame/burned.
    pub fn would_ignite(&self) -> bool {
        self.active && matches!(self.state, HostFlammableState::Normal)
    }

    /// C++ `tryToIgnite` (`FlammableUpdate.cpp:170-197`).
    pub fn try_to_ignite(&mut self, current_frame: u32) -> bool {
        if !self.would_ignite() {
            return false;
        }
        self.state = HostFlammableState::Aflame;
        self.smoldering = false;
        self.body_aflame = true;
        self.burning_sound_active = false;
        self.aflame_end_frame = current_frame.saturating_add(self.aflame_duration);
        // C++: `m_burnedEndFrame = burnedDelay ? now + burnedDelay : 0` (0 = never).
        self.burned_end_frame = if self.burned_delay > 0 {
            current_frame.saturating_add(self.burned_delay)
        } else {
            0
        };
        if self.spread_enabled {
            self.next_spread_frame =
                current_frame.saturating_add(self.calc_next_spread_delay(current_frame));
        }
        self.damage_end_frame = if self.aflame_damage_delay > 0 {
            current_frame.saturating_add(self.aflame_damage_delay)
        } else {
            0
        };
        true
    }
    pub fn apply_flame_damage(&mut self, amount: f32, current_frame: u32) -> bool {
        if matches!(
            self.state,
            HostFlammableState::Burned | HostFlammableState::Aflame
        ) {
            return false;
        }
        // C++: `if (now - expiration > last) reset remaining to authored`.
        if current_frame.saturating_sub(self.flame_damage_expiration) > self.last_flame_damage_dealt
        {
            self.flame_damage_limit = self.flame_damage_limit_data;
            self.flame_damage_accum = 0.0;
        }
        self.last_flame_damage_dealt = current_frame;
        let dealt = amount.max(0.0);
        self.flame_damage_accum += dealt;
        self.flame_damage_limit -= dealt;
        if self.flame_damage_limit <= 0.0 {
            return self.try_to_ignite(current_frame);
        }
        false
    }

    /// C++ `FireSpreadUpdate::calcNextSpreadDelay` GameLogicRandomValue(min, max).
    pub fn calc_next_spread_delay(&self, _salt: u32) -> u32 {
        let lo = self.min_spread_delay as i32;
        let hi = self.max_spread_delay.max(self.min_spread_delay) as i32;
        let delay = gamelogic::helpers::get_game_logic_random_value(lo, hi);
        (delay.max(1) as u32).max(1)
    }

    pub fn is_aflame(&self) -> bool {
        matches!(self.state, HostFlammableState::Aflame)
    }

    /// Per-frame flammable status progression (`FlammableUpdate.cpp:105-145`).
    pub fn tick_flammable(&mut self, current_frame: u32) -> FlammableTickResult {
        let mut r = FlammableTickResult::default();
        match self.state {
            HostFlammableState::Normal => {}
            HostFlammableState::Aflame => {
                r.aflame = true;
                if !self.burning_sound_active && !self.burning_sound_name.is_empty() {
                    self.burning_sound_active = true;
                    r.start_burning_sound = true;
                }
                // C++ FlammableUpdate.cpp:113-117 doAflameDamage on m_damageEndFrame.
                if self.damage_end_frame != 0 && current_frame >= self.damage_end_frame {
                    self.damage_end_frame =
                        current_frame.saturating_add(self.aflame_damage_delay.max(1));
                    r.aflame_damage = self.aflame_damage_amount;
                }
                // Independent burned timer: BURNED + SMOLDERING while still AFLAME.
                if self.burned_end_frame != 0 && current_frame >= self.burned_end_frame {
                    r.smoldering = true;
                    if !self.smoldering {
                        self.smoldering = true;
                        r.became_smoldering = true;
                    }
                }
                if self.aflame_end_frame != 0 && current_frame >= self.aflame_end_frame {
                    self.body_aflame = false;
                    if self.burning_sound_active {
                        r.stop_burning_sound = true;
                        self.burning_sound_active = false;
                    }
                    r.aflame = false;
                    if self.smoldering {
                        self.state = HostFlammableState::Burned;
                        r.became_burned = true;
                        r.burned = true;
                    } else {
                        self.state = HostFlammableState::Normal;
                        r.returned_to_normal = true;
                    }
                }
            }
            HostFlammableState::Burned => {
                r.burned = true;
                r.smoldering = self.smoldering;
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
        r.spawn_embers = !self.ocl_embers.is_empty();
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
    /// C++ burned timer: SMOLDERING while still AFLAME.
    pub smoldering: bool,
    pub became_smoldering: bool,
    /// Aflame ended without BURNED — object can catch fire again.
    pub returned_to_normal: bool,
    pub start_burning_sound: bool,
    pub stop_burning_sound: bool,
    /// C++ FlammableUpdate::doAflameDamage amount this frame (0 = none).
    pub aflame_damage: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadTickResult {
    pub try_spread: bool,
    pub spawn_embers: bool,
}

/// C++ `FROM_CENTER_3D` distance (host Y-up includes height).
pub fn fire_spread_center_3d_distance(a: Vec3, b: Vec3) -> f32 {
    a.distance(b)
}

/// Nature prop / shrubbery fire-spread templates.
pub fn is_fire_spread_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("dogwood")
        || n.contains("tree")
            && (n.contains("pt") || n.contains("shrub") || n.contains("pine") || n.contains("oak"))
        || n.contains("shrubbery")
        || n.ends_with("tree")
        || n.contains("burnabletree")
}

/// Particle Uplink building FlammableUpdate residual.
pub fn is_particle_uplink_flammable_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("particleuplink") || n.contains("particlecannonuplink"))
        && !n.contains("namedbutnomodule")
}

/// Any object that should install FlammableUpdate on the live host.
pub fn is_flammable_template(name: &str) -> bool {
    is_fire_spread_template(name)
        || crate::game_logic::host_oil_derrick::is_oil_derrick_template(name)
        || crate::game_logic::host_black_market::is_black_market_template(name)
        || is_particle_uplink_flammable_template(name)
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
        && fire_spread_ms_to_frames(TREE_FLAME_DAMAGE_EXPIRATION_MS)
            == TREE_FLAME_DAMAGE_EXPIRATION_FRAMES
        && TREE_SPREAD_TRY_RANGE == 50.0
        && TREE_FLAME_DAMAGE_LIMIT == 2.0
        && TREE_AFLAME_DAMAGE_AMOUNT == 5.0
        && TREE_AFLAME_DAMAGE_DELAY_FRAMES == 15
        && TREE_OCL_EMBERS == "OCL_BurningEmbers"
        && TREE_BURNING_SOUND == "GenericFireMediumLoop"
        && is_fire_spread_template("DogwoodTree")
        && is_fire_spread_template("PTDogwood01")
        && !is_fire_spread_template("AmericaTankCrusader")
        && HostFireSpreadData::for_template("TechOilDerrick").is_some()
        && HostFireSpreadData::for_template("GLABlackMarket").is_some()
        && HostFireSpreadData::for_template("AmericaParticleUplinkCannon").is_some()
        && !is_flammable_template("AmericaTankCrusader")
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
        d.next_spread_frame = 10;
        let s = d.tick_spread(10);
        assert!(s.try_spread);
        assert!(s.spawn_embers);
        assert!(d.next_spread_frame > 10);
    }

    #[test]
    fn flame_damage_expiration_resets_threshold() {
        // C++ FlammableUpdate.cpp:82-88 — spaced hits do not accumulate.
        let mut d = HostFireSpreadData::tree_default();
        assert!(!d.apply_flame_damage(1.0, 0));
        assert!(!d.apply_flame_damage(1.0, 90)); // 3s later, threshold reset
        assert!(!d.is_aflame());
        assert!(d.apply_flame_damage(1.0, 91));
        assert!(d.is_aflame());
    }

    #[test]
    fn smoldering_while_aflame_then_burned() {
        // Trees: BurnedDelay 75f, AflameDuration 105f.
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        assert_eq!(d.burned_end_frame, TREE_BURNED_DELAY_FRAMES);
        assert_eq!(d.aflame_end_frame, TREE_AFLAME_DURATION_FRAMES);
        let r75 = d.tick_flammable(TREE_BURNED_DELAY_FRAMES);
        assert!(r75.became_smoldering);
        assert!(r75.smoldering);
        assert!(d.is_aflame());
        assert!(d.body_aflame);
        let r105 = d.tick_flammable(TREE_AFLAME_DURATION_FRAMES);
        assert!(r105.became_burned);
        assert!(r105.stop_burning_sound);
        assert!(matches!(d.state, HostFlammableState::Burned));
        assert!(!d.body_aflame);
        assert!(!d.would_ignite());
    }

    #[test]
    fn aflame_without_burned_delay_returns_to_normal() {
        let mut d = HostFireSpreadData::oil_derrick_default();
        assert_eq!(d.burned_delay, 0);
        assert!(d.try_to_ignite(0));
        assert_eq!(d.burned_end_frame, 0);
        d.aflame_end_frame = 5;
        let r = d.tick_flammable(5);
        assert!(r.returned_to_normal);
        assert!(!r.became_burned);
        assert!(matches!(d.state, HostFlammableState::Normal));
        assert!(d.would_ignite());
    }

    #[test]
    fn burning_sound_starts_once_and_stops_on_extinguish() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        let r0 = d.tick_flammable(0);
        assert!(r0.start_burning_sound);
        assert!(!r0.stop_burning_sound);
        let r1 = d.tick_flammable(1);
        assert!(!r1.start_burning_sound);
        d.aflame_end_frame = 5;
        d.smoldering = true;
        let r5 = d.tick_flammable(5);
        assert!(r5.stop_burning_sound);
        assert!(!d.burning_sound_active);
    }

    #[test]
    fn aflame_dot_applies_after_damage_delay() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        assert_eq!(d.damage_end_frame, TREE_AFLAME_DAMAGE_DELAY_FRAMES);
        let r0 = d.tick_flammable(0);
        assert_eq!(r0.aflame_damage, 0.0);
        let r = d.tick_flammable(TREE_AFLAME_DAMAGE_DELAY_FRAMES);
        assert!((r.aflame_damage - TREE_AFLAME_DAMAGE_AMOUNT).abs() < 0.001);
        assert!(d.is_aflame());
    }

    #[test]
    fn spread_delay_is_random_in_authored_range() {
        let d = HostFireSpreadData::tree_default();
        let mut seen = std::collections::BTreeSet::new();
        for salt in 0..32 {
            let delay = d.calc_next_spread_delay(salt);
            assert!(
                (TREE_MIN_SPREAD_DELAY_FRAMES..=TREE_MAX_SPREAD_DELAY_FRAMES).contains(&delay),
                "delay {delay} outside 30..=60"
            );
            seen.insert(delay);
        }
        assert!(
            seen.len() > 1,
            "GameLogicRandomValue must not collapse to a fixed midpoint"
        );
    }

    #[test]
    fn center_3d_includes_height() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(30.0, 40.0, 0.0);
        let d = fire_spread_center_3d_distance(a, b);
        assert!((d - 50.0).abs() < 0.01);
        let planar = (30.0_f32 * 30.0).sqrt();
        assert!(d > planar + 1.0);
    }

    #[test]
    fn buildings_install_flammable_without_tree_spread() {
        let oil = HostFireSpreadData::for_template("TechOilDerrick").expect("oil");
        assert!((oil.flame_damage_limit_data - 20.0).abs() < 0.01);
        assert!(!oil.spread_enabled);
        assert!((oil.aflame_damage_amount - 25.0).abs() < 0.01);
        let mkt = HostFireSpreadData::for_template("GLABlackMarket").expect("market");
        assert!(!mkt.spread_enabled);
        let puc = HostFireSpreadData::for_template("AmericaParticleUplinkCannon").expect("puc");
        assert!(!puc.spread_enabled);
        assert!(HostFireSpreadData::for_template("AmericaTankCrusader").is_none());
    }
}
