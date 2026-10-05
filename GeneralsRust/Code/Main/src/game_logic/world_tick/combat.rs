//! Host combat pass. Named domain modules execute synchronously in the
//! original attacker order; they neither schedule another pass nor retain state.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

mod admission;
mod discharge;
mod ground_target;
mod object_hit;
mod object_target;

#[cfg(test)]
mod damage_promotion_fx_tests;

/// Only facts already copied by the old per-attacker loop. This value is
/// synchronous call input, not a second object state or service context.
#[derive(Clone, Copy)]
struct CombatAttackInput {
    attacker_id: ObjectId,
    current_time: f32,
    attacker_team: Team,
    target_id: Option<ObjectId>,
    target_location: Option<Vec3>,
    overcharge: bool,
}

/// A yielded attacker and an approach-only update both skip shot commit.
/// The marker is set at the original acceptance point, before hit effects.
#[derive(Clone, Copy)]
enum CombatShotResult {
    NoShot,
    Fired(u8),
}
impl GameLogic {
    pub(crate) fn update_combat(&mut self, object_ids: &[ObjectId], _dt: f32) {
        for &attacker_id in object_ids {
            let Some(input) = self.prepare_combat_attack(attacker_id) else {
                continue;
            };
            let shot = if let Some(target_id) = input.target_id {
                self.update_object_target_combat(input, target_id)
            } else if let Some(target_location) = input.target_location {
                self.update_ground_target_combat(input, target_location)
            } else {
                CombatShotResult::NoShot
            };
            if let CombatShotResult::Fired(slot) = shot {
                self.commit_combat_shot(input, slot);
            }
        }

        // AssistedTargeting residual: advance pending Patriot assist clips after
        // primary fire this combat pass (AssistingClipSize / DelayBetweenShots).
        // Wave 824: under coupled shadow, pending patriot assists sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_pending_patriot_assists();
        }
        // BinaryDataStream laser residual: expire DeletionUpdate lifetime beams.
        // Wave 823: under coupled shadow, patriot assist lasers sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_patriot_assist_lasers();
        }
        // Weapon.ini LaserName residual lifetime / scroll.
        crate::game_logic::host_weapon_laser::update_weapon_lasers(
            &mut self.weapon_lasers,
            self.frame,
        );
    }
}
