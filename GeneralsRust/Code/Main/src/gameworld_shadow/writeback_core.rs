//! Health/economy/upgrade/attack/move/transform writebacks and production tick.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    /// Write shadow entity health/destroyed onto host objects.
    pub fn writeback_health_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_damage_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let new_h = ent.health.max(0.0);
            let new_max = ent.max_health.max(1.0);
            let changed = (obj.health.current - new_h).abs() > 0.000_1
                || ((new_h <= 0.0) != obj.status.destroyed)
                || (obj.max_health - new_max).abs() > 0.000_1
                || (obj.health.maximum - new_max).abs() > 0.000_1;
            if !changed {
                continue;
            }
            let destroy = new_h <= 0.0;
            // Wave 944: health writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Health {
                id: ObjectId(hid),
                current: new_h.min(new_max),
                maximum: new_max,
                destroy,
            }) {
                continue;
            }
            if destroy {
                // Wave 621: GameWorld sole damage last-write lethal residual —
                // host process_destroy_list drains and marks die side effects.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    crate::game_logic::host_destroy_ready_log::record(ObjectId(hid), new_h);
                }
            }
            updated += 1;
        }
        updated
    }

    /// Queue damage on the shadow entity mapped from a host object.
    /// Returns false if the host id is not mapped.
    /// Write shadow player supplies/power onto host players (economy last writer).
    pub fn writeback_economy_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<crate::game_logic::host_economy_ready_log::HostEconomyReadyEvent> =
            Vec::new();
        for (&hid, &gw) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(gw) else {
                continue;
            };
            let Some(player) = logic.get_player_mut(hid) else {
                continue;
            };
            // Wave 760: under coupled tick, host economy log pending = mid-frame authority.
            if shadow_coupled_tick_active() && crate::game_logic::host_economy_log::has_pending(hid)
            {
                continue;
            }
            let prev_supplies = player.resources.supplies;
            let prev_power = player.power_available;
            let prev_radar = player.radar_count;
            let prev_radar_dis = player.radar_disabled;
            let prev_alive = player.is_alive;
            let mut dirty = false;
            let mut supplies_changed = false;
            let mut power_changed = false;
            let mut radar_changed = false;
            let mut alive_changed = false;
            if player.resources.supplies != pd.supplies {
                player.resources.supplies = pd.supplies;
                dirty = true;
                supplies_changed = true;
            }
            // Economy authority: host pending delta is consumed by absolute writeback.
            if player.pending_supply_delta != 0 {
                player.pending_supply_delta = 0;
                dirty = true;
            }
            if player.power_available != pd.power_available {
                player.power_available = pd.power_available;
                dirty = true;
                power_changed = true;
            }
            if player.power_produced != pd.power_produced {
                player.power_produced = pd.power_produced;
                dirty = true;
            }
            if player.power_consumed != pd.power_consumed {
                player.power_consumed = pd.power_consumed;
                dirty = true;
            }
            // C++ radar state is owned by the player whose objects grant it
            // (Player.cpp:3132 addRadar / Player.h:326). A GW slot may only
            // write radar/alive residuals onto the host player whose faction
            // it actually mirrors — a stale or observer slot (team None)
            // must never zero a live faction player's flag.
            let slot_owning_faction = match player.team {
                Team::USA => Some(0u8),
                Team::China => Some(1),
                Team::GLA => Some(2),
                Team::Neutral => None,
            };
            let slot_owns_player_state = pd.team == slot_owning_faction;
            if slot_owns_player_state && player.radar_count != pd.radar_count {
                player.radar_count = pd.radar_count;
                dirty = true;
                radar_changed = true;
            }
            if slot_owns_player_state && player.radar_disabled != pd.radar_disabled {
                player.radar_disabled = pd.radar_disabled;
                dirty = true;
                radar_changed = true;
            }
            if slot_owns_player_state && player.is_alive != pd.is_alive {
                player.is_alive = pd.is_alive;
                dirty = true;
                alive_changed = true;
            }
            if (player.cash_bounty_percent - pd.cash_bounty_percent).abs() > 1e-6 {
                player.cash_bounty_percent = pd.cash_bounty_percent;
                dirty = true;
            }
            if player.color_rgb != pd.color_rgb {
                player.color_rgb = pd.color_rgb;
                dirty = true;
            }
            if player.rank_level != pd.rank_level {
                player.rank_level = pd.rank_level;
                dirty = true;
            }
            if player.skill_points != pd.skill_points {
                player.skill_points = pd.skill_points;
                dirty = true;
            }
            if player.science_purchase_points != pd.science_purchase_points {
                player.science_purchase_points = pd.science_purchase_points;
                dirty = true;
            }
            {
                use std::collections::HashSet;
                let want: HashSet<String> = pd.unlocked_sciences.iter().cloned().collect();
                if player.unlocked_sciences != want {
                    player.unlocked_sciences = want;
                    dirty = true;
                }
            }
            // Shared superweapon cooldown last-writer (Debug-name keys).
            {
                use crate::command_system::SpecialPowerType;
                let mut next = std::collections::HashMap::new();
                // Preserve host keys while applying shadow remaining times by Debug name.
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
                // Insert shadow-only timers for a small set of known powers (writeback residual).
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
                    dirty = true;
                }
            }
            if dirty {
                // Wave 631: GameWorld economy last-write residual —
                // host applies presentation bookkeeping from ready log.
                if supplies_changed || power_changed || radar_changed || alive_changed {
                    ready.push(
                        crate::game_logic::host_economy_ready_log::HostEconomyReadyEvent {
                            player_id: hid,
                            previous_supplies: prev_supplies,
                            supplies: player.resources.supplies,
                            previous_power: prev_power,
                            power_available: player.power_available,
                            previous_radar_count: prev_radar,
                            radar_count: player.radar_count,
                            previous_radar_disabled: prev_radar_dis,
                            radar_disabled: player.radar_disabled,
                            previous_alive: prev_alive,
                            is_alive: player.is_alive,
                            supplies_changed,
                            power_changed,
                            radar_changed,
                            alive_changed,
                        },
                    );
                }
                updated += 1;
            }
        }
        for ev in ready {
            crate::game_logic::host_economy_ready_log::record(ev);
        }
        updated
    }

    /// Write shadow PlayerData::completed_upgrades back onto host HostUpgradeRegistry.
    /// Completes the CompleteUpgrade channel as GameWorld last-writer residual.
    ///
    /// Wave 624: new completions are recorded into `host_upgrade_ready_log` so
    /// host can apply unlock/EVA/radar side effects after writeback (GW decides
    /// completion; host still owns residual apply).
    pub fn writeback_completed_upgrades_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_upgrades::{HostUpgradePhase, normalize_upgrade_identity};
        let mut updated = 0usize;
        let frame = logic.get_frame();
        let mut ready: Vec<(u32, String)> = Vec::new();
        for (&host_id, &gw) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(gw) else {
                continue;
            };
            // Wave 760: under coupled tick, host upgrade frame events own completion path.
            if shadow_coupled_tick_active() {
                let hu = logic.host_upgrades();
                let host_busy = hu
                    .completed_this_frame_snapshot()
                    .iter()
                    .any(|e| e.player_id == host_id)
                    || hu
                        .queued_this_frame_snapshot()
                        .iter()
                        .any(|e| e.player_id == host_id);
                if host_busy {
                    continue;
                }
            }
            if pd.completed_upgrades.is_empty() {
                continue;
            }
            let mut dirty = false;
            for name in &pd.completed_upgrades {
                let key = normalize_upgrade_identity(name);
                let already = logic.host_upgrades().entries_snapshot().iter().any(|e| {
                    e.player_id == host_id
                        && e.phase == HostUpgradePhase::Completed
                        && normalize_upgrade_identity(&e.name) == key
                });
                if already {
                    continue;
                }
                // Wave 624: registry mark is deferred to host_apply so side effects
                // and record_complete stay on one host path (units_affected accurate).
                ready.push((host_id, name.clone()));
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        let _ = frame;
        for (host_id, name) in ready {
            crate::game_logic::host_upgrade_ready_log::record(
                host_id,
                name,
                crate::game_logic::ObjectId(0),
            );
        }
        updated
    }

    /// Write shadow Entity::attack_target back onto host Object::target (stable IDs).
    /// Completes the attack command channel: host log / set_target → shadow mutation → host writeback.
    pub fn writeback_attack_targets_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_ai_attack_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let host_target = ent.attack_target.and_then(|te| self.host_for_entity(te));
            // host_attack_log is a session input, not a writeback veto.
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if obj.target == host_target {
                continue;
            }
            let prev = obj.target;
            // Wave 944: attack-target writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::AttackTarget {
                id: ObjectId(hid),
                target: host_target,
                clear_target_location: host_target.is_some(),
            }) {
                continue;
            }
            // Wave 638: GameWorld attack-target last-write residual —
            // host applies AI/status/attack-log bookkeeping from ready log.
            ready.push((ObjectId(hid), prev, host_target));
            updated += 1;
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_attack_target_ready_log::record(oid, prev, next);
        }
        updated
    }

    /// Queue SetMoveTarget for a mapped host object (move-command channel).
    pub fn queue_set_move_target_for_host(
        &mut self,
        host: ObjectId,
        destination: Option<[f32; 3]>,
    ) -> bool {
        let Some(unit) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(WorldMutation::SetMoveTarget { unit, destination });
        true
    }

    /// Sync host movement.target_position onto shadow via SetMoveTarget mutations.
    pub fn apply_host_move_targets(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let dest = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
            if self.queue_set_move_target_for_host(ObjectId(hid), dest) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Write shadow Entity::move_target back onto host movement.target_position.
    /// Direct field write (no host_move_log) to avoid echo loops.
    pub fn writeback_move_targets_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<(ObjectId, Option<[f32; 3]>, Option<[f32; 3]>)> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 759: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_move_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host_dest = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
            let shadow_dest = ent.move_target;
            let same = match (host_dest, shadow_dest) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    (a[0] - b[0]).abs() < 0.01
                        && (a[1] - b[1]).abs() < 0.01
                        && (a[2] - b[2]).abs() < 0.01
                }
                _ => false,
            };
            if same {
                continue;
            }
            let prev = host_dest;
            // Wave 944: move-target writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::MoveTarget {
                id: ObjectId(hid),
                destination: shadow_dest.map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            }) {
                continue;
            }
            // Wave 639: GameWorld move-target last-write residual —
            // host applies AI/status/movement bookkeeping from ready log.
            ready.push((ObjectId(hid), prev, shadow_dest));
            updated += 1;
        }
        for (oid, prev, next) in ready {
            crate::game_logic::host_move_target_ready_log::record(oid, prev, next);
        }
        updated
    }

    /// Write shadow entity pose (position + orientation) onto host objects.
    /// Last-writer residual after SetTransform / apply_host_positions channel.
    pub fn writeback_transforms_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut n = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 759: under coupled tick, host move/movement pending owns pose.
            if shadow_coupled_tick_active()
                && (crate::game_logic::host_move_log::has_pending(ObjectId(hid))
                    || crate::game_logic::host_movement_log::has_pending(ObjectId(hid)))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let p = ent.transform.position;
            let host_p = obj.get_position();
            let host_o = obj.get_orientation();
            let dx = (host_p.x - p.x).abs();
            let dy = (host_p.y - p.y).abs();
            let dz = (host_p.z - p.z).abs();
            let d_o = (host_o - ent.transform.orientation).abs();
            if dx > 1e-3 || dy > 1e-3 || dz > 1e-3 || d_o > 1e-3 {
                // Wave 944: transform writeback via host writeback authority.
                if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::Transform {
                    id: ObjectId(hid),
                    position: glam::Vec3::new(p.x, p.y, p.z),
                    orientation: ent.transform.orientation,
                }) {
                    continue;
                }
                // Wave 636: GameWorld transform last-write residual —
                // host applies movement/presentation bookkeeping from ready log.
                ready.push(ObjectId(hid));
                n += 1;
            }
        }
        for oid in ready {
            crate::game_logic::host_transform_ready_log::record(oid);
        }
        n
    }

    /// Write shadow production queue + rally_point last-writer residual onto host buildings.

    /// Under PRODUCTION_AUTHORITY: advance entity production queue progress by dt.
    /// Host completes/spawns from writeback-finished heads next frame.
    pub fn tick_production_queues(&mut self, dt: f32) -> usize {
        if !gameworld_production_authority_enabled() {
            return 0;
        }
        use gamelogic::world::WorldMutation;
        use gamelogic::world::entities::{EntityId, EntityProductionItem};
        let mut n = 0usize;
        let mut updates: Vec<(EntityId, Vec<EntityProductionItem>)> = Vec::new();
        let mut exit_updates: Vec<(EntityId, f32)> = Vec::new();
        let mut queue_exit_runtime_updates: Vec<(EntityId, u32, u32)> = Vec::new();
        // Snapshot host ids for power lookup without double-borrow.
        let host_ids: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in host_ids {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // C++ GameLogic.cpp:3677 — ProductionUpdate process mask is HELD;
            // DISABLED_UNDERPOWERED skips the module (no 50-80% power-factor).
            let production_frozen = ent.disabled_underpowered && !ent.disabled_held;
            if !ent.production_queue_items.is_empty()
                && !ent.production_paused
                && !production_frozen
            {
                let mut items = ent.production_queue_items.clone();
                if let Some(head) = items.first_mut() {
                    let pf = self
                        .production_power_factor_by_host
                        .get(&hid)
                        .copied()
                        .unwrap_or(1.0)
                        .max(0.01);
                    if !head.is_complete_at_power(pf) {
                        // C++ ProductionUpdate::update increments an integer
                        // frame counter once per logic update, then compares
                        // against the current calcTimeToBuild threshold.  Do
                        // not reintroduce float seconds authority in the
                        // GameWorld sole-tick path.
                        head.advance_one_construction_frame(pf);
                        n += 1;
                        updates.push((eid, items));
                    }
                }
            }
            // C++ QueueProductionExitUpdate::update owns an integer logic
            // countdown.  A remaining InitialBurst keeps the interface free
            // and resets delay to zero; otherwise decrement exactly once per
            // GameWorld logic update.  Float-only state is retained solely for
            // old snapshots / unparsed legacy producers.
            if ent.queue_exit_state_initialized {
                let next_frames =
                    if ent.exit_burst_remaining > 0 || ent.exit_delay_remaining_frames == 0 {
                        0
                    } else {
                        ent.exit_delay_remaining_frames.saturating_sub(1)
                    };
                if next_frames != ent.exit_delay_remaining_frames {
                    queue_exit_runtime_updates.push((eid, next_frames, ent.exit_burst_remaining));
                    n += 1;
                }
            } else if ent.exit_delay_remaining > 0.0 && dt > 0.0 {
                let next = (ent.exit_delay_remaining - dt).max(0.0);
                if (next - ent.exit_delay_remaining).abs() > 1e-9 {
                    exit_updates.push((eid, next));
                    n += 1;
                }
            }
        }
        for (eid, items) in updates {
            self.world
                .queue_mutation(WorldMutation::SetProductionQueue { target: eid, items });
        }
        for (eid, exit_delay_remaining) in exit_updates {
            self.world.queue_mutation(WorldMutation::SetExitDelay {
                target: eid,
                exit_delay_remaining,
            });
        }
        for (eid, exit_delay_remaining_frames, exit_burst_remaining) in queue_exit_runtime_updates {
            self.world
                .queue_mutation(WorldMutation::SetProductionExitRuntime {
                    target: eid,
                    exit_delay_remaining_frames,
                    exit_burst_remaining,
                    queue_exit_state_initialized: true,
                });
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }
}
