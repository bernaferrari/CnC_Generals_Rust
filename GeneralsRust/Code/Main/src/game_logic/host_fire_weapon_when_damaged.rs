//! Host FireWeaponWhenDamagedBehavior residual.
//!
//! C++: `FireWeaponWhenDamagedBehavior::onDamage` + continuous `update`.
//! Fires reaction weapons when damage is taken (thresholded) and optional
//! continuous weapons while body damage state is Damaged/ReallyDamaged/Rubble.
//!
//! Residual playability slice:
//! - Body-state keyed reaction + continuous weapon names
//! - DamageAmount threshold on actual HP lost
//! - Continuous fire rate residual (default 30 frames / 1s)
//! - Splash via GameLogic instant-hit residual (not full forceFireWeapon matrix)
//!
//! Fail-closed:
//! - Not full UpgradeMux StartsActive gating / damage-type flag matrix
//! - Not full WeaponStore ammo / READY_TO_FIRE clip state
//! - Not full projectile Object creation

use crate::game_logic::host_enum_table_residual::{
    host_calc_body_damage_state, HostBodyDamageType,
};
use serde::{Deserialize, Serialize};

/// Default continuous fire interval residual (1 second @ 30 FPS).
pub const FWWDB_CONTINUOUS_RELOAD_FRAMES: u32 = 30;
/// Default reaction debounce residual (avoid multi-fire same frame stacks).
pub const FWWDB_REACTION_DEBOUNCE_FRAMES: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFireWeaponWhenDamagedData {
    pub active: bool,
    /// C++ m_damageAmount residual.
    pub damage_amount: f32,
    pub reaction_pristine: Option<String>,
    pub reaction_damaged: Option<String>,
    pub reaction_really_damaged: Option<String>,
    pub reaction_rubble: Option<String>,
    pub continuous_pristine: Option<String>,
    pub continuous_damaged: Option<String>,
    pub continuous_really_damaged: Option<String>,
    pub continuous_rubble: Option<String>,
    pub last_reaction_frame: u32,
    pub last_continuous_frame: u32,
    pub continuous_reload_frames: u32,
}

impl Default for HostFireWeaponWhenDamagedData {
    fn default() -> Self {
        Self {
            active: true,
            damage_amount: 1.0,
            reaction_pristine: None,
            reaction_damaged: None,
            reaction_really_damaged: None,
            reaction_rubble: None,
            continuous_pristine: None,
            continuous_damaged: None,
            continuous_really_damaged: None,
            continuous_rubble: None,
            last_reaction_frame: 0,
            last_continuous_frame: 0,
            continuous_reload_frames: FWWDB_CONTINUOUS_RELOAD_FRAMES,
        }
    }
}

impl HostFireWeaponWhenDamagedData {
    /// BattleshipTarget residual (AmericaMiscUnit.ini).
    pub fn battleship_target_residual() -> Self {
        let w = "BattleshipTargetDamagedWeapon".to_string();
        Self {
            active: true,
            damage_amount: 1.0,
            reaction_pristine: Some(w.clone()),
            reaction_damaged: Some(w.clone()),
            reaction_really_damaged: Some(w),
            ..Default::default()
        }
    }

    /// Toxic bunker continuous poison residual (CivilianBuilding.ini).
    pub fn toxic_bunker_residual() -> Self {
        Self {
            active: true,
            damage_amount: 1.0,
            continuous_damaged: Some("SmallPoisonFieldWeaponUpgraded".into()),
            continuous_really_damaged: Some("MediumPoisonFieldWeaponUpgraded".into()),
            continuous_reload_frames: FWWDB_CONTINUOUS_RELOAD_FRAMES,
            ..Default::default()
        }
    }

    fn weapon_for_state(&self, state: HostBodyDamageType, continuous: bool) -> Option<&str> {
        let (p, d, r, u) = if continuous {
            (
                self.continuous_pristine.as_deref(),
                self.continuous_damaged.as_deref(),
                self.continuous_really_damaged.as_deref(),
                self.continuous_rubble.as_deref(),
            )
        } else {
            (
                self.reaction_pristine.as_deref(),
                self.reaction_damaged.as_deref(),
                self.reaction_really_damaged.as_deref(),
                self.reaction_rubble.as_deref(),
            )
        };
        match state {
            HostBodyDamageType::Rubble => u.or(r).or(d).or(p),
            HostBodyDamageType::ReallyDamaged => r.or(d).or(p),
            HostBodyDamageType::Damaged => d.or(p),
            HostBodyDamageType::Pristine => p,
        }
    }

    /// C++ onDamage residual. Returns weapon name to force-fire at self position.
    pub fn on_damage(
        &mut self,
        actual_damage: f32,
        health: f32,
        max_health: f32,
        current_frame: u32,
    ) -> Option<String> {
        if !self.active {
            return None;
        }
        if actual_damage < self.damage_amount {
            return None;
        }
        if current_frame.saturating_sub(self.last_reaction_frame) < FWWDB_REACTION_DEBOUNCE_FRAMES
            && self.last_reaction_frame > 0
        {
            return None;
        }
        let state = host_calc_body_damage_state(health, max_health);
        let name = self.weapon_for_state(state, false)?.to_string();
        self.last_reaction_frame = current_frame;
        Some(name)
    }

    /// C++ continuous update residual.
    pub fn tick_continuous(
        &mut self,
        health: f32,
        max_health: f32,
        current_frame: u32,
    ) -> Option<String> {
        if !self.active {
            return None;
        }
        let reload = self.continuous_reload_frames.max(1);
        if self.last_continuous_frame > 0
            && current_frame.saturating_sub(self.last_continuous_frame) < reload
        {
            return None;
        }
        let state = host_calc_body_damage_state(health, max_health);
        // Continuous weapons typically only for damaged+ states in retail toxic bunkers.
        if matches!(state, HostBodyDamageType::Pristine) && self.continuous_pristine.is_none() {
            return None;
        }
        let name = self.weapon_for_state(state, true)?.to_string();
        self.last_continuous_frame = current_frame;
        Some(name)
    }
}

/// Residual splash peel for known FireWeaponWhenDamaged weapons.
/// Returns (primary_damage, primary_radius, secondary_damage, secondary_radius).
pub fn fire_when_damaged_weapon_splash(name: &str) -> (f32, f32, f32, f32) {
    let n = name.to_ascii_lowercase();
    if n.contains("battleshiptargetdamaged") {
        return (100.0, 30.0, 30.0, 65.0);
    }
    if n.contains("mediumpoisonfield") {
        let d = if n.contains("upgraded") { 2.5 } else { 2.0 };
        return (d, 80.0, 0.0, 0.0);
    }
    if n.contains("smallpoisonfield") {
        let d = if n.contains("upgraded") { 2.5 } else { 2.0 };
        let r = if n.contains("upgraded") { 7.5 } else { 12.0 };
        return (d, r, 0.0, 0.0);
    }
    if n.contains("largepoisonfield") {
        return (2.5, 120.0, 0.0, 0.0);
    }
    // Fall back to weapon store radii when available.
    let pr = crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name(name);
    let sr =
        crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name(name);
    let sd = crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name(name);
    let pd = if pr > 0.0 { 25.0 } else { 10.0 };
    (pd, pr.max(5.0), sd, sr)
}

/// Template peel: attach residual config for known FireWeaponWhenDamaged users.
pub fn fire_when_damaged_config_for_template(name: &str) -> Option<HostFireWeaponWhenDamagedData> {
    let n = name.to_ascii_lowercase();
    if n.contains("battleshiptarget") || n.contains("battleship_target") {
        return Some(HostFireWeaponWhenDamagedData::battleship_target_residual());
    }
    if n.contains("toxicbunker")
        || n.contains("toxinbunker")
        || (n.contains("bunker") && n.contains("toxin"))
        || n.contains("demotrap") && n.contains("toxin")
    {
        return Some(HostFireWeaponWhenDamagedData::toxic_bunker_residual());
    }
    // GLA demo trap toxic variants / stinger sites sometimes continuous poison.
    if n.contains("poison") && n.contains("field") && n.contains("generator") {
        return Some(HostFireWeaponWhenDamagedData::toxic_bunker_residual());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_fires_on_threshold_damage() {
        let mut d = HostFireWeaponWhenDamagedData::battleship_target_residual();
        assert!(d.on_damage(0.5, 100.0, 100.0, 1).is_none()); // below threshold
        let w = d.on_damage(5.0, 80.0, 100.0, 2).expect("reaction");
        assert!(w.contains("BattleshipTarget"));
    }

    #[test]
    fn continuous_fires_when_damaged() {
        let mut d = HostFireWeaponWhenDamagedData::toxic_bunker_residual();
        assert!(d.tick_continuous(100.0, 100.0, 1).is_none()); // pristine
        let w = d.tick_continuous(40.0, 100.0, 1).expect("cont");
        assert!(w.contains("PoisonField"));
        // reload gate
        assert!(d.tick_continuous(40.0, 100.0, 2).is_none());
        let w2 = d
            .tick_continuous(40.0, 100.0, 1 + FWWDB_CONTINUOUS_RELOAD_FRAMES)
            .unwrap();
        assert!(w2.contains("PoisonField"));
    }
}
