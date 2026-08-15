//! Residual muzzle/impact spawn for the direct `update_combat` finalizer.
//!
//! Wave-3 moved named FireFX playback to `world_combat::play_dispatch_fire_fx`.
//! That path fail-closes when the weapon has no FireFX list. This file owns the
//! host residual registry entries the combat tests observe for nameless weapons.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Spawn host residual muzzle/impact when visual dispatch will not.
    ///
    /// C++ `Weapon.cpp:904-939`: `FXList::doFXPos` only when `getFireFX` /
    /// `getProjectileDetonateFX` is non-null. `play_dispatch_fire_fx` mirrors
    /// that (`selected_fx_name` empty → return). `Weapon.cpp:889` still calls
    /// `Drawable::handleWeaponFireFX` (`Drawable.cpp:4216`) when FXList is null
    /// so recoil/barrel run; the host `MuzzleFlash`/`BulletImpact` registry is
    /// the residual stand-in for that client fire cue.
    ///
    /// Named FireFX stays on the dispatch path (do not double-spawn).
    /// Stealth gate matches `Weapon.cpp:911-919` (pretend handled, no FX).
    pub(in super::super) fn spawn_residual_muzzle_when_dispatch_has_no_fire_fx(
        &mut self,
        suppress_fire_fx: bool,
        fire_fx: &str,
        det_fx: &str,
        muzzle_pos: Vec3,
        impact_pos: Option<Vec3>,
        fire_frame: u32,
        attacker_id: ObjectId,
        fire_target: Option<ObjectId>,
    ) {
        if suppress_fire_fx || !fire_fx.is_empty() {
            return;
        }
        let _ = self.combat_particles.spawn_weapon_fire_fx_named(
            muzzle_pos,
            impact_pos,
            fire_frame,
            attacker_id,
            fire_target,
            fire_fx,
            det_fx,
        );
    }
}
