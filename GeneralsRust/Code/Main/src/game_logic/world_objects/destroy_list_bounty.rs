//! Host objects `impl GameLogic` — `destroy_list_bounty`.
//! process_destroy_list and cash bounty. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

#[cfg(test)]
#[path = "transport_movement_owner_tests.rs"]
mod transport_movement_owner_tests;

impl GameLogic {
    /// Wave 912: true when destroy queue or destroy-ready residual has work.
    #[inline]
    pub fn has_pending_destroy_work(&self) -> bool {
        if !self.objects_to_destroy.is_empty() {
            return true;
        }
        false
    }

    /// Wave 912: process destroy list only when residual work is pending.
    #[inline]
    pub fn process_destroy_list_if_needed(&mut self) {
        if self.has_pending_destroy_work() {
            self.process_destroy_list();
        }
    }

    pub fn process_destroy_list(&mut self) {
        // Wave 621: under damage authority, GameWorld health writeback records
        // lethal IDs; host marks them here before draining the destroy queue.
        {}
        let mut destroyed_structure = false;
        let mut rubble_stamps: Vec<(glam::Vec3, i32)> = Vec::new();
        while let Some(event) = self.objects_to_destroy.pop_front() {
            if !event.needs_death_effects() {
                destroyed_structure |= self.finish_direct_object_removal(event.id);
                continue;
            }
            #[cfg(feature = "game_client")]
            if let Some(draw_id) = self
                .objects
                .get(&event.id)
                .and_then(|o| o.jet_ai.lockon_drawable_id)
            {
                gamelogic::helpers::TheGameClient.destroy_drawable(draw_id);
            }
            self.pending_special_abilities.remove(&event.id);
            self.pending_special_abilities
                .retain(|_, ability| ability.target_id() != event.id);

            self.cancel_all_production(event.id);
            // Damage authority / an old snapshot can enqueue a death without
            // passing through mark_object_for_destruction.  Keep the one
            // typed EjectPilotDie onDie path live before removing the object.
            self.maybe_apply_eject_pilot_die(event.id);

            // C++ Object::onDie RECONSTRUCTING residual (lost rebuild → hole).
            let handled_recon = self.handle_reconstructing_death(event.id);
            // C++ RebuildHoleExposeDie residual. Primary peel is death-start
            // in mark_object_for_destruction; this is the snapshot fallback.
            // maybe_spawn_rebuild_hole is idempotent via rebuild_spawner_id.
            if !handled_recon {
                let _ = self.maybe_spawn_rebuild_hole(event.id);
            }

            // Snapshot CreateCrateDie residual fields before remove.
            let (crate_data, death_pos_pre, death_team_pre, last_src) =
                if let Some(o) = self.objects.get(&event.id) {
                    (
                        o.thing().template.create_crate_data.clone(),
                        o.get_position(),
                        o.team,
                        o.last_damage_source,
                    )
                } else {
                    (Vec::new(), glam::Vec3::ZERO, Team::Neutral, None)
                };
            if !crate_data.is_empty() {
                let _ = self.try_create_crates_on_die(
                    event.id,
                    death_pos_pre,
                    death_team_pre,
                    &crate_data,
                    last_src,
                );
            }

            // C++ FireWeaponWhenDeadBehavior::onDie residual.
            self.apply_fire_weapon_when_dead(event.id);

            // C++ classifyObjectFootprint: kill anyone still on LAYER_WALL
            // for this piece, then reclassify (AIPathfind.cpp:4126-4153).
            if self
                .objects
                .get(&event.id)
                .is_some_and(|o| o.is_kind_of(KindOf::WalkOnTopOfWall))
            {
                let splat = self
                    .pathfinding_system
                    .splat_units_on_wall_piece(event.id, &self.objects);
                self.pathfinding_system.remove_wall_piece(event.id);
                for sid in splat {
                    if sid == event.id {
                        continue;
                    }
                    if let Some(unit) = self.objects.get_mut(&sid) {
                        // C++ DAMAGE_FALLING / DEATH_SPLATTED / HUGE_DAMAGE_AMOUNT
                        // (AIPathfind.cpp:4143-4148).
                        if unit.take_damage_from_immediate_typed_death_with_repulsor_policy(
                            1.0e9,
                            Some(event.id),
                            crate::game_logic::combat::DamageType::Falling,
                            crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
                            &mut self.health_events,
                            &self.enable_repulsors,
                        ) {
                            self.objects_to_destroy
                                .push_back(DestructionEvent::after_death(sid, event.killer));
                        }
                    }
                }
            }

            // Defensive admission for residual after-death queue producers;
            // ordinary onDie/onDelete already consumed this registered latch.
            self.unregister_tunnel_containment(event.id);
            self.finish_building_sound(event.id);
            self.detach_host_fog_ghost(event.id);
            if let Some(obj) = self.objects.remove(&event.id) {
                // A DockUpdate queue is module-owned; discard it with the
                // owning dock Object rather than retaining a dead ObjectId.
                self.remove_host_dock_approach_queue(event.id);
                // C++ contain onRemoving when the occupant dies: leave the
                // garrison list and free this unit's FIREPOINT/STATION slot.
                if let Some(cid) = obj.contained_by {
                    let is_garrison = self
                        .objects
                        .get(&cid)
                        .is_some_and(|c| c.is_garrison_contain());
                    if let Some(container) = self.objects.get_mut(&cid) {
                        let _ = container.remove_occupant(event.id, &mut self.ownership_events);
                    }
                    if is_garrison {
                        self.recalc_garrison_apparent_controller(cid);
                    }
                }
                // C++ TunnelTracker::removeFromContain on occupant death.
                // Any non-container death (splash, scripts) must free the slot.
                if let Some(player_id) = self.tunnel_network.player_holding_unit(event.id) {
                    let exit_at = obj.contained_by.unwrap_or(event.id);
                    let _ = self
                        .tunnel_network
                        .record_exit(player_id, event.id, exit_at);
                }
                // C++ CaveContain::removeFromContain (CaveContain.cpp:54-83) on
                // occupant death: tracker remove then onRemoving (LastEmpty).
                if self.cave_system.index_holding_unit(event.id).is_some() {
                    let exit_at = obj.contained_by.unwrap_or(event.id);
                    let _ = self.exit_cave_unit(event.id, exit_at);
                }
                self.remove_host_radar_provider(event.id);
                self.host_radar_remove_object(event.id);
                crate::game_logic::host_destroy_log::record(event.id);
                // Wave 681: mid-frame GameWorld Destroy while coupled shadow tick is live.
                // End-of-tick host_destroy_log drain remains idempotent for unmapped IDs.

                // Combat particle residual: death → registry entry (explosion + smoke).
                // PresentationFrame / client can observe systems after the kill.
                let death_pos = obj.get_position();
                let is_structure = obj.is_kind_of(KindOf::Structure);
                if is_structure {
                    destroyed_structure = true;
                    let gs = self.pathfinding_system.grid.grid_size();
                    let r = ((obj.selection_radius / gs.max(1.0)).ceil() as i32)
                        .max(1)
                        .min(4);
                    rubble_stamps.push((death_pos, r));
                }
                let victim_team = obj.team;
                // C++ Object::onDie EVA residual (local, non-self-inflicted).
                let is_infantry = obj.is_kind_of(KindOf::Infantry);
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                // KINDOF_MP_COUNT_FOR_VICTORY residual class (main base buildings).
                let is_mp_count = is_structure
                    && (obj.is_kind_of(KindOf::CommandCenter)
                        || obj.is_kind_of(KindOf::FSPower)
                        || obj.is_kind_of(KindOf::PowerPlant)
                        || obj.is_kind_of(KindOf::FSBarracks)
                        || obj.is_kind_of(KindOf::FSWarFactory)
                        || obj.is_kind_of(KindOf::FSAirfield)
                        || obj.is_kind_of(KindOf::FSSuperweapon)
                        || obj.is_kind_of(KindOf::FSStrategyCenter)
                        || obj.is_kind_of(KindOf::FSTechnology)
                        || obj.is_kind_of(KindOf::SupplyCenter)
                        || obj.is_kind_of(KindOf::FSSupplyCenter));
                self.try_eva_on_local_object_death(
                    event.id,
                    victim_team,
                    is_structure,
                    is_infantry,
                    is_vehicle,
                    is_mp_count,
                    death_pos,
                    event.killer,
                );
                let frame = self.frame;
                let death_type = obj.status.death_type;
                let skip_generic_death_fx = obj
                    .slow_death
                    .as_ref()
                    .map(|s| s.has_authored_phase_fx())
                    .unwrap_or(false)
                    // C++ has no generic death explosion: an authored FXListDie
                    // module owns the death visual when present.
                    || obj.fx_list_die.is_some();
                if !skip_generic_death_fx {
                    let _ = self.combat_particles.spawn_death_fx_for_type(
                        death_pos,
                        frame,
                        event.id,
                        is_structure,
                        victim_team,
                        death_type,
                    );
                }

                // C++ death audio is DamageFX + FXList Sound nuggets (Wave 535 particles).
                // Never invent UnitDie / BuildingDie / UnitDieBurned.

                // Containment onDie completed while this object was installed.
                // Physical removal never replays passenger damage or teardown.

                // GLA Toxin Tractor death residual: ToxinShellWeapon → SmallPoisonField.
                // Fail-closed: not full FireWeaponWhenDead anthrax matrix / FX list.
                {
                    use crate::game_logic::host_toxin_tractor::{
                        UPGRADE_GLA_ANTHRAX_BETA, UPGRADE_GLA_ANTHRAX_GAMMA,
                        UPGRADE_GLA_ANTHRAX_GAMMA_ALT, anthrax_tier_from_flags,
                        is_chem_general_template, is_toxin_tractor_template,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_toxin_tractor_template(&obj.template_name)
                    {
                        let has_gamma = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                            || obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                        let has_beta = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                        let anthrax = anthrax_tier_from_flags(
                            has_gamma,
                            has_beta,
                            is_chem_general_template(&obj.template_name),
                        );
                        let death_pos = obj.get_position();
                        let team = obj.team;
                        let _ = self
                            .toxin_tractor
                            .spawn_death_field(event.id, team, death_pos, self.frame, anthrax);
                        self.queue_audio_event(
                            AudioEventRequest::new(
                                crate::game_logic::host_toxin_tractor::TOXIN_POISON_AUDIO,
                            )
                            .with_position(death_pos)
                            .with_priority(140),
                        );
                    }
                }

                // GLA Bomb Truck FireWeaponWhenDead residual: HE/Bio detonation matrix.
                // Fail-closed: not full exclusive module / SubObjectsUpgrade payload visuals.
                // Note: object already removed from map — use `obj` snapshot for upgrades/pos.
                {
                    use crate::game_logic::host_bomb_truck_detonate::{
                        BombTruckDetonationProfile, UPGRADE_BOMB_TRUCK_BIO, UPGRADE_BOMB_TRUCK_HE,
                        UPGRADE_GLA_ANTHRAX_BETA, is_bomb_truck_template,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_bomb_truck_template(&obj.template_name)
                    {
                        let he = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_HE)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
                        let bio = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_BIO)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckBioBomb");
                        let anthrax = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA,
                            )
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                            );
                        let profile = BombTruckDetonationProfile::from_upgrades(he, bio, anthrax);
                        let _ = self.apply_bomb_truck_death_detonation_at(
                            event.id, obj.team, death_pos, profile,
                        );
                    }
                }

                // China Nuclear Tanks FireWeaponWhenDead residual: dual-radius + radiation.
                // Fail-closed: not full exclusive module / Nuclear*Locomotor visual matrix.
                {
                    use crate::game_logic::host_nuclear_tanks::{
                        has_nuclear_tanks_upgrade, is_nuclear_tanks_eligible,
                        is_nuke_general_nuclear_tanks,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_nuclear_tanks_eligible(&obj.template_name)
                        && has_nuclear_tanks_upgrade(&obj.applied_upgrades)
                    {
                        let nuke_gen = is_nuke_general_nuclear_tanks(&obj.template_name);
                        let _ = self.apply_nuclear_tanks_death_detonation_at(
                            event.id, obj.team, death_pos, nuke_gen,
                        );
                    }
                }

                // Demo SuicideBomb FireWeaponWhenDead residual: Demo_DestroyedWeapon blast.
                // Skip intentional SUICIDED path (PlusFire already applied via TertiarySuicide).
                // Skip terrorists (already handled by host_terrorist SUICIDED residual).
                {
                    use crate::game_logic::host_demo_suicide_bomb::{
                        has_demo_suicide_bomb_upgrade, is_demo_suicide_bomb_eligible_template,
                    };
                    use crate::game_logic::host_terrorist::is_terrorist_template;
                    if !obj.fire_weapon_when_dead_fired
                        && !obj.demo_suicided_detonating
                        && is_demo_suicide_bomb_eligible_template(&obj.template_name)
                        && has_demo_suicide_bomb_upgrade(&obj.applied_upgrades)
                        && !is_terrorist_template(&obj.template_name)
                    {
                        let _ =
                            self.apply_demo_suicide_bomb_death_at(event.id, obj.team, death_pos);
                    }
                }

                // GLA Rebel BoobyTrap residual: structure death detonates trap.
                // C++ Object::checkAndDetonateBoobyTrap(NULL) on die path.
                if obj.status.booby_trapped || self.booby_trap.is_booby_trapped(event.id) {
                    let _ = self.detonate_booby_trap_at(event.id, death_pos, None, false, true);
                }

                log::debug!(
                    "Destroyed object {} ({})",
                    event.id,
                    obj.get_template().name
                );
                self.record_destruction(&obj, event.killer);

                self.clear_removed_object_references(event.id);
            }
        }

        if destroyed_structure {
            // Rebuild static path/LOS mask without the destroyed footprint.
            self.sync_structure_path_blocks();
            for (pos, radius) in rubble_stamps {
                self.pathfinding_system.stamp_rubble_at_world(pos, radius);
            }
        }
    }

    pub(in super::super) fn record_destruction(
        &mut self,
        destroyed_object: &Object,
        killer: Option<Team>,
    ) {
        let destroyed_is_structure = destroyed_object.is_kind_of(KindOf::Structure);
        let victim_team = destroyed_object.team;
        let victim_id = destroyed_object.id;
        let victim_pos = destroyed_object.get_position();
        // C++ Object::scoreTheKill / Player::doBountyForKill:
        // no bounty for under-construction, non-enemy, or same-controller kills.
        let under_construction = destroyed_object.status.under_construction;
        let victim_owner_player_id = self.player_owner_for_host_object(destroyed_object);
        // C++ victim->getTemplate()->calcCostToBuild(victim->getControllingPlayer()).
        // No controlling player → 0 (ThingTemplate.cpp:1510-1511).
        let build_cost = match victim_owner_player_id {
            Some(pid) => self.modified_build_cost_supplies(
                pid,
                &destroyed_object.template_name,
                destroyed_object.thing().template.build_cost.supplies,
            ),
            None => 0,
        };

        let mut bounty_awarded = 0_u32;
        let mut bounty_killer_id = ObjectId(0);
        let mut bounty_float_pos = victim_pos;
        let mut used_last_damage_source = false;
        if let Some(team) = killer {
            // `killer` is still a legacy team event, but BodyModule gives us
            // the actual attacking object.  Carry that object's player owner
            // through scoring instead of selecting the first same-faction
            // player slot.
            let mut killer_owner_player_id = None;
            // Prefer C++ BodyModule last_damage_source residual for killer ObjectId.
            if let Some(src) = destroyed_object.last_damage_source {
                if let Some(src_obj) = self.objects.get(&src) {
                    if src_obj.team == team {
                        bounty_killer_id = src;
                        bounty_float_pos = src_obj.get_position();
                        killer_owner_player_id = self.player_owner_for_host_object(src_obj);
                        used_last_damage_source = true;
                    }
                } else {
                    // Killer already removed this frame — still record ObjectId residual.
                    bounty_killer_id = src;
                    used_last_damage_source = true;
                }
            }
            // Fallback residual: nearest living unit on killer team near victim
            // (destruction event carries team; last_damage_source may be unset).
            if !used_last_damage_source {
                if let Some((kid, kpos)) = self
                    .objects
                    .iter()
                    .filter(|(_, o)| o.team == team && o.is_alive())
                    .map(|(id, o)| (*id, o.get_position()))
                    .min_by(|a, b| {
                        let da = (a.1.x - victim_pos.x).hypot(a.1.z - victim_pos.z);
                        let db = (b.1.x - victim_pos.x).hypot(b.1.z - victim_pos.z);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    bounty_killer_id = kid;
                    bounty_float_pos = kpos;
                }
            }
            let enemy_kill = match (killer_owner_player_id, victim_owner_player_id) {
                (Some(killer_player_id), Some(victim_player_id)) => {
                    self.player_relationship(killer_player_id, victim_player_id)
                        == gamelogic::common::Relationship::Enemies
                }
                // A genuinely unowned victim has no player relationship.
                // Preserve the legacy faction gate for map/old-save objects
                // without assigning it a player.
                _ => team != victim_team && team != Team::Neutral && victim_team != Team::Neutral,
            };
            let score_counts = self.score_the_kill_victim_counts(destroyed_object);
            let scoring_enabled = self.is_scoring_enabled();
            let counts_destroyed_building =
                Self::live_score_counts_as_building_destroy(destroyed_object);
            let counts_destroyed_unit = Self::live_score_counts_as_unit_destroy(destroyed_object);
            let template_name = destroyed_object.template_name.clone();
            if let Some(player_id) = killer_owner_player_id {
                let mut rank_skill = 0;
                let self_kill = victim_owner_player_id == Some(player_id);
                // C++ scoreTheKill: addObjectDestroyed only after ENEMIES and
                // controller != victimController (Object.cpp:2915-2927).
                let record_destroyed = !under_construction
                    && score_counts
                    && enemy_kill
                    && !self_kill
                    && (counts_destroyed_building || counts_destroyed_unit);
                if let Some(player) = self.players.get_mut(&player_id) {
                    if record_destroyed && scoring_enabled {
                        if counts_destroyed_building {
                            player.record_structure_destroyed();
                        } else if counts_destroyed_unit {
                            player.record_unit_destroyed();
                        }
                    }

                    // Cash bounty residual: award ceil(cost * percent) on enemy kill.
                    // C++ scoreTheKill gates doBountyForKill with playable-side + IGNORED_IN_GUI.
                    if enemy_kill
                        && !under_construction
                        && score_counts
                        && player.cash_bounty_percent > 0.0
                    {
                        bounty_awarded =
                            player.do_bounty_for_kill(build_cost, &mut self.economy_events);
                    }

                    // C++ Player::addSkillPointsForKill (scoreTheKill).
                    // Skill value is victim template SkillPointValue / ExperienceValue.
                    // C++ Object.cpp:2898-2905: skip non-playable and IGNORED_IN_GUI.
                    if enemy_kill && !under_construction && score_counts {
                        rank_skill = destroyed_object.kill_skill_point_value();
                    }
                }
                if rank_skill != 0 {
                    let _ = self.add_player_skill_points(player_id, rank_skill);
                }
                if record_destroyed {
                    if let Some(victim_id) = victim_owner_player_id {
                        gamelogic::player::notify_live_object_destroyed(
                            player_id,
                            victim_id,
                            &template_name,
                            under_construction,
                        );
                    }
                }
            }

            // C++ CashBountyPower presentation: floating "+$N" AddCash text at
            // the killer position; registry honesty records the award.
            if bounty_awarded > 0 {
                if used_last_damage_source {
                    self.cash_bounty.record_last_damage_source_kill();
                }
                self.cash_bounty.record_floating_text(
                    crate::game_logic::host_cash_bounty::HostCashBountyFloatingText::new(
                        bounty_killer_id,
                        destroyed_object.id,
                        bounty_float_pos,
                        bounty_awarded,
                        self.frame,
                    ),
                );
                self.cash_bounty.record_bounty_award(bounty_awarded);
            }

            // C++ addObjectLost only runs inside scoreTheKill (a killer called it)
            // after playable-side + IGNORED_IN_GUI. Killer-less sell/script destroy
            // never touches the keeper.
            if score_counts && !under_construction {
                if let Some(player_id) = victim_owner_player_id {
                    let counts_lost_building = counts_destroyed_building;
                    let counts_lost_unit = counts_destroyed_unit;
                    if let Some(player) = self.players.get_mut(&player_id) {
                        if scoring_enabled {
                            if counts_lost_building {
                                player.record_structure_lost();
                            } else if counts_lost_unit {
                                player.record_unit_lost();
                            }
                        }
                    }
                    if counts_lost_building || counts_lost_unit {
                        gamelogic::player::notify_live_object_lost(
                            player_id,
                            &destroyed_object.template_name,
                            under_construction,
                        );
                    }
                }
            }
        }
    }

    /// C++ Object::scoreTheKill playable-side + KINDOF_IGNORED_IN_GUI gate.
    pub(in super::super) fn score_the_kill_victim_counts(&self, victim: &Object) -> bool {
        if victim.is_kind_of(KindOf::IgnoredInGui) {
            return false;
        }
        if let Some(player_id) = self.player_owner_for_host_object(victim) {
            return self.player_is_playable_side(player_id);
        }
        // Unowned Neutral is civilian / observer leftover.
        victim.team != Team::Neutral
    }

    /// C++ `Player::isPlayableSide` — `PlayerTemplate::m_playableSide` only.
    /// Slot display names are never consulted.
    pub(in super::super) fn player_is_playable_side(&self, player_id: u32) -> bool {
        let Some(player) = self.players.get(&player_id) else {
            return false;
        };
        if let Some(template) = self.resolved_player_template(player_id) {
            return template.is_playable_side();
        }
        if let Some(ident) = self.player_template_identity(player_id) {
            return crate::game_logic::host_faction_skirmish_residual::find_player_template_residual(
                &ident.template_name,
            )
            .map(|residual| residual.playable)
            .unwrap_or(false);
        }
        // Unbound host slot: C++ `Player::init` always has a template.
        // FactionAmerica/China/GLA are PlayableSide=Yes; Civilian is No.
        match player.team {
            Team::USA | Team::China | Team::GLA => true,
            Team::Neutral => false,
        }
    }

    /// Set cash bounty percent on a player (residual / tests).
    /// Raises percent only (C++ CashBountyPower set if higher).
    pub fn set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    /// Force-set cash bounty percent (tests / load restore).
    pub fn force_set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.force_set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    fn cash_bounty_module_percent(
        module: &crate::game_logic::SpecialPowerModuleMetadata,
        science_name: Option<&str>,
        player_has_science: impl Fn(&str) -> bool,
    ) -> Option<f32> {
        if module.module_kind != crate::game_logic::SpecialPowerModuleKind::CashBountyPower {
            return None;
        }
        let required = module.required_science.as_deref().unwrap_or("");
        let template = module.special_power_template.as_str();
        let pct = crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(required)
            .or_else(|| {
                crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(template)
            })?;
        if let Some(science) = science_name {
            let sci_pct =
                crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(science);
            if sci_pct != Some(pct)
                && !required.eq_ignore_ascii_case(science)
                && !template.eq_ignore_ascii_case(science)
            {
                return None;
            }
        } else if !required.is_empty() && !player_has_science(required) {
            return None;
        }
        Some(pct)
    }

    /// C++ Player::addScience walks owned SpecialPowerModules and calls
    /// CashBountyPower::onSpecialPowerCreation. No palace module ⇒ no bounty.
    pub fn apply_cash_bounty_from_palace_modules(
        &mut self,
        player_id: u32,
        science_name: Option<&str>,
    ) -> bool {
        let Some(player) = self.players.get(&player_id) else {
            return false;
        };
        let sciences = player.unlocked_sciences.clone();
        let has_science = |name: &str| {
            sciences
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(name))
        };
        let mut best = 0.0f32;
        let mut found = false;
        for object in self.objects.values() {
            if !self.object_owned_by_player(object, player_id) {
                continue;
            }
            for module in &object.thing().template.special_power_modules {
                if let Some(pct) =
                    Self::cash_bounty_module_percent(module, science_name, has_science)
                {
                    found = true;
                    if pct > best {
                        best = pct;
                    }
                }
            }
        }
        if !found {
            return false;
        }
        self.set_player_cash_bounty(player_id, best)
    }

    /// C++ CashBountyPower::onObjectCreated — apply if the owner already has
    /// the module's required science.
    pub fn apply_cash_bounty_on_object_created(&mut self, object_id: ObjectId) {
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        let has_cash_bounty = object
            .thing()
            .template
            .special_power_modules
            .iter()
            .any(|m| m.module_kind == crate::game_logic::SpecialPowerModuleKind::CashBountyPower);
        if !has_cash_bounty {
            return;
        }
        let player_id = object.owner_player_id.or_else(|| {
            let team = object.team;
            self.unique_player_id_for_team(team)
        });
        let Some(player_id) = player_id else {
            return;
        };
        let _ = self.apply_cash_bounty_from_palace_modules(player_id, None);
    }

    /// Residual honesty: cash bounty was configured and at least one award paid.
    /// Fail-closed: not full palace module / floating-text parity.
    pub fn honesty_cash_bounty_ok(&self) -> bool {
        self.cash_bounty.honesty_ok()
    }

    /// Residual honesty: at least one bounty cash award on kill.
    pub fn honesty_cash_bounty_award_ok(&self) -> bool {
        self.cash_bounty.honesty_bounty_award_ok()
    }

    /// Residual cash bounty floating cash text honesty.
    pub fn honesty_cash_bounty_floating_text_ok(&self) -> bool {
        self.cash_bounty.honesty_floating_text_ok()
    }

    /// Total residual cash credited via kill bounty (observability).
    pub fn cash_bounty_earned_total(&self) -> u32 {
        self.cash_bounty.bounty_earned_total
    }

    /// Host cash bounty registry (tests / honesty).
    pub fn cash_bounty_registry(
        &self,
    ) -> &crate::game_logic::host_cash_bounty::HostCashBountyRegistry {
        &self.cash_bounty
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::game_logic::{
        GameLogic, KindOf, ObjectId, Player, PlayerTemplateIdentity, Team, ThingTemplate,
    };

    fn setup_two_tunnels_and_rider() -> (GameLogic, ObjectId, ObjectId, ObjectId, u32) {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::GLA, "GLA", true));
        let mut tn = ThingTemplate::new("GLATunnelNetwork");
        tn.add_kind_of(KindOf::Structure).set_health(1000.0);
        tn.build_cost.supplies = 800;
        logic.templates.insert("GLATunnelNetwork".into(), tn);
        let mut rebel = ThingTemplate::new("GLARebel");
        rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("GLARebel".into(), rebel);

        let t1 = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("t1");
        let t2 = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(80.0, 0.0, 0.0),
            )
            .expect("t2");
        let uid = logic
            .create_object("GLARebel", Team::GLA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("rider");
        if let Some(o) = logic.host_object_mut(t1) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
            let _ = o.add_occupant(uid);
        }
        if let Some(o) = logic.host_object_mut(t2) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
        }
        if let Some(u) = logic.host_object_mut(uid) {
            u.set_contained_by(Some(t1));
        }
        // C++ TunnelContain uses the admitted controlling player's tracker.
        // A faction sentinel is a different pool from the actual owner.
        let key = logic
            .host_object(t1)
            .expect("admitted entrance")
            .tunnel_system_key();
        assert_eq!(logic.host_object(t2).unwrap().tunnel_system_key(), key);
        assert!(
            logic
                .host_object(t1)
                .unwrap()
                .is_tunnel_network_style_container()
        );
        assert!(
            logic
                .host_object(t2)
                .unwrap()
                .is_tunnel_network_style_container()
        );
        logic.tunnel_network.on_tunnel_created(key, t1);
        logic.tunnel_network.on_tunnel_created(key, t2);
        assert!(logic.tunnel_network.record_enter(key, uid, t1));
        (logic, t1, t2, uid, key)
    }

    #[test]
    fn tunnel_on_die_keeps_shared_pool_when_another_entrance_lives() {
        // C++ TunnelContain.cpp:326 onDie — no OpenContain eject.
        let (mut logic, t1, t2, uid, key) = setup_two_tunnels_and_rider();
        let origin = logic
            .host_object(uid)
            .map(|o| o.get_position())
            .expect("pos");
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        assert!(
            logic.tunnel_network.is_in_network(key, uid),
            "occupant must stay in the shared pool"
        );
        let u = logic.host_object(uid).expect("rider lives");
        assert!(u.is_alive());
        assert!(!u.status.destroyed);
        assert_eq!(
            u.contained_by,
            Some(t2),
            "ContainedBy remaps to remaining tunnel"
        );
        let p = u.get_position();
        assert!(
            (p.x - origin.x).abs() < 0.01 && (p.z - origin.z).abs() < 0.01,
            "must not spill at rubble"
        );
        assert_eq!(logic.tunnel_network.contain_count(key), 1);
    }

    #[test]
    fn tunnel_on_die_remaps_to_oldest_registered_entrance() {
        // C++ TunnelTracker.cpp:201 m_tunnelIDs.front() after remove.
        let (mut logic, t1, t2, uid, key) = setup_two_tunnels_and_rider();
        let t3 = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(160.0, 0.0, 0.0),
            )
            .expect("t3");
        if let Some(o) = logic.host_object_mut(t3) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
        }
        assert_eq!(logic.host_object(t3).unwrap().tunnel_system_key(), key);
        logic.tunnel_network.on_tunnel_created(key, t3);
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        let u = logic.host_object(uid).expect("rider lives");
        assert_eq!(
            u.contained_by,
            Some(t2),
            "oldest surviving registered entrance, not later scan hit"
        );
        let _ = t3;
    }

    #[test]
    fn tunnel_on_die_remap_restarts_time_for_full_heal() {
        // C++ Object::onContainedBy restamps m_containedByFrame.
        let (mut logic, t1, _t2, uid, _key) = setup_two_tunnels_and_rider();
        logic.tunnel_network.stamp_contained_by_frame(uid, 0);
        logic.frame = 40;
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        assert_eq!(
            logic.tunnel_network.contained_by_frame(uid),
            Some(40),
            "remap must restart TimeForFullHeal"
        );
    }

    #[test]
    fn last_tunnel_die_cave_in_kills_pool() {
        // C++ TunnelTracker.cpp:192-197 last tunnel destroyObject all contained.
        let (mut logic, t1, t2, uid, key) = setup_two_tunnels_and_rider();
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        assert!(logic.tunnel_network.is_in_network(key, uid));
        logic.mark_object_for_destruction(t2, None);
        logic.process_destroy_list();
        assert_eq!(logic.tunnel_network.contain_count(key), 0);
        assert!(logic.tunnel_network.honesty_cave_in_ok());
        let dead = logic.host_object(uid);
        assert!(
            dead.is_none() || dead.is_some_and(|o| o.status.destroyed || !o.is_alive()),
            "cave-in must kill remaining pool"
        );
    }

    #[test]
    fn bunker_buster_record_exit_removes_tunnel_pool() {
        // C++ TunnelContain.cpp:95 harmAndForceExitAllContained + record_exit.
        let (mut logic, t1, _t2, uid, key) = setup_two_tunnels_and_rider();
        let (kills, _, _) = logic.apply_bunker_buster_to_target(t1, Team::USA, 100.0, None);
        assert!(kills >= 1);
        assert!(
            !logic.tunnel_network.is_in_network(key, uid),
            "bunker-buster must record_exit the shared pool occupant"
        );
    }

    #[test]
    fn tunnel_occupant_immune_to_entrance_splash() {
        // hq-vsp1v: C++ Weapon.cpp dealDamageInternal partition-world only.
        let (mut logic, t1, _t2, uid, key) = setup_two_tunnels_and_rider();
        logic
            .players
            .insert(1, Player::new(1, Team::USA, "USA", true));
        let mut gun = ThingTemplate::new("SplashGun");
        gun.add_kind_of(KindOf::Vehicle).set_health(100.0);
        logic.templates.insert("SplashGun".into(), gun);
        let gun_id = logic
            .create_object("SplashGun", Team::USA, glam::Vec3::new(-80.0, 0.0, 0.0))
            .expect("gun");
        let hp_before = logic.host_object(uid).unwrap().health.current;
        let _hits = logic.apply_instant_hit_splash_at(
            glam::Vec3::new(0.0, 0.0, 0.0),
            500.0,
            500.0,
            50.0,
            80.0,
            gun_id,
            Team::USA,
            t1,
            None,
        );
        let rider = logic.host_object(uid).expect("rider lives");
        assert!(
            rider.is_alive(),
            "tunnel occupant must survive entrance splash"
        );
        assert_eq!(rider.health.current, hp_before);
        assert!(logic.tunnel_network.is_in_network(key, uid));
        assert_eq!(logic.tunnel_network.contain_count(key), 1);
    }

    #[test]
    fn tunnel_occupant_death_frees_shared_slot() {
        // hq-vsp1v: C++ removeFromContain → TunnelTracker::removeFromContain.
        let (mut logic, _t1, _t2, uid, key) = setup_two_tunnels_and_rider();
        assert_eq!(logic.tunnel_network.contain_count(key), 1);
        logic.mark_object_for_destruction(uid, None);
        logic.process_destroy_list();
        assert!(
            !logic.tunnel_network.is_in_network(key, uid),
            "dead occupant must leave the shared MaxTunnelCapacity pool"
        );
        assert_eq!(logic.tunnel_network.contain_count(key), 0);
    }

    #[test]
    fn tunnel_death_callback_precedes_deferred_removal_and_is_once_only() {
        let (mut logic, first, second, rider, key) = setup_two_tunnels_and_rider();
        logic.frame = 40;
        // Explicitly admit a supported owner-local death continuation. The
        // callback phase must not depend on guessed name/kind topple admission.
        for entrance in [first, second] {
            let mut continuation =
                crate::game_logic::host_structure_topple::HostStructureToppleData::default();
            continuation.begin(40, 1.0, 0.0, 0);
            logic
                .host_object_mut(entrance)
                .unwrap()
                .structure_topple_data = Some(continuation);
        }
        let before = logic.tunnel_network.tunnels_destroyed;
        logic.mark_object_for_destruction(first, None);
        let dying = logic
            .host_object(first)
            .expect("entrance stays installed during onDie");
        assert!(dying.status.on_die_started);
        // The explicitly admitted continuation retains the entrance, while
        // its synchronous TunnelContain callback has already completed.
        assert!(
            dying
                .structure_topple_data
                .as_ref()
                .is_some_and(|data| data.is_active())
        );
        assert!(!logic.tunnel_network.tunnel_ids_for(key).contains(&first));
        assert_eq!(logic.tunnel_network.tunnel_ids_for(key), &[second]);
        assert_eq!(logic.host_object(rider).unwrap().contained_by, Some(second));
        assert_eq!(logic.tunnel_network.contained_by_frame(rider), Some(40));
        assert_eq!(logic.tunnel_network.tunnels_destroyed, before + 1);
        logic.process_destroy_list();
        assert!(
            logic.host_object(first).is_some(),
            "callbacks must precede deferred physical removal"
        );
        logic.mark_object_for_destruction(first, None);
        logic.destroy_object(first);
        logic.process_destroy_list();
        assert!(logic.host_object(first).is_none());
        assert_eq!(logic.tunnel_network.tunnels_destroyed, before + 1);
        assert_eq!(logic.tunnel_network.cave_ins, 0);
        assert_eq!(logic.tunnel_network.contain_count(key), 1);

        logic.mark_object_for_destruction(second, None);
        assert!(logic.host_object(second).unwrap().status.on_die_started);
        assert!(logic.tunnel_network.tunnel_ids_for(key).is_empty());
        assert_eq!(logic.tunnel_network.contain_count(key), 0);
        assert_eq!(logic.tunnel_network.tunnels_destroyed, before + 2);
        assert_eq!(logic.tunnel_network.cave_ins, 1);
        // C++ TunnelTracker::destroyObject calls destroyObject, not body damage.
        let child = logic
            .host_object(rider)
            .expect("queued child stays installed until removal");
        assert!(child.status.destroyed);
        assert!(!child.status.on_die_started);
        assert_eq!(child.contained_by, None);
        logic.mark_object_for_destruction(second, None);
        logic.destroy_object(second);
        logic.process_destroy_list();
        assert!(logic.host_object(second).is_none());
        assert!(logic.host_object(rider).is_none());
        assert_eq!(logic.tunnel_network.tunnels_destroyed, before + 2);
        assert_eq!(logic.tunnel_network.cave_ins, 1);
        assert_eq!(logic.tunnel_network.contain_count(key), 0);
    }

    #[test]
    fn bounty_uses_victim_calc_cost_to_build_and_money_earned() {
        // C++ Player.cpp:2409-2416 calcCostToBuild + addMoneyEarned.
        let mut logic = GameLogic::new();
        let mut killer = Player::new(0, Team::USA, "USA", true);
        killer.cash_bounty_percent = 0.20;
        killer.resources.supplies = 1_000;
        logic.add_player(killer);
        let mut victim = Player::new(2, Team::GLA, "GLA", true);
        victim.map_side.handicap_build_cost_generic = 0.80;
        logic.add_player(victim);
        let mut tank = ThingTemplate::new("TestTank");
        tank.add_kind_of(KindOf::Vehicle).set_health(100.0);
        tank.build_cost.supplies = 600;
        logic.templates.insert("TestTank".into(), tank);

        let cost = logic.modified_build_cost_supplies(2, "TestTank", 600);
        assert_eq!(cost, 480, "handicap 0.8 * 600");
        let bounty = {
            let (player, economy_events) = logic.player_and_economy_events_mut(0).unwrap();
            player.do_bounty_for_kill(cost, economy_events)
        };
        assert_eq!(bounty, 96);
        let usa = logic.get_player(0).expect("usa");
        assert_eq!(usa.statistics.money_earned, 96);
        assert_eq!(usa.resources.supplies, 1_096);
        assert_eq!(usa.calculate_score() as u32, 96);
    }

    pub(super) fn setup_zero_damage_transport(
        airborne: bool,
        hull: glam::Vec3,
    ) -> (GameLogic, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("TestAmphibTransport");
        t.add_kind_of(KindOf::Vehicle).set_health(200.0);
        t.contain_module = crate::game_logic::ContainModuleMetadata {
            kind: crate::game_logic::ContainModuleKind::Transport,
            slots: Some(5),
            ..Default::default()
        };
        logic.templates.insert("TestAmphibTransport".into(), t);
        let mut p = ThingTemplate::new("TestInfantry");
        p.add_kind_of(KindOf::Infantry).set_health(100.0);
        p.set_authored_ai_update_interface(Some(true));
        p.authored_locomotor_sets =
            Some(
                vec![crate::game_logic::host_upgrade_module_residuals::AuthoredLocomotorSet {
            kind: crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Normal,
            members: vec!["BasicHumanLocomotor".into()],
        }],
            );
        logic.templates.insert("TestInfantry".into(), p);

        let transport = logic
            .create_object("TestAmphibTransport", Team::USA, hull)
            .expect("transport");
        if let Some(c) = logic.host_object_mut(transport) {
            c.status.airborne_target = airborne;
            if airborne {
                c.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
            }
        }
        let rider = logic
            .create_object(
                "TestInfantry",
                Team::USA,
                hull + glam::Vec3::new(1.0, 0.0, 0.0),
            )
            .expect("rider");
        if let Some(r) = logic.host_object_mut(rider) {
            r.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
            r.set_contained_by(Some(transport));
        }
        assert!(
            logic
                .host_object_mut(transport)
                .unwrap()
                .add_occupant(rider)
        );
        (logic, transport, rider)
    }

    #[test]
    fn transport_death_kills_riders_not_free_to_exit() {
        // hq-yz6vy: amphibious / out-of-grid hull — infantry cannot stand → kill.
        let (mut logic, transport, rider) =
            setup_zero_damage_transport(false, glam::Vec3::new(10_000.0, 0.0, 10_000.0));
        logic.mark_object_for_destruction(transport, None);
        logic.process_destroy_list();
        let r = logic.host_object(rider);
        let dead = r.is_none() || r.is_some_and(|o| !o.is_alive() || o.status.destroyed);
        assert!(
            dead,
            "invalid-terrain transport death must kill riders, not dump them alive"
        );
    }

    #[test]
    fn airborne_transport_death_ejects_riders_alive() {
        // C++ isSpecificRiderFreeToExit: airborne hull always returns TRUE.
        let (mut logic, transport, rider) =
            setup_zero_damage_transport(true, glam::Vec3::new(10_000.0, 40.0, 10_000.0));
        logic.mark_object_for_destruction(transport, None);
        logic.process_destroy_list();
        let r = logic.host_object(rider).expect("rider stays in world");
        assert!(r.is_alive(), "airborne hull must eject living riders");
        assert!(!r.status.destroyed);
        assert!(r.contained_by.is_none());
    }

    #[test]
    fn transport_death_scatters_survivors_when_damage_percent_below_100() {
        // hq-lkiem / hq-j0ggx / hq-c77h2: Technical-style 10% DamagePercentToUnits
        // survivors use leftover scatterToNearbyPosition, not the Idle 8-unit ring.
        let hull = glam::Vec3::new(10_000.0, 40.0, 10_000.0);
        let (mut logic, transport, rider) = setup_zero_damage_transport(true, hull);
        logic.frame = 42;
        if let Some(c) = logic.host_object_mut(transport) {
            c.is_technical_transport = true;
            c.template_mut().geometry_info = crate::game_logic::HostGeometryInfo {
                geom_type: crate::game_logic::HostGeometryType::Cylinder,
                is_small: false,
                height: 10.0,
                major_radius: 10.0,
                minor_radius: 10.0,
                authored: true,
            };
        }
        if let Some(r) = logic.host_object_mut(rider) {
            r.health.current = 100.0;
            r.max_health = 100.0;
            r.next_mood_check_time = 9999;
            r.randomly_offset_mood_check = false;
        }
        logic.mark_object_for_destruction(transport, None);
        logic.process_destroy_list();
        let r = logic.host_object(rider).expect("survivor stays in world");
        assert!(
            r.is_alive(),
            "10% DamagePercentToUnits must leave a survivor"
        );
        assert!(!r.status.destroyed);
        assert!(r.contained_by.is_none());
        // TransportContain.cpp389-393 scatters, then wakes the surviving rider
        // to target; the installed aggressive exit phase admits attack-move.
        assert_eq!(r.ai_state, crate::game_logic::AIState::AttackMoving);
        assert_eq!(r.ignored_obstacle_id, Some(transport));
        assert_eq!(r.ignore_collisions_with, Some(transport));
        let pos = r.get_position();
        assert!(
            (pos - hull).length() < 0.01,
            "scatter places the rider at the wreck, not an Idle ring offset; got {pos:?}"
        );
        let dest = r.movement.path.last().copied().or_else(|| {
            logic
                .pathfinding_system
                .pending_paths()
                .find(|p| p.unit_id == rider)
                .map(|p| p.destination)
        });
        let dest = dest.expect("aiMoveToPosition dest");
        let dx = dest.x - hull.x;
        let dz = dest.z - hull.z;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist >= 10.0 - 0.01 && dist <= 15.0 + 0.01,
            "scatter dest must sit on the 1.0–1.5× bounding-radius ring, got {dist}"
        );
        let angle = dz.atan2(dx);
        assert!(
            (r.get_orientation() - angle).abs() < 0.05
                || (r.get_orientation() - angle).abs() > std::f32::consts::TAU - 0.05,
            "orientation must match the leftover scatter angle"
        );
        assert_eq!(
            r.next_mood_check_time, 9999,
            "death scatter starts movement before the idle-only mood wake"
        );
        assert!(!r.randomly_offset_mood_check);
        let audio =
            gamelogic::object::contain::open_contain::leftover_last_on_removing_template_call()
                .expect("onRemoving template audio");
        assert_eq!(audio.container_template, "TestAmphibTransport");
        assert_eq!(audio.container_id, transport.0);
        assert_eq!(audio.rider_template, "TestInfantry");
        assert_eq!(audio.rider_id, rider.0);
    }

    #[test]
    fn score_the_kill_gates_destroy_and_lost_counters() {
        // hq-oiyeg: C++ Object::scoreTheKill — lost only with a killer + playable
        // victim; destroyed only for ENEMIES and not self.
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.alliance_team = 1;
        logic.add_player(usa);
        let mut china = Player::new(1, Team::China, "China", false);
        china.alliance_team = 1;
        logic.add_player(china);
        let mut gla = Player::new(2, Team::GLA, "GLA", false);
        gla.alliance_team = 2;
        logic.add_player(gla);

        let mut tank = ThingTemplate::new("ScoreTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Score)
            .set_health(10.0);
        logic.templates.insert("ScoreTank".into(), tank);
        let mut decoy = ThingTemplate::new("ScoreDecoy");
        decoy
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::IgnoredInGui)
            .set_health(10.0);
        logic.templates.insert("ScoreDecoy".into(), decoy);

        let killer = logic
            .create_object("ScoreTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("killer");
        let enemy = logic
            .create_object("ScoreTank", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("enemy");
        let ally = logic
            .create_object("ScoreTank", Team::China, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("ally");
        let sold = logic
            .create_object("ScoreTank", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
            .expect("sold");
        let ignored = logic
            .create_object("ScoreDecoy", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
            .expect("ignored");

        if let Some(v) = logic.host_object_mut(enemy) {
            v.last_damage_source = Some(killer);
        }
        logic.mark_object_for_destruction(enemy, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(logic.get_player(0).unwrap().statistics.units_destroyed, 1);
        assert_eq!(logic.get_player(2).unwrap().statistics.units_lost, 1);

        if let Some(v) = logic.host_object_mut(ally) {
            v.last_damage_source = Some(killer);
        }
        logic.mark_object_for_destruction(ally, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(
            logic.get_player(0).unwrap().statistics.units_destroyed,
            1,
            "allied friendly-fire must not add Destroyed"
        );
        assert_eq!(logic.get_player(1).unwrap().statistics.units_lost, 1);

        let gla_lost = logic.get_player(2).unwrap().statistics.units_lost;
        logic.mark_object_for_destruction(sold, None);
        logic.process_destroy_list();
        assert_eq!(
            logic.get_player(2).unwrap().statistics.units_lost,
            gla_lost,
            "killer-less sell/script destroy must not add Lost"
        );

        if let Some(v) = logic.host_object_mut(ignored) {
            v.last_damage_source = Some(killer);
        }
        logic.mark_object_for_destruction(ignored, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(logic.get_player(0).unwrap().statistics.units_destroyed, 1);
        assert_eq!(logic.get_player(2).unwrap().statistics.units_lost, gla_lost);
    }

    fn bind_score_template_identity(logic: &mut GameLogic, player_id: u32, template_name: &str) {
        logic.player_template_bindings.insert(
            player_id,
            PlayerTemplateIdentity {
                template_name: template_name.to_string(),
                template_index: None,
            },
        );
    }

    fn insert_score_unit_template(logic: &mut GameLogic, name: &str, cost: u32, skill: i32) {
        let mut tank = ThingTemplate::new(name);
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Score)
            .set_health(10.0);
        tank.build_cost.supplies = cost;
        tank.experience_value = skill as f32;
        tank.experience_values = [skill as f32; 4];
        tank.skill_point_values = [skill; 4];
        logic.templates.insert(name.into(), tank);
    }

    #[test]
    fn score_the_kill_playable_side_uses_player_template_not_name() {
        // hq-yzafi: C++ Player::isPlayableSide is PlayerTemplate.PlayableSide only.
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "Civilian", true);
        usa.alliance_team = 1;
        usa.cash_bounty_percent = 0.20;
        logic.add_player(usa);
        bind_score_template_identity(&mut logic, 0, "FactionAmerica");
        let mut gla = Player::new(2, Team::GLA, "Bob", false);
        gla.alliance_team = 2;
        gla.cash_bounty_percent = 0.20;
        logic.add_player(gla);
        bind_score_template_identity(&mut logic, 2, "FactionCivilian");

        assert!(
            logic.player_is_playable_side(0),
            "playable FactionAmerica slot named Civilian must still count"
        );
        assert!(
            !logic.player_is_playable_side(2),
            "FactionCivilian PlayableSide=No must not count even if name is Bob"
        );

        insert_score_unit_template(&mut logic, "ScoreSideTank", 500, 25);
        let gla_killer = logic
            .create_object("ScoreSideTank", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("gla killer");
        let usa_victim = logic
            .create_object("ScoreSideTank", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("usa victim");
        let usa_killer = logic
            .create_object("ScoreSideTank", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("usa killer");
        let gla_victim = logic
            .create_object("ScoreSideTank", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
            .expect("gla victim");

        if let Some(v) = logic.host_object_mut(usa_victim) {
            v.last_damage_source = Some(gla_killer);
        }
        logic.mark_object_for_destruction(usa_victim, Some(Team::GLA));
        logic.process_destroy_list();
        assert_eq!(logic.get_player(2).unwrap().statistics.units_destroyed, 1);
        assert_eq!(logic.get_player(0).unwrap().statistics.units_lost, 1);
        assert!(
            logic.get_player(2).unwrap().statistics.money_earned > 0,
            "bounty still awards when the named-Civilian victim is playable"
        );

        let usa_destroyed = logic.get_player(0).unwrap().statistics.units_destroyed;
        let gla_lost = logic.get_player(2).unwrap().statistics.units_lost;
        let usa_money = logic.get_player(0).unwrap().statistics.money_earned;
        if let Some(v) = logic.host_object_mut(gla_victim) {
            v.last_damage_source = Some(usa_killer);
        }
        logic.mark_object_for_destruction(gla_victim, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(
            logic.get_player(0).unwrap().statistics.units_destroyed,
            usa_destroyed,
            "non-playable template victim must not increment Destroyed"
        );
        assert_eq!(
            logic.get_player(2).unwrap().statistics.units_lost,
            gla_lost,
            "non-playable template victim must not increment Lost"
        );
        assert_eq!(
            logic.get_player(0).unwrap().statistics.money_earned,
            usa_money,
            "non-playable template victim must not award bounty"
        );
    }

    #[test]
    fn destroy_lost_counters_honor_scoring_enabled() {
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.alliance_team = 1;
        usa.cash_bounty_percent = 0.20;
        logic.add_player(usa);
        let mut gla = Player::new(2, Team::GLA, "GLA", false);
        gla.alliance_team = 2;
        logic.add_player(gla);
        insert_score_unit_template(&mut logic, "ScoreFlagTank", 500, 25);

        let killer = logic
            .create_object("ScoreFlagTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("killer");
        let off_victim = logic
            .create_object("ScoreFlagTank", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("off victim");
        let on_victim = logic
            .create_object("ScoreFlagTank", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("on victim");

        logic.execute_scoring_script_for_test(false);
        if let Some(v) = logic.host_object_mut(off_victim) {
            v.last_damage_source = Some(killer);
        }
        logic.mark_object_for_destruction(off_victim, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(logic.get_player(0).unwrap().statistics.units_destroyed, 0);
        assert_eq!(logic.get_player(2).unwrap().statistics.units_lost, 0);
        assert!(
            logic.get_player(0).unwrap().statistics.money_earned > 0,
            "DISABLE_SCORING must still award bounty"
        );
        assert!(
            logic.get_player(0).unwrap().skill_points > 0,
            "DISABLE_SCORING must still award skill points"
        );

        logic.execute_scoring_script_for_test(true);
        if let Some(v) = logic.host_object_mut(on_victim) {
            v.last_damage_source = Some(killer);
        }
        logic.mark_object_for_destruction(on_victim, Some(Team::USA));
        logic.process_destroy_list();
        assert_eq!(logic.get_player(0).unwrap().statistics.units_destroyed, 1);
        assert_eq!(logic.get_player(2).unwrap().statistics.units_lost, 1);
    }

    /// Wave-621 kill-XP drain fixture: unit A kills unit B through the
    /// damage-authority health channel. The lethal writeback enqueues
    /// destruction with no killer attached, so process_destroy_list's
    /// ready-log drain must resolve the killing blow's source from the
    /// victim's lastDamageInfo residual and credit A (C++ scores in
    /// ActiveBody::attemptDamage, ActiveBody.cpp:640-650, before onDie;
    /// GameLogic::processDestroyList, GameLogic.cpp:2445-2511, only tears
    /// down). Under authority-live, gain_experience still commits to the canonical
    /// host owner before subsequent observers.
    #[test]
    fn damage_authority_destroy_drain_awards_kill_experience() {
        let mut logic = GameLogic::new();

        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.alliance_team = 1;
        logic.add_player(usa);
        let mut gla = Player::new(2, Team::GLA, "GLA", true);
        gla.alliance_team = 2;
        logic.add_player(gla);

        let mut tank = ThingTemplate::new("DrainXpTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Score)
            .set_health(10.0);
        tank.build_cost.supplies = 500;
        tank.experience_value = 25.0;
        tank.experience_values = [25.0, 25.0, 25.0, 25.0];
        tank.skill_point_values = [25, 25, 25, 25];
        tank.is_trainable = true;
        logic.templates.insert("DrainXpTank".into(), tank);

        let killer = logic
            .create_object("DrainXpTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("killer");
        let victim = logic
            .create_object("DrainXpTank", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("victim");

        // Killing blow: host take_damage stamps last_damage_source and drives
        // HP lethal; it never enqueues destruction — the ready-log channel owns
        // that under damage authority.
        let destroyed = {
            let (v, health_events) = logic
                .host_object_and_health_events_mut(victim)
                .expect("victim");
            v.take_damage_from(1.0e9, Some(killer), health_events)
        };
        assert!(destroyed, "lethal blow must destroy the victim");
        assert_eq!(
            logic.host_object(victim).map(|o| o.last_damage_source),
            Some(Some(killer)),
            "killing blow source must be stamped for killer credit"
        );

        // Canonical lethal damage already queued this receiving owner for removal.

        logic.process_destroy_list();

        assert!(
            logic.host_object(victim).is_none(),
            "canonical lethal damage must destroy the victim"
        );
        assert_eq!(
            logic.host_object(killer).unwrap().experience.current,
            25.0,
            "kill awards the victim ExperienceValue to the canonical killer immediately"
        );
        assert_eq!(
            logic.get_player(0).unwrap().statistics.units_destroyed,
            1,
            "canonical kill must credit the killer team's score keeper"
        );
        assert_eq!(logic.get_player(2).unwrap().statistics.units_lost, 1);
        assert!(
            logic.get_player(0).unwrap().skill_points > 0,
            "C++ scoreTheKill adds SkillPointValue for the killer's owner"
        );
    }
}
