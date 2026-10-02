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

        use crate::object::registry::OBJECT_REGISTRY;

        for &object_id in &self.owned_objects {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                // C++ Object::isAnyKindOf: any shared kind bit. Empty object mask does not match.
                if spy_on_kind_of != 0 && (obj_guard.get_kind_of() & spy_on_kind_of) != 0 {
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
