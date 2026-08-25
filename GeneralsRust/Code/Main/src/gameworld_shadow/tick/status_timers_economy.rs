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
            host_player_to_gw
                .iter()
                .find_map(|(&host_player_id, &gw_player_id)| {
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
                let origin = e.transform.position;
                let range = e.fire_spread_try_range;
                let mut best: Option<(u32, f32)> = None;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    for &(oid, ox, oy, oz, would) in fire_spread_candidates {
                        if oid == hid || !would {
                            continue;
                        }
                        let dist =
                            crate::game_logic::host_fire_spread::fire_spread_center_3d_distance(
                                glam::Vec3::new(origin.x, origin.y, origin.z),
                                glam::Vec3::new(ox, oy, oz),
                            );
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
                BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
                is_legal_black_market_income_source,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
            let neutral = e.team_ordinal == 255;
            let disabled = e.is_disabled();
            // Keep the ctor-style schedule while disabled (C++ GameLogic.cpp
            // sleepy skip does not call AutoDepositUpdate; m_depositOnFrame stays).
            if is_legal_black_market_income_source(alive, constructed, neutral, false) {
                if e.black_market_next_deposit_frame == 0 {
                    e.black_market_next_deposit_frame =
                        frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1));
                    changed = true;
                } else if !disabled
                    && is_legal_black_market_income_source(alive, constructed, neutral, disabled)
                    && frame >= e.black_market_next_deposit_frame
                {
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
                OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES, is_legal_oil_derrick_income_source,
                oil_derrick_deposit_amount,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
            let neutral = e.team_ordinal == 255;
            let disabled = e.is_disabled();
            if is_legal_oil_derrick_income_source(alive, constructed, neutral, false) {
                if e.oil_derrick_next_deposit_frame == 0 {
                    e.oil_derrick_next_deposit_frame =
                        frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1));
                    changed = true;
                } else if !disabled
                    && is_legal_oil_derrick_income_source(alive, constructed, neutral, disabled)
                    && frame >= e.oil_derrick_next_deposit_frame
                {
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
        // Parsed HackInternetAIUpdate cash. The GameWorld shadow owns the
        // active timer; amounts, delays, and XP were frozen from source
        // module data when the host object was mirrored.
        if e.hacker_unit && e.hacker_hacking {
            // C++ HackInternetAIUpdate::aiDoCommand (HackInternetAIUpdate.cpp:105):
            // PACKING on any command, including InternetHackContain evacuate/exit.
            // Riders are dropped Idle, so a move/attack-only stop leaked cash
            // forever while idle outside. Remirror clears leftover IC schedules;
            // fail-closed here if contain already emptied while still flagged IC.
            // Field HackInternet stays idle-outside and must keep depositing.
            let still_in_ic = e.hacker_in_internet_center && e.contained_by_host != 0;
            if (e.hacker_in_internet_center && e.contained_by_host == 0)
                || (!still_in_ic && (e.moving || e.attacking || e.move_target.is_some()))
            {
                e.hacker_hacking = false;
                e.hacker_next_deposit_frame = 0;
                changed = true;
            } else {
                use crate::game_logic::is_legal_hacker_income_source;
                let alive = e.health > 0.0 && !e.destroyed;
                let neutral = e.team_ordinal == 255;
                let disabled_hacked = e.disabled_hacked;
                if is_legal_hacker_income_source(alive, neutral, disabled_hacked) {
                    // HackInternetState descends through the configured tiers and
                    // falls through to $1 when this real module has all-zero
                    // amounts. Do not substitute retail host constants here.
                    let amount = match e.veterancy_ordinal {
                        3 if e.hacker_heroic_cash_amount != 0 => e.hacker_heroic_cash_amount,
                        3 | 2 if e.hacker_elite_cash_amount != 0 => e.hacker_elite_cash_amount,
                        3 | 2 | 1 if e.hacker_veteran_cash_amount != 0 => {
                            e.hacker_veteran_cash_amount
                        }
                        _ if e.hacker_regular_cash_amount != 0 => e.hacker_regular_cash_amount,
                        _ => 1,
                    };
                    // C++ selects the fast delay for any contained hacker, not
                    // only the particular InternetHackContain presentation state.
                    let contained = e.contained_by_host != 0;
                    let interval = if contained {
                        e.hacker_cash_update_delay_fast_frames
                    } else {
                        e.hacker_cash_update_delay_frames
                    };
                    if e.hacker_next_deposit_frame == 0 {
                        // First schedule includes UNPACKING then cash delay.
                        // Remirror usually bakes this; keep the same formula here.
                        e.hacker_next_deposit_frame =
                            frame.saturating_add(interval).saturating_add(1);
                        changed = true;
                    } else if frame >= e.hacker_next_deposit_frame {
                        e.hacker_next_deposit_frame =
                            frame.saturating_add(interval).saturating_add(1);
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            let pos = e.transform.position;
                            crate::game_logic::host_hacker_income_log::record(
                                crate::game_logic::host_hacker_income_log::HackerIncomeEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    owner_player_id,
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount,
                                    xp_per_cash_update: e.hacker_xp_per_cash_update,
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
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use crate::game_logic::{
        ContainAdmission, ContainModuleKind, ContainModuleMetadata, GameLogic,
        HackInternetAIUpdateMetadata, KindOf, Player, Team, ThingTemplate,
        host_hacker_income::{HACKER_CASH_INTERVAL_FAST_FRAMES, HACKER_CASH_REGULAR},
        host_hacker_income_log,
    };
    use crate::gameworld_shadow::GameWorldShadow;
    use glam::Vec3;

    /// C++ HackInternetAIUpdate.cpp:105 PACKING on evacuate/exit.
    /// Remirror must not keep leftover IC hacking while idle outside.
    #[test]
    fn remirror_and_economy_stop_evac_hacker_idle_outside() {
        host_hacker_income_log::clear();
        let mut logic = GameLogic::new();
        let mut player = Player::new(1, Team::China, "TestChina", true);
        player.resources.supplies = 0;
        logic.add_player(player);

        let mut hacker_t = ThingTemplate::new("GwHackerEvac");
        hacker_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::MoneyHacker)
            .set_health(100.0);
        hacker_t.transport_slot_count = Some(1);
        hacker_t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
            unpack_time_frames: 0,
            pack_time_frames: 0,
            cash_update_delay_frames: 60,
            cash_update_delay_fast_frames: HACKER_CASH_INTERVAL_FAST_FRAMES,
            regular_cash_amount: HACKER_CASH_REGULAR,
            veteran_cash_amount: 6,
            elite_cash_amount: 8,
            heroic_cash_amount: 10,
            xp_per_cash_update: 1.0,
            pack_unpack_variation_factor: 0.0,
        });
        logic.templates.insert("GwHackerEvac".into(), hacker_t);

        let mut ic_t = ThingTemplate::new("GwInternetCenterEvac");
        ic_t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSInternetCenter)
            .set_health(2000.0);
        ic_t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::InternetHack,
            slots: Some(8),
            admission: ContainAdmission::MoneyHackerOnly,
            ..Default::default()
        };
        logic.templates.insert("GwInternetCenterEvac".into(), ic_t);

        let ic = logic
            .create_object("GwInternetCenterEvac", Team::China, Vec3::ZERO)
            .expect("ic");
        if let Some(obj) = logic.host_object_mut(ic) {
            obj.set_status_under_construction(false);
        }
        let hacker = logic
            .create_object("GwHackerEvac", Team::China, Vec3::new(1.0, 0.0, 0.0))
            .expect("hacker");
        assert!(logic.host_object_mut(ic).expect("ic").add_occupant(hacker));
        if let Some(obj) = logic.host_object_mut(hacker) {
            obj.set_contained_by(Some(ic));
            obj.set_ai_state(crate::game_logic::AIState::Docked);
        }

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(hacker).expect("map");
        {
            let e = shadow.world().entity(eid).expect("e");
            assert!(e.hacker_unit);
            assert!(e.hacker_hacking, "contained remirror must start hacking");
            assert!(e.hacker_in_internet_center);
        }

        assert!(logic.evacuate_container_now(ic, false));
        shadow.sync_from_host(&logic);
        {
            let e = shadow.world().entity(eid).expect("e");
            assert!(
                !e.hacker_hacking,
                "remirror after evacuate must PACKING-clear hacking"
            );
            assert!(!e.hacker_in_internet_center);
            assert_eq!(e.contained_by_host, 0);
        }

        host_hacker_income_log::clear();
        shadow.tick_status_timer_expirations(HACKER_CASH_INTERVAL_FAST_FRAMES + 2);
        let deposits = host_hacker_income_log::drain();
        assert!(
            deposits.is_empty(),
            "idle outside after IC evacuate must not deposit: {deposits:?}"
        );
        let e = shadow.world().entity(eid).expect("e");
        assert!(!e.hacker_hacking);
    }
}
