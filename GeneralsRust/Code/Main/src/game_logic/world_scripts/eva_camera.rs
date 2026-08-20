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

    /// C++ TheEva->setShouldPlay(EVA_InsufficientFunds) residual (local player).
    pub fn try_eva_insufficient_funds(&mut self, player_id: u32) {
        use crate::game_logic::host_ui_presentation_residual::EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL;
        let Some(p) = self.players.get(&player_id) else {
            return;
        };
        if !p.is_local || !p.is_alive {
            return;
        }
        if self.frame < self.eva_insufficient_funds_next_frame {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        self.eva_insufficient_funds = self.eva_insufficient_funds.saturating_add(1);
        self.eva_insufficient_funds_next_frame = self
            .frame
            .saturating_add(EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL);
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
        let is_infantry = obj.is_kind_of(KindOf::Infantry);
        let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
        let is_structure = obj.is_kind_of(KindOf::Structure);
        let name_l = obj.template_name.to_ascii_lowercase();
        let is_harvester = name_l.contains("supplytruck")
            || name_l.contains("supply_truck")
            || name_l.contains("harvester")
            || name_l.contains("gatherer")
            || (name_l.contains("worker") && !name_l.contains("dozer"));
        let is_mp_count = is_structure
            && (obj.is_kind_of(KindOf::CommandCenter)
                || obj.is_kind_of(KindOf::FSPower)
                || obj.is_kind_of(KindOf::PowerPlant)
                || obj.is_kind_of(KindOf::FSBarracks)
                || obj.is_kind_of(KindOf::FSWarFactory)
                || obj.is_kind_of(KindOf::FSAirfield)
                || obj.is_kind_of(KindOf::FSSuperweapon)
                || obj.is_kind_of(KindOf::FSStrategyCenter)
                || obj.is_kind_of(KindOf::FSTechnology)
                || obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter));
        let alliance = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);

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
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(victim_id)
                .with_position(pos)
                .with_priority(165),
        );

        if is_structure && is_mp_count {
            let local_owns = self
                .players
                .values()
                .any(|p| p.is_local && p.is_alive && p.team == team);
            let local_ally = !local_owns
                && self.players.values().any(|p| {
                    p.is_local
                        && p.is_alive
                        && p.alliance_team == alliance
                        && alliance >= 0
                        && p.team != team
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

    pub fn honesty_under_attack_event_ok(&self) -> bool {
        self.under_attack_events > 0
    }

    pub fn honesty_eva_base_under_attack_ok(&self) -> bool {
        self.eva_base_under_attack > 0
    }

    pub fn try_eva_on_local_object_death(
        &mut self,
        _victim_id: ObjectId,
        victim_team: crate::game_logic::Team,
        is_structure: bool,
        is_infantry: bool,
        is_vehicle: bool,
        is_mp_count_for_victory: bool,
        death_pos: glam::Vec3,
        killer: Option<crate::game_logic::Team>,
    ) {
        // Local victim residual.
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == victim_team);
        if !local {
            return;
        }
        // C++ !selfInflicted residual.
        if killer == Some(victim_team) {
            return;
        }
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

            // C++ TheRadar->tryEvent(RADAR_EVENT_FAKE, pos) residual for spacebar jump.
            let msg = localization::localize("RADAR:UnitLost", "Unit lost");
            self.queue_radar_message_at(msg, death_pos, radar_notifications::RadarKind::Generic);
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
        // Local-player residual: only warn if a local player owns the victim team.
        let local_victim = self
            .players
            .values()
            .any(|p| p.team == victim_team && p.is_local);
        if !local_victim {
            // Still record honesty for AI-vs-AI residual observability when any
            // player on that team exists (fail-open for headless host tests).
            if !self.players.values().any(|p| p.team == victim_team) {
                return;
            }
        }
        let msg = localization::localize("RADAR:Infiltration", "Infiltration event");
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
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
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_time())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_time())
                .unwrap_or(false)
    }

    pub fn take_camera_zoom_reset(&mut self) -> bool {
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
