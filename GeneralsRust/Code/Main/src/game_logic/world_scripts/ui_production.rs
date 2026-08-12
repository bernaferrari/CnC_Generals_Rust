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
                let supply_centers = self
                    .objects
                    .values()
                    .filter(|obj| {
                        obj.team == p.team
                            && obj.is_constructed()
                            && obj.is_alive()
                            && obj.is_kind_of(KindOf::SupplyCenter)
                    })
                    .count();
                let income = 5.0 + supply_centers as f32 * 25.0;
                (
                    p.resources.supplies as i32,
                    produced,
                    consumed,
                    produced,
                    income,
                )
            } else {
                (10000, 100, 60, 100, 5.0)
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
        ui_state.new_beacons = std::mem::take(&mut self.recent_beacons);
        ui_state.minimap_viewport = crate::ui::default_minimap_viewport();
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

    pub fn cinematic_text(&self) -> Option<&str> {
        self.cinematic_text.as_ref().map(|(t, _)| t.as_str())
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

    pub fn set_cinematic_letterbox(&mut self, enabled: bool) {
        self.cinematic_letterbox = enabled;
    }

    pub fn set_cinematic_text(&mut self, text: Option<String>) {
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

    /// Structure placement radius residual for LBC_OBJECTS_IN_THE_WAY.
    pub(in super::super) fn structure_place_radius(obj: &Object) -> f32 {
        use crate::game_logic::host_production_buildable_command_residual::STRUCTURE_PLACE_CLEARANCE_RESIDUAL;
        // Prefer selection_radius when set; else default clearance residual.
        if obj.selection_radius > 1.0 {
            obj.selection_radius * 0.5
        } else {
            STRUCTURE_PLACE_CLEARANCE_RESIDUAL * 0.5
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
    pub fn legal_build_code_at_for_builder(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::{
            cell_shroud_blocks_build_residual, footprint_height_delta_residual,
            legal_build_code_from_checks_complete_residual,
            legal_build_objects_in_the_way_residual, legal_build_too_close_to_supplies_residual,
            min_dist_from_map_edge_residual, STRUCTURE_PLACE_CLEARANCE_RESIDUAL,
        };
        use crate::game_logic::host_structure_economy_residual::{
            is_legal_build_distance_from_map_edge, is_legal_build_height_variation,
            MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD, SUPPLY_BUILD_BORDER,
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
        let place_r = STRUCTURE_PLACE_CLEARANCE_RESIDUAL * 0.5;
        let mut blockers: Vec<(f32, f32, f32)> = Vec::new();
        let mut supply_sources: Vec<(f32, f32, f32)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let p = obj.get_position();
            let r = Self::structure_place_radius(obj);
            if obj.is_kind_of(KindOf::Structure) {
                blockers.push((p.x, p.z, r));
            }
            // C++ CANNOT_BUILD_NEAR_SUPPLIES asks only for
            // KINDOF_SUPPLY_SOURCE.  Resource/Harvestable are host bridge
            // kinds (and can describe crates), so a source-looking basename
            // or either bridge kind may not create this exclusion zone.
            if obj.is_kind_of(KindOf::SupplySource) {
                supply_sources.push((p.x, p.z, r.max(10.0)));
            }
        }
        let in_way =
            legal_build_objects_in_the_way_residual((position.x, position.z), place_r, &blockers);
        // C++ BuildAssistant checks the requested template's exact
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
        let dx = start.x - goal.x;
        let dz = start.z - goal.z;
        // Already close enough to pad residual — treat as clear.
        if dx * dx + dz * dz <= 64.0 * 64.0 {
            return true;
        }
        // Host pathfinding residual: need &mut pathfinding_system — use interior mutability
        // via a quick cell walk instead of full A* when possible.
        self.quick_path_available_residual(start, goal)
    }

    /// Simplified CLEAR_PATH residual without mutably borrowing pathfinding.
    ///
    /// Walks a coarse line of cells; blocked if any cell is impassable structure
    /// footprint residual. Fail-open when grid unavailable.
    pub(in super::super) fn quick_path_available_residual(
        &self,
        start: glam::Vec3,
        goal: glam::Vec3,
    ) -> bool {
        use crate::game_logic::pathfinding::GridPos;
        let grid = &self.pathfinding_system.grid;
        let gs = grid.world_to_grid(start);
        let gg = grid.world_to_grid(goal);
        // If either end invalid, fail-open residual (map placement still works).
        if !grid.is_valid_pos(gs) || !grid.is_valid_pos(gg) {
            return true;
        }
        // Goal on static structure residual is still a legal build pad — dozer
        // walks to the edge. Only intermediate cells block CLEAR_PATH residual.
        let steps = (gs.x - gg.x).abs().max((gs.y - gg.y).abs()).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = (gs.x as f32 + (gg.x - gs.x) as f32 * t).round() as i32;
            let y = (gs.y as f32 + (gg.y - gs.y) as f32 * t).round() as i32;
            let cell = GridPos::new(x, y);
            if !grid.is_valid_pos(cell) {
                continue;
            }
            // Skip start and goal cells residual.
            if cell == gs || cell == gg {
                continue;
            }
            if grid.is_static_blocked(cell) {
                return false;
            }
        }
        true
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
    pub(in super::super) fn is_build_location_shroud_clear(
        &self,
        player_id: u32,
        position: glam::Vec3,
    ) -> bool {
        use gamelogic::common::Coord3D;
        use gamelogic::system::shroud_manager::{get_shroud_manager, ShroudState};
        let Ok(shroud) = get_shroud_manager().lock() else {
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

    fn count_superweapon_link_key_owned_for_template(
        &self,
        team: Team,
        requested_template: &str,
    ) -> u32 {
        let Some(requested) = self.templates.get(requested_template) else {
            return 0;
        };
        let Some(link_key) = requested.max_simultaneous_link_key.as_deref() else {
            return 0;
        };
        self.objects
            .values()
            .filter(|obj| {
                obj.team == team
                    && obj.is_alive()
                    && obj.thing.template.has_superweapon_restriction_link_key()
                    && obj
                        .thing
                        .template
                        .max_simultaneous_link_key
                        .as_deref()
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(link_key))
            })
            .count() as u32
    }

    fn count_superweapon_link_key_owned_by_player_for_template(
        &self,
        player_id: u32,
        requested_template: &str,
    ) -> u32 {
        let Some(requested) = self.templates.get(requested_template) else {
            return 0;
        };
        let Some(link_key) = requested.max_simultaneous_link_key.as_deref() else {
            return 0;
        };
        self.objects
            .values()
            .filter(|obj| {
                obj.owner_player_id == Some(player_id)
                    && obj.is_alive()
                    && obj.thing.template.has_superweapon_restriction_link_key()
                    && obj
                        .thing
                        .template
                        .max_simultaneous_link_key
                        .as_deref()
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(link_key))
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

    /// C++ ProductionPrerequisite residual for known sample templates.
    ///
    /// Fail-closed: unknown templates (not in residual sample table) are allowed
    /// so map/script spawns and unported INI trees still work. Known SW / tech
    /// buildings require their Prerequisites Object list.
    pub fn team_satisfies_build_prerequisites(&self, team: Team, template_name: &str) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            prereq_is_satisfied_residual, prereq_objects_for_template_residual,
        };
        let Some((prereqs, or_chain)) = prereq_objects_for_template_residual(template_name) else {
            return true;
        };
        let owned = self.team_owned_constructed_templates(team);
        let owned_refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        // Science residual: fail-open for structure Object prereqs (no RequiredScience on SW).
        prereq_is_satisfied_residual(prereqs, or_chain, &owned_refs, true)
    }

    /// C++ player-owned prerequisite scan for a known controlling player.
    pub fn player_satisfies_build_prerequisites(
        &self,
        player_id: u32,
        template_name: &str,
    ) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            prereq_is_satisfied_residual, prereq_objects_for_template_residual,
        };
        let Some((prereqs, or_chain)) = prereq_objects_for_template_residual(template_name) else {
            return true;
        };
        let owned = self.player_owned_constructed_templates(player_id);
        let owned_refs: Vec<&str> = owned.iter().map(|name| name.as_str()).collect();
        prereq_is_satisfied_residual(prereqs, or_chain, &owned_refs, true)
    }

    /// C++ MaxSimultaneousOfType Superweapon residual gate.
    pub fn can_start_superweapon_building(&self, team: Team, template_name: &str) -> bool {
        use crate::game_logic::host_superweapon_kindof::superweapon_max_simultaneous_allowed;
        let Some(template) = self.templates.get(template_name) else {
            return true;
        };
        if !template.has_superweapon_restriction_link_key() {
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
        if !template.has_superweapon_restriction_link_key() {
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
                if template.is_kind_of(KindOf::Aircraft) {
                    // C++ exempts PRODUCED_AT_HELIPAD here, but Main's compact
                    // host KindOf bank does not retain that source bit.  Do
                    // not guess from an aircraft name and over-admit a queue:
                    // conservatively reserve a real parking space until that
                    // template-specific special case is represented.
                    queued = queued.saturating_add(1);
                }
            }
        }
        (occupied_slots.len() as u32).saturating_add(queued)
    }

    /// C++ BuildAssistant::canMakeUnit residual status for a producer + template.
    ///
    /// Parking uses the parsed `ParkingPlaceBehavior` reservation contract;
    /// the remaining MaxSimultaneous matrix is intentionally only the live
    /// hero/unique-unit residual until its complete INI representation lands.
    pub fn can_make_unit(&self, producer_id: ObjectId, template_name: &str) -> u32 {
        use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;
        use crate::game_logic::host_production_buildable_command_residual::{
            can_make_type_from_checks_residual, CANMAKE_OK,
        };

        let Some(template) = self.templates.get(template_name) else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        };
        let Some(producer) = self.objects.get(&producer_id) else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        };
        if !producer.is_alive() || !producer.is_constructed() {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        }
        let team = producer.team;
        let owner_player_id = producer.owner_player_id;
        let factory_disabled = producer.is_disabled();
        let Some(building) = producer.building_data.as_ref() else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        };
        // C++ BuildAssistant::isPossibleToMakeUnit: CommandSet UNIT_BUILD scan.
        // Prefer the exact live catalog that also drives GameClient's
        // ControlBar: Object CommandSet -> typed UNIT_BUILD -> Object target.
        // The small residual table remains only while that catalog has not
        // supplied a producer identity (for example test-only factories).
        let producer_template = producer.template_name.as_str();
        match gamelogic::control_bar::parsed_unit_build_authorization(
            producer_template,
            template_name,
        ) {
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Rejected => {
                return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
            }
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Authorized => {}
            gamelogic::control_bar::ParsedUnitBuildAuthorization::Unavailable => {
                match crate::game_logic::host_production_buildable_command_residual::command_set_allows_unit(
                    producer_template,
                    template_name,
                ) {
                    Some(false) => {
                        return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
                    }
                    Some(true) => {}
                    None if !building.can_produce(template) => {
                        return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
                    }
                    None => {}
                }
            }
        }
        let queue_full = building.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT;
        // C++ ParkingPlaceBehavior `hasAvailableSpaceFor` gate for a real
        // FSAirfield.  No building-type or template-name approximation is
        // allowed: absent authored ParkingPlace data fails closed.
        let parking_full = {
            let is_aircraft = template.is_kind_of(KindOf::Aircraft);
            if producer.is_kind_of(KindOf::FSAirfield) && is_aircraft {
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
        // C++ MaxSimultaneousOfType residual (heroes MaxSimultaneousOfType=1).
        let maxed_out = {
            use crate::game_logic::host_production_buildable_command_residual::{
                unit_max_simultaneous_of_type_residual, unit_maxed_out_for_player_residual,
            };
            let max = unit_max_simultaneous_of_type_residual(template_name);
            let owned = owner_player_id
                .map(|player_id| {
                    self.count_player_units_of_template_owned_or_queued(player_id, template_name)
                })
                .unwrap_or_else(|| {
                    self.count_team_units_of_template_owned_or_queued(team, template_name)
                });
            unit_maxed_out_for_player_residual(owned, max)
        };
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
        if let Some(producer) = self.objects.get(&producer_id) {
            let team = producer.team;
            let owner_player_id = producer.owner_player_id;
            let player = match owner_player_id {
                Some(player_id) => self.get_player_mut(player_id),
                None => self.get_player_mut_by_team(team),
            };
            let Some(player) = player else {
                return false;
            };
            let player_id = player.id;
            let base = template.build_cost.supplies;
            let mod_supplies = {
                let factor = player.production_cost_factor(
                    &crate::game_logic::host_upgrade_module_residuals::kindof_cost_tokens(
                        template.is_kind_of(crate::game_logic::KindOf::Vehicle),
                        template.is_kind_of(crate::game_logic::KindOf::Infantry),
                        template.is_kind_of(crate::game_logic::KindOf::Aircraft),
                        template.is_kind_of(crate::game_logic::KindOf::Structure),
                    ),
                );
                crate::game_logic::host_upgrade_module_residuals::apply_production_cost_factor(
                    base, factor,
                )
            };
            let mut cost = template.build_cost.clone();
            cost.supplies = mod_supplies;
            if !player.spend_resources(&cost) {
                // Race residual: money spent between can_make and charge.
                self.try_eva_insufficient_funds(player_id);
                return false;
            }
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
                if building.add_to_queue_with_quantity(template_name.clone(), &template, quantity) {
                    if science_gated && science_ok {
                        self.stealth_fighter_science.record_production_enqueue();
                    }
                    crate::game_logic::host_production_log::record(
                        producer_id,
                        template_name.clone(),
                    );
                    return true;
                }
                return false;
            }
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
