//! Synchronous factory delivery to the AI owned by the producing match.
//! C++ AIPlayer.cpp:1024-1120; ProductionUpdate.cpp:815 precedes Create callbacks.
use super::*;

/// Owned continuation for the one matching work order. No world/store borrow
/// survives into unit commands, and the AI remains discoverable during callbacks.
struct Delivery {
    queue: usize,
    order: usize,
    team_id: Option<u32>,
    team_name: String,
    home: Option<Vec3>,
    collector_dock: Option<(ObjectId, Option<ObjectId>)>,
    resource_gatherer: bool,
}

impl AIPlayer {
    fn prepare_delivery(
        &self,
        world: &GameLogic,
        factory: ObjectId,
        unit: &Object,
    ) -> Option<Delivery> {
        for (queue, team) in self.team_queue.iter().enumerate() {
            for (order, work) in team.work_orders.iter().enumerate() {
                if work.factory_id != Some(factory)
                    || work.num_completed >= work.num_required
                    || !unit.template_name.eq_ignore_ascii_case(&work.template_name)
                {
                    continue;
                }
                let (team_name, home) = {
                    let factory = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    let destination = team.team_id.and_then(|id| factory.find_team_by_id(id));
                    let name = destination
                        .as_ref()
                        .map(|team| {
                            team.read()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .get_name()
                                .to_string()
                        })
                        .unwrap_or_else(|| team.name.clone());
                    let home = destination
                        .and_then(|_| factory.find_team_prototype(&name))
                        .filter(|prototype| prototype.has_home_location())
                        .map(|prototype| {
                            let home = prototype.home_location();
                            Vec3::new(home.x, home.z, home.y)
                        });
                    (name, home)
                };
                return Some(Delivery {
                    queue,
                    order,
                    team_id: team.team_id,
                    team_name,
                    home,
                    collector_dock: (unit.is_resource_collector() && work.is_resource_gatherer)
                        .then_some(work.supply_center_id)
                        .flatten()
                        .map(|center| {
                            (center, self.nearest_supply_source_for_center(world, center))
                        }),
                    resource_gatherer: work.is_resource_gatherer,
                });
            }
        }
        None
    }
}

impl GameLogic {
    /// C++ AIPlayer::onStructureProduced resets both scheduling delays before
    /// looking for the exact bound BuildList entry. This is a synchronous owner
    /// operation, not a later economic poll or a Core registry notification.
    pub(crate) fn notify_owned_ai_structure_produced(&mut self, structure: ObjectId) {
        self.notify_owned_ai_structure_produced_with_engine(structure, None);
    }

    pub(crate) fn notify_owned_ai_structure_produced_with_engine(
        &mut self,
        structure: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        let Some(object) = self.host_object(structure) else {
            return;
        };
        let Some(owner) = self.player_owner_for_host_object(object) else {
            return;
        };
        // CPP AIPlayer.cpp128-157: an unmatched completion may be the exact
        // successor of a pad-bound rebuild hole. Resolve identity before the
        // mutable AI borrow; never guess from position or template alone.
        let rebuilt_pad = self.ai_manager.ai_players.get(&owner).and_then(|ai| {
            ai.building_queue.iter().position(|pad| {
                self.templates
                    .get(&pad.template_name)
                    .is_some_and(|plan| plan.is_equivalent_to(&object.thing().template))
                    && pad
                        .object_id
                        .and_then(|hole| self.host_object(hole))
                        .is_some_and(|hole| {
                            hole.is_rebuild_hole
                                && hole.rebuild_reconstructing_id == Some(structure)
                        })
            })
        });
        let definitions = self.ai_definitions.snapshot();
        let (completion, desired) = {
            let Some(ai) = self.ai_manager.ai_players.get_mut(&owner) else {
                return;
            };
            ai.next_team_queue_time = 0.0;
            ai.next_building_time = 0.0;
            let side = ai.side_info_name();
            let desired = definitions
                .side_info
                .iter()
                .filter(|info| side.is_some_and(|side| info.side == side))
                .last()
                .map_or(0, |info| match ai.difficulty {
                    AIDifficulty::Easy => info.easy,
                    AIDifficulty::Medium => info.normal,
                    AIDifficulty::Hard | AIDifficulty::Brutal => info.hard,
                })
                .wrapping_add(1);
            let Some(building) = ai
                .building_queue
                .iter_mut()
                .find(|building| building.object_id == Some(structure))
            else {
                if let Some(index) = rebuilt_pad {
                    ai.building_queue[index].object_id = Some(structure);
                }
                // Original rebound branch does not replay map properties,
                // attached scripts, upgrades, or supply bookkeeping.
                return;
            };
            building.is_built = true;
            (building.completion.clone(), desired)
        };
        if let Some(completion) = completion {
            if let Some(object) = self.host_object_mut(structure) {
                object.name = completion.name;
                object.set_initial_health_percent(completion.health_percent);
                object.set_script_unsellable(completion.unsellable);
                object.set_status_under_construction(false);
                object.status.reconstructing = false;
            }
            self.apply_researched_player_upgrades_to_object(structure);
            // Named-object queries resolve the canonical host roster directly;
            // no global cache of a second Object model is published.
            if let Some(engine) = interpreter {
                self.run_object_script_with_interpreter(engine, &completion.script, structure);
            } else {
                self.run_owned_object_script(&completion.script, structure);
            }
        }
        // Re-resolve after the immediate script; it may destroy/capture/change
        // the structure or its pad. CPP checks the actual completed module now.
        let is_supply = self.host_object(structure).is_some_and(|object| {
            object.thing().template.dock_kind == crate::game_logic::DockKind::SupplyCenter
        });
        if is_supply {
            if let Some(building) = self.ai_manager.ai_players.get_mut(&owner).and_then(|ai| {
                ai.building_queue
                    .iter_mut()
                    .find(|building| building.object_id == Some(structure))
            }) {
                building.supply_building = true;
                building.current_gatherers = -1;
                building.desired_gatherers = desired;
            }
        }
    }

    /// Deliver exactly one factory exit at the original synchronous callback
    /// point. A missing factory is C++'s null-factory startup case.
    pub(crate) fn notify_owned_ai_unit_produced(
        &mut self,
        factory_id: ObjectId,
        unit_id: ObjectId,
    ) {
        let Some(factory) = self.host_object(factory_id) else {
            return;
        };
        let Some(owner) = self.player_owner_for_host_object(factory) else {
            return;
        };
        let Some(unit) = self.host_object(unit_id) else {
            return;
        };
        if self.player_owner_for_host_object(unit) != Some(owner) {
            return;
        }
        let Some(ai) = self.ai_manager.ai_players.get(&owner) else {
            return;
        };
        let delivery = ai.prepare_delivery(self, factory_id, unit);
        let dozer = unit.is_kind_of(KindOf::Dozer);
        let collector = unit.is_resource_collector();
        let unit_position = unit.get_position();
        let exit_goal = unit.movement.target_position.unwrap_or(unit_position);
        let team_factory = ai.team_factory.clone();
        let now = self.get_frame() as f32 / LOGIC_FRAMES_PER_SECOND;

        // Mutate only this player's owned queue, then release the borrow before
        // issuing synchronous host effects. No polling or object removal.
        let ai = self
            .ai_manager
            .ai_players
            .get_mut(&owner)
            .expect("resolved owned AI");
        if let Some(delivery) = &delivery {
            let team = &mut ai.team_queue[delivery.queue];
            let order = &mut team.work_orders[delivery.order];
            order.num_completed += 1;
            order.queued_count = order.queued_count.saturating_sub(1);
            order.observed_unit_ids.push(unit_id);
            order.factory_id = None;
            if team.reinforcement {
                team.reinforcement_id = Some(unit_id);
            }
        }
        let resource_gatherer = collector && delivery.as_ref().is_some_and(|d| d.resource_gatherer);
        if !resource_gatherer && dozer {
            if ai.dozer_queued_for_repair {
                ai.repair_dozer = Some(unit_id);
                ai.repair_dozer_origin = unit_position;
                ai.dozer_queued_for_repair = false;
            } else {
                // Host economic scheduling combines C++ m_buildDelay=0 and
                // m_structureTimer=1: make the next logic frame eligible.
                ai.next_building_time = now + 1.0 / LOGIC_FRAMES_PER_SECOND;
            }
        }
        ai.next_team_queue_time = 0.0; // C++ m_teamDelay=0, not m_teamTimer.

        if let Some(delivery) = delivery {
            if let Some(team_id) = delivery.team_id {
                let factory = team_factory
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                for team in factory.get_all_teams() {
                    let mut team = team.write().unwrap_or_else(|poison| poison.into_inner());
                    if team.get_id() == team_id {
                        team.add_member(unit_id.0);
                    } else {
                        team.remove_member(unit_id.0);
                    }
                }
                drop(factory);
                if let Some(unit) = self.host_object_mut(unit_id) {
                    unit.team_instance_name = delivery.team_name;
                }
            }
            if let Some(home) = delivery.home {
                self.follow_ai_production_home_path(unit_id, &[exit_goal, home]);
            }
            if collector {
                if let Some(unit) = self.host_object_mut(unit_id) {
                    unit.supply_truck_force_pending = delivery.resource_gatherer;
                }
                if let Some((center, source)) = delivery.collector_dock {
                    if let Some(unit) = self.host_object_mut(unit_id) {
                        unit.preferred_dock_id = Some(center);
                    }
                    if let Some(source) = source {
                        let _ = self.unit_command_stop_moving_order_target(unit_id, Some(source));
                        let _ = self.unit_command_set_ai_state(unit_id, AIState::Gathering);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
