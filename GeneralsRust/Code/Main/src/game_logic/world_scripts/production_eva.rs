//! Host scripts `impl GameLogic` — `production_eva`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! unit training / demo suicide / radar messages / EVA / sell / overcharge
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `ThingTemplate::getVoiceTaskComplete` / `getPerUnitSound("VoiceTaskComplete")`.
fn resolve_dozer_voice_task_complete(template_name: &str) -> Option<String> {
    let factory = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = factory.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let key = "VoiceTaskComplete".to_string();
    if let Some(event) = tmpl.get_per_unit_sound(&key) {
        let name = event.get_event_name();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    tmpl.get_voice_task_complete().and_then(|event| {
        let name = event.get_event_name();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    })
}

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
            DEMO_SUICIDE_BOMB_AUDIO, plan_demo_destroyed_hits,
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
            DEMO_SUICIDE_BOMB_AUDIO, DEMO_SUICIDE_DYNAMITE_PLUS_FIRE, plan_demo_plus_fire_hits,
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

    /// Reconcile player-owned state and refund a production entry removed from
    /// a producer queue.
    ///
    /// Player upgrades have two coupled states in C++, not just the queue
    /// entry: `ProductionUpdate::cancelUpgrade` also removes the player's
    /// `IN_PRODUCTION` upgrade status.  Leaving that status behind makes a
    /// cancelled research item impossible to buy again.
    ///
    /// C++ `ProductionUpdate::cancelUnitCreate` / `cancelUpgrade` deposit to
    /// `getObject()->getControllingPlayer()` (ProductionUpdate.cpp:316, :456).
    /// Fail-closed: an explicit but stale owner is not rewritten to the first
    /// same-faction slot.
    pub(in super::super) fn refund_cancelled_production_item(
        &mut self,
        owner_player_id: Option<u32>,
        team: Team,
        item: &ProductionItem,
    ) {
        let Some(player_id) = self.player_owner_for_event(owner_player_id, team) else {
            return;
        };

        let mut cancelled_upgrade = None;

        if let Some(player) = self.get_player_mut(player_id) {
            if item.is_upgrade() {
                let player_id = player.id;
                if !player.cancel_queued_upgrade(&item.template_name, &item.cost) {
                    // A normal queue always has the matching player state.
                    // If a restored/legacy save left only the producer entry,
                    // still refund its recorded cost rather than deleting paid
                    // research with no recovery path.
                    player.apply_supply_gain(item.cost.supplies);
                    player.power_available -= item.cost.power;
                    crate::game_logic::host_economy_log::record(
                        player.id,
                        player.effective_supplies(),
                        player.power_available,
                    );
                }
                cancelled_upgrade = Some((player_id, item.template_name.clone()));
            } else {
                // Economy authority: refund via pending delta + log (GameWorld
                // remains the last writer for the absolute resource value).
                player.apply_supply_gain(item.cost.supplies);
                player.power_available -= item.cost.power;
                crate::game_logic::host_economy_log::record(
                    player.id,
                    player.effective_supplies(),
                    player.power_available,
                );
            }
        }

        if let Some((player_id, upgrade_name)) = cancelled_upgrade {
            self.record_host_upgrade_cancelled(player_id, &upgrade_name);
        }
    }

    /// Cancel a queued production item by template name (last match).
    ///
    /// C++ cancelUnitCreate is by ProductionID. Name-based callers (hotkey /
    /// legacy HUD) click the newest duplicate icon; first-match would refund
    /// the in-progress head and leave the fresh tail.
    pub fn cancel_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|p| (p.team, p.owner_player_id))
        else {
            return false;
        };
        // C++ cancelUnitCreate requires getControllingPlayer(); do not refund
        // an arbitrary same-faction teammate when ownership is ambiguous.
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        let cancel_pos = self.objects.get(&producer_id).and_then(|producer| {
            producer.building_data.as_ref().and_then(|building| {
                building
                    .production_queue
                    .iter()
                    .rposition(|item| item.template_name.eq_ignore_ascii_case(&template_name))
            })
        });
        if let Some(pos) = cancel_pos {
            self.unreserve_airfield_door_for_cancelled_queue_item(producer_id, pos, &template_name);
        }
        let mut cancelled: Option<ProductionItem> = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if let Some(pos) = cancel_pos {
                    cancelled = building.cancel_production(pos);
                }
            }
        }

        if let Some(item) = cancelled {
            self.refund_cancelled_production_item(owner_player_id, team, &item);
            crate::game_logic::host_production_log::record_cancel(producer_id, item.template_name);
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

    /// Cancel exactly one displayed production-queue slot and refund its owner.
    ///
    /// C++ ControlBar cancellation is positional: duplicate templates can be
    /// queued more than once, and clicking the second icon must leave the first
    /// one intact.  `cancel_production` remains the name-based API used by
    /// older callers; the authoritative HUD bridge uses this index-preserving
    /// variant.
    pub fn cancel_production_at_index(
        &mut self,
        producer_id: ObjectId,
        queue_index: usize,
    ) -> bool {
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|producer| (producer.team, producer.owner_player_id))
        else {
            return false;
        };
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        if let Some(name) = self.objects.get(&producer_id).and_then(|producer| {
            producer
                .building_data
                .as_ref()
                .and_then(|building| building.production_queue.get(queue_index))
                .map(|item| item.template_name.clone())
        }) {
            self.unreserve_airfield_door_for_cancelled_queue_item(producer_id, queue_index, &name);
        }
        let mut cancelled = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                cancelled = building.cancel_production(queue_index);
            }
        }

        let Some(item) = cancelled else {
            return false;
        };

        self.refund_cancelled_production_item(owner_player_id, team, &item);
        crate::game_logic::host_production_log::record_cancel(producer_id, item.template_name);

        // The final cancellation releases the factory door immediately just as
        // the name-based path does; otherwise stale exit-delay state can keep a
        // completed producer visually occupied.
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
        true
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
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|p| (p.team, p.owner_player_id))
        else {
            return false;
        };
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        let mut cancelled_items: Vec<ProductionItem> = Vec::new();
        let mut cancelled_any = false;
        let mut cancelled_names: Vec<String> = Vec::new();
        let mut cleared_exit_delay = false;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                for item in building.production_queue.drain(..) {
                    cancelled_names.push(item.template_name.clone());
                    cancelled_items.push(item);
                    cancelled_any = true;
                }
                // Wave 485: empty queue clears QueueProductionExitUpdate residual.
                if cancelled_any && building.exit_delay_remaining > 0.0 {
                    building.exit_delay_remaining = 0.0;
                    cleared_exit_delay = true;
                }
            }
        }
        self.unreserve_all_airfield_exit_doors(producer_id);

        if cancelled_any {
            for item in &cancelled_items {
                self.refund_cancelled_production_item(owner_player_id, team, item);
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
        // C++ ScriptActions::doRadarCreateEvent — TheRadar->createEvent only.
        // No InGameUI message, no audio. Leftover already matches; this drain
        // is the live handler leftover calls after create_event.
        crate::game_logic::host_radar::host_create_radar_event(
            event.position,
            Self::host_script_radar_event_type(event.event_type),
        );
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

    /// C++ Object::isLocallyControlled — controlling player is local, not faction Team.
    pub fn is_object_locally_controlled(&self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        let owner_id = obj
            .owner_player_id
            .filter(|id| self.players.get(id).is_some_and(|player| player.is_alive))
            .or_else(|| self.unique_player_id_for_team(obj.team));
        owner_id
            .and_then(|id| self.players.get(&id))
            .is_some_and(|player| player.is_local && player.is_alive)
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

    /// Map an already parsed SpecialPower command adapter to the three C++
    /// structure-superweapon EVA families.  This is intentionally separate
    /// from the legacy name helper above: live Object creation/readiness uses
    /// the exact parsed Behavior module, while the string helper remains a
    /// compatibility API for old scripted/test callers.
    fn classify_superweapon_eva_power(
        power: &crate::command_system::SpecialPowerType,
    ) -> Option<&'static str> {
        use crate::command_system::SpecialPowerType as P;
        match power {
            P::ParticleCannon | P::SuperweaponParticleCannon | P::LaserCannon => Some("particle"),
            P::ScudStorm => Some("scud"),
            P::NuclearMissile | P::NukeNeutronMissile | P::SuperweaponNeutronMissile => {
                Some("nuke")
            }
            _ => None,
        }
    }

    fn parsed_superweapon_eva_source(
        &self,
        source_id: ObjectId,
    ) -> Option<(Option<u32>, Team, &'static str)> {
        let obj = self.objects.get(&source_id)?;
        let kind = obj
            .thing
            .template
            .special_power_modules
            .iter()
            .filter(|module| module.public_timer)
            .filter_map(|module| module.command_power.as_ref())
            .find_map(Self::classify_superweapon_eva_power)?;
        Some((obj.owner_player_id, obj.team, kind))
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

    fn eva_local_player_id(&self) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.is_local && player.is_alive)
            .map(|player| player.id)
    }

    /// Owner id wins; otherwise the unique player for `owner_team`.
    /// Never first-of-faction — two USA slots stay unambiguous.
    fn eva_resolve_owner_player_id(
        &self,
        owner_player_id: Option<u32>,
        owner_team: Team,
    ) -> Option<u32> {
        owner_player_id
            .filter(|id| self.players.get(id).is_some_and(|player| player.is_alive))
            .or_else(|| self.unique_player_id_for_team(owner_team))
    }

    /// C++ Superweapon Detected/Launched/Ready:
    /// controllingPlayer == local → own; else getRelationship != ENEMIES → ally
    /// (NEUTRAL counted as ally); else enemy.
    fn eva_own_ally_enemy(
        &self,
        owner_player_id: Option<u32>,
        owner_team: Team,
    ) -> Option<&'static str> {
        let local_id = self.eva_local_player_id()?;
        let Some(owner_id) = self.eva_resolve_owner_player_id(owner_player_id, owner_team) else {
            return Some("enemy");
        };
        if owner_id == local_id {
            return Some("own");
        }
        Some(match self.player_relationship(local_id, owner_id) {
            gamelogic::common::Relationship::Enemies => "enemy",
            _ => "ally",
        })
    }

    /// C++ SpecialPowerModule SuperweaponLaunched EVA residual (own/ally/enemy × type).

    /// C++ GameLogicDispatch beacon place residual:
    /// EVA_BeaconDetected when local player is ALLIES with the placer (not self).

    /// C++ SpecialPowerModule SuperweaponLaunched GPS Scrambler / Sneak Attack residual.
    ///
    /// `kind`: "gps" | "sneak"
    pub fn try_eva_special_launched_misc(&mut self, owner_team: Team, kind: &str) {
        self.try_eva_special_launched_misc_owned(None, owner_team, kind);
    }

    pub fn try_eva_special_launched_misc_owned(
        &mut self,
        owner_player_id: Option<u32>,
        owner_team: Team,
        kind: &str,
    ) {
        let Some(relation) = self.eva_own_ally_enemy(owner_player_id, owner_team) else {
            return;
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
        let Some(local_id) = self.eva_local_player_id() else {
            return;
        };
        // C++ GameLogicDispatch.cpp:1632 — getRelationship(placer default team) == ALLIES.
        // PlayerList seeds self-relationship as ALLIES, so own place also plays EVA.
        if self.player_relationship(local_id, placer_player_id)
            != gamelogic::common::Relationship::Allies
        {
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
        let owner_player_id = obj.owner_player_id;
        let team = obj.team;
        let Some(local_id) = self.eva_local_player_id() else {
            return;
        };
        let Some(owner_id) = self.eva_resolve_owner_player_id(owner_player_id, team) else {
            return;
        };
        self.try_eva_hero_detected_kind(hero_id, owner_id == local_id);
    }

    /// C++ StealthDetectorUpdate.cpp:233-237 Enemy* / :269-274 Own*.
    /// `want_own` selects OwnDetectionEvaEvent vs EnemyDetectionEvaEvent.
    /// Callers must already be inside Radar tryEvent `doFeedback`.
    pub(crate) fn try_eva_hero_detected_kind(&mut self, hero_id: ObjectId, want_own: bool) {
        let Some(obj) = self.objects.get(&hero_id) else {
            return;
        };
        if !obj.is_alive() {
            return;
        }
        let name = obj.template_name.to_ascii_lowercase();
        let team = obj.team;
        let owner_player_id = obj.owner_player_id;
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
        let Some(local_id) = self.eva_local_player_id() else {
            return;
        };
        let Some(owner_id) = self.eva_resolve_owner_player_id(owner_player_id, team) else {
            return;
        };
        let is_own = owner_id == local_id;
        if is_own != want_own {
            return;
        }
        // C++ StealthDetectorUpdate: Own* only for local controller, Enemy* only
        // when the detector/victim pair is not ALLIES. No ally hero EVA names.
        if !is_own
            && self.player_relationship(local_id, owner_id)
                == gamelogic::common::Relationship::Allies
        {
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
        self.try_eva_superweapon_launched_owned(None, owner_team, kind);
    }

    pub fn try_eva_superweapon_launched_owned(
        &mut self,
        owner_player_id: Option<u32>,
        owner_team: Team,
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) {
        let Some(family) = Self::classify_superweapon_launched_kind(kind) else {
            return;
        };
        let Some(relation) = self.eva_own_ally_enemy(owner_player_id, owner_team) else {
            return;
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
        self.try_eva_superweapon_detected_kind(None, owner_team, kind);
    }

    /// Live construction path: exact parsed module authority, never a
    /// superweapon-shaped object name.
    pub fn try_eva_superweapon_detected_for_source(&mut self, source_id: ObjectId) {
        let Some((owner_player_id, owner_team, kind)) =
            self.parsed_superweapon_eva_source(source_id)
        else {
            return;
        };
        self.try_eva_superweapon_detected_kind(owner_player_id, owner_team, kind);
    }

    fn try_eva_superweapon_detected_kind(
        &mut self,
        owner_player_id: Option<u32>,
        owner_team: Team,
        kind: &'static str,
    ) {
        let Some(relation) = self.eva_own_ally_enemy(owner_player_id, owner_team) else {
            return;
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

    /// C++ InGameUI.cpp:3513 `!info->m_hiddenByScript && !info->m_hiddenByScience`.
    /// Global `m_superweaponHiddenByScript` is draw-only — do not consult
    /// `script_superweapon_display_enabled` here.
    fn eva_superweapon_ready_timer_hidden(&mut self, source_id: ObjectId) -> bool {
        if self.script_superweapon_hidden_objects.contains(&source_id) {
            return true;
        }
        self.latch_eva_superweapon_science_hidden(source_id)
    }

    /// C++ SuperweaponInfo::m_hiddenByScience is captured at addSuperweapon
    /// (InGameUI.cpp:559) and never cleared (srj :568-570).
    fn latch_eva_superweapon_science_hidden(&mut self, source_id: ObjectId) -> bool {
        if let Some(&hidden) = self.eva_superweapon_science_hidden.get(&source_id) {
            return hidden;
        }
        if !self.objects.contains_key(&source_id) {
            return false;
        }
        let hidden = self.compute_eva_hidden_by_science_now(source_id);
        self.eva_superweapon_science_hidden
            .insert(source_id, hidden);
        hidden
    }

    #[cfg(test)]
    fn eva_science_hidden_latched(&self, source_id: ObjectId) -> bool {
        self.eva_superweapon_science_hidden
            .get(&source_id)
            .copied()
            .unwrap_or(false)
    }

    #[cfg(test)]
    fn clear_eva_science_hidden_latch(&mut self, source_id: ObjectId) {
        self.eva_superweapon_science_hidden.remove(&source_id);
    }

    fn compute_eva_hidden_by_science_now(&self, source_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&source_id) else {
            return false;
        };
        let required = obj
            .thing
            .template
            .special_power_modules
            .iter()
            .filter(|module| module.public_timer)
            .filter(|module| {
                module
                    .command_power
                    .as_ref()
                    .and_then(Self::classify_superweapon_eva_power)
                    .is_some()
            })
            .find_map(|module| {
                module
                    .required_science
                    .as_deref()
                    .filter(|science| !science.is_empty())
                    .or_else(|| {
                        module.command_power.as_ref().and_then(
                            crate::game_logic::host_special_power_enum_residual::special_power_required_science,
                        )
                    })
            });
        let Some(req) = required else {
            return false;
        };
        match obj.owner_player_id.and_then(|id| self.players.get(&id)) {
            Some(player) => !player.has_unlocked_science(req),
            None => true,
        }
    }

    fn eva_player_superweapon_ready_hidden(&mut self, owner_player_id: u32, kind: &str) -> bool {
        let matching: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.owner_player_id != Some(owner_player_id) {
                    return None;
                }
                let obj_kind = obj
                    .thing
                    .template
                    .special_power_modules
                    .iter()
                    .filter(|module| module.public_timer)
                    .filter_map(|module| module.command_power.as_ref())
                    .find_map(Self::classify_superweapon_eva_power);
                (obj_kind == Some(kind)).then_some(*id)
            })
            .collect();
        if matching.is_empty() {
            return false;
        }
        matching
            .into_iter()
            .all(|id| self.eva_superweapon_ready_timer_hidden(id))
    }

    pub fn try_eva_superweapon_ready(
        &mut self,
        source_id: ObjectId,
        owner_team: Team,
        template_name: &str,
    ) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        if self.eva_superweapon_ready_timer_hidden(source_id) {
            return;
        }
        let owner_player_id = self
            .objects
            .get(&source_id)
            .and_then(|obj| obj.owner_player_id);
        self.try_eva_superweapon_ready_kind(owner_player_id, owner_team, kind);
    }

    pub fn try_eva_superweapon_ready_for_player(
        &mut self,
        owner_player_id: u32,
        template_name: &str,
    ) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        let owner_team = self
            .players
            .get(&owner_player_id)
            .map(|player| player.team)
            .unwrap_or(Team::Neutral);
        if self.eva_player_superweapon_ready_hidden(owner_player_id, kind) {
            return;
        }
        self.try_eva_superweapon_ready_kind(Some(owner_player_id), owner_team, kind);
    }

    /// Live cooldown-ready path: resolve family from the source's parsed
    /// module list rather than its template basename.
    pub fn try_eva_superweapon_ready_for_source(&mut self, source_id: ObjectId) {
        let Some((owner_player_id, owner_team, kind)) =
            self.parsed_superweapon_eva_source(source_id)
        else {
            return;
        };
        if self.eva_superweapon_ready_timer_hidden(source_id) {
            return;
        }
        self.try_eva_superweapon_ready_kind(owner_player_id, owner_team, kind);
    }

    fn try_eva_superweapon_ready_kind(
        &mut self,
        owner_player_id: Option<u32>,
        owner_team: Team,
        kind: &'static str,
    ) {
        let Some(relation) = self.eva_own_ally_enemy(owner_player_id, owner_team) else {
            return;
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
    /// radar message + authored dozer getVoiceTaskComplete + model condition bit.

    /// C++ `RadarUpgrade::upgradeImplementation` → `RadarUpdate::extendRadar`.
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
    /// Walks every `getSpecialPower()` module — including SharedNSync and
    /// ScriptedSpecialPowerOnly. SharedNSync then expresses ready-now on the
    /// player timer (Dustin: start ready to fire). StartsPaused still applies
    /// after startPowerRecharge.
    pub fn on_structure_superweapon_creation(&mut self, structure_id: ObjectId) {
        let Some(obj) = self.objects.get(&structure_id) else {
            return;
        };
        if !obj.is_alive() || !obj.is_constructed() {
            return;
        }
        let owner_id = self.player_owner_for_host_object(obj);
        // C++ SpecialPowerCreate walks every getSpecialPower() behavior — not a type whitelist.
        let modules: Vec<_> = obj.thing.template.special_power_modules.clone();
        if modules.is_empty() {
            return;
        }
        // C++ `SpecialPowerModule::onSpecialPowerCreation` starts the authored
        // reload *before* applying StartsPaused, then expresses SharedNSync
        // ready-now (and a second CC therefore resets a ticking A10 timer).
        let mut shared_ready = Vec::new();
        if let Some(obj) = self.objects.get_mut(&structure_id) {
            for module in modules.iter() {
                let Some(power) = module.command_power.as_ref() else {
                    continue;
                };
                obj.start_power_recharge_with_frames(power, module.reload_time_frames);
                if module.shared_n_sync {
                    shared_ready.push(power.clone());
                }
                // C++ SpecialPowerCreate::onBuildComplete → onSpecialPowerCreation
                // pauseCountdown(TRUE). Units without that Create module only get
                // the ctor pause from init_starts_paused_special_powers.
                if module.starts_paused && obj.thing.template.has_special_power_create {
                    obj.pause_special_power_countdown(power, true);
                }
            }
        }
        if let Some(player_id) = owner_id {
            if let Some(player) = self.players.get_mut(&player_id) {
                for power in &shared_ready {
                    player.express_shared_special_power_ready_now(power);
                }
            }
        }
        let _ = self
            .special_power_strikes
            .reset_timers_for_source_object(structure_id);
        let _ = self.latch_eva_superweapon_science_hidden(structure_id);
    }

    /// C++ Object::updateUpgradeModules — walk the player's completed PLAYER
    /// upgrades onto a building that just left UNDER_CONSTRUCTION.
    pub(in super::super) fn apply_researched_player_upgrades_to_object(
        &mut self,
        structure_id: ObjectId,
    ) {
        let Some(obj) = self.objects.get(&structure_id) else {
            return;
        };
        if obj.status.under_construction {
            return;
        }
        let Some(player_id) = self.player_owner_for_host_object(obj) else {
            return;
        };
        let upgrades: Vec<String> = self
            .players
            .get(&player_id)
            .map(|player| {
                let mut names: Vec<String> = player.completed_upgrades.iter().cloned().collect();
                // Research complete lands in unlocked_sciences; completed_upgrades
                // is only filled by add_completed_upgrade / GrantUpgradeCreate.
                for name in &player.unlocked_sciences {
                    if !names
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(name))
                    {
                        names.push(name.clone());
                    }
                }
                names
            })
            .unwrap_or_default();
        for name in upgrades {
            // C++ Object::updateUpgradeModules still walks Type=OBJECT
            // PowerPlantUpgrade (TriggeredBy Advanced Control Rods) so a plant
            // built after research gets EnergyBonus + extendRods.
            if crate::game_logic::host_upgrades::HostUpgradeKind::from_name(&name)
                == crate::game_logic::host_upgrades::HostUpgradeKind::AdvancedControlRods
            {
                self.apply_advanced_control_rods_to_object(structure_id, &name);
                continue;
            }
            if crate::game_logic::host_upgrades::is_object_scoped_upgrade(&name) {
                continue;
            }
            self.apply_upgrade_to_object(structure_id, &name);
        }
    }

    pub fn notify_structure_construction_complete(&mut self, structure_id: ObjectId) {
        let Some(obj) = self.objects.get_mut(&structure_id) else {
            return;
        };
        // C++ ProductionUpdate CONSTRUCTION_COMPLETE + duration residual.
        let now = self.frame.max(1);
        obj.set_construction_complete_condition_at(now);
        obj.stamp_partition_value_threat();
        let team = obj.team;
        let pos = obj.get_position();
        let name = obj.template_name.clone();
        // C++ CreateModules onBuildComplete (Preorder/GrantUpgrade/LockWeapon/SP/Supply).
        self.apply_create_modules_on_build_complete(structure_id);
        // C++ Object::updateUpgradeModules after UNDER_CONSTRUCTION clears
        // (DozerAIUpdate.cpp:539-591 / Object.cpp:2410-2438).
        self.apply_researched_player_upgrades_to_object(structure_id);
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
        crate::game_logic::host_radar::host_create_radar_event(
            pos,
            game_engine::common::system::radar::RadarEventType::Construction,
        );
        self.radar_construction_events = self.radar_construction_events.saturating_add(1);
        // C++ DozerAIUpdate.cpp:597-617 — local dozer getVoiceTaskComplete only.
        // Never the slot token or invented BuildingComplete.
        let dozer = self.objects.iter().find(|(_, o)| {
            o.team == team
                && o.is_alive()
                && o.can_construct()
                && o.get_position().distance(pos) <= 80.0
        });
        if let Some((&did, obj)) = dozer {
            let dpos = obj.get_position();
            let tmpl = obj.template_name.clone();
            if let Some(event) = resolve_dozer_voice_task_complete(&tmpl) {
                self.queue_audio_event(
                    AudioEventRequest::new(&event)
                        .with_object(did)
                        .with_position(dpos)
                        .with_priority(155),
                );
            }
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
        // C++ getVoiceCreated() — authored Voice.ini name, not the slot token.
        if let Some(event) = crate::game_logic::audio_dispatch_impl::resolve_unit_voice_event(
            template_name,
            crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Created,
        ) {
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(unit_id)
                    .with_position(pos)
                    .with_priority(140),
            );
        }
        let _ = local;
        // C++ ProductionUpdate.cpp:819-825 create onBuildComplete after spawn,
        // including SpecialPowerCreate::startPowerRecharge.
        self.apply_create_modules_on_build_complete(unit_id);
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
    /// - Dozers get the bit only while Constructing/Repairing **and at the dock**
    ///   (DozerAIUpdate.cpp:511/670). Driving to the site stays un-animated.
    /// - Factories with non-empty production queue get the bit set
    /// - Cleared when idle / empty queue
    pub fn update_actively_constructing_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let ac_mask = 1u128 << actively_constructing_model_bit();
        let mut updates = 0u32;
        // C++ DozerAIUpdate.cpp:511/670: ACTIVELY_CONSTRUCTING only while AT the dock.
        let dozer_at_site: std::collections::HashMap<ObjectId, bool> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && o.can_construct()
                    && matches!(o.ai_state, AIState::Constructing | AIState::Repairing)
            })
            .map(|(&id, o)| {
                let Some(tid) = o.target else {
                    return (id, false);
                };
                let Some(target) = self.objects.get(&tid) else {
                    return (id, false);
                };
                let range = match o.ai_state {
                    AIState::Repairing => {
                        crate::game_logic::host_repair::repair_action_range(target.selection_radius)
                    }
                    _ => crate::game_logic::host_repair::DOZER_MIN_ACTION_TOLERANCE,
                };
                let p = o.get_position();
                let goal = if matches!(o.ai_state, AIState::Repairing) {
                    target.get_position()
                } else {
                    // C++ DOZER_DO_BUILD_AT_DOCK (cpp:511): ACTION dock, not centre.
                    crate::game_logic::host_repair::resolve_dozer_action_dock(
                        o.dozer_dock_action,
                        p,
                        target.get_position(),
                        target.selection_radius,
                    )
                };
                let dx = p.x - goal.x;
                let dz = p.z - goal.z;
                (id, (dx * dx + dz * dz).sqrt() <= range)
            })
            .collect();
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
            let is_dozer_building = obj.can_construct()
                && matches!(obj.ai_state, AIState::Constructing | AIState::Repairing)
                && dozer_at_site.get(&id).copied().unwrap_or(false);
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
            o.status.unmanned_owner_player_id = None;
        }
        let ok = self.detonate_car_bomb(vehicle_id);
        if ok {
            self.carbomb_unmanned_detonations = self.carbomb_unmanned_detonations.saturating_add(1);
        }
        ok
    }

    /// C++ `OverchargeBehavior::enable(FALSE)`: PowerPlantUpdate owns rod
    /// model conditions, then the controlling player's exact ThingTemplate
    /// EnergyBonus is removed.  Callers provide the typed values captured
    /// from the same authoritative template that enabled the behavior.
    fn disable_overcharge_object(
        &mut self,
        object_id: ObjectId,
        energy_bonus: i32,
        has_power_plant_update: bool,
    ) -> bool {
        if has_power_plant_update {
            let _ = self.retract_power_plant_rods(object_id);
        }
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj.overcharge_enabled {
            return false;
        }
        obj.power_provided = obj.power_provided.saturating_sub(energy_bonus);
        obj.record_host_entity_power();
        obj.set_overcharge_enabled(false);
        true
    }

    /// C++ OverchargeBehavior::toggle / enable residual.
    ///
    /// Object INI `OverchargeBehavior` is the only authority gate.  A China
    /// name, `PowerPlant` KindOf, or a legacy fixed +5 value cannot authorize
    /// this action.  C++ does not reject an enable click at the health
    /// threshold; the active behavior checks it after its next drain update.
    pub fn toggle_overcharge_object(&mut self, object_id: ObjectId) -> bool {
        let Some((energy_bonus, has_power_plant_update, was_active)) =
            self.objects.get(&object_id).and_then(|obj| {
                (obj.is_alive() && obj.thing.template.supports_overcharge()).then(|| {
                    (
                        obj.thing.template.energy_bonus.unwrap_or(0),
                        obj.thing.template.power_plant_update.is_some(),
                        obj.overcharge_enabled,
                    )
                })
            })
        else {
            return false;
        };

        let changed = if was_active {
            self.disable_overcharge_object(object_id, energy_bonus, has_power_plant_update)
        } else {
            // C++ walks every PowerPlantUpdate interface before it adds the
            // power bonus.  This route is deliberately optional: an authored
            // OverchargeBehavior without PowerPlantUpdate still toggles.
            if has_power_plant_update {
                let _ = self.begin_power_plant_rods_extend(object_id);
            }
            let Some(obj) = self.objects.get_mut(&object_id) else {
                return false;
            };
            // No click-time health threshold: C++ evaluates the strict `<`
            // threshold only in OverchargeBehavior::update after damage.
            obj.power_provided = obj.power_provided.saturating_add(energy_bonus);
            obj.record_host_entity_power();
            obj.set_overcharge_enabled(true);
            true
        };
        if changed {
            self.overcharge_toggles = self.overcharge_toggles.saturating_add(1);
        }
        changed
    }

    /// C++ OverchargeBehavior::update residual — drain HP while the typed
    /// behavior is active, then evaluate its authored strict health threshold.
    pub fn update_overcharge_drain(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| object.overcharge_enabled)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some((behavior, energy_bonus, has_power_plant_update)) =
                self.objects.get(&id).and_then(|obj| {
                    obj.thing.template.overcharge_behavior.map(|behavior| {
                        (
                            behavior,
                            obj.thing.template.energy_bonus.unwrap_or(0),
                            obj.thing.template.power_plant_update.is_some(),
                        )
                    })
                })
            else {
                // An old snapshot or malformed template must not keep an
                // active effect when it cannot revalidate the source module.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_overcharge_enabled(false);
                }
                continue;
            };

            let (dead, below_threshold) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !obj.is_alive() {
                    (true, false)
                } else {
                    // C++ reads BodyModule::getMaxHealth directly; do not
                    // synthesize a 1 HP minimum for malformed/zero-body data.
                    let max_health = obj.max_health.max(0.0);
                    // C++ computes this once per 30 Hz logic frame.  This
                    // host receives seconds, so the equivalent is max × rate
                    // × dt and retains the parsed per-template rate.
                    let damage = max_health * behavior.health_percent_to_drain_per_second * dt;
                    if damage > 0.0 {
                        // C++ explicitly issues DAMAGE_PENALTY here, rather
                        // than the generic unresistable residual damage type.
                        let _ = obj.take_damage_from_typed(
                            damage,
                            Some(id),
                            crate::game_logic::combat::DamageType::Penalty,
                        );
                    }
                    self.overcharge_drain_ticks = self.overcharge_drain_ticks.saturating_add(1);
                    (
                        !obj.is_alive() || obj.health.current <= 0.0,
                        obj.health.current
                            < max_health * behavior.not_allowed_when_health_below_percent,
                    )
                }
            };

            // `OverchargeBehavior::update` only calls enable(FALSE) through
            // this strict post-damage threshold.  With retail's 0% threshold,
            // a lethal hit instead keeps the module active until normal Object
            // deletion, where C++ `onDelete` removes its power bonus.  Removing
            // it here would incorrectly turn that 0% case into an exhaustion
            // branch and retract rods before the death path owns the object.
            if below_threshold {
                let (pos, local) = self
                    .objects
                    .get(&id)
                    .map(|obj| {
                        let local = obj
                            .owner_player_id
                            .and_then(|pid| self.players.get(&pid))
                            .is_some_and(|p| p.is_local && p.is_alive);
                        (obj.get_position(), local)
                    })
                    .unwrap_or((glam::Vec3::ZERO, false));
                let _ = self.disable_overcharge_object(id, energy_bonus, has_power_plant_update);
                // C++ posts OverchargeExhausted only through the authored
                // threshold branch.  Destruction itself is cleaned up by
                // onDelete and is not an exhaustion message at the 0% default.
                self.overcharge_exhaustions = self.overcharge_exhaustions.saturating_add(1);
                if local {
                    let msg =
                        localization::localize("GUI:OverchargeExhausted", "Overcharge exhausted");
                    self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
                    crate::game_logic::host_radar::host_create_radar_event(
                        pos,
                        game_engine::common::system::radar::RadarEventType::Information,
                    );
                }
            }
            if dead {
                self.mark_object_for_destruction(id, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;
    use crate::game_logic::{
        GameLogic, KindOf, ObjectId, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata,
        Team, ThingTemplate,
    };
    use gamelogic::helpers::{EvaEvent, TheEva};

    #[test]
    fn campaign_ally_superweapon_is_ally_not_enemy() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        let china = Player::new(1, Team::China, "China", false);
        local.set_map_relationship(1, gamelogic::common::Relationship::Allies);
        logic.players.insert(0, local);
        logic.players.insert(1, china);

        logic.try_eva_superweapon_detected_kind(Some(1), Team::China, "nuke");

        let events = TheEva::drain_events().expect("eva");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponDetectedAllyNuke),
            "{events:?}"
        );

        let _ = TheEva::drain_events();
        logic.try_eva_superweapon_launched_owned(
            Some(1),
            Team::China,
            HostSuperweaponKind::NuclearMissile,
        );
        let events = TheEva::drain_events().expect("eva2");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponLaunchedAllyNuke),
            "{events:?}"
        );

        let _ = TheEva::drain_events();
        logic.try_eva_superweapon_ready_for_player(1, "ChinaNuclearMissileLauncher");
        let events = TheEva::drain_events().expect("eva3");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponReadyAllyNuke),
            "{events:?}"
        );
    }

    #[test]
    fn same_faction_other_player_superweapon_is_ally_not_own() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        let mut ally = Player::new(1, Team::USA, "Ally", false);
        local.alliance_team = 1;
        ally.alliance_team = 1;
        logic.players.insert(0, local);
        logic.players.insert(1, ally);

        logic.try_eva_superweapon_detected_kind(Some(1), Team::USA, "particle");

        let events = TheEva::drain_events().expect("eva");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponDetectedAllyParticleCannon),
            "{events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponDetectedOwnParticleCannon),
            "{events:?}"
        );
    }

    #[test]
    fn same_faction_ally_beacon_detected() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        let mut ally = Player::new(1, Team::USA, "Ally", false);
        local.alliance_team = 1;
        ally.alliance_team = 1;
        logic.players.insert(0, local);
        logic.players.insert(1, ally);
        logic.try_eva_beacon_detected(1);
        assert!(logic.honesty_eva_beacon_detected_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events.iter().any(|e| *e == EvaEvent::BeaconDetected),
            "{events:?}"
        );
    }

    #[test]
    fn campaign_ally_hero_detection_skipped() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        let china = Player::new(1, Team::China, "China", false);
        local.set_map_relationship(1, gamelogic::common::Relationship::Allies);
        logic.players.insert(0, local);
        logic.players.insert(1, china);
        let mut lotus = ThingTemplate::new("ChinaInfantryBlackLotus");
        lotus
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Hero)
            .set_health(100.0);
        logic
            .templates
            .insert("ChinaInfantryBlackLotus".into(), lotus);
        let id = logic
            .create_object_for_player("ChinaInfantryBlackLotus", 1, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("ally lotus");
        logic.try_eva_hero_detected(id);
        assert!(!logic.honesty_eva_hero_detected_ok());
    }

    #[test]
    fn fire_stealth_discover_skips_hero_eva_when_detector_not_local() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic
            .players
            .insert(1, Player::new(1, Team::China, "China", false));
        logic
            .players
            .insert(2, Player::new(2, Team::GLA, "GLA", false));
        let mut lotus = ThingTemplate::new("ChinaInfantryBlackLotus");
        lotus
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Hero)
            .set_health(100.0);
        logic
            .templates
            .insert("ChinaInfantryBlackLotus".into(), lotus);
        let mut det_t = ThingTemplate::new("GLAVehicleRadarVan");
        det_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
        logic.templates.insert("GLAVehicleRadarVan".into(), det_t);
        let hero = logic
            .create_object_for_player("ChinaInfantryBlackLotus", 1, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("lotus");
        let det = logic
            .create_object_for_player("GLAVehicleRadarVan", 2, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("ai detector");
        logic.fire_stealth_discover_feedback(hero, &[det]);
        assert!(
            !logic.honesty_eva_hero_detected_ok(),
            "AI detector revealing an enemy hero must not announce to local"
        );
    }

    #[test]
    fn fire_stealth_discover_enemy_hero_eva_throttled_by_try_event() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic
            .players
            .insert(1, Player::new(1, Team::China, "China", false));
        let mut lotus = ThingTemplate::new("ChinaInfantryBlackLotus");
        lotus
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Hero)
            .set_health(100.0);
        logic
            .templates
            .insert("ChinaInfantryBlackLotus".into(), lotus);
        let mut det_t = ThingTemplate::new("AmericaVehicleHumvee");
        det_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
        logic.templates.insert("AmericaVehicleHumvee".into(), det_t);
        let hero_a = logic
            .create_object_for_player("ChinaInfantryBlackLotus", 1, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("lotus a");
        let hero_b = logic
            .create_object_for_player(
                "ChinaInfantryBlackLotus",
                1,
                glam::Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("lotus b");
        let det = logic
            .create_object_for_player("AmericaVehicleHumvee", 0, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("local detector");
        logic.fire_stealth_discover_feedback(hero_a, &[det]);
        assert!(logic.honesty_eva_hero_detected_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::EnemyBlackLotusDetected),
            "{events:?}"
        );
        let before = logic.eva_hero_detected;
        logic.fire_stealth_discover_feedback(hero_b, &[det]);
        assert_eq!(
            logic.eva_hero_detected, before,
            "second hero in one scan must be throttled by Radar tryEvent"
        );
    }

    fn particle_cannon_module(required_science: Option<&str>) -> SpecialPowerModuleMetadata {
        SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_ParticleCannon".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: "SuperweaponParticleCannon".into(),
            special_power_template_id: 1,
            command_power: Some(crate::command_system::SpecialPowerType::ParticleCannon),
            reload_time_frames: 300,
            required_science: required_science.map(str::to_string),
            public_timer: true,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        }
    }

    fn spawn_local_particle_cannon(
        logic: &mut GameLogic,
        required_science: Option<&str>,
    ) -> ObjectId {
        if !logic.players.contains_key(&0) {
            logic
                .players
                .insert(0, Player::new(0, Team::USA, "Local", true));
        }
        let mut template = ThingTemplate::new("AmericaParticleUplinkCannon");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSSuperweapon)
            .set_health(4000.0);
        template
            .special_power_modules
            .push(particle_cannon_module(required_science));
        logic
            .templates
            .insert("AmericaParticleUplinkCannon".into(), template);
        let id = logic
            .create_object_for_player("AmericaParticleUplinkCannon", 0, glam::Vec3::ZERO)
            .expect("puc");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.thing.template.special_power_modules.clear();
            obj.thing
                .template
                .special_power_modules
                .push(particle_cannon_module(required_science));
        }
        logic.clear_eva_science_hidden_latch(id);
        id
    }

    #[test]
    fn superweapon_ready_skips_hidden_by_script_timer() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic.hide_script_superweapon_object_for_test(ObjectId(1));
        logic.try_eva_superweapon_ready(ObjectId(1), Team::USA, "AmericaParticleUplinkCannon");
        assert!(
            !logic.honesty_eva_superweapon_ready_ok(),
            "hideObjectSuperweaponDisplayByScript must skip SuperweaponReady EVA"
        );
    }

    #[test]
    fn superweapon_ready_ignores_global_display_hide() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic.set_script_superweapon_display_enabled_for_test(false);
        logic.try_eva_superweapon_ready(ObjectId(1), Team::USA, "AmericaParticleUplinkCannon");
        assert!(
            logic.honesty_eva_superweapon_ready_ok(),
            "global m_superweaponHiddenByScript is draw-only"
        );
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponReadyOwnParticleCannon),
            "{events:?}"
        );
    }

    #[test]
    fn superweapon_ready_skips_hidden_by_science_and_never_clears() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let id = spawn_local_particle_cannon(&mut logic, Some("SCIENCE_ParticleCannonTest"));
        logic.on_structure_superweapon_creation(id);
        assert!(
            logic.eva_science_hidden_latched(id),
            "creation without RequiredScience must latch hiddenByScience"
        );
        assert!(
            logic
                .players
                .get_mut(&0)
                .unwrap()
                .unlock_science("SCIENCE_ParticleCannonTest")
        );
        let _ = TheEva::drain_events();
        logic.try_eva_superweapon_ready_for_source(id);
        assert!(
            !logic.honesty_eva_superweapon_ready_ok(),
            "m_hiddenByScience never clears after later science grant"
        );
    }

    #[test]
    fn superweapon_ready_announces_when_science_owned_at_creation() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        assert!(
            logic
                .players
                .get_mut(&0)
                .unwrap()
                .unlock_science("SCIENCE_ParticleCannonTest")
        );
        let id = spawn_local_particle_cannon(&mut logic, Some("SCIENCE_ParticleCannonTest"));
        logic.on_structure_superweapon_creation(id);
        let _ = TheEva::drain_events();
        logic.try_eva_superweapon_ready_for_source(id);
        assert!(logic.honesty_eva_superweapon_ready_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events
                .iter()
                .any(|e| *e == EvaEvent::SuperweaponReadyOwnParticleCannon),
            "{events:?}"
        );
    }

    #[test]
    fn superweapon_ready_resumes_after_script_show() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic.hide_script_superweapon_object_for_test(ObjectId(1));
        logic.try_eva_superweapon_ready(ObjectId(1), Team::USA, "AmericaParticleUplinkCannon");
        assert!(!logic.honesty_eva_superweapon_ready_ok());
        logic.restore_script_superweapon_hidden_objects([]);
        logic.try_eva_superweapon_ready(ObjectId(1), Team::USA, "AmericaParticleUplinkCannon");
        assert!(logic.honesty_eva_superweapon_ready_ok());
    }

    #[test]
    fn superweapon_ready_for_player_skips_when_all_sources_hidden() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let id = spawn_local_particle_cannon(&mut logic, None);
        logic.hide_script_superweapon_object_for_test(id);
        logic.try_eva_superweapon_ready_for_player(0, "AmericaParticleUplinkCannon");
        assert!(
            !logic.honesty_eva_superweapon_ready_ok(),
            "shared-timer ready must skip when every matching object is hidden"
        );
    }
}
