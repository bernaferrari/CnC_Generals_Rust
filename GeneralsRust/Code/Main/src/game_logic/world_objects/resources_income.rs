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
            let (power_produced, power_consumed, supply_centers) = self
                .objects
                .values()
                .filter(|object| self.player_owner_for_host_object(object) == Some(player_id))
                .filter(|object| object.is_constructed() && object.is_alive())
                .fold((0_i32, 0_i32, 0_u32), |(produced, consumed, centers), object| {
                    (
                        produced.saturating_add(object.power_provided),
                        consumed.saturating_add(object.power_consumed.abs()),
                        centers.saturating_add(u32::from(object.is_kind_of(KindOf::SupplyCenter))),
                    )
                });

            let Some(player) = self.players.get_mut(&player_id) else {
                continue;
            };
            let mut income_per_second = 0.0f32;

            // Base passive income -- every player earns a small trickle so they are
            // never completely stuck even before building a supply center.
            // In the full C++ game this comes from supply-truck harvesting; here we
            // provide a simplified equivalent so the economy always moves forward.
            income_per_second += 5.0; // $5/sec base passive income

            // $25/sec per owned supply center approximates a single supply
            // truck's delivery rate (full Chinook ~= $600 / 25s).
            income_per_second += supply_centers as f32 * 25.0;

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
            // If power is sabotaged, zero out power production
            if player.power_sabotaged_till_frame > 0 {
                player.power_available = -power_consumed;
            }

            if income_per_second > 0.0 {
                player.income_accumulator += income_per_second * dt;
                let whole = player.income_accumulator.floor() as u32;
                player.income_accumulator -= whole as f32;
                if whole > 0 {
                    player.statistics.resources_collected =
                        player.statistics.resources_collected.saturating_add(whole);
                    if crate::gameworld_shadow::gameworld_economy_authority_live() {
                        player.pending_supply_delta += whole as i64;
                        crate::game_logic::host_economy_log::record(
                            player.id,
                            player.effective_supplies(),
                            player.power_available,
                        );
                    } else {
                        player.resources.supplies = player.resources.supplies.saturating_add(whole);
                        crate::game_logic::host_economy_log::record(
                            player.id,
                            player.resources.supplies,
                            player.power_available,
                        );
                    }
                }
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
        use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_AUDIO;
        use crate::game_logic::host_oil_derrick::{
            oil_derrick_deposit_amount, should_display_stealthed_floating_cash,
            HostAutoDepositFloatingText, OIL_DERRICK_DEPOSIT_AUDIO,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        if ev.amount == 0 {
            return;
        }
        let frame = self.frame;
        let owner_player_id = self.player_owner_for_event(ev.owner_player_id, ev.team);
        let (deposited, audio) = match ev.kind {
            AutoDepositKind::BlackMarket => {
                // GW already advanced next_deposit_frame; keep registry schedule in lockstep.
                self.black_markets
                    .set_next_deposit(ev.id, ev.next_deposit_frame);
                let d = self.black_markets.force_record_deposit(ev.id, ev.amount);
                (d, BLACK_MARKET_DEPOSIT_AUDIO)
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
                (d, OIL_DERRICK_DEPOSIT_AUDIO)
            }
        };
        if deposited == 0 {
            return;
        }
        if let Some(pid) = owner_player_id {
            if let Some(player) = self.get_player_mut(pid) {
                player.credit_supplies(deposited);
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
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(ev.id)
                .with_position(ev.pos)
                .with_priority(120),
        );
    }

    pub(in super::super) fn update_black_market_deposits(&mut self) {
        use crate::game_logic::host_black_market::{
            is_black_market_template, is_legal_black_market_income_source,
            BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_AUDIO,
        };
        use crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText;

        let frame = self.frame;
        let markets: Vec<(ObjectId, Team, Option<u32>, Vec3, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                // Fake black markets residual-skip (ActualMoney=No).
                if obj.template_name.to_ascii_lowercase().contains("fake") {
                    return None;
                }
                let is_bm = obj.is_kind_of(KindOf::FSBlackMarket)
                    || is_black_market_template(&obj.template_name);
                if !is_bm {
                    return None;
                }
                // C++ AutoDepositUpdate: neutral / under construction skip.
                let is_neutral = obj.team == Team::Neutral;
                if !is_legal_black_market_income_source(
                    obj.is_alive(),
                    obj.is_constructed() && !obj.status.under_construction,
                    is_neutral,
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
                ))
            })
            .collect();

        // Forget destroyed markets so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> =
            markets.iter().map(|(id, _, _, _, _, _)| *id).collect();
        let stale: Vec<ObjectId> = self
            .black_markets
            .next_deposit_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.black_markets.forget(id);
        }

        for (market_id, team, object_owner, pos, stealthed, detected) in markets {
            let deposited =
                self.black_markets
                    .try_deposit(market_id, frame, BLACK_MARKET_DEPOSIT_AMOUNT);
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
            if let Some(player_id) = owner_player_id {
                if let Some(player) = self.get_player_mut(player_id) {
                    player.credit_supplies(deposited);
                }
            }
            // AutoDeposit floating text residual + STEALTHED local display gate.
            // Structure geometry scatter residual (±0.3 major/minor radius).
            use crate::game_logic::host_oil_derrick::{
                should_display_stealthed_floating_cash, structure_floating_text_scatter,
                OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
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
            self.queue_audio_event(
                AudioEventRequest::new(BLACK_MARKET_DEPOSIT_AUDIO)
                    .with_object(market_id)
                    .with_position(pos)
                    .with_priority(120),
            );
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
            is_legal_oil_derrick_income_source, is_oil_derrick_template,
            oil_derrick_deposit_amount, HostAutoDepositFloatingText,
            OIL_DERRICK_CAPTURE_BONUS_AUDIO, OIL_DERRICK_DEPOSIT_AUDIO,
            OIL_DERRICK_INITIAL_CAPTURE_BONUS,
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

        let live: std::collections::HashSet<ObjectId> =
            derricks
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

        for (derrick_id, team, object_owner, pos, alive, constructed, stealthed, detected) in derricks {
            let is_neutral = team == Team::Neutral;
            if !is_legal_oil_derrick_income_source(alive, constructed, is_neutral) {
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

            // InitialCaptureBonus residual: first non-neutral ownership.
            let bonus = self
                .oil_derricks
                .try_capture_bonus(derrick_id, OIL_DERRICK_INITIAL_CAPTURE_BONUS);
            if bonus > 0 {
                self.oil_derricks
                    .reschedule_after_capture(derrick_id, frame);
                if let Some(player_id) = owner_player_id {
                    if let Some(player) = self.get_player_mut(player_id) {
                        player.credit_supplies(bonus);
                    }
                }
                // Capture bonus floating text is not STEALTH-gated in C++ (award path).
                // Structure geometry scatter residual still applies (KINDOF_STRUCTURE).
                use crate::game_logic::host_oil_derrick::{
                    structure_floating_text_scatter, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
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
                self.queue_audio_event(
                    AudioEventRequest::new(OIL_DERRICK_CAPTURE_BONUS_AUDIO)
                        .with_object(derrick_id)
                        .with_position(pos)
                        .with_priority(130),
                );
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
                }
            }
            if show_float {
                use crate::game_logic::host_oil_derrick::{
                    structure_floating_text_scatter, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
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
            self.queue_audio_event(
                AudioEventRequest::new(OIL_DERRICK_DEPOSIT_AUDIO)
                    .with_object(derrick_id)
                    .with_position(pos)
                    .with_priority(120),
            );
        }
    }

    /// China Hacker / Internet Center residual cash (HackInternetAIUpdate residual).
    ///
    /// Retail ChinaInfantry.ini HackInternetAIUpdate:
    /// CashUpdateDelay=2000 ms → 60 frames field; CashUpdateDelayFast=1800 ms → 54
    /// frames inside Internet Center; Regular/Vet/Elite/Heroic = 5/6/8/10.
    /// InternetHackContain residual: hackers contained in FSInternetCenter auto-hack.
    /// Fail-closed: not full unpack/pack animation / variation / floating text.
    pub(crate) fn apply_hacker_income_event(
        &mut self,
        ev: crate::game_logic::host_hacker_income_log::HackerIncomeEvent,
    ) {
        use crate::game_logic::host_hacker_income::{
            internet_center_floating_text_scatter, should_display_hacker_floating_cash,
            HostHackerFloatingText, HACKER_CASH_PING_AUDIO,
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
        self.queue_audio_event(
            AudioEventRequest::new(HACKER_CASH_PING_AUDIO)
                .with_object(ev.id)
                .with_position(ev.pos)
                .with_priority(110),
        );
    }

    pub(in super::super) fn update_hacker_income(&mut self) {
        use crate::game_logic::host_hacker_income::{
            is_legal_hacker_income_source, HACKER_CASH_PING_AUDIO,
        };

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
                if is_ic {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        // Collect residual hackers with container / legal gates.
        #[derive(Clone, Copy)]
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
                let (c_stealthed, c_detected, c_team, c_owner_player_id, c_radius) = container
                    .and_then(|cid| self.objects.get(&cid))
                    .map(|c| {
                        (
                            c.status.stealthed,
                            c.status.detected,
                            c.team,
                            c.owner_player_id,
                            c.thing.geometry.radius,
                        )
                    })
                    .unwrap_or((false, false, Team::Neutral, None, 0.0));
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
            if h.in_ic && is_legal_hacker_income_source(h.alive, h.neutral, h.disabled_hacked) {
                self.hacker_income
                    .ensure_internet_center_hacking(
                        h.id,
                        frame,
                        h.metadata.cash_update_delay_frames(true),
                    );
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
            self.queue_audio_event(
                AudioEventRequest::new(HACKER_CASH_PING_AUDIO)
                    .with_object(h.id)
                    .with_position(h.pos)
                    .with_priority(110),
            );
        }
    }

    /// Residual field command: start HackInternet for selected hacker unit(s).
    /// Fail-closed: not full unpack animation / pack-on-interrupt state machine.
    pub fn start_hacker_internet_hack(&mut self, hacker_id: ObjectId) -> bool {
        use crate::game_logic::host_hacker_income::is_legal_hacker_income_source;
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
            metadata.cash_update_delay_frames(false),
        );
        true
    }

    /// Residual: stop HackInternet (e.g. move interrupt residual).
    pub fn stop_hacker_internet_hack(&mut self, hacker_id: ObjectId) {
        self.hacker_income.stop_hacking(hacker_id);
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
        let zones: Vec<(ObjectId, Team, Option<u32>, Vec3)> = self
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
                Some((*id, obj.team, obj.owner_player_id, obj.get_position()))
            })
            .collect();

        // Forget destroyed zones so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> =
            zones.iter().map(|(id, _, _, _)| *id).collect();
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

        for (zone_id, team, object_owner, pos) in zones {
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
