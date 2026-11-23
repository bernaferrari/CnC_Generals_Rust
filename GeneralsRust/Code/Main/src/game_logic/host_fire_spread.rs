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

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const FIRE_SPREAD_LOGIC_FPS: f32 = 30.0;

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
    /// Live playback receipt. FlammableUpdate::xfer omits m_audioHandle.
    #[serde(skip)]
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

    /// Authored-module install: the first authored `FlammableUpdate` supplies
    /// the FlammableUpdate fields (C++ FlammableUpdateModuleData), and an
    /// authored `FireSpreadUpdate` supplies spread timers, range, and ember list.
    pub(crate) fn from_authored(template: &crate::game_logic::ThingTemplate) -> Option<Self> {
        use gamelogic::object::behavior::flammable_update::FlammableUpdateModuleData;
        use gamelogic::object::update::fire_spread_update::FireSpreadUpdateModuleData;
        let flame = template.authored_module_data::<FlammableUpdateModuleData>("FlammableUpdate")?;
        let spread = template.authored_module_data::<FireSpreadUpdateModuleData>("FireSpreadUpdate");
        Some(Self {
            flame_damage_limit: flame.flame_damage_limit,
            flame_damage_limit_data: flame.flame_damage_limit,
            flame_damage_expiration: flame.flame_damage_expiration_delay,
            aflame_duration: flame.aflame_duration,
            burned_delay: flame.burned_delay,
            aflame_damage_amount: flame.aflame_damage_amount,
            aflame_damage_delay: flame.aflame_damage_delay,
            burning_sound_name: flame.burning_sound_name.to_string(),
            spread_enabled: spread.is_some(),
            min_spread_delay: spread.as_ref().map_or(0, |s| s.min_spread_try_delay),
            max_spread_delay: spread.as_ref().map_or(0, |s| s.max_spread_try_delay),
            spread_try_range: spread.as_ref().map_or(0.0, |s| s.spread_try_range),
            ocl_embers: spread
                .as_ref()
                .and_then(|data| data.ocl_embers.as_deref())
                .unwrap_or_default()
                .to_owned(),
            ..Self::tree_default()
        })
    }

    /// C++ `wouldIgnite`: normal and not already aflame/burned.
    pub fn would_ignite(&self) -> bool {
        self.active && matches!(self.state, HostFlammableState::Normal)
    }

    /// Standalone compatibility for `tryToIgnite` fixtures. Production owners
    /// use the Body callback continuation before committing ignition.
    pub fn try_to_ignite(&mut self, current_frame: u32) -> bool {
        if !self.would_ignite() {
            return false;
        }
        self.burning_sound_active = false;
        self.start_fire_spreading(current_frame);
        self.finish_ignition(current_frame);
        true
    }

    /// Standalone compatibility startFireSpreading. Production owners call
    /// the `with_random` form at the receiving callback boundary.
    pub fn start_fire_spreading(&mut self, current_frame: u32) {
        if self.spread_enabled {
            self.next_spread_frame =
                current_frame.saturating_add(self.calc_next_spread_delay(current_frame));
        }
    }

    /// Receiving production path; preserves one inclusive logic draw at the
    /// original startFireSpreading point on this world's stream.
    pub fn start_fire_spreading_with_random(
        &mut self,
        current_frame: u32,
        random: &game_engine::common::random_value::SessionRandom,
    ) {
        if self.spread_enabled {
            self.next_spread_frame =
                current_frame.saturating_add(self.calc_next_spread_delay_with_random(random));
        }
    }

    /// Commit FlammableUpdate state/timers after the receiving visual callback.
    pub fn finish_ignition(&mut self, current_frame: u32) {
        self.state = HostFlammableState::Aflame;
        self.smoldering = false;
        self.body_aflame = true;
        self.aflame_end_frame = current_frame.saturating_add(self.aflame_duration);
        // C++: `m_burnedEndFrame = burnedDelay ? now + burnedDelay : 0` (0 = never).
        self.burned_end_frame = if self.burned_delay > 0 {
            current_frame.saturating_add(self.burned_delay)
        } else {
            0
        };
        self.damage_end_frame = if self.aflame_damage_delay > 0 {
            current_frame.saturating_add(self.aflame_damage_delay)
        } else {
            0
        };
    }
    /// Standalone convenience API. Production Object damage uses
    /// `apply_flame_damage_before_ignite` and the receiving Body callback.
    pub fn apply_flame_damage(&mut self, amount: f32, current_frame: u32) -> bool {
        let can_accumulate = self.would_ignite();
        if self.apply_flame_damage_before_ignite(amount, current_frame, can_accumulate) {
            return self.try_to_ignite(current_frame);
        }
        false
    }

    /// Update flame accumulation without committing FS_AFLAME. The receiving
    /// Object must run status/Body/model/audio callbacks before finalization.
    /// `can_accumulate` carries the caller's Object status check: C++ tests
    /// OBJECT_STATUS_AFLAME/BURNED after the threshold reset and timestamp,
    /// so a status-burned object still records the hit without accumulating.
    pub(in crate::game_logic) fn apply_flame_damage_before_ignite(
        &mut self,
        amount: f32,
        current_frame: u32,
        can_accumulate: bool,
    ) -> bool {
        // The original compares an unsigned frame difference, including
        // before the first expiration window has elapsed.
        if current_frame.wrapping_sub(self.flame_damage_expiration) > self.last_flame_damage_dealt {
            self.flame_damage_limit = self.flame_damage_limit_data;
            self.flame_damage_accum = 0.0;
        }
        self.last_flame_damage_dealt = current_frame;
        if !can_accumulate {
            return false;
        }
        let dealt = amount.max(0.0);
        self.flame_damage_accum += dealt;
        self.flame_damage_limit -= dealt;
        self.flame_damage_limit <= 0.0 && self.would_ignite()
    }

    /// Standalone compatibility draw. Receiving GameLogic code uses the
    /// explicit-session variant below.
    pub fn calc_next_spread_delay(&self, _salt: u32) -> u32 {
        let lo = self.min_spread_delay as i32;
        let hi = self.max_spread_delay.max(self.min_spread_delay) as i32;
        let delay = gamelogic::helpers::get_game_logic_random_value(lo, hi);
        (delay.max(1) as u32).max(1)
    }

    /// C++ `GameLogicRandomValue(MinSpreadDelay, MaxSpreadDelay)` — one draw,
    /// inclusive, unsigned result floored at 1 frame.
    pub fn calc_next_spread_delay_with_random(
        &self,
        random: &game_engine::common::random_value::SessionRandom,
    ) -> u32 {
        let delay = random.next_int(
            self.min_spread_delay as i32,
            self.max_spread_delay as i32,
        ) as u32;
        delay.max(1)
    }

    pub fn is_aflame(&self) -> bool {
        matches!(self.state, HostFlammableState::Aflame)
    }

    /// Per-frame flammable status progression (`FlammableUpdate.cpp:105-145`).
    pub fn tick_flammable(&mut self, current_frame: u32) -> FlammableTickResult {
        let aflame_damage = self.begin_tick_flammable(current_frame);
        FlammableTickResult {
            aflame_damage,
            ..self.finish_tick_flammable(current_frame)
        }
    }

    /// FlammableUpdate.cpp:113–117 runs damage before the burned/end timers.
    pub fn begin_tick_flammable(&mut self, current_frame: u32) -> f32 {
        if self.is_aflame() && self.damage_end_frame != 0 && current_frame >= self.damage_end_frame {
            self.damage_end_frame =
                current_frame.saturating_add(self.aflame_damage_delay.max(1));
            self.aflame_damage_amount
        } else {
            0.0
        }
    }

    /// C++ FlammableUpdate::update burned/end timer phase, after doAflameDamage.
    pub fn finish_tick_flammable(&mut self, current_frame: u32) -> FlammableTickResult {
        let object_burned = self.smoldering;
        self.finish_tick_flammable_with_burned_status(current_frame, object_burned)
    }

    /// The C++ expiry branch reads Object::BURNED independently of the
    /// FlammableUpdate module's private state.
    pub(in crate::game_logic) fn finish_tick_flammable_with_burned_status(
        &mut self,
        current_frame: u32,
        object_burned: bool,
    ) -> FlammableTickResult {
        let mut r = FlammableTickResult::default();
        match self.state {
            HostFlammableState::Normal => {}
            HostFlammableState::Aflame => {
                r.aflame = true;
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
                    if r.smoldering || object_burned {
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

    /// Standalone compatibility update. Production schedules after its effects.
    pub fn tick_spread(&mut self, current_frame: u32) -> SpreadTickResult {
        if !self.is_aflame() || !self.spread_due(current_frame) {
            return SpreadTickResult::default();
        }
        let delay = self.calc_next_spread_delay(current_frame);
        self.finish_spread_tick(current_frame, delay)
    }

    /// C++ schedules the next wake after creating embers and igniting a target.
    pub(in crate::game_logic) fn schedule_next_spread_with_random(
        &mut self,
        current_frame: u32,
        random: &game_engine::common::random_value::SessionRandom,
    ) {
        let delay = self.calc_next_spread_delay_with_random(random);
        self.next_spread_frame = current_frame.saturating_add(delay);
    }

    /// Due-clock predicate only — `u32::MAX` is the Forever sentinel an
    /// extinguished source is retired to. The original gates the wake on the
    /// Object's AFLAME status bit, not the private module state.
    pub(in crate::game_logic) fn spread_due(&self, current_frame: u32) -> bool {
        self.spread_enabled
            && self.next_spread_frame != u32::MAX
            && current_frame >= self.next_spread_frame
    }

    /// C++ FlammableUpdate work this frame still owes the clock: pending DOT,
    /// a smoldering transition, or the aflame-end transition. Authored
    /// ignition queues the burning sound synchronously
    /// (`Object::complete_flammable_ignition`), so a sleeping sound is not
    /// part of the predicate.
    pub(in crate::game_logic) fn flammable_due(&self, frame: u32) -> bool {
        // C++ calcSleepTime gates every timer behind a future nonzero aflame
        // end frame. At zero duration the module sleeps forever, even when
        // DOT or BurnedDelay deadlines were also authored.
        self.is_aflame()
            && self.aflame_duration > 0
            && ((self.damage_end_frame != 0 && frame >= self.damage_end_frame)
                || (!self.smoldering
                    && self.burned_end_frame != 0
                    && frame >= self.burned_end_frame)
                || (self.aflame_end_frame != 0 && frame >= self.aflame_end_frame))
    }

    fn finish_spread_tick(&mut self, current_frame: u32, delay: u32) -> SpreadTickResult {
        let mut r = SpreadTickResult::default();
        r.try_spread = true;
        r.spawn_embers = !self.ocl_embers.is_empty();
        self.next_spread_frame = current_frame.saturating_add(delay);
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
    pub stop_burning_sound: bool,
    /// C++ FlammableUpdate::doAflameDamage amount this frame (0 = none).
    pub aflame_damage: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadTickResult {
    pub try_spread: bool,
    pub spawn_embers: bool,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flame_damage_ignites_and_spreads() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.would_ignite());
        // Frame 60: `60 - expiration(60) == 0 <= last`, so neither hit resets.
        assert!(!d.apply_flame_damage(1.0, 60));
        assert!(d.apply_flame_damage(1.0, 60)); // limit 2
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
    fn zero_duration_end_sleeps_forever_even_with_other_timer_deadlines() {
        let mut sleeping = HostFireSpreadData::tree_default();
        sleeping.aflame_duration = 0;
        sleeping.burned_delay = 4;
        sleeping.aflame_damage_delay = 3;
        assert!(sleeping.try_to_ignite(17));
        assert_eq!(sleeping.aflame_end_frame, 17);
        assert!(!sleeping.flammable_due(17));
        assert!(!sleeping.flammable_due(18));
        assert!(!sleeping.flammable_due(20));
        assert!(!sleeping.flammable_due(21));
        assert!(sleeping.is_aflame());
    }

    #[test]
    fn aflame_expiry_uses_object_burned_bit_even_without_private_smoldering() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        d.aflame_end_frame = 5;
        assert!(!d.smoldering);
        let r = d.finish_tick_flammable_with_burned_status(5, true);
        assert!(r.became_burned);
        assert!(!r.returned_to_normal);
        assert!(matches!(d.state, HostFlammableState::Burned));
    }

    #[test]
    fn active_burning_sound_stops_on_extinction() {
        let mut d = HostFireSpreadData::tree_default();
        assert!(d.try_to_ignite(0));
        d.burning_sound_active = true;
        assert!(!d.tick_flammable(1).stop_burning_sound);
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
    fn spread_delay_follows_the_logic_draw_across_the_authored_range() {
        // FireSpreadUpdate: GameLogicRandomValue(MinSpreadDelay, MaxSpreadDelay).
        let d = HostFireSpreadData::tree_default();
        let random = game_engine::common::random_value::SessionRandom::default();
        let untouched = random.seed_words();
        for want in [
            TREE_MIN_SPREAD_DELAY_FRAMES,
            (TREE_MIN_SPREAD_DELAY_FRAMES + TREE_MAX_SPREAD_DELAY_FRAMES) / 2,
            TREE_MAX_SPREAD_DELAY_FRAMES,
        ] {
            random.script_int(
                TREE_MIN_SPREAD_DELAY_FRAMES as i32,
                TREE_MAX_SPREAD_DELAY_FRAMES as i32,
                want as i32,
            );
            assert_eq!(d.calc_next_spread_delay_with_random(&random), want);
        }
        assert_eq!(random.seed_words(), untouched, "one draw per delay, nothing else");
    }

    #[test]
    fn authored_modules_install_fire_spread_not_the_template_name() {
        // No name guesswork: a tree-shaped name without modules installs
        // nothing, a flammable-only building installs without spread, and the
        // tree's authored values drive the runtime fields.
        let mut world = crate::game_logic::GameLogic::new();
        let mut ini = fire_fixture_ini(&["FixtureTreeAlpha"], &["FixtureOilBuilding"]);
        ini.push_str(
            "Object DogwoodTreeDecoy\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n",
        );
        world
            .admit_object_ini_sources([("fire-fixture.ini".into(), ini)])
            .expect("fire fixture INI admitted");

        let tree = world.templates.get("FixtureTreeAlpha").unwrap();
        let tree_data = HostFireSpreadData::from_authored(tree).expect("tree");
        assert!(tree_data.spread_enabled);
        assert_eq!(tree_data.min_spread_delay, TREE_MIN_SPREAD_DELAY_FRAMES);
        assert_eq!(tree_data.max_spread_delay, TREE_MAX_SPREAD_DELAY_FRAMES);
        assert_eq!(tree_data.spread_try_range, TREE_SPREAD_TRY_RANGE);
        assert_eq!(tree_data.ocl_embers, TREE_OCL_EMBERS);
        assert_eq!(tree_data.burned_delay, TREE_BURNED_DELAY_FRAMES);
        assert!((tree_data.flame_damage_limit - TREE_FLAME_DAMAGE_LIMIT).abs() < 0.01);
        assert_eq!(tree_data.burning_sound_name, TREE_BURNING_SOUND);

        let building = world.templates.get("FixtureOilBuilding").unwrap();
        let building_data = HostFireSpreadData::from_authored(building).expect("oil");
        assert!(!building_data.spread_enabled);
        assert!((building_data.flame_damage_limit_data - 20.0).abs() < 0.01);
        assert!((building_data.aflame_damage_amount - 25.0).abs() < 0.01);
        assert_eq!(building_data.burned_delay, 0);
        assert_eq!(building_data.min_spread_delay, 0);
        assert_eq!(building_data.ocl_embers, "");

        let decoy = world.templates.get("DogwoodTreeDecoy").unwrap();
        assert!(
            HostFireSpreadData::from_authored(decoy).is_none(),
            "a tree-shaped name without authored fire modules installs nothing"
        );
    }
}

/// Retail-style flammable fixture INI (NatureProp.ini tree timings; the
/// building block mirrors the oil-derrick residual: no FireSpreadUpdate).
/// Assembled from the authored module blocks only — no name inference.
#[cfg(test)]
pub(crate) fn fire_fixture_ini(trees: &[&str], buildings: &[&str]) -> String {
    let mut ini = String::new();
    for name in trees {
        ini.push_str(&format!(
            "Object {name}\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\n Behavior = FlammableUpdate BurnTag\n FlameDamageLimit = 2\n FlameDamageExpiration = 2000\n BurnedDelay = 2500\n AflameDuration = 3500\n AflameDamageDelay = 500\n AflameDamageAmount = 5\n BurningSoundName = GenericFireMediumLoop\n End\n Behavior = FireSpreadUpdate SpreadTag\n MinSpreadDelay = 1000\n MaxSpreadDelay = 2000\n SpreadTryRange = 50\n OCLEmbers = OCL_BurningEmbers\n End\nEnd\n"
        ));
    }
    for name in buildings {
        ini.push_str(&format!(
            "Object {name}\n KindOf = STRUCTURE\n Body = ActiveBody BodyTag\n MaxHealth = 2000\n InitialHealth = 2000\n End\n Behavior = FlammableUpdate BurnTag\n FlameDamageLimit = 20\n FlameDamageExpiration = 2000\n BurnedDelay = 0\n AflameDuration = 5000\n AflameDamageDelay = 500\n AflameDamageAmount = 25\n BurningSoundName = GenericFireMediumLoop\n End\nEnd\n"
        ));
    }
    ini
}

/// Admit the fire fixture Objects as this world's ordered startup content.
#[cfg(test)]
pub(crate) fn admit_fire_fixture(
    world: &mut crate::game_logic::GameLogic,
    trees: &[&str],
    buildings: &[&str],
) {
    world
        .admit_object_ini_sources([("fire-fixture.ini".into(), fire_fixture_ini(trees, buildings))])
        .expect("fire fixture INI admitted");
}
