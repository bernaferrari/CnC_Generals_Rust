//! Live `FireWeaponWhenDamagedBehavior` / `FireWeaponWhenDeadBehavior` fire.
//!
//! C++: `FireWeaponWhenDamagedBehavior.cpp:147-241`,
//! `FireWeaponWhenDeadBehavior.cpp:60-94`.

use super::super::*;
use super::temporary_weapon_status::promote_temporary_weapon_status;
use crate::game_logic::host_enum_table_residual::{
    host_calc_body_damage_state, HostBodyDamageType,
};
use crate::game_logic::host_temporary_weapon_behavior::{
    FireWeaponBodyDamageState, FireWeaponWhenDamagedWeaponRole, TemporaryWeaponRuntimeKey,
    TemporaryWeaponStatus,
};

impl GameLogic {
    pub(in super::super) fn execute_temporary_weapon_on_damage(
        &mut self,
        source: ObjectId,
        actual_damage: f32,
        damage_type_ordinal: u32,
    ) -> u32 {
        let Some(plan) = self.damaged_reaction_plan(source, actual_damage, damage_type_ordinal)
        else {
            return 0;
        };
        self.force_fire_temporary_runtime(source, plan)
    }

    pub(in super::super) fn execute_temporary_weapon_continuous(
        &mut self,
        source: ObjectId,
    ) -> u32 {
        let Some(plan) = self.damaged_continuous_plan(source) else {
            return 0;
        };
        self.force_fire_temporary_runtime(source, plan)
    }

    pub(in super::super) fn execute_temporary_weapon_on_die(&mut self, source: ObjectId) -> u32 {
        let Some(spec) = self.dead_ephemeral_spec(source) else {
            return 0;
        };
        if let Some(object) = self.objects.get_mut(&source) {
            object.fire_weapon_when_dead_fired = true;
        }
        self.create_and_fire_temp_weapon(source, &spec)
    }

    pub(in super::super) fn fire_temporary_weapons_for_pending_deaths(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, object)| {
                (object.status.destroyed || object.health.current <= 0.0).then_some(*id)
            })
            .collect();
        for id in ids {
            let _ = self.execute_temporary_weapon_on_die(id);
        }
    }

    fn damaged_reaction_plan(
        &mut self,
        source: ObjectId,
        actual_damage: f32,
        damage_type_ordinal: u32,
    ) -> Option<TemporaryWeaponRuntimeKey> {
        let object = self.objects.get_mut(&source)?;
        let body = body_state_of(object);
        let logic_frame = self.frame;
        let mut chosen = None;
        for (runtime, metadata) in object.temporary_weapon_runtime.damaged.iter_mut().zip(
            object
                .thing
                .template
                .fire_weapon_when_damaged_behaviors
                .iter(),
        ) {
            if metadata.starts_active {
                runtime.upgrade_executed = true;
            }
            if !runtime.upgrade_executed {
                continue;
            }
            if !metadata.damage_types.contains_ordinal(damage_type_ordinal) {
                continue;
            }
            if actual_damage < metadata.damage_amount {
                continue;
            }
            let role = reaction_role(body);
            let key = TemporaryWeaponRuntimeKey {
                module_source_index: runtime.module_source_index,
                role,
            };
            let Some(weapon) = runtime.weapon_mut(key) else {
                continue;
            };
            if promote_temporary_weapon_status(weapon, logic_frame)
                == TemporaryWeaponStatus::ReadyToFire
            {
                chosen = Some(key);
                break;
            }
        }
        chosen
    }

    fn damaged_continuous_plan(&mut self, source: ObjectId) -> Option<TemporaryWeaponRuntimeKey> {
        let object = self.objects.get_mut(&source)?;
        let body = body_state_of(object);
        let logic_frame = self.frame;
        let mut chosen = None;
        for (runtime, metadata) in object.temporary_weapon_runtime.damaged.iter_mut().zip(
            object
                .thing
                .template
                .fire_weapon_when_damaged_behaviors
                .iter(),
        ) {
            if metadata.starts_active {
                runtime.upgrade_executed = true;
            }
            if !runtime.upgrade_executed {
                continue;
            }
            let role = continuous_role(body);
            let key = TemporaryWeaponRuntimeKey {
                module_source_index: runtime.module_source_index,
                role,
            };
            let Some(weapon) = runtime.weapon_mut(key) else {
                continue;
            };
            if promote_temporary_weapon_status(weapon, logic_frame)
                == TemporaryWeaponStatus::ReadyToFire
            {
                chosen = Some(key);
                break;
            }
        }
        chosen
    }

    fn dead_ephemeral_spec(
        &mut self,
        source: ObjectId,
    ) -> Option<
        crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec,
    > {
        let object = self.objects.get_mut(&source)?;
        if object.fire_weapon_when_dead_fired || object.status.under_construction {
            return None;
        }
        let death_ordinal = u32::from(object.status.death_type.ordinal());
        let veterancy_ordinal = object.experience.level as u32;
        let statuses = object.object_status_bits;
        let mut spec = None;
        for (runtime, metadata) in object
            .temporary_weapon_runtime
            .dead
            .iter_mut()
            .zip(object.thing.template.fire_weapon_when_dead_behaviors.iter())
        {
            if metadata.starts_active {
                runtime.upgrade_executed = true;
            }
            if !runtime.upgrade_executed {
                continue;
            }
            if !metadata.die_mux_allows(death_ordinal, veterancy_ordinal, statuses) {
                continue;
            }
            if !metadata.upgrade_mux.conflicts_with.is_empty() {
                continue;
            }
            spec = metadata.ephemeral_weapon_spec();
            if spec.is_some() {
                break;
            }
        }
        spec
    }
}

fn body_state_of(object: &Object) -> FireWeaponBodyDamageState {
    match host_calc_body_damage_state(
        object.health.current,
        object.health.maximum.max(object.max_health).max(1.0),
    ) {
        HostBodyDamageType::Pristine => FireWeaponBodyDamageState::Pristine,
        HostBodyDamageType::Damaged => FireWeaponBodyDamageState::Damaged,
        HostBodyDamageType::ReallyDamaged => FireWeaponBodyDamageState::ReallyDamaged,
        HostBodyDamageType::Rubble => FireWeaponBodyDamageState::Rubble,
    }
}

const fn reaction_role(body: FireWeaponBodyDamageState) -> FireWeaponWhenDamagedWeaponRole {
    match body {
        FireWeaponBodyDamageState::Pristine => FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        FireWeaponBodyDamageState::Damaged => FireWeaponWhenDamagedWeaponRole::ReactionDamaged,
        FireWeaponBodyDamageState::ReallyDamaged => {
            FireWeaponWhenDamagedWeaponRole::ReactionReallyDamaged
        }
        FireWeaponBodyDamageState::Rubble => FireWeaponWhenDamagedWeaponRole::ReactionRubble,
    }
}

const fn continuous_role(body: FireWeaponBodyDamageState) -> FireWeaponWhenDamagedWeaponRole {
    match body {
        FireWeaponBodyDamageState::Pristine => FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        FireWeaponBodyDamageState::Damaged => FireWeaponWhenDamagedWeaponRole::ContinuousDamaged,
        FireWeaponBodyDamageState::ReallyDamaged => {
            FireWeaponWhenDamagedWeaponRole::ContinuousReallyDamaged
        }
        FireWeaponBodyDamageState::Rubble => FireWeaponWhenDamagedWeaponRole::ContinuousRubble,
    }
}
