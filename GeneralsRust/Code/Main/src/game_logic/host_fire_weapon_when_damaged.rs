//! Host FireWeaponWhenDamagedBehavior residual.
//!
//! C++: `FireWeaponWhenDamagedBehavior::onDamage` + continuous `update`.
//! Fires reaction weapons when damage is taken (thresholded) and optional
//! continuous weapons while body damage state is Damaged/ReallyDamaged/Rubble.
//!
//! Residual playability slice:
//! - Exact body-state reaction + continuous weapon names (no cross-state fallback)
//! - DamageAmount threshold and authored DamageTypes mask
//! - Continuous rate from weapon DelayBetweenShots; ctor matches reloadAmmo
//! - Splash via GameLogic instant-hit residual (not full forceFireWeapon matrix)
//!
//! Fail-closed:
//! - Not full UpgradeMux StartsActive gating
//! - Reaction READY_TO_FIRE models per-role ClipSize / ClipReloadTime /
//!   AutoReloadsClip (Weapon.cpp privateFireWeapon 2622-2648, getStatus 2736)
//! - Not full projectile Object creation

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_temporary_weapon_behavior::FireWeaponDamageTypeMask;
use serde::{Deserialize, Serialize};

/// Fallback continuous interval when the weapon store has no DelayBetweenShots.
pub const FWWDB_CONTINUOUS_RELOAD_FRAMES: u32 = 30;

fn leftover_weapon_reload_frames(name: &str) -> u32 {
    crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_nominal(name)
        .map(|secs| (secs * 30.0).round() as u32)
        .filter(|&frames| frames > 0)
        .unwrap_or(FWWDB_CONTINUOUS_RELOAD_FRAMES)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFireWeaponWhenDamagedData {
    pub active: bool,
    /// C++ m_damageAmount residual.
    pub damage_amount: f32,
    /// C++ `m_damageTypes` (default ALL).
    #[serde(default)]
    pub damage_types: FireWeaponDamageTypeMask,
    pub reaction_pristine: Option<String>,
    pub reaction_damaged: Option<String>,
    pub reaction_really_damaged: Option<String>,
    pub reaction_rubble: Option<String>,
    pub continuous_pristine: Option<String>,
    pub continuous_damaged: Option<String>,
    pub continuous_really_damaged: Option<String>,
    pub continuous_rubble: Option<String>,
    pub last_reaction_frame: u32,
    /// C++ FireWeaponWhenDamagedBehavior.cpp:37-72: one reaction Weapon per
    /// body state, each with its own READY_TO_FIRE. Indexed by HostBodyDamageType.
    #[serde(default)]
    pub reaction_role_frames: [u32; 4],
    /// C++ Weapon::m_whenWeCanFireAgain per reaction role (0 = ready).
    #[serde(default)]
    pub reaction_role_ready_frames: [u32; 4],
    /// Shots fired from the current clip per reaction role (C++ ClipSize -
    /// m_ammoInClip); u32::MAX marks OUT_OF_AMMO without AutoReloadsClip.
    #[serde(default)]
    pub reaction_role_clip_used: [u32; 4],
    /// C++ per-role continuous Weapon m_whenWeCanFireAgain (FWWDB.cpp:76-99).
    #[serde(default)]
    pub continuous_role_ready_frames: [u32; 4],
    /// Continuous per-role clip usage; u32::MAX = OUT_OF_AMMO.
    #[serde(default)]
    pub continuous_role_clip_used: [u32; 4],
    pub last_continuous_frame: u32,
    pub continuous_reload_frames: u32,
}

impl Default for HostFireWeaponWhenDamagedData {
    fn default() -> Self {
        Self {
            active: true,
            damage_amount: 1.0,
            damage_types: FireWeaponDamageTypeMask::ALL,
            reaction_pristine: None,
            reaction_damaged: None,
            reaction_really_damaged: None,
            reaction_rubble: None,
            continuous_pristine: None,
            continuous_damaged: None,
            continuous_really_damaged: None,
            continuous_rubble: None,
            last_reaction_frame: 0,
            reaction_role_frames: [0; 4],
            reaction_role_ready_frames: [0; 4],
            reaction_role_clip_used: [0; 4],
            continuous_role_ready_frames: [0; 4],
            continuous_role_clip_used: [0; 4],
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
        let damaged = "SmallPoisonFieldWeaponUpgraded";
        Self {
            active: true,
            damage_amount: 1.0,
            continuous_damaged: Some(damaged.into()),
            continuous_really_damaged: Some("MediumPoisonFieldWeaponUpgraded".into()),
            continuous_reload_frames: leftover_weapon_reload_frames(damaged),
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
            HostBodyDamageType::Rubble => u,
            HostBodyDamageType::ReallyDamaged => r,
            HostBodyDamageType::Damaged => d,
            HostBodyDamageType::Pristine => p,
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior ctor (FireWeaponWhenDamagedBehavior.cpp:50-73)
    /// calls reloadAmmo on each reaction weapon. Weapon ctor leaves
    /// m_ammoInClip = 0 (Weapon.cpp:1730-1732), so reloadWithBonus
    /// (Weapon.cpp:1877-1891) never early-returns and sets
    /// m_whenWeCanFireAgain = now + ClipReloadTime: the first shot waits.
    pub fn apply_ctor_reload_ammo(&mut self, creation_frame: u32) {
        let roles = [
            self.reaction_pristine.clone(),
            self.reaction_damaged.clone(),
            self.reaction_really_damaged.clone(),
            self.reaction_rubble.clone(),
        ];
        for (role, name) in roles.iter().enumerate() {
            let Some(name) = name else { continue };
            let (_, clip_reload, _) = reaction_clip_params(name);
            self.reaction_role_clip_used[role] = 0;
            self.reaction_role_ready_frames[role] = creation_frame + clip_reload;
        }
        // Continuous weapons get the same ctor reloadAmmo (FWWDB.cpp:76-99).
        let roles = [
            self.continuous_pristine.clone(),
            self.continuous_damaged.clone(),
            self.continuous_really_damaged.clone(),
            self.continuous_rubble.clone(),
        ];
        for (role, name) in roles.iter().enumerate() {
            let Some(name) = name else { continue };
            let (_, clip_reload, _) = reaction_clip_params(name);
            self.continuous_role_clip_used[role] = 0;
            self.continuous_role_ready_frames[role] = creation_frame + clip_reload;
        }
    }

    /// C++ onDamage residual. Returns weapon name to force-fire at self position.
    /// Leftover/C++ gate only on READY_TO_FIRE (weapon DelayBetweenShots). There
    /// is no extra 1-frame reaction debounce — a second qualifying hit on the
    /// same or next frame fires again when DelayBetweenShots is 0.
    pub fn on_damage(
        &mut self,
        actual_damage: f32,
        state: HostBodyDamageType,
        current_frame: u32,
        damage_type_ordinal: u32,
    ) -> Option<String> {
        if !self.active {
            return None;
        }
        if !self.damage_types.contains_ordinal(damage_type_ordinal) {
            return None;
        }
        if actual_damage < self.damage_amount {
            return None;
        }
        let name = self.weapon_for_state(state, false)?.to_string();
        // C++ getStatus()==READY_TO_FIRE on this role's own Weapon
        // (FireWeaponWhenDamagedBehavior.cpp:167-191). DelayBetweenShots 0 stays ready.
        let role = role_index(state);
        if !role_try_fire(
            &name,
            current_frame,
            &mut self.reaction_role_ready_frames[role],
            &mut self.reaction_role_clip_used[role],
            0,
        ) {
            return None;
        }
        self.reaction_role_frames[role] = current_frame.max(1);
        self.last_reaction_frame = current_frame.max(1);
        Some(name)
    }

    /// C++ continuous update residual.
    pub fn tick_continuous(
        &mut self,
        state: HostBodyDamageType,
        current_frame: u32,
    ) -> Option<String> {
        if !self.active {
            return None;
        }
        let name = self.weapon_for_state(state, true)?.to_string();
        let role = role_index(state);
        // FWWDB.cpp:200-241: fire only when this role's Weapon is READY_TO_FIRE.
        if !role_try_fire(
            &name,
            current_frame,
            &mut self.continuous_role_ready_frames[role],
            &mut self.continuous_role_clip_used[role],
            FWWDB_CONTINUOUS_RELOAD_FRAMES,
        ) {
            return None;
        }
        self.last_continuous_frame = current_frame.max(1);
        self.continuous_reload_frames = leftover_weapon_reload_frames(&name).max(1);
        Some(name)
    }
}

fn role_index(state: HostBodyDamageType) -> usize {
    match state {
        HostBodyDamageType::Pristine => 0,
        HostBodyDamageType::Damaged => 1,
        HostBodyDamageType::ReallyDamaged => 2,
        HostBodyDamageType::Rubble => 3,
    }
}

/// Weapon::getStatus (Weapon.cpp:2736-2751) READY_TO_FIRE gate + privateFireWeapon
/// (Weapon.cpp:2622-2648) ammo spend for one role Weapon. Returns true if it fired.
/// `missing_delay`: DelayBetweenShots frames when the store lacks the weapon.
fn role_try_fire(
    name: &str,
    current_frame: u32,
    ready: &mut u32,
    used: &mut u32,
    missing_delay: u32,
) -> bool {
    if *used == u32::MAX || current_frame < *ready {
        return false;
    }
    let (clip_size, clip_reload, auto_reload) = reaction_clip_params(name);
    let delay = crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_nominal(name)
        .map(|secs| (secs * 30.0).round() as u32)
        .unwrap_or(missing_delay);
    // --ammo; empty clip reloads (ClipReloadTime) or goes OUT_OF_AMMO;
    // else BETWEEN_FIRING_SHOTS.
    let next = *used + 1;
    if clip_size > 0 && next >= clip_size {
        if auto_reload {
            *used = 0;
            *ready = current_frame + clip_reload;
        } else {
            *used = u32::MAX;
        }
    } else {
        *used = if clip_size > 0 { next } else { 0 };
        *ready = current_frame + delay;
    }
    true
}

/// (ClipSize, ClipReloadTime frames, AutoReloadsClip) from the WeaponStore.
/// Missing template: unlimited clip (Weapon.cpp reloadWithBonus 0 == unlimited).
fn reaction_clip_params(name: &str) -> (u32, u32, bool) {
    gamelogic::weapon::with_weapon_store(|store| {
        store.find_weapon_template(name).map(|t| {
            (
                t.get_clip_size().max(0) as u32,
                t.clip_reload_time.max(0) as u32,
                t.get_auto_reloads_clip(),
            )
        })
    })
    .ok()
    .flatten()
    .unwrap_or((0, 0, true))
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
        assert!(
            d.on_damage(0.5, HostBodyDamageType::Pristine, 1, 0)
                .is_none()
        ); // below threshold
        let w = d
            .on_damage(5.0, HostBodyDamageType::Pristine, 2, 0)
            .expect("reaction");
        assert!(w.contains("BattleshipTarget"));
        // Exact body slot: rubble has no authored reaction.
        assert!(
            d.on_damage(5.0, HostBodyDamageType::Rubble, 10, 0)
                .is_none()
        );
        // Authored type filter rejects non-matching ordinals.
        d.damage_types = FireWeaponDamageTypeMask::NONE;
        assert!(
            d.on_damage(5.0, HostBodyDamageType::Pristine, 20, 0)
                .is_none()
        );
    }

    #[test]
    fn reaction_fires_every_qualifying_hit_without_frame_debounce() {
        let mut d = HostFireWeaponWhenDamagedData::battleship_target_residual();
        let first = d
            .on_damage(5.0, HostBodyDamageType::Pristine, 2, 0)
            .expect("first");
        let same_frame = d
            .on_damage(5.0, HostBodyDamageType::Pristine, 2, 0)
            .expect("same frame");
        let next_frame = d
            .on_damage(5.0, HostBodyDamageType::Pristine, 3, 0)
            .expect("next frame");
        assert!(first.contains("BattleshipTarget"));
        assert!(same_frame.contains("BattleshipTarget"));
        assert!(next_frame.contains("BattleshipTarget"));
    }

    #[test]
    fn continuous_fires_when_damaged() {
        let mut d = HostFireWeaponWhenDamagedData::toxic_bunker_residual();
        d.apply_ctor_reload_ammo(1);
        assert!(d.tick_continuous(HostBodyDamageType::Pristine, 1).is_none()); // pristine
        let reload = d.continuous_reload_frames.max(1);
        // No ClipReloadTime on the unresolved template: ready after ctor.
        let w = d
            .tick_continuous(HostBodyDamageType::Damaged, 1)
            .expect("cont");
        assert!(w.contains("PoisonField"));
        // DelayBetweenShots gate
        assert!(
            d.tick_continuous(HostBodyDamageType::Damaged, reload)
                .is_none()
        );
        let w2 = d
            .tick_continuous(HostBodyDamageType::Damaged, 1 + reload)
            .unwrap();
        assert!(w2.contains("PoisonField"));
    }

    #[test]
    fn reaction_roles_have_independent_weapon_readiness() {
        // C++ FireWeaponWhenDamagedBehavior.cpp:37-72 allocates one Weapon per
        // reaction role; onDamage (167-191) checks only that role's
        // READY_TO_FIRE. A Damaged shot must not put Pristine on cooldown.
        // WeaponStore is per-world; publish a services bag for the lookup.
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            const NAME: &str = "__RustFwwdPerRoleDelay";
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
                let mut t = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
                t.primary_damage = 10.0;
                t.min_delay_between_shots = 60;
                t.max_delay_between_shots = 60;
                store.add_weapon_template(t);
            });
            let mut d = HostFireWeaponWhenDamagedData {
                reaction_pristine: Some(NAME.into()),
                reaction_damaged: Some(NAME.into()),
                ..Default::default()
            };
            assert!(d.on_damage(5.0, HostBodyDamageType::Damaged, 10, 0).is_some());
            assert!(
                d.on_damage(5.0, HostBodyDamageType::Damaged, 11, 0).is_none(),
                "same role reloading"
            );
            assert!(
                d.on_damage(5.0, HostBodyDamageType::Pristine, 11, 0).is_some(),
                "other role has its own ready Weapon"
            );
            assert!(d.on_damage(5.0, HostBodyDamageType::Damaged, 70, 0).is_some());
        });
    }

    #[test]
    fn reaction_clip_reloads_and_out_of_ammo_gate_ready_to_fire() {
        // C++ Weapon.cpp:2622-2648 privateFireWeapon decrements m_ammoInClip;
        // an empty clip reloads for ClipReloadTime (AutoReloadsClip) or stays
        // OUT_OF_AMMO, and getStatus (2736) only reports READY_TO_FIRE with ammo.
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            const AUTO: &str = "__RustFwwdClipAuto";
            const ONCE: &str = "__RustFwwdClipOnce";
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
                for (name, auto) in [(AUTO, true), (ONCE, false)] {
                    let mut t = gamelogic::weapon::WeaponTemplate::new(name.to_string());
                    t.primary_damage = 10.0;
                    t.clip_size = 2;
                    t.clip_reload_time = 90;
                    if !auto {
                        t.reload_type = gamelogic::weapon::WeaponReloadType::NoReload;
                    } else {
                        t.reload_type = gamelogic::weapon::WeaponReloadType::AutoReload;
                    }
                    store.add_weapon_template(t);
                }
            });
            let mut d = HostFireWeaponWhenDamagedData {
                reaction_pristine: Some(AUTO.into()),
                reaction_damaged: Some(ONCE.into()),
                ..Default::default()
            };
            let p = HostBodyDamageType::Pristine;
            assert!(d.on_damage(5.0, p, 10, 0).is_some());
            assert!(d.on_damage(5.0, p, 10, 0).is_some(), "second round in clip");
            assert!(d.on_damage(5.0, p, 11, 0).is_none(), "clip reloading");
            assert!(d.on_damage(5.0, p, 99, 0).is_none(), "clip reloading");
            assert!(d.on_damage(5.0, p, 100, 0).is_some(), "clip reloaded");
            let dm = HostBodyDamageType::Damaged;
            assert!(d.on_damage(5.0, dm, 10, 0).is_some());
            assert!(d.on_damage(5.0, dm, 10, 0).is_some());
            assert!(d.on_damage(5.0, dm, 1000, 0).is_none(), "OUT_OF_AMMO");
        });
    }

    #[test]
    fn ctor_reload_ammo_delays_first_reaction_by_clip_reload_time() {
        // C++ ctor reloadAmmo (FireWeaponWhenDamagedBehavior.cpp:50-73,
        // Weapon.cpp:1877-1891): m_whenWeCanFireAgain = ctor frame + ClipReloadTime,
        // so getStatus (2736-2751) is not READY_TO_FIRE until then.
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            const W: &str = "__RustFwwdCtorReload";
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
                let mut t = gamelogic::weapon::WeaponTemplate::new(W.to_string());
                t.primary_damage = 10.0;
                t.clip_size = 2;
                t.clip_reload_time = 60;
                t.reload_type = gamelogic::weapon::WeaponReloadType::AutoReload;
                store.add_weapon_template(t);
            });
            let mut d = HostFireWeaponWhenDamagedData {
                reaction_pristine: Some(W.into()),
                reaction_damaged: Some(W.into()),
                ..Default::default()
            };
            d.apply_ctor_reload_ammo(100);
            let p = HostBodyDamageType::Pristine;
            assert!(d.on_damage(5.0, p, 100, 0).is_none(), "ctor ClipReloadTime pending");
            assert!(d.on_damage(5.0, p, 159, 0).is_none(), "ctor ClipReloadTime pending");
            assert!(d.on_damage(5.0, p, 160, 0).is_some(), "ready at ctor + ClipReloadTime");
            // Clip was filled by ctor reload: second round still in clip.
            assert!(d.on_damage(5.0, p, 160, 0).is_some(), "full clip after ctor reload");
            // Each role's Weapon reloads independently in the ctor.
            let dm = HostBodyDamageType::Damaged;
            assert!(d.on_damage(5.0, dm, 120, 0).is_none());
            assert!(d.on_damage(5.0, dm, 160, 0).is_some());
        });
    }

    #[test]
    fn continuous_roles_use_clip_ammo_ready_to_fire() {
        // C++ FireWeaponWhenDamagedBehavior.cpp:76-99 allocates + reloadAmmo one
        // Weapon per continuous role; update (200-241) fires only when that
        // role's getStatus()==READY_TO_FIRE (Weapon.cpp:2736), and
        // privateFireWeapon (2622-2648) spends clip ammo / reloads / OUT_OF_AMMO.
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            const AUTO: &str = "__RustFwwdContClipAuto";
            const ONCE: &str = "__RustFwwdContClipOnce";
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
                for (name, auto) in [(AUTO, true), (ONCE, false)] {
                    let mut t = gamelogic::weapon::WeaponTemplate::new(name.to_string());
                    t.primary_damage = 10.0;
                    t.clip_size = 2;
                    t.clip_reload_time = 90;
                    t.min_delay_between_shots = 10;
                    t.max_delay_between_shots = 10;
                    t.reload_type = if auto {
                        gamelogic::weapon::WeaponReloadType::AutoReload
                    } else {
                        gamelogic::weapon::WeaponReloadType::NoReload
                    };
                    store.add_weapon_template(t);
                }
            });
            let mut d = HostFireWeaponWhenDamagedData {
                continuous_damaged: Some(AUTO.into()),
                continuous_really_damaged: Some(ONCE.into()),
                ..Default::default()
            };
            d.apply_ctor_reload_ammo(0);
            let dm = HostBodyDamageType::Damaged;
            assert!(d.tick_continuous(dm, 89).is_none(), "ctor ClipReloadTime pending");
            assert!(d.tick_continuous(dm, 90).is_some(), "ready after ctor reload");
            assert!(d.tick_continuous(dm, 99).is_none(), "DelayBetweenShots");
            assert!(d.tick_continuous(dm, 100).is_some(), "second round");
            assert!(d.tick_continuous(dm, 110).is_none(), "clip reloading");
            assert!(d.tick_continuous(dm, 189).is_none(), "clip reloading");
            assert!(d.tick_continuous(dm, 190).is_some(), "clip reloaded");
            // Independent role Weapon: own ctor reload, own clip, OUT_OF_AMMO.
            let rd = HostBodyDamageType::ReallyDamaged;
            assert!(d.tick_continuous(rd, 191).is_some(), "own READY_TO_FIRE");
            assert!(d.tick_continuous(rd, 201).is_some());
            assert!(d.tick_continuous(rd, 5000).is_none(), "OUT_OF_AMMO");
        });
    }
}
