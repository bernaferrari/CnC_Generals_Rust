//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// Build an upgrade (player upgrades only).
    /// C++ `AIPlayer::buildUpgrade` (AIPlayer.cpp).
    ///
    /// Validate upgrade type/affordability, then walk player build list for a
    /// ready factory whose command set can queue the upgrade.
    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let upgrade = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(upgrade) = upgrade else {
            log::debug!(
                "Upgrade {} does not exist.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        };

        if upgrade.get_upgrade_type() == UpgradeType::Object {
            log::debug!(
                "Player build upgrade: Upgrade {} is an object, not a player upgrade.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        if player_guard.has_upgrade_in_production(upgrade.as_ref()) {
            log::debug!(
                "already has upgrade {} queued.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }
        if player_guard.has_upgrade_complete(upgrade.as_ref()) {
            log::debug!(
                "already has upgrade {} completed.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let can_afford = with_upgrade_center(|center| {
            center.can_afford_upgrade(&player_guard, upgrade.as_ref(), false)
        });
        if !can_afford {
            log::debug!(
                "lacks money to build upgrade {} at this time.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        // C++ walks build list (not all objects) for factory order parity.
        let factory_ids: Vec<ObjectID> = {
            let mut ids = Vec::new();
            let mut cur = player_guard.get_build_list();
            while let Some(info) = cur {
                let id = info.get_object_id();
                if id != INVALID_ID {
                    ids.push(id);
                }
                cur = info.get_next();
            }
            ids
        };
        drop(player_guard);

        for object_id in factory_ids {
            let Some(command_set_name) = OBJECT_REGISTRY
                .with_object(object_id, |obj_guard| {
                    if obj_guard.test_status(ObjectStatusTypes::UnderConstruction)
                        || obj_guard.test_status(ObjectStatusTypes::Sold)
                    {
                        return None;
                    }
                    Some(obj_guard.get_command_set_string().to_string())
                })
                .flatten()
            else {
                continue;
            };
            let Some(command_set) = control_bar.find_command_set_by_name(&command_set_name) else {
                continue;
            };

            let mut can_upgrade_here = false;
            for button in &command_set.buttons {
                let Some(button) = button else {
                    continue;
                };
                let Some(button_upgrade) = button.get_upgrade_template() else {
                    continue;
                };
                if button_upgrade.get_name() == upgrade.get_name() {
                    can_upgrade_here = true;
                    break;
                }
            }
            if !can_upgrade_here {
                continue;
            }

            // Need production update interface residual — queue_upgrade covers it.
            let queued_name = OBJECT_REGISTRY
                .with_object_mut(object_id, |obj_guard| {
                    if obj_guard.queue_upgrade(&upgrade) {
                        Some(obj_guard.get_template_name().to_string())
                    } else {
                        None
                    }
                })
                .flatten();
            if let Some(name) = queued_name {
                log::debug!("queues {} at {}", upgrade.get_name(), name);
                return Ok(());
            }
        }

        log::debug!(
            "lacks factory to build upgrade {} at this time.  Ignoring request.",
            upgrade_name
        );
        Ok(())
    }

    /// C++ `AIPlayer::buildBySupplies` (AIPlayer.cpp).
    ///
    /// findSupplyCenter, then non-cash may override with m_curWarehouseID.
    /// Offset toward base (cash) or enemy bounds (defense), legalize/wiggle,
    /// always addToPriorityBuildList (even if placement stays at seed), stamp
    /// m_curWarehouseID. Uses m_baseCenter as-is (no auto recompute).
    pub fn build_by_supplies(
        &mut self,
        minimum_cash: i32,
        thing_name: &str,
    ) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(template) = crate::helpers::TheThingFactory::find_template(thing_name) else {
            log::warn!(
                "AIPlayer: template '{}' not found for build_by_supplies",
                thing_name
            );
            return Ok(());
        };

        // C++ uses m_baseCenter even when m_baseCenterSet is false.
        let base_center = self.base_center;

        let is_cash_generator = template.is_kind_of(KindOf::CashGenerator);

        // C++: always findSupplyCenter first.
        let mut best_supply_id: Option<ObjectID> = self
            .find_supply_center(minimum_cash)
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()));

        // Non-cash: live m_curWarehouseID overrides find result when present.
        if !is_cash_generator {
            if let Some(warehouse_id) = self.current_warehouse_id {
                if OBJECT_REGISTRY.with_object(warehouse_id, |_| ()).is_some() {
                    best_supply_id = Some(warehouse_id);
                }
            }
        }

        let Some(warehouse_id) = best_supply_id else {
            return Ok(());
        };
        let Some(mut location) = OBJECT_REGISTRY.with_object(warehouse_id, |warehouse_guard| {
            *warehouse_guard.get_position()
        }) else {
            return Ok(());
        };

        let mut offset_x = location.x - base_center.x;
        let mut offset_y = location.y - base_center.y;
        let mut radius = 3.0 * PATHFIND_CELL_SIZE_F;
        if !is_cash_generator {
            // Defensive structure — face toward enemy base center.
            let enemy_ndx = self.get_skirmish_enemy_player_index();
            if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_ndx) {
                offset_x = location.x - (lo.x + hi.x) * 0.5;
                offset_y = location.y - (lo.y + hi.y) * 0.5;
            }
            radius = OBJECT_REGISTRY
                .with_object(warehouse_id, |warehouse_guard| {
                    warehouse_guard
                        .get_geometry_info()
                        .get_bounding_circle_radius()
                })
                .unwrap_or(radius);
        }
        let len = (offset_x * offset_x + offset_y * offset_y).sqrt();
        if len > 0.0001 {
            offset_x /= len;
            offset_y /= len;
        }
        location.x -= offset_x * radius;
        location.y -= offset_y * radius;

        let angle = template.get_placement_view_angle();
        // C++: if seed illegal, wiggle; if wiggle succeeds use newPos; else keep seed.
        // Always priority-build regardless of legalize success.
        let placement = self
            .find_valid_build_location(&location, template.get_name().as_str(), angle)
            .unwrap_or(location);
        let mut final_loc = placement;
        final_loc.z = 0.0; // build list locations are ground relative

        if let Some(player_arc) = self.get_player_arc() {
            if let Ok(mut pg) = player_arc.write() {
                pg.add_to_priority_build_list(AsciiString::from(thing_name), final_loc, angle);
            }
        }
        self.current_warehouse_id = Some(warehouse_id);
        Ok(())
    }

    pub fn build_specific_building_near_location(
        &mut self,
        thing_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        let Some(template) = crate::helpers::TheThingFactory::find_template(thing_name) else {
            log::warn!(
                "AIPlayer: template '{}' not found for build_specific_building_near_location",
                thing_name
            );
            return Ok(());
        };

        // C++ near-location path does not recompute base center.
        let angle = template.get_placement_view_angle();
        let mut build_location = location;
        if let Some(valid) =
            self.find_valid_build_location(&build_location, template.get_name().as_str(), angle)
        {
            build_location = valid;
            self.queue_structure_construction(thing_name, build_location, angle)?;
        }

        Ok(())
    }

    /// Legacy compatibility wrapper used by skirmish AI paths.
    pub fn build_specific_ai_building_at(
        &mut self,
        thing_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        self.build_specific_building_near_location(thing_name, location)
    }

    /// Build near the first member of the specified team, falling back to a normal build request.
    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam` (AIPlayer.cpp).
    ///
    /// Team estimate position → legalize/wiggle → priority build list.
    pub fn build_specific_building_nearest_team(
        &mut self,
        thing_name: &str,
        team_name: &str,
    ) -> Result<(), AiError> {
        let Some(template) = TheThingFactory::find_template(thing_name) else {
            return Ok(());
        };
        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(team_name));
        let Some(team_arc) = team_arc else {
            return Ok(());
        };
        let Ok(team_g) = team_arc.read() else {
            return Ok(());
        };
        let Some(location) = team_g.get_estimate_team_position() else {
            return Ok(());
        };
        drop(team_g);

        // C++ does not recompute base center here (offset toward base is unused).
        let angle = template.get_placement_view_angle();
        // C++ only addToPriorityBuildList when wiggle set valid after initial fail
        // (same control flow as calcClosestConstructionZoneLocation).
        let adjusted =
            self.calc_closest_construction_zone_location(template.get_name().as_str(), &location)?;
        let Some(mut new_pos) = adjusted else {
            log::debug!(
                "{} - buildSpecificBuildingNearestTeam unable to place.",
                thing_name
            );
            return Ok(());
        };
        new_pos.z = 0.0;
        if let Some(player_arc) = self.get_player_arc() {
            if let Ok(mut pg) = player_arc.write() {
                pg.add_to_priority_build_list(AsciiString::from(thing_name), new_pos, angle);
            }
        }
        Ok(())
    }

    /// C++ `AIPlayer::findSupplyCenter` (AIPlayer.cpp).
    ///
    /// Closest non-enemy warehouse with enough cash, no nearby owned cash
    /// generator, not closer to enemy than us (60/40). Halve cash floor to 100.
    pub(super) fn find_supply_center(&self, minimum_cash: i32) -> Option<Arc<RwLock<Object>>> {
        // Wave 255: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let player_arc = self.get_player_arc()?;
        let player_guard = player_arc.read().ok()?;
        let base_center = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        // C++: Player *enemy = getAiEnemy(); structure bounds midpoint.
        // Prefer latched current-enemy index (skirmish acquireEnemy), then human.
        let mut enemy_center = Coord3D::new(0.0, 0.0, 0.0);
        let mut has_enemy = false;
        let enemy_index = {
            let mut idx = None;
            if let Ok(list) = player_list().read() {
                if let Some(me) = list.get_player(self.player_id as i32) {
                    if let Ok(mg) = me.read() {
                        idx = mg.get_current_enemy_player_index();
                    }
                }
            }
            idx.or_else(|| {
                self.select_current_enemy_player()
                    .ok()
                    .and_then(|o| o.map(|(_, i)| i))
            })
        };
        if let Some(enemy_index) = enemy_index {
            if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_index) {
                enemy_center = Coord3D::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, 0.0);
                has_enemy = true;
            }
        }

        let mut candidates: Vec<LeftoverSupplyCenterCandidate> = Vec::new();
        let mut own_cash_gens: Vec<LeftoverOwnedCashGenerator> = Vec::new();
        // Host path: dual-world factory empty — no supply-center residual.
        if OBJECT_REGISTRY.is_empty() {
            return None;
        }
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let obj = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if obj_guard.is_kind_of(KindOf::CashGenerator) {
                if obj_guard.get_controlling_player_id() == Some(self.player_id as _) {
                    let p = obj_guard.get_position();
                    own_cash_gens.push(LeftoverOwnedCashGenerator { x: p.x, y: p.y });
                }
            }
            if !obj_guard.is_kind_of(KindOf::Structure)
                || !obj_guard.is_kind_of(KindOf::SupplySource)
            {
                continue;
            }
            let is_enemy = obj_guard
                .get_team()
                .and_then(|team_arc| {
                    team_arc.read().ok().map(|team| {
                        player_guard.get_relationship_with_team(&team) == Relationship::Enemies
                    })
                })
                .unwrap_or(false);
            let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate") else {
                continue;
            };
            let boxes = module.with_module(|module| {
                module
                    .get_supply_warehouse_dock_interface()
                    .map(|warehouse| warehouse.boxes_stored())
            });
            let Some(boxes) = boxes else {
                continue;
            };
            let center = *obj_guard.get_position();
            candidates.push(LeftoverSupplyCenterCandidate {
                id: obj_id,
                x: center.x,
                y: center.y,
                bounding_circle: obj_guard.get_geometry_info().get_bounding_circle_radius(),
                available_cash: boxes * BASE_VALUE_PER_SUPPLY_BOX,
                is_structure: true,
                is_supply_source: true,
                has_warehouse_dock: true,
                is_enemy,
            });
        }
        // leftover_find_supply_center: SUPPLY_CENTER_CLOSE_DIST, dist_sqr * 0.4,
        // enemy_dist_sqr * 0.6, cash_floor /= 2, if cash_floor <= 100
        let enemy = has_enemy.then_some((enemy_center.x, enemy_center.y));
        let best_id = leftover_find_supply_center(
            &candidates,
            &own_cash_gens,
            base_center.x,
            base_center.y,
            enemy,
            minimum_cash,
        )?;
        OBJECT_REGISTRY.get_object(best_id)
    }

    /// Legalize helper for buildBySupplies / near-team placement.
    /// C++ seed: NO_OBJECT_OVERLAP; wiggle: CLEAR_PATH|TERRAIN|NO_OBJECT_OVERLAP.
    pub(super) fn find_valid_build_location(
        &self,
        location: &Coord3D,
        template_name: &str,
        angle: Real,
    ) -> Option<Coord3D> {
        let seed_validator =
            FoundationValidator::from_build_options(LocalLegalToBuildOptions::NO_OBJECT_OVERLAP);
        if seed_validator
            .validate_placement(location, template_name, angle, self.player_id as ObjectID)
            .is_ok()
        {
            // C++ keeps seed when NO_OBJECT_OVERLAP already passes.
            return Some(*location);
        }

        let wiggle_validator = FoundationValidator::from_build_options(
            LocalLegalToBuildOptions::CLEAR_PATH
                | LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
        );

        let mut pos_offset = 0.0;
        while pos_offset < 2.0 * SUPPLY_CENTER_CLOSE_DIST {
            let offset = pos_offset * 0.5;
            let mut x = location.x - offset;
            let y = location.y - offset;

            while x <= location.x + offset {
                let mut candidate = Coord3D::new(x, y, location.z);
                if wiggle_validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                candidate.y = y + pos_offset;
                if wiggle_validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                x += PATHFIND_CELL_SIZE_F;
            }

            let mut y_pos = location.y - offset;
            let x_pos = location.x - offset;
            while y_pos <= location.y + offset {
                let mut candidate = Coord3D::new(x_pos, y_pos, location.z);
                if wiggle_validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                candidate.x = x_pos + pos_offset;
                if wiggle_validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                y_pos += PATHFIND_CELL_SIZE_F;
            }

            pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
        }
        None
    }

    /// Calculate superweapon target location
    /// C++ `AIPlayer::computeSuperweaponTarget` (AIPlayer.cpp).
    ///
    /// Grid-sample enemy structure bounds (or map extent), randomize scan
    /// direction, score with getPlayerSuperweaponValue, then fine-tune.
    /// Preserves C++ fine-tune `(x-5)` on both axes (legacy bug).
    /// `player_index` is C++ `playerNdx` — player whose structures are scored.
    pub fn compute_superweapon_target(
        &self,
        power_template: &str,
        weapon_radius: Real,
        player_index: i32,
    ) -> Result<Option<Coord3D>, AiError> {
        // Prefer explicit playerNdx (C++). Fall back to current enemy only when
        // caller passes a negative / invalid index residual.
        let enemy_index = if player_index >= 0 {
            player_index
        } else {
            match self.select_current_enemy_player() {
                Ok(Some((_, idx))) => idx,
                _ => return Ok(None),
            }
        };

        let radius = weapon_radius.max(1.0);
        let (mut min_bounds, mut max_bounds) = self.get_player_structure_bounds(enemy_index)?;

        // Degenerate bounds (no buildings) → full map extent (C++ getExtent, not pathfind).
        if min_bounds.x == 0.0 && min_bounds.y == 0.0 && max_bounds.x == 0.0 && max_bounds.y == 0.0
        {
            if let Some(terrain) = TheTerrainLogic::get() {
                let extent = terrain.get_extent();
                min_bounds = extent.lo;
                max_bounds = extent.hi;
            }
        }

        // Shrink by weapon radius (C++ only shrinks X then clamps both axes).
        min_bounds.x += radius;
        max_bounds.x -= radius;
        if max_bounds.x < min_bounds.x {
            let mid = (max_bounds.x + min_bounds.x) / 2.0;
            max_bounds.x = mid;
            min_bounds.x = mid;
        }
        if max_bounds.y < min_bounds.y {
            let mid = (max_bounds.y + min_bounds.y) / 2.0;
            max_bounds.y = mid;
            min_bounds.y = mid;
        }

        let width = (max_bounds.x - min_bounds.x).max(0.0);
        let height = (max_bounds.y - min_bounds.y).max(0.0);
        // C++: REAL_TO_INT_CEIL(bounds.width()/weaponRadius)+1, cap 10.
        let mut x_count = (width / radius).ceil() as i32 + 1;
        let mut y_count = (height / radius).ceil() as i32 + 1;
        if x_count > 10 {
            x_count = 10;
        }
        if y_count > 10 {
            y_count = 10;
        }
        if x_count < 1 {
            x_count = 1;
        }
        if y_count < 1 {
            y_count = 1;
        }

        let power = find_or_create_special_power_template(&AsciiString::from(power_template));
        // SPECIAL_SNEAK_ATTACK → do not value military units positively.
        let target_military_units = power.get_special_power_type()
            != crate::object::special_power_types::SpecialPowerType::SneakAttack;

        // C++ GameLogicRandomValue(1,4): starts at xCount/yCount (not count-1)
        // when scanning max→min so first sample hits the far edge.
        let (x_delta, y_delta, x_start, y_start) = match game_logic_random_value(1, 4) {
            1 => (1_i32, 1_i32, 0_i32, 0_i32),
            2 => (-1, 1, x_count, 0),
            3 => (1, -1, 0, y_count),
            _ => (-1, -1, x_count, y_count),
        };

        let mut best_cash: i32 = -1;
        let mut best_pos = Coord3D::new(min_bounds.x, min_bounds.y, 0.0);
        let mut x_index = x_start;
        for _ in 0..x_count {
            let mut y_index = y_start;
            for _ in 0..y_count {
                let pos = Coord3D::new(
                    min_bounds.x + (width * x_index as f32) / x_count as f32,
                    min_bounds.y + (height * y_index as f32) / y_count as f32,
                    0.0,
                );
                let value = self.get_player_superweapon_value(
                    &pos,
                    enemy_index,
                    2.0 * radius,
                    target_military_units,
                )?;
                if value > best_cash {
                    best_cash = value;
                    best_pos = pos;
                }
                y_index += y_delta;
            }
            x_index += x_delta;
        }

        // Fine tune: C++ uses (x-5) for BOTH axes (legacy bug — keep for parity).
        let mut fine_best = best_pos;
        let mut fine_cash: i32 = -1;
        let mut fine_count = 0_i32;
        let fine_steps = 11;
        for x in 0..fine_steps {
            for _y in 0..fine_steps {
                let offset = (x - 5) as f32 * (radius / 10.0);
                let pos = Coord3D::new(best_pos.x + offset, best_pos.y + offset, 0.0);
                let value = self.get_player_superweapon_value(
                    &pos,
                    enemy_index,
                    radius,
                    target_military_units,
                )?;
                if value > fine_cash {
                    fine_cash = value;
                    fine_best = pos;
                    fine_count = 1;
                } else if value == fine_cash {
                    // C++ averages equal-score samples.
                    fine_best.x += pos.x;
                    fine_best.y += pos.y;
                    fine_count += 1;
                }
            }
        }
        if fine_count > 1 {
            fine_best.x /= fine_count as f32;
            fine_best.y /= fine_count as f32;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            fine_best.z = terrain.get_ground_height(fine_best.x, fine_best.y, None);
        }

        // C++ success = (cash > -1)
        if fine_cash > -1 {
            Ok(Some(fine_best))
        } else {
            Ok(None)
        }
    }

    /// Called when a unit we're training comes into existence
    /// C++ `AIPlayer::onUnitProduced` (AIPlayer.cpp).
    ///
    /// Match work order by factoryID + incomplete + template equivalent; complete
    /// one unit; setTeam; clear factoryID; dozer/repair shortcuts; always
    /// `teamDelay = 0`.
    pub fn on_unit_produced(
        &mut self,
        factory_id: ObjectID,
        unit_id: ObjectID,
    ) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // C++: factory could be NULL at start of game.
        if factory_id == INVALID_ID {
            return Ok(());
        }

        let Some((unit_template_name, is_dozer)) = OBJECT_REGISTRY.with_object(unit_id, |unit_g| {
            (
                unit_g.get_template_name().to_string(),
                unit_g.is_kind_of(KindOf::Dozer),
            )
        }) else {
            self.team_delay = 0;
            return Ok(());
        };

        let mut found = false;
        let mut supply_truck = false;
        let mut matched_team_name: Option<String> = None;
        let mut matched_team: Option<Arc<RwLock<crate::team::Team>>> = None;
        let mut is_resource_gatherer_order = false;

        for team_q in &mut self.team_build_queue {
            if found {
                break;
            }
            for order in &mut team_q.work_orders {
                if order.factory_id != Some(factory_id) {
                    continue;
                }
                if order.num_completed >= order.num_required {
                    continue;
                }
                // C++ unit->getTemplate()->isEquivalentTo(order->m_thing)
                let equiv = order
                    .thing_template
                    .eq_ignore_ascii_case(&unit_template_name)
                    || TheThingFactory::find_template(&order.thing_template)
                        .zip(TheThingFactory::find_template(&unit_template_name))
                        .map(|(a, b)| a.is_equivalent_to(b.as_ref()))
                        .unwrap_or(false);
                if !equiv {
                    continue;
                }

                order.num_completed = order.num_completed.saturating_add(1);
                // C++ clears factory after matching this production slot.
                order.factory_id = None;
                is_resource_gatherer_order = order.is_resource_gatherer;
                matched_team_name = team_q.team_name.clone();
                matched_team = team_q.team.clone();

                if team_q.reinforcement {
                    team_q.reinforcement_id = Some(unit_id);
                }

                found = true;
                break;
            }
        }

        // put new unit into the team under construction
        if found {
            let team_name = matched_team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            // Prefer TeamInQueue.m_team (C++); name lookup is fallback.
            let team_arc = matched_team.or_else(|| {
                get_team_factory().lock().ok().and_then(|mut factory| {
                    factory
                        .find_team_instances(&team_name)
                        .into_iter()
                        .next()
                        .or_else(|| factory.find_team(&team_name))
                })
            });
            if let Some(ref team_arc) = team_arc {
                let _ = OBJECT_REGISTRY.with_object_mut(unit_id, |ug| {
                    let _ = ug.set_team(Some(team_arc.clone()));
                });
            }

            // C++: if team has homeLocation → aiFollowExitProductionPath(goal, home).
            // path[0] = *ai->getGoalPosition() (not path destination).
            let (home, has_home) =
                self.queue_units_home_for_team(team_arc.as_ref(), team_name.as_str());
            // has_home is true only for prototype homeLocation (not base-center fallback).
            if has_home {
                if let Some(ai) = OBJECT_REGISTRY
                    .with_object(unit_id, |unit_g| {
                        unit_g.get_ai_update_interface().map(|ai| {
                            let start = ai
                                .get_goal_position()
                                .unwrap_or_else(|| *unit_g.get_position());
                            (ai, start)
                        })
                    })
                    .flatten()
                {
                    let (ai, start) = ai;
                    let path = [start, home];
                    ai.ai_follow_exit_production_path(&path, None, CommandSourceType::FromAi);
                }
            }

            // Supply truck force-wanting + dock (C++ SupplyTruckAIInterface).
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(unit_id, |unit_g| unit_g.get_ai_update_interface())
                .flatten()
            {
                if let Ok(mut ai_g) = ai.lock() {
                    if let Some(truck) = ai_g.get_supply_truck_ai_interface_mut() {
                        supply_truck = is_resource_gatherer_order;
                        truck.set_force_wanting_state(supply_truck);
                    }
                }
                if supply_truck {
                    // C++: assign to first supply build-list entry needing gatherers,
                    // then aiDock(obj, CMD_FROM_PLAYER).
                    if let Some(dock_id) = self.take_supply_gatherer_slot() {
                        ai.ai_dock(dock_id, CommandSourceType::FromPlayer);
                    }
                }
            }
        }

        // C++ dozer path is NOT gated on `found` (AIPlayer.cpp after the queue loop).
        // supplyTruck defaults false unless a matched order set force-wanting true —
        // C++ leaves it uninitialized when no SupplyTruckAI; treat unset as false.
        if !supply_truck && is_dozer {
            if self.dozer_queued_for_repair {
                self.repair_dozer = Some(unit_id);
                self.dozer_queued_for_repair = false;
            } else {
                self.build_delay = 0;
                self.structure_timer = 1;
            }
        }

        if !found {
            log::debug!("***AI PLAYER-Unit not found in production queue.");
        }

        // C++ always: m_teamDelay = 0
        self.team_delay = 0;
        Ok(())
    }

    /// Called when a structure we're building comes into existence
    /// C++ `AIPlayer::onStructureProduced` (AIPlayer.cpp).
    ///
    /// Match build-list by objectID: clear UC, upgrades, script attach residual,
    /// checkForSupplyCenter. Else match rebuild hole spawn and retarget list ID.
    pub fn on_structure_produced(
        &mut self,
        _factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // C++: m_teamDelay = 0; m_buildDelay = 0; (no frameLastBuildingBuilt here)
        self.team_delay = 0;
        self.build_delay = 0;

        if OBJECT_REGISTRY.with_object(structure_id, |_| ()).is_none() {
            return Ok(());
        }

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return Ok(());
        };

        // Pass 1: exact objectID match on build list.
        // Do NOT call check_for_supply_center while holding player write —
        // it re-acquires the same lock (would deadlock on std::sync::RwLock).
        // C++ order: map props → clear UC → upgrades → script cache → supply.
        let mut matched = false;
        let mut script_name = String::new();
        {
            let Ok(mut player_guard) = player_arc.write() else {
                return Ok(());
            };
            if let Some(info) = player_guard.get_build_list_mut() {
                let mut current = Some(&mut *info);
                while let Some(node) = current {
                    if node.get_object_id() == structure_id {
                        // C++ Dict: objectName/script/health/unsellable → map props.
                        let mut props = crate::common::Dict::new();
                        props.set_ascii_string(
                            crate::common::well_known_keys::key_object_name(),
                            node.get_building_name().as_str(),
                        );
                        props.set_ascii_string(
                            crate::common::well_known_keys::key_object_script_attachment(),
                            node.get_script().as_str(),
                        );
                        props.set_int(
                            crate::common::well_known_keys::key_object_initial_health(),
                            node.get_health(),
                        );
                        props.set_bool(
                            crate::common::well_known_keys::key_object_unsellable(),
                            node.get_unsellable(),
                        );
                        script_name = node.get_script().to_string();
                        node.set_under_construction(false);

                        let _ = OBJECT_REGISTRY.with_object_mut(structure_id, |sg| {
                            sg.update_obj_values_from_map_properties(&props);
                            let mask = ObjectStatusMaskType::from_status(
                                ObjectStatusTypes::UnderConstruction,
                            ) | ObjectStatusMaskType::from_status(
                                ObjectStatusTypes::Reconstructing,
                            );
                            sg.clear_status(mask);
                            // UnderConstruction just cleared → refresh upgrades.
                            sg.update_upgrade_modules_from_player();
                        });

                        matched = true;
                        break;
                    }
                    current = node.get_next_mut();
                }
            }
        }
        if matched {
            // C++ TheScriptEngine->addObjectToCache + runObjectScript
            if let Ok(mut eng) = get_script_engine().write() {
                if let Some(e) = eng.as_mut() {
                    e.add_object_to_cache(structure_id);
                    if !script_name.is_empty() {
                        e.run_object_script(&script_name, structure_id);
                    }
                }
            }
            // C++ checkForSupplyCenter(info, bldg) after script — outside player write.
            let _ = self.check_for_supply_center(structure_id);
            return Ok(());
        }

        // Pass 2: rebuild-hole spawn retarget (C++ getReconstructedBuildingID).
        let structure_template_name = OBJECT_REGISTRY
            .with_object(structure_id, |g| g.get_template_name().to_string())
            .unwrap_or_default();
        {
            let Ok(mut player_guard) = player_arc.write() else {
                return Ok(());
            };
            if let Some(info) = player_guard.get_build_list_mut() {
                let mut current = Some(&mut *info);
                while let Some(node) = current {
                    let name = node.get_template_name().to_string();
                    let equiv = TheThingFactory::find_template(&name)
                        .zip(TheThingFactory::find_template(&structure_template_name))
                        .map(|(a, b)| a.is_equivalent_to(b.as_ref()))
                        .unwrap_or(false)
                        || name.eq_ignore_ascii_case(&structure_template_name);
                    if !equiv {
                        current = node.get_next_mut();
                        continue;
                    }
                    let list_id = node.get_object_id();
                    if list_id != INVALID_ID {
                        if OBJECT_REGISTRY
                            .with_object(list_id, |hole_g| {
                                if !hole_g.is_kind_of(KindOf::RebuildHole) {
                                    return false;
                                }
                                // C++: only if bldg->getID() == rhbi->getReconstructedBuildingID().
                                let mut is_this_spawn = false;
                                let mut saw_rhbi = false;
                                for behavior in hole_g.get_behavior_modules() {
                                    if let Ok(mut bg) = behavior.lock() {
                                        if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface()
                                        {
                                            saw_rhbi = true;
                                            let rebuilt = rhbi.get_reconstructed_building_id();
                                            is_this_spawn = rebuilt == structure_id;
                                            break;
                                        }
                                    }
                                }
                                saw_rhbi && is_this_spawn
                            })
                            .unwrap_or(false)
                        {
                            log::debug!("AI got rebuilt {}", name);
                            node.set_object_id(structure_id);
                            matched = true;
                            break;
                        }
                    }
                    current = node.get_next_mut();
                }
            }
        }

        if !matched && TheGameLogic::get_frame() > 0 {
            log::debug!("***AI PLAYER-Structure not found in production queue.");
        }
        Ok(())
    }

    /// Set team delay in seconds
    pub fn set_team_delay_seconds(&mut self, delay: Real) {
        self.team_seconds = delay.max(0.0);
    }

    /// C++ `AIPlayer::calcClosestConstructionZoneLocation` (AIPlayer.cpp).
    ///
    /// Seed check: NO_OBJECT_OVERLAP only. If illegal, wiggle with
    /// CLEAR_PATH | TERRAIN_RESTRICTIONS | NO_OBJECT_OVERLAP.
    /// Returns Some only when the wiggle path set `valid` — matching GeneralsMD
    /// where an already-legal seed leaves `valid=false` and fails (location zeroed).
    pub fn calc_closest_construction_zone_location(
        &self,
        template_name: &str,
        location: &Coord3D,
    ) -> Result<Option<Coord3D>, AiError> {
        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };
        let angle = template.get_placement_view_angle();
        // C++ first gate: NO_OBJECT_OVERLAP only (builder NULL).
        let seed_validator =
            FoundationValidator::from_build_options(LocalLegalToBuildOptions::NO_OBJECT_OVERLAP);
        // C++ wiggle options: CLEAR_PATH | TERRAIN_RESTRICTIONS | NO_OBJECT_OVERLAP.
        let wiggle_validator = FoundationValidator::from_build_options(
            LocalLegalToBuildOptions::CLEAR_PATH
                | LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
        );

        // C++: Bool valid = false; only set true inside the adjust loop.
        let mut valid = false;
        let mut new_pos = *location;

        let initial_ok = seed_validator
            .validate_placement(location, template_name, angle, self.player_id as ObjectID)
            .is_ok();
        if !initial_ok {
            log::debug!(
                "{} - calcClosestConstructionZoneLocation unable to place.  Attempting to adjust position.",
                template_name
            );
            // Wiggle spiral (same extents as C++ 2*SUPPLY_CENTER_CLOSE_DIST).
            let mut pos_offset = 0.0_f32;
            'outer: while pos_offset < 2.0 * SUPPLY_CENTER_CLOSE_DIST {
                let offset = pos_offset * 0.5;
                let mut x = location.x - offset;
                let y0 = location.y - offset;
                while x <= location.x + offset + 0.001 {
                    for y in [y0, y0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, location.z);
                        if wiggle_validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            new_pos = candidate;
                            valid = true;
                            break 'outer;
                        }
                    }
                    x += PATHFIND_CELL_SIZE_F;
                }
                let mut y = location.y - offset;
                let x0 = location.x - offset;
                while y <= location.y + offset + 0.001 {
                    for x in [x0, x0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, location.z);
                        if wiggle_validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            new_pos = candidate;
                            valid = true;
                            break 'outer;
                        }
                    }
                    y += PATHFIND_CELL_SIZE_F;
                }
                pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
            }
        }
        // C++: if (valid) location=newPos success; else location.zero() fail.
        // Note: when initial_ok, valid stays false → None (C++ shipped behavior).
        let _ = initial_ok;
        if valid { Ok(Some(new_pos)) } else { Ok(None) }
    }

    /// Convenience: search near base center when no seed location given.
    pub fn calc_closest_construction_zone_near_base(
        &self,
        template_name: &str,
    ) -> Result<Option<Coord3D>, AiError> {
        if !self.base_center_set {
            return Ok(None);
        }
        self.calc_closest_construction_zone_location(template_name, &self.base_center)
    }
}
