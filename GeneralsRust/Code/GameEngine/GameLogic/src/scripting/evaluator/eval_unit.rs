// Unit health, object-status, container, and emptied conditions
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_unit_health_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("UnitHealth condition missing unit parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing comparison parameter".to_string(),
            )
        })?;
        let health_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing health parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_percent = health_param.get_int() as i64;

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        return OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                let max_health = obj_guard.get_max_health();
                if max_health <= f32::EPSILON {
                    return Ok(false);
                }
                let cur_health = obj_guard.get_health();
                let cur_percent = ((cur_health * 100.0) + (max_health / 2.0)) / max_health;
                let cur_percent = cur_percent.round() as i64;

                match comparison {
                    0 => Ok(cur_percent < target_percent),  // LessThan
                    1 => Ok(cur_percent <= target_percent), // LessEqual
                    2 => Ok(cur_percent == target_percent), // Equal
                    3 => Ok(cur_percent >= target_percent), // GreaterEqual
                    4 => Ok(cur_percent > target_percent),  // Greater
                    5 => Ok(cur_percent != target_percent), // NotEqual
                    _ => Err(GameLogicError::Configuration(format!(
                        "Invalid comparison type: {}",
                        comparison
                    ))),
                }
            })
            .unwrap_or(Ok(false));
    }

    fn evaluate_unit_has_object_status_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing unit parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let status_mask = status_param.get_object_status();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        return OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                Ok(obj_guard.get_status_bits().intersects(status_mask))
            })
            .unwrap_or(Ok(false));
    }

    fn evaluate_team_has_object_status_condition(
        &self,
        condition: &Condition,
        entire_team: bool,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing team parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let status_mask = status_param.get_object_status();

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(false);
        }

        for team_id in teams {
            let Some(members) = crate::team::with_team(team_id, |team_guard| {
                team_guard.get_members().to_vec()
            }) else {
                return Ok(false);
            };

            for member_id in members {
                let early = OBJECT_REGISTRY.with_object(member_id, |obj_guard| {
                    let has_status = obj_guard.get_status_bits().intersects(status_mask);
                    if entire_team && !has_status {
                        Some(Ok(false))
                    } else if !entire_team && has_status {
                        Some(Ok(true))
                    } else {
                        None
                    }
                });
                if let Some(v) = early.flatten() {
                    return v;
                }
            }
        }

        Ok(entire_team)
    }

    fn evaluate_player_acquired_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing science parameter".to_string(),
            )
        })?;

        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        // C++ goes through ScriptConditions::playerFromParam here.  Besides a
        // literal side name, that accepts the serialized player mask and the
        // legacy <Local Player>/<This Player> tokens used by campaign scripts.
        let Some(player_index) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };

        if self
            .with_evaluation_engine_mut(|engine| {
                engine.is_science_acquired(player_index as usize, science, true)
            })
            .unwrap_or(false)
        {
            return Ok(true);
        }

        let player_name = crate::player::with_player(player_index, |p| {
            NameKeyGenerator::key_to_name(p.get_player_name_key())
        })
        .flatten()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| player_param.get_string().to_string());
        if crate::scripting::host_query_player_has_science(&player_name, science_name)
            .unwrap_or(false)
        {
            let _ = self.with_evaluation_engine_mut(|engine| {
                engine.notify_of_acquired_science(player_index as usize, science);
                engine.is_science_acquired(player_index as usize, science, true)
            });
            return Ok(true);
        }
        Ok(false)
    }

    fn evaluate_player_has_science_purchase_points_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing player parameter".to_string(),
            )
        })?;
        let points_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing points parameter".to_string(),
            )
        })?;

        let points_needed = points_param.get_int();

        let player_name = self
            .resolve_player_from_param(player_param)
            .and_then(|p| {
                crate::player::with_player(p, |g| {
                    NameKeyGenerator::key_to_name(g.get_player_name_key())
                })
                .flatten()
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| player_param.get_string().to_string());
        if let Some(pts) =
            crate::scripting::host_query_player_science_purchase_points(&player_name)
        {
            return Ok(pts >= points_needed);
        }

        // Match C++ ScriptConditions::playerFromParam rather than treating a
        // legacy player token as a literal display name.
        let Some(player_index) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        Ok(crate::player::with_player(player_index, |player_guard| {
            player_guard.get_science_purchase_points() >= points_needed
        })
        .unwrap_or(false))
    }

    fn evaluate_player_can_purchase_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing science parameter".to_string(),
            )
        })?;

        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        // C++ ScriptConditions::playerFromParam supports both exact player
        // identities and legacy Side tokens/masks for this condition.
        let Some(player_index) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        if crate::player::with_player(player_index, |player_guard| {
            player_guard.is_capable_of_purchasing_science(science)
        })
        .unwrap_or(false)
        {
            return Ok(true);
        }

        let player_name = crate::player::with_player(player_index, |p| {
            NameKeyGenerator::key_to_name(p.get_player_name_key())
        })
        .flatten()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| player_param.get_string().to_string());
        if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
            if crate::scripting::host_query_player_has_science(&player_name, science_name)
                .unwrap_or(false)
            {
                return Ok(false);
            }
            if let Some(store) = get_science_store() {
                let cost = store.get_science_purchase_cost(science);
                if cost > 0 && cost <= census.science_purchase_points {
                    let owned: Vec<game_engine::common::rts::ScienceType> = census
                        .unlocked_sciences
                        .iter()
                        .map(|n| store.get_science_from_internal_name(n))
                        .filter(|s| *s != SCIENCE_INVALID)
                        .collect();
                    struct Access(Vec<game_engine::common::rts::ScienceType>);
                    impl game_engine::common::rts::science::ScienceAccess for Access {
                        fn has_science(&self, s: game_engine::common::rts::ScienceType) -> bool {
                            self.0.contains(&s)
                        }
                    }
                    if store.player_has_prereqs_for_science(&Access(owned), science) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn evaluate_named_has_free_container_slots_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedHasFreeContainerSlots condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        return OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                let Some(contain) = obj_guard.get_contain() else {
                    return Ok(false);
                };
                Ok(contain.get_contained_count() < contain.get_max_capacity())
            })
            .unwrap_or(Ok(false));
    }

    fn evaluate_unit_emptied_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitEmptied condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(num_peeps) = crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
            obj_guard
                .get_contain()
                .map(|contain| contain.get_contained_count())
                .unwrap_or(0)
        }) else {
            return Ok(false);
        };

        let frame = TheGameLogic::get_frame();
        let mut statuses = TRANSPORT_STATUSES.write().map_err(|e| {
            GameLogicError::Threading(format!("Transport status lock error: {}", e))
        })?;

        let entry = statuses.entry(object_id).or_insert((frame, num_peeps));
        if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
            return Ok(true);
        }

        *entry = (frame, num_peeps);
        Ok(false)
    }
}
