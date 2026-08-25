//! Live `FireWeaponWhenDamagedBehavior` / `FireWeaponWhenDeadBehavior` fire.
//!
//! C++: `FireWeaponWhenDamagedBehavior.cpp:147-241`,
//! `FireWeaponWhenDeadBehavior.cpp:60-94`.

use super::super::*;
use super::temporary_weapon_status::promote_temporary_weapon_status;
use crate::game_logic::host_enum_table_residual::{
    HostBodyDamageType, host_calc_body_damage_state,
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
        // C++ Object::onDamage delivers to every damage module; leftover
        // FireWeaponWhenDamagedBehavior::on_damage fires each instance whose
        // reaction weapon is READY_TO_FIRE. Do not stop at the first ready.
        let plans = self.damaged_reaction_plan(source, actual_damage, damage_type_ordinal);
        let mut hits = 0u32;
        for plan in plans {
            hits = hits.saturating_add(self.force_fire_temporary_runtime(source, plan));
        }
        hits
    }

    pub(in super::super) fn execute_temporary_weapon_continuous(
        &mut self,
        source: ObjectId,
    ) -> u32 {
        // C++ UpdateModule::update walks every instance independently.
        let plans = self.damaged_continuous_plan(source);
        let mut hits = 0u32;
        for plan in plans {
            hits = hits.saturating_add(self.force_fire_temporary_runtime(source, plan));
        }
        hits
    }

    pub(in super::super) fn execute_temporary_weapon_on_die(&mut self, source: ObjectId) -> u32 {
        let specs = self.dead_ephemeral_specs(source);
        if specs.is_empty() {
            return 0;
        }
        let mut hits = 0u32;
        let mut fired = false;
        for spec in specs {
            if let Some(fired_hits) = self.create_and_fire_temp_weapon(source, &spec) {
                fired = true;
                hits = hits.saturating_add(fired_hits);
            }
        }
        // C++ has no leftover fired latch. Stamp only after at least one
        // module actually entered createAndFireTempWeapon so a store miss
        // still lets the residual HE/Bio blast run.
        if fired {
            if let Some(object) = self.objects.get_mut(&source) {
                object.fire_weapon_when_dead_fired = true;
            }
        }
        hits
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

    pub(in super::super) fn damaged_reaction_plan(
        &mut self,
        source: ObjectId,
        actual_damage: f32,
        damage_type_ordinal: u32,
    ) -> Vec<TemporaryWeaponRuntimeKey> {
        let Some((object_tags, player_tags)) = self.fire_when_owned_upgrade_tags(source) else {
            return Vec::new();
        };
        let Some(object) = self.objects.get_mut(&source) else {
            return Vec::new();
        };
        let body = body_state_of(object);
        let logic_frame = self.frame;
        let owned = owned_upgrade_tag_refs(&object_tags, &player_tags);
        let mut chosen = Vec::new();
        for (runtime, metadata) in object.temporary_weapon_runtime.damaged.iter_mut().zip(
            object
                .thing
                .template
                .fire_weapon_when_damaged_behaviors
                .iter(),
        ) {
            if metadata.starts_active || metadata.upgrade_mux.triggered_by_owned(&owned) {
                runtime.upgrade_executed = true;
            }
            if metadata.upgrade_mux.conflicts_with_owned(&owned) {
                continue;
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
                chosen.push(key);
            }
        }
        chosen
    }

    pub(in super::super) fn damaged_continuous_plan(
        &mut self,
        source: ObjectId,
    ) -> Vec<TemporaryWeaponRuntimeKey> {
        let Some((object_tags, player_tags)) = self.fire_when_owned_upgrade_tags(source) else {
            return Vec::new();
        };
        let Some(object) = self.objects.get_mut(&source) else {
            return Vec::new();
        };
        let body = body_state_of(object);
        let logic_frame = self.frame;
        let owned = owned_upgrade_tag_refs(&object_tags, &player_tags);
        let mut chosen = Vec::new();
        for (runtime, metadata) in object.temporary_weapon_runtime.damaged.iter_mut().zip(
            object
                .thing
                .template
                .fire_weapon_when_damaged_behaviors
                .iter(),
        ) {
            if metadata.starts_active || metadata.upgrade_mux.triggered_by_owned(&owned) {
                runtime.upgrade_executed = true;
            }
            if metadata.upgrade_mux.conflicts_with_owned(&owned) {
                continue;
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
                chosen.push(key);
            }
        }
        chosen
    }

    fn fire_when_owned_upgrade_tags(&self, source: ObjectId) -> Option<(Vec<String>, Vec<String>)> {
        let object = self.objects.get(&source)?;
        let object_tags = object.applied_upgrades.iter().cloned().collect();
        let player_tags = object
            .owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|p| p.completed_upgrades.iter().cloned().collect())
            .unwrap_or_default();
        Some((object_tags, player_tags))
    }

    fn dead_ephemeral_specs(
        &mut self,
        source: ObjectId,
    ) -> Vec<crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec>
    {
        let Some(object) = self.objects.get(&source) else {
            return Vec::new();
        };
        if object.fire_weapon_when_dead_fired || object.status.under_construction {
            return Vec::new();
        }
        let death_ordinal = u32::from(object.status.death_type.ordinal());
        let veterancy_ordinal = object.experience.level as u32;
        let statuses = object.object_status_bits;
        let Some((object_tags, player_tags)) = self.fire_when_owned_upgrade_tags(source) else {
            return Vec::new();
        };
        let Some(object) = self.objects.get_mut(&source) else {
            return Vec::new();
        };
        let owned = owned_upgrade_tag_refs(&object_tags, &player_tags);
        let mut specs = Vec::new();
        for (runtime, metadata) in object
            .temporary_weapon_runtime
            .dead
            .iter_mut()
            .zip(object.thing.template.fire_weapon_when_dead_behaviors.iter())
        {
            if metadata.starts_active || metadata.upgrade_mux.triggered_by_owned(&owned) {
                runtime.upgrade_executed = true;
            }
            if !runtime.upgrade_executed {
                continue;
            }
            if !metadata.die_mux_allows(death_ordinal, veterancy_ordinal, statuses) {
                continue;
            }
            if metadata.upgrade_mux.conflicts_with_owned(&owned) {
                continue;
            }
            // C++ each FireWeaponWhenDead module onDie independently.
            if let Some(spec) = metadata.ephemeral_weapon_spec() {
                specs.push(spec);
            }
        }
        specs
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

fn owned_upgrade_tag_refs<'a>(
    object_tags: &'a [String],
    player_tags: &'a [String],
) -> Vec<&'a str> {
    object_tags
        .iter()
        .chain(player_tags.iter())
        .map(|s| s.as_str())
        .collect()
}
