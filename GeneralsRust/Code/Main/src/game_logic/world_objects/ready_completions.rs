//! Host objects `impl GameLogic` — `ready_completions`.
//! allocate_object_id and host_apply_*_ready_completions. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub(in super::super) fn allocate_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id = ObjectId(self.next_object_id.0 + 1);
        id
    }

    /// Wave 622: under damage authority, GameWorld experience writeback records
    /// veterancy level-ups; host applies combat bonus residual for those IDs.
    /// Wave 623: under damage authority, GameWorld body-damage writeback records
    /// state transitions; host applies model/FX residual for those IDs.
    /// Wave 624: under GameWorld completed-upgrade writeback, drain ready log and
    /// apply full host upgrade residual (unlocks, EVA, radar, status bits).
    /// Wave 625: GameWorld radar-extend complete writeback records ready IDs;
    /// host applies upgraded model residual and complete counter.
    /// Wave 626: under construction sole-tick, GameWorld writeback records
    /// producers whose CONSTRUCTION_COMPLETE clear deadline elapsed; host clears
    /// the model bit and counts residual.
    /// Wave 627: GameWorld production-door writeback records phase changes;
    /// host applies door model-condition residual for the new phase.
    /// Wave 628: GameWorld contain writeback records membership changes;
    /// host applies garrison AI residual + enter/exit honesty counters.
    /// Wave 629: GameWorld owner writeback records team changes; host applies
    /// capture residual (kick passengers, deselect, idle, score).
    /// Wave 630: GameWorld AI-state writeback records ordinal changes; host
    /// applies moving/attacking combat-status residual for the new state.
    /// Wave 631: GameWorld economy writeback records supply/power/radar/alive
    /// changes; host applies presentation residual via host_economy_log and
    /// radar log (GW decides absolute values; host still owns UI bookkeeping).
    /// Wave 632: GameWorld death-type writeback records ordinal changes; host
    /// applies destroy/pilot presentation bookkeeping residual.
    /// Wave 633: GameWorld model-condition writeback records bit changes; host
    /// applies presentation bookkeeping residual (drawable model condition log).
    /// Wave 634: GameWorld combat-status writeback records dirty objects; host
    /// applies status presentation residual via host_status_log.
    /// Wave 635: GameWorld weapon-stats writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_weapon_stats.
    /// Wave 636: GameWorld transform writeback records dirty objects; host
    /// applies movement/presentation bookkeeping residual.
    /// Wave 637: GameWorld movement writeback records dirty objects; host
    /// applies path/presentation bookkeeping residual via record_host_movement.
    /// Wave 638: GameWorld attack-target writeback records target changes; host
    /// applies AI/status/attack-log residual (without re-assigning target).
    /// Wave 639: GameWorld move-target writeback records destination changes;
    /// host applies AI/status/movement residual without re-assigning destination.
    /// Wave 640: GameWorld fire-intent writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_fire_intent.
    /// Wave 641: GameWorld stored-supplies writeback records changes; host
    /// applies gatherer presentation residual (HUD / supply counter consumers).
    /// Wave 642: GameWorld weapon-set writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_weapon_set.
    /// Wave 643: GameWorld combat-attack writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_combat_attack.
    /// Wave 644: GameWorld command-set writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_command_set.
    /// Wave 645: GameWorld AI-mood writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_ai_mood.
    /// Wave 646: GameWorld locomotor writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_locomotor.
    /// Wave 647: GameWorld hijacker writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_hijacker.
    /// Wave 648: GameWorld AI-request writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_ai_request.
    /// Wave 649: GameWorld physics motive writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_physics_motive.
    pub fn host_apply_physics_motive_ready_completions(&mut self) -> usize {
        // Wave 649: GameWorld physics motive writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_physics_motive.
        let events = crate::game_logic::host_physics_motive_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_physics_motive();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 650: GameWorld bounce land writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_bounce_land.
    pub fn host_apply_bounce_land_ready_completions(&mut self) -> usize {
        // Wave 650: GameWorld bounce land writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_bounce_land.
        let events = crate::game_logic::host_bounce_land_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_bounce_land();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 651: GameWorld stealth delay writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_stealth_delay.
    /// Wave 652: GameWorld stealth flags writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_stealth_flags.
    pub fn host_apply_stealth_flags_ready_completions(&mut self) -> usize {
        // Wave 652: GameWorld stealth flags writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_stealth_flags.
        let events = crate::game_logic::host_stealth_flags_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_stealth_flags();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 653: GameWorld disguise writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_disguise.
    pub fn host_apply_disguise_ready_completions(&mut self) -> usize {
        // Wave 653: GameWorld disguise writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_disguise.
        let events = crate::game_logic::host_disguise_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_disguise();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 654: GameWorld vision camo writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_vision_camo.
    /// Wave 655: GameWorld selection radius writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_selection_radius_ready_completions(&mut self) -> usize {
        // Wave 655: GameWorld selection radius writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_selection_radius_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_selection_radius();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 656: GameWorld ground height writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_ground_height_ready_completions(&mut self) -> usize {
        // Wave 656: GameWorld ground height writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_ground_height_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ground_height();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 657: GameWorld weapon slot writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_weapon_slot_ready_completions(&mut self) -> usize {
        // Wave 657: GameWorld weapon slot writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_weapon_slot_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_slot();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 658: GameWorld weapon bonus writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_weapon_bonus_ready_completions(&mut self) -> usize {
        // Wave 658: GameWorld weapon bonus writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_weapon_bonus_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_bonus();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 659: GameWorld AI attitude writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_ai_attitude_ready_completions(&mut self) -> usize {
        // Wave 659: GameWorld AI attitude writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_ai_attitude_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_attitude();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 660: GameWorld identity writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_identity_ready_completions(&mut self) -> usize {
        // Wave 660: GameWorld identity writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_identity_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_identity();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 661: GameWorld repulsor writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    /// Wave 662: GameWorld shock stun writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_shock_stun.
    pub fn host_apply_shock_stun_ready_completions(&mut self) -> usize {
        // Wave 662: GameWorld shock stun writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_shock_stun.
        let events = crate::game_logic::host_shock_stun_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_shock_stun();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 663: GameWorld sole healing writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_sole_healing.
    pub fn host_apply_sole_healing_ready_completions(&mut self) -> usize {
        // Wave 663: GameWorld sole healing writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_sole_healing.
        let events = crate::game_logic::host_sole_healing_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_sole_healing();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 664: GameWorld crush vision writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_crush_vision.
    pub fn host_apply_crush_vision_ready_completions(&mut self) -> usize {
        // Wave 664: GameWorld crush vision writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_crush_vision.
        let events = crate::game_logic::host_crush_vision_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_crush_vision();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 665: GameWorld demo mine cheer writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_demo_mine_cheer.
    pub fn host_apply_demo_mine_cheer_ready_completions(&mut self) -> usize {
        // Wave 665: GameWorld demo mine cheer writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_demo_mine_cheer.
        let events = crate::game_logic::host_demo_mine_cheer_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_demo_mine_cheer();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 666: GameWorld overlord writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_overlord.
    pub fn host_apply_overlord_ready_completions(&mut self) -> usize {
        // Wave 666: GameWorld overlord writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_overlord.
        let events = crate::game_logic::host_overlord_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_overlord();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 667: GameWorld hive writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_hive.
    pub fn host_apply_hive_ready_completions(&mut self) -> usize {
        // Wave 667: GameWorld hive writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_hive.
        let events = crate::game_logic::host_hive_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_hive();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 668: GameWorld overcharge writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_overcharge.
    pub fn host_apply_overcharge_ready_completions(&mut self) -> usize {
        // Wave 668: GameWorld overcharge writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_overcharge.
        let events = crate::game_logic::host_overcharge_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_overcharge();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 669: GameWorld guard writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_guard.
    pub fn host_apply_guard_ready_completions(&mut self) -> usize {
        // Wave 669: GameWorld guard writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_guard.
        let events = crate::game_logic::host_guard_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_guard();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 670: GameWorld continuous fire writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_continuous_fire.
    pub fn host_apply_continuous_fire_ready_completions(&mut self) -> usize {
        // Wave 670: GameWorld continuous fire writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_continuous_fire.
        let events = crate::game_logic::host_continuous_fire_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_continuous_fire();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 671: GameWorld detector writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_detector.
    pub fn host_apply_detector_ready_completions(&mut self) -> usize {
        // Wave 671: GameWorld detector writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_detector.
        let events = crate::game_logic::host_detector_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_detector();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 672: GameWorld target location writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_target_location.
    pub fn host_apply_target_location_ready_completions(&mut self) -> usize {
        // Wave 672: GameWorld target location writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_target_location.
        let events = crate::game_logic::host_target_location_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_target_location();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 673: GameWorld turret writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_turret.
    pub fn host_apply_turret_ready_completions(&mut self) -> usize {
        // Wave 673: GameWorld turret writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_turret.
        let events = crate::game_logic::host_turret_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_turret();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 674: GameWorld entity power writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_entity_power.
    pub fn host_apply_entity_power_ready_completions(&mut self) -> usize {
        // Wave 674: GameWorld entity power writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_entity_power.
        let events = crate::game_logic::host_entity_power_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_entity_power();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 675: GameWorld building type writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_building_type.
    /// Wave 676: GameWorld faerie-fire writeback records dirty objects; host
    /// applies presentation bookkeeping residual via host_faerie_fire_log.
    /// Wave 678: GameWorld projectiles writeback records dirty combat projectiles;
    /// host applies presentation bookkeeping residual via host_projectile_log.
    pub fn host_apply_projectiles_ready_completions(&mut self) -> usize {
        // Wave 678: GameWorld projectiles writeback records dirty combat projectiles;
        // host applies presentation bookkeeping residual via host_projectile_log.
        let events = crate::game_logic::host_projectiles_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.removed {
                // Removal already applied during writeback; residual log marks inactive.
                crate::game_logic::host_projectile_log::record(
                    ev.object.0,
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    0.0,
                    0,
                    0,
                    0.0,
                    0.0,
                    0.0,
                    false,
                    false,
                );
                n = n.saturating_add(1);
                continue;
            }
            let Some(p) = self.combat_system.get_projectiles().get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_projectile_log::record(
                p.id.0,
                [p.position.x, p.position.y, p.position.z],
                [p.velocity.x, p.velocity.y, p.velocity.z],
                [
                    p.target_position.x,
                    p.target_position.y,
                    p.target_position.z,
                ],
                p.damage,
                p.shooter_id.0,
                p.target_id.map(|t| t.0).unwrap_or(0),
                p.speed,
                p.lifetime,
                p.max_lifetime,
                p.is_homing,
                true,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_faerie_fire_ready_completions(&mut self) -> usize {
        // Wave 676: GameWorld faerie-fire writeback records dirty objects; host
        // applies presentation bookkeeping residual via host_faerie_fire_log.
        let events = crate::game_logic::host_faerie_fire_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_faerie_fire_log::record(
                obj.id,
                obj.status.faerie_fire,
                obj.faerie_fire_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 677: GameWorld disable-timers writeback records dirty objects; host
    /// applies presentation bookkeeping residual via host_disable_timers_log.
    pub fn host_apply_disable_timers_ready_completions(&mut self) -> usize {
        // Wave 677: GameWorld disable-timers writeback records dirty objects; host
        // applies presentation bookkeeping residual via host_disable_timers_log.
        let events = crate::game_logic::host_disable_timers_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_disable_timers_log::record(
                obj.id,
                obj.status.disabled_emp_until_frame,
                obj.status.disabled_hacked_until_frame,
                obj.status.disabled_paralyzed_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_building_type_ready_completions(&mut self) -> usize {
        // Wave 675: GameWorld building type writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_building_type.
        let events = crate::game_logic::host_building_type_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_building_type();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_repulsor_ready_completions(&mut self) -> usize {
        // Wave 661: GameWorld repulsor writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_repulsor_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_repulsor_log::record(
                obj.id,
                obj.status.repulsor,
                obj.repulsor_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_vision_camo_ready_completions(&mut self) -> usize {
        // Wave 654: GameWorld vision camo writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_vision_camo.
        let events = crate::game_logic::host_vision_camo_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_vision_camo();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_stealth_delay_ready_completions(&mut self) -> usize {
        // Wave 651: GameWorld stealth delay writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_stealth_delay.
        let events = crate::game_logic::host_stealth_delay_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_stealth_delay();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_ai_request_ready_completions(&mut self) -> usize {
        // Wave 648: GameWorld AI-request writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_ai_request.
        let events = crate::game_logic::host_ai_request_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_request();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_hijacker_ready_completions(&mut self) -> usize {
        // Wave 647: GameWorld hijacker writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_hijacker.
        let events = crate::game_logic::host_hijacker_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_hijacker();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_locomotor_ready_completions(&mut self) -> usize {
        // Wave 646: GameWorld locomotor writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_locomotor.
        let events = crate::game_logic::host_locomotor_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_locomotor();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_ai_mood_ready_completions(&mut self) -> usize {
        // Wave 645: GameWorld AI-mood writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_ai_mood.
        let events = crate::game_logic::host_ai_mood_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_mood();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_command_set_ready_completions(&mut self) -> usize {
        // Wave 644: GameWorld command-set writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_command_set.
        let events = crate::game_logic::host_command_set_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_command_set();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_combat_attack_ready_completions(&mut self) -> usize {
        // Wave 643: GameWorld combat-attack writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_combat_attack.
        let events = crate::game_logic::host_combat_attack_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_combat_attack();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_weapon_set_ready_completions(&mut self) -> usize {
        // Wave 642: GameWorld weapon-set writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_weapon_set.
        let events = crate::game_logic::host_weapon_set_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_set();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_stored_supplies_ready_completions(&mut self) -> usize {
        // Wave 641: GameWorld stored-supplies writeback records changes; host
        // applies gatherer presentation residual (HUD / supply counter consumers).
        let events = crate::game_logic::host_stored_supplies_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_supplies == ev.new_supplies {
                continue;
            }
            if !self.objects.contains_key(&ev.object) {
                continue;
            }
            // Supplies already writeback-synced. Re-record via host economy-adjacent
            // presentation residual when a gatherer carry amount changes.
            crate::game_logic::host_stored_supplies_log::record(ev.object, ev.new_supplies);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_fire_intent_ready_completions(&mut self) -> usize {
        // Wave 640: GameWorld fire-intent writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_fire_intent.
        let events = crate::game_logic::host_fire_intent_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Fire-intent already writeback-synced; re-record host fire-intent
            // log for presentation / combat consumers.
            obj.record_host_fire_intent();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_move_target_ready_completions(&mut self) -> usize {
        // Wave 639: GameWorld move-target writeback records destination changes;
        // host applies AI/status/movement residual without re-assigning destination.
        let events = crate::game_logic::host_move_target_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Destination already writeback-synced. Apply residual side effects.
            if ev.new_target.is_some() {
                // Prefer status bits over full set_ai_state to avoid fighting
                // GW AI-state writeback (Wave 630).
                obj.set_status_moving(true);
            } else {
                obj.set_status_moving(false);
            }
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_attack_target_ready_completions(&mut self) -> usize {
        // Wave 638: GameWorld attack-target writeback records target changes; host
        // applies AI/status/attack-log residual (without re-assigning target).
        let events = crate::game_logic::host_attack_target_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_target == ev.new_target {
                continue;
            }
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Target already writeback-synced. Apply residual side effects only.
            if ev.new_target.is_some() {
                let _ = obj.takeoff_from_airfield_parking();
                obj.target_location = None;
                obj.record_host_target_location();
                // Prefer combat status bits over full set_ai_state to avoid
                // host_ai_state_log re-entry fighting GW AI-state writeback.
                obj.set_status_attacking(true);
            } else {
                obj.target_location = None;
                obj.set_status_force_attack(false);
                obj.set_status_attacking(false);
            }
            crate::game_logic::host_attack_log::record(ev.object, ev.new_target);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_movement_ready_completions(&mut self) -> usize {
        // Wave 637: GameWorld movement writeback records dirty objects; host
        // applies path/presentation bookkeeping residual via record_host_movement.
        let events = crate::game_logic::host_movement_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Movement already writeback-synced; re-record host movement log
            // for presentation / path consumers.
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_transform_ready_completions(&mut self) -> usize {
        // Wave 636: GameWorld transform writeback records dirty objects; host
        // applies movement/presentation bookkeeping residual.
        let events = crate::game_logic::host_transform_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Transform already writeback-synced; re-record movement residual
            // for presentation / path consumers.
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_weapon_stats_ready_completions(&mut self) -> usize {
        // Wave 635: GameWorld weapon-stats writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_weapon_stats.
        let events = crate::game_logic::host_weapon_stats_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Stats already writeback-synced; re-record host weapon-stats log
            // for presentation / fire-intent consumers.
            obj.record_host_weapon_stats();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_combat_status_ready_completions(&mut self) -> usize {
        // Wave 634: GameWorld combat-status writeback records dirty objects; host
        // applies status presentation residual via host_status_log.
        use crate::game_logic::host_status_log as hsl;
        let events = crate::game_logic::host_combat_status_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            let s = &obj.status;
            let oid = ev.object;
            // Re-record key combat-status flags for presentation consumers.
            // Values already writeback-synced; this is bookkeeping only.
            hsl::record_selected(oid, s.selected);
            hsl::record_attacking(oid, s.attacking);
            hsl::record_moving(oid, s.moving);
            hsl::record_firing(oid, s.is_firing_weapon);
            hsl::record_aiming(oid, s.is_aiming_weapon);
            hsl::record_stealthed(oid, s.stealthed);
            hsl::record_detected(oid, s.detected);
            hsl::record_disabled_emp(oid, s.disabled_emp);
            hsl::record_weapons_jammed(oid, s.weapons_jammed);
            hsl::record_disabled_hacked(oid, s.disabled_hacked);
            hsl::record_disabled_unmanned(oid, s.disabled_unmanned);
            hsl::record_disabled_paralyzed(oid, s.disabled_paralyzed);
            hsl::record_disabled_subdued(oid, s.disabled_subdued);
            hsl::record_masked(oid, s.masked);
            hsl::record_disguised(oid, s.disguised);
            hsl::record_faerie_fire(oid, s.faerie_fire);
            hsl::record_deployed(oid, s.deployed);
            hsl::record_disabled_underpowered(oid, s.disabled_underpowered);
            hsl::record_is_carbomb(oid, s.is_carbomb);
            hsl::record_hijacked(oid, s.hijacked);
            hsl::record_force_attack(oid, obj.force_attack);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_model_condition_ready_completions(&mut self) -> usize {
        // Wave 633: GameWorld model-condition writeback records bit changes; host
        // applies presentation bookkeeping residual (drawable model condition log).
        let events = crate::game_logic::host_model_condition_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_bits == ev.new_bits {
                continue;
            }
            // Bits already writeback-synced; re-record host model-condition log
            // for presentation consumers without recomputing from health.
            if let Some(obj) = self.objects.get(&ev.object) {
                obj.record_host_model_condition();
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_death_type_ready_completions(&mut self) -> usize {
        // Wave 632: GameWorld death-type writeback records ordinal changes; host
        // applies destroy/pilot presentation bookkeeping residual.
        let events = crate::game_logic::host_death_type_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_ordinal == ev.new_ordinal {
                continue;
            }
            // Death type already writeback-synced; re-record host death-type
            // log for presentation / process_destroy consumers.
            if self.objects.contains_key(&ev.object) {
                crate::game_logic::host_death_type_log::record(ev.object, ev.new_ordinal);
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_economy_ready_completions(&mut self) -> usize {
        // Wave 631: GameWorld economy writeback records supply/power/radar/alive
        // changes; host applies presentation residual via host_economy_log and
        // radar log (GW decides absolute values; host still owns UI bookkeeping).
        let events = crate::game_logic::host_economy_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.supplies_changed || ev.power_changed {
                crate::game_logic::host_economy_log::record(
                    ev.player_id,
                    ev.supplies,
                    ev.power_available,
                );
                n = n.saturating_add(1);
            }
            if ev.radar_changed {
                if let Some(player) = self.players.get_mut(&ev.player_id) {
                    // Re-record radar residual for presentation without
                    // re-applying absolute values (already writeback-synced).
                    crate::game_logic::host_radar_log::record(
                        player.id,
                        player.radar_count,
                        player.radar_disabled,
                    );
                    n = n.saturating_add(1);
                }
            }
            let _ = (ev.alive_changed, ev.previous_alive, ev.is_alive);
            let _ = (
                ev.previous_supplies,
                ev.previous_power,
                ev.previous_radar_count,
                ev.previous_radar_disabled,
            );
        }
        n
    }

    pub fn host_apply_ai_state_ready_completions(&mut self) -> usize {
        // Wave 630: GameWorld AI-state writeback records ordinal changes; host
        // applies moving/attacking combat-status residual for the new state.
        let events = crate::game_logic::host_ai_state_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            if ev.previous_ordinal == ev.new_ordinal {
                continue;
            }
            // State already writeback-synced; apply status residual only.
            obj.apply_ai_state_combat_status_residual(ev.new_ordinal);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_owner_ready_completions(&mut self) -> usize {
        // Wave 629: GameWorld owner writeback records team changes; host applies
        // capture residual (kick passengers, deselect, idle, score).
        let events = crate::game_logic::host_owner_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_team == ev.new_team {
                continue;
            }
            // Team already writeback-synced; run capture residual side effects.
            self.on_capture_object_residual(ev.object, ev.previous_team, ev.new_team);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_contain_ready_completions(&mut self) -> usize {
        // Wave 628: GameWorld contain writeback records membership changes;
        // host applies garrison AI residual + enter/exit honesty counters.
        let events = crate::game_logic::host_contain_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Passenger residual: entered a container.
            if ev.previous_contained_by == 0 && ev.new_contained_by != 0 {
                obj.set_ai_state(AIState::Garrisoned);
                obj.set_status_moving(false);
                obj.stop_moving();
                self.record_garrison_residual_enter();
                n = n.saturating_add(1);
                continue;
            }
            // Passenger residual: left a container.
            if ev.previous_contained_by != 0 && ev.new_contained_by == 0 {
                if matches!(obj.ai_state, AIState::Garrisoned | AIState::Entering) {
                    obj.set_ai_state(AIState::Idle);
                }
                self.record_garrison_residual_exit();
                n = n.saturating_add(1);
                continue;
            }
            // Container residual: garrison count rose/fell (honesty only).
            if ev.garrison_list_changed {
                if ev.new_garrison_count > ev.previous_garrison_count {
                    let delta = ev.new_garrison_count - ev.previous_garrison_count;
                    for _ in 0..delta {
                        self.record_garrison_residual_enter();
                    }
                } else if ev.new_garrison_count < ev.previous_garrison_count {
                    let delta = ev.previous_garrison_count - ev.new_garrison_count;
                    for _ in 0..delta {
                        self.record_garrison_residual_exit();
                    }
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_production_door_ready_completions(&mut self) -> usize {
        // Wave 627: GameWorld production-door writeback records phase changes;
        // host applies door model-condition residual for the new phase.
        // apply_production_door_phase_residual maps leftover phase 3 (WAITING_TO_CLOSE)
        // to CLOSING — C++ ProductionUpdate never plays that pose.
        let events = crate::game_logic::host_production_door_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.producer) else {
                continue;
            };
            if obj.apply_production_door_phase_residual(ev.new_phase) {
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_construction_complete_clear_ready_completions(&mut self) -> usize {
        // Wave 626: under construction sole-tick, GameWorld writeback records
        // producers whose CONSTRUCTION_COMPLETE clear deadline elapsed; host clears
        // the model bit and counts residual.
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return 0;
        }
        let events = crate::game_logic::host_construction_complete_clear_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.producer) else {
                continue;
            };
            if obj.apply_construction_complete_clear_residual() {
                self.construction_complete_clears =
                    self.construction_complete_clears.saturating_add(1);
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_radar_extend_ready_completions(&mut self) -> usize {
        // Wave 625: GameWorld radar-extend complete writeback records ready IDs;
        // host applies upgraded model residual and complete counter.
        let events = crate::game_logic::host_radar_extend_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.structure) else {
                continue;
            };
            obj.apply_radar_extend_complete_residual();
            self.radar_extend_completes = self.radar_extend_completes.saturating_add(1);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_upgrade_ready_completions(&mut self) -> usize {
        // Wave 624: under GameWorld completed-upgrade writeback, drain ready log and
        // apply full host upgrade residual (unlocks, EVA, radar, status bits).
        let events = crate::game_logic::host_upgrade_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let team = self.players.get(&ev.player_id).map(|p| p.team).or_else(|| {
                self.players
                    .values()
                    .find(|p| p.id == ev.player_id)
                    .map(|p| p.team)
            });
            let Some(team) = team else {
                continue;
            };
            // Skip if host production path already completed this upgrade.
            use crate::game_logic::host_upgrades::{HostUpgradePhase, normalize_upgrade_identity};
            let key = normalize_upgrade_identity(&ev.upgrade_name);
            let already = self.host_upgrades().entries_snapshot().iter().any(|e| {
                e.player_id == ev.player_id
                    && e.phase == HostUpgradePhase::Completed
                    && normalize_upgrade_identity(&e.name) == key
            });
            if already {
                continue;
            }
            // Ensure player unlocked set tracks PLAYER completions only.
            if let Some(player) = self.players.get_mut(&ev.player_id) {
                player.complete_researched_upgrade(&ev.upgrade_name);
            } else if let Some(player) = self.players.values_mut().find(|p| p.id == ev.player_id) {
                player.complete_researched_upgrade(&ev.upgrade_name);
            }
            self.apply_host_upgrade_complete(team, ev.player_id, &ev.upgrade_name);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_body_damage_ready_completions(&mut self) -> usize {
        // Wave 623: under damage authority, GameWorld body-damage writeback records
        // state transitions; host applies model/FX residual for those IDs.
        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
            return 0;
        }
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let events = crate::game_logic::host_body_damage_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            let prev = HostBodyDamageType::from_ordinal(ev.previous_ordinal);
            let next = HostBodyDamageType::from_ordinal(ev.new_ordinal);
            if prev == next {
                continue;
            }
            obj.apply_body_damage_state_change_residual(prev, next);
            let pos = obj.get_position();
            let yaw = obj.get_orientation();
            let model = obj.thing.template.get_model_name().to_string();
            let scale = obj.thing.template.asset_scale;
            let aflame = obj.has_object_status_bit("AFLAME")
                || obj.fire_spread.as_ref().is_some_and(|f| f.is_aflame());
            let owner = ev.object;
            let ordinal = ev.new_ordinal;
            drop(obj);
            self.combat_particles.replace_body_auto_particles(
                owner,
                pos,
                self.frame,
                ordinal,
                aflame,
                crate::game_logic::combat_particles::BodyAutoParticlePose::new(&model, scale, yaw),
            );
            self.mirror_overlord_addon_damage_to_occupant(owner);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_veterancy_ready_completions(&mut self) -> usize {
        // Wave 622: under damage authority, GameWorld experience writeback records
        // veterancy level-ups; host applies combat bonus residual for those IDs.
        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
            return 0;
        }
        use crate::game_logic::VeterancyLevel as V;
        let events = crate::game_logic::host_veterancy_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            if obj.status.destroyed && !obj.is_alive() {
                continue;
            }
            let prev = match ev.previous_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            let next = match ev.new_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            if next == prev {
                continue;
            }
            // Level already writeback-synced; apply combat residual bonuses.
            obj.apply_veterancy_bonuses(prev, next);
            n = n.saturating_add(1);
        }
        n
    }
}
