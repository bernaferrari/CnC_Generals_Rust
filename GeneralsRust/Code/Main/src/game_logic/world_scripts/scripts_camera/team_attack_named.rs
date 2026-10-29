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
            // C++ SpawnBehavior::orderSlavesToAttackTarget walks m_spawnIDs
            // in admission order and calls aiForceAttackObject synchronously.
            let slaves = self
                .host_object(id)
                .map(|owner| owner.spawn_behavior_children())
                .unwrap_or_default();
            for (slave_id, birth) in slaves {
                let admitted_birth = self
                    .host_object(id)
                    .zip(self.host_object(slave_id))
                    .is_some_and(|(owner, slave)| {
                        owner.spawn_behavior_child_birth_matches(slave_id, &birth, slave)
                            && slave.has_ai_update_interface()
                    });
                if admitted_birth && self.unit_command_force_attack_with_effects(
                    slave_id,
                    target,
                    effects,
                )? {
                    self.stinger_slave_order_attack_count =
                        self.stinger_slave_order_attack_count.saturating_add(1);
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
