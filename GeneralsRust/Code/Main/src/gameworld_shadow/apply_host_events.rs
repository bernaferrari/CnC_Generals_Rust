//! Host upgrade/economy/production/spawn/queue/radar/contain apply batch.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    /// Host upgrade-complete residual: record completed research names on shadow players.
    /// Fail-closed: not full PlayerUpgradeManager effect matrix / science tree.
    pub fn apply_host_upgrade_events(
        &mut self,
        events: &[crate::game_logic::host_upgrades::HostUpgradeResearch],
    ) -> usize {
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut queued = 0usize;
        for ev in events {
            if ev.phase != HostUpgradePhase::Completed {
                continue;
            }
            let Some(&gw) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::CompleteUpgrade {
                player: gw,
                name: ev.name.clone(),
            });
            queued += 1;
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Apply drained host economy events as SetSupplies/SetPower mutations.
    pub fn apply_host_economy_events(
        &mut self,
        events: &[crate::game_logic::host_economy_log::HostEconomyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            let Some(&gw) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::SetSupplies {
                player: gw,
                supplies: ev.supplies,
            });
            self.world.queue_mutation(WorldMutation::SetPower {
                player: gw,
                power_available: ev.power_available,
            });
            queued += 2;
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    pub fn apply_host_production_events(
        &mut self,
        events: &[crate::game_logic::host_production_log::HostProductionEvent],
        logic: &GameLogic,
    ) -> usize {
        use crate::game_logic::host_production_log::HostProductionEvent;
        use gamelogic::world::entities::EntityProductionItem;
        let mut n = 0usize;
        let mut spawn_like = Vec::new();
        // Producers that need queue last-write from host snapshot.
        let mut enqueue_producers = std::collections::BTreeSet::new();
        for ev in events {
            match ev {
                HostProductionEvent::Enqueue { producer, .. } => {
                    enqueue_producers.insert(producer.0);
                }
                HostProductionEvent::Cancel { producer, .. } => {
                    // Wave 199: cancel refreshes producer queue from host snapshot.
                    enqueue_producers.insert(producer.0);
                    n += 1;
                }
                HostProductionEvent::Complete {
                    spawned,
                    template_name,
                    producer,
                } => {
                    enqueue_producers.insert(producer.0);
                    // Wave 483: upgrade complete uses spawned id 0 (queue refresh only).
                    if spawned.0 == 0 {
                        n += 1;
                        continue;
                    }
                    if self.host_to_entity.contains_key(&spawned.0) {
                        n += 1;
                        continue;
                    }
                    if let Some(obj) = logic.host_objects().get(spawned) {
                        let team_ord = match obj.team {
                            Team::USA => 0u8,
                            Team::China => 1,
                            Team::GLA => 2,
                            Team::Neutral => 255,
                        };
                        let pos = obj.get_position();
                        spawn_like.push(crate::game_logic::host_spawn_log::HostSpawnEvent {
                            id: *spawned,
                            template: template_name.clone(),
                            team_ordinal: team_ord,
                            position: [pos.x, pos.y, pos.z],
                        });
                    }
                }
            }
        }
        // Mutation-channel production queue last-writer from host building queues.
        for hid in enqueue_producers {
            let Some(eid) = self.host_to_entity.get(&hid).copied() else {
                continue;
            };
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let items: Vec<EntityProductionItem> = obj
                .building_data
                .as_ref()
                .map(|bd| {
                    bd.production_queue
                        .iter()
                        .take(16)
                        .map(|it| EntityProductionItem {
                            template_name: it.template_name.clone(),
                            progress: it.progress,
                            total_time: it.total_time,
                            construction_frames: it.construction_frames,
                            cost_supplies: it.cost.supplies,
                            is_upgrade: it.is_upgrade(),
                            quantity_total: it.quantity_total.max(1),
                            quantity_produced: it.quantity_produced,
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue {
                    target: eid,
                    items,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n + self.apply_host_spawn_events(&spawn_like, logic)
    }

    pub fn apply_host_production_progress_events(
        &mut self,
        events: &[crate::game_logic::host_production_progress_log::HostProductionProgressEvent],
    ) -> usize {
        use gamelogic::world::entities::EntityProductionItem;
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.producer.0) else {
                continue;
            };
            self.production_power_factor_by_host
                .insert(ev.producer.0, ev.power_factor.max(0.01));
            // Wave 477: sole-tick power-factor-only events must not stomp GW queue/exit.
            if ev.power_factor_only {
                n += 1;
                continue;
            }
            // Wave 480: post-spawn exit delay arm under sole-tick (no queue stomp).
            if ev.exit_delay_only {
                if ev.queue_exit_state_initialized {
                    self.world.queue_mutation(
                        gamelogic::world::WorldMutation::SetProductionExitRuntime {
                            target: eid,
                            exit_delay_remaining_frames: ev.exit_delay_remaining_frames,
                            exit_burst_remaining: ev.exit_burst_remaining,
                            queue_exit_state_initialized: true,
                        },
                    );
                } else {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetExitDelay {
                            target: eid,
                            exit_delay_remaining: ev.exit_delay_remaining,
                        });
                }
                n += 1;
                continue;
            }
            let items: Vec<EntityProductionItem> = ev
                .items
                .iter()
                .map(|it| EntityProductionItem {
                    template_name: it.template_name.clone(),
                    progress: it.progress,
                    total_time: it.total_time,
                    construction_frames: it.construction_frames,
                    cost_supplies: it.cost_supplies,
                    is_upgrade: it.is_upgrade,
                    quantity_total: it.quantity_total.max(1),
                    quantity_produced: it.quantity_produced,
                })
                .collect();
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue {
                    target: eid,
                    items,
                });
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetExitDelay {
                    target: eid,
                    exit_delay_remaining: ev.exit_delay_remaining,
                });
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionExitRuntime {
                    target: eid,
                    exit_delay_remaining_frames: ev.exit_delay_remaining_frames,
                    exit_burst_remaining: ev.exit_burst_remaining,
                    queue_exit_state_initialized: ev.queue_exit_state_initialized,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_production_door_events(
        &mut self,
        events: &[crate::game_logic::host_production_door_log::HostProductionDoorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.producer.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionDoor {
                    target: eid,
                    production_door_phase: ev.production_door_phase,
                    production_door_phase_end_frame: ev.production_door_phase_end_frame,
                    production_door_hold_open: ev.production_door_hold_open,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Host structure construction-complete residual: ensure completed buildings are
    /// mapped in the shadow (usually already present via sync; counts for probe honesty).
    /// Fail-closed: does not invent GameWorld construction modules.
    pub fn apply_host_construction_events(
        &mut self,
        events: &[crate::game_logic::host_construction_log::HostConstructionEvent],
        logic: &GameLogic,
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.host_to_entity.contains_key(&ev.id.0) {
                n += 1;
                continue;
            }
            // Completed structure missing from map — treat like a late spawn residual.
            if let Some(obj) = logic.host_objects().get(&ev.id) {
                if !obj.is_alive() {
                    continue;
                }
                let team_ordinal = match obj.team {
                    Team::USA => 0u8,
                    Team::China => 1,
                    Team::GLA => 2,
                    _ => 3,
                };
                let p = obj.get_position();
                let spawn = crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id: ev.id,
                    template: ev.template_name.clone(),
                    team_ordinal,
                    position: [p.x, p.y, p.z],
                };
                n += self.apply_host_spawn_events(std::slice::from_ref(&spawn), logic);
            }
        }
        n
    }

    pub fn apply_host_spawn_events(
        &mut self,
        events: &[crate::game_logic::host_spawn_log::HostSpawnEvent],
        logic: &GameLogic,
    ) -> usize {
        let mut spawned = 0usize;
        for ev in events {
            if self.host_to_entity.contains_key(&ev.id.0) {
                continue;
            }
            // Wave 736: if a production pre-spawn bind is pending for this event, map only.
            if let Some(raw) = take_next_host_spawn_bind_entity() {
                if bind_host_to_existing_entity(self, ev.id.0, raw) {
                    spawned += 1;
                    continue;
                }
                // Entity lost — fall through to normal Spawn residual.
            }
            let (health, owner) = if let Some(obj) = logic.host_objects().get(&ev.id) {
                let owner = self.owner_for_host_object(logic, obj);
                (obj.health.current.max(0.0), owner)
            } else {
                // A spawn log carries faction only. It cannot identify one
                // same-faction slot, so leave a missing host object unowned.
                (100.0, None)
            };
            // Mutation-channel spawn (sole create path) then map host ObjectId.
            self.world.queue_mutation(WorldMutation::Spawn {
                template: ev.template.clone(),
                owner,
                position: ev.position,
                health,
            });
            let _ = self.world.apply_pending_mutations();
            if let Some(eid) = self.world.take_last_spawned_entity() {
                self.host_to_entity.insert(ev.id.0, eid);
                self.entity_to_host.insert(eid.get(), ev.id.0);
                if crate::gameworld_shadow::gameworld_entity_modules_enabled() {
                    let spec = gamelogic::world::EntityModuleInstallSpec::default();
                    let _ = self.world.install_entity_modules(eid, &spec);
                }
                spawned += 1;
            }
        }
        spawned
    }

    /// Apply destroy-log events as WorldMutation::Destroy for mapped entities.

    pub fn apply_host_destroy_events(
        &mut self,
        events: &[crate::game_logic::host_destroy_log::HostDestroyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            if self.queue_destroy_for_host(ev.id) {
                queued += 1;
            }
        }
        let applied = self.apply_pending();
        let removed = self.world.process_destroy_list();
        if removed > 0 {
            self.invalidate_dead_entity_maps();
        }
        (queued, applied.max(removed))
    }

    /// Queue SetAttackTarget for a mapped host attacker.
    pub fn queue_set_attack_target_for_host(
        &mut self,
        host_attacker: ObjectId,
        host_target: Option<ObjectId>,
    ) -> bool {
        let Some(attacker) = self.entity_for_host(host_attacker) else {
            return false;
        };
        let target = host_target.and_then(|t| self.entity_for_host(t));
        self.world
            .queue_mutation(WorldMutation::SetAttackTarget { attacker, target });
        true
    }

    /// Queue borrow-first combat status residual onto a mapped host object.
    pub fn queue_set_combat_status_for_host(
        &mut self,
        ev: crate::game_logic::host_status_log::HostStatusEvent,
    ) -> bool {
        let Some(target) = self.entity_for_host(ev.object) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetCombatStatus {
                target,
                stealthed: ev.stealthed,
                detected: ev.detected,
                attacking: ev.attacking,
                moving: ev.moving,
                is_firing_weapon: ev.is_firing_weapon,
                is_aiming_weapon: ev.is_aiming_weapon,
                selected: ev.selected,
                disabled_emp: ev.disabled_emp,
                weapons_jammed: ev.weapons_jammed,
                disabled_hacked: ev.disabled_hacked,
                disabled_unmanned: ev.disabled_unmanned,
                disabled_paralyzed: ev.disabled_paralyzed,
                disabled_subdued: ev.disabled_subdued,
                masked: ev.masked,
                disguised: ev.disguised,
                no_collisions: ev.no_collisions,
                private_captured: ev.private_captured,
                disguise_transitioning_to: ev.disguise_transitioning_to,
                disguise_halfpoint_reached: ev.disguise_halfpoint_reached,
                faerie_fire: ev.faerie_fire,
                booby_trapped: ev.booby_trapped,
                eject_invulnerable: ev.eject_invulnerable,
                pilot_did_move_to_base: ev.pilot_did_move_to_base,
                parachuting: ev.parachuting,
                parachute_open: ev.parachute_open,
                parachute_landing_override_set: ev.parachute_landing_override_set,
                using_ability: ev.using_ability,
                deployed: ev.deployed,
                under_construction: ev.under_construction,
                sold: ev.sold,
                reconstructing: ev.reconstructing,
                unselectable: ev.unselectable,
                ignoring_stealth: ev.ignoring_stealth,
                repulsor: ev.repulsor,
                disabled_underpowered: ev.disabled_underpowered,
                disabled_freefall: ev.disabled_freefall,
                is_carbomb: ev.is_carbomb,
                hijacked: ev.hijacked,
                force_attack: ev.force_attack,
            });
        true
    }

    /// Queue SetVeterancy residual onto a mapped host object.
    pub fn queue_set_veterancy_for_host(&mut self, host: ObjectId, ordinal: u8) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetVeterancy {
                target,
                ordinal: ordinal.min(3),
            });
        true
    }

    /// Queue SetProductionQueue residual onto a mapped host producer.
    pub fn queue_set_production_queue_for_host(
        &mut self,
        host: ObjectId,
        items: Vec<gamelogic::world::entities::EntityProductionItem>,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue { target, items });
        true
    }

    /// Queue SetRallyPoint residual onto a mapped host structure (Wave 200).
    pub fn queue_set_rally_point_for_host(
        &mut self,
        host: ObjectId,
        position: Option<[f32; 3]>,
    ) -> bool {
        let Some(unit) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetRallyPoint { unit, position });
        true
    }

    /// Queue SetConstruction residual onto a mapped host structure.
    pub fn queue_set_construction_for_host(
        &mut self,
        host: ObjectId,
        percent: f32,
        under_construction: bool,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetConstruction {
                target,
                percent: percent.clamp(-1.0, 1.0),
                under_construction,
            });
        true
    }

    pub fn queue_set_special_power_for_host(
        &mut self,
        host_id: ObjectId,
        ready: bool,
        cooldown_remaining: f32,
        cooldown: f32,
    ) -> bool {
        let Some(&eid) = self.host_to_entity.get(&host_id.0) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetSpecialPower {
                target: eid,
                ready,
                cooldown_remaining,
                cooldown,
            });
        true
    }

    pub fn queue_set_ai_state_for_host(&mut self, host: ObjectId, ordinal: u8) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetAiState { target, ordinal });
        true
    }

    pub fn queue_set_contain_for_host(
        &mut self,
        host: ObjectId,
        contained_by_host: u32,
        garrison_count: Option<u16>,
        garrisoned_host_ids: Option<Vec<u32>>,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetContain {
                target,
                contained_by_host,
                garrison_count,
                garrisoned_host_ids,
            });
        true
    }

    pub fn queue_set_player_radar(
        &mut self,
        host_player_id: u32,
        radar_count: i32,
        radar_disabled: bool,
    ) -> bool {
        let Some(&player) = self.host_player_to_gw.get(&host_player_id) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetPlayerRadar {
                player,
                radar_count,
                radar_disabled,
            });
        true
    }

    pub fn apply_host_radar_events(
        &mut self,
        events: &[crate::game_logic::host_radar_log::HostRadarEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_player_radar(ev.player_id, ev.radar_count, ev.radar_disabled) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn queue_set_player_progress(
        &mut self,
        host_player_id: u32,
        rank_level: u32,
        skill_points: i32,
        science_purchase_points: i32,
        cash_bounty_percent: f32,
    ) -> bool {
        let Some(&player) = self.host_player_to_gw.get(&host_player_id) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetPlayerProgress {
                player,
                rank_level,
                skill_points,
                science_purchase_points,
                cash_bounty_percent,
            });
        true
    }

    pub fn apply_host_player_progress_events(
        &mut self,
        events: &[crate::game_logic::host_player_progress_log::HostPlayerProgressEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_player_progress(
                ev.player_id,
                ev.rank_level,
                ev.skill_points,
                ev.science_purchase_points,
                ev.cash_bounty_percent,
            ) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_player_meta_events(
        &mut self,
        events: &[crate::game_logic::host_player_meta_log::HostPlayerMetaEvent],
    ) -> usize {
        use crate::game_logic::host_player_meta_log::HostPlayerMetaEvent;
        let mut n = 0usize;
        for ev in events {
            match ev {
                HostPlayerMetaEvent::Sciences {
                    player_id,
                    unlocked_sciences,
                } => {
                    let Some(&player) = self.host_player_to_gw.get(player_id) else {
                        continue;
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetPlayerSciences {
                            player,
                            unlocked_sciences: unlocked_sciences.clone(),
                        });
                    n += 1;
                }
                HostPlayerMetaEvent::Alive {
                    player_id,
                    is_alive,
                } => {
                    let Some(&player) = self.host_player_to_gw.get(player_id) else {
                        continue;
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetPlayerAlive {
                            player,
                            is_alive: *is_alive,
                        });
                    n += 1;
                }
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_player_cooldown_events(
        &mut self,
        events: &[crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&player) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetPlayerCooldowns {
                    player,
                    cooldowns: ev.cooldowns.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_contain_events(
        &mut self,
        events: &[crate::game_logic::host_contain_log::HostContainEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_contain_for_host(
                ev.object,
                ev.contained_by_host,
                ev.garrison_count,
                ev.garrisoned_host_ids.clone(),
            ) {
                n += 1;
            }
            n = n.saturating_add(self.queue_contain_roster_mutations(ev));
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    fn queue_contain_roster_mutations(
        &mut self,
        ev: &crate::game_logic::host_contain_log::HostContainEvent,
    ) -> usize {
        let Some(target) = self.entity_for_host(ev.object) else {
            return 0;
        };
        let mut n = 0usize;
        if ev.contained_by_host == 0 {
            if let Some(container) = self.world.contain_roster().contained_by(target) {
                self.world
                    .queue_mutation(gamelogic::world::WorldMutation::ContainExit {
                        container,
                        occupant: target,
                    });
                n += 1;
            }
        } else if let Some(container) = self.entity_for_host(ObjectId(ev.contained_by_host)) {
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::ContainEnter {
                    container,
                    occupant: target,
                });
            n += 1;
        }
        if let Some(ids) = ev.garrisoned_host_ids.as_ref() {
            for hid in ids {
                let Some(occupant) = self.entity_for_host(ObjectId(*hid)) else {
                    continue;
                };
                self.world
                    .queue_mutation(gamelogic::world::WorldMutation::ContainEnter {
                        container: target,
                        occupant,
                    });
                n += 1;
            }
        }
        n
    }

    pub fn writeback_contain_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, u32, u32, bool, u16, u16)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_contain_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let mut did = false;
            let prev_cb = obj.contained_by.map(|c| c.0).unwrap_or(0);
            let new_cb = if ent.contained_by_host == 0 {
                None
            } else {
                Some(ObjectId(ent.contained_by_host))
            };
            let new_cb_u = new_cb.map(|c| c.0).unwrap_or(0);
            let mut prev_gcount = 0u16;
            let mut new_gcount = 0u16;
            let mut garrison_changed = false;
            if obj.contained_by != new_cb {
                obj.contained_by = new_cb;
                did = true;
            }
            if let Some(bd) = obj.building_data.as_mut() {
                prev_gcount = bd.garrisoned_units.len().min(u16::MAX as usize) as u16;
                let new_units: Vec<ObjectId> = ent
                    .garrisoned_host_ids
                    .iter()
                    .copied()
                    .map(ObjectId)
                    .collect();
                new_gcount = new_units.len().min(u16::MAX as usize) as u16;
                if bd.garrisoned_units != new_units {
                    bd.garrisoned_units = new_units;
                    garrison_changed = true;
                    did = true;
                }
            } else if !ent.garrisoned_host_ids.is_empty() || !obj.occupants.is_empty() {
                prev_gcount = obj.occupants.len().min(u16::MAX as usize) as u16;
                let new_occ: Vec<ObjectId> = ent
                    .garrisoned_host_ids
                    .iter()
                    .copied()
                    .map(ObjectId)
                    .collect();
                new_gcount = new_occ.len().min(u16::MAX as usize) as u16;
                if obj.occupants != new_occ {
                    obj.occupants = new_occ;
                    garrison_changed = true;
                    did = true;
                }
            }
            if did {
                // Wave 628: GameWorld contain membership last-write residual —
                // host applies AI/status counters from ready log.
                ready.push((
                    ObjectId(hid),
                    prev_cb,
                    new_cb_u,
                    garrison_changed,
                    prev_gcount,
                    new_gcount,
                ));
                updated += 1;
            }
        }
        for (oid, prev_cb, new_cb, gchg, prev_n, new_n) in ready {
            crate::game_logic::host_contain_ready_log::record(
                oid, prev_cb, new_cb, gchg, prev_n, new_n,
            );
        }
        updated
    }

    pub fn apply_host_ai_state_events(
        &mut self,
        events: &[crate::game_logic::host_ai_state_log::HostAiStateEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_ai_state_for_host(ev.object, ev.ordinal) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_ai_state_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, u8, u8)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Host AI-state log is an *input* to GameWorld (applied earlier in
            // the session). Pending leftovers must not block last-write —
            // C++ Object AI state is whatever the last update wrote
            // (AIUpdate.cpp state machine; no dual-world veto).
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_ord = Self::host_ai_state_ordinal(&obj.ai_state);
            if host_ord == ent.ai_state_ordinal {
                continue;
            }
            let prev = host_ord;
            let next = ent.ai_state_ordinal;
            // Wave 945: AI-state writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::AiState {
                id: ObjectId(hid),
                ordinal: next,
            }) {
                continue;
            }
            // Wave 630: GameWorld AI-state last-write residual —
            // host applies combat-status flags from ready log.
            ready.push((ObjectId(hid), prev, next));
            updated += 1;
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_ai_state_ready_log::record(oid, prev, next);
        }
        updated
    }

    pub fn queue_set_stored_supplies_for_host(&mut self, host: ObjectId, supplies: u32) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetStoredSupplies {
                target,
                supplies,
            });
        true
    }

    pub fn apply_host_special_power_events(
        &mut self,
        events: &[crate::game_logic::host_special_power_log::HostSpecialPowerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.special_power_frozen_by_host
                .insert(ev.object.0, ev.frozen);
            if self.queue_set_special_power_for_host(
                ev.object,
                ev.ready,
                ev.cooldown_remaining,
                ev.cooldown,
            ) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Under SPECIAL_POWER_AUTHORITY: advance entity SP cooldown remaining by dt.
    /// Host completes ready flip after writeback when remaining hits 0.
    pub fn tick_special_power_cooldowns(&mut self, dt: f32) -> usize {
        if !gameworld_special_power_sole_tick_enabled() || dt <= 0.0 {
            return 0;
        }
        use gamelogic::world::WorldMutation;
        use gamelogic::world::entities::EntityId;
        let mut n = 0usize;
        let mut updates: Vec<(EntityId, bool, f32, f32)> = Vec::new();
        let host_ids: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in host_ids {
            if self
                .special_power_frozen_by_host
                .get(&hid)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let rem = ent.special_power_cooldown_remaining;
            let cd = ent.special_power_cooldown;
            if rem <= 0.0 {
                continue;
            }
            let new_rem = (rem - dt).max(0.0);
            if (new_rem - rem).abs() < 1e-12 {
                continue;
            }
            let ready = new_rem <= 0.0;
            n += 1;
            updates.push((eid, ready, new_rem, cd));
        }
        for (eid, ready, rem, cd) in updates {
            self.world.queue_mutation(WorldMutation::SetSpecialPower {
                target: eid,
                ready,
                cooldown_remaining: rem,
                cooldown: cd,
            });
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }
    /// Under SPECIAL_POWER_AUTHORITY: advance GameWorld player shared SP cooldowns by dt.
    pub fn tick_player_shared_special_power_cooldowns(&mut self, dt: f32) -> usize {
        if !gameworld_special_power_sole_tick_enabled() || dt <= 0.0 {
            return 0;
        }
        use gamelogic::world::WorldMutation;
        let mut n = 0usize;
        // Snapshot player ids from host map.
        let players: Vec<_> = self.host_player_to_gw.values().copied().collect();
        for pid in players {
            let Some(pd) = self.world.player(pid) else {
                continue;
            };
            if pd.shared_special_power_cooldowns.is_empty() {
                continue;
            }
            let mut cds = pd.shared_special_power_cooldowns.clone();
            let mut dirty = false;
            for (_name, rem) in cds.iter_mut() {
                if *rem > 0.0 {
                    let next = (*rem - dt).max(0.0);
                    if (next - *rem).abs() > 1e-12 {
                        *rem = next;
                        dirty = true;
                    }
                }
            }
            // Drop zeros optional - keep keys for host parity
            if dirty {
                n += 1;
                self.world
                    .queue_mutation(WorldMutation::SetPlayerCooldowns {
                        player: pid,
                        cooldowns: cds,
                    });
            }
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }
    /// Last-write only shared SP cooldowns (does not touch cash/power).
    pub fn writeback_shared_special_power_cooldowns_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::command_system::SpecialPowerType;
        let mut updated = 0usize;
        for (&hid, &pid) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(pid) else {
                continue;
            };
            let Some(player) = logic.get_player_mut(hid) else {
                continue;
            };
            // Wave 760: under coupled tick, host cooldown log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_player_cooldown_log::has_pending(hid)
            {
                continue;
            }
            let mut next = std::collections::HashMap::new();
            for (hk, hv) in player.shared_special_power_cooldowns.iter() {
                let key = format!("{hk:?}");
                if let Some((_, rem)) = pd
                    .shared_special_power_cooldowns
                    .iter()
                    .find(|(k, _)| k == &key)
                {
                    next.insert(hk.clone(), *rem);
                } else {
                    next.insert(hk.clone(), *hv);
                }
            }
            for (sk, srem) in &pd.shared_special_power_cooldowns {
                let already = next.keys().any(|hk| format!("{hk:?}") == *sk);
                if already {
                    continue;
                }
                for c in [
                    SpecialPowerType::Airstrike,
                    SpecialPowerType::NuclearMissile,
                    SpecialPowerType::IonCannon,
                    SpecialPowerType::NapalmStrike,
                    SpecialPowerType::Paradrop,
                    SpecialPowerType::EmergencyRepair,
                    SpecialPowerType::CarpetBomb,
                ] {
                    if format!("{c:?}") == *sk {
                        next.insert(c, *srem);
                        break;
                    }
                }
            }
            if next != player.shared_special_power_cooldowns {
                player.shared_special_power_cooldowns = next;
                updated += 1;
            }
        }
        updated
    }

    pub fn writeback_special_power_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_special_power_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let was_ready = obj.special_power_ready;
            let changed = obj.special_power_ready != ent.special_power_ready
                || (obj.special_power_cooldown_remaining - ent.special_power_cooldown_remaining)
                    .abs()
                    > 1e-4
                || (obj.special_power_cooldown - ent.special_power_cooldown).abs() > 1e-4;
            if !changed {
                continue;
            }
            // Wave 945: special-power writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::SpecialPower {
                id: ObjectId(hid),
                ready: ent.special_power_ready,
                cooldown_remaining: ent.special_power_cooldown_remaining,
                cooldown: ent.special_power_cooldown,
            }) {
                continue;
            }
            // Wave 618: GameWorld sole-tick SP ready residual — host EVA/UI can drain.
            if crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled()
                && !was_ready
                && ent.special_power_ready
            {
                crate::game_logic::host_special_power_ready_log::record(
                    ObjectId(hid),
                    ent.special_power_cooldown_remaining.max(0.0),
                );
            }
            updated += 1;
        }
        updated
    }

    pub fn apply_host_stored_supplies_events(
        &mut self,
        events: &[crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_stored_supplies_for_host(ev.object, ev.supplies) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_stored_supplies_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, u32, u32)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_stored_supplies_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.stored_resources.supplies == ent.stored_supplies {
                continue;
            }
            let prev = obj.stored_resources.supplies;
            // Wave 945: stored-supplies writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::StoredSupplies {
                id: ObjectId(hid),
                supplies: ent.stored_supplies,
            }) {
                continue;
            }
            // Wave 641: GameWorld stored-supplies last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push((ObjectId(hid), prev, ent.stored_supplies));
            updated += 1;
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_stored_supplies_ready_log::record(oid, prev, next);
        }
        updated
    }

    /// Apply construction progress log as SetConstruction mutations.
    pub fn apply_host_construction_progress_events(
        &mut self,
        events: &[crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.construction_rate_by_host
                .insert(ev.object.0, ev.effective_rate);
            // Wave 478: sole-tick rate-only events must not stomp GW construction percent.
            if ev.rate_only {
                n += 1;
                continue;
            }
            if self.queue_set_construction_for_host(ev.object, ev.percent, ev.under_construction) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Queue SetTransform for a mapped host object (move-command channel).
    pub fn queue_set_transform_for_host(
        &mut self,
        host: ObjectId,
        position: [f32; 3],
        orientation: f32,
    ) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::SetTransform {
            target: eid,
            position,
            orientation,
        });
        true
    }

    /// Sync host Object::target onto shadow via SetAttackTarget mutations.
    pub fn apply_host_attack_targets(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if self.queue_set_attack_target_for_host(ObjectId(hid), obj.target) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Push current host positions onto shadow via SetTransform mutations.
    pub fn apply_host_positions_as_transforms(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let pos = obj.get_position();
            let orient = obj.get_orientation();
            if self.queue_set_transform_for_host(ObjectId(hid), [pos.x, pos.y, pos.z], orient) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }
}
