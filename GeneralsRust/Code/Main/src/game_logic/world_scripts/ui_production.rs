//! Host scripts `impl GameLogic` — `ui_production`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! update_ui_state / build legality / production / stealth science
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Update UI state from game logic
    /// This method extracts all data needed for UI rendering each frame
    /// Matches pattern from C++ InGameUI::preDraw() (InGameUI.h line 466)
    pub fn update_ui_state(&mut self, player_id: u32) -> crate::ui::GameUIState {
        use crate::ui::{
            BuildQueueEntry, GameUIState, MinimapDot, RadarMessageEntry, RadarPing, RadarPingKind,
            UnitDisplayInfo,
        };

        // Get player associated with the current viewport/camera
        let player = self.players.get(&player_id);

        let (credits, power_generated, power_used, max_power, credits_per_second) =
            if let Some(p) = player {
                let (produced, consumed) =
                    super::super::buildings::BuildingBehavior::calculate_power_for_team(
                        p.team,
                        &self.objects,
                    );
                (
                    p.resources.supplies as i32,
                    produced,
                    consumed,
                    produced,
                    0.0,
                )
            } else {
                (10000, 100, 60, 100, 0.0)
            };

        // Get selected units
        let mut selected_units = Vec::new();
        let mut selected_unit_infos = Vec::new();

        if let Some(player) = player {
            for &object_id in &player.selected_objects {
                selected_units.push(object_id);

                if let Some(obj) = self.objects.get(&object_id) {
                    selected_unit_infos.push(UnitDisplayInfo {
                        object_id,
                        name: obj.name.clone(),
                        health_current: obj.health.current,
                        health_maximum: obj.health.maximum,
                        unit_type: format!("{:?}", obj.object_type),
                        current_order: if obj.target.is_some() {
                            "Attacking".to_string()
                        } else if obj.movement.target_position.is_some() {
                            "Moving".to_string()
                        } else {
                            "Idle".to_string()
                        },
                        veterancy_overlay: None,
                        production_progress: None,
                        production_template: None,
                        command_set_override: obj.command_set_override.clone().unwrap_or_default(),
                        can_produce: obj.building_data.is_some()
                            && !obj.status.under_construction
                            && obj.construction_percent >= 1.0,
                        production_is_upgrade: false,
                        production_paused: false,
                    });
                }
            }
        }

        // Get build queues (from all constructing buildings)
        let mut build_queue = Vec::new();
        for obj in self.objects.values() {
            if obj.status.under_construction {
                // Estimate time remaining based on construction percent (assuming 30 second build time)
                let estimated_total_time = 30.0;
                let time_remaining = estimated_total_time * (1.0 - obj.construction_percent);

                build_queue.push(BuildQueueEntry {
                    template_name: obj.name.clone(),
                    percent_complete: obj.construction_percent,
                    time_remaining,
                });
            }
        }

        // Generate minimap dots for all units
        let mut minimap_unit_dots = Vec::new();
        let (world_min, world_max) = self.world_bounds();
        let world_span_x = (world_max.x - world_min.x).max(1.0);
        let world_span_z = (world_max.z - world_min.z).max(1.0);
        let viewing_team = player.map(|p| p.team).unwrap_or(Team::Neutral);
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);

        for (id, obj) in &self.objects {
            if obj.is_alive()
                && (obj.is_kind_of(KindOf::Selectable) || obj.is_kind_of(KindOf::Structure))
                && Self::is_object_visible_on_minimap_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                )
            {
                // Normalize position to 0.0-1.0 range based on world dimensions
                let normalized_x = ((obj.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
                let normalized_y = ((obj.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);

                let color = match obj.team {
                    Team::USA => color_for_player(1),
                    Team::China => color_for_player(0),
                    Team::GLA => color_for_player(4),
                    Team::Neutral => color_for_player(7),
                };

                let size = if obj.is_kind_of(KindOf::Structure) {
                    4.0
                } else {
                    2.0
                };

                minimap_unit_dots.push(MinimapDot::normalized(
                    normalized_x,
                    normalized_y,
                    color,
                    size,
                ));
            }
        }

        let mut minimap_beacons = Vec::new();
        for beacon in snapshot_beacons() {
            let pos = glam::Vec3::new(beacon.position.x, beacon.position.y, beacon.position.z);
            if crate::command_executor::host_beacon_position_is_hidden(self, pos) {
                continue;
            }
            let normalized_x = ((beacon.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
            let normalized_y = ((beacon.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);
            minimap_beacons.push(MinimapDot::normalized(
                normalized_x,
                normalized_y,
                color_for_player(beacon.player_id as u8),
                4.0,
            ));
        }

        // Use WW3D-synchronized time
        let game_time = self.sim_time_seconds;

        let player_name = player
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Commander {}", player_id + 1));

        let mut ui_state = GameUIState::default();
        ui_state.credits = credits;
        ui_state.power_generated = power_generated;
        ui_state.power_used = power_used;
        ui_state.max_power = max_power;
        ui_state.credits_per_second = credits_per_second;
        ui_state.player_id = player_id;
        ui_state.player_name = player_name;
        ui_state.selected_units = selected_units;
        ui_state.selected_unit_infos = selected_unit_infos;
        // Live path fills panel; production overlay replaces with PresentationFrame.
        ui_state.selection_panel = crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &ui_state.selected_unit_infos,
        );
        ui_state.build_queue = build_queue;
        ui_state.is_game_paused = self.is_paused;
        ui_state.current_game_time = game_time;
        ui_state.fps = LOGIC_FRAMES_PER_SECOND;
        ui_state.frame_time_ms = 1000.0 / LOGIC_FRAMES_PER_SECOND;
        ui_state.performance_score = 1.0;
        ui_state.minimap_unit_dots = minimap_unit_dots;
        ui_state.minimap_beacons = minimap_beacons.clone();
        ui_state.new_beacons = std::mem::take(&mut self.recent_beacons)
            .into_iter()
            .filter(|p| !crate::command_executor::host_beacon_position_is_hidden(self, *p))
            .collect();
        ui_state.minimap_viewport = crate::ui::default_minimap_viewport();
        ui_state.minimap_view_box = crate::ui::default_minimap_view_box();
        ui_state.minimap_texture_id = None;
        ui_state.minimap_coordinates = Some(crate::graphics::MinimapCoordinates {
            minimap_width: 1.0,
            minimap_height: 1.0,
            world_min,
            world_max,
            screen_pos: Vec2::ZERO,
        });

        // Pull fresh radar updates from GameLogic (typed) and turn them into HUD/radar pings.
        for update in radar_notifier::drain() {
            let pos_world = Vec3::new(update.position.0, 0.0, update.position.1);
            match update.event_type {
                RadarEventType::BaseAttacked => {
                    self.queue_radar_attack_at("Base under attack", pos_world);
                }
                RadarEventType::EnemyDetected => {
                    self.queue_radar_message_at(
                        "Enemy detected",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitCreated => {
                    self.queue_radar_message_at(
                        "Unit ready",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitDestroyed => {
                    self.queue_radar_message_at(
                        "Unit lost",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::BeaconPlaced | RadarEventType::BeaconRemoved => {
                    // Beacon events are already handled via beacon manager; skip to avoid duplicates.
                }
            }
        }

        let radar_entries = self.radar_notifications.drain();
        const RADAR_PING_LIFETIME: f32 = 6.0;
        let mut latest_by_kind: [Option<RadarEntry>; 3] = [None, None, None];
        ui_state.radar_messages = radar_entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect();
        ui_state.radar_events = radar_entries
            .iter()
            .map(|entry| RadarMessageEntry {
                text: entry.text.clone(),
                position: Some(entry.position),
                kind: match entry.kind {
                    radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                    radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                    radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                },
            })
            .collect();
        ui_state.radar_pings = radar_entries
            .iter()
            .filter_map(|entry| {
                let age = (self.sim_time_seconds - entry.timestamp).max(0.0);
                if age > RADAR_PING_LIFETIME {
                    return None;
                }
                // Fade out linearly and add a soft pulse to mimic C++ radar blips.
                let normalized = (1.0 - age / RADAR_PING_LIFETIME).clamp(0.0, 1.0);
                let pulse = 0.5 * (1.0 + (age * std::f32::consts::TAU).cos());
                let intensity = (normalized * 0.6 + pulse * 0.4).clamp(0.0, 1.0);
                Some(RadarPing {
                    position: entry.position,
                    intensity,
                    age_seconds: age,
                    kind: match entry.kind {
                        radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                        radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                        radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                    },
                })
            })
            .collect();
        for entry in radar_entries {
            let idx = match entry.kind {
                radar_notifications::RadarKind::Generic => 0,
                radar_notifications::RadarKind::Attack => 1,
                radar_notifications::RadarKind::Ally => 2,
            };
            let slot = &mut latest_by_kind[idx];
            if slot
                .as_ref()
                .map(|e| entry.timestamp >= e.timestamp)
                .unwrap_or(true)
            {
                *slot = Some(entry);
            }
        }
        if let Some(entry) = latest_by_kind
            .iter()
            .filter_map(|e| e.as_ref())
            .max_by(|a, b| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            self.last_radar_event = Some(entry.clone());
        }
        ui_state.last_radar_ping = self.last_radar_event.as_ref().map(|e| e.position);
        ui_state.script_messages = self
            .script_broadcasts
            .iter()
            .map(|msg| msg.text.clone())
            .collect();
        ui_state.cinematic_letterbox = self.cinematic_letterbox;
        ui_state.cinematic_text = self.cinematic_text.as_ref().map(|(text, _)| text.clone());
        ui_state.military_caption = self.military_caption.as_ref().map(|(text, _)| text.clone());
        // C++ W3DControlBar / ControlBarCallback:
        // isRadarForced() || (!isRadarHidden() && player->hasRadar())
        // radar_enabled here is script "not hidden"; has_radar is ownership residual.
        let local_has_radar = self
            .local_player_id()
            .and_then(|id| self.get_player(id))
            .map(|p| p.has_radar())
            .unwrap_or(false);
        ui_state.radar_enabled = self.radar_forced || (self.radar_enabled && local_has_radar);
        ui_state.radar_forced = self.radar_forced;
        ui_state.objectives = self.mission_objectives.clone();
        ui_state
    }

    /// Active script broadcast texts residual (presentation freeze).
    pub fn script_broadcast_texts(&self) -> Vec<String> {
        self.script_broadcasts
            .iter()
            .map(|msg| msg.text.clone())
            .collect()
    }

    /// Pending script messages this frame (presentation freeze; non-draining).
    pub fn peek_new_script_messages(&self) -> &[String] {
        &self.new_script_messages
    }

    pub fn cinematic_letterbox(&self) -> bool {
        self.cinematic_letterbox
    }

    #[cfg(test)]
    pub(crate) fn set_cinematic_letterbox(&mut self, enabled: bool) {
        self.cinematic_letterbox = enabled;
    }

    pub fn cinematic_text(&self) -> Option<&str> {
        self.cinematic_text.as_ref().map(|(t, _)| t.as_str())
    }

    /// C++ FONT_NAME leftover for `doDisplayCinematicText`.
    pub fn cinematic_font(&self) -> Option<&str> {
        self.cinematic_font.as_deref()
    }

    pub fn military_caption_text(&self) -> Option<&str> {
        self.military_caption.as_ref().map(|(t, _)| t.as_str())
    }

    /// Remaining military caption lifetime in milliseconds (0 if expired/absent).
    pub fn military_caption_remaining_ms(&self) -> Option<i32> {
        self.military_caption.as_ref().map(|(_, expiry)| {
            let rem = (*expiry - self.sim_time_seconds).max(0.0);
            (rem * 1000.0).round() as i32
        })
    }

    /// Remaining cinematic text lifetime in milliseconds.
    pub fn cinematic_text_remaining_ms(&self) -> Option<i32> {
        self.cinematic_text.as_ref().map(|(_, expiry)| {
            let rem = (*expiry - self.sim_time_seconds).max(0.0);
            (rem * 1000.0).round() as i32
        })
    }

    pub fn radar_script_enabled(&self) -> bool {
        self.radar_enabled
    }

    pub fn radar_forced(&self) -> bool {
        self.radar_forced
    }

    /// Push a script/UI message residual (broadcast + new-message feed).
    pub fn push_script_ui_message<S: Into<String>>(&mut self, message: S) {
        let msg = message.into();
        if msg.is_empty() {
            return;
        }
        self.script_broadcasts.push(ScriptBroadcast {
            text: msg.clone(),
            expires_at: self.sim_time_seconds + 10.0,
        });
        self.new_script_messages.push(msg);
    }

    pub fn set_cinematic_text(&mut self, text: Option<String>) {
        if text.is_none() {
            self.cinematic_font = None;
        }
        self.cinematic_text = text.map(|t| (t, self.sim_time_seconds + 10.0));
    }

    pub fn set_military_caption(&mut self, text: Option<String>) {
        self.military_caption = text.map(|t| (t, self.sim_time_seconds + 10.0));
    }

    pub fn set_radar_forced(&mut self, forced: bool) {
        self.radar_forced = forced;
    }

    pub fn take_new_script_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.new_script_messages)
    }

    /// Queue a command from the UI
    pub fn queue_command(&mut self, command: crate::command_system::GameCommand) {
        log::trace!("Queuing command: {:?}", command.command_type);
        crate::command_system::tap_host_command_for_recorder(&command);
        self.command_queue.push_back(command);
    }

    /// Process queued commands
    /// Wave 914: true when command queue has pending authority work.
    #[inline]
    pub fn has_pending_commands(&self) -> bool {
        !self.command_queue.is_empty()
    }

    /// Wave 914/915: process command queue only when non-empty (skip empty dual-write path).
    /// Returns whether any commands were processed.
    #[inline]
    pub fn process_commands_if_needed(&mut self) -> bool {
        // C++ Player::update posts MSG_ENABLE_RETALIATION_MODE once per second.
        self.leftover_dispatch_tick();
        if self.command_queue.is_empty() {
            return false;
        }
        self.process_commands();
        true
    }

    /// Wave 922: queue one command then process if the queue is non-empty.
    #[inline]
    pub fn queue_and_process_command(
        &mut self,
        command: crate::command_system::GameCommand,
    ) -> bool {
        self.queue_command(command);
        self.process_commands_if_needed()
    }

    pub fn process_commands(&mut self) {
        // Process all queued commands
        crate::command_system::flush_recorder_and_replay_authority(&mut self.command_queue);
        while let Some(command) = self.command_queue.pop_front() {
            self.execute_command(command);
        }
        // Standalone command processing (unit tests / host gates without a full
        // sim tick) must still settle deferred economy/damage authority logs.
        if !crate::gameworld_shadow::shadow_coupled_tick_active() {
            crate::gameworld_shadow::materialize_host_economy_pending(self);
        }
    }

    /// Snapshot number of active beacons (used by HUD to clear highlights).

    /// Object IDs currently following a path (pathfinding step residual).
    ///
    /// Prefer this over iterating every object key each frame.
    pub fn object_ids_with_active_path(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.movement.path.is_empty()
                    && o.movement.current_path_index < o.movement.path.len()
            })
            .map(|(&id, _)| id)
            .collect()
    }

    /// Peek beacon placements queued this frame (HUD bloom residual).
    pub fn recent_beacons(&self) -> &[glam::Vec3] {
        &self.recent_beacons
    }

    /// Drain beacon placements queued this frame (presentation / UI residual).
    pub fn drain_recent_beacons(&mut self) -> Vec<glam::Vec3> {
        std::mem::take(&mut self.recent_beacons)
    }

    pub fn beacon_count(&self) -> usize {
        snapshot_beacons().len()
    }

    /// C++ `ThingTemplate` geometry used by `iteratePotentialCollisions`.
    /// Returns (major, minor, is_box).
    pub(crate) fn structure_place_footprint(&self, template_name: &str) -> (f32, f32, bool) {
        if let Some(tmpl) = self.templates.get(template_name) {
            if tmpl.geometry_info.authored {
                let g = &tmpl.geometry_info;
                let is_box = matches!(g.geom_type, crate::game_logic::HostGeometryType::Box);
                let major = g.major_radius.max(1.0);
                let minor = if is_box {
                    g.minor_radius.max(1.0)
                } else {
                    major
                };
                return (major, minor, is_box);
            }
        }
        if let Some(footprint) = leftover_structure_place_footprint(template_name) {
            return footprint;
        }
        let r = crate::game_logic::host_production_buildable_command_residual::STRUCTURE_PLACE_CLEARANCE_RESIDUAL
            * 0.5;
        (r, r, false)
    }

    pub(crate) fn structure_place_radius_for_template(&self, template_name: &str) -> f32 {
        let (major, minor, is_box) = self.structure_place_footprint(template_name);
        if is_box {
            (major * major + minor * minor).sqrt()
        } else {
            major
        }
    }

    /// Structure placement radius residual for LBC_OBJECTS_IN_THE_WAY.
    pub(in super::super) fn structure_place_radius(obj: &Object) -> f32 {
        if obj.thing.template.geometry_info.authored {
            return obj
                .thing
                .template
                .geometry_info
                .bounding_circle_radius()
                .max(1.0);
        }
        if obj.selection_radius > 1.0 {
            obj.selection_radius
        } else {
            crate::game_logic::host_production_buildable_command_residual::STRUCTURE_PLACE_CLEARANCE_RESIDUAL
                * 0.5
        }
    }

    /// C++ BuildAssistant::isLocationLegalToBuild residual (subset).
    ///
    /// Checks world bounds, living structure overlap, and for supply centers
    /// LBC_TOO_CLOSE_TO_SUPPLIES vs SUPPLY_SOURCE residual. Fail-closed vs full
    /// terrain slope / shroud graph.
    pub fn legal_build_code_at(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
    ) -> u32 {
        self.legal_build_code_at_for_builder(team, position, template_name, None)
    }

    /// C++ isLocationLegalToBuild with optional builder for CLEAR_PATH residual.
    /// Confirm-place: IGNORE_STEALTHED | FAIL_STEALTHED_WITHOUT_FEEDBACK.
    pub fn legal_build_code_at_for_builder(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::{
            LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK, LOCAL_LEGAL_IGNORE_STEALTHED,
        };
        self.legal_build_code_at_for_builder_ex(
            team,
            position,
            template_name,
            builder_id,
            LOCAL_LEGAL_IGNORE_STEALTHED | LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK,
        )
    }

    /// Preview ghost: IGNORE_STEALTHED so unseen stealthed units do not redden.
    pub fn legal_build_code_at_for_preview(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::LOCAL_LEGAL_IGNORE_STEALTHED;
        self.legal_build_code_at_for_builder_ex(
            team,
            position,
            template_name,
            builder_id,
            LOCAL_LEGAL_IGNORE_STEALTHED,
        )
    }

    pub fn legal_build_code_at_for_builder_ex(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
        options: u32,
    ) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::{
            STRUCTURE_PLACE_CLEARANCE_RESIDUAL, cell_shroud_blocks_build_residual,
            footprint_height_delta_residual, legal_build_code_from_checks_complete_residual,
            legal_build_objects_in_the_way_residual, legal_build_too_close_to_supplies_residual,
            min_dist_from_map_edge_residual,
        };
        use crate::game_logic::host_structure_economy_residual::{
            MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD, SUPPLY_BUILD_BORDER,
            is_legal_build_distance_from_map_edge, is_legal_build_height_variation,
        };
        let (min, max) = self.world_bounds();
        // Use real map extent (no generous pad) for C++ off-map / edge residual.
        let min_x = min.x;
        let max_x = max.x;
        let min_z = min.z;
        let max_z = max.z;
        let in_bounds = position.x.is_finite()
            && position.z.is_finite()
            && position.x >= min_x
            && position.x <= max_x
            && position.z >= min_z
            && position.z <= max_z;
        let edge_dist = min_dist_from_map_edge_residual(
            (position.x, position.z),
            (min_x, min_z),
            (max_x, max_z),
        );
        let too_close_edge = in_bounds
            && MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD > 0.0
            && !is_legal_build_distance_from_map_edge(edge_dist);
        let (_, extra_bib) = leftover_factory_exit_widths(template_name);
        let place_r = self.structure_place_radius_for_template(template_name) + extra_bib.max(0.0);
        let builder = builder_id.and_then(|id| self.objects.get(&id));
        let mut blockers: Vec<(f32, f32, f32)> = Vec::new();
        let mut blocker_ids: Vec<ObjectId> = Vec::new();
        let mut supply_sources: Vec<(f32, f32, f32)> = Vec::new();
        let mut stealth_fail_no_bib = false;
        let mut busy_ally_in_way = false;
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(KindOf::Mine) {
                continue;
            }
            let p = obj.get_position();
            let r = Self::structure_place_radius(obj)
                + leftover_factory_exit_widths(&obj.template_name).1.max(0.0);
            if obj.is_kind_of(KindOf::SupplySource) {
                supply_sources.push((p.x, p.z, r.max(10.0)));
            }
            // C++ BuildAssistant.cpp:642-742 isLocationClearOfObjects:
            // reject immobile, disabled, and ENEMY. Mines are ignored.
            let rel = builder
                .map(|b| self.object_relationship(b, obj))
                .unwrap_or(gamelogic::common::Relationship::Neutral);
            // Patch 1.01: allied USING_ABILITY / isBusy cannot be scooted.
            if rel == gamelogic::common::Relationship::Allies
                && (obj.status.using_ability
                    || matches!(
                        obj.ai_state,
                        crate::game_logic::AIState::SpecialAbility
                            | crate::game_logic::AIState::Capturing
                    ))
            {
                let dx = p.x - position.x;
                let dz = p.z - position.z;
                if dx * dx + dz * dz < (place_r + r) * (place_r + r) {
                    busy_ally_in_way = true;
                }
            }
            // IGNORE_STEALTHED: unseen stealthed non-allies do not redden preview.
            // FAIL_STEALTHED_WITHOUT_FEEDBACK: confirm fails without a bib.
            if rel != gamelogic::common::Relationship::Allies && obj.is_effectively_stealthed() {
                use crate::game_logic::host_production_buildable_command_residual::{
                    LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK, LOCAL_LEGAL_IGNORE_STEALTHED,
                };
                if options & LOCAL_LEGAL_IGNORE_STEALTHED != 0 {
                    if options & LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK != 0 {
                        let dx = p.x - position.x;
                        let dz = p.z - position.z;
                        if dx * dx + dz * dz < (place_r + r) * (place_r + r) {
                            stealth_fail_no_bib = true;
                        }
                    }
                    continue;
                }
            }
            let blocks = obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Immobile)
                || obj.is_disabled()
                || rel == gamelogic::common::Relationship::Enemies;
            if blocks {
                blockers.push((p.x, p.z, r));
                blocker_ids.push(obj.id);
            }
            // C++ BuildAssistant.cpp:759-870 factory exit-width bibs.
            if let Some((ex, ez, er)) =
                leftover_factory_exit_blocker(&obj.template_name, p, obj.get_orientation())
            {
                blockers.push((ex, ez, er));
            }
        }
        if let Some((ex, ez, er)) = leftover_factory_exit_blocker(
            template_name,
            position,
            leftover_placement_view_angle(template_name),
        ) {
            if legal_build_objects_in_the_way_residual((ex, ez), er, &blockers) {
                return crate::game_logic::host_production_buildable_command_residual::LBC_OBJECTS_IN_THE_WAY;
            }
        }

        let in_way = busy_ally_in_way
            || legal_build_objects_in_the_way_residual(
                (position.x, position.z),
                place_r,
                &blockers,
            );
        if stealth_fail_no_bib {
            return crate::game_logic::host_production_buildable_command_residual::LBC_GENERIC_FAILURE;
        }
        if in_way && !busy_ally_in_way {
            self.bib_blocking_objects_for_build(position, &blocker_ids);
        }
        // KINDOF_CANNOT_BUILD_NEAR_SUPPLIES bit, rather than assigning the
        // rule to every supply-looking basename.
        let too_close = if self
            .templates
            .get(template_name)
            .is_some_and(|template| template.is_kind_of(KindOf::CannotBuildNearSupplies))
        {
            legal_build_too_close_to_supplies_residual(
                (position.x, position.z),
                place_r,
                &supply_sources,
                SUPPLY_BUILD_BORDER,
            )
        } else {
            false
        };
        // C++ SHROUD_REVEALED residual: require CELLSHROUD_CLEAR for human build.
        // When fog_of_war is off or no shroud grid is initialized, fail-open (clear).
        let shrouded = if !self.skirmish_rules.fog_of_war {
            false
        } else {
            let player_id = builder_id
                .and_then(|id| self.objects.get(&id))
                .and_then(|builder| builder.owner_player_id)
                .or_else(|| self.unique_player_id_for_team(team))
                .unwrap_or(0);
            let clear = self.is_build_location_shroud_clear(player_id, position);
            cell_shroud_blocks_build_residual(clear)
        };
        // C++ footprint height sample residual (hiZ-loZ > AllowedHeightVariation).
        // Fail-open when no height samples available (synthetic maps without terrain).
        let not_flat = self.footprint_not_flat_enough(position, place_r);
        // C++ CLEAR_PATH residual when a mobile builder is provided.
        let no_clear = match builder_id {
            Some(bid) => !self.builder_has_clear_path_to(bid, position),
            None => false,
        };
        // C++ BuildAssistant.cpp:491-499 CELL_WATER/CLIFF/IMPASSABLE and
        // :1006-1009 non-GROUND layer (bridge) → LBC_RESTRICTED_TERRAIN.
        if self.placement_terrain_restricted(position, place_r) {
            return crate::game_logic::host_production_buildable_command_residual::LBC_RESTRICTED_TERRAIN;
        }
        crate::game_logic::host_production_buildable_command_residual::legal_build_code_from_checks_with_path_residual(
            in_bounds,
            shrouded,
            not_flat,
            in_way,
            too_close,
            too_close_edge,
            no_clear,
        )
    }

    /// C++ `TheTerrainVisual->addFactionBib(them, TRUE)` on the blocking object.
    pub(crate) fn bib_blocking_objects_for_build(
        &self,
        _position: glam::Vec3,
        blocker_ids: &[ObjectId],
    ) {
        #[cfg(feature = "game_client")]
        {
            let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() else {
                return;
            };
            let Some(visual) = guard.as_mut() else {
                return;
            };
            for id in blocker_ids {
                let Some(obj) = self.objects.get(id) else {
                    continue;
                };
                if !obj.is_alive() {
                    continue;
                }
                let p = obj.get_position();
                let angle = obj.get_orientation();
                let transform = glam::Mat4::from_translation(glam::Vec3::new(p.x, p.y, p.z))
                    * glam::Mat4::from_rotation_y(angle);
                let (major, minor, is_box) = {
                    let g = &obj.thing.template.geometry_info;
                    if g.authored {
                        (
                            g.major_radius.max(1.0),
                            g.minor_radius.max(1.0),
                            g.minor_radius > 0.0 && (g.major_radius - g.minor_radius).abs() > 0.01,
                        )
                    } else {
                        let r = Self::structure_place_radius(obj).max(1.0);
                        (r, r, false)
                    }
                };
                let _ = visual.add_faction_bib(
                    id.0,
                    game_client::terrain::terrain_visual::TerrainBibOwnerKind::Object,
                    transform,
                    major,
                    minor,
                    is_box,
                    0.0,
                    0.0,
                    true,
                    0.0,
                );
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = blocker_ids;
        }
    }

    /// C++ BuildAssistant.cpp:491-499 / :1006-1009 terrain + bridge gate.
    pub(in super::super) fn placement_terrain_restricted(
        &self,
        position: glam::Vec3,
        place_radius: f32,
    ) -> bool {
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        let r = place_radius.max(1.0);
        let offsets = [
            (0.0, 0.0),
            (-r, -r),
            (r, -r),
            (-r, r),
            (r, r),
            (0.0, -r),
            (0.0, r),
            (-r, 0.0),
            (r, 0.0),
        ];
        for (dx, dz) in offsets {
            let sample = glam::Vec3::new(position.x + dx, position.y, position.z + dz);
            let cell = self.pathfinding_system.grid.world_to_grid(sample);
            match self.pathfinding_system.grid.cell_type(cell) {
                PathfindCellType::Water
                | PathfindCellType::Cliff
                | PathfindCellType::Impassable => return true,
                _ => {}
            }
        }
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            let dest = gamelogic::common::Coord3D::new(position.x, position.z, position.y);
            if terrain.get_layer_for_destination(&dest)
                != gamelogic::path::PathfindLayerEnum::Ground
            {
                return true;
            }
            if terrain.is_underwater(position.x, position.z, None, None)
                || terrain.is_cliff_cell(position.x, position.z)
            {
                return true;
            }
        }
        // C++ getLayerForDestination != LAYER_GROUND (bridge deck).
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let name = obj.template_name.to_ascii_lowercase();
            if !name.contains("bridge") {
                continue;
            }
            let p = obj.get_position();
            let r = Self::structure_place_radius(obj).max(place_radius);
            let dx = p.x - position.x;
            let dz = p.z - position.z;
            if dx * dx + dz * dz <= (r + place_radius) * (r + place_radius) {
                return true;
            }
        }
        false
    }

    /// C++ BuildAssistant::moveObjectsForConstruction — leftover bool contract.
    /// FALSE when any footprint occupant is an enemy or cannot be scooted
    /// (no AI / not mobile). Mines, inert, ALWAYS_SELECTABLE, and removable
    /// occupants are skipped. Allied/neutral mobiles are issued an aside.
    pub(crate) fn move_objects_for_construction(
        &mut self,
        location: glam::Vec3,
        place_r: f32,
        builder_id: Option<ObjectId>,
    ) -> bool {
        use game_engine::common::system::kind_of::KindOfMask;
        let builder = builder_id.and_then(|id| self.objects.get(&id)).cloned();
        let player_id = builder.as_ref().and_then(|b| b.owner_player_id);
        // Leftover `hypot(major,minor)*1.4` scoot radius.
        let aside_r = place_r * 1.4;
        let mut any_unmovables = false;
        let mut to_move: Vec<(ObjectId, glam::Vec3)> = Vec::new();
        for (id, obj) in self.objects.iter() {
            if builder_id == Some(*id) || !obj.is_alive() {
                continue;
            }
            let leftover = leftover_kindof_bits(&obj.template_name);
            if obj.is_kind_of(KindOf::Mine)
                || obj.is_kind_of(KindOf::Inert)
                || leftover & (KindOfMask::MINE.bits() | KindOfMask::INERT.bits()) != 0
            {
                continue;
            }
            if obj.is_kind_of(KindOf::AlwaysSelectable)
                || leftover & KindOfMask::ALWAYS_SELECTABLE.bits() != 0
                || self.occupant_is_removable_for_construction(obj)
            {
                continue;
            }
            let p = obj.get_position();
            let r = Self::structure_place_radius(obj);
            let dx = p.x - location.x;
            let dz = p.z - location.z;
            if dx * dx + dz * dz >= (place_r + r) * (place_r + r) {
                continue;
            }
            // Leftover `object_relationship_enemy` — C++ ENEMIES are unmovable.
            let enemy = if let Some(b) = &builder {
                self.object_relationship(b, obj) == gamelogic::common::Relationship::Enemies
            } else if let Some(pid) = player_id {
                obj.owner_player_id.is_some_and(|oid| {
                    self.player_relationship(pid, oid) == gamelogic::common::Relationship::Enemies
                })
            } else {
                false
            };
            if enemy {
                any_unmovables = true;
                continue;
            }
            // Leftover `move_object_aside`: C++ getAIUpdateInterface().
            // Live `is_mobile` is the host stand-in for getAI() (physics.rs).
            if leftover_has_ai_update(&obj.template_name).unwrap_or(false) || obj.is_mobile() {
                let dir = if dx * dx + dz * dz < 0.01 {
                    glam::Vec3::new(1.0, 0.0, 0.0)
                } else {
                    glam::Vec3::new(dx, 0.0, dz).normalize_or_zero()
                };
                to_move.push((*id, location + dir * aside_r.max(place_r + r + 8.0)));
            } else {
                any_unmovables = true;
            }
        }
        for (id, dest) in to_move {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_destination(dest);
            }
        }
        !any_unmovables
    }

    /// C++ `Player::getPlayerType()==PLAYER_HUMAN`. Leftover PlayerList wins;
    /// host `is_local` is the live stand-in when leftover is unbound.
    pub(crate) fn player_is_human(&self, player_id: u32) -> bool {
        if let Some(name) = self.players.get(&player_id).map(|p| p.name.clone()) {
            if let Some(human) = leftover_player_is_human(player_id, &name) {
                return human;
            }
        } else if let Some(human) = leftover_player_is_human(player_id, "") {
            return human;
        }
        self.players
            .get(&player_id)
            .map(|p| p.is_local)
            .unwrap_or(false)
    }

    /// C++ BuildAssistant::isRemovableForConstruction.
    fn occupant_is_removable_for_construction(&self, obj: &Object) -> bool {
        use game_engine::common::system::kind_of::KindOfMask;
        if obj.is_kind_of(KindOf::Inert) {
            return false;
        }
        let leftover = leftover_kindof_bits(&obj.template_name);
        if leftover & KindOfMask::INERT.bits() != 0 {
            return false;
        }
        if obj.is_kind_of(KindOf::Shrubbery) || obj.is_kind_of(KindOf::ClearedByBuild) {
            return true;
        }
        if leftover & (KindOfMask::SHRUBBERY.bits() | KindOfMask::CLEARED_BY_BUILD.bits()) != 0 {
            return true;
        }
        obj.status.effectively_dead
    }

    /// C++ DozerAIUpdate.cpp:1692-1696 flattenTerrain + getGroundHeight Z snap.
    /// C++ `TerrainLogic::flattenTerrain` uses GEOMETRY_BOX two-triangle coverage
    /// (TerrainLogic.cpp:2620-2746); cylinder/sphere uses majorRadius disk.
    pub(in super::super) fn flatten_and_snap_construction(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let pos = obj.get_position();
        let angle = obj.get_orientation();
        let geom = obj.thing.template.geometry_info;
        let radius = Self::structure_place_radius(obj).max(1.0);
        // C++ flattenTerrain returns immediately for GeometryIsSmall.
        let skip_flatten = geom.authored && geom.is_small;
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            if terrain.has_height_map() {
                if !skip_flatten {
                    if geom.authored
                        && matches!(geom.geom_type, crate::game_logic::HostGeometryType::Box)
                    {
                        terrain.flatten_terrain_box_at(
                            pos.x,
                            pos.z,
                            angle,
                            geom.major_radius,
                            geom.minor_radius,
                        );
                    } else if geom.authored {
                        terrain.flatten_terrain_at(pos.x, pos.z, geom.major_radius.max(1.0));
                    } else {
                        terrain.flatten_terrain_at(pos.x, pos.z, radius);
                    }
                }
                let z = terrain.get_ground_height(pos.x, pos.z, None);
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_position(glam::Vec3::new(pos.x, z, pos.z));
                }
                self.sweep_under_construction_footprint_mines(id);
                return;
            }
        }
        if let Some(h) = self.terrain_height_at(pos) {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_position(glam::Vec3::new(pos.x, h, pos.z));
            }
        }
        self.sweep_under_construction_footprint_mines(id);
    }

    /// C++ AIUpdateInterface::isQuickPathAvailable residual (simplified host pathfind).
    ///
    /// Fail-open when builder missing / immobile / already at goal. Fail-closed
    /// when pathfinding returns no path for a mobile constructor.
    pub fn builder_has_clear_path_to(&self, builder_id: ObjectId, goal: glam::Vec3) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::builder_skips_clear_path_residual;
        let Some(builder) = self.objects.get(&builder_id) else {
            return builder_skips_clear_path_residual(true);
        };
        if !builder.is_alive() {
            return false;
        }
        // Structures / immobile skip CLEAR_PATH residual.
        if builder.is_kind_of(KindOf::Structure) || builder.is_kind_of(KindOf::Immobile) {
            return builder_skips_clear_path_residual(true);
        }
        if !builder.can_move() && !builder.can_construct() {
            return builder_skips_clear_path_residual(true);
        }
        let start = builder.get_position();
        // C++ AIPlayer.cpp:595-602 clientSafeQuickDoesPathExist (structure-aware).
        // No dist<=64 early-true: a walled-off dozer 50 units from the pad is stuck.
        self.pathfinding_system
            .client_safe_quick_does_path_exist(start, goal)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExistForUI residual.
    pub(in super::super) fn quick_path_available_residual(
        &self,
        start: glam::Vec3,
        goal: glam::Vec3,
    ) -> bool {
        self.pathfinding_system
            .grid
            .quick_path_exists_for_ui(start, goal)
    }

    /// C++ BuildAssistant footprint hiZ-loZ residual vs AllowedHeightVariationForBuilding.
    pub(in super::super) fn footprint_not_flat_enough(
        &self,
        position: glam::Vec3,
        place_radius: f32,
    ) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::footprint_height_delta_residual;
        use crate::game_logic::host_structure_economy_residual::is_legal_build_height_variation;
        let r = place_radius.max(1.0);
        // 3x3 sample residual across pad (simplified vs full iterateFootprint resolution).
        let offsets = [
            (-r, -r),
            (0.0, -r),
            (r, -r),
            (-r, 0.0),
            (0.0, 0.0),
            (r, 0.0),
            (-r, r),
            (0.0, r),
            (r, r),
        ];
        let mut samples = Vec::with_capacity(9);
        for (dx, dz) in offsets {
            let p = glam::Vec3::new(position.x + dx, 0.0, position.z + dz);
            if let Some(h) = self.terrain_height_at(p) {
                samples.push(h);
            }
        }
        if samples.is_empty() {
            return false; // fail-open residual
        }
        let delta = footprint_height_delta_residual(&samples);
        !is_legal_build_height_variation(delta)
    }

    /// C++ PartitionManager::getShroudStatusForPlayer == CELLSHROUD_CLEAR residual.
    ///
    /// Fail-open when shroud grid is not initialized (synthetic/host tests).
    pub fn is_build_location_shroud_clear(&self, player_id: u32, position: glam::Vec3) -> bool {
        use gamelogic::common::Coord3D;
        use gamelogic::system::shroud_manager::{ShroudState, get_shroud_manager};
        let shroud_manager = get_shroud_manager();
        let Ok(shroud) = shroud_manager.lock() else {
            return true;
        };
        if !shroud.has_shroud_grid() {
            return true;
        }
        // Match host vision residual Coord3D axis order (x, z, y).
        let coord = Coord3D::new(position.x, position.z, position.y);
        matches!(
            shroud.get_shroud_state(player_id, &coord),
            ShroudState::Visible
        )
    }

    /// True when residual LegalBuildCode is LBC_OK.
    pub fn is_location_legal_to_build(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
    ) -> bool {
        self.is_location_legal_to_build_for_builder(team, position, template_name, None)
    }

    pub fn is_location_legal_to_build_for_builder(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::LBC_OK;
        self.legal_build_code_at_for_builder(team, position, template_name, builder_id) == LBC_OK
    }

    /// Count living/under-construction Superweapon-link-key objects for a team residual.
    pub fn count_superweapon_link_key_owned(&self, team: Team) -> u32 {
        self.objects
            .values()
            .filter(|o| {
                o.team == team
                    && o.is_alive()
                    && o.thing.template.has_superweapon_restriction_link_key()
            })
            .count() as u32
    }

    /// Player-scoped counterpart of the Superweapon restriction. Faction is
    /// still used to identify the template, but each skirmish slot has its own
    /// quota and must not consume a same-faction opponent's allowance.
    pub fn count_superweapon_link_key_owned_by_player(&self, player_id: u32) -> u32 {
        self.objects
            .values()
            .filter(|obj| {
                obj.owner_player_id == Some(player_id)
                    && obj.is_alive()
                    && obj.thing.template.has_superweapon_restriction_link_key()
            })
            .count() as u32
    }

    /// Resolved C++ `MaxSimultaneousLinkKey` for a template: the authored INI
    /// key, else the static retail identity residual — PUC / Nuke / Scud all
    /// author `MaxSimultaneousLinkKey = Superweapon` in retail Object INI, so
    /// templates the live catalog carries without the row still group under
    /// the one link key (C++ Player.cpp:2835 counts by that key).
    fn superweapon_link_key_for_template(&self, template_name: &str) -> Option<String> {
        if let Some(key) = self
            .templates
            .get(template_name)
            .and_then(|template| template.max_simultaneous_link_key.clone())
        {
            return Some(key);
        }
        crate::game_logic::host_superweapon_kindof::is_superweapon_link_key_template(template_name)
            .then(|| {
                crate::game_logic::host_superweapon_kindof::SUPERWEAPON_LINK_KEY.to_string()
            })
    }

    fn count_superweapon_link_key_owned_for_template(
        &self,
        team: Team,
        requested_template: &str,
    ) -> u32 {
        let Some(link_key) = self.superweapon_link_key_for_template(requested_template) else {
            return 0;
        };
        self.objects
            .values()
            .filter(|obj| {
                obj.team == team
                    && obj.is_alive()
                    && self
                        .superweapon_link_key_for_template(&obj.template_name)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&link_key))
            })
            .count() as u32
    }

    fn count_superweapon_link_key_owned_by_player_for_template(
        &self,
        player_id: u32,
        requested_template: &str,
    ) -> u32 {
        let Some(link_key) = self.superweapon_link_key_for_template(requested_template) else {
            return 0;
        };
        self.objects
            .values()
            .filter(|obj| {
                obj.owner_player_id == Some(player_id)
                    && obj.is_alive()
                    && self
                        .superweapon_link_key_for_template(&obj.template_name)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&link_key))
            })
            .count() as u32
    }

    /// Living constructed template names owned by a team residual (prereq scan).
    pub fn team_owned_constructed_templates(&self, team: Team) -> Vec<String> {
        let mut names = Vec::new();
        for obj in self.objects.values() {
            if obj.team == team && obj.is_alive() && obj.is_constructed() {
                names.push(obj.template_name.clone());
            }
        }
        names
    }

    /// Constructed template scan for a controlling player rather than a
    /// faction. This is the prerequisite source used by player-issued builds
    /// and production queues.
    pub fn player_owned_constructed_templates(&self, player_id: u32) -> Vec<String> {
        self.objects
            .values()
            .filter(|obj| {
                obj.owner_player_id == Some(player_id) && obj.is_alive() && obj.is_constructed()
            })
            .map(|obj| obj.template_name.clone())
            .collect()
    }

    /// C++ `Player::canBuild` walks leftover `getNthPrereq(i)->isSatisfied`
    /// for every template. Empty leftover/live prereq lists are satisfied.
    pub fn team_satisfies_build_prerequisites(&self, team: Team, template_name: &str) -> bool {
        let player_id = self.get_player_by_team(team).map(|player| player.id);
        self.leftover_prereqs_satisfied(template_name, player_id, Some(team))
    }

    /// C++ player-owned prerequisite scan for a known controlling player.
    pub fn player_satisfies_build_prerequisites(
        &self,
        player_id: u32,
        template_name: &str,
    ) -> bool {
        self.leftover_prereqs_satisfied(template_name, Some(player_id), None)
    }

    fn leftover_prereqs_satisfied(
        &self,
        template_name: &str,
        player_id: Option<u32>,
        team: Option<Team>,
    ) -> bool {
        let prereqs =
            leftover_or_live_production_prereqs(template_name, self.templates.get(template_name));
        for prereq in &prereqs {
            if !prereq.is_satisfied_with_counter(
                |science| self.host_player_has_prereq_science(player_id, team, science),
                |handles, ignore_dead, counts| {
                    self.count_prereq_objects(
                        player_id,
                        team,
                        handles,
                        ignore_dead,
                        counts,
                        prereq,
                    );
                },
            ) {
                return false;
            }
        }
        true
    }

    fn host_player_has_prereq_science(
        &self,
        player_id: Option<u32>,
        team: Option<Team>,
        science: game_engine::common::rts::ScienceType,
    ) -> bool {
        use game_engine::common::rts::SCIENCE_INVALID;
        if science == SCIENCE_INVALID {
            return true;
        }
        let player = match player_id {
            Some(id) => self.get_player(id),
            None => team.and_then(|team| self.get_player_by_team(team)),
        };
        if let Some(player) = player {
            if leftover_player_has_science(player.id, &player.name, science) == Some(true) {
                return true;
            }
            return live_unlocked_has_science(&player.unlocked_sciences, science);
        }
        if let Some(id) = player_id {
            return leftover_player_has_science(id, "", science) == Some(true);
        }
        false
    }

    fn count_prereq_objects(
        &self,
        player_id: Option<u32>,
        team: Option<Team>,
        handles: &[game_engine::common::rts::ThingTemplateHandle],
        ignore_dead: bool,
        counts: &mut [i32],
        prereq: &game_engine::common::rts::ProductionPrerequisite,
    ) {
        let units = prereq.get_unit_prereqs();
        for (i, handle) in handles.iter().enumerate() {
            if i >= counts.len() {
                break;
            }
            let required = leftover_template_name_for_handle(*handle)
                .or_else(|| {
                    units
                        .get(i)
                        .and_then(|rec| (!rec.name.is_empty()).then(|| rec.name.clone()))
                })
                .unwrap_or_default();
            if required.is_empty() {
                counts[i] = 0;
                continue;
            }
            counts[i] = self
                .objects
                .values()
                .filter(|obj| {
                    let owned = match player_id {
                        Some(id) => obj.owner_player_id == Some(id),
                        None => team.is_some_and(|team| obj.team == team),
                    };
                    owned
                        && obj.is_constructed()
                        && (!ignore_dead || obj.is_alive())
                        && object_matches_prereq_template(obj, &required)
                })
                .count() as i32;
        }
    }

    /// C++ MaxSimultaneousOfType Superweapon residual gate.
    pub fn can_start_superweapon_building(&self, team: Team, template_name: &str) -> bool {
        use crate::game_logic::host_superweapon_kindof::superweapon_max_simultaneous_allowed;
        let Some(template) = self.templates.get(template_name) else {
            return true;
        };
        // Retail PUC/Nuke/Scud Object INI author
        // MaxSimultaneousOfType = DeterminedBySuperweaponRestriction with
        // MaxSimultaneousLinkKey "Superweapon"; the static retail identity
        // residual covers templates the live catalog authored without it.
        if !template.has_superweapon_restriction_link_key()
            && !crate::game_logic::host_superweapon_kindof::is_superweapon_link_key_template(
                template_name,
            )
        {
            return true;
        }
        let Some(max) =
            superweapon_max_simultaneous_allowed(self.skirmish_rules.limit_superweapons)
        else {
            return true;
        };
        self.count_superweapon_link_key_owned_for_template(team, template_name) < max
    }

    /// Player-scoped superweapon restriction used where a command has exact
    /// controlling-player provenance.
    pub fn can_start_superweapon_building_for_player(
        &self,
        player_id: u32,
        template_name: &str,
    ) -> bool {
        use crate::game_logic::host_superweapon_kindof::superweapon_max_simultaneous_allowed;
        let Some(template) = self.templates.get(template_name) else {
            return true;
        };
        // Retail PUC/Nuke/Scud Object INI author
        // MaxSimultaneousOfType = DeterminedBySuperweaponRestriction with
        // MaxSimultaneousLinkKey "Superweapon"; the static retail identity
        // residual covers templates the live catalog authored without it.
        if !template.has_superweapon_restriction_link_key()
            && !crate::game_logic::host_superweapon_kindof::is_superweapon_link_key_template(
                template_name,
            )
        {
            return true;
        }
        let Some(max) =
            superweapon_max_simultaneous_allowed(self.skirmish_rules.limit_superweapons)
        else {
            return true;
        };
        self.count_superweapon_link_key_owned_by_player_for_template(player_id, template_name) < max
    }

    /// Enqueue unit production on a building if permitted.

    /// Living units of template for a team + queued production of that template residual.
    pub fn count_team_units_of_template_owned_or_queued(
        &self,
        team: Team,
        template_name: &str,
    ) -> u32 {
        let mut n = 0u32;
        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if obj.template_name.eq_ignore_ascii_case(template_name) {
                n = n.saturating_add(1);
            }
            // Queued production residual.
            if let Some(b) = obj.building_data.as_ref() {
                for item in &b.production_queue {
                    if item.template_name.eq_ignore_ascii_case(template_name) {
                        n = n.saturating_add(1);
                    }
                }
            }
        }
        n
    }

    /// Player-scoped MaxSimultaneous count, including that player's queues.
    pub fn count_player_units_of_template_owned_or_queued(
        &self,
        player_id: u32,
        template_name: &str,
    ) -> u32 {
        let mut count = 0u32;
        for obj in self.objects.values() {
            if obj.owner_player_id != Some(player_id) || !obj.is_alive() {
                continue;
            }
            if obj.template_name.eq_ignore_ascii_case(template_name) {
                count = count.saturating_add(1);
            }
            if let Some(building) = obj.building_data.as_ref() {
                for item in &building.production_queue {
                    if item.template_name.eq_ignore_ascii_case(template_name) {
                        count = count.saturating_add(1);
                    }
                }
            }
        }
        count
    }

    /// C++ `ThingTemplate::getMaxSimultaneousOfType` with live SW restriction.
    pub fn template_max_simultaneous_of_type(&self, template: &ThingTemplate) -> u32 {
        let restriction = if self.skirmish_rules.limit_superweapons {
            crate::game_logic::host_superweapon_kindof::SUPERWEAPON_MAX_SIMULTANEOUS_WHEN_LIMITED
        } else {
            0
        };
        template.get_max_simultaneous_of_type(restriction)
    }

    fn object_counts_toward_max_simultaneous(
        &self,
        obj: &crate::game_logic::Object,
        wanted: &ThingTemplate,
        wanted_name: &str,
    ) -> bool {
        if obj.template_name.eq_ignore_ascii_case(wanted_name) {
            return true;
        }
        let candidate = self
            .templates
            .get(&obj.template_name)
            .unwrap_or(&obj.thing.template);
        candidate.counts_toward_max_simultaneous_of(wanted)
    }

    /// C++ `countExisting` living match: equivalent name or shared link key.
    /// Factory queues count only for non-`KINDOF_STRUCTURE` (Player.cpp:2865-2872).
    /// `countUnitTypeInQueue` is exact production-template identity.
    pub fn count_player_units_matching_max_simultaneous(
        &self,
        player_id: u32,
        template_name: &str,
    ) -> u32 {
        let Some(wanted) = self.templates.get(template_name) else {
            return self.count_player_units_of_template_owned_or_queued(player_id, template_name);
        };
        let check_queue = !wanted.is_kind_of(KindOf::Structure);
        let mut count = 0u32;
        for obj in self.objects.values() {
            if obj.owner_player_id != Some(player_id) || !obj.is_alive() {
                continue;
            }
            if self.object_counts_toward_max_simultaneous(obj, wanted, template_name) {
                count = count.saturating_add(1);
            }
            if check_queue {
                if let Some(building) = obj.building_data.as_ref() {
                    for item in &building.production_queue {
                        if item.template_name.eq_ignore_ascii_case(template_name) {
                            count = count.saturating_add(1);
                        }
                    }
                }
            }
        }
        count
    }

    pub fn count_team_units_matching_max_simultaneous(
        &self,
        team: Team,
        template_name: &str,
    ) -> u32 {
        let Some(wanted) = self.templates.get(template_name) else {
            return self.count_team_units_of_template_owned_or_queued(team, template_name);
        };
        let check_queue = !wanted.is_kind_of(KindOf::Structure);
        let mut count = 0u32;
        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if self.object_counts_toward_max_simultaneous(obj, wanted, template_name) {
                count = count.saturating_add(1);
            }
            if check_queue {
                if let Some(building) = obj.building_data.as_ref() {
                    for item in &building.production_queue {
                        if item.template_name.eq_ignore_ascii_case(template_name) {
                            count = count.saturating_add(1);
                        }
                    }
                }
            }
        }
        count
    }

    /// C++ `Player::canBuildMoreOfType` (Player.cpp:2853-2876).
    pub fn can_build_more_of_type(
        &self,
        player_id: Option<u32>,
        team: Team,
        template_name: &str,
    ) -> bool {
        let Some(template) = self.templates.get(template_name) else {
            return true;
        };
        let max = self.template_max_simultaneous_of_type(template);
        if max == 0 {
            return true;
        }
        let count = match player_id {
            Some(id) => self.count_player_units_matching_max_simultaneous(id, template_name),
            None => self.count_team_units_matching_max_simultaneous(team, template_name),
        };
        count < max
    }

    /// C++ `ParkingPlaceBehavior` occupancy plus queued non-helipad aircraft.
    ///
    /// `producer_id` plus `airfield_parking_space_index` is the persisted
    /// `m_spaces` reservation.  A generic building garrison is not parking
    /// state and must never consume an airfield slot here.
    pub fn airfield_parking_occupied_or_queued(&self, airfield_id: ObjectId) -> u32 {
        let Some(capacity) = self.airfield_parking_capacity(airfield_id) else {
            // An FSAirfield without parsed ParkingPlaceBehavior data cannot
            // safely answer C++ `hasAvailableSpaceFor`; callers fail closed.
            return u32::MAX;
        };
        let Some(airfield) = self.objects.get(&airfield_id) else {
            return u32::MAX;
        };

        // Multiple stale objects must not turn one retained m_spaces index
        // into multiple occupied slots.  Runtime normalization resolves such
        // records too; this `&self` UI query stays deterministic and
        // conservative between ticks.
        let mut occupied_slots = HashSet::new();
        for obj in self.objects.values() {
            if obj.is_alive()
                && (obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft)
                && obj.producer_id == Some(airfield_id)
            {
                if let Some(slot) = obj.airfield_parking_space_index {
                    if usize::try_from(slot)
                        .ok()
                        .is_some_and(|slot| slot < capacity)
                    {
                        occupied_slots.insert(slot);
                    }
                }
            }
        }

        let mut queued = 0u32;
        if let Some(building) = airfield.building_data.as_ref() {
            for item in &building.production_queue {
                let Some(template) = self.templates.get(&item.template_name) else {
                    // C++ needs the actual ThingTemplate to decide whether
                    // `shouldReserveDoorWhenQueued` applies.  Do not guess
                    // from a name and accidentally overbook a parking slot.
                    return u32::MAX;
                };
                if template.is_kind_of(KindOf::Aircraft)
                    && !Self::template_is_produced_at_helipad(template)
                {
                    queued = queued.saturating_add(1);
                }
            }
        }
        (occupied_slots.len() as u32).saturating_add(queued)
    }

    /// C++ `ThingTemplate::getBuildable` — leftover GameLogic override first.
    fn effective_buildable_status(&self, template_name: &str, ini_status: u32) -> u32 {
        if let Some(status) =
            gamelogic::helpers::TheGameLogic::find_buildable_status_override(template_name)
        {
            return status.max(0) as u32;
        }
        ini_status
    }

    /// C++ `Player::canBuild` allowedToBuild + BuildableStatus (not prereqs).
    /// Returns `(player_may_build, ignore_prerequisites)`.
    fn host_player_can_build(
        &self,
        owner_player_id: Option<u32>,
        team: crate::game_logic::Team,
        template_name: &str,
        is_structure: bool,
        ini_buildable: u32,
    ) -> (bool, bool) {
        use crate::game_logic::host_production_buildable_command_residual::{
            BSTATUS_IGNORE_PREREQUISITES, BSTATUS_NO, BSTATUS_ONLY_BY_AI,
        };
        let status = self.effective_buildable_status(template_name, ini_buildable);
        let ignore_prereq = status == BSTATUS_IGNORE_PREREQUISITES;
        let owner = match owner_player_id {
            Some(player_id) => self.get_player(player_id),
            None => self.get_player_by_team(team),
        };
        let Some(player) = owner else {
            return (status != BSTATUS_NO, ignore_prereq);
        };
        if !player.allowed_to_build(is_structure) {
            return (false, ignore_prereq);
        }
        if status == BSTATUS_NO {
            return (false, ignore_prereq);
        }
        let is_computer = self.ai_manager.ai_players.contains_key(&player.id);
        if status == BSTATUS_ONLY_BY_AI && !is_computer {
            return (false, ignore_prereq);
        }
        (true, ignore_prereq)
    }

    /// Parking uses the parsed `ParkingPlaceBehavior` reservation contract.
    /// MaxSimultaneous uses the authored Object INI cap (or SW restriction),
    /// counted by equivalent template / shared link key.
    pub fn can_make_unit(&self, producer_id: ObjectId, template_name: &str) -> u32 {
        use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;
        use crate::game_logic::host_production_buildable_command_residual::{
            CANMAKE_OK, can_make_type_from_checks_residual,
        };

        let Some(template) = self.templates.get(template_name) else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        };
        let Some(producer) = self.objects.get(&producer_id) else {
            // C++ BuildAssistant::canMakeUnit: NULL builder → CANMAKE_NO_PREREQ.
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        };
        if !producer.is_alive() {
            // Stale/dead producer is the live equivalent of a NULL builder.
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        }
        let team = producer.team;
        let owner_player_id = producer.owner_player_id;
        let factory_disabled = producer.is_disabled();
        let Some(building) = producer.building_data.as_ref() else {
            // C++ dozer/worker GUI_COMMAND_DOZER_CONSTRUCT still runs
            // Player::canBuild + BuildAssistant::canMakeUnit (prereq / money /
            // maxed). There is no ProductionUpdate queue or parking place.
            if producer.is_kind_of(KindOf::Dozer) || producer.is_kind_of(KindOf::Worker) {
                return self.can_make_dozer_construct(producer_id, template_name);
            }
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        };
        // C++ BuildAssistant::isPossibleToMakeUnit: CommandSet UNIT_BUILD scan.
        // Prefer the exact live catalog that also drives GameClient's
        // ControlBar: Object CommandSet -> typed UNIT_BUILD -> Object target.
        // The small residual table remains only while that catalog has not
        // supplied a producer identity (for example test-only factories).
        let producer_template = producer.template_name.as_str();
        let command_set_ok = match gamelogic::control_bar::parsed_unit_build_authorization(
            producer_template,
            template_name,
        ) {
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Rejected => false,
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Authorized => true,
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Unavailable => {
                match crate::game_logic::host_production_buildable_command_residual::command_set_allows_unit(
                    producer_template,
                    template_name,
                ) {
                    Some(false) => false,
                    Some(true) => true,
                    None => building.can_produce(template),
                }
            }
        };
        let queue_full = building.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT;
        // C++ ParkingPlaceBehavior `hasAvailableSpaceFor` gate for a real
        // FSAirfield.  No building-type or template-name approximation is
        // allowed: absent authored ParkingPlace data fails closed.
        let parking_full = {
            let is_aircraft = template.is_kind_of(KindOf::Aircraft);
            if producer.is_kind_of(KindOf::FSAirfield)
                && is_aircraft
                && !Self::template_is_produced_at_helipad(template)
            {
                self.airfield_parking_capacity(producer_id)
                    .map_or(true, |capacity| {
                        let capacity = u32::try_from(capacity).unwrap_or(u32::MAX);
                        self.airfield_parking_occupied_or_queued(producer_id) >= capacity
                    })
            } else {
                false
            }
        };
        let has_prereq = match owner_player_id {
            Some(player_id) => {
                self.player_satisfies_build_prerequisites(player_id, template_name)
                    && self.can_start_superweapon_building_for_player(player_id, template_name)
            }
            None => {
                self.team_satisfies_build_prerequisites(team, template_name)
                    && self.can_start_superweapon_building(team, template_name)
            }
        };
        // Science residual (stealth fighter etc.) as prereq gate.
        let science_ok = {
            use crate::game_logic::host_stealth_fighter::{
                is_stealth_fighter_science, player_may_produce_stealth_aircraft,
                requires_stealth_fighter_science,
            };
            if requires_stealth_fighter_science(template_name) {
                let owner = match owner_player_id {
                    Some(player_id) => self.get_player(player_id),
                    None => self.get_player_by_team(team),
                };
                match owner {
                    Some(p) => {
                        let has = p
                            .unlocked_sciences
                            .iter()
                            .any(|s| is_stealth_fighter_science(s));
                        player_may_produce_stealth_aircraft(has, template_name)
                    }
                    None => false,
                }
            } else {
                true
            }
        };
        let has_prereq = has_prereq && science_ok;
        let (player_can_build, ignore_prereq) = self.host_player_can_build(
            owner_player_id,
            team,
            template_name,
            template.is_kind_of(KindOf::Structure),
            template.buildable_status,
        );
        let has_prereq = player_can_build && command_set_ok && (ignore_prereq || has_prereq);
        let owner = match owner_player_id {
            Some(player_id) => self.get_player(player_id),
            None => self.get_player_by_team(team),
        };
        let has_money = match owner {
            Some(p) => {
                let cost = self.modified_build_cost_supplies(
                    p.id,
                    template_name,
                    template.build_cost.supplies,
                );
                p.resources.supplies >= cost
            }
            None => false,
        };
        let _ = CANMAKE_OK;
        // C++ Player::canBuildMoreOfType — INI MaxSimultaneousOfType, not a
        // hero-name residual table.
        let maxed_out = !self.can_build_more_of_type(owner_player_id, team, template_name);
        // C++ `ProductionUpdate::canQueueCreateUnit` asks the parking-place
        // interface before it checks `m_productionCount`.  Preserve that
        // player-visible failure reason when a full authored airfield also
        // has a full generic queue.
        let queue_full_after_parking_gate = queue_full && !parking_full;
        can_make_type_from_checks_residual(
            has_prereq,
            has_money,
            factory_disabled,
            queue_full_after_parking_gate,
            parking_full,
            maxed_out,
        )
    }

    /// True when CanMake residual is CANMAKE_OK.
    pub fn can_make_unit_ok(&self, producer_id: ObjectId, template_name: &str) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK;
        self.can_make_unit(producer_id, template_name) == CANMAKE_OK
    }
    /// C++ ControlBarCommand.cpp:1112-1150 GUI_COMMAND_DOZER_CONSTRUCT.
    /// Prereq / money / maxed plus `isTaskPending(DOZER_TASK_BUILD)` busy gate.
    /// No factory queue or parking place.
    fn can_make_dozer_construct(&self, producer_id: ObjectId, template_name: &str) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::{
            CANMAKE_FACTORY_IS_DISABLED, CANMAKE_NO_PREREQ, can_make_type_from_checks_residual,
            command_set_allows_unit,
        };

        let Some(template) = self.templates.get(template_name) else {
            return CANMAKE_NO_PREREQ;
        };
        let Some(producer) = self.objects.get(&producer_id) else {
            return CANMAKE_NO_PREREQ;
        };
        if !producer.is_alive() {
            return CANMAKE_NO_PREREQ;
        }
        let command_set_ok = !matches!(
            command_set_allows_unit(&producer.template_name, template_name),
            Some(false)
        );
        let team = producer.team;
        let owner_player_id = producer.owner_player_id;
        // C++ ControlBarCommand.cpp:1139-1141 — pending BUILD greys every construct cameo.
        let factory_disabled = producer.is_disabled() || producer.dozer_task_build_target.is_some();
        let has_prereq = match owner_player_id {
            Some(player_id) => {
                self.player_satisfies_build_prerequisites(player_id, template_name)
                    && self.can_start_superweapon_building_for_player(player_id, template_name)
            }
            None => {
                self.team_satisfies_build_prerequisites(team, template_name)
                    && self.can_start_superweapon_building(team, template_name)
            }
        };
        let (player_can_build, ignore_prereq) = self.host_player_can_build(
            owner_player_id,
            team,
            template_name,
            template.is_kind_of(KindOf::Structure),
            template.buildable_status,
        );
        let has_prereq = player_can_build && command_set_ok && (ignore_prereq || has_prereq);
        let owner = match owner_player_id {
            Some(player_id) => self.get_player(player_id),
            None => self.get_player_by_team(team),
        };
        let has_money = match owner {
            Some(p) => {
                let cost = self.modified_build_cost_supplies(
                    p.id,
                    template_name,
                    template.build_cost.supplies,
                );
                p.resources.supplies >= cost
            }
            None => false,
        };
        let maxed_out = !self.can_build_more_of_type(owner_player_id, team, template_name);
        can_make_type_from_checks_residual(
            has_prereq,
            has_money,
            factory_disabled,
            false,
            false,
            maxed_out,
        )
    }

    /// C++ ProductionUpdate::queueUpgrade OBJECT gate: hasUpgrade or
    /// !affectedByUpgrade (drone ConflictsWith mux) cannot be queued.
    pub fn producer_refuses_completed_object_upgrade(
        &self,
        producer_id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name)
            && self
                .objects
                .get(&producer_id)
                .is_some_and(|obj| obj.refuses_object_upgrade(upgrade_name))
    }

    pub fn enqueue_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            CANMAKE_NO_MONEY, CANMAKE_OK,
        };
        use crate::game_logic::host_stealth_fighter::requires_stealth_fighter_science;

        let template = match self.templates.get(&template_name) {
            Some(t) => t.clone(),
            None => return false,
        };
        let science_gated = requires_stealth_fighter_science(&template_name);
        // C++ BuildAssistant::canMakeUnit residual gate (before charging).
        let can_make = self.can_make_unit(producer_id, &template_name);
        if can_make != CANMAKE_OK {
            if can_make == CANMAKE_NO_MONEY {
                if let Some(producer) = self.objects.get(&producer_id) {
                    let owner = match producer.owner_player_id {
                        Some(player_id) => self.get_player(player_id),
                        None => self.get_player_by_team(producer.team),
                    };
                    if let Some(p) = owner {
                        let pid = p.id;
                        self.try_eva_insufficient_funds(pid);
                    }
                }
            }
            if science_gated
                && can_make
                    == crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ
            {
                self.stealth_fighter_science.record_production_denied();
            }
            return false;
        }
        let science_ok = science_gated; // residual already validated via can_make

        // C++ ThingTemplate::calcCostToBuild / calcTimeToBuild are evaluated
        // for the actual controlling Player.  Resolve that identity before a
        // mutable player borrow so an exact General's name-key modifiers stay
        // coupled to the charged queue entry (including cancellation refunds).
        let (player_id, charged_cost, total_time) = {
            let Some(producer) = self.objects.get(&producer_id) else {
                return false;
            };
            let Some(player_id) = producer.owner_player_id.or_else(|| {
                self.get_player_by_team(producer.team)
                    .map(|player| player.id)
            }) else {
                return false;
            };
            let mut cost = template.build_cost;
            cost.supplies = self.modified_build_cost_supplies(
                player_id,
                &template_name,
                template.build_cost.supplies,
            );
            let total_time =
                self.modified_build_time_seconds(player_id, &template_name, template.build_time);
            (player_id, cost, total_time)
        };

        // C++ ProductionUpdate::queueCreateUnit reserveDoorForExit before charge.
        let reserved_exit_door =
            if self.should_reserve_airfield_door_when_queued(producer_id, &template) {
                match self.reserve_airfield_door_for_exit(producer_id) {
                    Some(door) => Some(door),
                    None => return false,
                }
            } else {
                None
            };

        let Some(player) = self.get_player_mut(player_id) else {
            if let Some(door) = reserved_exit_door {
                self.unreserve_airfield_door_for_exit(producer_id, door);
            }
            return false;
        };
        if !player.spend_resources(&charged_cost) {
            // Race residual: money spent between can_make and charge.
            if let Some(door) = reserved_exit_door {
                self.unreserve_airfield_door_for_exit(producer_id, door);
            }
            self.try_eva_insufficient_funds(player_id);
            return false;
        }

        let producer_template_name = self
            .objects
            .get(&producer_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();
        let quantity = crate::game_logic::host_production_buildable_command_residual::production_quantity_modifier(
            &producer_template_name,
            &template_name,
        );
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if building.add_to_queue_with_quantity_and_terms(
                    template_name.clone(),
                    &template,
                    quantity,
                    total_time,
                    charged_cost,
                ) {
                    if science_gated && science_ok {
                        self.stealth_fighter_science.record_production_enqueue();
                    }
                    crate::game_logic::host_production_log::record(
                        producer_id,
                        template_name.clone(),
                    );
                    return true;
                }
                if let Some(door) = reserved_exit_door {
                    self.unreserve_airfield_door_for_exit(producer_id, door);
                }
                return false;
            }
        }
        if let Some(door) = reserved_exit_door {
            self.unreserve_airfield_door_for_exit(producer_id, door);
        }
        false
    }

    /// Unlock a science for a team and record residual honesty hooks.
    ///
    /// Fail-closed: not full PrerequisiteSciences rank tree / control-bar UI.
    pub fn unlock_team_science(&mut self, team: Team, science_name: &str) -> bool {
        use crate::game_logic::host_stealth_fighter::is_stealth_fighter_science;
        use crate::game_logic::host_unit_training::is_unit_training_science;

        let player_id = {
            let Some(player) = self.get_player_mut_by_team(team) else {
                return false;
            };
            if !player.unlock_science(science_name) {
                return false;
            }
            player.id
        };
        if is_stealth_fighter_science(science_name) {
            self.stealth_fighter_science.record_science_unlock();
        }
        if is_unit_training_science(science_name) {
            self.unit_training.record_science_unlock();
        }
        self.on_special_power_science_creation(player_id, science_name);
        true
    }

    /// Record SCIENCE_StealthFighter unlock honesty (PurchaseScience residual path).
    pub fn record_stealth_fighter_science_unlock(&mut self) {
        self.stealth_fighter_science.record_science_unlock();
    }

    /// Host SCIENCE_StealthFighter residual honesty registry.
    pub fn stealth_fighter_science(
        &self,
    ) -> &crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry {
        &self.stealth_fighter_science
    }

    /// Residual honesty: SCIENCE_StealthFighter unlocked at least once.
    pub fn honesty_stealth_fighter_science_unlock_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_unlock_ok()
    }

    /// Residual honesty: science-gated Stealth Fighter accepted into production.
    pub fn honesty_stealth_fighter_science_produce_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_produce_ok()
    }

    /// Residual honesty: production denied for missing SCIENCE_StealthFighter.
    pub fn honesty_stealth_fighter_science_deny_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_deny_ok()
    }

    /// Residual honesty: science-gated Stealth Fighter finished production spawn.
    pub fn honesty_stealth_fighter_science_spawn_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_spawn_ok()
    }

    /// Combined residual honesty for SCIENCE_StealthFighter host path.
    pub fn honesty_stealth_fighter_science_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_ok()
    }
}

fn leftover_kindof_bits(template_name: &str) -> u128 {
    leftover_thing_template(template_name)
        .map(|tmpl| tmpl.get_kindof_bits())
        .unwrap_or(0)
}

fn leftover_has_ai_update(template_name: &str) -> Option<bool> {
    let tmpl = leftover_thing_template(template_name)?;
    Some(tmpl.get_behavior_module_info().iter().any(|entry| {
        let name = entry.name.as_str();
        name.eq_ignore_ascii_case("AIUpdateInterface")
            || name.eq_ignore_ascii_case("AIUpdate")
            || name.to_ascii_lowercase().ends_with("aiupdate")
    }))
}

fn leftover_player_is_human(player_id: u32, host_name: &str) -> Option<bool> {
    let list = gamelogic::player::ThePlayerList().read().ok()?;
    let named = format!("player{player_id}");
    let arc = list.find_player_by_name(&named).or_else(|| {
        if host_name.is_empty() {
            None
        } else {
            list.find_player_by_name(host_name)
        }
    })?;
    let guard = arc.read().ok()?;
    Some(guard.get_player_type() == gamelogic::player::PlayerType::Human)
}

fn leftover_thing_template(
    name: &str,
) -> Option<std::sync::Arc<game_engine::common::thing::thing_template::ThingTemplate>> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    factory.find_template(name, false)
}

fn leftover_or_live_production_prereqs(
    template_name: &str,
    live: Option<&ThingTemplate>,
) -> Vec<game_engine::common::rts::ProductionPrerequisite> {
    if let Some(tmpl) = leftover_thing_template(template_name) {
        return tmpl.get_prereqs().to_vec();
    }
    if let Some(template) = live {
        if !template.production_prerequisites.is_empty() {
            return template.production_prerequisites.clone();
        }
    }
    // C++ FactionBuilding.ini / Science-authored `Prerequisites` residual:
    // retail superweapon + tech-building chains carried by the static sample
    // table apply when neither the leftover factory nor the live template
    // authored Prerequisites (C++ Player::canBuild →
    // ProductionPrerequisite::isSatisfied).
    if let Some(row) = crate::game_logic::host_production_buildable_command_residual::prereq_row_for_template_residual(template_name)
    {
        let mut prereq = game_engine::common::rts::ProductionPrerequisite::new();
        for (index, required) in row.prereq_objects.iter().enumerate() {
            // or_chain: every entry after the first is OR-with-previous
            // (C++ ProductionPrerequisite UNIT_OR_WITH_PREV flag).
            prereq.add_unit_prereq((*required).to_string(), row.or_chain && index > 0);
        }
        return vec![prereq];
    }
    Vec::new()
}

fn leftover_template_name_for_handle(
    handle: game_engine::common::rts::ThingTemplateHandle,
) -> Option<String> {
    if !handle.is_valid() {
        return None;
    }
    let id = u16::try_from(handle.value()).ok()?;
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    Some(factory.find_by_template_id(id)?.get_name().to_string())
}

fn leftover_player_has_science(
    player_id: u32,
    host_name: &str,
    science: game_engine::common::rts::ScienceType,
) -> Option<bool> {
    let list = gamelogic::player::ThePlayerList().read().ok()?;
    let named = format!("player{player_id}");
    let arc = list.find_player_by_name(&named).or_else(|| {
        if host_name.is_empty() {
            None
        } else {
            list.find_player_by_name(host_name)
        }
    })?;
    let guard = arc.read().ok()?;
    Some(guard.has_science(science))
}

fn live_unlocked_has_science(
    unlocked: &std::collections::HashSet<String>,
    science: game_engine::common::rts::ScienceType,
) -> bool {
    let Some(store) = game_engine::common::rts::get_science_store() else {
        return false;
    };
    let name = store.get_internal_name_for_science(science);
    if name.is_empty() {
        return false;
    }
    let name = name.as_str();
    unlocked.iter().any(|owned| {
        owned.eq_ignore_ascii_case(name)
            || owned.eq_ignore_ascii_case(&format!("SCIENCE_{name}"))
            || name.eq_ignore_ascii_case(&format!("SCIENCE_{owned}"))
    })
}

fn object_matches_prereq_template(obj: &Object, required: &str) -> bool {
    if obj.template_name.eq_ignore_ascii_case(required) {
        return true;
    }
    let Some(owned) = leftover_thing_template(&obj.template_name) else {
        return false;
    };
    let Some(wanted) = leftover_thing_template(required) else {
        return false;
    };
    owned.is_equivalent_to(wanted.as_ref())
}

fn leftover_structure_place_footprint(name: &str) -> Option<(f32, f32, bool)> {
    let tmpl = leftover_thing_template(name)?;
    let g = tmpl.get_template_geometry_info();
    let major = g.major_radius();
    if major <= 0.5 {
        return None;
    }
    let is_box = matches!(
        g.geometry_type,
        game_engine::common::system::geometry::GeometryType::Box
    );
    let minor = if is_box {
        g.minor_radius().max(1.0)
    } else {
        major
    };
    Some((major.max(1.0), minor, is_box))
}

fn leftover_factory_exit_widths(name: &str) -> (f32, f32) {
    leftover_thing_template(name)
        .map(|tmpl| {
            (
                tmpl.get_factory_exit_width(),
                tmpl.get_factory_extra_bib_width(),
            )
        })
        .unwrap_or((0.0, 0.0))
}

fn leftover_placement_view_angle(name: &str) -> f32 {
    leftover_thing_template(name)
        .map(|tmpl| tmpl.get_placement_view_angle())
        .unwrap_or(0.0)
}

/// C++ BuildAssistant.cpp:786-794 factory exit rectangle as a circle residual.
fn leftover_factory_exit_blocker(
    template_name: &str,
    position: glam::Vec3,
    orientation: f32,
) -> Option<(f32, f32, f32)> {
    let (exit_w, _) = leftover_factory_exit_widths(template_name);
    if exit_w <= 0.0 {
        return None;
    }
    let major = leftover_structure_place_footprint(template_name)
        .map(|(major, _, _)| major)
        .unwrap_or(exit_w * 0.5);
    let offset = major + exit_w * 0.5;
    Some((
        position.x + orientation.cos() * offset,
        position.z + orientation.sin() * offset,
        (exit_w * 0.5).max(1.0),
    ))
}
