//! CPP AIGroup.cpp2102–2174: per-member passenger, slave, owner attack phases.
use super::*;
use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
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
            let contain_owner = self.redirected_containment_owner(id);
            let (may_fire, helix_rider) = self.host_object(contain_owner).map(|owner| {
                let nested = ((owner.is_overlord_style_container() || owner.is_helix_transport)
                    && owner.contained_by.is_some())
                    || owner.contained_by.and_then(|id| self.host_object(id))
                        .is_some_and(|outer| outer.is_overlord_style_container() && outer.contained_by.is_some());
                let ordinary = !nested && if owner.is_garrison_contain() {
                    !owner.is_subdued_disabled()
                } else {
                    owner.passengers_allowed_to_fire
                };
                let rider = if owner.is_helix_transport && owner.contained_by.is_none() {
                    owner.overlord_portable_occupant
                } else {
                    None
                };
                (ordinary, rider)
            }).unwrap_or((false, None));
            let passengers = self.contained_units_for_interface(id);
            for passenger in passengers {
                if !may_fire && helix_rider != Some(passenger) {
                    continue;
                }
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
            let ordered = self.order_spawn_children_to_attack_target(
                id,
                target,
                HUNT_CMD_FROM_SCRIPT,
                effects,
            )?;
            self.stinger_slave_order_attack_count =
                self.stinger_slave_order_attack_count.saturating_add(ordered);
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
