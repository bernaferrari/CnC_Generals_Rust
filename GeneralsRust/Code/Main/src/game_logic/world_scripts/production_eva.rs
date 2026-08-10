//! Host scripts `impl GameLogic` — `production_eva`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! unit training / demo suicide / radar messages / EVA / sell / overcharge
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Host SCIENCE unit-training residual honesty registry.
    pub fn unit_training(
        &self,
    ) -> &crate::game_logic::host_unit_training::HostUnitTrainingRegistry {
        &self.unit_training
    }

    pub fn honesty_unit_training_unlock_ok(&self) -> bool {
        self.unit_training.honesty_unlock_ok()
    }

    pub fn honesty_unit_training_grant_ok(&self) -> bool {
        self.unit_training.honesty_grant_ok()
    }

    pub fn honesty_unit_training_ok(&self) -> bool {
        self.unit_training.honesty_ok()
    }

    /// Host Demo SuicideBomb residual honesty registry.
    pub fn demo_suicide_bomb(
        &self,
    ) -> &crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry {
        &self.demo_suicide_bomb
    }

    pub fn honesty_demo_suicide_bomb_upgrade_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_upgrade_ok()
    }

    pub fn honesty_demo_suicide_bomb_death_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_death_ok()
    }

    pub fn honesty_demo_suicide_bomb_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_host_path_ok()
    }

    /// Apply residual Demo_DestroyedWeapon blast at a Demo SuicideBomb death site.
    pub fn apply_demo_suicide_bomb_death_at(
        &mut self,
        source_id: ObjectId,
        source_team: Team,
        source_pos: Vec3,
    ) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            plan_demo_destroyed_hits, DEMO_SUICIDE_BOMB_AUDIO,
        };

        let candidates: Vec<(ObjectId, Vec3, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                (
                    *id,
                    o.get_position(),
                    o.is_alive(),
                    o.status.under_construction,
                )
            })
            .collect();
        let hits = plan_demo_destroyed_hits(source_id, source_pos, &candidates);
        let mut damage_dealt = 0.0f32;
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        for hit in &hits {
            if let Some(victim) = self.objects.get_mut(&hit.target_id) {
                if !victim.is_alive() {
                    continue;
                }
                damage_dealt += hit.damage.min(victim.health.current.max(0.0));
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate(hit.damage, Some(source_id)) {
                    destroy_ids.push((hit.target_id, source_team));
                }
            }
        }
        let destroyed = destroy_ids.len() as u32;
        self.demo_suicide_bomb
            .record_death_detonation(blast_hits, damage_dealt, destroyed);
        self.queue_audio_event(
            AudioEventRequest::new(DEMO_SUICIDE_BOMB_AUDIO)
                .with_object(source_id)
                .with_position(source_pos)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            source_pos,
            self.frame,
            Some(source_id),
            None,
        );
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Apply residual Demo_SuicideDynamitePackPlusFire blast (SUICIDED residual).
    pub fn apply_demo_plus_fire_death_at(
        &mut self,
        source_id: ObjectId,
        source_team: Team,
        source_pos: Vec3,
    ) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            plan_demo_plus_fire_hits, DEMO_SUICIDE_BOMB_AUDIO, DEMO_SUICIDE_DYNAMITE_PLUS_FIRE,
        };

        let _ = DEMO_SUICIDE_DYNAMITE_PLUS_FIRE; // honesty weapon name residual
        let candidates: Vec<(ObjectId, Vec3, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                (
                    *id,
                    o.get_position(),
                    o.is_alive(),
                    o.status.under_construction,
                )
            })
            .collect();
        let hits = plan_demo_plus_fire_hits(source_id, source_pos, &candidates);
        let mut damage_dealt = 0.0f32;
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        for hit in &hits {
            if let Some(victim) = self.objects.get_mut(&hit.target_id) {
                if !victim.is_alive() {
                    continue;
                }
                damage_dealt += hit.damage.min(victim.health.current.max(0.0));
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate(hit.damage, Some(source_id)) {
                    destroy_ids.push((hit.target_id, source_team));
                }
            }
        }
        let destroyed = destroy_ids.len() as u32;
        self.demo_suicide_bomb
            .record_suicided_detonation(blast_hits, damage_dealt, destroyed);
        self.queue_audio_event(
            AudioEventRequest::new(DEMO_SUICIDE_BOMB_AUDIO)
                .with_object(source_id)
                .with_position(source_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            source_pos,
            self.frame,
            Some(source_id),
            None,
        );
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Issue Demo_Command_TertiarySuicide residual (intentional SUICIDED PlusFire).
    ///
    /// Fail-closed: requires SuicideBomb upgrade + CommandSetUpgrade residual.
    /// Terrorists keep host_terrorist path (not TertiarySuicide).
    pub fn issue_demo_tertiary_suicide(&mut self, unit_id: ObjectId) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            can_issue_demo_tertiary_suicide, command_set_enables_tertiary_suicide,
        };
        use crate::game_logic::host_terrorist::is_terrorist_template;

        let Some(obj) = self.objects.get(&unit_id) else {
            self.demo_suicide_bomb.record_tertiary_suicide_denied();
            return false;
        };
        let is_terrorist = is_terrorist_template(&obj.template_name);
        let can = can_issue_demo_tertiary_suicide(
            &obj.template_name,
            &obj.applied_upgrades,
            obj.is_alive(),
            is_terrorist,
        ) && command_set_enables_tertiary_suicide(obj.command_set_override.as_deref());
        if !can {
            self.demo_suicide_bomb.record_tertiary_suicide_denied();
            return false;
        }

        let source_team = obj.team;
        let source_pos = obj.get_position();
        // Mark before blast so destroy path skips DestroyedWeapon double-fire.
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.demo_suicided_detonating = true;
            obj.record_host_demo_mine_cheer();
            Self::mark_object_destroyed_authority_aware(obj, Some(unit_id));
        }
        self.demo_suicide_bomb.record_tertiary_suicide_issued();
        let _ = self.apply_demo_plus_fire_death_at(unit_id, source_team, source_pos);
        self.mark_object_for_destruction(unit_id, Some(source_team));
        true
    }

    pub fn honesty_demo_suicide_bomb_command_set_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_command_set_ok()
    }

    pub fn honesty_demo_suicide_bomb_suicided_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_suicided_ok()
    }

    pub fn honesty_demo_suicide_bomb_plus_fire_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_plus_fire_path_ok()
    }

    /// Cancel a queued production item by template name (first match).
    pub fn cancel_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let Some(team) = self.objects.get(&producer_id).map(|p| p.team) else {
            return false;
        };
        if !self.players.values().any(|player| player.team == team) {
            return false;
        }

        let mut refund: Option<Resources> = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if let Some(pos) = building
                    .production_queue
                    .iter()
                    .position(|item| item.template_name == template_name)
                {
                    refund = building.cancel_production(pos).map(|item| item.cost);
                }
            }
        }

        if let Some(cost) = refund {
            if let Some(player) = self.get_player_mut_by_team(team) {
                // Economy authority: refund via pending delta + log (GameWorld last-writer).
                player.apply_supply_gain(cost.supplies);
                player.power_available -= cost.power;
                crate::game_logic::host_economy_log::record(
                    player.id,
                    player.effective_supplies(),
                    player.power_available,
                );
            }
            crate::game_logic::host_production_log::record_cancel(
                producer_id,
                template_name.clone(),
            );
            // Wave 485: last cancelled item clears factory exit-delay residual.
            if let Some(producer) = self.objects.get_mut(&producer_id) {
                if let Some(building) = producer.building_data.as_mut() {
                    if building.production_queue.is_empty() && building.exit_delay_remaining > 0.0 {
                        building.exit_delay_remaining = 0.0;
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            0.0,
                        );
                    }
                }
            }
            return true;
        }

        false
    }

    /// Wave 985: host production pause residual (ControlBar empty dual-world queue).
    pub fn set_production_paused(&mut self, producer_id: ObjectId, paused: bool) -> bool {
        let Some(producer) = self.objects.get_mut(&producer_id) else {
            return false;
        };
        let Some(building) = producer.building_data.as_mut() else {
            return false;
        };
        building.set_production_paused(paused);
        true
    }

    /// Cancel every queued production item on a producer and refund the owner.
    pub fn cancel_all_production(&mut self, producer_id: ObjectId) -> bool {
        let Some(team) = self.objects.get(&producer_id).map(|p| p.team) else {
            return false;
        };
        if !self.players.values().any(|player| player.team == team) {
            return false;
        }

        let mut refund = Resources::default();
        let mut cancelled_any = false;
        let mut cancelled_names: Vec<String> = Vec::new();
        let mut cleared_exit_delay = false;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                for item in building.production_queue.drain(..) {
                    refund.supplies = refund.supplies.saturating_add(item.cost.supplies);
                    refund.power += item.cost.power;
                    cancelled_names.push(item.template_name);
                    cancelled_any = true;
                }
                // Wave 485: empty queue clears QueueProductionExitUpdate residual.
                if cancelled_any && building.exit_delay_remaining > 0.0 {
                    building.exit_delay_remaining = 0.0;
                    cleared_exit_delay = true;
                }
            }
        }

        if cancelled_any {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.apply_supply_gain(refund.supplies);
                player.power_available -= refund.power;
                crate::game_logic::host_economy_log::record(
                    player.id,
                    player.effective_supplies(),
                    player.power_available,
                );
            }
            // Wave 484: sole-tick skips per-frame progress log — Cancel refreshes
            // GW producer queue snapshot after host drain (sell/death/cancel-all).
            if cancelled_names.is_empty() {
                crate::game_logic::host_production_log::record_cancel(producer_id, String::new());
            } else {
                for name in cancelled_names {
                    crate::game_logic::host_production_log::record_cancel(producer_id, name);
                }
            }
            // Wave 485: publish exit-delay clear so GW sole-tick does not hold a ghost timer.
            if cleared_exit_delay {
                crate::game_logic::host_production_progress_log::record_exit_delay_only(
                    producer_id,
                    0.0,
                );
            }
        }

        cancelled_any
    }

    /// Snapshot pending radar texts for PresentationFrame (does not drain).
    pub fn radar_notification_snapshot(
        &self,
    ) -> Vec<crate::game_logic::radar_notifications::RadarEntry> {
        self.radar_notifications.snapshot()
    }

    pub fn queue_radar_message<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Generic);
    }

    pub(in super::super) fn queue_script_radar_event(&mut self, event: RadarScriptEventRequest) {
        let position = event.position;
        match event.event_type {
            1 => self.queue_radar_message_at(
                "Construction event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            2 => self.queue_radar_message_at(
                "Upgrade event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            3 => self.queue_radar_attack_at("Under attack", position),
            4 => self.queue_radar_message_at(
                "Radar event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            5 => self.queue_radar_message_at(
                "Beacon pulse",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            6 => self.queue_radar_message_at(
                "Infiltration event",
                position,
                radar_notifications::RadarKind::Attack,
            ),
            7 => self.queue_radar_message_at(
                "Battle plan event",
                position,
                radar_notifications::RadarKind::Ally,
            ),
            8 => self.queue_radar_message_at(
                "Stealth discovered",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            9 => self.queue_radar_message_at(
                "Stealth neutralized",
                position,
                radar_notifications::RadarKind::Attack,
            ),
            10 => {
                self.last_radar_event = Some(RadarEntry {
                    text: "Radar event".to_string(),
                    position,
                    timestamp: self.sim_time_seconds,
                    kind: radar_notifications::RadarKind::Generic,
                });
            }
            _ => {}
        }
    }

    pub fn queue_radar_message_at<S: Into<String>>(
        &mut self,
        message: S,
        position: Vec3,
        kind: radar_notifications::RadarKind,
    ) {
        let kind_index = match kind {
            radar_notifications::RadarKind::Generic => 0,
            radar_notifications::RadarKind::Attack => 1,
            radar_notifications::RadarKind::Ally => 2,
        };
        const RADAR_DEDUP_WINDOW: f32 = 0.5;
        if self.sim_time_seconds - self.last_radar_kind_time[kind_index] < RADAR_DEDUP_WINDOW {
            // Drop duplicate of same kind emitted too fast.
            return;
        }
        let entry = RadarEntry {
            text: message.into(),
            position,
            timestamp: self.sim_time_seconds,
            kind,
        };
        self.radar_notifications.push(entry.clone());
        self.last_radar_event = Some(entry);
        self.last_radar_kind_time[kind_index] = self.sim_time_seconds;

        // Trigger the classic radar/EVA audio cue to mirror the C++ client feedback.
        self.maybe_play_radar_audio("Radar_Event");
    }

    /// Radar attack warning at a location (plays distinct EVA cue).
    pub fn queue_radar_attack_at<S: Into<String>>(&mut self, message: S, position: Vec3) {
        self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Attack);
        self.maybe_play_radar_audio("Radar_Attack");
    }

    /// Radar ally request cue.
    pub fn queue_radar_ally<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Ally);
        self.maybe_play_radar_audio("Radar_Ally");
    }

    /// C++ Radar::tryInfiltrationEvent residual.
    ///
    /// Notifies the **victim** controlling team (local player residual) with
    /// RADAR_EVENT_INFILTRATION + audio honesty. Saboteur / hijack / special
    /// ability paths call this when an enemy structure/vehicle is compromised.

    /// Residual honesty: last radar message text (tests / presentation bridge).
    pub fn last_radar_message_text(&self) -> Option<&str> {
        self.last_radar_event.as_ref().map(|e| e.text.as_str())
    }

    /// C++ Object::isLocallyControlled residual for EVA/radar victim gates.
    pub fn is_object_locally_controlled(&self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        self.players
            .values()
            .any(|p| p.team == obj.team && p.is_local && p.is_alive)
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingSabotaged) when victim is local.

    /// C++ CrateCollide::doSabotageFeedbackFX residual.
    ///
    /// Type-specific MiscAudio cue + Drawable::flashAsSelected on the victim.
    /// Fake buildings skip additional feedback (C++ early return).

    /// C++ SabotageSupplyCenter floating cash text residual:
    /// green GUI:AddCash over saboteur (z+20), red GUI:LoseCash over victim (z+30).
    pub fn spawn_sabotage_cash_floating_texts(
        &mut self,
        saboteur_id: ObjectId,
        victim_id: ObjectId,
        amount: u32,
    ) {
        if amount == 0 {
            return;
        }
        use crate::game_logic::host_money_crate::HostMoneyFloatingText;
        use crate::game_logic::host_saboteur::{
            SABOTEUR_ADD_CASH_COLOR_RGBA, SABOTEUR_ADD_CASH_TEXT_KEY, SABOTEUR_ADD_CASH_Z_OFFSET,
            SABOTEUR_LOSE_CASH_COLOR_RGBA, SABOTEUR_LOSE_CASH_TEXT_KEY,
            SABOTEUR_LOSE_CASH_Z_OFFSET,
        };
        let sab_pos = self
            .objects
            .get(&saboteur_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let vic_pos = self
            .objects
            .get(&victim_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let frame = self.frame;
        // Host world uses Y-up; C++ Coord3D.z is height → map to .y.
        let add = HostMoneyFloatingText {
            text: format!("+${amount}"),
            text_key: SABOTEUR_ADD_CASH_TEXT_KEY.to_string(),
            position: glam::Vec3::new(sab_pos.x, sab_pos.y + SABOTEUR_ADD_CASH_Z_OFFSET, sab_pos.z),
            color_rgba: SABOTEUR_ADD_CASH_COLOR_RGBA,
            amount,
            spawn_frame: frame,
            crate_id: saboteur_id,
            picker_id: victim_id,
        };
        let lose = HostMoneyFloatingText {
            text: format!("-${amount}"),
            text_key: SABOTEUR_LOSE_CASH_TEXT_KEY.to_string(),
            position: glam::Vec3::new(
                vic_pos.x,
                vic_pos.y + SABOTEUR_LOSE_CASH_Z_OFFSET,
                vic_pos.z,
            ),
            color_rgba: SABOTEUR_LOSE_CASH_COLOR_RGBA,
            amount,
            spawn_frame: frame,
            crate_id: victim_id,
            picker_id: saboteur_id,
        };
        self.host_money_crates.record_money_floating_text(add);
        self.host_money_crates.record_money_floating_text(lose);
        self.saboteur.record_cash_floating_texts();
    }

    pub fn do_sabotage_feedback_fx(
        &mut self,
        victim_id: ObjectId,
        kind: crate::game_logic::host_saboteur::SaboteurEffectKind,
    ) {
        use crate::game_logic::host_saboteur::SaboteurEffectKind;
        // Flash first so FakeBuilding still returns without audio but we match
        // C++: FakeBuilding returns before flash. So skip entirely for fake.
        if matches!(kind, SaboteurEffectKind::FakeBuilding) {
            return;
        }
        if let Some(audio) = kind.feedback_audio() {
            let pos = self
                .objects
                .get(&victim_id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            self.queue_audio_event(
                AudioEventRequest::new(audio)
                    .with_object(victim_id)
                    .with_position(pos)
                    .with_priority(170),
            );
        }
        if let Some(obj) = self.objects.get_mut(&victim_id) {
            obj.flash_as_selected();
            self.saboteur.record_flash_as_selected();
        }
        self.saboteur.record_feedback_fx();
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingBeingStolen) when capture prep starts.

    /// C++ TheEva->setShouldPlay(EVA_VehicleStolen) when hijack victim is local.

    /// C++ Object::onDie EVA residual for local non-self-inflicted losses.
    ///
    /// - STRUCTURE + MP_COUNT_FOR_VICTORY-class → EVA_BuildingLost (C++ typo BuldingLost)
    /// - INFANTRY or VEHICLE → EVA_UnitLost + RADAR_EVENT_FAKE residual

    /// C++ Radar::tryUnderAttackEvent residual.
    ///
    /// Throttled by tryEvent distance/time residual. Fires radar attack message,
    /// audio honesty, and EVA BaseUnderAttack / AllyUnderAttack for victory-class
    /// structures owned by local / allied players.

    /// C++ Eva::shouldPlayLowPower residual for the local player.

    /// C++ TheEva->setShouldPlay(EVA_UpgradeComplete) residual (local player).

    /// Classify superweapon residual family from template name.
    /// Returns Some("particle"|"nuke"|"scud") for EVA SuperweaponReady paths.
    pub fn classify_superweapon_eva_kind(template_name: &str) -> Option<&'static str> {
        let n = template_name.to_ascii_lowercase();
        if n.contains("particle") && (n.contains("cannon") || n.contains("uplink")) {
            Some("particle")
        } else if n.contains("scudstorm") || n.contains("scud_storm") {
            Some("scud")
        } else if n.contains("nuclearmissile")
            || n.contains("nuclear_missile")
            || (n.contains("nuke") && n.contains("silo"))
            || n.contains("neutronmissile")
        {
            Some("nuke")
        } else if n.contains("particlecannon") || n.contains("particleuplink") {
            Some("particle")
        } else {
            None
        }
    }

    /// C++ InGameUI SuperweaponReady EVA residual (own/ally/enemy × type).

    /// C++ Player::onStructureConstructionComplete SuperweaponDetected EVA residual.

    /// Map HostSuperweaponKind residual to EVA SuperweaponLaunched family key.
    /// Only ParticleCannon / NuclearMissile / ScudStorm map to C++ launched EVA.
    pub fn classify_superweapon_launched_kind(
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) -> Option<&'static str> {
        use crate::game_logic::special_power_strikes::HostSuperweaponKind;
        match kind {
            HostSuperweaponKind::ParticleCannon => Some("particle"),
            HostSuperweaponKind::NuclearMissile => Some("nuke"),
            HostSuperweaponKind::ScudStorm => Some("scud"),
            _ => None,
        }
    }

    /// C++ SpecialPowerModule SuperweaponLaunched EVA residual (own/ally/enemy × type).

    /// C++ GameLogicDispatch beacon place residual:
    /// EVA_BeaconDetected when local player is ALLIES with the placer (not self).

    /// C++ SpecialPowerModule SuperweaponLaunched GPS Scrambler / Sneak Attack residual.
    ///
    /// `kind`: "gps" | "sneak"
    pub fn try_eva_special_launched_misc(&mut self, owner_team: Team, kind: &str) {
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("gps", "own") => EvaEvent::SuperweaponLaunchedOwnGpsScrambler,
            ("gps", "ally") => EvaEvent::SuperweaponLaunchedAllyGpsScrambler,
            ("gps", _) => EvaEvent::SuperweaponLaunchedEnemyGpsScrambler,
            ("sneak", "own") => EvaEvent::SuperweaponLaunchedOwnSneakAttack,
            ("sneak", "ally") => EvaEvent::SuperweaponLaunchedAllySneakAttack,
            ("sneak", _) => EvaEvent::SuperweaponLaunchedEnemySneakAttack,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_special_launched_misc = self.eva_special_launched_misc.saturating_add(1);
    }

    pub fn honesty_eva_special_launched_misc_ok(&self) -> bool {
        self.eva_special_launched_misc > 0
    }

    pub fn try_eva_beacon_detected(&mut self, placer_player_id: u32) {
        let Some(placer) = self.players.get(&placer_player_id) else {
            return;
        };
        let placer_team = placer.team;
        let placer_alliance = placer.alliance_team;
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        // C++ relationship ALLIES — exclude self / same controlling player.
        if local.id == placer_player_id || local.team == placer_team {
            return;
        }
        let is_ally = local.alliance_team >= 0 && local.alliance_team == placer_alliance;
        if !is_ally {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BeaconDetected,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::BeaconDetected);
        self.eva_beacon_detected = self.eva_beacon_detected.saturating_add(1);
    }

    pub fn honesty_eva_beacon_detected_ok(&self) -> bool {
        self.eva_beacon_detected > 0
    }

    /// C++ stealth detector hero EVA residual (own vs enemy).
    ///
    /// When a stealth hero is newly detected, fire Own* if local owns the hero,
    /// else Enemy* if local is hostile to the hero team.
    pub fn try_eva_hero_detected(&mut self, hero_id: ObjectId) {
        let Some(obj) = self.objects.get(&hero_id) else {
            return;
        };
        if !obj.is_alive() {
            return;
        }
        let name = obj.template_name.to_ascii_lowercase();
        let team = obj.team;
        let kind =
            if crate::game_logic::host_hero_abilities::is_black_lotus_template(&obj.template_name)
                || name.contains("blacklotus")
                || name.contains("black_lotus")
            {
                "lotus"
            } else if name.contains("jarmen") || name.contains("kell") {
                "jarmen"
            } else if name.contains("burton") || name.contains("colonel") {
                "burton"
            } else {
                return;
            };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let is_own = team == local_team;
        let is_ally = !is_own && local_alliance >= 0 && local_alliance == owner_alliance;
        // Enemy residual for non-own non-ally; ally residual fail-closed (no ally EVA names).
        if is_ally {
            return;
        }
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, is_own) {
            ("lotus", true) => EvaEvent::OwnBlackLotusDetected,
            ("lotus", false) => EvaEvent::EnemyBlackLotusDetected,
            ("jarmen", true) => EvaEvent::OwnJarmenKellDetected,
            ("jarmen", false) => EvaEvent::EnemyJarmenKellDetected,
            ("burton", true) => EvaEvent::OwnColonelBurtonDetected,
            ("burton", false) => EvaEvent::EnemyColonelBurtonDetected,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_hero_detected = self.eva_hero_detected.saturating_add(1);
    }

    pub fn honesty_eva_hero_detected_ok(&self) -> bool {
        self.eva_hero_detected > 0
    }

    pub fn try_eva_superweapon_launched(
        &mut self,
        owner_team: Team,
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) {
        let Some(family) = Self::classify_superweapon_launched_kind(kind) else {
            return;
        };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (family, relation) {
            ("particle", "own") => EvaEvent::SuperweaponLaunchedOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponLaunchedAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponLaunchedEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponLaunchedOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponLaunchedAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponLaunchedEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponLaunchedOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponLaunchedAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponLaunchedEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_launched = self.eva_superweapon_launched.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_launched_ok(&self) -> bool {
        self.eva_superweapon_launched > 0
    }

    pub fn try_eva_superweapon_detected(&mut self, owner_team: Team, template_name: &str) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("particle", "own") => EvaEvent::SuperweaponDetectedOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponDetectedAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponDetectedEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponDetectedOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponDetectedAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponDetectedEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponDetectedOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponDetectedAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponDetectedEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_detected = self.eva_superweapon_detected.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_detected_ok(&self) -> bool {
        self.eva_superweapon_detected > 0
    }

    pub fn try_eva_superweapon_ready(
        &mut self,
        _source_id: ObjectId,
        owner_team: Team,
        template_name: &str,
    ) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        // Need a local player to attribute own/ally/enemy residual.
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);

        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };

        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("particle", "own") => EvaEvent::SuperweaponReadyOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponReadyAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponReadyEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponReadyOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponReadyAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponReadyEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponReadyOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponReadyAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponReadyEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_ready = self.eva_superweapon_ready.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_ready_ok(&self) -> bool {
        self.eva_superweapon_ready > 0
    }

    /// C++ ProductionUpdate RADAR_EVENT_UPGRADE + UPGRADE:UpgradeComplete residual.
    ///
    /// Creates a radar event at a producer structure (or team centroid residual)
    /// and queues a localized upgrade-complete radar message for the local player.

    /// C++ structure construction-complete residual feedback for local owner:
    /// radar message + BuildingComplete audio honesty + model condition bit.

    /// Start radar dish extend residual on a newly completed radar provider.
    pub fn maybe_start_radar_extend(&mut self, structure_id: ObjectId) {
        use crate::game_logic::host_radar::is_legal_radar_provider;
        use crate::game_logic::host_radar_stealth_vision_residual::RADAR_EXTEND_TIME_FRAMES_RESIDUAL;
        let Some(obj) = self.objects.get_mut(&structure_id) else {
            return;
        };
        let is_cc = obj.is_command_center() || obj.is_kind_of(KindOf::CommandCenter);
        if !is_legal_radar_provider(obj.is_alive(), true, is_cc, &obj.template_name) {
            return;
        }
        let done = self.frame.saturating_add(RADAR_EXTEND_TIME_FRAMES_RESIDUAL);
        obj.extend_radar(done);
        self.radar_extend_starts = self.radar_extend_starts.saturating_add(1);
    }

    pub fn honesty_radar_extend_start_ok(&self) -> bool {
        self.radar_extend_starts > 0
    }

    pub fn honesty_radar_extend_complete_ok(&self) -> bool {
        self.radar_extend_completes > 0
    }

    /// C++ SpecialPowerModule::onSpecialPowerCreation residual for SW structures.
    ///
    /// Starts full ReloadTime recharge on the structure's PublicTimer power
    /// (ParticleCannon / NuclearMissile / ScudStorm). SharedNSync science powers
    /// are handled separately via `on_special_power_science_creation`.
    pub fn on_structure_superweapon_creation(&mut self, structure_id: ObjectId) {
        use crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure;
        let Some(obj) = self.objects.get(&structure_id) else {
            return;
        };
        if !obj.is_alive() || !obj.is_constructed() {
            return;
        }
        let Some(power) = special_power_for_superweapon_structure(&obj.template_name) else {
            return;
        };
        // Non-shared structure SWs: startPowerRecharge only (not express ready-now).
        if let Some(obj) = self.objects.get_mut(&structure_id) {
            // Retail KindOf POWERED residual for energy-draining SWs (PUC/Nuke).
            if crate::game_logic::host_superweapon_kindof::superweapon_energy_production_for_template(
                &obj.template_name,
            )
            .is_some_and(|e| e < 0)
            {
                obj.thing.template.add_kind_of(KindOf::Powered);
            }
            obj.start_power_recharge(&power);
        }
        let _ = self
            .special_power_strikes
            .reset_timers_for_source_object(structure_id);
    }

    pub fn notify_structure_construction_complete(&mut self, structure_id: ObjectId) {
        let Some(obj) = self.objects.get_mut(&structure_id) else {
            return;
        };
        // C++ ProductionUpdate CONSTRUCTION_COMPLETE + duration residual.
        let now = self.frame.max(1);
        obj.set_construction_complete_condition_at(now);
        let team = obj.team;
        let pos = obj.get_position();
        let name = obj.template_name.clone();
        // NLL ends `obj` borrow after last field copy above.
        // C++ PreorderCreate::onBuildComplete residual.
        let did_preorder = self
            .players
            .values()
            .find(|p| p.team == team && p.is_alive)
            .map(|p| p.did_preorder)
            .unwrap_or(false);
        if crate::game_logic::host_preorder_create::is_preorder_create_template(&name) {
            if let Some(o) = self.objects.get_mut(&structure_id) {
                o.model_condition_bits =
                    crate::game_logic::host_preorder_create::apply_preorder_model_bit(
                        o.model_condition_bits,
                        did_preorder,
                    );
                o.refresh_model_condition_bits();
            }
            if did_preorder {
                self.preorder_create_reg.record_set();
            } else {
                self.preorder_create_reg.record_clear();
            }
        }
        // C++ SpecialPowerCreate → onSpecialPowerCreation (all owners, not local-only).
        self.on_structure_superweapon_creation(structure_id);
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == team);
        if !local {
            self.structure_complete_events = self.structure_complete_events.saturating_add(1);
            return;
        }
        // C++ DozerAIUpdate complete residual: DOZER:ConstructionComplete +
        // VoiceTaskComplete on dozer + RADAR_EVENT_CONSTRUCTION.
        let msg = localization::localize(
            "DOZER:ConstructionComplete",
            &format!("Construction complete: {name}"),
        );
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        self.radar_construction_events = self.radar_construction_events.saturating_add(1);
        // Prefer nearby same-team dozer VoiceTaskComplete residual.
        let dozer_id = self
            .objects
            .iter()
            .find(|(_, o)| {
                o.team == team
                    && o.is_alive()
                    && o.can_construct()
                    && o.get_position().distance(pos) <= 80.0
            })
            .map(|(id, _)| *id);
        if let Some(did) = dozer_id {
            let dpos = self
                .objects
                .get(&did)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            self.queue_audio_event(
                AudioEventRequest::new("VoiceTaskComplete")
                    .with_object(did)
                    .with_position(dpos)
                    .with_priority(155),
            );
        } else {
            self.queue_audio_event(
                AudioEventRequest::new("BuildingComplete")
                    .with_object(structure_id)
                    .with_position(pos)
                    .with_priority(150),
            );
        }
        self.structure_complete_events = self.structure_complete_events.saturating_add(1);
    }

    /// C++ unit production complete residual: VoiceCreated + UnitReady radar for local.
    pub fn notify_unit_production_complete(
        &mut self,
        unit_id: ObjectId,
        producer_id: ObjectId,
        template_name: &str,
    ) {
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let team = unit.team;
        let pos = unit.get_position();
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == team);
        // C++ VoiceCreated on new unit always (all owners).
        self.queue_audio_event(
            AudioEventRequest::new("VoiceCreated")
                .with_object(unit_id)
                .with_position(pos)
                .with_priority(140),
        );
        if local {
            let msg =
                localization::localize("GUI:UnitReady", &format!("Unit ready: {template_name}"));
            self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        }
        let _ = producer_id;
        self.unit_ready_events = self.unit_ready_events.saturating_add(1);
    }

    pub fn honesty_structure_complete_ok(&self) -> bool {
        self.structure_complete_events > 0
    }

    pub fn honesty_radar_construction_event_ok(&self) -> bool {
        self.radar_construction_events > 0
    }

    pub fn honesty_production_door_cycle_ok(&self) -> bool {
        self.production_door_cycles > 0
    }

    /// C++ DozerAIUpdate / ProductionUpdate ACTIVELY_CONSTRUCTING residual.
    ///
    /// - Dozers with AIState::Constructing get the bit set
    /// - Factories with non-empty production queue get the bit set
    /// - Cleared when idle / empty queue
    pub fn update_actively_constructing_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let ac_mask = 1u128 << actively_constructing_model_bit();
        let mut updates = 0u32;
        // Only workers / producers / objects already carrying the bit — skip the
        // rest of Lone Eagle's ~900 decorative props each frame.
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.can_construct()
                        || o.building_data
                            .as_ref()
                            .map(|b| !b.production_queue.is_empty())
                            .unwrap_or(false)
                        || (o.model_condition_bits & ac_mask) != 0)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // C++ DozerAIUpdate: ACTIVELY_CONSTRUCTING for BUILD and REPAIR.
            let is_dozer_building = obj.can_construct()
                && matches!(obj.ai_state, AIState::Constructing | AIState::Repairing);
            let is_producing = obj
                .building_data
                .as_ref()
                .map(|b| !b.production_queue.is_empty())
                .unwrap_or(false);
            let want = is_dozer_building || is_producing;
            // Cheap edge: always set to desired state (idempotent bit ops).
            let bit_before = obj.model_condition_bits;
            obj.set_actively_constructing(want);
            if obj.model_condition_bits != bit_before {
                updates = updates.saturating_add(1);
            }
        }
        if updates > 0 {
            self.actively_constructing_updates =
                self.actively_constructing_updates.saturating_add(updates);
        }
    }

    /// C++ BuildAssistant::sellObject residual — start multi-frame sell process.

    /// C++ Object::setDisabled(DISABLED_UNMANNED) car-bomb dead-man trigger residual.
    ///
    /// If vehicle has WEAPONSET_CARBOMB / IS_CARBOMB, sniping the pilot detonates
    /// it instead of leaving an unmanned car bomb.
    pub fn maybe_detonate_carbomb_on_unmanned(&mut self, vehicle_id: ObjectId) -> bool {
        let is_bomb = self
            .objects
            .get(&vehicle_id)
            .map(|o| o.is_alive() && o.is_car_bomb())
            .unwrap_or(false);
        if !is_bomb {
            return false;
        }
        // Clear unmanned so detonation path owns the object (not recrewable).
        if let Some(o) = self.objects.get_mut(&vehicle_id) {
            o.set_status_disabled_unmanned(false);
            o.status.unmanned_owner_team = None;
        }
        let ok = self.detonate_car_bomb(vehicle_id);
        if ok {
            self.carbomb_unmanned_detonations = self.carbomb_unmanned_detonations.saturating_add(1);
        }
        ok
    }

    /// C++ OverchargeBehavior::enable / toggle residual for China power plants.
    ///
    /// Adjusts power_provided by EnergyBonus when toggling; auto-disable path
    /// is handled by `update_overcharge_drain`.
    pub fn toggle_overcharge_object(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_structure_economy_residual::{
            is_power_plant_template, CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC,
            CHINA_POWER_ENERGY_BONUS,
        };
        let _ = CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        if !is_power_plant_template(&obj.template_name)
            && !obj.is_kind_of(KindOf::PowerPlant)
            && !obj.is_kind_of(KindOf::FSPower)
        {
            return false;
        }
        // C++ NotAllowedWhenHealthBelowPercent residual (China  = typically 0.2?).
        // Use 20% if enabling while critically damaged.
        const NOT_ALLOWED_BELOW: f32 = 0.20;
        let hp_frac = if obj.max_health > 0.0 {
            obj.health.current / obj.max_health
        } else {
            0.0
        };
        if !obj.overcharge_enabled && hp_frac < NOT_ALLOWED_BELOW {
            return false;
        }
        let bonus = CHINA_POWER_ENERGY_BONUS;
        if obj.overcharge_enabled {
            // Disable.
            obj.set_overcharge_enabled(false);
            obj.power_provided = (obj.power_provided - bonus).max(0);
            obj.record_host_entity_power();
            // C++ PowerPlantUpdate::extendRods(FALSE) residual.
            use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
            if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                obj.model_condition_bits &= !(1u128 << bit);
            }
        } else {
            obj.set_overcharge_enabled(true);
            obj.power_provided = obj.power_provided.saturating_add(bonus);
            obj.record_host_entity_power();
            if let Some(bit) =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                    "POWER_PLANT_UPGRADED",
                )
            {
                obj.model_condition_bits |= 1u128 << bit;
            }
        }
        self.overcharge_toggles = self.overcharge_toggles.saturating_add(1);
        true
    }

    /// C++ OverchargeBehavior::update residual — drain HP while overcharge active.
    pub fn update_overcharge_drain(&mut self, dt: f32) {
        use crate::game_logic::host_structure_economy_residual::CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC;
        if dt <= 0.0 {
            return;
        }
        const NOT_ALLOWED_BELOW: f32 = 0.20;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.overcharge_enabled && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_hp = obj.max_health.max(1.0);
            // C++ amount = (maxHealth * percentPerSec) / LOGICFRAMES_PER_SECOND per frame
            // We receive dt seconds so: maxHealth * percentPerSec * dt
            let dmg = max_hp * CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC * dt;
            if dmg > 0.0 {
                let _ = obj.take_damage_from(dmg, Some(id));
            }
            self.overcharge_drain_ticks = self.overcharge_drain_ticks.saturating_add(1);
            let frac = obj.health.current / max_hp;
            let dead = !obj.is_alive() || obj.health.current <= 0.0;
            if dead || frac < NOT_ALLOWED_BELOW {
                // Auto-disable residual (GUI:OverchargeExhausted).
                let bonus =
                    crate::game_logic::host_structure_economy_residual::CHINA_POWER_ENERGY_BONUS;
                obj.set_overcharge_enabled(false);
                obj.power_provided = (obj.power_provided - bonus).max(0);
                obj.record_host_entity_power();
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "POWER_PLANT_UPGRADED",
                    )
                {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
                self.overcharge_exhaustions = self.overcharge_exhaustions.saturating_add(1);
                if dead {
                    self.mark_object_for_destruction(id, None);
                }
            }
        }
    }
}
