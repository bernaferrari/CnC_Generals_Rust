use super::*;

impl GameLogic {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_gathering_support_state(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        is_alive: bool,
        dt: f32,
        tree_move_frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Retail GameData.ini `ValuePerSupplyBox = 75` (ZH override of
        // C++ GlobalData.cpp default 100). Player::getSupplyBoxValue
        // (Player.cpp:1928-1930) reads TheGlobalData->m_baseValuePerSupplyBox.
        const SUPPLY_BOX_VALUE: u32 =
            crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32;

        // C++ WorkerAIUpdate.cpp:283-287 — harvest path arms
        // WEAPONSET_MINE_CLEARING_DETAIL so GLA workers can be
        // diverted onto mines while ferrying.
        self.arm_worker_harvest_mine_clearing(object_id);
        // C++ ChinookAIUpdate::isAvailableForSupplying
        // (ChinookAIUpdate.cpp:982-991): loaded / enter-exit
        // pending Chinooks must not auto-dock warehouses.
        if !self.collector_available_for_supplying(object_id) {
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
        }

        let Some(source_id) = target_id else {
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
        };

        // Extract source state before any mutations.
        let (source_alive, source_pos, source_supplies, source_is_warehouse, delete_when_empty) =
            self.objects
                .get(&source_id)
                .map(|s| {
                    (
                        s.is_alive(),
                        s.get_position(),
                        s.stored_resources.supplies,
                        s.thing().template.dock_kind
                            == crate::game_logic::DockKind::SupplyWarehouse,
                        s.thing().template.dock_delete_when_empty,
                    )
                })
                .unwrap_or((false, position, 0, false, false));

        if !source_alive {
            // C++ dock session ends → WANTING. Cargo banks first.
            self.route_supply_wanting(object_id, team, owner_player_id, position, can_move)?;
            return Ok(());
        }

        let collector_metadata_early = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing().template.supply_truck_metadata);
        if source_is_warehouse && collector_metadata_early.is_some() {
            let docker_r = self
                .objects
                .get(&object_id)
                .map(|o| {
                    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                        (*o.geometry_info()).authored,
                        (*o.geometry_info()).bounding_circle_radius(),
                        o.thing().geometry.radius,
                    )
                })
                .unwrap_or(1.0);
            let warehouse_r = self
                .objects
                .get(&source_id)
                .map(|o| {
                    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                        (*o.geometry_info()).authored,
                        (*o.geometry_info()).bounding_circle_radius(),
                        o.thing().geometry.radius,
                    )
                })
                .unwrap_or(0.0);
            if crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                (position.x, position.z),
                (source_pos.x, source_pos.z),
                docker_r,
                warehouse_r,
            ) {
                // C++ aiDock: path to the reserved approach, and only
                // transfer a box once that claim is ClearToAct.
                if !(self.try_claim_dock(source_id, object_id)?) {
                    let close = docker_r * 2.0;
                    if !(can_move && position.distance(source_pos) > close + 1.0) {
                        let (dx, dz) =
                            crate::game_logic::host_supply_gather::warehouse_twitch_delta(
                                crate::game_logic::host_supply_gather::twitch_seed(
                                    object_id, self.frame,
                                ),
                                1,
                            );
                        if let Some(mut pos) = self
                            .objects
                            .get(&object_id)
                            .map(|object| object.get_position())
                        {
                            pos.x += dx;
                            pos.z += dz;
                            self.set_owned_object_position_with_effects(
                                object_id,
                                pos,
                                tree_move_frame,
                                effects,
                            );
                        }
                    }
                    return Ok(());
                }
            }
        } else if can_move && position.distance(source_pos) > INTERACT_RANGE {
            self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
            return Ok(());
        }
        if source_is_warehouse && !(self.try_claim_dock(source_id, object_id)?) {
            return Ok(());
        }

        // C++ AIDock waits for the authored warehouse action
        // delay, then SupplyWarehouseDockUpdate transfers one
        // box. Only an authored SupplyTruckAIUpdate uses this
        // path; the older generic harvest command remains intact.
        let collector_metadata = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing().template.supply_truck_metadata);
        if let Some(metadata) = collector_metadata {
            let (state, next_frame) = self
                .objects
                .get(&object_id)
                .map(|object| {
                    (
                        object.supply_truck_state,
                        object.supply_truck_next_dock_action_frame,
                    )
                })
                .unwrap_or((SupplyTruckState::Idle, 0));
            if state != SupplyTruckState::DockingWarehouse {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.supply_truck_force_pending = false;
                    object.supply_truck_state = SupplyTruckState::DockingWarehouse;
                    object.supply_truck_next_dock_action_frame =
                        self.frame.saturating_add(metadata.warehouse_delay_frames);
                }
                return Ok(());
            }
            if self.frame < next_frame {
                return Ok(());
            }
        }

        // In range — gather resources.  The host tracks cash
        // value rather than C++ individual boxes, but a warehouse
        // still cannot grant more than its authored stock.  This
        // avoids turning an empty `SupplyWarehouseDockUpdate`
        // into an infinite source.
        let gather_amount = collector_metadata
            .map(|_| SUPPLY_BOX_VALUE)
            .unwrap_or_else(|| (100.0 * dt) as u32);
        let max_carry = collector_metadata
            .map(|metadata| metadata.max_boxes.saturating_mul(SUPPLY_BOX_VALUE))
            .unwrap_or(1000);
        let current_carry = self
            .objects
            .get(&object_id)
            .map(|o| o.stored_resources.supplies)
            .unwrap_or(0);
        // C++ SupplyTruckAIUpdate::gainOneBox (SupplyTruckAIUpdate.cpp:134-135)
        // fails when m_numberBoxes >= m_maxBoxesData. Warehouse action
        // (SupplyWarehouseDockUpdate.cpp:89-111) then ++m_boxesStored
        // to take the tentative debit back.
        let already_at_max_boxes = collector_metadata.is_some_and(|metadata| {
            let (_remaining, _carry, transferred) =
                crate::game_logic::host_supply_gather::warehouse_action_transfer_one_box(
                    source_supplies / SUPPLY_BOX_VALUE,
                    current_carry / SUPPLY_BOX_VALUE,
                    metadata.max_boxes,
                );
            !transferred && current_carry / SUPPLY_BOX_VALUE >= metadata.max_boxes
        });
        // Keep the legacy generic-resource path intact.  The
        // precise stock gate is required specifically for the
        // newly-authored SupplyWarehouseDockUpdate target.
        let taken = if source_is_warehouse {
            gather_amount.min(source_supplies)
        } else {
            gather_amount
        };
        if source_is_warehouse {
            let crippled = self.objects.get(&source_id).is_some_and(|s| {
                matches!(
                    s.body_damage_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                        | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                )
            });
            if crippled {
                let airborne = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|o| o.is_kind_of(KindOf::Aircraft) || o.status.airborne_target);
                let docker_r = self
                    .objects
                    .get(&object_id)
                    .map(|o| {
                        crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                            (*o.geometry_info()).authored,
                            (*o.geometry_info()).bounding_circle_radius(),
                            o.thing().geometry.radius,
                        )
                    })
                    .unwrap_or(1.0);
                let warehouse_r = self
                    .objects
                    .get(&source_id)
                    .map(|o| {
                        crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                            (*o.geometry_info()).authored,
                            (*o.geometry_info()).bounding_circle_radius(),
                            o.thing().geometry.radius,
                        )
                    })
                    .unwrap_or(0.0);
                let inside = !crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                    (position.x, position.z),
                    (source_pos.x, source_pos.z),
                    docker_r,
                    warehouse_r,
                );
                match crate::game_logic::host_supply_gather::dock_cripple_victim_action(
                                true, inside, airborne,
                            ) {
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::KillGround => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        Self::mark_object_destroyed_authority_aware(
                                            obj,
                                            None,
                                            &mut self.health_events,
                                        );
                                    }
                                    self.mark_object_for_destruction_with_effects(object_id, None, effects)?;
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::IdleAndForceWanting => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.supply_truck_force_pending = true;
                                        obj.supply_truck_state = SupplyTruckState::Wanting;
                                    }
                                    self.stop_attack_decision_aware(object_id);
                                    self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::None => {}
                            }
                self.release_dock_if_holder(source_id, object_id);

                return Ok(());
            }
        }
        if source_is_warehouse && already_at_max_boxes {
            self.release_dock_if_holder(source_id, object_id);
            // C++ aiDock(bestCenter): DockUpdate approach, not the center.
            self.begin_return_to_supply_center(object_id, team, owner_player_id, position)?;
            return Ok(());
        }
        if source_is_warehouse && taken == 0 {
            self.release_dock_if_holder(source_id, object_id);
            // C++ action() FALSE → AIDock SUCCESS → WANTING.
            // Partial cargo banks; empty seeks another warehouse.
            self.route_supply_wanting(object_id, team, owner_player_id, position, can_move)?;
            return Ok(());
        }
        let is_full = current_carry + taken >= max_carry;

        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_stored_supplies(
                obj.stored_resources
                    .supplies
                    .saturating_add(taken)
                    .min(max_carry),
            );
            if let Some(metadata) = collector_metadata {
                obj.supply_truck_next_dock_action_frame =
                    self.frame.saturating_add(metadata.warehouse_delay_frames);
            }
        }

        let remaining_after = source_supplies.saturating_sub(taken);
        // Deplete the supply source.
        if let Some(source) = self.objects.get_mut(&source_id) {
            source.set_stored_supplies(remaining_after);
            if remaining_after == 0 && (!source_is_warehouse || delete_when_empty) {
                Self::mark_object_destroyed_authority_aware(source, None, &mut self.health_events);
                self.mark_object_for_destruction_with_effects(source_id, None, effects)?;
            }
        }
        if remaining_after == 0 && collector_metadata.is_some() {
            let scan = self
                .collector_warehouse_scan(object_id, owner_player_id)
                .unwrap_or(0.0);
            let next_dist = self
                .find_nearest_harvestable_supply_within(
                    team,
                    position,
                    Some(scan).filter(|d| *d > 0.0),
                    object_id,
                )
                .and_then(|nid| {
                    self.objects
                        .get(&nid)
                        .map(|s| s.get_position().distance(position))
                });
            let voice = self
                .objects
                .get(&object_id)
                .map(|o| o.thing().template.supplies_depleted_voice.clone())
                .unwrap_or_default();
            if crate::game_logic::host_supply_gather::should_play_supplies_depleted_voice(
                next_dist, scan, &voice,
            ) {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&voice)
                        .with_object(object_id)
                        .with_position(position)
                        .with_priority(160),
                );
            }
        }

        if source_is_warehouse && (is_full || remaining_after == 0) {
            self.release_dock_if_holder(source_id, object_id);
        }

        if is_full {
            self.begin_return_to_supply_center(object_id, team, owner_player_id, position)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_returning_resources_support_state(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        is_alive: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.arm_worker_harvest_mine_clearing(object_id);
        if !self.collector_available_for_supplying(object_id) {
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
        }
        // Deposit resources when close to a supply center.
        let refinery_id =
            self.preferred_or_allied_supply_center(object_id, team, owner_player_id, position);

        if let Some(rid) = refinery_id {
            if !(self.try_claim_dock(rid, object_id)?) {
                return Ok(());
            }

            let collector_metadata = self
                .objects
                .get(&object_id)
                .and_then(|object| object.thing().template.supply_truck_metadata);
            if let Some(metadata) = collector_metadata {
                let (state, next_frame) = self
                    .objects
                    .get(&object_id)
                    .map(|object| {
                        (
                            object.supply_truck_state,
                            object.supply_truck_next_dock_action_frame,
                        )
                    })
                    .unwrap_or((SupplyTruckState::Idle, 0));
                if state != SupplyTruckState::DockingCenter {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.supply_truck_state = SupplyTruckState::DockingCenter;
                        object.supply_truck_next_dock_action_frame =
                            self.frame.saturating_add(metadata.center_delay_frames);
                    }
                    return Ok(());
                }
                if self.frame < next_frame {
                    return Ok(());
                }
            }
            // Deposit.
            // C++ SupplyCenterDockUpdate::action always returns FALSE
            // (AIDock SUCCESS) even at 0 boxes, then banks
            // getUpgradedSupplyBoost (Supply Lines / Worker Shoes).
            // Never leave the exclusive dock claimed.
            let deposit_amount = self
                .objects
                .get(&object_id)
                .map(|o| o.stored_resources.supplies)
                .unwrap_or(0);

            // Snapshot carrier for C++ getUpgradedSupplyBoost identity.
            let (
                carrier_is_gla_worker,
                carrier_has_worker_shoes,
                carrier_is_chinook,
                carrier_is_combat_chinook,
                carrier_authored_boost,
                carrier_has_supply_lines,
            ) = self
                .objects
                .get(&object_id)
                .map(|o| {
                    let is_w = crate::game_logic::host_gla_worker::is_gla_worker_template(
                        &o.template_name,
                    );
                    // C++ WorkerAIUpdate::getUpgradedSupplyBoost
                    // (WorkerAIUpdate.cpp:1376-1384): controlling
                    // player hasUpgradeComplete only. Object tags
                    // and any same Team enum leak 2v2 allies.
                    let shoes = self
                        .player_owner_for_host_object(o)
                        .and_then(|pid| self.players.get(&pid))
                        .is_some_and(|p| {
                            p.has_unlocked_upgrade(
                                crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                            )
                        });
                    // C++ ChinookAIUpdate::getUpgradedSupplyBoost
                    // (ChinookAIUpdate.cpp:1644-1652): controlling
                    // player hasUpgradeComplete only. A same-Team
                    // ally researching the upgrade pays nothing
                    // for this carrier.
                    let supply_lines = self
                        .player_owner_for_host_object(o)
                        .and_then(|pid| self.players.get(&pid))
                        .is_some_and(|p| {
                            p.has_unlocked_upgrade(
                                crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
                            )
                        });
                    let is_chinook =
                        crate::game_logic::host_supply_gather::is_chinook_supply_collector(
                            &o.template_name,
                        ) || o.chinook_ai.is_some()
                            || o.is_combat_chinook_style_container();
                    let is_combat = o.is_combat_chinook_style_container();
                    let authored = o
                        .thing()
                        .template
                        .supply_truck_metadata
                        .map(|m| m.upgraded_supply_boost)
                        .unwrap_or(0);
                    (is_w, shoes, is_chinook, is_combat, authored, supply_lines)
                })
                .unwrap_or((false, false, false, false, 0, false));

            // Clear carried resources.
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_stored_supplies(0);
            }
            // C++ ChinookAIUpdate::getUpgradedSupplyBoost
            // (ChinookAIUpdate.cpp:1644-1652): the CARRIER's
            // controlling player must have completed
            // Upgrade_AmericaSupplyLines; an allied researcher
            // alone pays nothing. Trucks return 0.
            let has_supply_lines = carrier_has_supply_lines;
            let supply_lines_boost =
                crate::game_logic::host_supply_gather::collector_supply_lines_boost(
                    carrier_is_chinook,
                    carrier_is_combat_chinook,
                    carrier_authored_boost,
                    has_supply_lines,
                );
            // GLA WorkerShoes residual: +8 per drop-off when unlocked.
            let worker_shoes_boost =
                crate::game_logic::host_gla_worker::residual_worker_shoes_drop_off_boost(
                    carrier_is_gla_worker,
                    carrier_has_worker_shoes,
                );
            let boost = supply_lines_boost.saturating_add(worker_shoes_boost);
            let credited = deposit_amount.saturating_add(boost);
            // Credit the player (carried supplies + optional economy boost).
            // Capture the concrete owner before the mutable
            // credit so the typed event below is tied to this
            // real ReturningResources deposit, not a later
            // resource-total observation or passive income.
            // C++ SupplyCenterDockUpdate::action credits the
            // *center* controlling player, so allied drop-offs
            // pay the dock owner instead of vanishing.
            let credited_player_id = refinery_id.and_then(|rid| {
                self.objects
                    .get(&rid)
                    .and_then(|center| self.player_owner_for_host_object(center))
            });

            if credited > 0 {
                if let Some(player_id) = credited_player_id {
                    let credited_player = if let Some((player, economy_events)) =
                        self.player_and_economy_events_mut(player_id)
                    {
                        player.credit_supplies(credited, economy_events);
                        // C++ SupplyCenterDockUpdate::action:
                        // deposit(value) and ScoreKeeper::addMoneyEarned(value).
                        player.add_money_earned(credited);
                        true
                    } else {
                        false
                    };
                    if credited_player {
                        self.record_supply_dropoff_event(crate::game_logic::SupplyDropoffEvent {
                            carrier_id: object_id,
                            player_id,
                            carried_amount: deposit_amount,
                        });
                        let center_status = self
                            .objects
                            .get(&refinery_id.expect("checked above"))
                            .map(|c| (c.status.stealthed, c.status.detected));
                        let local = self
                            .players
                            .get(&player_id)
                            .map(|_| self.is_local_player(player_id))
                            .unwrap_or(false);
                        let hide = center_status.is_some_and(|(stealth, detected)| {
                            crate::game_logic::host_supply_gather::hide_stealth_supply_cash(
                                stealth, local, detected,
                            )
                        });

                        if !hide && credited > 0 {
                            let ground_y = self.terrain_height_at(position).unwrap_or(position.y);
                            let color = self
                                .players
                                .get(&player_id)
                                .map(|p| p.color_rgb)
                                .unwrap_or((0, 255, 0));
                            self.oil_derricks.record_floating_text(
                                            crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText {
                                                text: crate::game_logic::host_supply_gather::format_gui_add_cash(credited),
                                                text_key: crate::game_logic::host_supply_gather::SUPPLY_CENTER_ADD_CASH_KEY
                                                    .to_string(),
                                                position: glam::Vec3::new(position.x, ground_y, position.z),
                                                color_rgba: (
                                                    color.0,
                                                    color.1,
                                                    color.2,
                                                    crate::game_logic::host_supply_gather::SUPPLY_CENTER_FLOATING_TEXT_ALPHA,
                                                ),
                                                amount: credited,
                                                spawn_frame: self.frame,
                                                source_id: object_id,
                                                is_capture_bonus: false,
                                            },
                                        );
                        }
                    }
                }
            }
            if let Some(rid) = refinery_id {
                if credited > 0 {
                    self.grant_center_temporary_stealth(rid, object_id);
                }
                self.release_dock_if_holder(rid, object_id);
            }

            if supply_lines_boost > 0 {
                self.supply_lines_bonus_cash_total = self
                    .supply_lines_bonus_cash_total
                    .saturating_add(supply_lines_boost);
            }
            if worker_shoes_boost > 0 {
                self.gla_worker
                    .record_shoes_drop_off_boost(worker_shoes_boost);
            }
            if deposit_amount > 0 {
                let has_supply_truck = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|o| o.thing().template.supply_truck_metadata.is_some());
                let source_id = (!has_supply_truck)
                    .then_some(target_id)
                    .flatten()
                    .filter(|sid| self.objects.get(sid).is_some_and(|s| s.is_alive()));
                if let Some(sid) = source_id {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.supply_truck_state = SupplyTruckState::Wanting;
                        object.supply_truck_next_dock_action_frame = 0;
                    }
                    self.set_ai_state_decision_aware_with_effects(
                        object_id,
                        AIState::Gathering,
                        effects,
                    )?;
                    let warehouse = self.objects.get(&sid).is_some_and(|s| {
                        s.thing().template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse
                            || s.thing().template.dock_delete_when_empty
                            || s.template_name.to_ascii_lowercase().contains("supplypile")
                    });
                    // A warehouse PathTo already aims at the approach.
                    // A plain supply pile has no dock slots.
                    if !(self.try_claim_dock(sid, object_id)?) && !warehouse {
                        if let Some(dest) = self.objects.get(&sid).map(|s| s.get_position()) {
                            self.path_approach_with_state(object_id, dest, AIState::Gathering);
                        }
                    }
                } else if let Some(next) = self.find_nearest_harvestable_supply_within(
                    team,
                    position,
                    self.collector_warehouse_scan(object_id, owner_player_id),
                    object_id,
                ) {
                    let warehouse = self.objects.get(&next).is_some_and(|s| {
                        s.thing().template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse
                            || s.thing().template.dock_delete_when_empty
                            || s.template_name.to_ascii_lowercase().contains("supplypile")
                    });
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_order_target_with_operation(
                            Some(next),
                            &mut self.attack_events,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                            self.frame,
                            &mut effects.reborrow_for_object(&mut self.partition_manager),
                        );
                        obj.supply_truck_state = SupplyTruckState::Wanting;
                        obj.supply_truck_next_dock_action_frame = 0;
                    }
                    self.set_ai_state_decision_aware_with_effects(
                        object_id,
                        AIState::Gathering,
                        effects,
                    )?;
                    if warehouse {
                        let _claimed = self.try_claim_dock(next, object_id)?;
                    } else if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                        self.path_approach_with_state(object_id, dest, AIState::Gathering);
                    }
                } else {
                    self.begin_supply_regroup(object_id, team, owner_player_id, position)?;
                }
            } else {
                // C++ action() FALSE → AIDock SUCCESS → WANTING.
                self.route_supply_wanting(object_id, team, owner_player_id, position, can_move)?;
            }
        } else if can_move {
            let _ = can_move;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_docked_support_state(
        &mut self,
        object_id: ObjectId,
        ai_state: AIState,
        target_id: Option<ObjectId>,
        is_alive: bool,
        tree_move_frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Aircraft parking: C++ JetTakeoffOrLandingState reserves the
        // stall-column runway and keeps the hangar when
        // KeepsParkingSpaceWhenAirborne (JetAIUpdate.cpp:1630, 897-900).
        let wants_sortie = self
            .objects
            .get(&object_id)
            .map(|o| {
                (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && (o.movement.target_position.is_some()
                        || o.target.is_some()
                        || o.target_location.is_some())
            })
            .unwrap_or(false);
        if wants_sortie {
            let _ = self.try_runway_takeoff_from_airfield_with_effects(object_id, effects);
            return Ok(());
        }
        // Prefer contained_by (authoritative residual link) over target.
        let container_id = self
            .objects
            .get(&object_id)
            .and_then(|o| o.container_id())
            .or(target_id);
        let Some(container_id) = container_id else {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                let containment_actor_id = obj.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let obj = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        };

        let Some((container_pos, container_alive, container_has_unit, station_pin, enclosing)) =
            self.objects.get(&container_id).map(|container| {
                // C++ GarrisonContain::positionObjectsAtStationGarrisonPoints:
                // non-enclosing Fire Base stays on STATION, not building center.
                let enclosing = container.is_enclosing_garrison_container();
                let station_pin = if matches!(ai_state, AIState::Garrisoned) && !enclosing {
                    container.building_data.as_ref().and_then(|bd| {
                        bd.garrison_point_occupant
                            .iter()
                            .enumerate()
                            .find(|(_, id)| **id == Some(object_id))
                            .and_then(|(i, _)| bd.garrison_station_points.get(i).copied())
                    })
                } else {
                    None
                };
                (
                    container
                        .contained_by
                        .and_then(|outer| {
                            (self.redirected_containment_owner(outer) == container_id)
                                .then(|| self.host_object(outer).map(|host| host.get_position()))
                                .flatten()
                        })
                        .unwrap_or_else(|| container.get_position()),
                    container.is_alive(),
                    container.contained_units().contains(&object_id),
                    station_pin,
                    enclosing,
                )
            })
        else {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                let containment_actor_id = obj.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let obj = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        };

        if !container_alive || !container_has_unit {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                let containment_actor_id = obj.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let obj = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }

        let pin_pos = station_pin.unwrap_or(container_pos);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            let containment_actor_id = obj.id;
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                &mut self.ai_groups,
                &mut self.pathfinding_system,
                containment_actor_id,
                Some(container_id),
                enclosing,
            );
            if !self.set_owned_object_position_with_effects(
                containment_actor_id,
                pin_pos,
                tree_move_frame,
                effects,
            ) {
                return Ok(());
            }
            let Some(obj) = self.objects.get_mut(&containment_actor_id) else {
                return Ok(());
            };
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.set_status_moving(false);
        }
        self.admit_object_drawable_deselection(object_id);
        Ok(())
    }
}
