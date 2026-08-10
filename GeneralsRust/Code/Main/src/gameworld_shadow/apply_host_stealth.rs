//! Host hijacker/stealth/overlord/identity/body/death/rebuild apply batch.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn apply_host_hijacker_events(
        &mut self,
        events: &[crate::game_logic::host_hijacker_log::HostHijackerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetHijacker {
                    target: eid,
                    hijack_vehicle_host: ev.hijack_vehicle_host,
                    hijacker_in_vehicle: ev.hijacker_in_vehicle,
                    hijacker_update_active: ev.hijacker_update_active,
                    hijacker_was_airborne: ev.hijacker_was_airborne,
                    hijacker_eject_pos: ev.hijacker_eject_pos,
                    hive_slave_respawn_frame: ev.hive_slave_respawn_frame,
                    next_detection_scan_frame: ev.next_detection_scan_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_stealth_flags_events(
        &mut self,
        events: &[crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetStealthFlags {
                    target: eid,
                    innate_stealth: ev.innate_stealth,
                    stealth_breaks_on_attack: ev.stealth_breaks_on_attack,
                    stealth_breaks_on_move: ev.stealth_breaks_on_move,
                    is_tunnel_network: ev.is_tunnel_network,
                    passengers_allowed_to_fire: ev.passengers_allowed_to_fire,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_stealth_delay_events(
        &mut self,
        events: &[crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetStealthDelay {
                    target: eid,
                    stealth_allowed_frame: ev.stealth_allowed_frame,
                    stealth_delay_pending: ev.stealth_delay_pending,
                    stealth_delay_frames: ev.stealth_delay_frames,
                    stealth_breaks_on_damage: ev.stealth_breaks_on_damage,
                    detection_expires_frame: ev.detection_expires_frame,
                    camo_opacity_pulse_phase: ev.camo_opacity_pulse_phase,
                    camo_heat_vision_opacity: ev.camo_heat_vision_opacity,
                    camo_net_sub_object_shown: ev.camo_net_sub_object_shown,
                    camo_net_sub_object_observer_visible: ev.camo_net_sub_object_observer_visible,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_overlord_events(
        &mut self,
        events: &[crate::game_logic::host_overlord_log::HostOverlordEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetOverlordAddon {
                    target: eid,
                    has_gattling: ev.has_gattling,
                    has_propaganda: ev.has_propaganda,
                    bunker_capacity: ev.bunker_capacity,
                    is_helix_transport: ev.is_helix_transport,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_command_set_events(
        &mut self,
        events: &[crate::game_logic::host_command_set_log::HostCommandSetEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCommandSet {
                    target: eid,
                    command_set: ev.command_set.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_command_set_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            // Wave 758: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_command_set_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let Some(obj) = logic.host_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let host = obj.command_set_override.clone().unwrap_or_default();
            if host == ent.command_set_override {
                continue;
            }
            // Wave 945: command-set writeback via host writeback authority.
            if !logic.apply_host_writeback_op(crate::game_logic::HostWritebackOp::CommandSet {
                id: ObjectId(hid),
                override_name: if ent.command_set_override.is_empty() {
                    None
                } else {
                    Some(ent.command_set_override.clone())
                },
            }) {
                continue;
            }
            // Wave 644: GameWorld command-set last-write residual —
            // host applies presentation bookkeeping from ready log.
            ready.push(ObjectId(hid));
            updated += 1;
        }
        for oid in ready {
            crate::game_logic::host_command_set_ready_log::record(oid);
        }
        updated
    }

    pub fn apply_host_disguise_events(
        &mut self,
        events: &[crate::game_logic::host_disguise_log::HostDisguiseEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDisguise {
                    target: eid,
                    template: ev.template.clone(),
                    team_ordinal: ev.team_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_vision_camo_events(
        &mut self,
        events: &[crate::game_logic::host_vision_camo_log::HostVisionCamoEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetVisionCamo {
                    target: eid,
                    vision_spied_mask: ev.vision_spied_mask,
                    camo_friendly_opacity: ev.camo_friendly_opacity,
                    camo_stealth_look: ev.camo_stealth_look,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_stats_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponStats {
                    target: eid,
                    has_weapon: ev.has_weapon,
                    weapon_damage: ev.weapon_damage,
                    weapon_range: ev.weapon_range,
                    weapon_min_range: ev.weapon_min_range,
                    weapon_reload_time: ev.weapon_reload_time,
                    weapon_last_fire_time: ev.weapon_last_fire_time,
                    weapon_clip_size: ev.weapon_clip_size,
                    weapon_clip_reload_time: ev.weapon_clip_reload_time,
                    weapon_ammo: ev.weapon_ammo,
                    weapon_can_target_air: ev.weapon_can_target_air,
                    weapon_can_target_ground: ev.weapon_can_target_ground,
                    weapon_projectile_speed: ev.weapon_projectile_speed,
                    has_secondary_weapon: ev.has_secondary_weapon,
                    secondary_weapon_damage: ev.secondary_weapon_damage,
                    secondary_weapon_range: ev.secondary_weapon_range,
                    leech_range_active_primary: ev.leech_range_active_primary,
                    leech_range_active_secondary: ev.leech_range_active_secondary,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Queue SetBodyDamage from host BodyDamageType residual log.
    pub fn apply_host_body_damage_events(
        &mut self,
        events: &[crate::game_logic::host_body_damage_log::HostBodyDamageEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBodyDamage {
                    target: eid,
                    body_damage_state: ev.body_damage_state,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_death_type_events(
        &mut self,
        events: &[crate::game_logic::host_death_type_log::HostDeathTypeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDeathType {
                    target: eid,
                    death_type: ev.death_type,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_radar_extend_events(
        &mut self,
        events: &[crate::game_logic::host_radar_extend_log::HostRadarExtendEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRadarExtend {
                    target: eid,
                    radar_extend_done_frame: ev.radar_extend_done_frame,
                    radar_extend_complete: ev.radar_extend_complete,
                    radar_active: ev.radar_active,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_shock_stun_events(
        &mut self,
        events: &[crate::game_logic::host_shock_stun_log::HostShockStunEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetShockStun {
                    target: eid,
                    shock_stun_frames: ev.shock_stun_frames,
                    shock_yaw_rate: ev.shock_yaw_rate,
                    shock_pitch_rate: ev.shock_pitch_rate,
                    shock_roll_rate: ev.shock_roll_rate,
                    shock_up_z: ev.shock_up_z,
                    shock_allow_bounce: ev.shock_allow_bounce,
                    shock_grounded_once: ev.shock_grounded_once,
                    shock_was_airborne: ev.shock_was_airborne,
                    cell_is_cliff: ev.cell_is_cliff,
                    cell_is_underwater: ev.cell_is_underwater,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_rebuild_producer_events(
        &mut self,
        events: &[crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRebuildProducer {
                    target: eid,
                    is_rebuild_hole: ev.is_rebuild_hole,
                    rebuild_template_name: ev.rebuild_template_name.clone(),
                    rebuild_ready_frame: ev.rebuild_ready_frame,
                    rebuild_spawner_id: ev.rebuild_spawner_id,
                    rebuild_worker_id: ev.rebuild_worker_id,
                    rebuild_reconstructing_id: ev.rebuild_reconstructing_id,
                    producer_id: ev.producer_id,
                    construction_complete_clear_frame: ev.construction_complete_clear_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_sole_healing_events(
        &mut self,
        events: &[crate::game_logic::host_sole_healing_log::HostSoleHealingEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetSoleHealing {
                    target: eid,
                    sole_healing_benefactor_id: ev.sole_healing_benefactor_id,
                    sole_healing_benefactor_expiration_frame: ev
                        .sole_healing_benefactor_expiration_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }
}
