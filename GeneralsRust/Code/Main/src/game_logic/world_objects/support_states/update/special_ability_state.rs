use super::*;

impl GameLogic {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_special_ability_support_state(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        selection_radius: f32,
        is_alive: bool,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(ability) = self.pending_special_abilities.get(&object_id).copied() else {
            // A live leftover channel is an active C++
            // SpecialAbilityUpdate module (GameLogic.cpp:3677-3718
            // UpdateModules; prep/onExit owned by the module itself,
            // SpecialAbilityUpdate.cpp:1276-1293). The generic
            // no-pending cleanup must not run for it: set_target(None)
            // force-Idles the unit and the new-order gate above then
            // aborts the channel one tick later, killing the whole
            // MissileDefender laser prep before triggerAbilityEffect.
            let has_live_leftover = self.hero_abilities.leftover_channel(object_id).is_some();
            if !has_live_leftover {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.ignored_obstacle_id = None;
                    obj.hacker_disable_channel = None;
                    obj.set_status_using_ability(false);
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            return Ok(());
        };
        if matches!(ability, PendingSpecialAbility::HelixNapalmBomb { .. }) {
            self.update_helix_napalm_bomb_channel_with_effects(object_id, ability, effects)?;
            return Ok(());
        }

        let special_target_id = ability.target_id();

        // HDB is an authored, persistent SpecialAbilityUpdate
        // channel.  Keep it wholly outside the legacy generic
        // special branch: that branch uses a fixed range and used
        // to apply the disable instantly (and reject an already
        // disabled target), none of which matches C++.
        if matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. }) {
            self.update_hacker_disable_building_channel_with_effects(
                object_id,
                special_target_id,
                dt,
                effects,
            )?;
            return Ok(());
        }

        let Some((
            target_position,
            target_radius,
            target_team,
            target_alive,
            target_is_vehicle,
            target_is_structure,
            target_is_airborne,
            target_is_carbomb,
            target_is_hijacked,
            target_is_hacked,
            target_is_unmanned,
        )) = self.objects.get(&special_target_id).map(|target| {
            (
                target.get_position(),
                target.selection_radius,
                target.team,
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Structure),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.status.is_carbomb,
                target.status.hijacked,
                target.status.disabled_hacked,
                target.status.disabled_unmanned,
            )
        })
        else {
            self.release_rejected_special(object_id, effects)?;
            return Ok(());
        };

        let requires_enemy_target = !matches!(
            ability,
            PendingSpecialAbility::CarBomb { .. } | PendingSpecialAbility::DisguiseAsVehicle { .. }
        );
        // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
        // residual: complete without approach walk.
        let disguise_instant = matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
        let black_lotus_range = matches!(
            ability,
            PendingSpecialAbility::StealCashHack { .. }
                | PendingSpecialAbility::DisableVehicleHack { .. }
        );
        let snipe_range = matches!(ability, PendingSpecialAbility::SnipeVehicle { .. });
        let booby_trap_range = matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
        let plant_range = matches!(
            ability,
            PendingSpecialAbility::PlantTimedDemoCharge { .. }
                | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
        );
        let leftover_busy = self.hero_abilities.leftover_channel(object_id).is_some();
        let out_of_start_range = if plant_range {
            !self.leftover_sa_within_start_range(
                object_id,
                special_target_id,
                crate::game_logic::host_hero_abilities::PLANT_START_ABILITY_RANGE,
            )
        } else if black_lotus_range {
            !self.leftover_lotus_within_start_range(
                object_id,
                special_target_id,
                crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE,
            )
        } else if snipe_range {
            crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
                position,
                selection_radius,
                target_position,
                target_radius,
            ) > crate::game_logic::host_jarmen_kell::JARMEN_PILOT_SNIPE_RANGE
        } else if booby_trap_range {
            position.distance(target_position)
                > crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                    + selection_radius
                    + target_radius
        } else {
            position.distance(target_position)
                > selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
        };
        if !leftover_busy && !disguise_instant && can_move && out_of_start_range {
            // C++ SpecialAbilityUpdate::approachTarget ignores the object.
            // Snipe is the Jarmen weapon and Sabotage is a crate collide;
            // neither calls ignoreObstacle before the move.
            let sau_object_approach = matches!(
                ability,
                PendingSpecialAbility::Hijack { .. }
                    | PendingSpecialAbility::PlantTimedDemoCharge { .. }
                    | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    | PendingSpecialAbility::StealCashHack { .. }
                    | PendingSpecialAbility::DisableVehicleHack { .. }
                    | PendingSpecialAbility::PlantBoobyTrap { .. }
            );
            if sau_object_approach {
                self.path_approach_with_state_ignoring(
                    object_id,
                    target_position,
                    AIState::SpecialAbility,
                    Some(special_target_id),
                );
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.ignored_obstacle_id = Some(special_target_id);
                }
            } else {
                self.path_approach_with_state(object_id, target_position, AIState::SpecialAbility);
            }
            return Ok(());
        }

        // Disguise: C++ ActionManager SPECIAL_DISGUISE_AS_VEHICLE.
        // Bomb-truck same-template reject is commented out in retail;
        // boats / trains / aircraft are illegal.
        if matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. }) {
            use crate::game_logic::host_bomb_truck_disguise::is_legal_disguise_target;
            let legal = self
                .objects
                .get(&special_target_id)
                .map(|t| {
                    is_legal_disguise_target(
                        t.is_alive(),
                        t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                        t.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || t.status.airborne_target,
                        t.is_kind_of(crate::game_logic::KindOf::Boat)
                            || t.is_kind_of(crate::game_logic::KindOf::Boat),
                        t.is_kind_of(crate::game_logic::KindOf::CliffJumper),
                        t.has_authored_railroad_behavior(),
                        t.status.disguised,
                    )
                })
                .unwrap_or(false);
            if !legal {
                self.release_rejected_special(object_id, effects)?;
                return Ok(());
            }
        }

        // ConvertToCarBomb: C++ isValidToExecute — boat, IS_CARBOMB,
        // WEAPONSET_CARBOMB template set, already-flagged weaponset.
        if matches!(ability, PendingSpecialAbility::CarBomb { .. }) {
            let reject = self
                .objects
                .get(&special_target_id)
                .map(crate::game_logic::host_car_bomb::carbomb_target_rejected)
                .unwrap_or(true);
            if reject {
                self.release_rejected_special(object_id, effects)?;
                return Ok(());
            }
        }

        // Hijack: C++ isValidToExecute — ImmuneToCapture, boat, drone,
        // already hijacked, occupied KINDOF_TRANSPORT.
        if matches!(ability, PendingSpecialAbility::Hijack { .. }) {
            let reject = self
                .objects
                .get(&special_target_id)
                .map(crate::game_logic::host_car_bomb::hijack_target_rejected)
                .unwrap_or(true);
            if reject {
                self.release_rejected_special(object_id, effects)?;
                return Ok(());
            }
        }

        // Disable vehicle hack: unmanned still matches ActionManager.
        // Already-hacked is legal — C++ triggerAbilityEffect refreshes.
        if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. }) && target_is_unmanned
        {
            self.release_rejected_special(object_id, effects)?;
            return Ok(());
        }

        if matches!(ability, PendingSpecialAbility::Sabotage { .. }) && !target_is_structure {
            self.release_rejected_special(object_id, effects)?;
            return Ok(());
        }

        // Burton timed/remote/Helix: leftover can_place_special_object_charge
        // rejects dead / Bridge / BridgeTower. Tank Hunter TNT is the
        // leftover Structure-or-(Vehicle && !Aircraft) arm — bridges legal.
        if matches!(
            ability,
            PendingSpecialAbility::PlantTimedDemoCharge { .. }
                | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
        ) {
            let is_tank_hunter = self.objects.get(&object_id).is_some_and(|o| {
                crate::game_logic::host_tank_hunter::is_tank_hunter_template(&o.template_name)
            });
            let plant_ok = if is_tank_hunter {
                crate::game_logic::host_hero_abilities::leftover_tank_hunter_tnt_target_ok(
                    target_alive,
                    target_is_structure,
                    target_is_vehicle,
                    target_is_airborne,
                )
            } else {
                crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
                    target_alive,
                    self.objects
                        .get(&special_target_id)
                        .is_some_and(|t| t.is_kind_of(KindOf::Bridge)),
                    self.objects
                        .get(&special_target_id)
                        .is_some_and(|t| t.is_kind_of(KindOf::BridgeTower)),
                    target_is_structure,
                    target_is_vehicle && !target_is_airborne,
                )
            };
            if !plant_ok {
                self.pending_special_abilities.remove(&object_id);
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
            }
        }

        // Black Lotus cash hack: enemy cash-generator structures only.
        if matches!(ability, PendingSpecialAbility::StealCashHack { .. }) {
            let is_cash_gen = self
                .objects
                .get(&special_target_id)
                .map(|t| {
                    crate::game_logic::host_hero_abilities::is_cash_hack_target(
                        &t.template_name,
                        t.is_kind_of(KindOf::SupplyCenter),
                        t.is_kind_of(KindOf::FSSupplyCenter),
                        t.is_kind_of(KindOf::FSBlackMarket),
                        t.is_kind_of(KindOf::FSSupplyDropzone),
                    )
                })
                .unwrap_or(false);
            if !target_is_structure || !is_cash_gen {
                self.pending_special_abilities.remove(&object_id);
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
            }
        }

        // GLA Rebel BoobyTrap: allied/neutral structures only
        // (C++ ActionManager.cpp:1610-1618).
        if matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. }) {
            use gamelogic::common::Relationship;
            let (src_pid, tgt_pid) = match (
                self.objects.get(&object_id),
                self.objects.get(&special_target_id),
            ) {
                (Some(src), Some(tgt)) => (src.owner_player_id, tgt.owner_player_id),
                _ => (None, None),
            };
            let rel = match (src_pid, tgt_pid) {
                (Some(a), Some(b)) => self.player_relationship(a, b),
                _ => Relationship::Neutral,
            };
            if !target_is_structure || !matches!(rel, Relationship::Neutral | Relationship::Allies)
            {
                self.pending_special_abilities.remove(&object_id);
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
            }
        }

        if let Some(kind) = Self::leftover_sa_kind(ability) {
            match self.tick_leftover_special_ability_with_effects(
                object_id,
                special_target_id,
                kind,
                dt,
                effects,
            )? {
                LeftoverSaTick::Waiting | LeftoverSaTick::Finished => return Ok(()),
                LeftoverSaTick::Trigger => {
                    self.leftover_sa_queue_trigger_sound(object_id, kind);
                }
            }
        }

        match ability {
            PendingSpecialAbility::Hijack { .. } => {
                let is_hijacker = self.objects.get(&object_id).is_some_and(|unit| {
                    unit.template_name.to_ascii_lowercase().contains("hijacker")
                });
                if !is_hijacker {
                    self.release_rejected_special(object_id, effects)?;
                    return Ok(());
                }
                let Some(actor_birth) = self
                    .objects
                    .get(&object_id)
                    .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
                else {
                    return Ok(());
                };
                let Some(target_birth) = self
                    .objects
                    .get(&special_target_id)
                    .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
                else {
                    return Ok(());
                };
                // C++ ConvertToHijackedVehicleCrateCollide residual:
                // walk → transfer team + OBJECT_STATUS_HIJACKED;
                // ride-hide (drawable + partition unRegister) or consume.
                // Endow MAX veterancy + hijacker vision/shroud + cancel dozer tasks.
                // C++ order: tryInfiltrationEvent → EVA_VehicleStolen → setTeam.
                self.try_infiltration_event(special_target_id);
                self.try_eva_vehicle_stolen(special_target_id);
                let donor_snap = self.objects.get(&object_id).cloned();
                if let Some(target) = self.objects.get_mut(&special_target_id) {
                    target.apply_hijacked_from_before_veterancy(
                        donor_snap.as_ref(),
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        &mut self.partition_manager,
                    );
                }
                match owner_player_id {
                    Some(player_id) => {
                        let _ = self.transfer_object_to_player_with_effects(
                            special_target_id,
                            player_id,
                            effects,
                        );
                    }
                    None => {
                        if let Some(target) = self.objects.get_mut(&special_target_id) {
                            target.set_team_with_partition_owner(
                                team,
                                &mut self.ownership_events,
                                &mut self.partition_manager,
                            );
                        }
                    }
                }
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                // C++ transferObjectName residual.
                let _ = self.transfer_script_object_name(object_id, special_target_id);
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                // The C++ collision reads both ranks after the team
                // and script-name callbacks, which can change them.
                let Some((actor_level, target_level)) =
                    self.objects.get(&object_id).and_then(|actor| {
                        self.objects
                            .get(&special_target_id)
                            .map(|target| (actor.experience.level, target.experience.level))
                    })
                else {
                    return Ok(());
                };
                let level = if crate::game_logic::host_unit_training::veterancy_rank(actor_level)
                    >= crate::game_logic::host_unit_training::veterancy_rank(target_level)
                {
                    actor_level
                } else {
                    target_level
                };
                self.set_exact_veterancy_level_with_upgrade_callback_quiet(
                    object_id, level, effects,
                )?;
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                self.set_exact_veterancy_level_with_upgrade_callback_quiet(
                    special_target_id,
                    level,
                    effects,
                )?;
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                self.car_bomb.record_hijack();
                self.queue_audio_event(
                    AudioEventRequest::new(crate::game_logic::host_car_bomb::HIJACK_AUDIO)
                        .with_object(special_target_id)
                        .with_position(target_position)
                        .with_priority(170),
                );
                let msg = localization::localize("hud.hijack.complete", "Vehicle hijacked");
                self.queue_radar_message_for_team(team, msg);
                // C++: if target has EjectPilotDie → hide hijacker in vehicle;
                // else destroy hijacker immediately.
                // Wave 753: ride-hide only when the hijacker is infantry
                // (HijackerUpdate module). Non-infantry steal path destroys
                // the attacker immediately (test/tank harness + C++ shape).
                // C++: if target has EjectPilotDie and hijacker is infantry
                // (HijackerUpdate) → hide in vehicle; else consume attacker.
                // Wave 753: ride-hide only for infantry; non-infantry steal
                // destroys immediately. SlowDeath must not clear destroyed —
                // hijacker consume is same-frame (begin_slow_death clears the
                // destroyed flag for delayed peels).
                let hijacker_is_infantry = self
                    .objects
                    .get(&object_id)
                    .map(|h| {
                        h.is_kind_of(KindOf::Infantry) || h.object_type == ObjectType::Infantry
                    })
                    .unwrap_or(false);
                if hijacker_is_infantry && self.vehicle_supports_hijacker_ride(special_target_id) {
                    self.begin_owned_hijacker_ride(object_id, special_target_id);
                    self.admit_object_drawable_deselection(object_id);
                } else {
                    self.mark_destroyed_authority_aware(object_id, None);
                    // Suppress SlowDeath/jet/heli peels so consume sticks.
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        o.slow_death = None;
                        o.jet_slow_death = None;
                        o.helicopter_slow_death = None;
                        o.status.effectively_dead = true;
                        o.status.destroyed = true;
                    }
                    self.mark_object_for_destruction_with_effects(object_id, Some(team), effects)?;
                    // mark_object may re-enter SlowDeath and clear destroyed;
                    // re-assert consume residual for hijack steal.
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        o.slow_death = None;
                        o.jet_slow_death = None;
                        o.helicopter_slow_death = None;
                        o.status.effectively_dead = true;
                        o.status.destroyed = true;
                        if o.health.current > 0.0 {
                            o.internal_change_health(-o.health.current);
                        }
                    }
                }
            }
            PendingSpecialAbility::Sabotage { .. } => {
                // C++ Sabotage*CrateCollide residual: type-specific structure
                // sabotage; saboteur consumed on success (mobile crate).
                use crate::game_logic::host_saboteur::{
                    SABOTEUR_CASH_STEAL_AUDIO, SABOTEUR_RESET_TIMER_AUDIO,
                    SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO, SaboteurEffectKind,
                    classify_sabotage_target, is_saboteur_template,
                };
                let saboteur_ok = self
                    .objects
                    .get(&object_id)
                    .map(|o| is_saboteur_template(&o.template_name))
                    .unwrap_or(false);
                let effect = self.objects.get(&special_target_id).and_then(|t| {
                    classify_sabotage_target(
                        &t.template_name,
                        t.is_kind_of(KindOf::FSPower),
                        t.is_kind_of(KindOf::PowerPlant),
                        t.is_kind_of(KindOf::FSSupplyCenter),
                        t.is_kind_of(KindOf::SupplyCenter),
                        t.is_kind_of(KindOf::FSBarracks),
                        t.is_kind_of(KindOf::FSWarFactory),
                        t.is_kind_of(KindOf::FSAirfield),
                        t.is_kind_of(KindOf::FSSuperweapon),
                        t.is_kind_of(KindOf::FSStrategyCenter),
                        t.is_kind_of(KindOf::CommandCenter),
                        t.is_kind_of(KindOf::FSInternetCenter),
                        t.is_kind_of(KindOf::FSFake),
                        t.is_kind_of(KindOf::FSSupplyDropzone),
                        crate::game_logic::host_saboteur::is_aircraft_carrier_template(
                            &t.template_name,
                        ),
                    )
                });
                if saboteur_ok {
                    if let Some(kind) = effect {
                        let mut cash_stolen = 0u32;
                        match kind {
                            SaboteurEffectKind::PowerPlant => {
                                let until = self.frame.saturating_add(
                                                crate::game_logic::host_saboteur::SABOTEUR_POWER_DURATION_FRAMES,
                                            );
                                // C++ SabotagePowerPlantCrateCollide.cpp:112-120
                                // other->getControllingPlayer(), never first same-faction slot.
                                let victim_id = self
                                    .objects
                                    .get(&special_target_id)
                                    .and_then(|target| self.player_owner_for_host_object(target));
                                if let Some(player) =
                                    victim_id.and_then(|id| self.players.get_mut(&id))
                                {
                                    player.power_sabotaged_till_frame = until;
                                }
                            }
                            SaboteurEffectKind::SupplyCenter
                            | SaboteurEffectKind::SupplyDropzone => {
                                if matches!(kind, SaboteurEffectKind::SupplyDropzone) {
                                    // C++ OCLUpdate::resetTimer
                                    // (SabotageSupplyDropzoneCrateCollide.cpp:112-117).
                                    self.supply_drop_zones
                                        .reset_timer(special_target_id, self.frame);
                                }
                                // C++ other/obj getControllingPlayer() money,
                                // never first same-faction slot.
                                let from_player_id = self
                                    .objects
                                    .get(&special_target_id)
                                    .and_then(|target| self.player_owner_for_host_object(target));
                                let to_player_id = self
                                    .objects
                                    .get(&object_id)
                                    .and_then(|caster| self.player_owner_for_host_object(caster));
                                cash_stolen = match (from_player_id, to_player_id) {
                                    (Some(from), Some(to)) => self.steal_cash_between_players(
                                        from,
                                        to,
                                        SABOTEUR_STEAL_CASH_AMOUNT,
                                    ),
                                    _ => 0,
                                };
                            }
                            SaboteurEffectKind::MilitaryFactory => {
                                if let Some(until) = kind.disabled_hacked_until(self.frame) {
                                    self.apply_owned_hacked_disabled(special_target_id, until);
                                }
                            }
                            SaboteurEffectKind::InternetCenter => {
                                // C++ SabotageInternetCenterCrateCollide residual:
                                // 1) disable SpyVisionUpdate on ALL team internet centers
                                // 2) DISABLED_HACKED on the sabotaged center
                                // 3) DISABLED_HACKED on contained hackers
                                let until = kind
                                                .disabled_hacked_until(self.frame)
                                                .unwrap_or_else(|| {
                                                    self.frame.saturating_add(
                                                        crate::game_logic::host_saboteur::SABOTEUR_INTERNET_DURATION_FRAMES,
                                                    )
                                                });
                                let (centers, hackers) = self
                                    .apply_internet_center_sabotage_residual(
                                        special_target_id,
                                        target_team,
                                        until,
                                    );
                                self.saboteur
                                    .record_internet_spy_vision_disable(centers, hackers);
                            }
                            SaboteurEffectKind::SuperweaponOrCommand => {
                                // C++ SabotageSuperweaponCrateCollide: reset ALL
                                // SpecialPowerModule interfaces via startPowerRecharge.
                                // Host residual: object-level special power + strike
                                // registry timers for this structure.
                                let reset_ok =
                                    self.apply_superweapon_sabotage_recharge(special_target_id);
                                if reset_ok {
                                    self.saboteur.record_superweapon_power_reset();
                                }
                            }
                            SaboteurEffectKind::FakeBuilding => {
                                // C++ SabotageFakeBuildingCrateCollide:
                                // DAMAGE_UNRESISTABLE / DEATH_DETONATED for max health.
                                let max_hp = self.objects.get(&special_target_id).map(|target| {
                                    target.health.maximum.max(target.max_health).max(1.0)
                                });
                                let destroyed = max_hp.is_some_and(|max_hp| {
                                    self.apply_residual_damage_owned_with_effects(
                                        special_target_id,
                                        max_hp,
                                        Some(object_id),
                                        crate::game_logic::combat::DamageType::Unresistable,
                                        crate::game_logic::host_usa_pilot::HostDeathType::Detonated,
                                        None,
                                        effects,
                                    )
                                });
                                if destroyed {
                                    self.mark_object_for_destruction_with_effects(
                                        special_target_id,
                                        Some(team),
                                        effects,
                                    )?;
                                    self.saboteur.record_fake_detonated();
                                }
                            }
                        }
                        self.admit_host_radar_disabled_edges(special_target_id);
                        self.saboteur.record(kind, cash_stolen);
                        // C++ TheRadar->tryInfiltrationEvent(other) residual
                        // (victim local player warning).
                        self.try_infiltration_event(special_target_id);
                        // C++ TheEva->setShouldPlay residual when victim local.
                        // Supply center: CashStolen if cash taken, else BuildingSabotaged.
                        if kind.steals_cash() && cash_stolen > 0 {
                            // C++ controller ScoreKeeper::addMoneyEarned residual.
                            if let Some(p) = self
                                .objects
                                .get(&object_id)
                                .and_then(|caster| self.player_owner_for_host_object(caster))
                                .and_then(|id| self.get_player_mut(id))
                            {
                                p.add_money_earned(cash_stolen);
                            }
                            self.try_eva_cash_stolen(special_target_id);
                            // C++ GUI:AddCash / GUI:LoseCash floating text residual.
                            self.spawn_sabotage_cash_floating_texts(
                                object_id,
                                special_target_id,
                                cash_stolen,
                            );
                        } else {
                            self.try_eva_building_sabotaged(special_target_id);
                        }
                        // C++ doSabotageFeedbackFX residual (type audio + flash).
                        self.do_sabotage_feedback_fx(special_target_id, kind);
                        let msg =
                            localization::localize("hud.saboteur.complete", "Building sabotaged");
                        self.queue_radar_message_for_team(team, msg);
                        // C++ CrateCollide: destroy saboteur (mobile crate).
                        self.mark_destroyed_authority_aware(object_id, None);
                        self.mark_object_for_destruction_with_effects(
                            object_id,
                            Some(team),
                            effects,
                        )?;
                        self.saboteur.record_consumed();
                    } else if let Some(obj) = self.objects.get_mut(&object_id) {
                        // Fail-closed: non-matching structure — cancel residual.
                        obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                            &mut self.queued_audio_events,
                        ));
                        obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                            &mut self.queued_audio_events,
                        ));
                    }
                } else if let Some(obj) = self.objects.get_mut(&object_id) {
                    // Fail-closed: non-saboteur cannot complete residual.
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::SnipeVehicle { .. } => {
                // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle becomes
                // unmanned + Neutral so it can be recrewed/captured.
                // C++ car-bomb dead-man: IS_CARBOMB detonates instead.
                let is_bomb = self
                    .objects
                    .get(&special_target_id)
                    .map(|t| t.is_car_bomb())
                    .unwrap_or(false);
                if is_bomb {
                    let _ = self.maybe_detonate_carbomb_on_unmanned_with_effects(
                        special_target_id,
                        effects,
                    );
                } else if let Some(target) = self.objects.get_mut(&special_target_id) {
                    target.apply_kill_pilot_unmanned_at_frame(
                        &mut self.health_events,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                    );
                    target.set_team_with_partition_owner(
                        Team::Neutral,
                        &mut self.ownership_events,
                        &mut self.partition_manager,
                    );
                    let sniped_vehicle = target.is_kind_of(crate::game_logic::KindOf::Vehicle);
                    drop(target);
                    if sniped_vehicle {
                        Self::record_neutral_vehicle_sniped(&mut self.players);
                    }
                    self.admit_local_drawable_deselected(special_target_id);
                    for player in self.players.values_mut() {
                        player
                            .selected_objects
                            .retain(|id| *id != special_target_id);
                    }
                }
                self.hero_abilities.record_snipe();
                self.queue_audio_event(
                    AudioEventRequest::new(
                        crate::game_logic::host_hero_abilities::SNIPE_VEHICLE_AUDIO,
                    )
                    .with_object(special_target_id)
                    .with_position(target_position)
                    .with_priority(170),
                );
                let msg = localization::localize("hud.snipe.vehicle_unmanned", "Vehicle unmanned");
                self.queue_radar_message_for_team(team, msg);
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::PlantTimedDemoCharge { .. } => {
                // C++ triggerAbilityEffect checkAndDetonateBoobyTrap before plant.
                if self.leftover_probe_booby_at_target_with_effects(
                    object_id,
                    special_target_id,
                    team,
                    effects,
                ) {
                    self.hero_abilities.take_leftover_channel(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return Ok(());
                }
                // Burton / Tank Hunter TNT residual: plant sticky timed charge at target.
                let is_tank_hunter = self
                    .objects
                    .get(&object_id)
                    .map(|o| {
                        crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                            &o.template_name,
                        )
                    })
                    .unwrap_or(false);
                // Tank Hunter TNT reload residual (7500ms / 225 frames).
                let tnt_ready = if is_tank_hunter {
                    crate::game_logic::host_tank_hunter::tnt_ready(
                        self.frame,
                        self.tank_hunter_tnt_last_frame.get(&object_id).copied(),
                    )
                } else {
                    true
                };
                let charge_id = if tnt_ready {
                    self.place_timed_demo_charge_with_effects(
                        team,
                        target_position,
                        Some(object_id),
                        Some(special_target_id),
                        None,
                        effects,
                    )
                } else {
                    None
                };
                if charge_id.is_some() {
                    self.hero_abilities.record_timed_charge_plant();
                    if is_tank_hunter {
                        self.tank_hunter_residual_tnt_plants =
                            self.tank_hunter_residual_tnt_plants.saturating_add(1);
                        self.tank_hunter_tnt_last_frame
                            .insert(object_id, self.frame);
                        self.queue_audio_event(
                            AudioEventRequest::new(
                                crate::game_logic::host_tank_hunter::TNT_INITIATE_AUDIO,
                            )
                            .with_object(object_id)
                            .with_position(target_position)
                            .with_priority(160),
                        );
                    }
                    let msg =
                        localization::localize("hud.demo_charge.planted", "Demo charge planted");
                    self.queue_radar_message_for_team(team, msg);
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                if self.leftover_probe_booby_at_target_with_effects(
                    object_id,
                    special_target_id,
                    team,
                    effects,
                ) {
                    self.hero_abilities.take_leftover_channel(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return Ok(());
                }
                // Burton residual: plant sticky remote charge (no auto-timer).
                let charge_id = self.place_remote_demo_charge_with_effects(
                    team,
                    target_position,
                    Some(object_id),
                    Some(special_target_id),
                    effects,
                );
                if charge_id.is_some() {
                    self.hero_abilities.record_remote_charge_plant();
                    let msg = localization::localize(
                        "hud.remote_demo_charge.planted",
                        "Remote demo charge planted",
                    );
                    self.queue_radar_message_for_team(team, msg);
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::StealCashHack { .. } => {
                self.leftover_kill_special_objects_with_effects(object_id, effects);
                // C++ `triggerAbilityEffect` AwardXPForTriggering
                // (`SpecialAbilityUpdate.cpp:1248-1253`). Retail
                // `SpecialAbilityBlackLotusStealCashHack` = 20.
                self.award_ability_trigger_experience(
                    object_id,
                    crate::game_logic::host_hero_abilities::LOTUS_STEAL_AWARD_XP as i32,
                );
                // Black Lotus residual: steal cash from enemy economy.
                // C++ SPECIAL_BLACKLOTUS_STEAL_CASH_HACK:
                // target->getControllingPlayer() / object->getControllingPlayer()
                // withdraw/deposit, scorekeeper money earned, EVA_CashStolen
                // when victim local, GUI:AddCash/LoseCash floating texts.
                // Never debit/credit the first same-faction player.
                let amount = crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT;
                let from_player_id = self
                    .objects
                    .get(&special_target_id)
                    .and_then(|target| self.player_owner_for_host_object(target));
                let to_player_id = self
                    .objects
                    .get(&object_id)
                    .and_then(|caster| self.player_owner_for_host_object(caster));
                let stolen = match (from_player_id, to_player_id) {
                    (Some(from), Some(to)) => self.steal_cash_between_players(from, to, amount),
                    _ => 0,
                };
                if stolen > 0 {
                    self.hero_abilities.record_cash_steal(stolen);
                    // C++ controller->getScoreKeeper()->addMoneyEarned(cash)
                    if let Some(p) = to_player_id.and_then(|id| self.get_player_mut(id)) {
                        p.add_money_earned(stolen);
                    }
                    self.try_eva_cash_stolen(special_target_id);
                    self.spawn_sabotage_cash_floating_texts(object_id, special_target_id, stolen);
                    let msg = localization::localize("hud.cash_hack.complete", "Cash stolen");

                    self.queue_radar_message_for_team(team, msg);
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::CarBomb { .. } => {
                let is_terrorist = self.objects.get(&object_id).is_some_and(|unit| {
                    crate::game_logic::is_terrorist_template(&unit.template_name)
                });
                if !is_terrorist {
                    self.pending_special_abilities.remove(&object_id);
                    return Ok(());
                }
                let Some(actor_birth) = self
                    .objects
                    .get(&object_id)
                    .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
                else {
                    return Ok(());
                };
                let Some(target_birth) = self
                    .objects
                    .get(&special_target_id)
                    .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
                else {
                    return Ok(());
                };
                // C++ ConvertToCarBombCrateCollide residual:
                // vehicle defects to converter team, gains IS_CARBOMB +
                // SuicideCarBomb weapon residual. Converter is consumed.
                // Detonation happens later when the car bomb attacks.
                // Booby-trap residual: cancel if mine detonates and either dies.
                let booby = self
                    .objects
                    .get(&special_target_id)
                    .map(|t| t.status.booby_trapped)
                    .unwrap_or(false);
                if booby {
                    // Detonate trap residual damage on both.
                    if let Some(t) = self.objects.get_mut(&special_target_id) {
                        let receiving_damage_target_id = t.id;
                        let receiving_damage_argument_1 = t.health.maximum.max(1.0);
                        let _ = self.apply_residual_damage_owned_with_effects(
                            receiving_damage_target_id,
                            receiving_damage_argument_1,
                            Some(object_id),
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                crate::game_logic::combat::DamageType::Unresistable,
                            ),
                            None,
                            effects,
                        );
                    }
                    if let Some(b) = self.objects.get_mut(&object_id) {
                        let receiving_damage_target_id = b.id;
                        let receiving_damage_argument_1 = b.health.maximum.max(1.0);
                        let _ = self.apply_residual_damage_owned_with_effects(
                            receiving_damage_target_id,
                            receiving_damage_argument_1,
                            Some(special_target_id),
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                crate::game_logic::combat::DamageType::Unresistable,
                            ),
                            None,
                            effects,
                        );
                    }
                    let t_dead = self
                        .objects
                        .get(&special_target_id)
                        .map(|t| !t.is_alive() || t.status.destroyed)
                        .unwrap_or(true);
                    let b_dead = self
                        .objects
                        .get(&object_id)
                        .map(|b| !b.is_alive() || b.status.destroyed)
                        .unwrap_or(true);
                    if t_dead || b_dead {
                        if t_dead {
                            self.mark_object_for_destruction_with_effects(
                                special_target_id,
                                Some(team),
                                effects,
                            )?;
                        }
                        if b_dead {
                            self.mark_object_for_destruction_with_effects(
                                object_id,
                                Some(team),
                                effects,
                            )?;
                        }
                        return Ok(());
                    }
                }
                // Snapshot donor residual (vision/vet) before consume.
                let donor_snap = self.objects.get(&object_id).cloned();
                let frame = self.frame;
                self.with_object_veterancy_inputs(
                    special_target_id,
                    |target, audio, feedback, partition, _locomotors| {
                        target.apply_convert_to_car_bomb_from_before_veterancy(
                            donor_snap.as_ref(),
                            audio,
                            feedback,
                            partition,
                            frame,
                        );
                    },
                );
                match owner_player_id {
                    Some(player_id) => {
                        let _ = self.transfer_object_to_player_with_effects(
                            special_target_id,
                            player_id,
                            effects,
                        );
                    }
                    None => {
                        if let Some(target) = self.objects.get_mut(&special_target_id) {
                            target.set_team_with_partition_owner(
                                team,
                                &mut self.ownership_events,
                                &mut self.partition_manager,
                            );
                        }
                    }
                }
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                // C++ transferObjectName residual (script named object).
                let _ = self.transfer_script_object_name(object_id, special_target_id);
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                let Some(level) = self
                    .objects
                    .get(&object_id)
                    .map(|actor| actor.experience.level)
                else {
                    return Ok(());
                };
                self.set_exact_veterancy_level_with_upgrade_callback(
                    special_target_id,
                    level,
                    effects,
                )?;
                if !same_live_object_birth(self, object_id, &actor_birth)
                    || !same_live_object_birth(self, special_target_id, &target_birth)
                {
                    return Ok(());
                }
                self.car_bomb.record_conversion();
                // C++ FXList::doFXObj(m_fxList, other) → FX_MakeCarBombSuccess
                // (Sound nugget TerroristCarBomb). Never play the FXList name.
                let fx = crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_LIST;
                if !self.dispatch_fx_list_at_host_object_with_effects(
                    fx,
                    special_target_id,
                    Some(object_id),
                    effects,
                ) {
                    #[cfg(feature = "game_client")]
                    let sounds = self.fx_list_sound_names(fx);
                    #[cfg(not(feature = "game_client"))]
                    let sounds = crate::game_logic::sound_names_for_fx_list(fx);
                    if sounds.is_empty() {
                        self.queue_audio_event(
                            AudioEventRequest::new(
                                crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_SOUND,
                            )
                            .with_object(special_target_id)
                            .with_position(target_position)
                            .with_priority(170),
                        );
                    } else {
                        for sound in sounds {
                            self.queue_audio_event(
                                AudioEventRequest::new(&sound)
                                    .with_object(special_target_id)
                                    .with_position(target_position)
                                    .with_priority(170),
                            );
                        }
                    }
                }
                let msg = localization::localize(
                    "hud.carbomb.converted",
                    "Vehicle converted to car bomb",
                );
                self.queue_radar_message_for_team(team, msg);
                self.mark_destroyed_authority_aware(object_id, None);
                self.mark_object_for_destruction_with_effects(object_id, Some(team), effects)?;
            }
            PendingSpecialAbility::DisableVehicleHack { .. } => {
                // C++ SpecialAbilityUpdate BLACKLOTUS_DISABLE_VEHICLE_HACK:
                // setDisabledUntil(DISABLED_HACKED, now + EffectDuration).
                let until = self.frame.saturating_add(
                    crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                );
                self.apply_owned_hacked_disabled(special_target_id, until);
                let do_fx = self
                    .hero_abilities
                    .leftover_channel(object_id)
                    .map(|ch| ch.do_disable_fx_particles)
                    .unwrap_or(true);
                let new_do_fx = self.leftover_spawn_disable_fx(
                    object_id,
                    special_target_id,
                    crate::game_logic::host_hero_abilities::LOTUS_DISABLE_FX_PARTICLE,
                    crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                    do_fx,
                    effects,
                );
                if let Some(mut ch) = self.hero_abilities.leftover_channel(object_id).copied() {
                    ch.do_disable_fx_particles = new_do_fx;
                    self.hero_abilities.set_leftover_channel(object_id, ch);
                }
                self.leftover_kill_special_objects_with_effects(object_id, effects);
                self.hero_abilities.record_vehicle_disable();
                let msg = localization::localize("hud.vehicle_hack.disabled", "Vehicle disabled");
                self.queue_radar_message_for_team(team, msg);
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.ignored_obstacle_id = None;
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            PendingSpecialAbility::HackerDisableBuilding { .. } => {
                unreachable!("HDB is intercepted by its typed persistent channel")
            }
            PendingSpecialAbility::HelixNapalmBomb { .. } => {
                unreachable!("Helix NapalmBomb is intercepted by its typed approach channel")
            }
            PendingSpecialAbility::DisguiseAsVehicle { .. } => 'disguise: {
                // C++ StealthUpdate::disguiseAsObject residual:
                // if target already disguised, copy *its* disguise
                // template + player; else copy target template + team.
                // set OBJECT_STATUS_DISGUISED + STEALTHED.
                let Some((tpl, as_team, as_player, copied_disguise)) =
                    self.objects.get(&special_target_id).map(|t| {
                        if t.status.disguised {
                            if let (Some(dt), Some(dteam)) =
                                (t.disguise_as_template.as_ref(), t.disguise_as_team)
                            {
                                return (dt.clone(), dteam, t.disguise_as_player_id, true);
                            }
                        }
                        (t.template_name.clone(), t.team, t.owner_player_id, false)
                    })
                else {
                    break 'disguise;
                };
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.apply_disguise(&tpl, as_team, as_player);
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.ignored_obstacle_id = None;
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
                self.bomb_truck_disguise.record_disguise(object_id, &tpl);
                self.bomb_truck_disguise.record_transition_start();
                if copied_disguise {
                    self.bomb_truck_disguise.record_disguise_copy();
                }
                self.queue_audio_event(
                    AudioEventRequest::new(
                        crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_AUDIO,
                    )
                    .with_object(object_id)
                    .with_position(position)
                    .with_priority(160),
                );
                let msg = localization::localize("hud.bombtruck.disguised", "Bomb truck disguised");
                self.queue_radar_message_for_team(team, msg);
            }
            PendingSpecialAbility::PlantBoobyTrap { .. } => {
                // C++ triggerAbilityEffect SPECIAL_BOOBY_TRAP:
                // checkAndDetonateBoobyTrap first; cancel if either dies;
                // refuse a second plant while BOOBY_TRAPPED remains.
                if self.leftover_probe_booby_at_target_with_effects(
                    object_id,
                    special_target_id,
                    team,
                    effects,
                ) {
                    self.hero_abilities.take_leftover_channel(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return Ok(());
                }
                let still_trapped = self
                    .objects
                    .get(&special_target_id)
                    .is_some_and(|target| target.status.booby_trapped);
                if still_trapped {
                    self.pending_special_abilities.remove(&object_id);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                            &mut self.queued_audio_events,
                        ));
                        obj.ignored_obstacle_id = None;
                        obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                            &mut self.queued_audio_events,
                        ));
                    }
                    return Ok(());
                }
                let (can_plant, ready) = self
                    .objects
                    .get(&object_id)
                    .map(|_| {
                        let planter_ok = self.object_may_plant_booby_trap(object_id);
                        let ready = self.booby_trap.plant_ready(object_id, self.frame);
                        (planter_ok, ready)
                    })
                    .unwrap_or((false, false));
                if can_plant && ready && self.booby_trap.can_place_special_object(object_id) {
                    let geom = self
                        .objects
                        .get(&special_target_id)
                        .map(|t| t.selection_radius.max(8.0))
                        .unwrap_or(8.0);
                    let _ = self.booby_trap.install(
                        special_target_id,
                        object_id,
                        team,
                        self.frame,
                        geom,
                        None,
                    );
                    if let Some(cid) = self.spawn_booby_trap_special_object_with_effects(
                        object_id,
                        team,
                        special_target_id,
                        crate::game_logic::game_logic::StickyBombInitPosition::Target,
                        effects,
                    ) {
                        self.booby_trap.set_charge_object(special_target_id, cid);
                    } else {
                        // C++ StickyBombUpdate sets the object bit
                        // only after the charge object exists.
                        let _ = self.booby_trap.take_plant(special_target_id);
                    }
                    let msg =
                        localization::localize("hud.booby_trap.planted", "Booby trap planted");
                    self.queue_radar_message_for_team(team, msg);
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
        }

        if let Some(kind) = Self::leftover_sa_kind(ability) {
            let (timings, variation) = self.leftover_timings_for(object_id, kind);
            if timings.flee_range > 0.0 {
                self.leftover_flee_after_plant(
                    object_id,
                    special_target_id,
                    team,
                    timings.flee_range,
                    timings.flip_after_unpack,
                );
            } else {
                let pack_ms = crate::game_logic::thing::vary_pack_unpack_duration_ms(
                    timings.pack_ms,
                    variation,
                    &self.logic_random,
                );
                self.leftover_begin_packing_with_effects(
                    object_id,
                    special_target_id,
                    kind,
                    pack_ms,
                    effects,
                )?;
            }
        } else {
            self.pending_special_abilities.remove(&object_id);
        }
        Ok(())
    }
}
