//! Host objects `impl GameLogic` — `resources_income`.
//! player resources, deposits, hacker, supply drop. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Resolve the player controlling an object without collapsing two active
    /// players that happen to use the same faction.  A legacy team-only object
    /// may retain the old behavior only when that faction has one unambiguous
    /// active player; an explicit but stale owner fails closed instead.
    pub(in super::super) fn player_owner_for_host_object(&self, object: &Object) -> Option<u32> {
        match object.owner_player_id {
            Some(player_id) => self
                .players
                .get(&player_id)
                .filter(|player| player.is_alive && player.team == object.team)
                .map(|player| player.id),
            None => self.unique_player_id_for_team(object.team),
        }
    }

    /// Resolve owner provenance transported by a deferred event.  Unlike the
    /// old `player_id_for_team` lookup this never chooses the first matching
    /// faction slot.
    pub(in super::super) fn player_owner_for_event(
        &self,
        owner_player_id: Option<u32>,
        team: Team,
    ) -> Option<u32> {
        match owner_player_id {
            Some(player_id) => self
                .players
                .get(&player_id)
                .filter(|player| player.is_alive && player.team == team)
                .map(|player| player.id),
            None => self.unique_player_id_for_team(team),
        }
    }

    pub(in super::super) fn update_player_resources(&mut self, dt: f32) {
        // Calculate power and resource generation for each player.  `Team` is
        // only a faction identity: two USA players must not share a power grid
        // or supply-center income.
        let player_ids: Vec<u32> = self.players.keys().copied().collect();
        for player_id in player_ids {
            let (object_power_produced, power_consumed) = self
                .objects
                .values()
                .filter(|object| self.player_owner_for_host_object(object) == Some(player_id))
                .filter(|object| object.is_constructed() && object.is_alive())
                .fold((0_i32, 0_i32), |(produced, consumed), object| {
                    // C++ Object::friend_adjustPowerForPlayer: disabledness
                    // only affects producers. onDisabledEdge also folds
                    // Overcharge EnergyBonus out of the same production pool.
                    let produced = if object.is_disabled() && object.power_provided > 0 {
                        produced
                    } else {
                        produced.saturating_add(object.power_provided)
                    };
                    (
                        produced,
                        consumed.saturating_add(object.power_consumed.abs()),
                    )
                });

            // C++ Energy is incremental. Disabled producers (including an
            // active Overcharge EnergyBonus) are omitted above, so the
            // capture-while-disabled delta is not applied — onDisabledEdge
            // already stripped that pool before onCapture no-ops.
            let power_produced = object_power_produced;

            let Some(player) = self.players.get_mut(&player_id) else {
                continue;
            };
            player.power_available = power_produced - power_consumed;
            player.power_produced = power_produced;
            player.power_consumed = power_consumed;

            // C++ parity: check if power sabotage timer has expired and clear it
            // Matches C++ Player::update() sabotage recovery logic
            if player.power_sabotaged_till_frame > 0
                && self.frame > player.power_sabotaged_till_frame
            {
                player.power_sabotaged_till_frame = 0;
            }
            // C++ Energy::getProduction / getEnergySupplyRatio return 0 while sabotaged.
            if player.power_sabotaged_till_frame > 0 {
                player.power_produced = 0;
                player.power_available = -power_consumed;
            }

            // Shadow economy channel: effective supplies + power after host tick residual.
            crate::game_logic::host_economy_log::record(
                player.id,
                if crate::gameworld_shadow::gameworld_economy_authority_live() {
                    player.effective_supplies()
                } else {
                    player.resources.supplies
                },
                player.power_available,
            );
        }
    }

    /// GLA Black Market residual cash (AutoDepositUpdate residual).
    ///
    /// Retail FactionBuilding.ini GLABlackMarket:
    /// DepositAmount=20, DepositTiming=2000 ms → 60 logic frames @ 30 FPS.
    /// Floating cash text residual: GUI:AddCash @ pos+Z10, player color | A230.
    /// Fail-closed: not full InGameUI GPU draw / InitialCaptureBonus (retail 0).
    pub(crate) fn apply_auto_deposit_event(
        &mut self,
        ev: crate::game_logic::host_auto_deposit_log::AutoDepositEvent,
    ) {
        use crate::game_logic::host_auto_deposit_log::AutoDepositKind;
        use crate::game_logic::host_oil_derrick::{
            HostAutoDepositFloatingText, oil_derrick_deposit_amount,
            should_display_stealthed_floating_cash,
        };

        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        if ev.amount == 0 {
            return;
        }
        let frame = self.frame;
        let owner_player_id = self.player_owner_for_event(ev.owner_player_id, ev.team);
        let deposited = match ev.kind {
            AutoDepositKind::BlackMarket => {
                // GW already advanced next_deposit_frame; keep registry schedule in lockstep.
                self.black_markets
                    .set_next_deposit(ev.id, ev.next_deposit_frame);
                self.black_markets.force_record_deposit(ev.id, ev.amount)
            }
            AutoDepositKind::OilDerrick => {
                let has_supply_lines = owner_player_id
                    .and_then(|player_id| self.players.get(&player_id))
                    .is_some_and(|player| {
                        player.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
                    });
                let (amount, boost) = oil_derrick_deposit_amount(has_supply_lines);
                self.oil_derricks
                    .set_next_deposit(ev.id, ev.next_deposit_frame);
                let d = self.oil_derricks.force_record_deposit(ev.id, amount, boost);
                if boost > 0 {
                    self.oil_derricks.supply_lines_boost_cash_total = self
                        .oil_derricks
                        .supply_lines_boost_cash_total
                        .saturating_add(boost);
                }
                d
            }
        };
        if deposited == 0 {
            return;
        }
        if let Some(pid) = owner_player_id {
            if let Some(player) = self.get_player_mut(pid) {
                player.credit_supplies(deposited);
                // C++ AutoDepositUpdate.cpp:148 addMoneyEarned(m_depositAmount)
                // — SupplyLines boost is deposited but not scored.
                let earned = match ev.kind {
                    AutoDepositKind::BlackMarket => deposited,
                    AutoDepositKind::OilDerrick => {
                        crate::game_logic::host_oil_derrick::OIL_DERRICK_DEPOSIT_AMOUNT
                            .min(deposited)
                    }
                };
                player.add_money_earned(earned);
            }
        }
        let player_color = owner_player_id
            .and_then(|player_id| self.players.get(&player_id))
            .map(|player| player.color_rgb)
            .unwrap_or((200, 200, 200));
        let is_local = owner_player_id
            .map(|player_id| self.is_local_player(player_id))
            .unwrap_or(false);
        let show = should_display_stealthed_floating_cash(ev.stealthed, ev.detected, is_local);
        let mut float_pos = ev.pos;
        float_pos.y += 10.0;
        match ev.kind {
            AutoDepositKind::BlackMarket => {
                if show {
                    self.black_markets
                        .record_floating_text(HostAutoDepositFloatingText::new(
                            ev.id,
                            float_pos,
                            deposited,
                            player_color,
                            frame,
                            false,
                        ));
                } else {
                    self.black_markets.record_floating_text_suppressed();
                }
            }
            AutoDepositKind::OilDerrick => {
                if show {
                    self.oil_derricks
                        .record_floating_text(HostAutoDepositFloatingText::new(
                            ev.id,
                            float_pos,
                            deposited,
                            player_color,
                            frame,
                            false,
                        ));
                } else {
                    self.oil_derricks.record_floating_text_suppressed();
                }
            }
        }
        // Money::deposit audio is queued by credit_supplies (MoneyDepositSound).
    }

    pub(in super::super) fn update_black_market_deposits(&mut self) {
        use crate::game_logic::host_black_market::{
            BLACK_MARKET_DEPOSIT_AMOUNT, is_black_market_template, is_fake_black_market_template,
            is_legal_black_market_income_source,
        };

        use crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText;

        let frame = self.frame;
        let markets: Vec<(ObjectId, Team, Option<u32>, Vec3, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let fake = is_fake_black_market_template(&obj.template_name);
                let is_bm = fake
                    || obj.is_kind_of(KindOf::FSBlackMarket)
                    || is_black_market_template(&obj.template_name);
                if !is_bm {
                    return None;
                }
                // Track constructed non-neutral markets even while disabled so
                // C++ GameLogic sleepy-skip freezes m_depositOnFrame (no forget).
                let is_neutral = obj.team == Team::Neutral;
                if !is_legal_black_market_income_source(
                    obj.is_alive(),
                    obj.is_constructed() && !obj.status.under_construction,
                    is_neutral,
                    false,
                ) {
                    return None;
                }
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    obj.status.stealthed,
                    obj.status.detected,
                    !fake,
                    obj.is_disabled(),
                ))
            })
            .collect();

        // Forget destroyed markets so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> = markets
            .iter()
            .map(|(id, _, _, _, _, _, _, _)| *id)
            .collect();
        let stale: Vec<ObjectId> = self
            .black_markets
            .next_deposit_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.black_markets.forget(id);
        }

        for (market_id, team, object_owner, pos, stealthed, detected, actual_money, disabled) in
            markets
        {
            // C++ GameLogic.cpp:3715-3718: AutoDepositUpdate is not called while
            // disabled; m_depositOnFrame stays put (freeze, not miss-and-reschedule).
            self.black_markets.ensure_scheduled(market_id, frame);
            if disabled {
                continue;
            }
            // C++ AutoDepositUpdate.cpp:143-149 — only `m_isActualMoney`
            // credits `Money::deposit`; an ActualMoney=No (fake GLA Black
            // Market) still runs its schedule/display but never credits cash.
            // The residual registry observes actual credits, so fake markets
            // must not record deposits here.
            let deposited = if actual_money {
                self.black_markets
                    .try_deposit(market_id, frame, BLACK_MARKET_DEPOSIT_AMOUNT)
            } else {
                0
            };
            if deposited == 0 {
                continue;
            }
            let owner_player_id = self.player_owner_for_event(object_owner, team);
            let player_color = owner_player_id
                .and_then(|player_id| self.players.get(&player_id))
                .map(|player| player.color_rgb)
                .unwrap_or((200, 200, 200));
            let is_local = owner_player_id
                .map(|player_id| self.is_local_player(player_id))
                .unwrap_or(false);
            if actual_money {
                if let Some(player_id) = owner_player_id {
                    if let Some(player) = self.get_player_mut(player_id) {
                        player.credit_supplies(deposited);
                        player.add_money_earned(deposited);
                    }
                }
            }
            // AutoDeposit floating text residual + STEALTHED local display gate.
            // Structure geometry scatter residual (±0.3 major/minor radius).
            use crate::game_logic::host_oil_derrick::{
                OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS, should_display_stealthed_floating_cash,
                structure_floating_text_scatter,
            };
            if should_display_stealthed_floating_cash(stealthed, detected, is_local) {
                let radius = self
                    .objects
                    .get(&market_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    market_id.0.wrapping_add(frame),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.black_markets.record_geometry_scatter();
                self.black_markets
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        market_id,
                        float_pos,
                        deposited,
                        player_color,
                        frame,
                        false,
                    ));
            } else {
                self.black_markets.record_floating_text_suppressed();
            }
        }
    }

    /// Tech Oil Derrick residual cash (AutoDepositUpdate residual).
    ///
    /// Retail CivilianBuilding.ini TechOilDerrick:
    /// DepositAmount=200, DepositTiming=12000 ms → 360 logic frames @ 30 FPS,
    /// InitialCaptureBonus=1000 once when first non-neutral owned,
    /// UpgradedBoost SupplyLines +20, floating cash text residual.
    /// Fail-closed: not full InGameUI GPU draw (STEALTHED local display gate residual closed).
    pub(in super::super) fn update_oil_derrick_deposits(&mut self) {
        use crate::game_logic::host_oil_derrick::{
            HostAutoDepositFloatingText, OIL_DERRICK_INITIAL_CAPTURE_BONUS,
            is_legal_oil_derrick_income_source, is_oil_derrick_template,
            oil_derrick_deposit_amount,
        };

        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        let frame = self.frame;
        // Collect all oil derricks (including neutral — need for stale cleanup / capture detect).
        let derricks: Vec<(ObjectId, Team, Option<u32>, Vec3, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !is_oil_derrick_template(&obj.template_name) {
                    return None;
                }
                let alive = obj.is_alive();
                let constructed = obj.is_constructed() && !obj.status.under_construction;
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    alive,
                    constructed,
                    obj.status.stealthed,
                    obj.status.detected,
                ))
            })
            .collect();

        let live: std::collections::HashSet<ObjectId> = derricks
            .iter()
            .map(|(id, _, _, _, _, _, _, _)| *id)
            .collect();
        let stale: Vec<ObjectId> = self
            .oil_derricks
            .next_deposit_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.oil_derricks.forget(id);
        }

        for (derrick_id, team, object_owner, pos, alive, constructed, stealthed, detected) in
            derricks
        {
            let is_neutral = team == Team::Neutral;
            if is_neutral {
                self.oil_derricks.mark_neutral_owner(derrick_id);
            }
            if !is_legal_oil_derrick_income_source(alive, constructed, is_neutral, false) {
                continue;
            }

            let owner_player_id = self.player_owner_for_event(object_owner, team);
            let player_color = owner_player_id
                .and_then(|player_id| self.players.get(&player_id))
                .map(|player| player.color_rgb)
                .unwrap_or((200, 200, 200));
            let is_local = owner_player_id
                .map(|player_id| self.is_local_player(player_id))
                .unwrap_or(false);
            let has_supply_lines = owner_player_id
                .and_then(|player_id| self.players.get(&player_id))
                .is_some_and(|player| player.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));
            use crate::game_logic::host_oil_derrick::should_display_stealthed_floating_cash;
            let show_float = should_display_stealthed_floating_cash(stealthed, detected, is_local);

            // C++ awardInitialCaptureBonus always resets depositOnFrame on
            // becomingTeamMember(yes), even when InitialCaptureBonus is spent.
            let owner_key = owner_player_id.unwrap_or(u32::MAX);
            if self
                .oil_derricks
                .note_non_neutral_gain(derrick_id, owner_key)
            {
                self.oil_derricks
                    .reschedule_after_capture(derrick_id, frame);
            }
            // InitialCaptureBonus residual: first non-neutral ownership only.
            let bonus = self
                .oil_derricks
                .try_capture_bonus(derrick_id, OIL_DERRICK_INITIAL_CAPTURE_BONUS);
            if bonus > 0 {
                if let Some(player_id) = owner_player_id {
                    if let Some(player) = self.get_player_mut(player_id) {
                        player.credit_supplies(bonus);
                        // C++ AutoDepositUpdate.cpp:103 addMoneyEarned(initialCaptureBonus).
                        player.add_money_earned(bonus);
                    }
                }
                // Capture bonus floating text is not STEALTH-gated in C++ (award path).
                // Structure geometry scatter residual still applies (KINDOF_STRUCTURE).
                use crate::game_logic::host_oil_derrick::{
                    OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS, structure_floating_text_scatter,
                };
                let radius = self
                    .objects
                    .get(&derrick_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    derrick_id.0.wrapping_add(frame).wrapping_add(1),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.oil_derricks.record_geometry_scatter();
                self.oil_derricks
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        derrick_id,
                        float_pos,
                        bonus,
                        player_color,
                        frame,
                        true,
                    ));
            }

            // C++ GameLogic.cpp:3715-3718: skip AutoDepositUpdate while disabled;
            // freeze depositOnFrame (ensure schedule, do not try_deposit).
            let disabled = self
                .objects
                .get(&derrick_id)
                .is_some_and(|obj| obj.is_disabled());
            self.oil_derricks.ensure_scheduled(derrick_id, frame);
            if disabled {
                continue;
            }

            let (amount, boost) = oil_derrick_deposit_amount(has_supply_lines);
            let deposited = self
                .oil_derricks
                .try_deposit(derrick_id, frame, amount, boost);
            if deposited == 0 {
                continue;
            }
            if boost > 0 {
                self.supply_lines_bonus_cash_total =
                    self.supply_lines_bonus_cash_total.saturating_add(boost);
            }
            if let Some(player_id) = owner_player_id {
                if let Some(player) = self.get_player_mut(player_id) {
                    player.credit_supplies(deposited);
                    // C++ AutoDepositUpdate.cpp:148 addMoneyEarned(m_depositAmount)
                    // — not moneyAmount (deposit + SupplyLines boost).
                    player.add_money_earned(deposited.saturating_sub(boost));
                }
            }
            if show_float {
                use crate::game_logic::host_oil_derrick::{
                    OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS, structure_floating_text_scatter,
                };
                let radius = self
                    .objects
                    .get(&derrick_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    derrick_id.0.wrapping_add(frame),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.oil_derricks.record_geometry_scatter();
                self.oil_derricks
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        derrick_id,
                        float_pos,
                        deposited,
                        player_color,
                        frame,
                        false,
                    ));
            } else {
                self.oil_derricks.record_floating_text_suppressed();
            }
        }
    }

    /// China Hacker / Internet Center residual cash (HackInternetAIUpdate residual).
    ///
    /// Retail ChinaInfantry.ini HackInternetAIUpdate:
    /// CashUpdateDelay=2000 ms → 60 frames field; CashUpdateDelayFast=1800 ms → 54
    /// frames inside Internet Center; Regular/Vet/Elite/Heroic = 5/6/8/10.
    /// InternetHackContain residual: hackers contained in FSInternetCenter auto-hack.
    /// Fail-closed: unpack uses authored frames (variation 1.0); pack is cash-stop.
    pub(crate) fn apply_hacker_income_event(
        &mut self,
        ev: crate::game_logic::host_hacker_income_log::HackerIncomeEvent,
    ) {
        use crate::game_logic::host_hacker_income::{
            HostHackerFloatingText, internet_center_floating_text_scatter,
            should_display_hacker_floating_cash,
        };

        if ev.amount == 0 {
            return;
        }
        let frame = self.frame;
        self.hacker_income.mark_hacking(ev.id);
        self.hacker_income
            .set_next_deposit(ev.id, ev.next_deposit_frame);
        let deposited =
            self.hacker_income
                .force_record_deposit(ev.id, ev.amount, ev.in_internet_center);
        if deposited == 0 {
            return;
        }
        let owner_player_id = self.player_owner_for_event(ev.owner_player_id, ev.team);
        if let Some(pid) = owner_player_id {
            if let Some(player) = self.get_player_mut(pid) {
                player.credit_supplies(deposited);
                player.add_money_earned(deposited);
            }
        }
        // C++ awards XP to the hacker, not to a fixed retail template.  The
        // shadow event carries its exact parsed `XpPerCashUpdate` value.
        if let Some(hacker) = self.objects.get_mut(&ev.id) {
            hacker.gain_experience(ev.xp_per_cash_update);
        }
        let is_local = owner_player_id
            .map(|pid| self.is_local_player(pid))
            .unwrap_or(false);
        let show = should_display_hacker_floating_cash(
            ev.stealthed,
            ev.detected,
            is_local,
            ev.in_internet_center,
            false,
            false,
            is_local,
        );
        let mut float_pos = ev.pos;
        float_pos.y += 10.0;
        if show {
            if ev.in_internet_center && ev.container_radius > 0.0 {
                let (dx, dz) = internet_center_floating_text_scatter(
                    ev.id.0.wrapping_add(frame),
                    ev.container_radius,
                    ev.container_radius,
                );
                float_pos.x += dx;
                float_pos.z += dz;
                self.hacker_income.record_ic_scatter();
            }
            self.hacker_income
                .record_floating_text(HostHackerFloatingText::new(
                    ev.id,
                    float_pos,
                    deposited,
                    frame,
                    ev.in_internet_center,
                ));
        } else {
            self.hacker_income.record_floating_text_suppressed();
        }
    }

    /// C++ `ActiveBody::onSubdualChange` passenger walk (leftover
    /// `order_all_passengers_to_idle` / `order_all_passengers_to_hack_internet`).
    pub(crate) fn flush_subdual_passenger_orders(&mut self) {
        use crate::game_logic::host_hacker_income::is_legal_hacker_income_source;
        let mut idle_containers: Vec<ObjectId> = Vec::new();
        let mut hack_containers: Vec<ObjectId> = Vec::new();
        for (id, obj) in &self.objects {
            if obj.status.pending_subdual_passenger_idle {
                idle_containers.push(*id);
            }
            if obj.status.pending_internet_center_resume_hack {
                hack_containers.push(*id);
            }
        }
        for container_id in idle_containers {
            let occupants = match self.objects.get_mut(&container_id) {
                Some(obj) => {
                    let _ = obj.take_pending_subdual_passenger_idle();
                    obj.contained_units()
                }
                None => continue,
            };
            for occ_id in occupants {
                if let Some(occ) = self.objects.get_mut(&occ_id) {
                    occ.status.attacking = false;
                    occ.set_status_force_attack(false);
                    occ.target = None;
                    occ.target_location = None;
                    occ.set_ai_state(AIState::Idle);
                }
                // C++ `aiIdle` leaves HACK_INTERNET (`aiDoCommand` PACKING).
                self.hacker_income.stop_hacking(occ_id);
            }
        }
        let frame = self.frame;
        for container_id in hack_containers {
            let occupants = match self.objects.get_mut(&container_id) {
                Some(obj) => {
                    let _ = obj.take_pending_internet_center_resume_hack();
                    obj.contained_units()
                }
                None => continue,
            };
            for occ_id in occupants {
                let Some((is_hacker, metadata, alive, neutral, disabled_hacked)) =
                    self.objects.get(&occ_id).map(|occ| {
                        (
                            occ.is_kind_of(KindOf::MoneyHacker),
                            occ.thing.template.hack_internet_ai_update,
                            occ.is_alive(),
                            occ.team == Team::Neutral,
                            occ.status.disabled_hacked,
                        )
                    })
                else {
                    continue;
                };
                if !is_hacker {
                    continue;
                }
                let Some(metadata) = metadata else {
                    continue;
                };
                if !is_legal_hacker_income_source(alive, neutral, disabled_hacked) {
                    continue;
                }
                self.hacker_income.ensure_internet_center_hacking(
                    occ_id,
                    frame,
                    metadata.unpack_time_frames,
                    metadata.cash_update_delay_frames(true),
                );
            }
        }
    }

    pub(crate) fn update_hacker_income(&mut self) {
        use crate::game_logic::host_hacker_income::is_legal_hacker_income_source;
        self.flush_subdual_passenger_orders();
        self.tick_hacker_pack_phases();

        let frame = self.frame;

        // Snapshot internet-center membership for container queries.
        let internet_centers: std::collections::HashSet<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let is_ic = obj.thing.template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::InternetHack
                    && obj.thing.template.contain_module.admission
                        == crate::game_logic::ContainAdmission::MoneyHackerOnly
                    && obj.is_constructed()
                    && !obj.status.under_construction
                    && !obj.status.sold;
                if is_ic { Some(*id) } else { None }
            })
            .collect();

        // Collect residual hackers with container / legal gates.
        struct HackerSnap {
            id: ObjectId,
            team: Team,
            owner_player_id: Option<u32>,
            pos: Vec3,
            level: crate::game_logic::VeterancyLevel,
            metadata: crate::game_logic::HackInternetAIUpdateMetadata,
            in_ic: bool,
            contained: bool,
            alive: bool,
            neutral: bool,
            disabled_hacked: bool,
            stealthed: bool,
            detected: bool,
            container_id: Option<ObjectId>,
            container_stealthed: bool,
            container_detected: bool,
            container_team: Team,
            container_owner_player_id: Option<u32>,
            container_radius: f32,
            container_subdued: bool,
            moving: bool,
            ai_state: AIState,
        }
        let hackers: Vec<HackerSnap> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let metadata = obj.thing.template.hack_internet_ai_update?;
                let container = obj.container_id();
                // `InternetHackContain::onContaining` is an exact normal
                // Enter relationship: the source must still be an actual
                // passenger of a parsed InternetHackContain controlled by
                // the same player.  A stale `contained_by` link must not
                // manufacture auto-hacking income.
                let in_ic = container.is_some_and(|cid| {
                    self.objects.get(&cid).is_some_and(|target| {
                        internet_centers.contains(&cid)
                            && target.contained_units().contains(id)
                            && self.normal_enter_controller_matches(obj, target)
                    })
                });
                let (c_stealthed, c_detected, c_team, c_owner_player_id, c_radius, c_subdued) =
                    container
                        .and_then(|cid| self.objects.get(&cid))
                        .map(|c| {
                            (
                                c.status.stealthed,
                                c.status.detected,
                                c.team,
                                c.owner_player_id,
                                c.thing.geometry.radius,
                                c.status.disabled_subdued,
                            )
                        })
                        .unwrap_or((false, false, Team::Neutral, None, 0.0, false));
                Some(HackerSnap {
                    id: *id,
                    team: obj.team,
                    owner_player_id: obj.owner_player_id,
                    pos: obj.get_position(),
                    level: obj.experience.level,
                    metadata,
                    in_ic,
                    contained: container.is_some_and(|cid| {
                        self.objects
                            .get(&cid)
                            .is_some_and(|target| target.contained_units().contains(id))
                    }),
                    alive: obj.is_alive(),
                    neutral: obj.team == Team::Neutral,
                    disabled_hacked: obj.status.disabled_hacked,
                    stealthed: obj.status.stealthed,
                    detected: obj.status.detected,
                    container_id: container,
                    container_stealthed: c_stealthed,
                    container_detected: c_detected,
                    container_team: c_team,
                    container_owner_player_id: c_owner_player_id,
                    container_radius: c_radius,
                    container_subdued: c_subdued,
                    moving: obj.status.moving,
                    ai_state: obj.ai_state.clone(),
                })
            })
            .collect();

        let live: std::collections::HashSet<ObjectId> = hackers.iter().map(|h| h.id).collect();
        let stale: Vec<ObjectId> = self
            .hacker_income
            .tracked_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.hacker_income.forget(id);
        }

        for h in &hackers {
            if !h.alive {
                self.hacker_income.forget(h.id);
                continue;
            }
            // Internet Center residual: auto-start hacking when contained.
            // C++ `onSubdualChange` idles occupants while DISABLED_SUBDUED;
            // do not re-enter HACK_INTERNET until the center clears.
            if h.in_ic && h.container_subdued {
                self.hacker_income.stop_hacking(h.id);
            } else if h.in_ic
                && is_legal_hacker_income_source(h.alive, h.neutral, h.disabled_hacked)
            {
                self.hacker_income.ensure_internet_center_hacking(
                    h.id,
                    frame,
                    h.metadata.unpack_time_frames,
                    h.metadata.cash_update_delay_frames(true),
                );
            }
            // C++ aiDoCommand PACKING: any new move/attack command leaves HACK_INTERNET.
            if !h.in_ic
                && self.hacker_income.is_hacking(h.id)
                && (h.moving
                    || !matches!(
                        h.ai_state,
                        AIState::Idle | AIState::Docked | AIState::Garrisoned
                    ))
            {
                self.hacker_income.stop_hacking(h.id);
                continue;
            }
            // If no longer in IC and never field-started, keep active only if
            // still marked hacking (field residual). Leaving IC mid-hack continues
            // at field interval (C++ uses getCashUpdateDelay each cycle).
            if !self.hacker_income.is_hacking(h.id) {
                continue;
            }
            if !is_legal_hacker_income_source(h.alive, h.neutral, h.disabled_hacked) {
                // C++: DISABLED_HACKED skips deposit but stays in HACK_INTERNET state.
                continue;
            }
            let amount = h.metadata.cash_amount_for_level(h.level);
            let interval = h.metadata.cash_update_delay_frames(h.contained);
            let deposited = self
                .hacker_income
                .try_deposit(h.id, frame, amount, interval, h.in_ic);
            if deposited == 0 {
                continue;
            }
            let owner_player_id = self.player_owner_for_event(h.owner_player_id, h.team);
            if let Some(player_id) = owner_player_id {
                if let Some(player) = self.get_player_mut(player_id) {
                    player.credit_supplies(deposited);
                    player.add_money_earned(deposited);
                }
            }
            // Residual XpPerCashUpdate.
            if let Some(obj) = self.objects.get_mut(&h.id) {
                obj.gain_experience(h.metadata.xp_per_cash_update);
            }
            // STEALTHED local display gate residual (owner + containedBy).
            let owner_local = owner_player_id
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            let container_local = self
                .player_owner_for_event(h.container_owner_player_id, h.container_team)
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            use crate::game_logic::host_hacker_income::{
                internet_center_floating_text_scatter, should_display_hacker_floating_cash,
            };
            let show = should_display_hacker_floating_cash(
                h.stealthed,
                h.detected,
                owner_local,
                h.container_id.is_some() && h.in_ic,
                h.container_stealthed,
                h.container_detected,
                container_local,
            );
            if show {
                let mut float_pos = h.pos;
                if h.in_ic && h.container_radius > 0.0 {
                    let (dx, dz) = internet_center_floating_text_scatter(
                        h.id.0.wrapping_add(frame),
                        h.container_radius,
                        h.container_radius,
                    );
                    float_pos.x += dx;
                    float_pos.z += dz;
                    self.hacker_income.record_ic_scatter();
                }
                self.hacker_income.record_floating_text(
                    crate::game_logic::host_hacker_income::HostHackerFloatingText::new(
                        h.id, float_pos, deposited, frame, h.in_ic,
                    ),
                );
            } else {
                self.hacker_income.record_floating_text_suppressed();
            }
            // C++ HackInternetAIUpdate::doCashUpdate UnitCashPing (object ID), even
            // when floating text is suppressed.
            self.queue_resolved_per_unit_sound(
                h.id,
                crate::game_logic::host_hacker_income::HACKER_UNIT_CASH_PING_AUDIO,
                true,
                false,
                None,
                150,
            );
        }
    }

    /// Residual field command: start HackInternet for selected hacker unit(s).
    /// C++ `hackInternet()` enters UNPACKING then HACK_INTERNET cash delay.
    pub fn start_hacker_internet_hack(&mut self, hacker_id: ObjectId) -> bool {
        use crate::game_logic::host_hacker_income::is_legal_hacker_income_source;
        if self.hacker_income.is_hacking(hacker_id) {
            return false;
        }
        let frame = self.frame;
        let Some(obj) = self.objects.get(&hacker_id) else {
            return false;
        };
        let Some(metadata) = obj.thing.template.hack_internet_ai_update else {
            return false;
        };
        if !is_legal_hacker_income_source(
            obj.is_alive(),
            obj.team == Team::Neutral,
            obj.status.disabled_hacked,
        ) {
            return false;
        }
        self.hacker_income.start_hacking(
            hacker_id,
            frame,
            metadata.unpack_time_frames,
            metadata.cash_update_delay_frames(false),
        );
        if let Some(obj) = self.objects.get_mut(&hacker_id) {
            obj.set_status_moving(false);
            obj.stop_moving();
            obj.set_ai_state(AIState::Idle);
        }
        self.leftover_sa_set_pack_model(hacker_id, true, false, false);
        self.queue_resolved_per_unit_sound(
            hacker_id,
            crate::game_logic::host_hacker_income::HACKER_UNIT_UNPACK_AUDIO,
            true,
            false,
            None,
            150,
        );
        true
    }

    /// C++ `HackInternetAIUpdate::aiDoCommand`: PACKING on any new command.
    pub fn stop_hacker_internet_hack(&mut self, hacker_id: ObjectId) {
        self.hacker_income.stop_hacking(hacker_id);
    }

    fn tick_hacker_pack_phases(&mut self) {
        use crate::game_logic::host_hacker_income::PendingHackerCommand;
        let frame = self.frame;
        let ids = self.hacker_income.tracked_pack_ids();
        let mut replay = Vec::new();
        for id in ids {
            if self.hacker_income.finish_unpack_if_due(id, frame) {
                self.leftover_sa_set_pack_model(id, false, false, true);
            }
            if let Some(pending) = self.hacker_income.take_finished_pack(id, frame) {
                self.leftover_sa_set_pack_model(id, false, false, false);
                replay.push((id, pending));
            }
        }
        for (id, pending) in replay {
            match pending {
                PendingHackerCommand::MoveTo(destination) => {
                    let _ = self.unit_command_move_to(id, destination);
                }
                PendingHackerCommand::Attack(target_id) => {
                    let _ = self.unit_command_attack(id, target_id);
                }
                PendingHackerCommand::Stop => {
                    if let Some(unit) = self.objects.get_mut(&id) {
                        unit.stop();
                        unit.set_ai_state(AIState::Idle);
                    }
                }
            }
        }
    }

    /// America Supply Drop Zone residual: OCL interval queues cargo DeliverPayload.
    ///
    /// Retail FactionBuilding.ini AmericaSupplyDropZone:
    /// MinDelay/MaxDelay=120000 ms → 3600 logic frames @ 30 FPS,
    /// OCL_AmericaSupplyDropZoneCrateDrop → AmericaJetCargoPlane DeliverPayload
    /// with 6× SupplyDropZoneCrate @ $250 (+25 each with Upgrade_AmericaSupplyLines).
    ///
    /// Host residual: when OCL is due, queue a cargo flight (approach delay), then
    /// [`Self::update_deliver_payloads`] spawns crates and credits BuildingPickup cash.
    /// Fail-closed: not full CreateAtEdge aircraft Object / parachute fall physics.
    pub(in super::super) fn update_supply_drop_zone_drops(&mut self) {
        use crate::game_logic::host_deliver_payload::{
            HostDeliverPayloadKind, SUPPLY_DROP_CARGO_APPROACH_AUDIO,
            SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE, SUPPLY_DROP_PAYLOAD_TEMPLATE,
        };
        use crate::game_logic::host_supply_drop_zone::{
            is_legal_supply_drop_zone_income_source, is_supply_drop_zone_template,
        };

        let frame = self.frame;
        let zones: Vec<(ObjectId, Team, Option<u32>, Vec3, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !is_supply_drop_zone_template(&obj.template_name) {
                    return None;
                }
                let is_neutral = obj.team == Team::Neutral;
                if !is_legal_supply_drop_zone_income_source(
                    obj.is_alive(),
                    obj.is_constructed() && !obj.status.under_construction,
                    is_neutral,
                ) {
                    return None;
                }
                // C++ Object::isDisabled — under_construction already filtered.
                Some((
                    *id,
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    obj.is_disabled(),
                ))
            })
            .collect();

        // Forget destroyed zones so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> =
            zones.iter().map(|(id, _, _, _, _)| *id).collect();
        let stale: Vec<ObjectId> = self
            .supply_drop_zones
            .next_drop_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.supply_drop_zones.forget(id);
            self.host_deliver_payloads.cancel_for_source(id);
        }

        for (zone_id, team, object_owner, pos, disabled) in zones {
            // C++ OCLUpdate.cpp:102-106 — freeze m_nextCreationFrame, do not create.
            if disabled {
                self.supply_drop_zones.freeze_timer(zone_id);
                continue;
            }
            if !self.supply_drop_zones.try_start_flight(zone_id, frame) {
                continue;
            }

            // Prefer retail crate template; residual TestSupplyDropZoneCrate otherwise.
            let payload_template = if self.templates.contains_key(SUPPLY_DROP_PAYLOAD_TEMPLATE) {
                SUPPLY_DROP_PAYLOAD_TEMPLATE.to_string()
            } else {
                self.ensure_residual_supply_drop_crate_template();
                SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string()
            };

            let mission_id = self.host_deliver_payloads.queue_for_owner(
                HostDeliverPayloadKind::SupplyDropZoneCrate,
                zone_id,
                team,
                self.player_owner_for_event(object_owner, team),
                pos,
                frame,
                payload_template,
            );

            self.queue_audio_event(
                AudioEventRequest::new(SUPPLY_DROP_CARGO_APPROACH_AUDIO)
                    .with_object(zone_id)
                    .with_position(pos)
                    .with_priority(120),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponMuzzleFlash,
                pos,
                frame,
                Some(zone_id),
                None,
            );

            log::info!(
                "Host SupplyDropZone cargo DeliverPayload mission {} queued at {:?} (frame={})",
                mission_id,
                pos,
                frame
            );
        }
    }

    /// Ensure residual SupplyDropZoneCrate template for cargo DeliverPayload path.
    pub(in super::super) fn ensure_residual_supply_drop_crate_template(&mut self) {
        use crate::game_logic::host_deliver_payload::SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE;
        if self
            .templates
            .contains_key(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE)
        {
            return;
        }
        let mut t = ThingTemplate::new(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Selectable)
            .set_health(1.0)
            .set_cost(0, 0);
        self.templates
            .insert(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string(), t);
    }
}
