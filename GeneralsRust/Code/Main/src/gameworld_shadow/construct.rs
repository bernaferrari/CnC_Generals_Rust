//! Construct, ID maps, ordinal helpers, and host sync.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
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

    pub(super) fn host_building_type_ordinal(t: crate::game_logic::BuildingType) -> u8 {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => 0,
            B::Barracks => 1,
            B::WarFactory => 2,
            B::Airfield => 3,
            B::RepairPad => 4,
            B::HealPad => 5,
            B::SupplyCenter => 6,
            B::PowerPlant => 7,
            B::DefenseTurret => 8,
            B::SupplyDropZone => 9,
            B::Palace => 10,
            B::Propaganda => 11,
            B::Bunker => 12,
        }
    }

    pub(super) fn host_veterancy_ordinal(level: crate::game_logic::VeterancyLevel) -> u8 {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => 0,
            V::Veteran => 1,
            V::Elite => 2,
            V::Heroic => 3,
        }
    }

    /// Presentation KindOf ORDER residual (must match PresentationFrame freeze ORDER).
    pub(super) fn host_kind_of_bits(obj: &crate::game_logic::Object) -> u32 {
        obj.presentation_kind_of_bits()
    }

    pub(super) fn host_object_type_ordinal(t: crate::game_logic::ObjectType) -> u8 {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => 0,
            T::Vehicle => 1,
            T::Aircraft => 2,
            T::Building => 3,
            T::Supply => 4,
            T::Projectile => 5,
            T::Neutral => 6,
        }
    }

    pub(super) fn host_team_ordinal(team: Team) -> u8 {
        match team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 255,
        }
    }

    pub(super) fn entity_team_from_ordinal(ord: u8) -> Team {
        match ord {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
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
        use crate::fow_rendering::{ObjectShroudFrame, ObjectVisibility};

        let host_objects = logic.host_objects();
        let mut ids = host_objects.registered_ids().to_vec();
        ids.sort_unstable_by_key(|id| id.0);
        self.sync_object_identities(logic, &ids);
        let assets = logic.application_assets();
        let catalog = assets.as_ref().and_then(|assets| {
            assets.lock().ok().map(|manager| manager.object_catalog())
        });
        let local_player = logic.local_player_id().unwrap_or(0);
        let shell = logic.isInShellGame();
        let shroud = if shell {
            None
        } else {
            logic.world_services.shroud().lock().ok()
        };
        let shroud_frame = ObjectShroudFrame::new(shroud.as_deref(), local_player);
        for oid in ids {
            let Some(obj) = logic.host_object(oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            // Registered objects missing their controller retain the vision pass's
            // faction fallback. GameWorld ownership itself uses the explicit ID.
            let visibility = if shell
                || obj.owner_player_id.or_else(|| logic.player_id_for_team(obj.team))
                    == Some(local_player)
            {
                ObjectVisibility::FULLY_VISIBLE
            } else {
                shroud_frame.visibility(obj.id)
            };
            self.refresh_mapped_from_host(
                logic,
                oid,
                obj,
                eid,
                catalog.as_deref(),
                visibility,
            );
        }
        drop(shroud);

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

}

#[path = "object_observation.rs"]
mod object_observation;
