//! Host America Microwave Tank residual (disable buildings + clear garrison).
//!
//! Residual slice (playability):
//! - AmericaTankMicrowave / *Microwave* residual sources continuously subdue
//!   enemy/neutral structures they are actively attacking within residual
//!   MicrowaveTankBuildingDisabler AttackRange **200** (SUBDUAL_BUILDING →
//!   DISABLED_SUBDUED). Subdued structures count as `is_disabled()` so
//!   production / powered functions stop while cooked.
//! - Garrison clear residual (KILL_GARRISONED / MicrowaveTankBuildingClearer)
//!   is applied via the existing combat path (`host_bunker_buster` clearer
//!   residual): floor(damage) occupants killed, no structure HP damage.
//!
//! Wave 55 residual pack (retail Weapon.ini honesty):
//! - Cook radius residual: BuildingDisabler AttackRange **200**, clearer **125**,
//!   Emitter self-field PrimaryDamageRadius **100** / dmg **8**
//! - Disable residual: PrimaryDamage **50** SUBDUAL_BUILDING, Delay **100**ms → **3**f,
//!   FireSoundLoopTime **120**ms, LaserName MicrowaveDisableStream
//! - Ally filter residual: RadiusDamageAffects ALLIES ENEMIES NEUTRALS in INI,
//!   host residual cooks enemy/neutral only (fail-closed vs ally griefing)
//! - Weapon residual: clearer PrimaryDamage **1** KILL_GARRISONED, Delay **100**ms,
//!   emitter Delay **250**ms → **8**f, DamageType MICROWAVE / DeathType BURNED
//!
//! Fail-closed honesty:
//! - Not full subdual damage accumulate / SubdualDamageHelper heal drain
//! - Not full MicrowaveDisableStream laser attach / FireWeaponUpdate emitter
//!   infantry MICROWAVE damage field volume (emitter residual constants only)
//! - Not full vehicle disabler (retail WeaponSet has VehicleDisabler commented out)
//! - Not network microwave replication (network deferred)

use serde::{Deserialize, Serialize};

/// Logic frames per second residual.
pub const MICROWAVE_LOGIC_FPS: f32 = 30.0;

/// Retail MicrowaveTankBuildingDisabler AttackRange residual.
pub const HOST_MICROWAVE_DISABLE_RANGE: f32 = 200.0;

/// Retail MicrowaveTankBuildingClearer AttackRange residual (secondary).
pub const HOST_MICROWAVE_CLEAR_RANGE: f32 = 125.0;

/// Retail MicrowaveTankBuildingDisabler PrimaryDamage residual (subdual/pulse).
/// Fail-closed continuous residual does not accumulate; used for honesty docs.
pub const HOST_MICROWAVE_SUBDUAL_PULSE: f32 = 50.0;

/// Retail MicrowaveTankBuildingClearer PrimaryDamage residual (= 1 occupant).
pub const HOST_MICROWAVE_CLEAR_PER_SHOT: f32 = 1.0;

/// Retail MicrowaveTankBuildingDisabler / Clearer DelayBetweenShots residual (msec).
pub const HOST_MICROWAVE_DELAY_MS: u32 = 100;
/// DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const HOST_MICROWAVE_DELAY_FRAMES: u32 = 3;

/// Retail MicrowaveTankBuildingDisabler FireSoundLoopTime residual (msec).
pub const HOST_MICROWAVE_FIRE_SOUND_LOOP_MS: u32 = 120;
/// FireSoundLoopTime 120ms → 4 frames @ 30 FPS.
pub const HOST_MICROWAVE_FIRE_SOUND_LOOP_FRAMES: u32 = 4;

/// Retail LaserName residual on disabler/clearer.
pub const HOST_MICROWAVE_LASER_NAME: &str = "MicrowaveDisableStream";
/// Retail LaserBoneName residual.
pub const HOST_MICROWAVE_LASER_BONE: &str = "WEAPON02";

/// Retail DamageType residual (building cook).
pub const HOST_MICROWAVE_DAMAGE_TYPE_SUBDUAL: &str = "SUBDUAL_BUILDING";
/// Retail DamageType residual (garrison clear).
pub const HOST_MICROWAVE_DAMAGE_TYPE_CLEAR: &str = "KILL_GARRISONED";
/// Retail MicrowaveTankEmitterWeapon DamageType residual.
pub const HOST_MICROWAVE_DAMAGE_TYPE_EMITTER: &str = "MICROWAVE";
/// Retail emitter DeathType residual.
pub const HOST_MICROWAVE_DEATH_TYPE_EMITTER: &str = "BURNED";

/// Retail RadiusDamageAffects residual tokens (Weapon.ini).
pub const HOST_MICROWAVE_RADIUS_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Host residual ally-cook filter: disable ally structures (fail-closed No).
pub const HOST_MICROWAVE_AFFECTS_ALLIES: bool = false;
/// Host residual cooks enemies.
pub const HOST_MICROWAVE_AFFECTS_ENEMIES: bool = true;
/// Host residual cooks neutrals.
pub const HOST_MICROWAVE_AFFECTS_NEUTRALS: bool = true;

/// Retail MicrowaveTankEmitterWeapon PrimaryDamage residual.
pub const HOST_MICROWAVE_EMITTER_DAMAGE: f32 = 8.0;
/// Retail MicrowaveTankEmitterWeapon PrimaryDamageRadius residual (cook field).
pub const HOST_MICROWAVE_EMITTER_RADIUS: f32 = 100.0;
/// Retail MicrowaveTankEmitterWeapon AttackRange residual.
pub const HOST_MICROWAVE_EMITTER_RANGE: f32 = 100.0;
/// Retail MicrowaveTankEmitterWeapon DelayBetweenShots residual (msec).
pub const HOST_MICROWAVE_EMITTER_DELAY_MS: u32 = 250;
/// Delay 250ms → 8 frames @ 30 FPS (ceil).
pub const HOST_MICROWAVE_EMITTER_DELAY_FRAMES: u32 = 8;
/// Retail emitter RadiusDamageAffects residual (enemies, not airborne).
pub const HOST_MICROWAVE_EMITTER_AFFECTS: &str = "ENEMIES NOT_AIRBORNE";
/// Retail DamageDealtAtSelfPosition residual.
pub const HOST_MICROWAVE_EMITTER_DAMAGE_AT_SELF: bool = true;
/// Retail emitter FireFX residual.
pub const HOST_MICROWAVE_EMITTER_FX: &str = "FX_MicrowaveTankEmitter";

/// Retail weapon template residual names.
pub const MICROWAVE_WEAPON_BUILDING_DISABLER: &str = "MicrowaveTankBuildingDisabler";
pub const MICROWAVE_WEAPON_BUILDING_CLEARER: &str = "MicrowaveTankBuildingClearer";
pub const MICROWAVE_WEAPON_EMITTER: &str = "MicrowaveTankEmitterWeapon";
/// Retail VehicleDisabler exists but is commented out on WeaponSet residual.
pub const MICROWAVE_WEAPON_VEHICLE_DISABLER: &str = "MicrowaveTankVehicleDisabler";
pub const MICROWAVE_WEAPON_VEHICLE_DISABLER_ENABLED: bool = false;

/// PrimaryDamageRadius 0 residual = hits only intended victim.
pub const HOST_MICROWAVE_PRIMARY_RADIUS: f32 = 0.0;
/// WeaponSpeed residual (effectively instant).
pub const HOST_MICROWAVE_WEAPON_SPEED: f32 = 999_999.0;

/// Activate / cook audio residual.
pub const MICROWAVE_DISABLE_AUDIO: &str = "MicrowaveWeaponLoop";

/// Convert msec residual → logic frames @ 30 FPS (ceil).
pub fn microwave_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * MICROWAVE_LOGIC_FPS / 1000.0).ceil() as u32
}

/// Whether template is a residual Microwave Tank source.
///
/// Fail-closed: name residual (not full INI WeaponSet / FireWeaponUpdate matrix).
pub fn is_microwave_tank(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testmicrowave" || n == "testmicrowavetank" {
        return true;
    }
    // AmericaTankMicrowave / USA_MicrowaveTank / Lazr_AmericaTankMicrowave / …
    if n.contains("microwave") {
        // Skip pure weapon / stream FX object names.
        if n.contains("stream")
            || n.contains("emitter")
            || n.contains("weapon")
            || n.contains("fx")
            || n.contains("particle")
        {
            return false;
        }
        return true;
    }
    false
}

/// Whether residual target can be subdued by a Microwave building disabler.
///
/// Retail: SUBDUAL_BUILDING on structures; RadiusDamageAffects ALLIES ENEMIES NEUTRALS
/// but residual only cooks enemy/neutral (fail-closed vs ally disable griefing).
pub fn is_legal_microwave_disable_target(
    is_structure: bool,
    is_alive: bool,
    enemy_or_neutral: bool,
    under_construction: bool,
) -> bool {
    is_structure && is_alive && enemy_or_neutral && !under_construction
}

/// True when microwave team vs target team is residual-hostile (enemy) or Neutral victim.
pub fn is_microwave_hostile_team(
    microwave_team_is_neutral: bool,
    same_team: bool,
    target_is_neutral: bool,
) -> bool {
    if microwave_team_is_neutral {
        // Neutral microwave residual does not cook anyone (fail-closed).
        return false;
    }
    !same_team || target_is_neutral
}

/// Ally-filter residual: should cook this relationship under host residual rules.
///
/// Maps retail RadiusDamageAffects ALLIES ENEMIES NEUTRALS → host filters allies out.
pub fn microwave_ally_filter_allows(
    same_team_ally: bool,
    target_is_neutral: bool,
    target_is_enemy: bool,
) -> bool {
    if same_team_ally {
        return HOST_MICROWAVE_AFFECTS_ALLIES;
    }
    if target_is_neutral {
        return HOST_MICROWAVE_AFFECTS_NEUTRALS;
    }
    if target_is_enemy {
        return HOST_MICROWAVE_AFFECTS_ENEMIES;
    }
    false
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_microwave_range_2d(src: (f32, f32), dst: (f32, f32), range: f32) -> bool {
    let dx = src.0 - dst.0;
    let dy = src.1 - dst.1;
    dx * dx + dy * dy <= range * range
}

/// Whether residual microwave should cook this structure target (attacking + range).
pub fn should_microwave_disable(
    is_microwave: bool,
    microwave_alive: bool,
    microwave_attacking: bool,
    has_target: bool,
    in_range: bool,
    legal_target: bool,
) -> bool {
    is_microwave && microwave_alive && microwave_attacking && has_target && in_range && legal_target
}

/// Whether residual garrison clearer is in range of structure.
pub fn should_microwave_clear_garrison(
    is_microwave: bool,
    microwave_alive: bool,
    in_clear_range: bool,
    legal_structure: bool,
    has_garrison: bool,
) -> bool {
    is_microwave && microwave_alive && in_clear_range && legal_structure && has_garrison
}

/// Emitter cook-field residual: damage at distance from tank center.
pub fn microwave_emitter_damage_at(distance: f32) -> f32 {
    if distance <= HOST_MICROWAVE_EMITTER_RADIUS {
        HOST_MICROWAVE_EMITTER_DAMAGE
    } else {
        0.0
    }
}

/// Host residual honesty counters for microwave tank.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostMicrowaveRegistry {
    /// Times residual applied DISABLED_SUBDUED to a structure (new grants).
    pub disable_grants: u32,
    /// Frames / ticks where at least one structure remained cooked.
    pub disable_ticks: u32,
    /// Structures currently cooked at last update (diagnostic).
    pub currently_disabled: u32,
    /// Wave 55: residual disabler weapon pulses booked.
    pub disable_weapon_pulses: u32,
    /// Wave 55: residual clearer shots booked.
    pub clear_shots: u32,
    /// Wave 55: residual emitter field ticks booked.
    pub emitter_ticks: u32,
    /// Wave 55: ally filter rejections (would-be ally cook blocked).
    pub ally_filter_rejects: u32,
}

impl HostMicrowaveRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_disable_grant(&mut self) {
        self.disable_grants = self.disable_grants.saturating_add(1);
        self.disable_ticks = self.disable_ticks.saturating_add(1);
    }

    pub fn record_disable_refresh(&mut self) {
        self.disable_ticks = self.disable_ticks.saturating_add(1);
    }

    pub fn set_currently_disabled(&mut self, count: u32) {
        self.currently_disabled = count;
    }

    pub fn record_disable_weapon_pulse(&mut self) {
        self.disable_weapon_pulses = self.disable_weapon_pulses.saturating_add(1);
    }

    pub fn record_clear_shot(&mut self) {
        self.clear_shots = self.clear_shots.saturating_add(1);
    }

    pub fn record_emitter_tick(&mut self) {
        self.emitter_ticks = self.emitter_ticks.saturating_add(1);
    }

    pub fn record_ally_filter_reject(&mut self) {
        self.ally_filter_rejects = self.ally_filter_rejects.saturating_add(1);
    }

    /// Residual honesty: at least one structure was disabled by microwave.
    pub fn honesty_disable_ok(&self) -> bool {
        self.disable_grants > 0
    }

    /// Combined host path honesty for microwave disable residual.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_disable_ok()
    }

    /// Wave 55 weapon residual counters honesty.
    pub fn honesty_weapon_residual_ok(&self) -> bool {
        self.disable_weapon_pulses > 0 || self.clear_shots > 0 || self.emitter_ticks > 0
    }
}

// --- Wave 55 residual honesty packs ---

/// Cook radius residual (disabler / clearer / emitter).
pub fn honesty_microwave_cook_radius_residual_ok() -> bool {
    (HOST_MICROWAVE_DISABLE_RANGE - 200.0).abs() < 0.01
        && (HOST_MICROWAVE_CLEAR_RANGE - 125.0).abs() < 0.01
        && HOST_MICROWAVE_DISABLE_RANGE > HOST_MICROWAVE_CLEAR_RANGE
        && (HOST_MICROWAVE_EMITTER_RADIUS - 100.0).abs() < 0.01
        && (HOST_MICROWAVE_EMITTER_RANGE - 100.0).abs() < 0.01
        && (HOST_MICROWAVE_EMITTER_DAMAGE - 8.0).abs() < 0.01
        && (microwave_emitter_damage_at(50.0) - 8.0).abs() < 0.01
        && microwave_emitter_damage_at(150.0).abs() < 0.01
        && in_microwave_range_2d((0.0, 0.0), (200.0, 0.0), HOST_MICROWAVE_DISABLE_RANGE)
        && !in_microwave_range_2d((0.0, 0.0), (201.0, 0.0), HOST_MICROWAVE_DISABLE_RANGE)
}

/// Disable residual (subdual pulse / delay / laser / audio).
pub fn honesty_microwave_disable_residual_ok() -> bool {
    (HOST_MICROWAVE_SUBDUAL_PULSE - 50.0).abs() < 0.01
        && HOST_MICROWAVE_DELAY_MS == 100
        && HOST_MICROWAVE_DELAY_FRAMES == microwave_ms_to_frames(HOST_MICROWAVE_DELAY_MS)
        && HOST_MICROWAVE_DELAY_FRAMES == 3
        && HOST_MICROWAVE_FIRE_SOUND_LOOP_MS == 120
        && HOST_MICROWAVE_FIRE_SOUND_LOOP_FRAMES
            == microwave_ms_to_frames(HOST_MICROWAVE_FIRE_SOUND_LOOP_MS)
        && HOST_MICROWAVE_LASER_NAME == "MicrowaveDisableStream"
        && HOST_MICROWAVE_LASER_BONE == "WEAPON02"
        && HOST_MICROWAVE_DAMAGE_TYPE_SUBDUAL == "SUBDUAL_BUILDING"
        && MICROWAVE_WEAPON_BUILDING_DISABLER == "MicrowaveTankBuildingDisabler"
        && (HOST_MICROWAVE_PRIMARY_RADIUS - 0.0).abs() < 0.01
        && !MICROWAVE_DISABLE_AUDIO.is_empty()
}

/// Ally filter residual (INI affects allies; host residual rejects allies).
pub fn honesty_microwave_ally_filter_residual_ok() -> bool {
    HOST_MICROWAVE_RADIUS_AFFECTS.contains("ALLIES")
        && HOST_MICROWAVE_RADIUS_AFFECTS.contains("ENEMIES")
        && HOST_MICROWAVE_RADIUS_AFFECTS.contains("NEUTRALS")
        && !HOST_MICROWAVE_AFFECTS_ALLIES
        && HOST_MICROWAVE_AFFECTS_ENEMIES
        && HOST_MICROWAVE_AFFECTS_NEUTRALS
        && !microwave_ally_filter_allows(true, false, false)
        && microwave_ally_filter_allows(false, true, false)
        && microwave_ally_filter_allows(false, false, true)
        && !is_microwave_hostile_team(false, true, false)
        && is_microwave_hostile_team(false, false, false)
}

/// Weapon residual damage/rate (clearer + emitter + vehicle disabler off).
pub fn honesty_microwave_weapon_residual_ok() -> bool {
    (HOST_MICROWAVE_CLEAR_PER_SHOT - 1.0).abs() < 0.01
        && HOST_MICROWAVE_DAMAGE_TYPE_CLEAR == "KILL_GARRISONED"
        && MICROWAVE_WEAPON_BUILDING_CLEARER == "MicrowaveTankBuildingClearer"
        && HOST_MICROWAVE_EMITTER_DELAY_MS == 250
        && HOST_MICROWAVE_EMITTER_DELAY_FRAMES
            == microwave_ms_to_frames(HOST_MICROWAVE_EMITTER_DELAY_MS)
        && HOST_MICROWAVE_EMITTER_DELAY_FRAMES == 8
        && HOST_MICROWAVE_DAMAGE_TYPE_EMITTER == "MICROWAVE"
        && HOST_MICROWAVE_DEATH_TYPE_EMITTER == "BURNED"
        && HOST_MICROWAVE_EMITTER_DAMAGE_AT_SELF
        && HOST_MICROWAVE_EMITTER_FX == "FX_MicrowaveTankEmitter"
        && HOST_MICROWAVE_EMITTER_AFFECTS.contains("ENEMIES")
        && HOST_MICROWAVE_EMITTER_AFFECTS.contains("NOT_AIRBORNE")
        && MICROWAVE_WEAPON_EMITTER == "MicrowaveTankEmitterWeapon"
        && !MICROWAVE_WEAPON_VEHICLE_DISABLER_ENABLED
        && MICROWAVE_WEAPON_VEHICLE_DISABLER == "MicrowaveTankVehicleDisabler"
        && (HOST_MICROWAVE_WEAPON_SPEED - 999_999.0).abs() < 0.1
}

/// Combined Wave 55 microwave residual honesty pack.
pub fn honesty_microwave_residual_pack_ok() -> bool {
    honesty_microwave_cook_radius_residual_ok()
        && honesty_microwave_disable_residual_ok()
        && honesty_microwave_ally_filter_residual_ok()
        && honesty_microwave_weapon_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microwave_name_matrix() {
        assert!(is_microwave_tank("AmericaTankMicrowave"));
        assert!(is_microwave_tank("USA_MicrowaveTank"));
        assert!(is_microwave_tank("Lazr_AmericaTankMicrowave"));
        assert!(is_microwave_tank("TestMicrowave"));
        assert!(is_microwave_tank("TestMicrowaveTank"));
        assert!(!is_microwave_tank("MicrowaveDisableStream"));
        assert!(!is_microwave_tank("MicrowaveTankEmitterWeapon"));
        assert!(!is_microwave_tank("USA_Ranger"));
        assert!(!is_microwave_tank("ChinaTankECM"));
        assert!(!is_microwave_tank("TestTank"));
    }

    #[test]
    fn legal_target_and_team_filters() {
        // structure, alive, enemy_or_neutral, under_construction
        assert!(is_legal_microwave_disable_target(true, true, true, false));
        assert!(!is_legal_microwave_disable_target(false, true, true, false));
        assert!(!is_legal_microwave_disable_target(true, false, true, false));
        assert!(!is_legal_microwave_disable_target(true, true, false, false));
        assert!(!is_legal_microwave_disable_target(true, true, true, true));

        assert!(is_microwave_hostile_team(false, false, false)); // enemy
        assert!(is_microwave_hostile_team(false, false, true)); // neutral victim
        assert!(!is_microwave_hostile_team(false, true, false)); // ally
        assert!(!is_microwave_hostile_team(true, false, false)); // neutral microwave
    }

    #[test]
    fn range_and_should_disable() {
        assert!(HOST_MICROWAVE_DISABLE_RANGE > HOST_MICROWAVE_CLEAR_RANGE);
        assert!(in_microwave_range_2d((0.0, 0.0), (150.0, 0.0), 200.0));
        assert!(!in_microwave_range_2d((0.0, 0.0), (250.0, 0.0), 200.0));
        assert!(should_microwave_disable(true, true, true, true, true, true));
        assert!(!should_microwave_disable(
            true, true, false, true, true, true
        ));
        assert!(!should_microwave_disable(
            false, true, true, true, true, true
        ));
    }

    #[test]
    fn honesty_tracks_disable_grants() {
        let mut reg = HostMicrowaveRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_disable_grant();
        assert!(reg.honesty_disable_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.disable_grants, 1);
        reg.record_disable_weapon_pulse();
        reg.record_clear_shot();
        reg.record_emitter_tick();
        reg.record_ally_filter_reject();
        assert!(reg.honesty_weapon_residual_ok());
        assert_eq!(reg.ally_filter_rejects, 1);
    }

    #[test]
    fn microwave_residual_pack_honesty() {
        assert!(honesty_microwave_cook_radius_residual_ok());
        assert!(honesty_microwave_disable_residual_ok());
        assert!(honesty_microwave_ally_filter_residual_ok());
        assert!(honesty_microwave_weapon_residual_ok());
        assert!(honesty_microwave_residual_pack_ok());
        assert_eq!(microwave_ms_to_frames(100), 3);
        assert_eq!(microwave_ms_to_frames(250), 8);
        assert_eq!(microwave_ms_to_frames(120), 4);
        assert!(should_microwave_clear_garrison(
            true, true, true, true, true
        ));
        assert!(!should_microwave_clear_garrison(
            true, true, true, true, false
        ));
    }
}
