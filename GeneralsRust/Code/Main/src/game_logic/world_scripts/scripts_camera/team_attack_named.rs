//! CPP AIGroup.cpp2102–2174: per-member passenger, slave, owner attack phases.
use super::*;
impl GameLogic {
    pub(super) fn apply_owned_team_attack_named(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        let Some(target) = self.owned_named_script_object(target, this_object) else {
            return Ok(());
        };
        let Some(position) = self.host_object(target).map(|object| object.get_position()) else {
            return Ok(());
        };
        // Containers without an AI still forward the order. C++ excludes
        // DISABLED_HELD members before sorting; containment alone is not held.
        let mut ordered: Vec<_> = members
            .into_iter()
            .map(ObjectId)
            .filter_map(|id| {
                let object = self.host_object(id)?;
                if object.status.disabled_held {
                    return None;
                }
                let delta = object.get_position() - position;
                Some((id, delta.x * delta.x + delta.z * delta.z))
            })
            .collect();
        ordered.sort_by(|left, right| left.1.total_cmp(&right.1));
        for (id, _) in ordered {
            // Release the owner's borrow before a passenger runs its actual
            // attack algorithm. Later phases reread the current owner.
            let passengers = self.host_object(id).filter(|owner| {
                if owner.is_garrison_contain() {
                    !owner.is_subdued_disabled()
                } else {
                    owner.passengers_allowed_to_fire
                        || crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
                            owner.overlord_bunker_slot_capacity(), owner.contained_by.is_some())
                }
            }).map(|_| self.contained_units_for_interface(id)).unwrap_or_default();
            for passenger in passengers {
                if self
                    .host_object(passenger)
                    .is_some_and(|object| object.has_ai_update_interface())
                    && matches!(
                        self.get_able_to_attack_specific_object(
                            passenger,
                            target,
                            AbleToAttackType::NewTarget,
                            false
                        ),
                        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                    )
                {
                    self.unit_command_script_attack_with_effects(passenger, target, effects)?;
                }
            }
            // Main's physical Stinger roster is the locked SpawnBehavior
            // domain. Free-will AngryMob members receive independent orders.
            if let Some(owner) = self.host_object_mut(id) {
                if owner.hive_slave_count > 0 || owner.hive_slaves.iter().any(|slave| slave.alive) {
                    crate::game_logic::host_base_defense::order_hive_slaves_to_attack_target(
                        &mut owner.hive_slaves,
                        target.0,
                    );
                }
            }
            if id != target
                && self
                    .host_object(id)
                    .is_some_and(|object| object.has_ai_update_interface())
            {
                self.unit_command_script_attack_with_effects(id, target, effects)?;
            }
        }
        Ok(())
    }
}
