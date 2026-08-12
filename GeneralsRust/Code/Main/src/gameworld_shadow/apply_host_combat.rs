//! Host damage/heal/combat/projectile/AI decision apply batch.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn queue_damage_for_host(&mut self, host: ObjectId, amount: f32) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::Damage {
            target: eid,
            amount,
        });
        true
    }

    /// Apply drained host damage events as GameWorld mutations (order preserved).
    /// Returns (queued, applied_after_flush).
    pub fn queue_transfer_owner_for_host(
        &mut self,
        host: ObjectId,
        owner: Option<gamelogic::world::PlayerId>,
    ) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::TransferOwner {
                object: eid,
                player: owner,
            });
        true
    }

    pub fn apply_host_owner_events(
        &mut self,
        logic: &GameLogic,
        events: &[crate::game_logic::host_owner_log::HostOwnerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            // An event that carried exact provenance must retain that player
            // even if the host object changes again before this batch drains.
            // Team-only legacy events deliberately use the live object, whose
            // `set_team` path clears player provenance.
            let owner = match ev.owner_player_id {
                Some(player_id) => self.host_player_to_gw.get(&player_id).copied(),
                None => logic
                    .host_object(ev.object)
                    .and_then(|object| self.owner_for_host_object(logic, object)),
            };
            if self.queue_transfer_owner_for_host(ev.object, owner) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn queue_set_health_for_host(&mut self, host_id: ObjectId, health: f32) -> bool {
        let Some(&eid) = self.host_to_entity.get(&host_id.0) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetHealth {
                target: eid,
                health,
            });
        true
    }

    pub fn apply_host_heal_events(
        &mut self,
        events: &[crate::game_logic::host_heal_log::HostHealEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_health_for_host(ev.target, ev.health) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_max_health_events(
        &mut self,
        events: &[crate::game_logic::host_max_health_log::HostMaxHealthEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetMaxHealth {
                    target: eid,
                    max_health: ev.max_health,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_experience_events(
        &mut self,
        events: &[crate::game_logic::host_experience_log::HostExperienceEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetExperience {
                    target: eid,
                    points: ev.points,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_bonus_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponBonus {
                    target: eid,
                    enthusiastic: ev.enthusiastic,
                    subliminal: ev.subliminal,
                    horde: ev.horde,
                    nationalism: ev.nationalism,
                    frenzy: ev.frenzy,
                    frenzy_level: ev.frenzy_level,
                    battle_plan_bombardment: ev.battle_plan_bombardment,
                    battle_plan_hold_the_line: ev.battle_plan_hold_the_line,
                    battle_plan_search_and_destroy: ev.battle_plan_search_and_destroy,
                    frenzy_until_frame: ev.frenzy_until_frame,
                    battle_plan_sight_scalar_applied: ev.battle_plan_sight_scalar_applied,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_slot_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetActiveWeaponSlot {
                    target: eid,
                    slot: ev.slot,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_entity_power_events(
        &mut self,
        events: &[crate::game_logic::host_entity_power_log::HostEntityPowerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetEntityPower {
                    target: eid,
                    power_provided: ev.power_provided,
                    power_consumed: ev.power_consumed,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_turret_events(
        &mut self,
        events: &[crate::game_logic::host_turret_log::HostTurretEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetTurret {
                    target: eid,
                    angle_deg: ev.angle_deg,
                    pitch_deg: ev.pitch_deg,
                    holding: ev.holding,
                    idle_scanning: ev.idle_scanning,
                    turret_turn_rate_rad: ev.turret_turn_rate_rad,
                    turret_recenter_frames: ev.turret_recenter_frames,
                    turret_hold_until_frame: ev.turret_hold_until_frame,
                    turret_idle_recentering: ev.turret_idle_recentering,
                    turret_enabled: ev.turret_enabled,
                    turret_rotating: ev.turret_rotating,
                    turret_natural_angle_deg: ev.turret_natural_angle_deg,
                    turret_natural_pitch_deg: ev.turret_natural_pitch_deg,
                    turret_target_host: ev.turret_target_host,
                    turret_force_attacking: ev.turret_force_attacking,
                    turret_mood_target: ev.turret_mood_target,
                    turret_idle_scan_next_frame: ev.turret_idle_scan_next_frame,
                    turret_idle_scan_desired_angle_deg: ev.turret_idle_scan_desired_angle_deg,
                    turret_idle_scan_index: ev.turret_idle_scan_index,
                    turret_substate: ev.turret_substate,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_target_location_events(
        &mut self,
        events: &[crate::game_logic::host_target_location_log::HostTargetLocationEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetTargetLocation {
                    unit: eid,
                    location: ev.location,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_detector_events(
        &mut self,
        events: &[crate::game_logic::host_detector_log::HostDetectorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDetector {
                    target: eid,
                    is_detector: ev.is_detector,
                    detection_range: ev.detection_range,
                    detection_rate_frames: ev.detection_rate_frames,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_continuous_fire_events(
        &mut self,
        events: &[crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetContinuousFire {
                    target: eid,
                    level: ev.level,
                    consecutive: ev.consecutive,
                    coast_until_frame: ev.coast_until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_combat_attack_events(
        &mut self,
        events: &[crate::game_logic::host_combat_attack_log::HostCombatAttackEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCombatAttack {
                    target: eid,
                    pre_attack_target_host: ev.pre_attack_target_host,
                    pre_attack_ready_at: ev.pre_attack_ready_at,
                    consecutive_shots_at_target: ev.consecutive_shots_at_target,
                    max_shots_to_fire: ev.max_shots_to_fire,
                    attack_substate_ordinal: ev.attack_substate_ordinal,
                    approach_timestamp: ev.approach_timestamp,
                    continuous_fire_victim: ev.continuous_fire_victim,
                    maintain_pos_valid: ev.maintain_pos_valid,
                    maintain_pos: ev.maintain_pos,
                    temporary_move_frames: ev.temporary_move_frames,
                    group_speed_factor: ev.group_speed_factor,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_fire_intent_events(
        &mut self,
        events: &[crate::game_logic::host_fire_intent_log::HostFireIntentEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFireIntent {
                    target: eid,
                    last_fire_victim_host: ev.last_fire_victim_host,
                    last_fire_slot: ev.last_fire_slot,
                    last_fire_damage: ev.last_fire_damage,
                    last_fire_range: ev.last_fire_range,
                    last_fire_sim_time: ev.last_fire_sim_time,
                    last_fire_frame: ev.last_fire_frame,
                    fire_intent_count: ev.fire_intent_count,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_projectile_events(
        &mut self,
        events: &[crate::game_logic::host_projectile_log::HostProjectileEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProjectileFlight {
                    host_id: ev.host_id,
                    position: ev.position,
                    velocity: ev.velocity,
                    target_position: ev.target_position,
                    damage: ev.damage,
                    shooter_host: ev.shooter_host,
                    target_host: ev.target_host,
                    speed: ev.speed,
                    lifetime: ev.lifetime,
                    max_lifetime: ev.max_lifetime,
                    is_homing: ev.is_homing,
                    flight_state: ev.flight_state,
                    active: ev.active,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Apply deferred fire-spawns into host CombatSystem (fire-spawn authority).
    pub fn apply_host_fire_spawn_events(
        &mut self,
        logic: &mut GameLogic,
        events: Vec<crate::game_logic::combat::PendingProjectile>,
    ) -> usize {
        if events.is_empty() {
            // Still drain residual hitscan marks so they cannot leak across frames.
            let _ = crate::game_logic::host_fire_spawn_log::drain_residual_hitscans();
            return 0;
        }
        // Residual auto-fire already applied same-frame hitscan HP — zero those
        // spawns' damage so dual-tick projectile resolve does not double-dip.
        let residual_hitscans = crate::game_logic::host_fire_spawn_log::drain_residual_hitscans();
        // Push into the global pending queue then drain into CombatSystem so
        // scatter/target resolution stays on the production spawn path.
        for mut ev in events {
            if let Some(tid) = ev.target_id {
                if residual_hitscans
                    .iter()
                    .any(|(s, t)| *s == ev.shooter_id && *t == tid)
                {
                    ev.damage = 0.0;
                    ev.secondary_damage = 0.0;
                }
            }
            crate::game_logic::combat::queue_projectile_direct(ev);
        }
        {
            let objects = logic.host_objects();
            // SAFETY: drain only needs shared objects map + mut combat.
            // Split via raw pointers is avoided — clone keys/positions is heavy;
            // use GameLogic helper instead.
            let _ = objects;
        }
        logic.drain_pending_projectiles_into_combat();
        crate::game_logic::host_projectile_log::record_snapshot(
            logic.combat_system.projectiles_snapshot(),
        );
        self.apply_host_projectile_events(&crate::game_logic::host_projectile_log::drain())
    }

    /// Last-write host CombatSystem projectile pose/lifetime from GameWorld residual.
    pub fn writeback_projectiles_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_projectile_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, bool)> = Vec::new();
        let gw_ids: std::collections::HashSet<u32> =
            self.world.projectiles().keys().copied().collect();
        let to_remove: Vec<crate::game_logic::ObjectId> = logic
            .combat_system
            .get_projectiles()
            .keys()
            .copied()
            .filter(|id| !gw_ids.contains(&id.0))
            .collect();
        for id in to_remove {
            // Wave 760: under coupled tick, host projectile log pending owns flight.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_projectile_log::has_pending(id.0)
            {
                continue;
            }
            if logic.combat_system.remove_projectile(id) {
                // Wave 678: GameWorld projectiles last-write residual —
                // host applies presentation bookkeeping from ready log.
                ready.push((id, true));
                updated += 1;
            }
        }
        for (hid, res) in self.world.projectiles() {
            // Wave 760: under coupled tick, host projectile log pending owns flight.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_projectile_log::has_pending(*hid)
            {
                continue;
            }
            let Some(p) = logic
                .combat_system
                .projectile_mut(crate::game_logic::ObjectId(*hid))
            else {
                continue;
            };
            let np = glam::Vec3::new(res.position[0], res.position[1], res.position[2]);
            let nv = glam::Vec3::new(res.velocity[0], res.velocity[1], res.velocity[2]);
            let nt = glam::Vec3::new(
                res.target_position[0],
                res.target_position[1],
                res.target_position[2],
            );
            let changed = (p.position - np).length_squared() > 1e-10
                || (p.velocity - nv).length_squared() > 1e-10
                || (p.target_position - nt).length_squared() > 1e-10
                || (p.lifetime - res.lifetime).abs() > f32::EPSILON
                || (p.speed - res.speed).abs() > f32::EPSILON
                || p.is_homing != res.is_homing;
            if !changed {
                continue;
            }
            p.position = np;
            p.velocity = nv;
            p.target_position = nt;
            p.lifetime = res.lifetime;
            p.max_lifetime = res.max_lifetime;
            p.speed = res.speed;
            p.is_homing = res.is_homing;
            p.damage = res.damage;
            // Wave 678: GameWorld projectiles last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push((ObjectId(*hid), false));
            updated += 1;
        }
        for (oid, removed) in ready {
            crate::game_logic::host_projectiles_ready_log::record(oid, removed);
        }
        updated
    }

    pub fn apply_host_guard_events(
        &mut self,
        events: &[crate::game_logic::host_guard_log::HostGuardEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetGuard {
                    unit: eid,
                    position: ev.position,
                    target_host: ev.target_host,
                    radius: ev.radius,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Apply host rally-point events as SetRallyPoint mutations (Wave 200).
    pub fn apply_host_rally_events(
        &mut self,
        events: &[crate::game_logic::host_rally_log::HostRallyEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_rally_point_for_host(ev.object, ev.position) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_attitude_events(
        &mut self,
        events: &[crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiAttitude {
                    target: eid,
                    attitude: ev.attitude,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_mood_events(
        &mut self,
        events: &[crate::game_logic::host_ai_mood_log::HostAiMoodEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiMood {
                    target: eid,
                    idle_since_frame: ev.idle_since_frame,
                    mood_attack_check_rate: ev.mood_attack_check_rate,
                    auto_acquire_when_idle: ev.auto_acquire_when_idle,
                    attack_priority_set: ev.attack_priority_set.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_request_events(
        &mut self,
        events: &[crate::game_logic::host_ai_request_log::HostAiRequestEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiRequest {
                    target: eid,
                    requested_victim_host: ev.requested_victim_host,
                    requested_destination: ev.requested_destination,
                    prev_victim_pos: ev.prev_victim_pos,
                    crate_created_host: ev.crate_created_host,
                    guard_retaliate_victim_host: ev.guard_retaliate_victim_host,
                    guard_retaliate_anchor: ev.guard_retaliate_anchor,
                    path_timestamp: ev.path_timestamp,
                    disguise_pending_template: ev.disguise_pending_template.clone(),
                    disguise_pending_team_ordinal: ev.disguise_pending_team_ordinal,
                    weapon_crate_upgrade: ev.weapon_crate_upgrade,
                    armor_crate_upgrade: ev.armor_crate_upgrade,
                    selection_flash_remaining: ev.selection_flash_remaining,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_decision_events(
        &mut self,
        events: &[crate::game_logic::host_ai_decision_log::HostAiDecisionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::PushAiDecision {
                    host_object: ev.host_object.0,
                    kind: ev.kind,
                    target_host: ev.target_host,
                    destination: ev.destination,
                    ai_state_ordinal: ev.ai_state_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Apply ordered AICommand residuals as GameWorld mutations (attack/move/state).
    ///
    /// Used when [`gameworld_ai_decision_authority_enabled`] — host only logged
    /// decisions; this is the authoritative apply path before writeback.
    pub fn apply_ai_decisions_as_world_mutations(
        &mut self,
        events: &[crate::game_logic::host_ai_decision_log::HostAiDecisionEvent],
    ) -> usize {
        use crate::game_logic::host_ai_decision_log::{
            AI_DECISION_ATTACK, AI_DECISION_MOVE_TO, AI_DECISION_SET_STATE, AI_DECISION_STOP_ATTACK,
        };
        let mut n = 0usize;
        for ev in events {
            // Always keep the ordered decision buffer residual.
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::PushAiDecision {
                    host_object: ev.host_object.0,
                    kind: ev.kind,
                    target_host: ev.target_host,
                    destination: ev.destination,
                    ai_state_ordinal: ev.ai_state_ordinal,
                });
            let Some(eid) = self.entity_for_host(ev.host_object) else {
                n += 1;
                continue;
            };
            match ev.kind {
                x if x == AI_DECISION_ATTACK => {
                    let target = if ev.target_host == 0 {
                        None
                    } else {
                        self.entity_for_host(crate::game_logic::ObjectId(ev.target_host))
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAttackTarget {
                            attacker: eid,
                            target,
                        });
                    // Attacking state residual.
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: 2, // Attacking
                        });
                }
                x if x == AI_DECISION_STOP_ATTACK => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAttackTarget {
                            attacker: eid,
                            target: None,
                        });
                }
                x if x == AI_DECISION_MOVE_TO => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetMoveTarget {
                            unit: eid,
                            destination: ev.destination,
                        });
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: 1, // Moving
                        });
                }
                x if x == AI_DECISION_SET_STATE => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: ev.ai_state_ordinal,
                        });
                }
                _ => {}
            }
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }
}
