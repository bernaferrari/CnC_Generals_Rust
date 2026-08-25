//! Host scripts `impl GameLogic` — `eva_camera`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! EVA events / radar audio / camera presentation / execute_command
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn try_radar_upgrade_complete(
        &mut self,
        player_id: u32,
        team: Team,
        upgrade_name: &str,
        source_object: Option<ObjectId>,
    ) {
        if !self.is_local_player(player_id) {
            return;
        }
        let pos = source_object
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .or_else(|| {
                // Prefer command center / any structure residual position.
                self.objects
                    .values()
                    .filter(|o| o.team == team && o.is_alive() && o.is_kind_of(KindOf::Structure))
                    .map(|o| o.get_position())
                    .next()
            })
            .unwrap_or(glam::Vec3::ZERO);

        let msg = localization::localize(
            "UPGRADE:UpgradeComplete",
            &format!("Upgrade complete: {upgrade_name}"),
        );
        // C++ TheRadar->createEvent(..., RADAR_EVENT_UPGRADE) residual.
        // Host maps upgrade events as Generic radar kind with upgrade honesty.
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        crate::game_logic::host_radar::host_create_radar_event(
            pos,
            game_engine::common::system::radar::RadarEventType::Upgrade,
        );
        self.radar_upgrade_events = self.radar_upgrade_events.saturating_add(1);
    }

    pub fn honesty_radar_upgrade_event_ok(&self) -> bool {
        self.radar_upgrade_events > 0
    }

    pub fn try_eva_upgrade_complete(&mut self, player_id: u32) {
        if !self.is_local_player(player_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::UpgradeComplete,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::UpgradeComplete,
        );
        self.eva_upgrade_complete = self.eva_upgrade_complete.saturating_add(1);
    }

    /// C++ TheEva->setShouldPlay(EVA_GeneralLevelUp) residual (local player).
    pub fn try_eva_general_level_up(&mut self, player_id: u32) {
        if !self.is_local_player(player_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::GeneralLevelUp,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::GeneralLevelUp);
        self.eva_general_level_up = self.eva_general_level_up.saturating_add(1);
    }

    /// Award skill points and fire GeneralLevelUp EVA on rank change residual.
    pub fn add_player_skill_points(&mut self, player_id: u32, points: i32) -> bool {
        let Some(p) = self.players.get_mut(&player_id) else {
            return false;
        };
        let leveled = p.add_skill_points(points);
        if leveled {
            self.try_eva_general_level_up(player_id);
        }
        leveled
    }

    pub fn honesty_eva_upgrade_complete_ok(&self) -> bool {
        self.eva_upgrade_complete > 0
    }

    pub fn honesty_eva_general_level_up_ok(&self) -> bool {
        self.eva_general_level_up > 0
    }

    pub fn update_eva_low_power(&mut self) {
        use crate::game_logic::host_ui_presentation_residual::EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL;
        let local_low = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.power_available < 0);
        if !local_low {
            self.eva_low_power_active = false;
            return;
        }
        let edge = !self.eva_low_power_active;
        self.eva_low_power_active = true;
        if !edge && self.frame < self.eva_low_power_next_frame {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::LowPower);
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::LowPower);
        self.eva_low_power = self.eva_low_power.saturating_add(1);
        self.eva_low_power_next_frame = self
            .frame
            .saturating_add(EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL);
    }

    /// C++ TheEva->setShouldPlay(EVA_InsufficientFunds) on every failed click.
    /// Replay suppression is leftover Eva.ini TimeBetweenChecksMS, not a host throttle.
    pub fn try_eva_insufficient_funds(&mut self, player_id: u32) {
        let Some(p) = self.players.get(&player_id) else {
            return;
        };
        if !p.is_local || !p.is_alive {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        self.eva_insufficient_funds = self.eva_insufficient_funds.saturating_add(1);
    }

    pub fn honesty_eva_low_power_ok(&self) -> bool {
        self.eva_low_power > 0
    }

    pub fn eva_low_power_count(&self) -> u32 {
        self.eva_low_power
    }

    pub fn eva_insufficient_funds_count(&self) -> u32 {
        self.eva_insufficient_funds
    }

    pub fn eva_base_under_attack_count(&self) -> u32 {
        self.eva_base_under_attack
    }

    pub fn eva_ally_under_attack_count(&self) -> u32 {
        self.eva_ally_under_attack
    }

    pub fn honesty_eva_insufficient_funds_ok(&self) -> bool {
        self.eva_insufficient_funds > 0
    }

    pub fn try_under_attack_event(&mut self, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_radar_stealth_vision_residual::{
            RADAR_AUDIO_HARVESTER_UNDER_ATTACK, RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            RADAR_MSG_HARVESTER_UNDER_ATTACK, RADAR_MSG_STRUCTURE_UNDER_ATTACK,
            RADAR_MSG_UNDER_ATTACK, RADAR_MSG_UNIT_UNDER_ATTACK,
            SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL,
        };
        let Some(obj) = self.objects.get(&victim_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        let pos = obj.get_position();
        let team = obj.team;
        let owner_player_id = obj.owner_player_id;
        let is_infantry = obj.is_kind_of(KindOf::Infantry);
        let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
        let is_structure = obj.is_kind_of(KindOf::Structure);
        let name_l = obj.template_name.to_ascii_lowercase();
        let is_harvester = name_l.contains("supplytruck")
            || name_l.contains("supply_truck")
            || name_l.contains("harvester")
            || name_l.contains("gatherer")
            || (name_l.contains("worker") && !name_l.contains("dozer"));
        // C++ Radar.cpp:1194 / Object.cpp:4597 — STRUCTURE + MP_COUNT_FOR_VICTORY.
        // Do not invent an FS-kind union; Black Market / Internet Center carry
        // the authored bit without FS_FACTORY.
        let is_mp_count = is_structure && obj.is_kind_of(KindOf::MpCountForVictory);

        // C++ Radar.cpp:1293-1294 operator-precedence quirk: 250² is not a
        // real distance filter. Same-type UnderAttack pings throttle map-wide
        // for 10s (LOGICFRAMES_PER_SECOND * 10).
        let now = self.frame;
        let frames_between = SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL;
        let px = pos.x;
        let pz = pos.z;
        for &(frame, _ex, _ez) in &self.under_attack_event_history {
            if now.saturating_sub(frame) < frames_between {
                return false;
            }
        }
        self.under_attack_event_history.push((now, px, pz));
        if self.under_attack_event_history.len() > 64 {
            let drain = self.under_attack_event_history.len() - 64;
            self.under_attack_event_history.drain(0..drain);
        }
        self.under_attack_events = self.under_attack_events.saturating_add(1);

        let (msg_key, msg_fallback, audio) = if is_infantry || is_vehicle {
            if is_harvester {
                (
                    RADAR_MSG_HARVESTER_UNDER_ATTACK,
                    "Harvester under attack",
                    RADAR_AUDIO_HARVESTER_UNDER_ATTACK,
                )
            } else {
                (
                    RADAR_MSG_UNIT_UNDER_ATTACK,
                    "Unit under attack",
                    RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
                )
            }
        } else if is_structure && is_mp_count {
            (
                RADAR_MSG_STRUCTURE_UNDER_ATTACK,
                "Structure under attack",
                RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            )
        } else {
            (
                RADAR_MSG_UNDER_ATTACK,
                "Under attack",
                RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            )
        };
        let msg = localization::localize(msg_key, msg_fallback);
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
        // C++ Radar.cpp:1155 tryEvent(UNDER_ATTACK) — minimap blip + last-event.
        if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
            let loc = game_engine::common::system::radar::Coord3D {
                x: pos.x,
                y: pos.z,
                z: pos.y,
            };
            let _ = radar.try_event(
                game_engine::common::system::radar::RadarEventType::UnderAttack,
                &loc,
            );
        }
        gamelogic::helpers::TheControlBar::trigger_radar_attack_glow();
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(victim_id)
                .with_position(pos)
                .with_priority(165),
        );

        if is_structure && is_mp_count {
            // C++ Radar.cpp:1197-1200:
            // controllingPlayer->isLocalPlayer → BaseUnderAttack
            // else getLocalPlayer()->getRelationship(obj->getTeam()) == ALLIES
            // → AllyUnderAttack. Faction Team / alliance_team is not a proxy.
            let owner_id = owner_player_id
                .filter(|id| self.players.get(id).is_some_and(|player| player.is_alive))
                .or_else(|| self.unique_player_id_for_team(team));
            let local_owns = owner_id
                .and_then(|id| self.players.get(&id))
                .is_some_and(|player| player.is_local && player.is_alive);
            let local_ally = !local_owns
                && owner_id.is_some_and(|oid| {
                    self.players.values().any(|player| {
                        player.is_local
                            && player.is_alive
                            && self.player_relationship(player.id, oid)
                                == gamelogic::common::Relationship::Allies
                    })
                });

            if local_owns {
                let _ = gamelogic::helpers::TheEva::set_should_play(
                    gamelogic::helpers::EvaEvent::BaseUnderAttack,
                );
                crate::game_logic::host_eva_log::record_event(
                    gamelogic::helpers::EvaEvent::BaseUnderAttack,
                );
                self.eva_base_under_attack = self.eva_base_under_attack.saturating_add(1);
            } else if local_ally {
                let _ = gamelogic::helpers::TheEva::set_should_play(
                    gamelogic::helpers::EvaEvent::AllyUnderAttack,
                );
                crate::game_logic::host_eva_log::record_event(
                    gamelogic::helpers::EvaEvent::AllyUnderAttack,
                );
                self.eva_ally_under_attack = self.eva_ally_under_attack.saturating_add(1);
            }
        }

        true
    }

    /// C++ Object.cpp:1846-1854 before TheRadar->tryUnderAttackEvent.
    /// Radar.cpp tryUnderAttackEvent itself has no ownership gate.
    pub fn try_under_attack_from_damage(&mut self, victim_id: ObjectId) -> bool {
        if !self.object_qualifies_for_under_attack(victim_id) {
            return false;
        }
        self.try_under_attack_event(victim_id)
    }

    fn object_qualifies_for_under_attack(&self, victim_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&victim_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        // C++ getControllingPlayer() == ThePlayerList->getLocalPlayer().
        if !self.is_object_locally_controlled(victim_id) {
            return false;
        }
        // C++ damageType != PENALTY && != HEALING.
        if matches!(
            obj.last_damage_fx_done,
            Some(crate::game_logic::combat::DamageType::Penalty)
                | Some(crate::game_logic::combat::DamageType::Healing)
        ) {
            return false;
        }
        // C++ !BitTest(sourcePlayerMask, victim controller mask).
        let victim_owner = self.player_owner_for_host_object(obj);
        if let Some(src_id) = obj.last_damage_source {
            if let Some(src) = self.objects.get(&src_id) {
                if victim_owner.is_some() && self.player_owner_for_host_object(src) == victim_owner
                {
                    return false;
                }
            }
        }
        // C++ m_radarData != NULL — live NotOnRadar never gets radar data.
        if obj.thing.template.radar_priority == 1 {
            return false;
        }
        true
    }

    pub fn honesty_under_attack_event_ok(&self) -> bool {
        self.under_attack_events > 0
    }

    pub fn honesty_eva_base_under_attack_ok(&self) -> bool {
        self.eva_base_under_attack > 0
    }

    pub fn honesty_eva_ally_under_attack_ok(&self) -> bool {
        self.eva_ally_under_attack > 0
    }

    pub fn try_eva_on_local_object_death(
        &mut self,
        victim_id: ObjectId,
        _victim_team: crate::game_logic::Team,
        is_structure: bool,
        is_infantry: bool,
        is_vehicle: bool,
        is_mp_count_for_victory: bool,
        death_pos: glam::Vec3,
        _killer: Option<crate::game_logic::Team>,
    ) {
        // C++ Object::isLocallyControlled — controlling player == local, not faction Team.
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        // C++ selfInflicted = (damageInfo->in.m_sourceID == getID()).
        let self_inflicted = self
            .objects
            .get(&victim_id)
            .and_then(|o| o.last_damage_source)
            == Some(victim_id);
        if self_inflicted {
            return;
        }

        // C++ Object.cpp:4597 — STRUCTURE + KINDOF_MP_COUNT_FOR_VICTORY.
        let is_mp_count_for_victory = self
            .objects
            .get(&victim_id)
            .map(|o| o.is_kind_of(KindOf::MpCountForVictory))
            .unwrap_or(is_mp_count_for_victory);
        if is_structure && is_mp_count_for_victory {
            let _ = gamelogic::helpers::TheEva::set_should_play(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            crate::game_logic::host_eva_log::record_event(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            self.saboteur.record_eva_building_lost();
        } else if is_infantry || is_vehicle {
            let _ =
                gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::UnitLost);
            crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::UnitLost);
            self.saboteur.record_eva_unit_lost();
            // C++ TheRadar->tryEvent(RADAR_EVENT_FAKE, pos) — spacebar jump, no text.
            if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
                let loc = game_engine::common::system::radar::Coord3D {
                    x: death_pos.x,
                    y: death_pos.z,
                    z: death_pos.y,
                };
                let _ = radar.try_event(
                    game_engine::common::system::radar::RadarEventType::Fake,
                    &loc,
                );
            }
        }
    }

    pub fn try_eva_vehicle_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::VehicleStolen,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::VehicleStolen);
        self.car_bomb.record_eva_vehicle_stolen();
    }

    pub fn try_eva_building_being_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingBeingStolen,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::BuildingBeingStolen,
        );
        self.hero_abilities.record_eva_building_being_stolen();
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingStolen) when capture completes.
    pub fn try_eva_building_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingStolen,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::BuildingStolen);
        self.hero_abilities.record_eva_building_stolen();
    }

    pub fn try_eva_building_sabotaged(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingSabotaged,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::BuildingSabotaged,
        );
        self.saboteur.record_eva_building_sabotaged();
    }

    /// C++ TheEva->setShouldPlay(EVA_CashStolen) when local supply center is robbed.
    pub fn try_eva_cash_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ =
            gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::CashStolen);
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::CashStolen);
        self.saboteur.record_eva_cash_stolen();
    }

    pub fn try_infiltration_event(&mut self, victim_id: ObjectId) {
        let Some(obj) = self.objects.get(&victim_id) else {
            return;
        };
        if !obj.is_alive() {
            return;
        }
        let victim_team = obj.team;
        let pos = obj.get_position();
        // C++ Radar.cpp:1243 — only the local victim is warned.
        let local_victim = self
            .players
            .values()
            .any(|p| p.team == victim_team && p.is_local);
        if !local_victim {
            return;
        }
        let msg = localization::localize("RADAR:Infiltration", "Infiltration event");
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
        if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
            let loc = game_engine::common::system::radar::Coord3D {
                x: pos.x,
                y: pos.z,
                z: pos.y,
            };
            radar.create_event(
                &loc,
                game_engine::common::system::radar::RadarEventType::Infiltration,
                4.0,
            );
        }
        self.queue_audio_event(
            AudioEventRequest::new(
                crate::game_logic::host_radar_stealth_vision_residual::RADAR_INFILTRATION_AUDIO,
            )
            .with_object(victim_id)
            .with_position(pos)
            .with_priority(175),
        );
        self.saboteur.record_infiltration_event();
    }

    pub fn queue_radar_message_for_team<S: Into<String>>(&mut self, team: Team, message: S) {
        if let Some(position) = self.command_center_position(team) {
            self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Generic);
        } else {
            self.queue_radar_message(message);
        }
    }

    /// Track a newly placed beacon so the UI can bloom/highlight it this frame.
    pub fn note_beacon_placed(&mut self, position: Vec3) {
        // Wave 211: host-owned active list + frame bloom residual.
        const MATCH: f32 = 3.0; // beacon_manager BEACON_MATCH_THRESHOLD residual
        self.host_beacons
            .retain(|p| (*p - position).length() > MATCH);
        self.host_beacons.push(position);
        self.recent_beacons.push(position);
    }

    /// Wave 211: remove latest host beacon for player place-order residual
    /// (manager remove_latest is player-scoped; host list is position-only).
    pub fn note_beacon_removed_latest(&mut self) {
        let _ = self.host_beacons.pop();
    }

    /// Remove a specific host beacon position (hide / selected destroy).
    pub fn note_beacon_removed_at(&mut self, position: Vec3) {
        const MATCH: f32 = 3.0;
        if let Some(idx) = self
            .host_beacons
            .iter()
            .position(|p| (*p - position).length() <= MATCH)
        {
            self.host_beacons.remove(idx);
        }
    }

    /// Active host beacon positions for presentation freeze.
    pub fn host_beacons(&self) -> &[Vec3] {
        &self.host_beacons
    }

    /// Play radar audio with a short cooldown to avoid stacking duplicates if many events fire simultaneously.
    pub(in super::super) fn maybe_play_radar_audio(&mut self, cue: &str) {
        const RADAR_AUDIO_COOLDOWN: f32 = 1.0;
        if self.sim_time_seconds - self.last_radar_audio_time >= RADAR_AUDIO_COOLDOWN {
            self.queue_audio_event(AudioEventRequest::new(translate_audio_event(cue)));
            self.last_radar_audio_time = self.sim_time_seconds;
        }
    }

    pub fn last_radar_event_position(&self) -> Option<Vec3> {
        self.last_radar_event.as_ref().map(|entry| entry.position)
    }

    pub fn request_camera_focus(&mut self, position: Vec3) {
        static DEBUG_CAMERA_FOCUS_LOGS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if DEBUG_CAMERA_FOCUS_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 24 {
            log::trace!("DEBUG_SHELL_CAMERA_BRIDGE: request_camera_focus position={position:?}");
        }
        self.pending_camera_focus = Some(position);
        self.script_camera_focus_estimate = position;
    }

    /// C++ `W3DView::setAngle`/`setPitch`/`setZoom`/`setHeightAboveGround`:
    /// drop the in-flight scripted path so player input is not overwritten.
    pub fn cancel_scripted_camera_from_player_set(&mut self) {
        self.clear_in_flight_scripted_camera_move();
        // Residual peeks these every frame (drain is a no-op). If they stay set,
        // live scripted zoom/pitch/rotate re-arm after the one-frame skip.
        self.pending_camera_rotate = None;
        self.pending_camera_zoom = None;
        self.pending_camera_pitch = None;
        self.pending_camera_zoom_reset = false;
        self.pending_camera_zoom_reset_duration = 0.0;
        self.pending_camera_zoom_reset_ease_in = 0.0;
        self.pending_camera_zoom_reset_ease_out = 0.0;
        self.clear_script_camera_orientation_remaining();
    }

    /// C++ `W3DView::lookAt`: drop rotate + waypoint path + scripted lock.
    pub fn cancel_scripted_camera_from_player_look_at(&mut self) {
        self.clear_in_flight_scripted_camera_move();
        self.pending_camera_rotate = None;
        self.clear_script_camera_orientation_remaining();
    }

    /// C++ player `TheTacticalView->lookAt`: cancel then queue the snap.
    pub fn request_player_camera_look_at(&mut self, position: Vec3) {
        self.cancel_scripted_camera_from_player_look_at();
        self.request_camera_focus(position);
    }

    fn clear_in_flight_scripted_camera_move(&mut self) {
        self.script_camera_move_to = None;
        self.script_camera_path = None;
        self.pending_camera_look_toward = None;
        self.script_look_toward_object_id = None;
        self.script_look_toward_hold_seconds = 0.0;
        self.pending_camera_focus = None;
        self.mission_scripts.set_camera_movement_finished(true);
    }

    pub(in super::super) fn selected_objects_center_for_local_player(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let player = self.players.get(&local_player_id)?;
        if player.selected_objects.is_empty() {
            return None;
        }

        let mut count = 0usize;
        let mut sum = Vec3::ZERO;
        for object_id in &player.selected_objects {
            let Some(obj) = self.objects.get(object_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            sum += obj.get_position();
            count += 1;
        }

        if count == 0 {
            None
        } else {
            Some(sum / count as f32)
        }
    }

    pub(in super::super) fn local_player_camera_home_position(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let team = self.players.get(&local_player_id)?.team;
        self.command_center_position(team)
            .or_else(|| self.team_base_position(team))
    }

    pub fn peek_pending_screen_shakes(
        &self,
    ) -> &[crate::game_logic::mission_scripts::ScreenShakeRequest] {
        &self.pending_screen_shakes
    }

    pub fn peek_script_skybox_enabled(&self) -> bool {
        self.script_skybox_enabled
    }

    pub fn peek_script_superweapon_display_enabled(&self) -> bool {
        self.script_superweapon_display_enabled
    }

    pub fn peek_script_named_timer_display_shown(&self) -> bool {
        self.script_named_timer_display_shown
    }

    pub fn peek_script_superweapon_hidden_objects(
        &self,
    ) -> &std::collections::HashSet<crate::game_logic::ObjectId> {
        &self.script_superweapon_hidden_objects
    }

    pub fn queue_pending_screen_shake(&mut self, intensity: i32) {
        self.pending_screen_shakes
            .push(crate::game_logic::mission_scripts::ScreenShakeRequest { intensity });
    }

    pub fn set_script_skybox_enabled_for_test(&mut self, enabled: bool) {
        self.script_skybox_enabled = enabled;
    }

    pub fn set_script_superweapon_display_enabled_for_test(&mut self, enabled: bool) {
        self.script_superweapon_display_enabled = enabled;
    }

    pub fn set_script_named_timer_display_shown_for_test(&mut self, shown: bool) {
        self.script_named_timer_display_shown = shown;
    }

    pub fn hide_script_superweapon_object_for_test(
        &mut self,
        object_id: crate::game_logic::ObjectId,
    ) {
        self.script_superweapon_hidden_objects.insert(object_id);
    }

    pub fn peek_pending_camera_zoom(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraZoomRequest> {
        self.pending_camera_zoom.as_ref()
    }

    pub fn peek_pending_camera_zoom_reset(&self) -> bool {
        self.pending_camera_zoom_reset
    }

    pub fn peek_pending_camera_zoom_reset_duration(&self) -> f32 {
        self.pending_camera_zoom_reset_duration
    }

    pub fn peek_pending_camera_zoom_reset_ease(&self) -> (f32, f32) {
        (
            self.pending_camera_zoom_reset_ease_in,
            self.pending_camera_zoom_reset_ease_out,
        )
    }

    pub fn peek_pending_camera_pitch(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraPitchRequest> {
        self.pending_camera_pitch.as_ref()
    }

    pub fn peek_pending_camera_rotate(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraRotateRequest> {
        self.pending_camera_rotate.as_ref()
    }

    pub fn peek_pending_camera_look_toward(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraLookTowardWaypointRequest> {
        self.pending_camera_look_toward.as_ref()
    }

    pub fn set_pending_camera_look_toward_reverse_rotation(&mut self, reverse: bool) {
        if let Some(req) = self.pending_camera_look_toward.as_mut() {
            req.reverse_rotation = reverse;
        }
    }

    pub fn peek_pending_camera_slave_enable(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraSlaveModeRequest> {
        self.pending_camera_slave_mode_enable.as_ref()
    }

    pub fn peek_pending_camera_slave_disable(&self) -> bool {
        self.pending_camera_slave_mode_disable
    }

    pub fn peek_script_named_timers(&self) -> &std::collections::HashMap<String, (String, bool)> {
        &self.script_named_timers
    }

    pub fn peek_script_cameo_flash_count(&self) -> &std::collections::HashMap<String, i32> {
        &self.script_cameo_flash_count
    }

    pub fn queue_pending_camera_zoom(&mut self, zoom: f32, duration_seconds: f32) {
        self.pending_camera_zoom = Some(crate::game_logic::mission_scripts::CameraZoomRequest {
            zoom,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    }

    pub fn queue_pending_camera_zoom_reset(&mut self) {
        self.pending_camera_zoom_reset = true;
        self.pending_camera_zoom_reset_duration = 0.0;
        self.pending_camera_zoom_reset_ease_in = 0.0;
        self.pending_camera_zoom_reset_ease_out = 0.0;
    }

    pub fn queue_pending_camera_pitch(&mut self, pitch: f32, duration_seconds: f32) {
        self.pending_camera_pitch = Some(crate::game_logic::mission_scripts::CameraPitchRequest {
            pitch,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    }

    pub fn queue_pending_camera_rotate(&mut self, rotations: f32, duration_seconds: f32) {
        self.pending_camera_rotate =
            Some(crate::game_logic::mission_scripts::CameraRotateRequest {
                rotations,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
    }

    pub fn queue_pending_camera_look_toward(&mut self, position: Vec3, duration_seconds: f32) {
        self.pending_camera_look_toward = Some(
            crate::game_logic::mission_scripts::CameraLookTowardWaypointRequest {
                position,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
                reverse_rotation: false,
            },
        );
    }

    pub fn queue_pending_camera_slave_enable(
        &mut self,
        thing_template_name: impl Into<String>,
        bone_name: impl Into<String>,
    ) {
        self.pending_camera_slave_mode_enable =
            Some(crate::game_logic::mission_scripts::CameraSlaveModeRequest {
                thing_template_name: thing_template_name.into(),
                bone_name: bone_name.into(),
            });
    }

    pub fn queue_pending_camera_slave_disable(&mut self) {
        self.pending_camera_slave_mode_disable = true;
    }

    pub fn upsert_script_named_timer(
        &mut self,
        name: impl Into<String>,
        text: impl Into<String>,
        countdown: bool,
    ) {
        self.script_named_timers
            .insert(name.into(), (text.into(), countdown));
    }

    pub fn restore_script_named_timers(
        &mut self,
        timers: impl IntoIterator<Item = (String, String, bool)>,
    ) {
        self.script_named_timers = timers
            .into_iter()
            .map(|(name, text, countdown)| (name, (text, countdown)))
            .collect();
    }

    pub fn restore_script_named_timer_display_shown(&mut self, shown: bool) {
        self.script_named_timer_display_shown = shown;
    }

    pub fn restore_script_superweapon_display_enabled(&mut self, enabled: bool) {
        self.script_superweapon_display_enabled = enabled;
    }

    pub fn restore_script_superweapon_hidden_objects(
        &mut self,
        ids: impl IntoIterator<Item = crate::game_logic::ObjectId>,
    ) {
        self.script_superweapon_hidden_objects = ids.into_iter().collect();
    }

    pub fn restore_radar_script_state(&mut self, enabled: bool, forced: bool) {
        self.radar_enabled = enabled;
        self.radar_forced = forced;
    }

    pub fn snapshot_script_actives(&self) -> Vec<(String, bool, bool)> {
        let mut entries = Vec::new();
        for list in &self.loaded_script_lists {
            let mut script = list.get_script();
            while let Some(node) = script {
                entries.push((node.get_name().to_string(), false, node.is_active()));
                script = node.get_next();
            }
            let mut group = list.get_script_group();
            while let Some(node) = group {
                entries.push((node.get_name().to_string(), true, node.is_active()));
                let mut inner = node.get_script();
                while let Some(script_node) = inner {
                    entries.push((
                        script_node.get_name().to_string(),
                        false,
                        script_node.is_active(),
                    ));
                    inner = script_node.get_next();
                }
                group = node.get_next();
            }
        }
        entries
    }

    pub fn restore_script_actives(&mut self, entries: &[(String, bool, bool)]) {
        for (name, is_group, active) in entries {
            for list in &mut self.loaded_script_lists {
                if *is_group {
                    let mut group = list.first_group.as_mut();
                    while let Some(node) = group {
                        if node.get_name() == name {
                            node.set_active(*active);
                            break;
                        }
                        group = node.next_group.as_mut();
                    }
                } else {
                    let mut script = list.first_script.as_mut();
                    while let Some(node) = script {
                        if node.get_name() == name {
                            node.set_active(*active);
                            break;
                        }
                        script = node.next_script.as_mut();
                    }
                    let mut group = list.first_group.as_mut();
                    while let Some(node) = group {
                        let mut inner = node.first_script.as_mut();
                        while let Some(script_node) = inner {
                            if script_node.get_name() == name {
                                script_node.set_active(*active);
                                break;
                            }
                            inner = script_node.next_script.as_mut();
                        }
                        group = node.next_group.as_mut();
                    }
                }
            }
        }
    }

    pub fn set_script_cameo_flash(&mut self, button: impl Into<String>, flash_count: i32) {
        self.script_cameo_flash_count
            .insert(button.into(), flash_count);
    }

    pub fn peek_pending_camera_focus(&self) -> Option<Vec3> {
        self.pending_camera_focus
    }

    pub fn peek_pending_view_guardband(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::ViewGuardbandRequest> {
        self.pending_view_guardband.as_ref()
    }

    pub fn peek_pending_script_fps_limit(&self) -> Option<i32> {
        self.pending_script_fps_limit
    }

    pub fn peek_pending_camera_bw_mode(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraBwModeRequest> {
        self.pending_camera_bw_mode.as_ref()
    }

    pub fn peek_pending_camera_add_shakers(
        &self,
    ) -> &[crate::game_logic::mission_scripts::CameraAddShakerRequest] {
        &self.pending_camera_add_shakers
    }

    pub fn peek_pending_camera_motion_blur_count(&self) -> usize {
        self.pending_camera_motion_blur.len()
    }

    pub fn queue_pending_camera_focus(&mut self, pos: Vec3) {
        self.pending_camera_focus = Some(pos);
    }

    pub fn queue_pending_view_guardband(&mut self, x_bias: f32, y_bias: f32) {
        self.pending_view_guardband =
            Some(crate::game_logic::mission_scripts::ViewGuardbandRequest { x_bias, y_bias });
    }

    pub fn queue_pending_script_fps_limit(&mut self, fps: i32) {
        self.pending_script_fps_limit = Some(fps);
    }

    pub fn queue_pending_camera_bw_mode(&mut self, enabled: bool, frames: i32) {
        self.pending_camera_bw_mode =
            Some(crate::game_logic::mission_scripts::CameraBwModeRequest { enabled, frames });
    }

    pub fn queue_pending_camera_shaker(
        &mut self,
        position: Vec3,
        amplitude: f32,
        duration_seconds: f32,
        radius: f32,
    ) {
        self.pending_camera_add_shakers.push(
            crate::game_logic::mission_scripts::CameraAddShakerRequest {
                position,
                amplitude,
                duration_seconds,
                radius,
            },
        );
    }

    pub fn set_script_time_frozen_for_test(&mut self, frozen: bool) {
        self.script_time_frozen_by_script = frozen;
    }

    pub fn take_camera_focus_request(&mut self) -> Option<Vec3> {
        self.pending_camera_focus.take()
    }

    pub fn script_default_camera_pitch(&self) -> f32 {
        self.script_default_camera_pitch
    }

    pub fn script_default_camera_max_height(&self) -> f32 {
        self.script_default_camera_max_height
    }

    pub fn visual_speed_multiplier(&self) -> f32 {
        self.visual_speed_multiplier
    }

    pub fn is_script_camera_time_frozen(&self) -> bool {
        let move_or_path = self
            .script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_time())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_time())
                .unwrap_or(false);
        if move_or_path {
            return true;
        }
        // C++ GameLogic: isTimeFrozen() && !isCameraMovementFinished()
        // (rotate/zoom/pitch/look-toward count as unfinished).
        self.script_camera_freeze_time && !self.is_script_camera_movement_finished_now()
    }

    pub fn take_camera_zoom_reset(&mut self) -> bool {
        self.pending_camera_zoom_reset_duration = 0.0;
        self.pending_camera_zoom_reset_ease_in = 0.0;
        self.pending_camera_zoom_reset_ease_out = 0.0;
        std::mem::take(&mut self.pending_camera_zoom_reset)
    }

    pub fn take_camera_zoom_request(&mut self) -> Option<CameraZoomRequest> {
        self.pending_camera_zoom.take()
    }

    pub fn take_camera_pitch_request(&mut self) -> Option<CameraPitchRequest> {
        self.pending_camera_pitch.take()
    }

    pub fn take_camera_rotate_request(&mut self) -> Option<CameraRotateRequest> {
        self.pending_camera_rotate.take()
    }

    pub fn take_camera_look_toward_request(&mut self) -> Option<CameraLookTowardWaypointRequest> {
        self.pending_camera_look_toward.take()
    }

    pub fn take_camera_slave_mode_enable_request(&mut self) -> Option<CameraSlaveModeRequest> {
        self.pending_camera_slave_mode_enable.take()
    }

    pub fn take_camera_slave_mode_disable_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_camera_slave_mode_disable)
    }

    pub fn take_screen_shake_requests(&mut self) -> Vec<ScreenShakeRequest> {
        std::mem::take(&mut self.pending_screen_shakes)
    }

    pub fn take_camera_add_shaker_requests(&mut self) -> Vec<CameraAddShakerRequest> {
        std::mem::take(&mut self.pending_camera_add_shakers)
    }

    pub fn take_popup_message_requests(&mut self) -> Vec<ScriptPopupMessageRequest> {
        std::mem::take(&mut self.pending_popup_messages)
    }

    pub fn take_view_guardband_request(&mut self) -> Option<ViewGuardbandRequest> {
        self.pending_view_guardband.take()
    }

    pub fn take_camera_bw_mode_request(&mut self) -> Option<CameraBwModeRequest> {
        self.pending_camera_bw_mode.take()
    }

    pub fn take_camera_motion_blur_requests(&mut self) -> Vec<CameraMotionBlurRequest> {
        std::mem::take(&mut self.pending_camera_motion_blur)
    }

    pub fn queue_pending_movie(&mut self, name: impl Into<String>) {
        self.pending_movie = Some(name.into());
    }

    pub fn queue_pending_radar_movie(&mut self, name: impl Into<String>) {
        self.pending_radar_movie = Some(name.into());
    }

    pub fn queue_pending_music_stop(&mut self) {
        self.pending_music_stop = true;
    }

    pub fn queue_pending_popup_message(&mut self, message: impl Into<String>) {
        // Presentation retains the one currently active C++ popup, never an
        // historical list of already-replaced dialogs.
        self.pending_popup_messages.clear();
        self.pending_popup_messages.push(
            crate::game_logic::mission_scripts::ScriptPopupMessageRequest {
                message: message.into(),
                x_percent: 50,
                y_percent: 50,
                width: 40,
                pause: false,
                pause_music: false,
                // This direct presentation-test helper does not create a
                // live GameClient popup, so it deliberately has no host ACK
                // identity. Script hooks assign nonzero identities instead.
                popup_generation: 0,
            },
        );
    }

    /// Opaque identity of the one Main-retained popup, if it was emitted by a
    /// live mission-script hook.  Zero is intentionally not acknowledgeable.
    pub(crate) fn active_popup_message_generation(&self) -> Option<usize> {
        self.pending_popup_messages
            .last()
            .and_then(|popup| (popup.popup_generation != 0).then_some(popup.popup_generation))
    }

    pub fn peek_pending_movie(&self) -> Option<&str> {
        self.pending_movie.as_deref()
    }

    pub fn peek_pending_radar_movie(&self) -> Option<&str> {
        self.pending_radar_movie.as_deref()
    }

    /// Consume pending script movie (after presentation freeze/apply).
    pub fn take_pending_movie(&mut self) -> Option<String> {
        self.pending_movie.take()
    }

    /// Consume pending radar movie (after presentation freeze/apply).
    pub fn take_pending_radar_movie(&mut self) -> Option<String> {
        self.pending_radar_movie.take()
    }

    pub fn peek_pending_music_stop(&self) -> bool {
        self.pending_music_stop
    }

    pub fn peek_pending_popup_messages(
        &self,
    ) -> &[crate::game_logic::mission_scripts::ScriptPopupMessageRequest] {
        &self.pending_popup_messages
    }

    pub fn take_music_stop_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_music_stop)
    }

    pub fn take_script_fps_limit_request(&mut self) -> Option<i32> {
        self.pending_script_fps_limit.take()
    }

    pub fn is_script_time_frozen(&self) -> bool {
        self.script_time_frozen_by_script
    }

    pub fn is_time_frozen_for_simulation(&self) -> bool {
        self.is_script_time_frozen() || self.is_script_camera_time_frozen()
    }

    pub fn set_camera_follow_object(&mut self, id: Option<ObjectId>) {
        self.camera_follow_target = id;
        if id.is_none() {
            self.camera_tether_play = None;
        }
        if let Some(oid) = id {
            if let Some(obj) = self.objects.get(&oid) {
                self.request_camera_focus(obj.get_position());
            }
        }
    }

    pub fn set_camera_tether_object(&mut self, id: ObjectId, snap_to_unit: bool, play: f32) {
        self.camera_follow_target = Some(id);
        self.camera_tether_play = Some(play.max(0.0));
        if snap_to_unit {
            if let Some(obj) = self.objects.get(&id) {
                self.request_camera_focus(obj.get_position());
            }
        }
    }

    pub fn peek_camera_tether_play(&self) -> Option<f32> {
        self.camera_tether_play
    }

    pub fn camera_follow_object_id(&self) -> Option<ObjectId> {
        self.camera_follow_target
    }

    pub fn camera_follow_target_position(&mut self) -> Option<Vec3> {
        let target = self.camera_follow_target?;
        let Some(obj) = self.objects.get(&target) else {
            self.camera_follow_target = None;
            return None;
        };
        if !obj.is_alive() {
            self.camera_follow_target = None;
            return None;
        }
        Some(obj.get_position())
    }

    /// Peek camera-follow world position without clearing the follow target.
    /// Used to freeze presentation residual each frame.
    pub fn peek_camera_follow_target_position(&self) -> Option<Vec3> {
        let target = self.camera_follow_target?;
        let obj = self.objects.get(&target)?;
        if !obj.is_alive() {
            return None;
        }
        Some(obj.get_position())
    }

    /// Execute a single command
    pub(in super::super) fn execute_command(
        &mut self,
        command: crate::command_system::GameCommand,
    ) {
        let command_type = command.command_type.clone();
        let accepted_gather_metadata = match &command.command_type {
            crate::command_system::CommandType::Gather { target_id } => Some((
                command.command_id,
                command.timestamp.clone(),
                command.player_id,
                *target_id,
            )),
            _ => None,
        };
        let mut executor = crate::command_executor::CommandExecutor::new(self, command.player_id);
        let result = executor.execute_command(command);
        let accepted_carrier_ids = executor.take_accepted_gather_carrier_ids();
        drop(executor);

        // The command executor, rather than input/UI code, decides which
        // selected workers actually accepted the Gather order.  Preserve that
        // precise carrier subset as a typed event; Main later supplies the
        // physical mouse provenance only for a matching right-click command.
        if matches!(
            result.as_ref(),
            Ok(crate::command_system::CommandResult::Success)
        ) {
            if let Some((command_id, issued_at, player_id, target_id)) = accepted_gather_metadata {
                self.record_accepted_gather_command(crate::game_logic::AcceptedGatherCommand {
                    command_id,
                    issued_at,
                    player_id,
                    target_id,
                    carrier_ids: accepted_carrier_ids,
                });
            }
        }

        match result {
            Ok(crate::command_system::CommandResult::Success) => {}
            Ok(result) => {
                log::debug!(
                    "[GameLogic] Command {:?} completed with {:?}",
                    command_type,
                    result
                );
            }
            Err(err) => {
                log::warn!(
                    "[GameLogic] Failed to execute command {:?}: {}",
                    command_type,
                    err
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use gamelogic::helpers::{EvaEvent, TheEva};

    #[test]
    fn black_market_with_mp_count_fires_base_under_attack() {
        // C++ Radar.cpp:1194 — STRUCTURE + MP_COUNT_FOR_VICTORY, not FS union.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::GLA, "Local", true));
        let mut st = ThingTemplate::new("GLABlackMarket");
        st.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .add_kind_of(KindOf::MpCountForVictory)
            .set_health(500.0);
        logic.templates.insert("GLABlackMarket".into(), st);
        let id = logic
            .create_object(
                "GLABlackMarket",
                Team::GLA,
                glam::Vec3::new(10.0, 0.0, 20.0),
            )
            .expect("market");
        assert!(logic.try_under_attack_event(id));
        assert!(logic.honesty_eva_base_under_attack_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events.iter().any(|e| *e == EvaEvent::BaseUnderAttack),
            "{events:?}"
        );
    }

    #[test]
    fn structure_without_mp_count_does_not_fire_base_under_attack() {
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        let mut st = ThingTemplate::new("AmericaCommandCenter");
        st.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(5000.0);
        logic.templates.insert("AmericaCommandCenter".into(), st);
        let id = logic
            .create_object(
                "AmericaCommandCenter",
                Team::USA,
                glam::Vec3::new(10.0, 0.0, 20.0),
            )
            .expect("cc");
        assert!(logic.try_under_attack_event(id));
        assert!(!logic.honesty_eva_base_under_attack_ok());
    }

    #[test]
    fn same_faction_ally_cc_fires_ally_under_attack_not_base() {
        // Two USA slots: victim owner is the ally, not the local player.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        local.alliance_team = 1;
        let mut ally = Player::new(1, Team::USA, "Ally", false);
        ally.alliance_team = 1;
        logic.players.insert(0, local);
        logic.players.insert(1, ally);
        let mut st = ThingTemplate::new("AmericaCommandCenter");
        st.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::MpCountForVictory)
            .set_health(5000.0);
        logic.templates.insert("AmericaCommandCenter".into(), st);
        let id = logic
            .create_object_for_player("AmericaCommandCenter", 1, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("ally cc");
        assert!(logic.try_under_attack_event(id));
        assert!(!logic.honesty_eva_base_under_attack_ok());
        assert!(logic.honesty_eva_ally_under_attack_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events.iter().any(|e| *e == EvaEvent::AllyUnderAttack),
            "{events:?}"
        );
        assert!(
            !events.iter().any(|e| *e == EvaEvent::BaseUnderAttack),
            "{events:?}"
        );
    }

    #[test]
    fn try_under_attack_from_damage_skips_non_local() {
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
        let mut t = ThingTemplate::new("ChinaTankBattleMaster");
        t.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic.templates.insert("ChinaTankBattleMaster".into(), t);
        let victim = logic
            .create_object_for_player("ChinaTankBattleMaster", 1, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("ai victim");
        assert!(
            !logic.try_under_attack_from_damage(victim),
            "AI-vs-AI damage must not warn the local player"
        );
        assert!(!logic.honesty_under_attack_event_ok());

        let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
        ranger_t.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger_t);
        let local = logic
            .create_object_for_player("AmericaInfantryRanger", 0, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("local ranger");
        assert!(logic.try_under_attack_from_damage(local));
        assert!(logic.honesty_under_attack_event_ok());
    }
    #[test]
    fn campaign_player_allies_cc_fires_ally_under_attack() {
        // alliance_team stays -1; map playerAllies must still be ALLIES.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        let mut china = Player::new(1, Team::China, "China", false);
        local.set_map_relationship(1, gamelogic::common::Relationship::Allies);
        logic.players.insert(0, local);
        logic.players.insert(1, china);
        let mut st = ThingTemplate::new("ChinaCommandCenter");
        st.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::MpCountForVictory)
            .set_health(5000.0);
        logic.templates.insert("ChinaCommandCenter".into(), st);
        let id = logic
            .create_object_for_player("ChinaCommandCenter", 1, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("china cc");
        assert!(logic.try_under_attack_event(id));
        assert!(!logic.honesty_eva_base_under_attack_ok());
        assert!(logic.honesty_eva_ally_under_attack_ok());
        let events = TheEva::drain_events().expect("eva");
        assert!(
            events.iter().any(|e| *e == EvaEvent::AllyUnderAttack),
            "{events:?}"
        );
    }

    #[test]
    fn same_faction_ally_death_does_not_fire_unit_lost() {
        // C++ isLocallyControlled, not faction Team. USA 2v2 ally deaths stay silent.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic
            .players
            .insert(1, Player::new(1, Team::USA, "Ally", false));
        let mut t = ThingTemplate::new("AmericaInfantryRanger");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("AmericaInfantryRanger".into(), t);
        let ally_id = logic
            .create_object_for_player("AmericaInfantryRanger", 1, glam::Vec3::ZERO)
            .expect("ally ranger");
        assert!(!logic.is_object_locally_controlled(ally_id));
        logic.try_eva_on_local_object_death(
            ally_id,
            Team::USA,
            false,
            true,
            false,
            false,
            glam::Vec3::ZERO,
            Some(Team::GLA),
        );
        assert_eq!(logic.saboteur.eva_unit_lost, 0);
        let events = TheEva::drain_events().unwrap_or_default();
        assert!(
            !events.iter().any(|e| *e == EvaEvent::UnitLost),
            "{events:?}"
        );
    }

    #[test]
    fn same_faction_teamkill_of_local_unit_fires_unit_lost() {
        // C++ selfInflicted is sourceID == victim ID, not same-faction killer Team.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic
            .players
            .insert(1, Player::new(1, Team::USA, "Ally", false));
        let mut t = ThingTemplate::new("AmericaInfantryRanger");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("AmericaInfantryRanger".into(), t);
        let mine = logic
            .create_object_for_player("AmericaInfantryRanger", 0, glam::Vec3::ZERO)
            .expect("local ranger");
        let ally = logic
            .create_object_for_player("AmericaInfantryRanger", 1, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("ally ranger");

        if let Some(obj) = logic.objects.get_mut(&mine) {
            obj.last_damage_source = Some(ally);
        }
        assert!(logic.is_object_locally_controlled(mine));
        logic.try_eva_on_local_object_death(
            mine,
            Team::USA,
            false,
            true,
            false,
            false,
            glam::Vec3::ZERO,
            Some(Team::USA),
        );
        assert!(logic.saboteur.honesty_eva_unit_lost_ok());
    }

    #[test]
    fn try_eva_insufficient_funds_has_no_host_frame_throttle() {
        // C++ ControlBarCommandProcessing setShouldPlay every failed click.
        // Eva.ini TimeBetweenChecksMS is leftover Eva, not a 900-frame host gate.
        let _ = TheEva::drain_events();
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "Local", true));
        logic.try_eva_insufficient_funds(0);
        logic.frame = 1;
        logic.try_eva_insufficient_funds(0);
        assert_eq!(logic.eva_insufficient_funds_count(), 2);
        let events = TheEva::drain_events().expect("eva");
        let funds = events
            .iter()
            .filter(|e| **e == EvaEvent::InsufficientFunds)
            .count();
        assert_eq!(funds, 2, "{events:?}");
    }
}
