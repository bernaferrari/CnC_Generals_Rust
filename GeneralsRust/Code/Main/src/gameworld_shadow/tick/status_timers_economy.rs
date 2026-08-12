//! Status timers: constructing / crates / dozer idle / fire-spread / auto-deposit / hacker.

use super::status_timers::StatusTimerSnapshots;
use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 815–822: construction, crates, dozer, fire-spread, auto-deposit, hacker cash.
    pub(super) fn tick_status_economy(
        &mut self,
        eid: EntityId,
        frame: u32,
        snaps: &StatusTimerSnapshots,
    ) -> bool {
        // The entity is mutably borrowed for the whole timer pass, so capture
        // the reverse player map before borrowing it.  Event consumers need
        // the exact host owner, not merely the faction ordinal.
        let host_player_to_gw = self.host_player_to_gw.clone();
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let owner_player_id = e.owner.and_then(|owner| {
            host_player_to_gw.iter().find_map(|(&host_player_id, &gw_player_id)| {
                (gw_player_id == owner).then_some(host_player_id)
            })
        });
        let mut changed = false;
        let fire_spread_candidates = &snaps.fire_spread_candidates;
        // Wave 815: ACTIVELY_CONSTRUCTING model condition residual.
        {
            use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
            let ac_bit = actively_constructing_model_bit();
            let ac_mask = 1u128 << ac_bit;
            const WORKER_BIT: u32 = 1u32 << 9; // KindOf::Worker
            let alive = e.health > 0.0 && !e.destroyed;
            let name = e.template_name();
            let is_worker = (e.kind_of_bits & WORKER_BIT) != 0
                || name.contains("Dozer")
                || name.contains("Worker")
                || name.contains("dozer")
                || name.contains("worker");
            const STRUCTURE_BIT: u32 = 1u32 << 0;
            let can_construct = is_worker && (e.kind_of_bits & STRUCTURE_BIT) == 0;
            let is_dozer_building = can_construct && matches!(e.ai_state_ordinal, 7 | 8); // Constructing | Repairing
            let is_producing = e.production_queue_len > 0 || !e.production_queue_items.is_empty();
            let has_bit = (e.model_condition_bits & ac_mask) != 0;
            if alive && (can_construct || is_producing || has_bit) {
                let want = is_dozer_building || is_producing;
                let before = e.model_condition_bits;
                if want {
                    e.model_condition_bits |= ac_mask;
                } else {
                    e.model_condition_bits &= !ac_mask;
                }
                if e.model_condition_bits != before {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_actively_constructing_log::record(
                                crate::game_logic::host_actively_constructing_log::ActivelyConstructingEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    model_condition_bits: e.model_condition_bits,
                                    want,
                                },
                            );
                    }
                    changed = true;
                }
            }
        }

        // Wave 817: Money/salvage crate DeletionUpdate residual.
        if e.money_crate && e.money_crate_expires_frame > 0 && frame >= e.money_crate_expires_frame
        {
            e.money_crate = false;
            e.money_crate_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::MoneyCrate,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }

        // Wave 819: Dozer/worker bored idle timer residual.
        {
            const WORKER_BIT: u32 = 1u32 << 9;
            const STRUCTURE_BIT: u32 = 1u32 << 0;
            let alive = e.health > 0.0 && !e.destroyed;
            let name = e.template_name();
            let is_worker = (e.kind_of_bits & WORKER_BIT) != 0
                || name.contains("Dozer")
                || name.contains("Worker")
                || name.contains("dozer")
                || name.contains("worker");
            let can_repair = is_worker && (e.kind_of_bits & STRUCTURE_BIT) == 0;
            if alive && can_repair {
                if e.ai_state_ordinal == 0 {
                    // Idle
                    if e.idle_since_frame == 0 {
                        e.idle_since_frame = frame.max(1);
                        changed = true;
                    }
                    let bored = crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
                    if frame.saturating_sub(e.idle_since_frame) >= bored {
                        // Match host: reset stamp then attempt service acquire on host drain.
                        e.idle_since_frame = frame.max(1);
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_dozer_bored_log::record(
                                crate::game_logic::host_dozer_bored_log::DozerBoredEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                },
                            );
                        }
                        changed = true;
                    }
                } else if e.idle_since_frame != 0 {
                    e.idle_since_frame = 0;
                    changed = true;
                }
            }
        }

        // Wave 820: FireSpread/Flammable residual.
        if e.fire_spread_active {
            use crate::game_logic::host_fire_spread::HostFireSpreadData;
            // Rebuild temp data, tick, write back fields.
            let mut fs = HostFireSpreadData::tree_default();
            fs.active = e.fire_spread_active;
            fs.state = match e.fire_spread_state {
                1 => crate::game_logic::host_fire_spread::HostFlammableState::Aflame,
                2 => crate::game_logic::host_fire_spread::HostFlammableState::Burned,
                _ => crate::game_logic::host_fire_spread::HostFlammableState::Normal,
            };
            fs.aflame_end_frame = e.fire_spread_aflame_end_frame;
            fs.burned_end_frame = e.fire_spread_burned_end_frame;
            fs.next_spread_frame = e.fire_spread_next_spread_frame;
            fs.min_spread_delay = e.fire_spread_min_delay;
            fs.max_spread_delay = e.fire_spread_max_delay;
            fs.spread_try_range = e.fire_spread_try_range;
            fs.aflame_duration = e.fire_spread_aflame_duration;
            fs.burned_delay = e.fire_spread_burned_delay;
            fs.spread_enabled = e.fire_spread_enabled;
            fs.flame_damage_accum = e.fire_spread_flame_damage_accum;
            fs.flame_damage_limit = e.fire_spread_flame_damage_limit;
            let fr = fs.tick_flammable(frame);
            let sr = fs.tick_spread(frame);
            e.fire_spread_state = match fs.state {
                crate::game_logic::host_fire_spread::HostFlammableState::Normal => 0,
                crate::game_logic::host_fire_spread::HostFlammableState::Aflame => 1,
                crate::game_logic::host_fire_spread::HostFlammableState::Burned => 2,
            };
            e.fire_spread_aflame_end_frame = fs.aflame_end_frame;
            e.fire_spread_burned_end_frame = fs.burned_end_frame;
            e.fire_spread_next_spread_frame = fs.next_spread_frame;
            e.fire_spread_flame_damage_accum = fs.flame_damage_accum;
            let mut ignite = None;
            if sr.try_spread {
                let px = e.transform.position.x;
                let pz = e.transform.position.z;
                let range = e.fire_spread_try_range;
                let mut best: Option<(u32, f32)> = None;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    for &(oid, ox, oz, would) in fire_spread_candidates {
                        if oid == hid || !would {
                            continue;
                        }
                        let dx = ox - px;
                        let dz = oz - pz;
                        let dist = (dx * dx + dz * dz).sqrt();
                        if dist <= range && best.map(|(_, d)| dist < d).unwrap_or(true) {
                            best = Some((oid, dist));
                        }
                    }
                    ignite = best.map(|(oid, _)| crate::game_logic::ObjectId(oid));
                }
            }
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                let pos = e.transform.position;
                crate::game_logic::host_fire_spread_log::record(
                    crate::game_logic::host_fire_spread_log::FireSpreadTickEvent {
                        id: crate::game_logic::ObjectId(hid),
                        state: e.fire_spread_state,
                        aflame_end_frame: e.fire_spread_aflame_end_frame,
                        burned_end_frame: e.fire_spread_burned_end_frame,
                        next_spread_frame: e.fire_spread_next_spread_frame,
                        became_burned: fr.became_burned,
                        aflame: fr.aflame,
                        try_spread: sr.try_spread,
                        spawn_embers: sr.spawn_embers,
                        ignite_target: ignite,
                        pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                        spread_try_range: e.fire_spread_try_range,
                        flame_damage_accum: e.fire_spread_flame_damage_accum,
                    },
                );
            }
            changed = true;
        }

        // Wave 821: Black market / oil derrick AutoDeposit residual.
        if e.black_market_building {
            use crate::game_logic::{
                is_legal_black_market_income_source, BLACK_MARKET_DEPOSIT_AMOUNT,
                BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
            let neutral = e.team_ordinal == 255;
            if is_legal_black_market_income_source(alive, constructed, neutral) {
                if e.black_market_next_deposit_frame == 0 {
                    e.black_market_next_deposit_frame =
                        frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1));
                    changed = true;
                } else if frame >= e.black_market_next_deposit_frame {
                    e.black_market_next_deposit_frame =
                        frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1));
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let pos = e.transform.position;
                        crate::game_logic::host_auto_deposit_log::record(
                                crate::game_logic::host_auto_deposit_log::AutoDepositEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    kind: crate::game_logic::host_auto_deposit_log::AutoDepositKind::BlackMarket,
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    owner_player_id,
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount: BLACK_MARKET_DEPOSIT_AMOUNT,
                                    next_deposit_frame: e.black_market_next_deposit_frame,
                                    stealthed: e.stealthed,
                                    detected: e.detected,
                                    supply_lines_boost: 0,
                                },
                            );
                    }
                    changed = true;
                }
            }
        }
        if e.oil_derrick_building {
            use crate::game_logic::{
                is_legal_oil_derrick_income_source, oil_derrick_deposit_amount,
                OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
            let neutral = e.team_ordinal == 255;
            if is_legal_oil_derrick_income_source(alive, constructed, neutral) {
                if e.oil_derrick_next_deposit_frame == 0 {
                    e.oil_derrick_next_deposit_frame =
                        frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1));
                    changed = true;
                } else if frame >= e.oil_derrick_next_deposit_frame {
                    e.oil_derrick_next_deposit_frame =
                        frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1));
                    // Supply lines boost resolved on host drain (player upgrades).
                    let (amount, boost) = oil_derrick_deposit_amount(false);
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let pos = e.transform.position;
                        crate::game_logic::host_auto_deposit_log::record(
                                crate::game_logic::host_auto_deposit_log::AutoDepositEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    kind: crate::game_logic::host_auto_deposit_log::AutoDepositKind::OilDerrick,
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    owner_player_id,
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount,
                                    next_deposit_frame: e.oil_derrick_next_deposit_frame,
                                    stealthed: e.stealthed,
                                    detected: e.detected,
                                    supply_lines_boost: boost,
                                },
                            );
                    }
                    changed = true;
                }
            }
        }

        // Wave 822: China Hacker HackInternet residual cash.
        if e.hacker_unit && e.hacker_hacking {
            use crate::game_logic::{
                cash_amount_for_level, cash_interval_frames, is_legal_hacker_income_source,
                VeterancyLevel,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            let neutral = e.team_ordinal == 255;
            let disabled_hacked = e.disabled_hacked;
            if is_legal_hacker_income_source(alive, neutral, disabled_hacked) {
                let level = match e.veterancy_ordinal {
                    1 => VeterancyLevel::Veteran,
                    2 => VeterancyLevel::Elite,
                    3 => VeterancyLevel::Heroic,
                    _ => VeterancyLevel::Rookie,
                };
                let amount = cash_amount_for_level(level);
                let interval = cash_interval_frames(e.hacker_in_internet_center);
                if e.hacker_next_deposit_frame == 0 {
                    e.hacker_next_deposit_frame = frame.saturating_add(interval.max(1));
                    changed = true;
                } else if frame >= e.hacker_next_deposit_frame {
                    e.hacker_next_deposit_frame = frame.saturating_add(interval.max(1));
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let pos = e.transform.position;
                        crate::game_logic::host_hacker_income_log::record(
                            crate::game_logic::host_hacker_income_log::HackerIncomeEvent {
                                id: crate::game_logic::ObjectId(hid),
                                team: Self::entity_team_from_ordinal(e.team_ordinal),
                                owner_player_id,
                                pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                amount,
                                next_deposit_frame: e.hacker_next_deposit_frame,
                                in_internet_center: e.hacker_in_internet_center,
                                stealthed: e.stealthed,
                                detected: e.detected,
                                veterancy_ordinal: e.veterancy_ordinal,
                                container_radius: 0.0,
                            },
                        );
                    }
                    changed = true;
                }
            }
        }
        changed
    }
}
