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
enum CombatShotResult {
    NoShot,
    Failed(gamelogic::GameLogicError),
    Fired(
        u8,
        Option<super::super::world_combat::AcceptedWeaponDischarge>,
    ),
}
impl GameLogic {
    pub(crate) fn update_combat(
        &mut self,
        object_ids: &[ObjectId],
        _dt: f32,
    ) -> gamelogic::GameLogicResult<()> {
        self.update_combat_with_effects(object_ids, _dt, &mut Default::default())
    }

    pub(crate) fn update_combat_with_effects(
        &mut self,
        object_ids: &[ObjectId],
        _dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        for &attacker_id in object_ids {
            let Some(input) = self.prepare_combat_attack_with_effects(attacker_id, effects)? else {
                continue;
            };
            let shot = if let Some(target_id) = input.target_id {
                self.update_object_target_combat_with_effects(input, target_id, effects)
            } else if let Some(target_location) = input.target_location {
                self.update_ground_target_combat_with_effects(input, target_location, effects)
            } else {
                CombatShotResult::NoShot
            };
            match shot {
                CombatShotResult::Failed(error) => return Err(error),
                CombatShotResult::Fired(slot, visual) => {
                    self.commit_combat_shot_with_effects(input, slot, visual, effects);
                }
                CombatShotResult::NoShot => {}
            }
        }

        // AssistedTargeting residual: advance pending Patriot assist clips after
        // primary fire this combat pass (AssistingClipSize / DelayBetweenShots).
        // Wave 824: under coupled shadow, pending patriot assists sole-tick after GW writeback.
        {
            self.update_pending_patriot_assists_with_effects(effects);
        }
        // BinaryDataStream laser residual: expire DeletionUpdate lifetime beams.
        // Wave 823: under coupled shadow, patriot assist lasers sole-tick after GW writeback.
        {
            self.update_patriot_assist_lasers();
        }
        // Weapon.ini LaserName residual lifetime / scroll.
        crate::game_logic::host_weapon_laser::update_weapon_lasers(
            &mut self.weapon_lasers,
            self.frame,
        );
        Ok(())
    }
}

#[cfg(test)]
mod firing_clock_tests;

#[cfg(test)]
#[path = "combat/selected_weapon_owner_tests.rs"]
mod selected_weapon_owner_tests;
