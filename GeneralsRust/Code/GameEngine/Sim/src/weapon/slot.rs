use serde::{Deserialize, Serialize};

use super::{WeaponAntiMask, WeaponBonus, WeaponDefinition, WeaponFireStatus};

/// Weapon and combat stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: f32,
    /// Authored WeaponTemplate::m_attackRange. Runtime applies RANGE bonus
    /// before the C++ quarter-cell deduction; this field is not pre-reduced.
    pub range: f32,
    /// Authored WeaponTemplate::m_minimumAttackRange. Runtime subtracts one
    /// quarter-cell before its minimum-distance gate; 0.0 means no minimum.
    pub min_range: f32,
    pub reload_time: f32,
    pub last_fire_time: f32,
    /// C++ Weapon::m_lastFireFrame, maintained independently for each slot.
    pub last_fire_frame: u32,
    /// Authored Weapon::m_whenWeCanFireAgain. Never reconstructed from seconds.
    #[serde(skip)]
    pub next_fire_frame: u32,
    /// C++ m_whenLastReloadStarted, used only for progress presentation.
    #[serde(skip)]
    pub reload_started_frame: u32,
    /// Explicit ownership of the authored integer scheduler; residual adapters
    /// alone retain the historical seconds clock.
    #[serde(skip)]
    pub frame_clock_active: bool,
    /// C++ m_status for authored scheduling. Ammo is not a status proxy:
    /// shared cooldowns also overwrite the status of empty sibling weapons.
    #[serde(skip)]
    pub frame_clock_status: WeaponFireStatus,
    pub ammo: Option<u32>,
    /// C++ ClipSize residual. 0 = unlimited (ammo ignored for readiness).
    #[serde(default)]
    pub clip_size: u32,
    /// C++ ClipReloadTime residual (seconds) when clip empties. 0 = ready same frame.
    #[serde(default)]
    pub clip_reload_time: f32,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    /// C++ parity (WeaponTemplate::m_weaponSpeed): projectile travel speed.
    /// 0.0 = instant-hit (laser/flame weapons).
    pub projectile_speed: f32,
    /// C++ parity (WeaponTemplate::m_preAttackDelay): delay before firing
    /// after a target is acquired, in seconds.  0.0 = no delay.
    pub pre_attack_delay: f32,
    /// C++ radius damage residual (WeaponTemplate primary/secondary radius).
    /// 0.0 = no splash (direct hit only).
    #[serde(default)]
    pub splash_radius: f32,
    /// C++ `Weapon::m_status == RELOADING_CLIP`. ShareWeaponReloadTime siblings
    /// keep this set so `getRemainingAmmo` / pips report 0 while the clip waits.
    #[serde(default)]
    pub reloading_clip: bool,
    /// Last RATE_OF_FIRE bonus applied to this slot. 0 = unset.
    /// C++ `Weapon::onWeaponBonusChange` restarts an in-progress wait.
    #[serde(default)]
    pub last_bonus_rof: f32,
    /// C++ `Weapon::m_suspendFXFrame` (Weapon.cpp:1742).  This is client-only
    /// runtime state: FireFX is suppressed while the current logic frame is
    /// below this absolute frame, but recoil still broadcasts.  Keep it out
    /// of the nested serde/bincode Weapon payload; ObjectSnapshot owns the
    /// versioned parallel tail so historical positional records remain safe.
    #[serde(skip)]
    pub suspend_fx_frame: u32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 25.0,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            last_fire_frame: 0,
            next_fire_frame: 0,
            reload_started_frame: 0,
            frame_clock_active: false,
            frame_clock_status: Default::default(),
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
            suspend_fx_frame: 0,
        }
    }
}

impl Weapon {
    pub fn from_definition(wt: &WeaponDefinition) -> Self {
        const FPS: f32 = 30.0;
        // CPP WeaponStore::allocateNewWeapon installs a supplied definition
        // even with zero damage/range (WeaponTemplate constructor defaults).
        // FX-only/status/detonation rules are weapons, not missing content.
        // CPP Weapon.cpp:475–490. Read the minimum for the installed stat.
        // Only private fire draws a delay from the receiving world's RNG.
        let delay_frames = wt.delay_between_shots(&WeaponBonus::new(), |min, _| min);
        // CPP WeaponTemplate::getDelayBetweenShots returns zero for an
        // authored zero delay; no synthetic one-second cooldown is added.
        let reload_time = delay_frames.max(0) as f32 / FPS;
        let pre_attack_delay = (wt.pre_attack_delay.max(0) as f32) / FPS;
        let projectile_speed = if wt.weapon_speed >= 999_999.0 {
            0.0
        } else {
            wt.weapon_speed
        };
        let suspend_fx_frame = 0; // Immutable stat lookup has no installed owner deadline.
        // Keep authored range values, as WeaponTemplate does in C++.
        // Runtime applies RANGE first, then one quarter-cell deduction
        // (Weapon.cpp:437-462); pre-reducing here would deduct twice and
        // multiply the first deduction by RANGE bonuses.
        Self {
            damage: wt.primary_damage,
            range: wt.get_unmodified_attack_range(),
            min_range: wt.minimum_attack_range,
            reload_time,
            last_fire_time: 0.0,
            last_fire_frame: 0,
            next_fire_frame: 0,
            reload_started_frame: 0,
            frame_clock_active: false,
            frame_clock_status: Default::default(),
            ammo: if wt.clip_size > 0 {
                Some(wt.clip_size as u32)
            } else {
                None
            },
            clip_size: wt.clip_size.max(0) as u32,
            // C++ ClipReloadTime is independent of DelayBetweenShots. Store
            // already converted msec → frames; host Weapon uses seconds.
            // Absent/0 stays 0 — reloadWithBonus is ready the same frame.
            clip_reload_time: if wt.clip_size > 0 {
                (wt.clip_reload_time.max(0) as f32) / FPS
            } else {
                0.0
            },
            can_target_air: wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE)
                || wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_INFANTRY),
            // C++ WeaponTemplate defaults to AntiGround and accepts a ground
            // victim only when that actual anti-mask bit is set.  Treating an
            // arbitrary non-air mask (for example AntiProjectile) as ground
            // let point-defense-only weapons attack ordinary units.
            can_target_ground: wt.anti_mask.contains(WeaponAntiMask::GROUND),
            projectile_speed,
            pre_attack_delay,
            splash_radius: wt.primary_damage_radius.max(0.0),
            reloading_clip: false,
            last_bonus_rof: 0.0,
            suspend_fx_frame,
        }
    }

    pub fn from_definition_at(definition: &WeaponDefinition, frame: u32) -> Self {
        let mut weapon = Self::from_definition(definition);
        // Weapon.cpp:1742 sets the deadline only on an installed weapon.
        weapon.suspend_fx_frame = frame.wrapping_add(definition.suspend_fx_delay);
        weapon
    }

    /// Read-only equivalent of Weapon::getStatus after its pre-attack gate.
    pub fn frame_status_at(&self, frame: u32) -> WeaponFireStatus {
        if frame >= self.next_fire_frame {
            if self.clip_size == 0 || self.ammo.is_some_and(|ammo| ammo > 0) {
                WeaponFireStatus::ReadyToFire
            } else {
                WeaponFireStatus::OutOfAmmo
            }
        } else {
            self.frame_clock_status
        }
    }

    /// C++ `TheGameLogic->getFrame() < getSuspendFXFrame()` gate used by
    /// Weapon::fireWeapon.  The renderer/DrawModule route remains fail-closed
    /// until its pre-mutation owner snapshot exists; this helper only exposes
    /// the authoritative source state.
    #[inline]
    pub fn fire_fx_is_suspended_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.suspend_fx_frame
    }

    /// Set the absolute frame at which FireFX becomes eligible.  This mirrors
    /// construction/copy state without deriving a delay from a model name.
    #[inline]
    pub fn set_suspend_fx_frame(&mut self, frame: u32) {
        self.suspend_fx_frame = frame;
    }
}

/// C++ FiringTracker thresholds for an installed weapon definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponTrackerBind {
    pub continuous_fire_one_shots: u32,
    pub continuous_fire_two_shots: u32,
    pub continuous_fire_coast_frames: u32,
    pub auto_reload_when_idle_frames: u32,
}

impl Default for WeaponTrackerBind {
    fn default() -> Self {
        Self {
            continuous_fire_one_shots: u32::MAX,
            continuous_fire_two_shots: u32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
        }
    }
}

impl WeaponTrackerBind {
    pub fn from_definition(definition: &WeaponDefinition) -> Self {
        fn shot_threshold(value: i32) -> u32 {
            if value <= 0 || value == i32::MAX {
                u32::MAX
            } else {
                value as u32
            }
        }
        Self {
            continuous_fire_one_shots: shot_threshold(definition.continuous_fire_one_shots_needed),
            continuous_fire_two_shots: shot_threshold(definition.continuous_fire_two_shots_needed),
            continuous_fire_coast_frames: definition.continuous_fire_coast_frames,
            auto_reload_when_idle_frames: definition.auto_reload_when_idle_frames,
        }
    }
}
