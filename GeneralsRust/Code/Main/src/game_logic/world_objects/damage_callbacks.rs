//! Original source-order dispatch for the two supported synchronous families.
use super::*;

enum DamageCallback {
    Minefield,
    ReactionWeapon(u32),
}

impl GameLogic {
    /// Synchronous supported Damage interfaces in the original declaration
    /// order. The vector is one operation's traversal, never stored world state.
    /// None selects the existing uncaptured-template compatibility path.
    pub(super) fn run_authored_mine_and_weapon_damage_callbacks(
        &mut self,
        victim_id: ObjectId,
        actual_damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        healing: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<bool> {
        let object = self.objects.get(&victim_id)?;
        let identity = self.live_draw_playback_identity(object);
        let definition = object
            .thing()
            .template
            .receiving_object_definition
            .as_ref()?;
        let callbacks: Vec<_> = definition
            .behavior_modules
            .iter()
            .enumerate()
            .filter_map(|(index, module)| {
                if module.class_name.eq_ignore_ascii_case("MinefieldBehavior") {
                    Some(DamageCallback::Minefield)
                } else if !healing
                    && module
                        .class_name
                        .eq_ignore_ascii_case("FireWeaponWhenDamagedBehavior")
                {
                    Some(DamageCallback::ReactionWeapon(index as u32))
                } else {
                    None
                }
            })
            .collect();
        let mut ran = false;
        for callback in callbacks {
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == identity)
            {
                break;
            }
            ran = true;
            match callback {
                DamageCallback::Minefield => {
                    let self_drain = source_id == Some(victim_id)
                        && damage_type == crate::game_logic::combat::DamageType::Unresistable;
                    self.synchronize_mine_body_callback_with_effects(
                        victim_id, healing, self_drain, effects,
                    );
                }
                DamageCallback::ReactionWeapon(index) => {
                    self.execute_temporary_weapon_damage_slot_with_effects(
                        victim_id,
                        index,
                        actual_damage,
                        damage_type.to_store() as u32,
                        effects,
                    );
                }
            }
        }
        Some(ran)
    }
}

#[cfg(test)]
#[path = "mine_damage_callback_tests.rs"]
mod tests;
