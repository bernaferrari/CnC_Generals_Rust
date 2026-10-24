//! Host objects `impl GameLogic` — `crates_radar_power`.
//! crates, parachute, radar, power. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Advance pending DeliverPayload cargo missions with DropDelay stagger.
    ///
    /// Spawns one payload item per due frame (DoorDelay before first item, then
    /// DropDelay between items). Registers residual MoneyCrateCollide entries and
    /// AmericaCrateParachute fall-physics residual (elevated spawn → OpenDist open
    /// → sink). BuildingPickup residual cash is applied on mission complete
    /// (zone bulk residual) and/or via [`Self::update_money_crate_collides`].
    ///
    /// Fail-closed: not full cargo-plane Object flight / full container Object.
    pub fn update_deliver_payloads(&mut self) {
        self.update_deliver_payloads_with_effects(&mut Default::default())
    }

    pub(crate) fn update_deliver_payloads_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_deliver_payload::SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE;
        use crate::game_logic::host_supply_drop_zone::{
            SUPPLY_DROP_ZONE_DROP_CASH, drop_cash_amount,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        self.host_deliver_payloads.clear_frame_events();
        // CreateAtEdge cargo-plane flight residual presentation (approach band /
        // door open). Fail-closed: not full aircraft Object / locomotor.
        self.host_deliver_payloads.tick_cargo_flights();
        // Sync OCLSpecialPower transport objects to cargo-flight residual positions.
        let flight_sync: Vec<(ObjectId, Vec3, f32)> = self
            .host_deliver_payloads
            .missions_snapshot()
            .into_iter()
            .filter_map(|m| {
                let tid = m.transport_object_id?;
                let flight = self.host_deliver_payloads.cargo_flight(m.id)?;
                let yaw = flight.dir_z.atan2(flight.dir_x);
                Some((tid, flight.current_pos, yaw))
            })
            .collect();
        for (tid, pos, yaw) in flight_sync {
            if let Some(o) = self.objects.get_mut(&tid) {
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                o.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
            }
        }
        // AmericaParadrop cargo bookkeeping is completed from update_paradrops
        // (infantry spawn ownership). Only spawn-capable kinds resolve here.
        let item_plans: Vec<_> = self
            .host_deliver_payloads
            .plan_due_item_spawns(self.frame)
            .into_iter()
            .filter(|p| p.kind.spawns_payload_objects())
            .collect();

        for plan in item_plans {
            if !self.templates.contains_key(&plan.payload_template) {
                self.ensure_residual_supply_drop_crate_template();
            }
            let template_name = if self.templates.contains_key(&plan.payload_template) {
                plan.payload_template.clone()
            } else {
                SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string()
            };

            // A deferred cargo mission must retain the player that initiated it:
            // using `source_team` here used to attach every USA payload to the
            // first USA player.  Genuinely unowned missions keep the legacy
            // team-only constructor and stay unowned when that team is ambiguous.
            let spawned_id = match plan.source_owner_player_id {
                Some(player_id) => self.create_object_for_player_with_effects(
                    &template_name,
                    player_id,
                    plan.spawn_position,
                    None,
                    effects,
                ),
                None => self.create_object_with_script_effects(
                    &template_name,
                    plan.source_team,
                    plan.spawn_position,
                    None,
                    effects,
                ),
            };
            if let Some(id) = spawned_id {
                if plan.kind
                    == crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::SuperweaponOclBomb
                {
                    // OCL bomb/missile residual: course-home to target; no crate parachute.
                    if let Some(obj) = self.objects.get_mut(&id) {
                        if let Some(m) = self.host_deliver_payloads.get(plan.mission_id) {
                            let _ = obj.set_smart_bomb_target(m.target_position);
                            if let Some(tid) = m.transport_object_id {
                                obj.producer_id = Some(tid);
                            }
                        }
                        // C++ CreateObjectDie + HeightDie on fuel-air / daisy payloads.
                        obj.ensure_create_object_die();
                        obj.ensure_height_die(self.frame);
                    }
                    self.ocl_special_power_reg.record_payload_spawn();
                } else {
                // Residual MoneyCrateCollide registration.
                if plan.kind
                    == crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::SuperweaponCrateDrop
                {
                    self.host_money_crates.register(
                        id,
                        crate::game_logic::host_money_crate::SUPERWEAPON_CRATE_DROP_MONEY,
                        false,
                        0,
                    );
                } else {
                    self.host_money_crates.register_supply_drop_crate(id);
                }
                if let Some(name) = self.objects.get(&id).map(|o| o.template_name.clone()) {
                    self.host_money_crates.apply_crate_collide_gates(id, &name);
                }
                self.host_money_crates.arm_default_deletion(
                    id,
                    self.frame,
                    id.0.wrapping_add(self.frame),
                );
                // AmericaCrateParachute residual: freefall → OpenDist → open → land.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_crate_parachuting();
                }
                // C++ DeliverPayloadAIUpdate: m_isParachuteDirectly →
                // contain->setOverrideDestination(ai->getTargetPos()).
                if crate::game_logic::host_deliver_payload::SUPPLY_DROP_PARACHUTE_DIRECTLY {
                    if self.set_parachute_override_destination(id, plan.target_position) {
                        self.host_deliver_payloads
                            .record_parachute_directly_override();
                    }
                }
                } // else crate path
            }
            self.host_deliver_payloads
                .record_item_spawned(plan.mission_id, spawned_id);

            // Drop audio / particle on first item of the mission.
            if plan.item_index == 0 {
                self.queue_audio_event(
                    AudioEventRequest::new(plan.kind.drop_audio())
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(130),
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    plan.target_position,
                    self.frame,
                    Some(plan.source_object),
                    None,
                );
            }

            // BuildingPickup residual bulk cash when final item lands
            // (zone path; crates remain for unit MoneyCrateCollide residual
            // only if not marked paid — mark paid after bulk to avoid double).
            if plan.is_final_item && plan.kind.credits_building_pickup_cash() {
                let owner_player_id =
                    self.player_owner_for_event(plan.source_owner_player_id, plan.source_team);
                let has_supply_lines = owner_player_id
                    .and_then(|player_id| self.players.get(&player_id))
                    .is_some_and(|player| {
                        player.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
                    });
                let amount = drop_cash_amount(has_supply_lines);
                let boost = amount.saturating_sub(SUPPLY_DROP_ZONE_DROP_CASH);
                self.supply_drop_zones.record_payload_cash(amount, boost);
                if let Some(player_id) = owner_player_id {
                    if let Some((player, economy_events)) =
                        self.player_and_economy_events_mut(player_id)
                    {
                        player.credit_supplies(amount, economy_events);
                    }
                }
                if boost > 0 {
                    self.supply_lines_bonus_cash_total =
                        self.supply_lines_bonus_cash_total.saturating_add(boost);
                }
                self.host_deliver_payloads
                    .record_cash_credited(plan.mission_id, amount);

                // Prevent unit MoneyCrateCollide double-credit after zone bulk cash.
                if let Some(mission) = self.host_deliver_payloads.get(plan.mission_id) {
                    let ids: Vec<ObjectId> = mission.spawned_payload_ids.clone();
                    self.host_money_crates
                        .mark_building_pickup_residual_paid(&ids);
                }

                log::info!(
                    "Host DeliverPayload {} mission {} complete at {:?} (items={}, cash={})",
                    plan.kind.label(),
                    plan.mission_id,
                    plan.target_position,
                    plan.item_index + 1,
                    amount
                );
            } else {
                log::debug!(
                    "Host DeliverPayload {} mission {} item {} spawned at {:?}",
                    plan.kind.label(),
                    plan.mission_id,
                    plan.item_index,
                    plan.spawn_position
                );
            }
        }
    }

    /// Residual MoneyCrateCollide: unit + BuildingPickup cash collect.
    ///
    /// Supply Drop Zone cargo crates that already received bulk BuildingPickup
    /// residual cash are marked paid (no double-credit). Standalone residual
    /// crates (tests / future map crates) credit MoneyProvided on proximity.
    ///
    /// Residual gates (C++ CrateCollide::isValidToExecute subset):
    /// - RequiredKindOf / ForbiddenKindOf via isKindOfMulti
    /// - AIUpdateInterface required unless BuildingPickup (mines cannot scoop)
    /// - KINDOF_PARACHUTE pickers rejected
    /// - Above-terrain crates block unit path (BuildingPickup still allowed)
    ///
    /// Fail-closed: not full CollideModule partition pairs / Anim2D GPU / EVA text.

    /// C++ DeletionUpdate::update residual for money/salvage crates.
    ///
    /// Destroys (NOT kills) crates whose dieFrame has elapsed.
    pub fn update_crate_deletion_updates(&mut self) {
        self.update_crate_deletion_updates_with_effects(&mut Default::default())
    }

    pub(crate) fn update_crate_deletion_updates_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let expired = self.host_money_crates.expired_ids(self.frame);
        for id in expired {
            self.host_money_crates.forget(id);
            // Destroy object if still present (C++ destroyObject, not kill).
            if self.objects.contains_key(&id) {
                self.mark_object_for_destruction_with_effects(id, None, effects);
            }
        }
    }
    pub fn update_money_crate_collides(&mut self) {
        self.update_money_crate_collides_with_effects(&mut Default::default())
    }

    pub(crate) fn update_money_crate_collides_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_deliver_payload::crate_is_above_terrain;
        use crate::game_logic::host_money_crate::{
            HostMoneyCrateRegistry, MONEY_CRATE_BUILDING_PICKUP_RADIUS, MONEY_CRATE_PICKUP_AUDIO,
            MONEY_CRATE_UNIT_PICKUP_RADIUS,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        let crate_ids = self.host_money_crates.ids();
        if crate_ids.is_empty() {
            return;
        }

        // Snapshot crate positions + entry flags + above-terrain residual.
        let mut crates: Vec<(
            ObjectId,
            Vec3,
            bool, // building_pickup
            bool, // residual paid
            bool, // above_terrain
            bool, // is_salvage
            u128, // required_kind_of
            u128, // forbidden_kind_of
        )> = Vec::new();
        for id in crate_ids {
            // Forget destroyed crates.
            let Some(obj) = self.host_object(id) else {
                self.host_money_crates.forget(id);
                continue;
            };
            if !obj.is_alive() {
                self.host_money_crates.forget(id);
                continue;
            }
            let entry = match self.host_money_crates.get(id) {
                Some(e) => e,
                None => continue,
            };
            // Salvage crates may grant upgrades with money_provided still set.
            if entry.building_pickup_residual_paid {
                continue;
            }
            if entry.money_provided == 0
                && !entry.is_salvage
                && !entry.is_veterancy
                && !entry.is_unit_crate
                && !entry.is_heal_crate
                && !entry.is_shroud_crate
            {
                continue;
            }
            let pos = obj.get_position();
            // Host residual ground plane 0; airborne while parachuting or elevated.
            let above = obj.is_parachuting() || crate_is_above_terrain(pos.y, 0.0);
            crates.push((
                id,
                pos,
                entry.building_pickup,
                entry.building_pickup_residual_paid,
                above,
                entry.is_salvage,
                entry.required_kind_of,
                entry.forbidden_kind_of,
            ));
        }

        // Snapshot candidate pickers.
        let pickers: Vec<(
            ObjectId,
            Team,
            Option<u32>,
            Vec3,
            bool, /*structure*/
            bool, /*constructed*/
            bool, /*projectile*/
            bool, /*parachute picker*/
            bool, /*salvager*/
            bool, /*has_ai*/
            u128, /*kind_mask*/
        )> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.team == Team::Neutral {
                    return None;
                }
                let is_projectile = obj.is_kind_of(KindOf::Projectile);
                // C++ rejects `other->isKindOf(KINDOF_PARACHUTE)`, not OBJECT_STATUS_PARACHUTING.
                let is_parachute_picker = obj.is_kind_of(KindOf::Parachute);
                let is_structure =
                    obj.is_kind_of(KindOf::Structure) || obj.object_type == ObjectType::Building;
                let constructed = obj.is_constructed() && !obj.status.under_construction;
                let is_salvager = obj.is_kind_of(KindOf::Salvager)
                    || obj.is_kind_of(KindOf::WeaponSalvager)
                    || obj.is_kind_of(KindOf::ArmorSalvager);
                let is_mine = obj.is_kind_of(KindOf::Mine)
                    || obj.is_kind_of(KindOf::DemoTrap)
                    || obj.mine_data.is_some();
                let is_crate = obj.is_kind_of(KindOf::Crate);
                let has_ai = HostMoneyCrateRegistry::picker_has_ai_update(
                    is_structure,
                    is_mine,
                    is_projectile,
                    is_crate,
                );
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    is_structure,
                    constructed,
                    is_projectile,
                    is_parachute_picker,
                    is_salvager,
                    has_ai,
                    obj.kind_of_cpp_mask(),
                ))
            })
            .collect();

        let mut pickups: Vec<(ObjectId, ObjectId, Team, Option<u32>, bool)> = Vec::new();
        let mut above_rejects = 0_u32;
        let mut forbidden_rejects = 0_u32;
        for (
            crate_id,
            crate_pos,
            building_pickup,
            _paid,
            above_terrain,
            is_salvage,
            required_kind_of,
            forbidden_kind_of,
        ) in &crates
        {
            let (crate_owner, crate_human_only, crate_forbid_owner) = {
                let entry = self.host_money_crates.get(*crate_id);
                let crate_obj = self.objects.get(crate_id);
                let template = crate_obj.map(|o| o.template_name.as_str()).unwrap_or("");
                let owner = crate_obj.and_then(|o| o.owner_player_id);
                let human = entry.is_some_and(|e| e.human_only)
                    || crate::game_logic::host_money_crate::crate_object_human_only(template);
                let forbid = entry.is_some_and(|e| e.forbid_owner_player)
                    || crate::game_logic::host_money_crate::crate_object_forbid_owner(template);
                (owner, human, forbid)
            };
            // Pure residual acquire: nearest legal picker in unit/building pickup radius (XZ).
            // Reject counters still run in the candidate filter phase.
            let mut structure_by_id: std::collections::HashMap<ObjectId, bool> =
                std::collections::HashMap::new();
            let mut team_by_id: std::collections::HashMap<ObjectId, Team> =
                std::collections::HashMap::new();
            let mut owner_by_id: std::collections::HashMap<ObjectId, Option<u32>> =
                std::collections::HashMap::new();
            let mut cands: Vec<crate::game_logic::host_residual_acquire::ResidualAcquireCandidate> =
                Vec::new();
            for (
                picker_id,
                team,
                picker_owner,
                picker_pos,
                is_structure,
                constructed,
                is_projectile,
                is_parachute_picker,
                is_salvager,
                has_ai,
                kind_mask,
            ) in &pickers
            {
                // C++ SalvageCrateCollide::isValidToExecute — only SALVAGER units.
                if *is_salvage && !*is_salvager {
                    continue;
                }
                if *is_salvage {
                    // C++ base isValidToExecute: md->m_pickupScience requires
                    // other->getControllingPlayer()->hasScience(...). Resolve the
                    // controlling player (owner id or unique faction player).
                    let picker_player = self.player_owner_for_event(*picker_owner, *team);
                    let has_science = picker_player
                        .and_then(|pid| self.players.get(&pid))
                        .is_some_and(|player| {
                            crate::game_logic::host_supply_gather::player_has_science(
                                player.unlocked_sciences.iter().map(|s| s.as_str()),
                                crate::game_logic::host_gamedata_lobby_residual::SALVAGE_PICKUP_SCIENCE_RESIDUAL,
                            )
                        });
                    if !has_science {
                        continue;
                    }
                }

                // Salvage crates are not building-pickup residual.
                if *is_salvage && *is_structure {
                    continue;
                }
                if *picker_id == *crate_id {
                    continue;
                }
                let dist = HostMoneyCrateRegistry::horizontal_distance(*crate_pos, *picker_pos);
                // C++ CrateCollide::isValidToExecute: isNeutralControlled rejects.
                let picker_is_neutral = *team == Team::Neutral
                    || picker_owner
                        .and_then(|pid| self.players.get(&pid))
                        .is_some_and(|player| player.team == Team::Neutral);
                if picker_is_neutral {
                    continue;
                }
                // C++ CrateCollide::isValidToExecute ForbidOwnerPlayer / HumanOnly.
                if crate_forbid_owner && crate_owner.is_some() && crate_owner == *picker_owner {
                    continue;
                }
                if crate_human_only {
                    let picker_human = picker_owner
                        .and_then(|pid| self.players.get(&pid))
                        .is_some_and(|player| player.is_local);
                    if !picker_human {
                        continue;
                    }
                }
                // C++ isKindOfMulti(m_kindof, m_kindofnot) + KINDOF_PARACHUTE veto.
                if *is_parachute_picker
                    || !HostMoneyCrateRegistry::picker_matches_kindof(
                        *kind_mask,
                        *required_kind_of,
                        *forbidden_kind_of,
                    )
                {
                    forbidden_rejects = forbidden_rejects.saturating_add(1);
                    continue;
                }
                if *is_structure {
                    if !HostMoneyCrateRegistry::is_legal_building_picker(
                        true,
                        picker_is_neutral,
                        true,
                        *constructed,
                        *building_pickup,
                    ) {
                        continue;
                    }
                    if dist > MONEY_CRATE_BUILDING_PICKUP_RADIUS {
                        continue;
                    }
                } else {
                    if *above_terrain {
                        // Unit path blocked while crate airborne residual.
                        above_rejects = above_rejects.saturating_add(1);
                        continue;
                    }
                    if !HostMoneyCrateRegistry::is_legal_unit_picker(
                        true,
                        picker_is_neutral,
                        false,
                        *is_projectile,
                        *is_parachute_picker,
                        *above_terrain,
                        *has_ai,
                    ) {
                        continue;
                    }
                    if dist > MONEY_CRATE_UNIT_PICKUP_RADIUS {
                        continue;
                    }
                }
                structure_by_id.insert(*picker_id, *is_structure);
                team_by_id.insert(*picker_id, *team);
                owner_by_id.insert(*picker_id, *picker_owner);
                cands.push(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: *picker_id,
                        team: *team,
                        position: *picker_pos,
                        is_alive: true,
                        is_neutral: picker_is_neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                );
            }
            let max_r = MONEY_CRATE_BUILDING_PICKUP_RADIUS.max(MONEY_CRATE_UNIT_PICKUP_RADIUS);
            if let Some((picker_id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(*crate_id),
                    (crate_pos.x, crate_pos.z),
                    cands,
                    max_r,
                    |c| !c.is_neutral,
                )
            {
                let is_structure = structure_by_id.get(&picker_id).copied().unwrap_or(false);
                let team = team_by_id.get(&picker_id).copied().unwrap_or(Team::Neutral);
                let owner_player_id = owner_by_id.get(&picker_id).copied().flatten();
                pickups.push((*crate_id, picker_id, team, owner_player_id, is_structure));
            }
        }
        for _ in 0..above_rejects.min(1) {
            // Count one honesty reject per update when any airborne unit path blocked.
            self.host_money_crates.record_above_terrain_unit_reject();
        }
        if above_rejects > 1 {
            // Extra airborne reject events (still one honesty flag is enough;
            // keep counter proportional for observability).
            for _ in 1..above_rejects.min(8) {
                self.host_money_crates.record_above_terrain_unit_reject();
            }
        }
        for _ in 0..forbidden_rejects.min(4) {
            self.host_money_crates.record_forbidden_kindof_reject();
        }

        for (crate_id, picker_id, team, picker_owner, is_structure) in pickups {
            let Some(entry) = self.host_money_crates.get(crate_id).cloned() else {
                continue;
            };
            let owner_player_id = self.player_owner_for_event(picker_owner, team);
            let has_supply_lines = owner_player_id
                .and_then(|player_id| self.players.get(&player_id))
                .is_some_and(|player| player.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));

            // C++ ShroudCrateCollide residual path.
            if entry.is_shroud_crate {
                let _ = self.execute_shroud_crate_behavior(picker_id);
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateShroud")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object_with_effects(crate_id, effects);
                continue;
            }
            // C++ HealCrateCollide residual path.
            if entry.is_heal_crate {
                let _ = self.execute_heal_crate_behavior(picker_id);
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateHeal")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object_with_effects(crate_id, effects);
                continue;
            }
            // C++ UnitCrateCollide residual path.
            if entry.is_unit_crate {
                let _ = self.execute_unit_crate_behavior_with_effects(
                    picker_id,
                    &entry.unit_crate_type,
                    entry.unit_crate_count,
                    effects,
                );
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateFreeUnit")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object_with_effects(crate_id, effects);
                continue;
            }
            // C++ VeterancyCrateCollide residual path.
            if entry.is_veterancy {
                // C++ onCollide: isValidToExecute false leaves the crate.
                if !self.veterancy_crate_is_valid_to_execute(picker_id, entry.veterancy_levels) {
                    continue;
                }
                // C++ executeCrateBehavior: crate AI goal must be the picker.
                if !self.veterancy_crate_ai_goal_matches(crate_id, picker_id) {
                    continue;
                }
                let _ = self.execute_veterancy_crate_behavior(
                    crate_id,
                    picker_id,
                    entry.veterancy_effect_range,
                    entry.veterancy_levels,
                );

                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CratePromote")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object_with_effects(crate_id, effects);
                continue;
            }
            // C++ SalvageCrateCollide residual path.
            let (amount, boost, salvage_kind) = if entry.is_salvage {
                let seed = crate_id
                    .0
                    .wrapping_add(picker_id.0)
                    .wrapping_add(self.frame);
                let (kind, money) =
                    self.execute_salvage_crate_behavior(picker_id, entry.money_provided, seed);
                (money, 0u32, Some(kind))
            } else {
                let (cash, extra) =
                    HostMoneyCrateRegistry::cash_for_pickup(&entry, has_supply_lines);
                (cash, extra, None)
            };
            // Salvage may grant upgrade with 0 money — still consume crate.
            if amount == 0 && !entry.is_salvage {
                continue;
            }
            if !self
                .host_money_crates
                .record_pickup(crate_id, amount.max(1), boost, is_structure)
            {
                continue;
            }
            if amount > 0 {
                if let Some(player_id) = owner_player_id {
                    if let Some((player, economy_events)) =
                        self.player_and_economy_events_mut(player_id)
                    {
                        player.credit_supplies(amount, economy_events);
                        // C++ ScoreKeeper::addMoneyEarned — crate cash is earned.
                        player.add_money_earned(amount);
                    }
                }
            }
            if boost > 0 {
                self.supply_lines_bonus_cash_total =
                    self.supply_lines_bonus_cash_total.saturating_add(boost);
            }
            let pos = self
                .host_object(crate_id)
                .map(|o| o.get_position())
                .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                .unwrap_or(Vec3::ZERO);
            // C++ SalvageCrateCollide::doMoney emits GUI:AddCash in player color.
            // C++ MoneyCrateCollide never addFloatingText (audio + optional INI anim).
            let salvage_upgrade = matches!(salvage_kind, Some("armor" | "weapon"));
            let salvage_money = matches!(salvage_kind, Some("money"));
            let money_crate = salvage_kind.is_none();
            if money_crate {
                // ExecuteAnimation MoneyPickUp residual (INI-authored money crates).
                let anim =
                    HostMoneyCrateRegistry::money_pickup_anim(crate_id, picker_id, pos, self.frame);
                self.admit_money_pickup_animation(anim);
            }
            if salvage_money && amount > 0 {
                let player_color = owner_player_id
                    .and_then(|player_id| self.players.get(&player_id))
                    .map(|player| player.color_rgb)
                    .unwrap_or((200, 200, 200));
                let floating = HostMoneyCrateRegistry::salvage_money_floating_text(
                    crate_id,
                    picker_id,
                    pos,
                    amount,
                    self.frame,
                    player_color,
                );
                self.host_money_crates.record_money_floating_text(floating);
            }
            let money_feedback = money_crate || salvage_money;

            let audio_name = if salvage_upgrade {
                "CrateSalvage"
            } else if money_feedback {
                MONEY_CRATE_PICKUP_AUDIO
            } else {
                // Level: C++ promotion audio is owned by gainExpForLevel.
                ""
            };
            if !audio_name.is_empty() {
                self.queue_audio_event(
                    AudioEventRequest::new(audio_name)
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
            }
            self.destroy_object_with_effects(crate_id, effects);
            log::info!(
                "Host MoneyCrateCollide residual: crate {:?} → picker {:?} team={:?} amount={} building={}",
                crate_id,
                picker_id,
                team,
                amount,
                is_structure
            );
        }
    }

    /// AmericaCrateParachute residual: freefall → OpenDist open → sink to ground.
    ///
    /// Applied to residual money crates that spawned from DeliverPayload cargo
    /// (PutInContainer AmericaCrateParachute). Fail-closed: not full container
    /// Object / W3D bone / CrateParachuteLocomotor force matrix.
    pub(crate) fn tick_crate_parachute_residual(&mut self, crate_id: ObjectId) {
        self.tick_crate_parachute_residual_with_effects(crate_id, &mut Default::default());
    }

    pub(crate) fn tick_crate_parachute_residual_with_effects(
        &mut self,
        crate_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_deliver_payload::{
            CRATE_PARACHUTE_LAND_AUDIO, CRATE_PARACHUTE_OPEN_AUDIO, should_open_crate_parachute,
            tick_crate_parachute_height,
        };

        // Only residual money crates use this path (pilot path is separate).
        if !self.host_money_crates.contains(crate_id) {
            return;
        }
        let (pos, chute_open, start_h, pitch, roll, landing_override) =
            match self.objects.get(&crate_id) {
                Some(obj) if obj.is_alive() && obj.is_parachuting() => (
                    obj.get_position(),
                    obj.is_parachute_open(),
                    obj.status.parachute_start_height,
                    obj.parachute_pitch(),
                    obj.parachute_roll(),
                    obj.parachute_landing_override(),
                ),
                _ => return,
            };
        let ground = 0.0_f32;
        let mut just_opened = false;
        let mut open = chute_open;
        if !open && should_open_crate_parachute(start_h, pos.y) {
            open = true;
            just_opened = true;
        }
        let (new_y, landed) = tick_crate_parachute_height(pos.y, ground, open);
        // ParachuteDirectly residual: open chute steers XZ to DeliverPayload target.
        let mut nx = pos.x;
        let mut nz = pos.z;
        let mut did_override_step = false;
        if open && !landed {
            if let Some(target) = landing_override {
                use crate::game_logic::host_usa_pilot::{
                    PARACHUTE_LANDING_OVERRIDE_SPEED, step_parachute_landing_override,
                };
                let (sx, sz, moved) = step_parachute_landing_override(
                    pos.x,
                    pos.z,
                    target.x,
                    target.z,
                    PARACHUTE_LANDING_OVERRIDE_SPEED,
                );
                if moved {
                    nx = sx;
                    nz = sz;
                    did_override_step = true;
                }
            }
        }
        if let Some(obj) = self.objects.get_mut(&crate_id) {
            if just_opened {
                obj.open_eject_parachute();
            }
            let mut p = obj.get_position();
            p.x = nx;
            p.z = nz;
            p.y = new_y;
            obj.set_position_with_effects(
                p,
                self.frame,
                &mut effects.reborrow_for_object(&mut self.partition_manager),
            );
            {}
            if landed {
                obj.clear_eject_parachuting();
            }
        }
        if did_override_step {
            self.usa_pilot.record_landing_override_step();
            self.host_deliver_payloads.record_parachute_directly_step();
        }
        if just_opened {
            self.host_deliver_payloads.record_crate_parachute_open();
            self.queue_audio_event(
                AudioEventRequest::new(CRATE_PARACHUTE_OPEN_AUDIO)
                    .with_position(Vec3::new(pos.x, new_y, pos.z))
                    .with_priority(145),
            );
        }
        // AmericaCrateParachute bone attach residual presentation (open chute).
        if open && !landed {
            let _attach = self.host_deliver_payloads.build_crate_parachute_attach(
                (pos.x, new_y, pos.z),
                pitch,
                roll,
                true,
            );
        }
        if landed {
            self.host_deliver_payloads.record_crate_parachute_land();
            self.queue_audio_event(
                AudioEventRequest::new(CRATE_PARACHUTE_LAND_AUDIO)
                    .with_position(Vec3::new(pos.x, ground, pos.z))
                    .with_priority(140),
            );
        }
    }

    /// CommandCenter / RadarVan radar-online residual (C++ Player::hasRadar).
    ///
    /// Retail: America CC GrantUpgradeCreate Upgrade_AmericaRadar + RadarUpgrade;
    /// China CC RadarUpgrade TriggeredBy Upgrade_ChinaRadar (researched);
    /// GLA RadarVan GrantUpgradeCreate Upgrade_GLARadar + RadarUpgrade DisableProof.
    pub(crate) fn update_player_radar(&mut self) {
        self.update_player_radar_with_effects(&mut Default::default())
    }

    pub(crate) fn update_player_radar_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let ids: Vec<_> = self.objects.registered_keys().copied().collect();
        for id in ids {
            self.admit_host_radar_provider_with_effects(id, effects);
        }
        let removed: Vec<_> = self
            .host_radar
            .providers
            .keys()
            .copied()
            .filter(|id| !self.objects.contains_key(id))
            .collect();
        for id in removed {
            self.remove_host_radar_provider(id);
        }
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            let player = self.players.get_mut(&id).unwrap();
            let had = player.has_radar();
            let brownout = !player.has_sufficient_energy_at(self.frame);
            if brownout {
                player.disable_radar();
            } else {
                player.enable_radar();
            }
            let count = player.radar_count.max(0) as u32;
            let has = player.has_radar();
            self.record_host_radar_transition(id, count, had, has);
        }
    }

    /// C++ VeterancyCrateCollide::isValidToExecute for the collider.
    /// Heroic / untrainable / levels<=0 / airborne leave the crate on the ground.
    fn veterancy_crate_is_valid_to_execute(&self, picker_id: ObjectId, levels: u8) -> bool {
        let Some(picker) = self.objects.get(&picker_id) else {
            return false;
        };
        if !picker.is_alive() || picker.status.destroyed {
            return false;
        }
        if levels == 0 {
            return false;
        }
        // C++ VeterancyCrateCollide.cpp:59-62 — flying / jumping pickers skip.
        if picker.is_significantly_above_terrain() {
            return false;
        }
        if !picker.is_trainable() {
            return false;
        }
        crate::game_logic::host_usa_pilot::vehicle_can_gain_exp_for_levels(
            picker.experience.level,
            levels,
        )
    }

    /// C++ parity (Player::update → doPowerDisable): set/clear
    /// `disabled_underpowered` on all KINDOF_POWERED objects depending on
    /// whether their owning player has sufficient power.
    ///   C++ Energy::getEnergySupplyRatio: if consumption==0 return production
    ///   (0.0 with no plants). calcTimeToBuild clamps excess to 1.0, applies
    ///   the loaded GameData values, then uses 0.01 only for a non-positive rate.
    /// Observe this producer's actual player at its own update boundary.
    /// An earlier synchronous upgrade may have changed Energy this frame.
    pub(in super::super) fn production_power_factor_for_player(&self, id: u32) -> f32 {
        let Some(player) = self.players.get(&id) else {
            return 1.0;
        };
        let rules = self.production_energy_rules();
        let energy_percent = player.energy_supply_ratio_at(self.frame).min(1.0);
        let energy_short = (1.0 - energy_percent) * rules.penalty;
        let mut rate = (1.0 - energy_short).max(rules.min_speed);
        if energy_percent < 1.0 {
            rate = rate.min(rules.max_speed);
        }
        if rate <= 0.0 {
            rate = 0.01;
        }
        rate
    }

    pub(in super::super) fn compute_player_power_factors(
        &self,
    ) -> std::collections::HashMap<u32, f32> {
        // The selected session owns effective engine/map definitions.
        // Preserve exact authored zeros and C++ arithmetic below.
        let rules = self.production_energy_rules();
        let (
            low_energy_penalty_modifier,
            min_low_energy_production_speed,
            max_low_energy_production_speed,
        ) = (rules.penalty, rules.min_speed, rules.max_speed);

        let mut factors = std::collections::HashMap::new();
        for player in self.players.values() {
            let energy_ratio = player.energy_supply_ratio_at(self.frame);
            // C++ clamps excess supply to 100%, applies Min even at full
            // power, and only applies Max while energy is below 100%.
            let energy_percent = energy_ratio.min(1.0);
            let energy_short = (1.0 - energy_percent) * low_energy_penalty_modifier;
            let mut rate = (1.0 - energy_short).max(min_low_energy_production_speed);
            if energy_percent < 1.0 {
                rate = rate.min(max_low_energy_production_speed);
            }
            if rate <= 0.0 {
                rate = 0.01;
            }
            let factor = rate;
            factors.insert(player.id, factor);
        }
        factors
    }

    /// C++ GarrisonContain::onBodyDamageStateChange: BODY_REALLYDAMAGED edge
    /// ejects occupants unless KINDOF_GARRISONABLE_UNTIL_DESTROYED.
    pub(in super::super) fn check_building_damage_states(&mut self, object_ids: &[ObjectId]) {
        self.check_building_damage_states_with_effects(
            object_ids,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn check_building_damage_states_with_effects(
        &mut self,
        object_ids: &[ObjectId],
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Collect garrison buildings that need evacuation (borrow-safe).
        let mut evacuate_ids: Vec<(ObjectId, Vec<ObjectId>)> = Vec::new();

        for &obj_id in object_ids {
            let Some(obj) = self.objects.get(&obj_id) else {
                continue;
            };
            if !obj.is_alive() || !obj.is_constructed() || !obj.is_garrison_contain() {
                continue;
            }
            if obj.is_kind_of(KindOf::GarrisonableUntilDestroyed) {
                continue;
            }
            if obj.body_damage_state
                != crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
            {
                continue;
            }
            let occupants = obj.contained_units();
            if occupants.is_empty() {
                continue;
            }
            evacuate_ids.push((obj_id, occupants));
        }

        // C++ orderAllPassengersToExit → exitObjectViaDoor walk, not a ring dump.
        for (cid, occupants) in evacuate_ids {
            let _ = self.evacuate_container_now_with_effects(cid, false, presentation_effects);
            for occ_id in occupants {
                if let Some(unit) = self.objects.get(&occ_id) {
                    let p = unit.get_position();
                    {}
                    {}
                }
                self.record_garrison_residual_exit();
            }
        }
    }

    pub(in super::super) fn update_power_disabled_state(&mut self) {
        // One canonical callback implementation for immediate Energy changes
        // and periodic sabotage observation. PlayerList slot order is numeric.
        let mut players = self.players.keys().copied().collect::<Vec<_>>();
        players.sort_unstable();
        for id in players {
            let brownout = !self.players[&id].has_sufficient_energy_at(self.frame);
            self.admit_player_energy_brownout(id, brownout);
        }
        // C++ Eva::shouldPlayLowPower residual (local energy insufficient).
        self.update_eva_low_power();
    }

    pub(in super::super) fn check_bridge_disabled_statuses(&self) {
        // Dual-world OBJECT_REGISTRY status peel retired — host owns disabled state.
        let _ = self;
    }
}
