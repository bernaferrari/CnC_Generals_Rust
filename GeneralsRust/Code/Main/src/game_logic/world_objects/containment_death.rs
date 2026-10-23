//! Installed containment death callbacks; CPP OpenContain833 and derived overrides.
use super::super::*;

/// C++ `OpenContain::getDamagePercentageToUnits` — INI `DamagePercentToUnits`
/// via `parsePercentToReal` (`100%` → `1.0`). Live `building_data` is never
/// populated, so resolve authored retail percents by container kind.
fn damage_percent_to_units_from_ini(obj: &Object) -> f32 {
    if let Some(percent) = obj.thing().template.contain_module.damage_percent_to_units {
        return percent;
    }
    let authored = obj
        .building_data
        .as_ref()
        .map(|bd| bd.damage_percent_to_units)
        .unwrap_or(0.0);
    if authored > 0.0 {
        return authored;
    }
    if obj.is_humvee_style_container() {
        return crate::game_logic::host_humvee::HUMVEE_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_battle_bus_style_container() {
        return crate::game_logic::host_battle_bus::BATTLE_BUS_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_combat_chinook_style_container() {
        return crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_DAMAGE_PERCENT_TO_UNITS
            / 100.0;
    }
    if obj.is_technical_style_container() {
        return crate::game_logic::host_technical::TECHNICAL_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_troop_crawler_style_container() {
        return crate::game_logic::host_troop_crawler::TROOP_CRAWLER_DAMAGE_PERCENT_TO_UNITS
            / 100.0;
    }
    if obj.is_listening_outpost_style_container() {
        return crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DAMAGE_PERCENT_TO_UNITS
            / 100.0;
    }
    if obj.is_overlord_style_container() || obj.is_helix_transport {
        return crate::game_logic::host_overlord_addons::OVERLORD_CONTAIN_DAMAGE_PERCENT_TO_UNITS;
    }
    if crate::game_logic::host_heal::is_ambulance_healer(&obj.template_name) {
        return crate::game_logic::host_heal::AMBULANCE_TRANSPORT_DAMAGE_PERCENT_TO_UNITS;
    }
    if obj.template_name.to_ascii_lowercase().contains("firebase") {
        // Retail AmericaFireBase DamagePercentToUnits 100% (parsePercentToReal).
        return 1.0;
    }
    // C++ OpenContainModuleData default DamagePercentToUnits.
    0.0
}

/// C++ TransportContain (and subclasses) override `killRidersWhoAreNotFreeToExit`.
/// OpenContain / Garrison / Tunnel / Cave do not.
fn transport_contain_kills_unfree_riders(obj: &Object) -> bool {
    use crate::game_logic::ContainModuleKind;
    matches!(
        obj.thing().template.contain_module.kind,
        ContainModuleKind::Transport
            | ContainModuleKind::RailedTransport
            | ContainModuleKind::RiderChange
            | ContainModuleKind::InternetHack
    ) || obj.is_overlord_style_container()
        || obj.is_helix_transport
        || obj.is_battle_bus_transport
        || obj.is_technical_transport
        || obj.is_humvee_transport
        || obj.is_troop_crawler_transport
        || obj.is_combat_chinook_transport
        || obj.is_listening_outpost_transport
}

/// C++ `TransportContain::onRemoving` when the hull is effectively dead:
/// leftover `OpenContain::scatterToNearbyPosition` — place at wreck, face the
/// ring angle, `aiMoveToPosition` toward a 1.0–1.5× bounding-radius dest.
fn apply_transport_death_scatter(
    unit: &mut Object,
    container_id: ObjectId,
    container_pos: glam::Vec3,
    bounding_radius: f32,
    frame: u32,
    effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    audio: &mut crate::game_logic::ObjectAudio<'_>,
) -> glam::Vec3 {
    let leftover = gamelogic::object::contain::open_contain::leftover_scatter_to_nearby_position(
        container_pos.x,
        container_pos.z,
        container_pos.y,
        bounding_radius,
        Some(container_pos.y),
    );
    let dest = glam::Vec3::new(leftover.dest_x, leftover.dest_z, leftover.dest_y);
    unit.stop_moving(audio);
    unit.target = None;
    unit.set_contained_by(None);
    unit.set_position_with_effects(container_pos, frame, effects);
    unit.set_orientation(leftover.orientation);
    unit.set_ai_state(AIState::Moving, audio);
    unit.status.moving = true;
    // Pathfinder ignore for the queued move. Physics still skips the wreck
    // through ignore_collisions_with; scatter does not start a collision timer.
    unit.ignored_obstacle_id = Some(container_id);
    unit.ignore_collisions_with = Some(container_id);
    // TransportContain.cpp:389–393 scatters before wakeUpAndAttemptToTarget.
    // The move makes the rider active, so AIUpdate.cpp:4430 preserves its
    // mood deadline and jitter flag even when ResetMoodCheckTimeOnExit is true.
    unit.set_status_attacking(false);
    dest
}

/// C++ `Pathfinder::validMovementTerrain` at the hull (AIPathfind.cpp:4763-4783).
fn valid_rider_movement_terrain(
    grid: &crate::game_logic::PathfindingGrid,
    surfaces: u32,
    world_pos: glam::Vec3,
) -> bool {
    use crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type;
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let cell = grid.world_to_grid(world_pos);
    if !grid.is_valid_pos(cell) {
        return false;
    }
    let ty = grid.cell_type(cell);
    if matches!(
        ty,
        PathfindCellType::Obstacle | PathfindCellType::Impassable
    ) {
        return true;
    }
    (surfaces & valid_locomotor_surfaces_for_cell_type(ty)) != 0
}

/// C++ `TransportContain::isSpecificRiderFreeToExit` (TransportContain.cpp:536-567).
fn is_specific_rider_free_to_exit(
    container: &Object,
    rider: &Object,
    grid: &crate::game_logic::PathfindingGrid,
) -> bool {
    if let Some(ai) = container.chinook_ai.as_ref() {
        let can_rappel =
            crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                rider.is_kind_of(KindOf::Infantry),
            );
        if ai.ai_free_to_exit(can_rappel)
            != crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
        {
            return false;
        }
    }
    let airborne = container.is_kind_of(KindOf::Aircraft)
        || container.status.airborne_target
        || (container.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0;
    if airborne {
        return true;
    }
    if !rider.has_ai_update_interface() || rider.cur_locomotor_name.is_none() {
        return false;
    }
    let surfaces = if rider.locomotor_surfaces != 0 {
        rider.locomotor_surfaces
    } else {
        Object::default_locomotor_surfaces_for_template(&rider.thing().template)
    };
    if surfaces == 0 {
        return false;
    }
    valid_rider_movement_terrain(grid, surfaces, container.get_position())
}

impl GameLogic {
    /// C++ derived containment onDie dispatch. Communal stores consume the
    /// exact entrance identity; ordinary containers keep literal roster order.
    pub(in crate::game_logic) fn complete_installed_containment_death(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) {
        self.complete_installed_containment_death_with_effects(
            id,
            killer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn complete_installed_containment_death_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(container) = self.objects.get(&id) else {
            return;
        };
        let applicable = container
            .thing()
            .template
            .contain_module
            .death_rules
            .as_ref()
            .is_none_or(|rules| {
                rules.allows(
                    container.status.death_type,
                    container.experience.level,
                    container.object_status_bits,
                )
            });
        if container.is_tunnel_network_style_container() {
            if !applicable {
                return;
            }
            self.unregister_tunnel_containment_with_effects(id, presentation_effects);
            return;
        }
        if container.is_cave_style_container() {
            if !applicable {
                return;
            }
            if container.status.under_construction {
                return;
            }
            let index = container.cave_index;
            if self.cave_system.index_of(id).is_none() {
                return;
            }
            let remaining: Vec<_> = self
                .objects
                .registered_values()
                .filter(|object| {
                    object.id != id
                        && object.is_alive()
                        && !object.status.sold
                        && object.is_cave_style_container()
                        && object.cave_index == index
                })
                .map(|object| object.id)
                .collect();
            let outcome = self.cave_system.on_cave_destroyed(id, &remaining);
            if outcome.cave_in {
                for child in outcome.cave_in_units {
                    if let Some(child) = self.objects.get_mut(&child) {
                        child.set_contained_by(None);
                    }
                    // TunnelTracker cave-in is direct destruction, not body death.
                    self.destroy_object_with_effects(child, presentation_effects);
                }
            } else if let Some(entrance) = outcome.remapped_to {
                for child in self.cave_system.contained_for_index(index) {
                    if let Some(child) = self.objects.get_mut(&child) {
                        if child.contained_by == Some(id) {
                            child.set_contained_by(Some(entrance));
                        }
                    }
                }
            }
            return;
        }
        let parachute_airborne = container.paradrop_parachute
            || container
                .template_name
                .eq_ignore_ascii_case(crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME);
        let position = container.get_position();
        let airborne = parachute_airborne
            && crate::game_logic::host_usa_pilot::should_apply_parachute_free_fall_damage(
                true, position.y,
            );
        if airborne {
            let riders = container.contained_units();
            // ParachuteContain618 removes the literal roster before damaging
            // its first rider; the subsequent OpenContain sees an empty list.
            for &rider in &riders {
                self.remove_direct_contained_child_with_effects(id, rider, presentation_effects);
            }
            if let Some(rider) = riders.first() {
                self.apply_rider_free_fall_damage_with_effects(
                    *rider,
                    position,
                    presentation_effects,
                );
            }
            self.car_bomb.record_airborne_parachute_free_fall();
        }
        self.complete_open_contain_death_with_effects(id, killer, presentation_effects);
    }

    /// TransportContain389 scatters during onRemoving, before mood reset.
    pub(in crate::game_logic) fn scatter_dying_transport_rider(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) {
        self.scatter_dying_transport_rider_with_effects(
            container_id,
            rider_id,
            &mut Default::default(),
        );
    }

    pub(in crate::game_logic) fn scatter_dying_transport_rider_with_effects(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if (container.is_alive() && !container.status.on_die_started)
            || !self
                .objects
                .get(&rider_id)
                .is_some_and(|rider| rider.is_alive() && rider.has_ai_update_interface())
        {
            return;
        }
        let position = container.get_position();
        let geometry = &container.thing().template.geometry_info;
        let radius = if geometry.authored {
            geometry.bounding_circle_radius()
        } else {
            container
                .selection_radius
                .max(container.thing().geometry.radius)
        };
        let destination = {
            let rider = self
                .objects
                .get_mut(&rider_id)
                .expect("installed contain rider");
            apply_transport_death_scatter(
                rider,
                container_id,
                position,
                radius,
                self.frame,
                effects,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            )
        };
        self.path_approach_with_state_ignoring(
            rider_id,
            destination,
            AIState::Moving,
            Some(container_id),
        );
    }

    /// Complete OpenContain::onDie while both owner and riders are installed.
    /// The physical removal queue never repeats this callback.
    pub(in crate::game_logic) fn complete_open_contain_death(
        &mut self,
        container_id: ObjectId,
        _killer: Option<Team>,
    ) {
        self.complete_open_contain_death_with_effects(
            container_id,
            _killer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn complete_open_contain_death_with_effects(
        &mut self,
        container_id: ObjectId,
        _killer: Option<Team>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container
            .thing()
            .template
            .contain_module
            .death_rules
            .as_ref()
            .is_none_or(|rules| {
                rules.allows(
                    container.status.death_type,
                    container.experience.level,
                    container.object_status_bits,
                )
            })
        {
            return;
        }
        let riders = container.contained_units();
        let damage_percentage = damage_percent_to_units_from_ini(container);
        let passenger_death_type = if container
            .thing()
            .template
            .contain_module
            .burned_death_to_units
        {
            crate::game_logic::host_usa_pilot::HostDeathType::Burned
        } else {
            crate::game_logic::host_usa_pilot::HostDeathType::Normal
        };
        let container_team = container.team;
        let kill_unfree = transport_contain_kills_unfree_riders(container);
        let destroy_unfree = container
            .thing()
            .template
            .contain_module
            .destroy_riders_who_are_not_free_to_exit;
        // CPP processDamageToContained completes before the free-to-exit pass.
        for id in &riders {
            let mut died = false;
            if damage_percentage > 0.0 {
                if let Some(rider) = self.objects.get(id) {
                    let damage = rider.max_health * damage_percentage;
                    let receiving_damage_id = rider.id;
                    died = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_id,
                        damage,
                        Some(container_id),
                        crate::game_logic::combat::DamageType::Unresistable,
                        passenger_death_type,
                        None,
                        presentation_effects,
                    );
                    if !died
                        && damage_percentage == 1.0
                        && self.objects.get(id).is_some_and(Object::is_alive)
                    {
                        died = self
                            .apply_owned_kill_with_effects(
                                *id,
                                crate::game_logic::combat::DamageType::Unresistable,
                                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                                presentation_effects,
                            )
                            .is_some_and(|result| result.destroyed());
                    }
                }
            }
            if died {
                self.mark_object_for_destruction_with_effects(
                    *id,
                    Some(container_team),
                    presentation_effects,
                );
            }
        }
        for &id in &riders {
            // Resolve after the preceding damage callbacks, as CPP does.
            let free = !kill_unfree
                || self
                    .objects
                    .get(&container_id)
                    .zip(self.objects.get(&id))
                    .is_some_and(|(container, rider)| {
                        is_specific_rider_free_to_exit(
                            container,
                            rider,
                            &self.pathfinding_system.grid,
                        )
                    });
            if !free && self.objects.get(&id).is_some_and(Object::is_alive) {
                if destroy_unfree {
                    self.destroy_object_with_effects(id, presentation_effects);
                    continue;
                }
                self.apply_owned_kill_with_effects(
                    id,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    presentation_effects,
                );
            }
        }
        for id in riders {
            self.remove_direct_contained_child_with_effects(container_id, id, presentation_effects);
        }
    }
}

#[cfg(test)]
#[path = "containment_death_owner_tests.rs"]
mod containment_death_owner_tests;
