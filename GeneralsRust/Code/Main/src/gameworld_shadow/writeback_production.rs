//! Production/door/body/death and related host writebacks.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, ProductionExitRuntimeState, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn writeback_production_to_host(&mut self, logic: &mut GameLogic) -> usize {
        if !gameworld_production_authority_enabled() {
            return 0;
        }
        use crate::game_logic::{ProductionItem, ProductionKind, Resources};
        use gamelogic::world::WorldMutation;
        let mut updated = 0usize;
        // Wave 736: (host_id, template, is_upgrade, spawn_pos, rally, owner, health)
        let mut sole_ready_intents: Vec<(
            u32,
            String,
            bool,
            Option<[f32; 3]>,
            Option<[f32; 3]>,
            Option<PlayerId>,
            f32,
        )> = Vec::new();
        let host_frame = logic.get_frame();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let production_power_factor = self
                .production_power_factor_by_host
                .get(&hid)
                .copied()
                .unwrap_or(1.0)
                .max(0.01);
            let Some(obj) = /* Wave 946/947 */ logic./* Wave 950 */ host_object_mut(ObjectId(hid)) else {
                continue;
            };
            let exit_metadata = obj.thing.template.production_exit_metadata;
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_production_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            // Keep the short `BuildingData` borrow separate from the door
            // transition below.  Production queue progress is shadow-owned on
            // a coupled frame, while C++ `ProductionUpdate` still owns the
            // door animation and its model-condition side effects.
            let completed_head = {
                let Some(bd) = obj.building_data.as_mut() else {
                    continue;
                };
                let mut dirty = false;
                // Rally last-writer.
                let rally = ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
                if bd.rally_point != rally {
                    bd.rally_point = rally;
                    dirty = true;
                }
                // Production queue residual (template/progress/cost/upgrade).
                let new_q: Vec<ProductionItem> = ent
                    .production_queue_items
                    .iter()
                    .map(|it| ProductionItem {
                        template_name: it.template_name.clone(),
                        progress: it.progress,
                        total_time: it.total_time,
                        construction_frames: it.construction_frames,
                        cost: Resources {
                            supplies: it.cost_supplies,
                            power: 0,
                        },
                        // Wave 463: preserve C++ production quantity residual through GW writeback.
                        quantity_total: it.quantity_total.max(1),
                        quantity_produced: it.quantity_produced,
                        kind: if it.is_upgrade {
                            ProductionKind::Upgrade
                        } else {
                            ProductionKind::Unit
                        },
                    })
                    .collect();
                let queue_differs = bd.production_queue.len() != new_q.len()
                    || bd.production_queue.iter().zip(new_q.iter()).any(|(a, b)| {
                        a.template_name != b.template_name
                            || (a.progress - b.progress).abs() > 1e-5
                            || (a.total_time - b.total_time).abs() > 1e-5
                            || a.construction_frames != b.construction_frames
                            || a.cost.supplies != b.cost.supplies
                            || a.kind != b.kind
                            || a.quantity_total != b.quantity_total
                            || a.quantity_produced != b.quantity_produced
                    });
                if queue_differs {
                    bd.production_queue = new_q;
                    dirty = true;
                }
                // Wave 990: production_paused residual last-writer (GameWorld ↔ host).
                if bd.production_paused != ent.production_paused {
                    bd.production_paused = ent.production_paused;
                    dirty = true;
                }
                // Parsed Queue state is integer C++ authority.  Float-only
                // entities remain backwards-compatible legacy producers.
                let entity_exit_state = ProductionExitRuntimeState {
                    delay_frames: ent.exit_delay_remaining_frames,
                    burst_remaining: ent.exit_burst_remaining,
                    initialized: ent.queue_exit_state_initialized,
                };
                if entity_exit_state.initialized
                    && bd.production_exit_runtime_state() != entity_exit_state
                {
                    bd.set_production_exit_runtime_state(entity_exit_state);
                    dirty = true;
                } else if !entity_exit_state.initialized
                    && (bd.exit_delay_remaining - ent.exit_delay_remaining).abs() > 1e-5
                {
                    bd.exit_delay_remaining = ent.exit_delay_remaining.max(0.0);
                    dirty = true;
                }
                // C++ skips ProductionUpdate while DISABLED_UNDERPOWERED (HELD
                // is the only process-mask exception). Do not emit ready/spawn.
                let production_frozen = ent.disabled_underpowered && !ent.disabled_held;
                let completed = if crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
                    && !production_frozen
                {
                    (bd.production_head_complete_at_power(production_power_factor)
                        && bd.production_head_exit_available(exit_metadata.as_ref()))
                    .then(|| {
                        let (template, is_upgrade, remaining) =
                            bd.production_queue.first().map(|head| {
                                (
                                    head.template_name.clone(),
                                    head.is_upgrade(),
                                    head.remaining_quantity(),
                                )
                            })?;
                        // C++ ProductionUpdate loops every remaining unit
                        // only while this exact interface reserves it.
                        let release_quantity = if is_upgrade {
                            1
                        } else {
                            bd.production_exit_release_limit(exit_metadata.as_ref(), remaining)
                        };
                        Some((template, is_upgrade, release_quantity))
                    })
                    .flatten()
                } else {
                    None
                };
                if dirty {
                    updated += 1;
                }
                completed
            };

            let Some((template, is_upgrade, release_quantity)) = completed_head else {
                continue;
            };
            if release_quantity == 0 {
                // Normal host completion removes a fully-produced entry.  Do
                // not manufacture a ready event from a stale/corrupt mirror.
                continue;
            }
            let door_count = crate::game_logic::host_production_buildable_command_residual::producer_num_door_animations(
                &obj.template_name,
            );
            // C++ `ProductionUpdate` routes only PRODUCTION_UNIT entries
            // through ExitInterface / door animation.  Research completes
            // immediately once its frame threshold is reached.
            if !is_upgrade
                && !crate::game_logic::host_production_buildable_command_residual::production_door_allows_spawn(
                    door_count,
                    obj.production_door_phase,
                )
            {
                // The completed head is retained until C++ ProductionUpdate has
                // opened its exit door.  Starting that animation here avoids a
                // speculative GameWorld spawn whose ready event the host must
                // defer while the door is closed.
                if obj.production_door_phase == 0 {
                    obj.start_production_door_cycle(host_frame);
                    updated += 1;
                }
                continue;
            }

            // Wave 614: GameWorld sole-tick ready residual — finished heads
            // (progress complete + exit delay clear) are recorded for host collect.
            // Wave 735: GW spawn pose + rally ride the ready event.
            // Wave 736: collect sole-tick ready intents (pose + entity-first spawn
            // data). Entity Spawn + ready-log record happen after this loop so we
            // do not mutably borrow GameWorld while iterating host maps.
            let p = ent.transform.position;
            let yaw = ent.transform.orientation;
            let radius = ent.selection_radius.max(10.0);
            let spawn_pos = if let Some(exit) = exit_metadata {
                let forward = glam::Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let pos = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                    glam::Vec3::new(p.x, p.y, p.z),
                    forward,
                    (
                        exit.unit_create_point[0],
                        exit.unit_create_point[1],
                        exit.unit_create_point[2],
                    ),
                );
                [pos.x, pos.y, pos.z]
            } else {
                [p.x + yaw.cos() * radius, p.y, p.z + yaw.sin() * radius]
            };
            for _ in 0..release_quantity {
                sole_ready_intents.push((
                    hid,
                    template.clone(),
                    is_upgrade,
                    if is_upgrade { None } else { Some(spawn_pos) },
                    ent.rally_point,
                    ent.owner,
                    obj.health.maximum.max(1.0),
                ));
            }
        }
        // Wave 736: entity-first production spawn + ready-log after host queue writeback.
        for (hid, template, is_upgrade, spawn_pos, rally, owner, health) in sole_ready_intents {
            let gw_entity_raw = if is_upgrade {
                None
            } else if let Some(pos) = spawn_pos {
                self.world.queue_mutation(WorldMutation::Spawn {
                    template: template.clone(),
                    owner,
                    position: pos,
                    health,
                });
                let _ = self.world.apply_pending_mutations();
                self.world.take_last_spawned_entity().map(|eid| eid.get())
            } else {
                None
            };
            crate::game_logic::host_production_ready_log::record_with_pose(
                ObjectId(hid),
                template,
                is_upgrade,
                spawn_pos,
                rally,
                gw_entity_raw,
            );
        }
        updated
    }

    pub fn writeback_production_door_to_host(&self, logic: &mut GameLogic) -> usize {
        // Host `updateDoors` is skipped under production sole-tick (Wave 743).
        // GameWorld then last-writes phase/end/hold. When sole-tick is off,
        // a coupled host still owns the door and writeback must not stomp it.
        if shadow_coupled_tick_active() && !gameworld_production_sole_tick_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        let mut transitions: Vec<(ObjectId, u8, u8, u32, bool)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_production_door_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let prev_phase = obj.production_door_phase;
            let changed = obj.production_door_phase != ent.production_door_phase
                || obj.production_door_phase_end_frame != ent.production_door_phase_end_frame
                || obj.production_door_hold_open != ent.production_door_hold_open;
            if !changed {
                continue;
            }
            obj.production_door_phase = ent.production_door_phase;
            obj.production_door_phase_end_frame = ent.production_door_phase_end_frame;
            obj.production_door_hold_open = ent.production_door_hold_open;
            // Wave 627: GameWorld production-door phase last-write residual —
            // host applies door model bits from ready log on phase change.
            if prev_phase != obj.production_door_phase {
                transitions.push((
                    ObjectId(hid),
                    prev_phase,
                    obj.production_door_phase,
                    obj.production_door_phase_end_frame,
                    obj.production_door_hold_open,
                ));
            }
            updated += 1;
        }
        for (oid, prev, next, end, hold) in transitions {
            crate::game_logic::host_production_door_ready_log::record(oid, prev, next, end, hold);
        }
        updated
    }

    /// Write GameWorld BodyDamageType residual onto host objects.
    pub fn writeback_body_damage_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut updated = 0usize;
        let mut transitions: Vec<(ObjectId, u8, u8)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_body_damage_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let want = HostBodyDamageType::from_ordinal(ent.body_damage_state);
            if obj.body_damage_state != want {
                let prev_ord = obj.body_damage_state.ordinal();
                let new_ord = want.ordinal();
                // Wave 945: body-damage writeback via host writeback authority.
                if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::BodyDamage {
                    id: ObjectId(hid),
                    state: want,
                }) {
                    continue;
                }
                // Wave 623: GameWorld body-damage last-write residual —
                // host applies model/FX side effects from ready log.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    transitions.push((ObjectId(hid), prev_ord, new_ord));
                }
                updated += 1;
            }
        }
        for (oid, prev, next) in transitions {
            crate::game_logic::host_body_damage_ready_log::record(oid, prev, next);
        }
        updated
    }

    pub fn writeback_death_type_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, u8, u8)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_death_type_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let want = HostDeathType::from_ordinal(ent.death_type);
            if obj.status.death_type != want {
                let prev = obj.status.death_type.ordinal();
                let next = want.ordinal();
                // Wave 945: death-type writeback via host writeback authority.
                if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::DeathType {
                    id: ObjectId(hid),
                    death_type: want,
                }) {
                    continue;
                }
                // Wave 632: GameWorld death-type last-write residual —
                // host applies destroy/pilot bookkeeping from ready log.
                ready.push((ObjectId(hid), prev, next));
                updated += 1;
            }
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_death_type_ready_log::record(oid, prev, next);
        }
        updated
    }

    pub fn writeback_radar_extend_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut completed: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_radar_extend_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let was_complete = obj.radar_extend_complete;
            let changed = obj.radar_extend_done_frame != ent.radar_extend_done_frame
                || obj.radar_extend_complete != ent.radar_extend_complete
                || obj.radar_active != ent.radar_active;
            if !changed {
                continue;
            }
            obj.radar_extend_done_frame = ent.radar_extend_done_frame;
            obj.radar_extend_complete = ent.radar_extend_complete;
            obj.radar_active = ent.radar_active;
            // Wave 625: GameWorld radar-extend complete residual —
            // host applies upgraded model bits / complete counter from ready log.
            if !was_complete && obj.radar_extend_complete {
                completed.push(ObjectId(hid));
            }
            updated += 1;
        }
        for oid in completed {
            crate::game_logic::host_radar_extend_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_shock_stun_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 756: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_shock_stun_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.shock_stun_frames != ent.shock_stun_frames
                || (obj.shock_yaw_rate - ent.shock_yaw_rate).abs() > f32::EPSILON
                || (obj.shock_pitch_rate - ent.shock_pitch_rate).abs() > f32::EPSILON
                || (obj.shock_roll_rate - ent.shock_roll_rate).abs() > f32::EPSILON
                || (obj.shock_up_z - ent.shock_up_z).abs() > f32::EPSILON
                || obj.shock_allow_bounce != ent.shock_allow_bounce
                || obj.shock_grounded_once != ent.shock_grounded_once
                || obj.shock_was_airborne != ent.shock_was_airborne
                || obj.cell_is_cliff != ent.cell_is_cliff
                || obj.cell_is_underwater != ent.cell_is_underwater;
            if !changed {
                continue;
            }
            obj.shock_stun_frames = ent.shock_stun_frames;
            obj.shock_yaw_rate = ent.shock_yaw_rate;
            obj.shock_pitch_rate = ent.shock_pitch_rate;
            obj.shock_roll_rate = ent.shock_roll_rate;
            obj.shock_up_z = ent.shock_up_z;
            obj.shock_allow_bounce = ent.shock_allow_bounce;
            obj.shock_grounded_once = ent.shock_grounded_once;
            obj.shock_was_airborne = ent.shock_was_airborne;
            obj.cell_is_cliff = ent.cell_is_cliff;
            obj.cell_is_underwater = ent.cell_is_underwater;
            // Wave 662: GameWorld shock-stun last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_shock_stun_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_rebuild_producer_to_host(&mut self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let host_frame = logic.get_frame();
        use gamelogic::world::WorldMutation;
        // Wave 740: (hole_hid, ready_frame, template, pos, orient, owner, health)
        let mut sole_ready_intents: Vec<(u32, u32, String, [f32; 3], f32, Option<PlayerId>, f32)> =
            Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 759: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_rebuild_producer_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_tpl = obj.rebuild_template_name.clone().unwrap_or_default();
            let changed = obj.is_rebuild_hole != ent.is_rebuild_hole
                || host_tpl != ent.rebuild_template_name
                || obj.rebuild_ready_frame != ent.rebuild_ready_frame
                || obj.rebuild_spawner_id.map(|id| id.0) != ent.rebuild_spawner_id
                || obj.rebuild_worker_id.map(|id| id.0) != ent.rebuild_worker_id
                || obj.rebuild_reconstructing_id.map(|id| id.0) != ent.rebuild_reconstructing_id
                || obj.producer_id.map(|id| id.0) != ent.producer_id
                || obj.construction_complete_clear_frame != ent.construction_complete_clear_frame;
            if changed {
                obj.is_rebuild_hole = ent.is_rebuild_hole;
                obj.rebuild_template_name = if ent.rebuild_template_name.is_empty() {
                    None
                } else {
                    Some(ent.rebuild_template_name.clone())
                };
                obj.rebuild_ready_frame = ent.rebuild_ready_frame;
                obj.rebuild_spawner_id = ent.rebuild_spawner_id.map(ObjectId);
                obj.rebuild_worker_id = ent.rebuild_worker_id.map(ObjectId);
                obj.rebuild_reconstructing_id = ent.rebuild_reconstructing_id.map(ObjectId);
                obj.producer_id = ent.producer_id.map(ObjectId);
                obj.construction_complete_clear_frame = ent.construction_complete_clear_frame;
                updated += 1;
            }
            // Wave 620: GameWorld sole-tick rebuild-ready residual —
            // hole ready when ready_frame reached and not already reconstructing.
            // Record every coupled frame (not only when fields dirty) so host can drain.
            if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
                let ready_frame = obj.rebuild_ready_frame;
                if obj.is_rebuild_hole
                    && obj.rebuild_reconstructing_id.is_none()
                    && ready_frame > 0
                    && host_frame >= ready_frame
                {
                    // Wave 740: collect intent; entity pre-spawn after loop.
                    let p = ent.transform.position;
                    let tpl = if !ent.rebuild_template_name.is_empty() {
                        ent.rebuild_template_name.clone()
                    } else {
                        obj.rebuild_template_name.clone().unwrap_or_default()
                    };
                    if !tpl.is_empty() {
                        sole_ready_intents.push((
                            hid,
                            ready_frame,
                            tpl,
                            [p.x, p.y, p.z],
                            ent.transform.orientation,
                            ent.owner,
                            obj.health.maximum.max(1.0),
                        ));
                    } else {
                        crate::game_logic::host_rebuild_ready_log::record(
                            ObjectId(hid),
                            ready_frame,
                        );
                    }
                }
                // Wave 626: construction-complete clear deadline elapsed residual.
                let clear_at = obj.construction_complete_clear_frame;
                if clear_at > 0 && host_frame >= clear_at {
                    crate::game_logic::host_construction_complete_clear_ready_log::record(
                        ObjectId(hid),
                        clear_at,
                    );
                }
            }
        }
        // Wave 740: entity-first worker + reconstruct pre-spawn under construction sole-tick.
        for (hid, ready_frame, template, spawn_pos, orientation, owner, health) in
            sole_ready_intents
        {
            // Worker entity. C++ FactionBuilding.ini WorkerObjectName = GLAInfantryWorker.
            self.world.queue_mutation(WorldMutation::Spawn {
                template: "GLAInfantryWorker".to_string(),
                owner,
                position: spawn_pos,
                health: 100.0_f32.max(1.0),
            });
            let _ = self.world.apply_pending_mutations();
            let worker_raw = self.world.take_last_spawned_entity().map(|eid| eid.get());
            // Reconstructing structure entity.
            self.world.queue_mutation(WorldMutation::Spawn {
                template: template.clone(),
                owner,
                position: spawn_pos,
                health,
            });
            let _ = self.world.apply_pending_mutations();
            let rebuild_raw = self.world.take_last_spawned_entity().map(|eid| eid.get());
            // Stamp orientation on entities when present.
            if let Some(raw) = worker_raw {
                use gamelogic::world::entities::EntityId;
                if let Some(e) = self.world.world_mut().entity_mut(EntityId::from_raw(raw)) {
                    e.transform.orientation = orientation;
                }
            }
            if let Some(raw) = rebuild_raw {
                use gamelogic::world::entities::EntityId;
                if let Some(e) = self.world.world_mut().entity_mut(EntityId::from_raw(raw)) {
                    e.transform.orientation = orientation;
                    e.construction_percent = 0.0;
                }
            }
            crate::game_logic::host_rebuild_ready_log::record_with_entities(
                ObjectId(hid),
                ready_frame,
                worker_raw,
                rebuild_raw,
                Some(spawn_pos),
                orientation,
                template,
            );
        }
        updated
    }

    pub fn writeback_sole_healing_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_heal_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let changed = obj.sole_healing_benefactor.map(|id| id.0)
                != ent.sole_healing_benefactor_id
                || obj.sole_healing_benefactor_expiration_frame
                    != ent.sole_healing_benefactor_expiration_frame;
            if !changed {
                continue;
            }
            obj.sole_healing_benefactor = ent.sole_healing_benefactor_id.map(ObjectId);
            obj.sole_healing_benefactor_expiration_frame =
                ent.sole_healing_benefactor_expiration_frame;
            // Wave 663: GameWorld sole-healing last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_sole_healing_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_ai_mood_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 757: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_ai_mood_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_prio = obj.attack_priority_set.clone().unwrap_or_default();
            let changed = obj.idle_since_frame != ent.idle_since_frame
                || obj.mood_attack_check_rate != ent.mood_attack_check_rate
                || obj.auto_acquire_when_idle != ent.auto_acquire_when_idle
                || host_prio != ent.attack_priority_set;
            if !changed {
                continue;
            }
            obj.idle_since_frame = ent.idle_since_frame;
            obj.mood_attack_check_rate = ent.mood_attack_check_rate;
            obj.auto_acquire_when_idle = ent.auto_acquire_when_idle;
            obj.attack_priority_set = if ent.attack_priority_set.is_empty() {
                None
            } else {
                Some(ent.attack_priority_set.clone())
            };
            // Wave 645: GameWorld AI-mood last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_ai_mood_ready_log::record(oid);
        }
        updated
    }

    pub fn writeback_ai_request_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_ai_request_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let host_victim = obj.requested_victim_id.map(|id| id.0);
            let host_dest = obj.requested_destination.map(|p| [p.x, p.y, p.z]);
            let host_prev = obj.prev_victim_pos.map(|p| [p.x, p.y, p.z]);
            let host_crate = obj.crate_created.map(|id| id.0).unwrap_or(0);
            let host_ret_v = obj.guard_retaliate_victim.map(|id| id.0).unwrap_or(0);
            let host_ret_a = obj.guard_retaliate_anchor.map(|p| [p.x, p.y, p.z]);
            let host_pending_tpl = obj.disguise_pending_template.clone().unwrap_or_default();
            let host_pending_team = obj
                .disguise_pending_team
                .map(|t| match t {
                    Team::USA => 0u8,
                    Team::China => 1u8,
                    Team::GLA => 2u8,
                    Team::Neutral => 3u8,
                })
                .unwrap_or(255u8);
            let changed = host_victim != ent.requested_victim_id
                || host_dest != ent.requested_destination
                || host_prev != ent.prev_victim_pos
                || host_crate != ent.crate_created_host
                || host_ret_v != ent.guard_retaliate_victim_host
                || host_ret_a != ent.guard_retaliate_anchor
                || obj.path_timestamp != ent.path_timestamp
                || host_pending_tpl != ent.disguise_pending_template
                || host_pending_team != ent.disguise_pending_team_ordinal
                || obj.weapon_crate_upgrade != ent.weapon_crate_upgrade
                || obj.armor_crate_upgrade != ent.armor_crate_upgrade
                || obj.selection_flash_remaining != ent.selection_flash_remaining;
            if !changed {
                continue;
            }
            obj.requested_victim_id = ent.requested_victim_id.map(ObjectId);
            obj.requested_destination = ent
                .requested_destination
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.prev_victim_pos = ent
                .prev_victim_pos
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.crate_created = if ent.crate_created_host == 0 {
                None
            } else {
                Some(ObjectId(ent.crate_created_host))
            };
            obj.guard_retaliate_victim = if ent.guard_retaliate_victim_host == 0 {
                None
            } else {
                Some(ObjectId(ent.guard_retaliate_victim_host))
            };
            obj.guard_retaliate_anchor = ent
                .guard_retaliate_anchor
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.path_timestamp = ent.path_timestamp;
            obj.disguise_pending_template = if ent.disguise_pending_template.is_empty() {
                None
            } else {
                Some(ent.disguise_pending_template.clone())
            };
            obj.disguise_pending_team = match ent.disguise_pending_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                3 => Some(Team::Neutral),
                _ => None,
            };
            obj.weapon_crate_upgrade = ent.weapon_crate_upgrade;
            obj.armor_crate_upgrade = ent.armor_crate_upgrade;
            obj.selection_flash_remaining = ent.selection_flash_remaining;
            // Wave 648: GameWorld AI-request last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_ai_request_ready_log::record(oid);
        }
        updated
    }

    /// Write shadow entity owner last-writer onto host object team.
    pub fn writeback_owner_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, crate::game_logic::Team, crate::game_logic::Team)> =
            Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let (want_owner_player_id, want_team) = match ent.owner {
                None => (None, crate::game_logic::Team::Neutral),
                Some(_) => {
                    let Some(player_id) = self.host_player_for_gw_owner(ent.owner) else {
                        continue;
                    };
                    let Some(team) = logic
                        .get_player(player_id)
                        .filter(|player| player.is_alive)
                        .map(|player| player.team)
                    else {
                        continue;
                    };
                    (Some(player_id), team)
                }
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_owner_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.team != want_team || obj.owner_player_id != want_owner_player_id {
                let prev = obj.team;
                let team_changed = prev != want_team;
                if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Owner {
                    id: ObjectId(hid),
                    team: want_team,
                    team_color: want_team.get_color(),
                    owner_player_id: want_owner_player_id,
                }) {
                    continue;
                }
                // Wave 629: GameWorld owner last-write residual —
                // host applies capture side effects from ready log.
                if team_changed {
                    ready.push((ObjectId(hid), prev, want_team));
                }
                updated += 1;
            }
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_owner_ready_log::record(oid, prev, next);
        }
        updated
    }

    /// Write shadow construction/status residual last-writer onto host objects.

    /// Under CONSTRUCTION_AUTHORITY: advance entity construction_percent by rate*dt.
    /// Host completes when writeback reaches 1.0 (or sell finish).
    pub fn tick_construction_progress(&mut self, dt: f32) -> usize {
        // GameWorld advances whenever construction authority is on. Host sole-tick
        // freeze is a separate coupled-frame gate (`gameworld_construction_sole_tick_enabled`).
        if !gameworld_construction_authority_enabled() {
            return 0;
        }
        use gamelogic::world::WorldMutation;
        let mut n = 0usize;
        let mut updates: Vec<(gamelogic::world::entities::EntityId, f32, bool)> = Vec::new();
        let host_ids: Vec<(u32, gamelogic::world::entities::EntityId)> = self
            .host_to_entity
            .iter()
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in host_ids {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let rate = self
                .construction_rate_by_host
                .get(&hid)
                .copied()
                .unwrap_or(0.0);
            if rate.abs() < 1e-12 {
                continue;
            }
            // Positive rate is host "this object is being built" even if the
            // entity UC bit was never stamped (rate-only progress events skip
            // SetConstruction). Without this, barracks stay unfinished forever.
            // Do not keep mutating already-complete buildings every frame.
            if rate > 0.0 && ent.construction_percent + 1e-6 >= 1.0 && !ent.under_construction {
                continue;
            }
            let mut pct = ent.construction_percent.max(0.0);
            let uc = ent.under_construction || rate > 0.0;
            if rate > 0.0 {
                pct = (pct + rate * dt).min(1.0);
            } else if rate < 0.0 {
                pct = (pct + rate * dt).max(-1.0);
            } else {
                continue;
            }
            if (pct - ent.construction_percent).abs() > 1e-8 {
                n += 1;
                updates.push((eid, pct, uc));
            }
        }
        for (eid, pct, uc) in updates {
            self.world.queue_mutation(WorldMutation::SetConstruction {
                target: eid,
                percent: pct,
                under_construction: uc,
            });
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }

    pub fn writeback_construction_to_host(&self, logic: &mut GameLogic) -> usize {
        // Construction/sell/rebuild residual only.
        // Combat status, AI state, contain, supplies, veterancy, and special-power
        // last-writer residuals use dedicated writebacks in the shadow session.
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_construction_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let mut dirty = false;
            // Sell deconstruction uses negative percent (finish <= -0.5); do not floor at 0.
            let pct = ent.construction_percent.clamp(-1.0, 1.0);
            if (obj.construction_percent - pct).abs() > 1e-5 {
                obj.construction_percent = pct;
                dirty = true;
            }
            if obj.status.under_construction != ent.under_construction {
                obj.status.under_construction = ent.under_construction;
                dirty = true;
            }
            if obj.status.sold != ent.sold {
                obj.status.sold = ent.sold;
                dirty = true;
            }
            if obj.status.reconstructing != ent.reconstructing {
                obj.status.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.status.unselectable != ent.unselectable {
                obj.status.unselectable = ent.unselectable;
                dirty = true;
            }
            // Wave 617: GameWorld sole-tick construction-ready residual —
            // finished builds (percent>=1, still under_construction) for host complete.
            // Wave 619: sell-finish ready residual (sold + percent <= -0.5).
            if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
                if obj.status.under_construction && pct + 1e-6 >= 1.0 {
                    crate::game_logic::host_construction_ready_log::record(ObjectId(hid), pct);
                }
                // SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL = -0.5
                if obj.status.sold && pct <= -0.5 + 1e-6 {
                    crate::game_logic::host_sell_ready_log::record(ObjectId(hid), pct);
                }
            }
            if dirty {
                updated += 1;
            }
        }
        updated
    }
}
