use super::*;

impl Player {
    /// Check if player has any objects at all.
    /// C++ Reference: Player::hasAnyObjects()
    pub fn has_any_objects(&self) -> Bool {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.owned_objects {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    if object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Inert)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if player has any units (non-structure objects)
    /// C++ Reference: Player::hasAnyUnits() - checks for non-structure units
    pub fn has_any_units(&self) -> Bool {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.owned_objects {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    if object_guard.is_kind_of(KindOf::Structure)
                        || object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if player has any buildings that count for victory.
    /// C++ Reference: Player::hasAnyBuildings(KINDOF_MP_COUNT_FOR_VICTORY)
    pub fn has_any_buildings_counts_for_victory(&self) -> Bool {
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let object_ids = manager.get_objects_owned_by_player(self.player_index as UnsignedInt);
            for obj_id in object_ids {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    let base_arc = obj_arc.read().ok().map(|g| g.base());
                    if let Some(base_arc) = base_arc {
                        if let Ok(base_obj) = base_arc.read() {
                            if base_obj.is_kind_of(KindOf::Structure)
                                && base_obj.is_kind_of(KindOf::CountsForVictory)
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if player has any build facilities (structures that can produce units)
    /// C++ Reference: Player::hasAnyBuildFacility() - checks for buildings with production capability
    pub fn has_any_build_facility(&self) -> Bool {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.owned_objects {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    object_guard.get_template().is_build_facility()
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Called when a unit is created by this player
    /// Matches C++ Player::onUnitCreated
    /// ID-first unit/structure creation notification.
    pub fn on_unit_created_id(&mut self, _producer_id: ObjectID, unit_id: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let score_keeper = &mut self.score_keeper;
        let academy_stats = &mut self.academy_stats;
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(unit_id, |unit_guard| {
            // C++ Player::onUnitCreated → ScoreKeeper::addObjectBuilt (KindOf + map).
            score_keeper.add_object_built_obj(unit_guard);
            let type_name = unit_guard.get_template().get_name().as_str();
            if unit_guard.is_kind_of(KindOf::Structure) {
                academy_stats.record_building_built(type_name);
            } else {
                academy_stats.record_unit_built(type_name);
            }
        });
    }

    pub fn on_unit_created(&mut self, producer: &Arc<RwLock<Object>>, unit: &Arc<RwLock<Object>>) {
        let producer_id = producer
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        let unit_id = unit.read().ok().map(|g| g.get_id()).unwrap_or(INVALID_ID);
        self.on_unit_created_id(producer_id, unit_id);
    }

    /// Called when a structure is undone (e.g. AI rebuild clears old CC).
    /// Matches C++ Player::onStructureUndone — scoreKeeper.removeObjectBuilt only.
    pub fn on_structure_undone(&mut self, structure: &Arc<RwLock<Object>>) {
        let structure_id = structure
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        self.on_structure_undone_id(structure_id);
    }

    /// Borrow-first ObjectID variant of [`Self::on_structure_undone`].
    pub fn on_structure_undone_id(&mut self, structure_id: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let score_keeper = &mut self.score_keeper;
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(structure_id, |guard| {
            score_keeper.remove_object_built_obj(guard);
        });
    }

    /// Called when a structure under construction is completed.
    /// Matches C++ Player::onStructureConstructionComplete.
    pub fn on_structure_construction_complete_id(
        &mut self,
        builder_id: Option<ObjectID>,
        structure_id: ObjectID,
        is_rebuild: Bool,
    ) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();

        let Some(structure) = crate::object::registry::OBJECT_REGISTRY
            .get_object(structure_id)
            .or_else(|| crate::helpers::TheGameLogic::find_object_by_id(structure_id))
        else {
            return;
        };

        let (
            structure_pos,
            structure_layer,
            is_superweapon_particle,
            is_superweapon_nuke,
            is_superweapon_scud,
        ) = {
            let Ok(structure_guard) = structure.read() else {
                return;
            };
            (
                *structure_guard.get_position(),
                structure_guard.get_layer(),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::ParticleUplinkCannon,
                ),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::NeutronMissile,
                ),
                structure_guard.has_special_power(
                    crate::object::special_power_types::SpecialPowerType::ScudStorm,
                ),
            )
        };

        let ai_store = crate::ai::the_ai(); if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinding) = ai_guard.pathfinding_system() {
                if let Ok(mut system) = pathfinding.write() {
                    let layer =
                        crate::ai::pathfinding_system::PathfindLayerEnum::from(structure_layer);
                    let positions = [structure_pos];
                    system.remove_obstacle(structure_id, &positions, layer);
                    system.add_obstacle(structure_id, &positions, layer);
                }
            }
        }

        if !is_rebuild {
            if let Ok(structure_guard) = structure.read() {
                // C++ onStructureConstructionComplete → addObjectBuilt + addMoneySpent.
                self.score_keeper.add_object_built_obj(&*structure_guard);
                let cost = structure_guard
                    .get_template()
                    .calc_cost_to_build(Some(self))
                    .max(0) as u32;
                self.score_keeper.add_money_spent(cost);
                self.academy_stats
                    .record_building_built(structure_guard.get_template().get_name().as_str());
            }
        }

        if let Ok(structure_guard) = structure.read() {
            structure_guard.adjust_power_for_player(true);
        }

        if let Some(factory_id) = builder_id.filter(|id| *id != INVALID_ID) {
            let player_id = self.player_index as u32;
            let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
                manager.with_ai_player_mut(player_id, |ai_player| {
                    let _ = ai_player.on_structure_produced(factory_id, structure_id);
                })
            });
        }

        crate::control_bar::mark_ui_dirty();

        let local_player = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Ok(structure_guard) = structure.read() else {
            return;
        };
        if let Some(local_player) = local_player {
            let relation = structure_guard
                .get_team()
                .and_then(|team| {
                    team.read().ok().map(|team_guard| {
                        local_player
                            .read()
                            .ok()
                            .map(|p| p.get_relationship_with_team(&team_guard))
                    })
                })
                .flatten()
                .unwrap_or(Relationship::Neutral);

            if is_superweapon_particle {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnParticleCannon,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyParticleCannon,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyParticleCannon,
                    );
                }
            }

            if is_superweapon_nuke {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnNuke,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyNuke,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyNuke,
                    );
                }
            }

            if is_superweapon_scud {
                if local_player.read().ok().map(|p| p.get_player_index()) == Some(self.player_index)
                {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedOwnScudStorm,
                    );
                } else if relation != Relationship::Enemies {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedAllyScudStorm,
                    );
                } else {
                    let _ = crate::helpers::TheEva::set_should_play(
                        crate::helpers::EvaEvent::SuperweaponDetectedEnemyScudStorm,
                    );
                }
            }
        }
    }

    /// Prefer [`Self::on_structure_construction_complete_id`].
    pub fn on_structure_construction_complete(
        &mut self,
        builder: Option<&Arc<RwLock<Object>>>,
        structure: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
    ) {
        let builder_id = builder.and_then(|b| b.read().ok().map(|g| g.get_id()));
        let structure_id = structure
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        self.on_structure_construction_complete_id(builder_id, structure_id, is_rebuild);
    }

    /// Set units vision spied state
    /// Matches C++ Player::setUnitsVisionSpied
    pub fn set_units_vision_spied(
        &mut self,
        on: Bool,
        spy_on_kind_of: crate::common::KindOfMaskType,
        spying_player_index: PlayerIndex,
    ) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        use crate::common::{ALL_KIND_OF, KIND_OF_MASK_ALL, KIND_OF_MASK_NONE};
        use crate::object::registry::OBJECT_REGISTRY;

        pub(super) fn matches_any_kind_of(
            object: &Object,
            mask: crate::common::KindOfMaskType,
        ) -> bool {
            if mask == KIND_OF_MASK_ALL {
                return true;
            }
            if mask == KIND_OF_MASK_NONE {
                return false;
            }

            for &kind in ALL_KIND_OF {
                let bit = kind.cpp_mask();
                if (mask & bit) != 0 && object.is_kind_of(kind) {
                    return true;
                }
            }

            false
        }

        for &object_id in &self.owned_objects {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                if matches_any_kind_of(obj_guard, spy_on_kind_of) {
                    obj_guard.set_vision_spied_by_player(spying_player_index, on);
                }
            });
        }
    }

    /// Called when a unit owned by this player is destroyed
    /// Matches C++ Player::onUnitDestroyed
    pub fn on_unit_destroyed_id(&mut self, unit_id: ObjectID, _by_player: Option<PlayerIndex>) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let score_keeper = &mut self.score_keeper;
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(unit_id, |unit_guard| {
            if unit_guard.is_kind_of(KindOf::Structure) {
                score_keeper.buildings_lost += 1;
            } else {
                score_keeper.add_unit_lost();
            }
        });
    }

    pub fn on_unit_destroyed(
        &mut self,
        unit: &Arc<RwLock<Object>>,
        by_player: Option<PlayerIndex>,
    ) {
        let unit_id = unit.read().ok().map(|g| g.get_id()).unwrap_or(INVALID_ID);
        self.on_unit_destroyed_id(unit_id, by_player);
    }

    /// Called when this player destroys an enemy unit
    /// Matches C++ Player::onEnemyUnitKilled
    pub fn on_enemy_unit_killed_id(&mut self, killed_unit_id: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let score_keeper = &mut self.score_keeper;
        let academy_stats = &mut self.academy_stats;
        let _ =
            crate::object::registry::OBJECT_REGISTRY.with_object(killed_unit_id, |unit_guard| {
                if unit_guard.is_kind_of(KindOf::Structure) {
                    score_keeper.add_building_destroyed();
                    let type_name = unit_guard.get_template().get_name().as_str();
                    academy_stats.record_building_destroyed(type_name);
                } else {
                    score_keeper.add_unit_killed();
                    let type_name = unit_guard.get_template().get_name().as_str();
                    academy_stats.record_unit_killed(type_name);
                }
            });
    }

    pub fn on_enemy_unit_killed(&mut self, killed_unit: &Arc<RwLock<Object>>) {
        let killed_unit_id = killed_unit
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        self.on_enemy_unit_killed_id(killed_unit_id);
    }

    /// C++ placeNetworkBuildingsForPlayer starting CC:
    /// onStructureConstructionComplete(..., FALSE) → addObjectBuilt + addMoneySpent.
    pub fn score_starting_structure_complete(&mut self, template_name: &str) {
        let bits = retail_kindof_bits_for_template(template_name);
        self.score_keeper
            .add_object_built_template(template_name, bits);
        if let Ok(factory_guard) = game_engine::common::thing::thing_factory::get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                if let Some(template) = factory.find_template(template_name, false) {
                    let cost = template.get_build_cost().max(0) as u32;
                    self.score_keeper.add_money_spent(cost);
                }
            }
        }
        self.academy_stats.record_building_built(template_name);
    }

    /// C++ placeNetworkBuildingsForPlayer StartingUnit0..N → onUnitCreated.
    pub fn score_starting_unit_created(&mut self, template_name: &str) {
        let bits = retail_kindof_bits_for_template(template_name);
        self.score_keeper
            .add_object_built_template(template_name, bits);
        self.academy_stats.record_unit_built(template_name);
    }
}

fn retail_kindof_bits_for_template(template_name: &str) -> u64 {
    game_engine::common::thing::thing_factory::get_thing_factory()
        .ok()
        .and_then(|factory_guard| {
            factory_guard
                .as_ref()
                .and_then(|factory| factory.find_template(template_name, false))
                .map(|template| template.get_kindof_mask())
        })
        .unwrap_or(0)
}

/// Host `spawn_skirmish_starting_units` scores leftover Player like C++.
pub fn notify_skirmish_starting_object(player_id: u32, template_name: &str, is_structure: bool) {
    if template_name.is_empty() {
        return;
    }
    let Some(player) = leftover_player_for_host_id(player_id) else {
        return;
    };
    let Ok(mut guard) = player.write() else {
        return;
    };
    if is_structure {
        guard.score_starting_structure_complete(template_name);
    } else {
        guard.score_starting_unit_created(template_name);
    }
}

fn leftover_player_for_host_id(
    player_id: u32,
) -> Option<std::sync::Arc<std::sync::RwLock<Player>>> {
    let Ok(list) = ThePlayerList().read() else {
        return None;
    };
    let named = format!("player{player_id}");
    list.find_player_by_name(&named)
        .or_else(|| list.get_player(player_id as PlayerIndex).cloned())
}

/// Live mid-game create → leftover `ScoreKeeper::addObjectBuilt` (KindOf filter).
pub fn notify_live_object_built(player_id: u32, template_name: &str) {
    if template_name.is_empty() {
        return;
    }
    let Some(player) = leftover_player_for_host_id(player_id) else {
        return;
    };
    let Ok(mut guard) = player.write() else {
        return;
    };
    let bits = retail_kindof_bits_for_template(template_name);
    guard
        .score_keeper
        .add_object_built_template(template_name, bits);
    // Live notify previously wrote ScoreKeeper only; leftover academy stayed empty.
    if bits & (1u64 << 7) != 0 {
        guard.academy_stats.record_building_built(template_name);
    } else {
        guard.academy_stats.record_unit_built(template_name);
    }
}

/// Live mid-game kill → leftover `ScoreKeeper::addObjectDestroyed`.
pub fn notify_live_object_destroyed(
    killer_player_id: u32,
    victim_player_id: u32,
    template_name: &str,
    under_construction: bool,
) {
    if template_name.is_empty() {
        return;
    }
    let Some(player) = leftover_player_for_host_id(killer_player_id) else {
        return;
    };
    let Ok(mut guard) = player.write() else {
        return;
    };
    let bits = retail_kindof_bits_for_template(template_name);
    guard.score_keeper.add_object_destroyed_template(
        template_name,
        bits,
        victim_player_id as Int,
        under_construction,
    );
    if !under_construction {
        if bits & (1u64 << 7) != 0 {
            guard.academy_stats.record_building_destroyed(template_name);
        } else {
            guard.academy_stats.record_unit_killed(template_name);
        }
    }
}

/// Live mid-game loss → leftover `ScoreKeeper::addObjectLost`.
pub fn notify_live_object_lost(player_id: u32, template_name: &str, under_construction: bool) {
    if template_name.is_empty() {
        return;
    }
    let Some(player) = leftover_player_for_host_id(player_id) else {
        return;
    };
    let Ok(mut guard) = player.write() else {
        return;
    };
    let bits = retail_kindof_bits_for_template(template_name);
    guard
        .score_keeper
        .add_object_lost_template(template_name, bits, under_construction);
}

/// C++ GameLogic.cpp:1720-1723 occupied observer slot.
pub fn notify_live_observer_slot(player_id: u32) {
    let Some(player) = leftover_player_for_host_id(player_id) else {
        return;
    };
    if let Ok(mut guard) = player.write() {
        guard.set_observer(true);
    }
}
