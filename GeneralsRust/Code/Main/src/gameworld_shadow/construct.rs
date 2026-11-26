//! Construct, ID maps, ordinal helpers, and host sync.

use super::*;
use crate::game_logic::{GameLogic, ObjectId};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::GameWorld;
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    fn host_ready_structure_special_power_template(
        logic: &GameLogic,
        obj: &crate::game_logic::Object,
    ) -> Option<(String, u32)> {
        use crate::command_system::SpecialPowerType as P;

        obj.thing()
            .template
            .special_power_modules
            .iter()
            .find(|module| {
                module.command_power.as_ref().is_some_and(|power| {
                    matches!(
                        power,
                        &P::ParticleCannon
                            | &P::SuperweaponParticleCannon
                            | &P::LaserCannon
                            | &P::ScudStorm
                            | &P::NuclearMissile
                            | &P::NukeNeutronMissile
                            | &P::SuperweaponNeutronMissile
                    ) && logic.is_special_power_ready_for(obj.id, power)
                })
            })
            .map(|module| {
                (
                    module.special_power_template.clone(),
                    module.special_power_template_id,
                )
            })
    }

    pub fn new() -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: Default::default(),
        }
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }

    #[cfg(test)]
    pub fn world_mut(&mut self) -> &mut GameWorld {
        &mut self.world
    }

    pub fn entity_for_host(&self, host: ObjectId) -> Option<EntityId> {
        self.host_to_entity.get(&host.0).copied()
    }

    fn sync_object_identities(&mut self, logic: &GameLogic, ids: &[ObjectId]) {
        self.host_to_entity.retain(|host, entity| {
            if ids.binary_search_by_key(host, |id| id.0).is_ok() {
                return self.world.entity(*entity).is_some();
            }
            self.world.world_mut().remove_entity(*entity);
            false
        });

        // Bind every ObjectID before publishing any cross-object references.
        for &id in ids {
            if self.host_to_entity.contains_key(&id.0) {
                continue;
            }
            let Some(object) = logic.host_object(id) else {
                continue;
            };
            let position = object.get_position();
            let pose = Transform::new(position.to_array(), object.get_orientation());
            let entity = self.world.spawn_entity(
                TemplateRef::new(object.template_name.clone()),
                None,
                pose,
                object.health.current.max(0.0),
            );
            self.host_to_entity.insert(id.0, entity);
        }
    }

    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        let host_objects = logic.host_objects();
        let mut ids = host_objects.registered_ids().to_vec();
        ids.sort_unstable_by_key(|id| id.0);
        self.sync_object_identities(logic, &ids);
        for oid in ids {
            let Some(obj) = logic.host_object(oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            self.refresh_mapped_from_host(logic, oid, obj, eid);
        }

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

}

#[path = "object_observation.rs"]
mod object_observation;
