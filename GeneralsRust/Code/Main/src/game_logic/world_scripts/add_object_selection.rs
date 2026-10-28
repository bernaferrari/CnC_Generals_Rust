//! Host scripts `impl GameLogic` — `add_object_selection`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! add_object / selection / visibility / AI / initialize_scripts
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Add object to the game world
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        self.add_object_with_effects(object, &mut Default::default())
    }

    pub(crate) fn add_object_with_effects(
        &mut self,
        object: Object,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> ObjectId {
        self.try_add_object_with_effects(object, effects)
            .expect("Object admission requires an unoccupied, fully retired ID")
    }

    /// Checked public ingress for external tools that may supply an occupied ID.
    pub fn try_add_object(&mut self, object: Object) -> gamelogic::GameLogicResult<ObjectId> {
        self.try_add_object_with_effects(object, &mut Default::default())
    }

    pub(crate) fn try_add_object_with_effects(
        &mut self,
        object: Object,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<ObjectId> {
        self.try_add_object_with_team_interpreter(object, None, effects)
    }

    pub(crate) fn try_add_object_with_team_interpreter(
        &mut self,
        mut object: Object,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<ObjectId> {
        if object.team_instance_name.trim().is_empty() && object.owner_player_id.is_some() {
            object.team_instance_name =
                self.default_host_team_instance_name(object.owner_player_id, object.team);
        }
        self.validate_host_object_admission(object.id)?;
        let prepared = self.prepare_external_host_object_team(&object, interpreter, effects)?;
        let id = self.admit_external_object_to_prepared_team(object, prepared)?;
        self.partition_manager.attach_receiving_object(
            self.objects
                .get_mut(&id)
                .expect("external Object admitted before partition attachment"),
        );
        self.host_radar_add_object_with_effects(id, effects);
        Ok(id)
    }

    /// Compatibility ingress: duplicate IDs are a caller error, never an
    /// implicit replacement or acknowledgement of native retirement.
    pub(crate) fn admit_host_object(&mut self, object: Object) {
        self.try_admit_host_object(object)
            .expect("Object admission requires an unoccupied, fully retired ID");
    }

    pub(crate) fn validate_host_object_admission(
        &self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if id.0 == 0
            || self.objects.contains_key(&id)
            || self.objects_to_destroy.iter().any(|event| event.id == id)
        {
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "Object admission ID {} is invalid, occupied, or awaiting physical retirement",
                id.0
            )));
        }
        Ok(())
    }

    /// Admit a distinct lifetime only after the old identity has completed its
    /// ordinary callbacks and physical removal. Detached/query clone state is
    /// not a birth constructor; preserve its scalars, including score receipts.
    pub(crate) fn try_admit_host_object(
        &mut self,
        object: Object,
    ) -> gamelogic::GameLogicResult<()> {
        self.admit_host_object_storage(object)
    }

    /// Snapshot owns the subsequent whole-roster Energy reconstruction. It
    /// must not publish partial Player callbacks while saved tails are absent.
    pub(crate) fn try_admit_restored_host_object(
        &mut self,
        object: Object,
    ) -> gamelogic::GameLogicResult<()> {
        let id = object.id;
        self.admit_host_object_storage(object)?;
        self.partition_manager.attach_receiving_object(
            self.objects
                .get_mut(&id)
                .expect("restored Object admitted before partition attachment"),
        );
        Ok(())
    }

    pub(in crate::game_logic) fn admit_host_object_storage(
        &mut self,
        mut object: Object,
    ) -> gamelogic::GameLogicResult<()> {
        // Validate before touching old reservations, identities or callback state.
        self.validate_host_object_admission(object.id)?;
        // Observation copies retain scalar history, not a receipt authorizing
        // subtraction from another receiving grid. New admission starts clean.
        if object.partition_namespace.is_detached_clone() {
            object.forget_retired_partition_scalar_receipt();
        }
        // Every admission, including detached clones, receives this world's
        // immutable runtime rules. Preserve resolved module/snapshot scalars.
        object.bind_body_damage_policy(self.body_damage_policy());
        object.physics_rules = self.physics_definitions.effective;
        if let Some(catalog) = self.armor_definitions.as_ref() {
            object.template_mut().armor_definitions = Some(catalog.clone());
        }
        // Object-local C++ behavior state starts with the new admitted
        // lifetime. Retire the host mirror even when a replacement reuses ID.
        self.warehouse_crippling_states.remove(&object.id);
        // A cloned Object may still hold a weak handle to its source world's
        // trigger owner. Rebind it on admission without touching pose/flags;
        // load restoration and the C++ cell no-op depend on preserving those.
        object.attach_host_trigger_world(&self.host_trigger_world);
        object.visual_object_generation = self.allocate_visual_object_generation();
        object.visual_draw_identity_token = std::sync::Arc::new(());
        object.partition_namespace = Default::default();
        let idle_bits = object
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|definition| definition.receiving_ai_policy())
            .map(|policy| policy.0)
            .unwrap_or(object.thing().template.auto_acquire_enemies_when_idle);
        object.unit_ai_runtime =
            crate::game_logic::object::unit_ai_runtime::UnitAiRuntime::with_auto_acquire(
                idle_bits & gamelogic::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE != 0,
            );
        let id = object.id;
        self.objects.admit(id, object);
        self.attach_host_fog_ghost(id);
        Ok(())
    }

    /// Runtime visual identity belongs to an admission, including replacing a
    /// cloned object. Temporary extraction/reinsertion preserves its identity.
    pub(crate) fn allocate_visual_object_generation(&mut self) -> u64 {
        let generation = self.next_visual_object_generation.max(1);
        self.next_visual_object_generation = generation
            .checked_add(1)
            .expect("visual object generation exhausted");
        generation
    }

    pub(crate) fn host_visual_world_epoch(&self) -> u64 {
        self.visual_world_epoch
    }

    // ====== ENHANCED RTS COMMAND SYSTEM ======

    /// Get all objects visible to the receiving player (for rendering and UI).
    pub fn get_visible_objects(&self, player_id: u32) -> Vec<ObjectId> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_player(player_id);
        self.objects
            .registered()
            .filter_map(|(id, obj)| {
                if self.is_object_visible_for_player(*id, obj, player_id, shroud_snapshot.as_ref())
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get visual information for all visible objects
    pub fn get_visual_object_info(
        &self,
        player_id: u32,
    ) -> Vec<(ObjectId, super::super::ObjectVisualInfo)> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_player(player_id);
        self.objects
            .registered()
            .filter_map(|(id, obj)| {
                if self.is_object_visible_for_player(*id, obj, player_id, shroud_snapshot.as_ref())
                {
                    Some((*id, obj.get_visual_info()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Select objects within a rectangular area
    pub fn select_objects_in_area(
        &mut self,
        player_id: u32,
        min_pos: Vec3,
        max_pos: Vec3,
        add_to_selection: bool,
    ) -> Vec<ObjectId> {
        if !self.players.contains_key(&player_id) {
            return Vec::new();
        }
        let selected: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(&id, object)| {
                if object.owner_player_id != Some(player_id) || !object.is_selectable() {
                    return None;
                }
                let position = object.get_position();
                let closest_x = position.x.clamp(min_pos.x, max_pos.x);
                let closest_z = position.z.clamp(min_pos.z, max_pos.z);
                let distance_squared =
                    (position.x - closest_x).powi(2) + (position.z - closest_z).powi(2);
                (distance_squared <= object.selection_radius * object.selection_radius)
                    .then_some(id)
            })
            .collect();
        self.apply_player_drawable_selection(player_id, &selected, !add_to_selection);
        selected
    }

    /// Select a single object by click
    pub fn select_object_at_position(
        &mut self,
        player_id: u32,
        position: Vec3,
        selection_radius: f32,
        add_to_selection: bool,
    ) -> Option<ObjectId> {
        if let Some(player) = self.players.get(&player_id) {
            let team = player.team;
            // Pure residual acquire: nearest selectable friendly in click radius (3D).
            // Per-object selection_radius expands the pick disk (C++ pick residual).
            let candidates: Vec<_> = self
                .objects
                .registered()
                .filter_map(|(&id, obj)| {
                    if obj.owner_player_id != Some(player_id) || !obj.is_selectable() {
                        return None;
                    }
                    Some((
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: false,
                            under_construction: obj.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        },
                        obj.selection_radius,
                    ))
                })
                .collect();
            let closest_object =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    ObjectId(u32::MAX),
                    team,
                    position,
                    candidates.iter().map(|(c, _)| c.clone()),
                    |_| selection_radius + 256.0, // upper bound; legality enforces per-object radius
                    |c| {
                        let sel_r = candidates
                            .iter()
                            .find(|(cand, _)| cand.id == c.id)
                            .map(|(_, r)| *r)
                            .unwrap_or(0.0);
                        let dist = position.distance(c.position);
                        dist <= selection_radius.max(sel_r)
                    },
                )
                .map(|(id, dist, _)| (id, dist));

            let selected_id = closest_object.map(|(id, _)| id);
            let selected: Vec<_> = selected_id.into_iter().collect();
            self.apply_player_drawable_selection(player_id, &selected, !add_to_selection);
            selected_id
        } else {
            None
        }
    }

    /// Command selected units to stop all actions
    pub fn command_stop(&mut self, player_id: u32) {
        self.command_stop_with_effects(player_id, &mut Default::default())
    }

    pub(crate) fn command_stop_with_effects(
        &mut self,
        player_id: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Some(player) = self.players.get(&player_id) {
            let selected: Vec<ObjectId> = player
                .selected_objects
                .iter()
                .copied()
                .filter(|object_id| {
                    self.objects
                        .get(object_id)
                        .is_some_and(|obj| obj.owner_player_id == Some(player_id))
                })
                .collect();
            for &object_id in &selected {
                if self.flight_deck_ai_do_command_with_effects(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::Idle,
                    None,
                    None,
                    effects,
                ) {
                    continue;
                }

                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    if obj.ai_state() != AIState::Idle {
                        obj.set_ai_state(
                            AIState::Idle,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                    }
                    {}
                }
            }
            log::trace!("{} commanded {} units to stop", player_id, selected.len());
        }
    }

    /// Command selected units to attack-move to a position (with pathfinding)
    pub fn command_attack_move(&mut self, player_id: u32, target_position: Vec3) {
        self.command_attack_move_with_effects(player_id, target_position, &mut Default::default())
    }

    pub(crate) fn command_attack_move_with_effects(
        &mut self,
        player_id: u32,
        target_position: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Some(player) = self.players.get(&player_id) {
            let selected: Vec<ObjectId> = player
                .selected_objects
                .iter()
                .copied()
                .filter(|object_id| {
                    self.objects
                        .get(object_id)
                        .is_some_and(|obj| obj.owner_player_id == Some(player_id))
                })
                .collect();
            for &object_id in &selected {
                if self.flight_deck_ai_do_command_with_effects(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackMoveToPosition,
                    None,
                    Some(target_position),
                 effects) {
                    continue;
                }

                let (is_mobile, can_attack) = self
                    .objects
                    .get(&object_id)
                    .map(|obj| (obj.is_mobile(), obj.can_attack() || obj.weapon.is_some()))
                    .unwrap_or((false, false));
                if is_mobile {
                    if can_attack {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ));
                        }
                    }
                    let installed = self.move_object_with_pathfinding(
                        object_id,
                        target_position,
                        Some(if can_attack {
                            AIState::AttackMoving
                        } else {
                            AIState::Moving
                        }),
                    );
                    if can_attack && installed {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_auto_acquire_when_idle(true);
                            obj.set_max_shots_to_fire(-1);
                            obj.requested_destination = Some(target_position);
                            obj.set_ai_state(
                                AIState::AttackMoving,
                                &mut crate::game_logic::ObjectAudio::queue(
                                    &mut self.queued_audio_events,
                                ),
                            );
                        }
                    } else if can_attack {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.requested_destination = None;
                            obj.retry_path = false;
                            if obj.ai_state() != AIState::Idle {
                                obj.set_ai_state(
                                    AIState::Idle,
                                    &mut crate::game_logic::ObjectAudio::queue(
                                        &mut self.queued_audio_events,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            // C++ MSG_DO_ATTACKMOVETO VoiceMove (`CommandXlat.cpp:384-412`).
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                self.queue_picked_move_voice(&selected);
            }
        }
    }

    /// Get detailed information about an object (for UI display)
    pub fn get_object_info(&self, object_id: ObjectId) -> Option<ObjectInfo> {
        self.objects.get(&object_id).map(|obj| ObjectInfo {
            id: object_id,
            name: obj.get_display_name(),
            team: obj.team,
            object_type: obj.object_type,
            health: obj.health.clone(),
            max_health: obj.max_health,
            position: obj.get_position(),
            is_selected: obj.selected,
            is_moving: obj.status.moving,
            is_attacking: obj.status.attacking,
            under_construction: obj.status.under_construction,
            construction_percent: obj.construction_percent,
            experience_level: obj.experience.level,
            ai_state: obj.ai_state().clone(),
            can_attack: obj.can_attack(),
            can_move: obj.is_mobile(),
        })
    }

    /// Spawn a unit at the specified position (for testing/cheats)
    pub fn spawn_unit(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.spawn_unit_with_effects(template_name, team, position, &mut Default::default())
    }

    pub(crate) fn spawn_unit_with_effects(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.create_object_with_script_effects(template_name, team, position, None, effects)
    }

    pub(in super::super) fn template_team_hint(name: &str) -> Option<Team> {
        let upper = name.to_ascii_uppercase();
        if upper.starts_with("USA_") || upper.starts_with("AMERICA_") {
            Some(Team::USA)
        } else if upper.starts_with("CHINA_") {
            Some(Team::China)
        } else if upper.starts_with("GLA_") {
            Some(Team::GLA)
        } else if upper.starts_with("NEUTRAL_") || upper.starts_with("CIVILIAN_") {
            Some(Team::Neutral)
        } else {
            None
        }
    }

    /// Get available unit/building templates for a team.
    ///
    /// This keeps a broad fallback for generic templates while avoiding obvious
    /// cross-faction leakage for names with clear faction prefixes.
    pub fn get_available_templates(&self, team: Team) -> Vec<String> {
        let mut templates = self
            .templates
            .iter()
            .filter(|(name, template)| {
                // Exclude non-interactive map/decorative templates.
                let is_interactive = template.is_kind_of(KindOf::Selectable)
                    || template.is_kind_of(KindOf::Infantry)
                    || template.is_kind_of(KindOf::Vehicle)
                    || template.is_kind_of(KindOf::Aircraft)
                    || template.is_kind_of(KindOf::Structure)
                    || template.is_kind_of(KindOf::Worker)
                    || template.is_kind_of(KindOf::SupplyCenter)
                    || template.is_kind_of(KindOf::CommandCenter);
                if !is_interactive {
                    return false;
                }

                // Keep generic templates for all teams; faction-tagged names are filtered.
                match Self::template_team_hint(name.as_str()) {
                    Some(hinted_team) => hinted_team == team || team == Team::Neutral,
                    None => true,
                }
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        templates.sort();
        templates
    }

    /// Get templates registry (immutable access)
    pub fn get_templates(&self) -> &HashMap<String, ThingTemplate> {
        &self.templates
    }

    /// Get templates registry (mutable access)
    pub fn get_templates_mut(&mut self) -> &mut HashMap<String, ThingTemplate> {
        &mut self.templates
    }

    /// Demonstrate RTS functionality (for testing)
    pub fn demonstrate_rts_features(&mut self) {
        println!("\n🎮 DEMONSTRATING RTS FUNCTIONALITY:");

        // Show all objects and their status
        println!("\n📊 CURRENT GAME STATE:");
        println!("   Total Objects: {}", self.objects.registered_len());
        println!("   Players: {}", self.players.len());

        // Show objects by team
        for team in [Team::USA, Team::China, Team::GLA, Team::Neutral] {
            let team_objects: Vec<_> = self
                .objects
                .registered()
                .filter(|(_, obj)| obj.team == team && obj.is_alive())
                .collect();

            if !team_objects.is_empty() {
                println!(
                    "\n   {} Team Objects ({}): ",
                    team.get_name(),
                    team_objects.len()
                );
                for (id, obj) in team_objects.iter().take(5) {
                    // Show first 5
                    let health_percent = (obj.health.percentage() * 100.0) as u32;
                    let pos = obj.get_position();
                    println!(
                        "      {} - {} [{}% HP] at ({:.0}, {:.0}, {:.0})",
                        id,
                        obj.get_display_name(),
                        health_percent,
                        pos.x,
                        pos.y,
                        pos.z
                    );
                }
                if team_objects.len() > 5 {
                    println!("      ... and {} more", team_objects.len() - 5);
                }
            }
        }

        // Demonstrate selection
        println!("\n🖱️ TESTING SELECTION SYSTEM:");
        let usa_objects: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if obj.team == Team::USA && obj.is_alive() && obj.is_selectable() {
                    Some(*id)
                } else {
                    None
                }
            })
            .take(3)
            .collect();

        if !usa_objects.is_empty() {
            let local_player = self.local_player_id().unwrap_or(0);
            self.select_objects(local_player, usa_objects.clone());
            println!("   Selected {} USA units", usa_objects.len());
        }

        // Demonstrate movement command
        println!("\n⚡ TESTING MOVEMENT COMMANDS:");
        if let Some(player) = self.players.get(&0) {
            if !player.selected_objects.is_empty() {
                let target_pos = Vec3::new(50.0, 0.0, 50.0);
                self.command_move(0, target_pos);
                println!(
                    "   Commanded selected units to move to ({}, {}, {})",
                    target_pos.x, target_pos.y, target_pos.z
                );
            }
        }

        // Show visual info for rendering
        println!("\n🎨 VISUAL INFORMATION:");
        let visual_info = self
            .local_player_id()
            .map(|player_id| self.get_visual_object_info(player_id))
            .unwrap_or_default();
        println!("   {} objects visible to local player", visual_info.len());

        for (id, info) in visual_info.iter().take(3) {
            println!(
                "      {} - {} {} [Selected: {}, Health: {:.0}%]",
                id,
                info.team.get_name(),
                if let Some(ref model) = info.model_name {
                    model
                } else {
                    "Unknown"
                },
                info.is_selected,
                info.health_percentage * 100.0
            );
        }

        // Show available templates
        println!("\n🏭 AVAILABLE UNIT TEMPLATES:");
        let templates = self.get_available_templates(Team::USA);
        println!("   {} unit templates available:", templates.len());
        for template in templates.iter().take(8) {
            println!("      - {}", template);
        }
        if templates.len() > 8 {
            println!("      ... and {} more", templates.len() - 8);
        }

        println!("\n✅ RTS FUNCTIONALITY DEMONSTRATION COMPLETE!\n");
    }

    /// Add one AI opponent with an explicit difficulty (skirmish config path).
    pub fn add_ai_opponent(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        self.ensure_ai_faction_templates(team);
        self.ai_manager.add_ai_player(player_id, team, difficulty);
        crate::ai::AIManager::apply_ctor_can_build_units(self, player_id);
        // C++ AISkirmishPlayer ctor runs after the AIPlayer base ctor and
        // re-enables production (AISkirmishPlayer.cpp:61).
        crate::ai::AIManager::apply_skirmish_can_build_units(self, player_id);
    }

    /// After `load_map` wipes world objects, rebind host AI rebuild soup and
    /// re-ensure faction structure/unit templates used by the AI build path.
    ///
    /// Preserves: registered AI players, difficulty, `is_active`, base layout
    /// template names, and host `players` (cash/slots). Does **not** claim full
    /// C++ AI parity — only keeps the host AI update path non-panicking and able
    /// to issue builds after a skirmish preserve load.
    pub fn rebind_host_ai_after_map_load(&mut self) {
        let mut teams = self.ai_manager.registered_teams();
        for player in self.players.values() {
            if player.team != Team::Neutral && !teams.contains(&player.team) {
                teams.push(player.team);
            }
        }
        for team in teams {
            self.ensure_ai_faction_templates(team);
        }
        self.ai_manager.rebind_after_world_reset();
        // Skirmish residual: AI must have enough cash to start rebuild soup after
        // preserve load. Never wipe intentional positive cash; only top-up empty
        // AI slots (e.g. map path that recreated players without starting_cash).
        self.ensure_skirmish_ai_starting_cash(10_000);
        // Map residual: synthetic default bases (e.g. 120,120) are often shrouded /
        // far from the retail start. Anchor each AI rebuild soup on that team's
        // living CommandCenter (or any structure) so LegalBuildCode can pass FOW.
        self.relocate_host_ai_bases_to_map_starts();
        log::info!(
            "Host AI rebound after map load: ai_players={}, host_players={}",
            self.ai_manager.ai_players.len(),
            self.players.len()
        );
    }

    /// After load_map, move host AI base layouts onto map start structures.
    pub fn relocate_host_ai_bases_to_map_starts(&mut self) {
        let ai_ids: Vec<(u32, Team)> = self
            .ai_manager
            .ai_players
            .iter()
            .map(|(&id, ai)| (id, ai.team))
            .collect();
        for (pid, team) in ai_ids {
            let anchor = self
                .objects
                .registered_values()
                .filter(|o| o.team == team && o.is_alive() && o.is_kind_of(KindOf::CommandCenter))
                .map(|o| o.get_position())
                .next()
                .or_else(|| {
                    self.objects
                        .registered_values()
                        .filter(|o| {
                            o.team == team && o.is_alive() && o.is_kind_of(KindOf::Structure)
                        })
                        .map(|o| o.get_position())
                        .next()
                });
            if let Some(pos) = anchor {
                // Offset slightly so soup pads are not stacked on the CC footprint.
                self.relocate_host_ai_base(pid, pos + glam::Vec3::new(40.0, 0.0, 40.0));
            }
        }
    }

    /// Ensure registered host AI players have at least `min_cash` supplies.
    /// Used after load_map rebind so Medium AI can produce/rebuild without a
    /// full C++ economy parity pass.
    pub fn ensure_skirmish_ai_starting_cash(&mut self, min_cash: u32) {
        let ai_ids: Vec<u32> = self.ai_manager.ai_players.keys().copied().collect();
        for pid in ai_ids {
            if let Some(player) = self.players.get_mut(&pid) {
                if player.effective_supplies() < min_cash {
                    let need = min_cash.saturating_sub(player.effective_supplies());
                    log::info!(
                        "Topping up AI player {} cash {} -> {} after map rebind",
                        pid,
                        player.effective_supplies(),
                        min_cash
                    );
                    // Economy authority: gain delta to absolute floor, not host poke.
                    player.apply_supply_gain(need, &mut self.economy_events);
                    self.economy_events.record(
                        player.id,
                        player.effective_supplies(),
                        player.power_available,
                    );
                }
            }
        }
    }

    /// Whether a host AI player is registered and currently active.
    pub fn is_host_ai_active(&self, player_id: u32) -> bool {
        self.ai_manager.is_ai_active(player_id)
    }

    /// Configured host AI difficulty, if the player is a registered AI opponent.
    pub fn host_ai_difficulty(&self, player_id: u32) -> Option<AIDifficulty> {
        self.ai_manager.ai_difficulty(player_id)
    }

    /// Ensure faction templates the host AI build/produce paths require are registered.
    pub fn ensure_ai_faction_templates(&mut self, team: Team) {
        // Prefer real WeaponStore / LocomotorStore stats (seeded/INI).
        let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
        if self.locomotor_definitions.is_none() {
            let _ = super::super::locomotor_bootstrap::ensure_host_locomotor_store();
        }
        // `vision` is the retail Object INI VisionRange for the object
        // (FactionBuilding.ini / faction unit INIs). Buildings author
        // ShroudClearingRange == VisionRange; units leave it at the -1
        // default so it resolves to VisionRange.
        fn structure(
            name: &str,
            kinds: &[KindOf],
            hp: f32,
            cost: u32,
            vision: f32,
        ) -> ThingTemplate {
            let mut t = ThingTemplate::new(name);
            t.set_health(hp);
            t.set_cost(cost, 0);
            t.build_time = 0.05;
            t.sight_range = vision;
            t.shroud_clearing_range = vision;
            for k in kinds {
                t.add_kind_of(*k);
            }
            t
        }
        fn unit(name: &str, kinds: &[KindOf], hp: f32, cost: u32, vision: f32) -> ThingTemplate {
            let mut t = structure(name, kinds, hp, cost, vision);
            // Retail units do not author ShroudClearingRange; the -1 default
            // resolves to VisionRange in resolved_shroud_clearing_range().
            t.shroud_clearing_range = -1.0;
            // Host combat: bind retail Weapon.ini name when known so create_object
            // resolves via WeaponStore (seed/INI). Do not set explicit
            // primary_weapon(Weapon::default()) — that short-circuits the store.
            if let Some(wname) = super::super::weapon_bootstrap::primary_weapon_name_for_unit(name)
            {
                t.set_primary_weapon_name(wname);
            }
            if let Some(wname) =
                super::super::weapon_bootstrap::secondary_weapon_name_for_unit(name)
            {
                t.set_secondary_weapon_name(wname);
            }
            // Host movement: bind SET_NORMAL Locomotor.ini name when known so
            // create_object applies retail-ish max_speed (e.g. BasicHuman 20).
            if let Some(lname) = super::super::locomotor_bootstrap::locomotor_name_for_unit(name) {
                t.set_locomotor_name(lname);
            }
            let set_names = super::super::locomotor_bootstrap::locomotor_set_names_for_unit(name);
            if set_names.len() >= 2 {
                t.set_locomotor_set_names(&set_names);
            }

            t
        }
        fn collector(
            name: &str,
            kinds: &[KindOf],
            hp: f32,
            cost: u32,
            build_time: f32,
            vision: f32,
        ) -> ThingTemplate {
            let mut t = unit(name, kinds, hp, cost, vision);
            // These are the retail Object INI values.  The loaded Object
            // catalog wins via the insert pass below; this only keeps
            // a headless skirmish's typed SupplyCenter production path alive.
            t.build_time = build_time;
            t.primary_weapon = None;
            t.secondary_weapon = None;
            t.primary_weapon_name = None;
            t.secondary_weapon_name = None;
            t
        }
        let entries: Vec<ThingTemplate> = match team {
            Team::USA => vec![
                structure(
                    "USA_CommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                    // FactionBuilding.ini VisionRange / ShroudClearingRange.
                    300.0,
                ),
                structure(
                    "AmericaCommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                    300.0,
                ),
                structure(
                    "USA_SupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                    200.0,
                ),
                structure(
                    "AmericaSupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                    200.0,
                ),
                structure(
                    "USA_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                    200.0,
                ),
                structure(
                    "AmericaPowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                    200.0,
                ),
                structure(
                    "USA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                    200.0,
                ),
                structure(
                    "AmericaBarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                    200.0,
                ),
                structure(
                    "USA_WarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                    200.0,
                ),
                structure(
                    "AmericaWarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                    200.0,
                ),
                unit(
                    "USA_Ranger",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    120.0,
                    100,
                    100.0,
                ),
                unit(
                    "AmericaInfantryRanger",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    120.0,
                    100,
                    100.0,
                ),
                unit(
                    "USA_Humvee",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    300.0,
                    400,
                    150.0,
                ),
                unit(
                    "AmericaVehicleHumvee",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    300.0,
                    400,
                    150.0,
                ),
                collector(
                    "AmericaVehicleChinook",
                    &[
                        KindOf::Vehicle,
                        KindOf::Aircraft,
                        KindOf::Harvester,
                        KindOf::Selectable,
                    ],
                    300.0,
                    1200,
                    10.0,
                    300.0,
                ),
                {
                    let mut d = unit(
                        "USA_Dozer",
                        &[
                            KindOf::Vehicle,
                            KindOf::Worker,
                            KindOf::Dozer,
                            KindOf::Selectable,
                        ],
                        300.0,
                        1000,
                        200.0,
                    );
                    // Workers are not combat units — clear default weapon.
                    d.primary_weapon = None;
                    d.secondary_weapon = None;
                    d.primary_weapon_name = None;
                    d.secondary_weapon_name = None;
                    d
                },
                {
                    // Retail PlayerTemplate StartingUnit0 name (ThingFactory).
                    // spawn_skirmish_starting_units uses this; Lone Eagle maps
                    // keep buildings but omit the starting dozer.
                    let mut d = unit(
                        "AmericaVehicleDozer",
                        &[
                            KindOf::Vehicle,
                            KindOf::Worker,
                            KindOf::Dozer,
                            KindOf::Selectable,
                        ],
                        300.0,
                        1000,
                        200.0,
                    );
                    d.primary_weapon = None;
                    d.secondary_weapon = None;
                    d.primary_weapon_name = None;
                    d.secondary_weapon_name = None;
                    d
                },
            ],
            Team::China => vec![
                structure(
                    "China_CommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                    300.0,
                ),
                structure(
                    "ChinaCommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                    300.0,
                ),
                structure(
                    "China_SupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                    200.0,
                ),
                structure(
                    "ChinaSupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                    200.0,
                ),
                structure(
                    "China_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                    200.0,
                ),
                structure(
                    "ChinaPowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                    200.0,
                ),
                structure(
                    "China_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                    200.0,
                ),
                structure(
                    "ChinaBarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                    200.0,
                ),
                structure(
                    "China_WarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                    200.0,
                ),
                structure(
                    "ChinaWarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                    200.0,
                ),
                unit(
                    "China_RedGuard",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                    100.0,
                ),
                unit(
                    "ChinaInfantryRedguard",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                    100.0,
                ),
                collector(
                    "ChinaVehicleSupplyTruck",
                    &[KindOf::Vehicle, KindOf::Harvester, KindOf::Selectable],
                    300.0,
                    600,
                    10.0,
                    150.0,
                ),
            ],
            Team::GLA => vec![
                structure(
                    "GLA_CommandCenter",
                    &[
                        KindOf::Structure,
                        KindOf::CommandCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1800.0,
                    500,
                    300.0,
                ),
                structure(
                    "GLACommandCenter",
                    &[
                        KindOf::Structure,
                        KindOf::CommandCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1800.0,
                    500,
                    300.0,
                ),
                structure(
                    "GLA_SupplyStash",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    900.0,
                    300,
                    200.0,
                ),
                structure(
                    "GLASupplyStash",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::CannotBuildNearSupplies,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    900.0,
                    300,
                    200.0,
                ),
                structure(
                    "GLA_ArmsDealer",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1100.0,
                    400,
                    200.0,
                ),
                structure(
                    "GLAArmsDealer",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1100.0,
                    400,
                    200.0,
                ),
                structure(
                    "GLA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    900.0,
                    200,
                    200.0,
                ),
                structure(
                    "GLABarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    900.0,
                    200,
                    200.0,
                ),
                unit(
                    "GLA_Soldier",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                    150.0,
                ),
                unit(
                    "GLAInfantryRebel",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                    150.0,
                ),
                unit(
                    "GLA_Technical",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    250.0,
                    300,
                    150.0,
                ),
                collector(
                    "GLAInfantryWorker",
                    &[
                        KindOf::Infantry,
                        KindOf::Worker,
                        KindOf::Dozer,
                        KindOf::Harvester,
                        KindOf::Selectable,
                    ],
                    100.0,
                    200,
                    3.0,
                    100.0,
                ),
            ],
            Team::Neutral => vec![],
        };
        for mut t in entries {
            // Retail faction structures author KINDOF_MP_COUNT_FOR_VICTORY
            // (FactionBuilding.ini `Object GLABarracks` et al.); victory
            // evaluation counts only STRUCTURE && MP_COUNT_FOR_VICTORY
            // members (C++ Team::hasAnyBuildings mask).
            if t.is_kind_of(KindOf::Structure) {
                t.add_kind_of(KindOf::MpCountForVictory);
            }
            match self.templates.get_mut(&t.name) {
                Some(existing) => {
                    // Retail vision floor: a curated starter that is still
                    // blind (VisionRange 0) keeps every host binding but
                    // gains the authored VisionRange so the shroud look pass
                    // can register it. Without it a real-map skirmish start
                    // never reveals even the owner's base and every AI build
                    // fails LBC_SHROUD forever (frozen economy). Authored
                    // non-zero values always win.
                    if existing.sight_range <= 0.0 {
                        existing.sight_range = t.sight_range;
                    }
                    if t.shroud_clearing_range > 0.0 && existing.shroud_clearing_range <= 0.0 {
                        existing.shroud_clearing_range = t.shroud_clearing_range;
                    }
                }
                None => {
                    self.templates.insert(t.name.clone(), t);
                }
            }
        }
    }

    /// Total host-AI activity counter (builds/production/attacks issued).
    pub fn host_ai_activity_count(&self) -> u64 {
        self.ai_manager.total_activity_count()
    }

    /// Snapshot the bounded host-AI registration and decision state that is
    /// safe to restore after a map/object load.  Transient commands and target
    /// pointers intentionally remain outside this surface.
    pub fn snapshot_host_ai_players_for_save(&self) -> Vec<crate::save_load::AIPlayerSnapshot> {
        self.ai_manager.snapshot_players_for_save()
    }

    /// Restore host-AI players after `PlayerSnapshot` and `ObjectSnapshot`
    /// have rebuilt their owning ids.  Invalid defensive-unit references are
    /// dropped rather than becoming stale post-load orders.
    pub fn restore_host_ai_players_from_save(
        &mut self,
        snapshots: &[crate::save_load::AIPlayerSnapshot],
    ) {
        let player_teams = self
            .players
            .iter()
            .map(|(&player_id, player)| (player_id, player.team))
            .collect();
        self.ai_manager
            .restore_players_from_save(snapshots, &player_teams);

        let valid_object_ids: std::collections::HashSet<ObjectId> =
            self.objects.registered_keys().copied().collect();
        for ai_player in self.ai_manager.ai_players.values_mut() {
            ai_player
                .defensive_units
                .retain(|id| valid_object_ids.contains(id));
        }
    }

    pub fn capture_ai_player_queue_persist(
        &self,
    ) -> Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist> {
        self.ai_manager.capture_queue_persist()
    }

    pub fn apply_ai_player_queue_persist(
        &mut self,
        rows: Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist>,
    ) {
        self.ai_manager.apply_queue_persist(rows);
        let valid_object_ids: std::collections::HashSet<ObjectId> =
            self.objects.registered_keys().copied().collect();
        for ai_player in self.ai_manager.ai_players.values_mut() {
            ai_player.retain_queue_object_ids(&valid_object_ids);
        }
    }

    pub fn clear_ai_player_queue_persist(&mut self) {
        self.ai_manager.clear_queue_persist();
    }

    /// Number of registered host AI players.
    pub fn host_ai_player_count(&self) -> usize {
        self.ai_manager.ai_players.len()
    }

    /// Set up AI opponents for skirmish matches
    pub fn setup_skirmish_ai(&mut self, human_player_id: u32) {
        println!("🤖 Setting up AI opponents for skirmish match...");

        // Add AI players for non-human players
        for player_id in 0..4 {
            if player_id == human_player_id {
                continue;
            }
            let team = self.players.get(&player_id).and_then(|p| {
                if p.is_human || !p.is_alive || p.is_replay_observer() || p.team == Team::Neutral {
                    None
                } else {
                    Some(p.team)
                }
            });
            if let Some(team) = team {
                // Legacy fallback when no SkirmishMatchConfig was supplied:
                // difficulty-by-player-id. Prefer apply_skirmish_config.
                let difficulty = match player_id {
                    1 => AIDifficulty::Medium,
                    2 => AIDifficulty::Hard,
                    3 => AIDifficulty::Easy,
                    _ => AIDifficulty::Medium,
                };

                self.add_ai_opponent(player_id, team, difficulty);
                println!(
                    "  Added AI player {} ({}) with {:?} difficulty",
                    player_id,
                    team.get_name(),
                    difficulty
                );
            }
        }

        println!("✅ AI opponents configured for challenging gameplay!");
    }

    /// Relocate host AI base layout (building queue positions) without mutating
    /// the template catalog. Keeps AI active while placing rebuild sites in range.
    pub fn relocate_host_ai_base(&mut self, player_id: u32, base_position: glam::Vec3) {
        self.ai_manager.relocate_ai_base(player_id, base_position);
    }

    /// Enable/disable AI for specific player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        self.ai_manager.set_ai_active(player_id, active);
    }

    /// True when this object's exact controlling player is non-local and its
    /// skirmish AI is inactive. Missing AI registration retains that policy;
    /// missing object/owner/player identity never falls back to a faction.
    pub fn skirmish_ai_auto_engage_paused(&self, object_id: ObjectId) -> bool {
        self.objects
            .get(&object_id)
            .and_then(|object| object.owner_player_id)
            .and_then(|player_id| self.players.get(&player_id).map(|p| (player_id, p)))
            .is_some_and(|(player_id, player)| {
                !player.is_local && !self.is_host_ai_active(player_id)
            })
    }

    /// Pause skirmish AI for `player_id` and clear that player's combat targets so
    /// residual unit AI does not keep counterfiring after the manager pause.
    /// Also cancels that player's production queues so barracks/factories do not
    /// keep spawning units during a golden map clear while AI is paused.
    /// Used by golden map clear (AI rebuild off + no structure auto-engage).
    pub fn pause_skirmish_ai_and_clear_combat(&mut self, player_id: u32) {
        self.set_ai_active(player_id, false);
        if !self.players.contains_key(&player_id) {
            return;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|o| o.owner_player_id == Some(player_id) && o.is_alive())
            .map(|o| o.id)
            .collect();
        // Cancel production first (needs building_data); then clear combat state.
        for id in ids.iter().copied() {
            let _ = self.cancel_all_production(id);
        }
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.target = None;
                obj.target_location = None;
                obj.set_status_force_attack(false);
                // Halt workers/dozers mid-rebuild so paused AI does not finish distant CCs.
                if obj.is_kind_of(KindOf::Worker) || obj.template_name.contains("Dozer") {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.movement.target_position = None;
                    if matches!(
                        obj.ai_state(),
                        AIState::Moving | AIState::Constructing | AIState::Repairing
                    ) {
                        if obj.ai_state() != AIState::Idle {
                            obj.set_ai_state(
                                AIState::Idle,
                                &mut crate::game_logic::ObjectAudio::queue(
                                    &mut self.queued_audio_events,
                                ),
                            );
                        }
                    }
                }
                if matches!(
                    obj.ai_state(),
                    AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                ) {
                    obj.set_ai_state(
                        AIState::Idle,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
            }
        }
    }

    /// Set AI difficulty for a player
    pub fn set_ai_difficulty(&mut self, player_id: u32, difficulty: AIDifficulty) {
        self.ai_manager.set_difficulty(player_id, difficulty);
    }

    /// Get AI status information
    pub fn get_ai_status(&self, player_id: u32) -> Option<String> {
        self.ai_manager.get_ai_info(player_id)
    }

    /// Start skirmish match with AI opponents
    pub fn start_skirmish_match(&mut self, human_team: Team, map_name: &str) {
        println!(
            "🎮 Starting skirmish match: {} vs AI",
            human_team.get_name()
        );

        // Start new game
        self.start_new_game(GameMode::Skirmish);

        // Load map
        self.load_map(map_name);

        // Create human player
        let human_player = Player::new(0, human_team, "Human Player", true);
        self.players.insert(0, human_player);

        // Create AI players with different teams
        let ai_teams = match human_team {
            Team::USA => vec![Team::China, Team::GLA],
            Team::China => vec![Team::USA, Team::GLA],
            Team::GLA => vec![Team::USA, Team::China],
            _ => vec![Team::USA, Team::China, Team::GLA],
        };

        for (i, &team) in ai_teams.iter().enumerate() {
            let ai_player_id = (i + 1) as u32;
            let ai_player = Player::new(
                ai_player_id,
                team,
                &format!("{} AI", team.get_name()),
                false,
            );
            self.players.insert(ai_player_id, ai_player);
        }

        // Set up AI opponents
        self.setup_skirmish_ai(0);

        println!(
            "✅ Skirmish match started with {} AI opponents!",
            ai_teams.len()
        );
    }

    /// Demonstrate AI capabilities
    pub fn demonstrate_ai_functionality(&mut self) {
        println!("\n🤖 DEMONSTRATING AI FUNCTIONALITY:");

        // Show AI status for each AI player
        for player_id in 1..4 {
            if let Some(status) = self.get_ai_status(player_id) {
                println!("\n{}", status);
            }
        }

        // Show AI decision making
        println!("\n🧠 AI DECISION MAKING:");
        println!("   - Economic management: Resource optimization and base construction");
        println!("   - Military strategy: Unit production and attack coordination");
        println!("   - Intelligence gathering: Enemy assessment and reconnaissance");
        println!("   - Base defense: Defensive positioning and threat response");
        println!("   - Advanced tactics: Combined arms and veteran unit management");

        println!("\n✅ AI SYSTEM FULLY OPERATIONAL!\n");
    }

    /// Add comprehensive faction-specific building templates
    /// This ensures perfect alignment with C++ template expectations
    pub(in super::super) fn add_faction_building_templates(&mut self) {
        log::debug!("Adding faction-specific building templates for C++ alignment");

        // Integrate the comprehensive building templates from buildings.rs
        let building_templates = create_building_templates();
        let template_count = building_templates.len();

        for (name, template) in building_templates {
            self.templates.insert(name, template);
        }

        log::info!(
            "Added {} faction-specific building templates",
            template_count
        );
    }

    /// Initialize script system for mission/level scripting
    /// Called once per map load to set up script engine and load mission scripts
    pub fn initialize_scripts(&mut self, map_name: &str) {
        self.initialize_scripts_with_map_data(map_name, None);
    }

    pub(in super::super) fn initialize_scripts_with_map_data(
        &mut self,
        map_name: &str,
        decoded_map: Option<&super::super::script_loader::ChunkyMap>,
    ) {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        self.initialize_scripts_with_map_data_and_difficulty(map_name, decoded_map, difficulty);
    }

    pub(in super::super) fn initialize_scripts_with_map_data_and_difficulty(
        &mut self,
        map_name: &str,
        decoded_map: Option<&super::super::script_loader::ChunkyMap>,
        difficulty: Option<gamelogic::player::GameDifficulty>,
    ) {
        if self.scripts_loaded {
            return;
        }
        if self.owned_script_engine.is_none()
            && !self.admit_map_script_definitions_with_difficulty(map_name, decoded_map, difficulty)
        {
            return;
        }
        if let Some(engine) = self.owned_script_engine.as_mut() {
            engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
                self.mission_scripts.clone(),
            ))));
        }
        self.finish_map_script_foundation();
    }

    /// Terrain admission provides script definitions before PlayerList::newGame
    /// creates singleton teams. ScriptEngine::newMap is a separate later phase
    /// (GameLogic.cpp 1468-1474), so those callbacks can observe earlier callbacks.
    pub(in super::super) fn admit_map_script_definitions(
        &mut self,
        map_name: &str,
        decoded_map: Option<&super::super::script_loader::ChunkyMap>,
    ) -> bool {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        self.admit_map_script_definitions_with_difficulty(map_name, decoded_map, difficulty)
    }

    pub(in super::super) fn admit_map_script_definitions_with_difficulty(
        &mut self,
        map_name: &str,
        decoded_map: Option<&super::super::script_loader::ChunkyMap>,
        difficulty: Option<gamelogic::player::GameDifficulty>,
    ) -> bool {
        let Some(receiving) = self.owned_script_engine.as_ref() else {
            log::error!(
                "Map script admission requires the receiving interpreter loan to be returned"
            );
            return false;
        };
        let templates =
            gamelogic::scripting::chunk_codec::ScriptTemplateLookup::from_engine(receiving);
        let result = match decoded_map {
            Some(map) => super::super::script_loader::load_map_scripts_from_chunky_with_templates(
                map, &templates,
            ),
            None => {
                super::super::script_loader::load_map_scripts_with_templates(map_name, &templates)
            }
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                log::error!("Failed to decode map scripts for '{}': {}", map_name, error);
                return false;
            }
        };
        let (lists, path) = result
            .map(|result| (result.script_lists, Some(result.source_path)))
            .unwrap_or_default();
        // set_script_list_for_player's only rejection is this bound. Validate
        // every decoded list before changing any receiving list or handler.
        if lists.len() > gamelogic::common::MAX_PLAYER_COUNT {
            log::error!("Too many map script lists for '{}'", map_name);
            return false;
        }
        let handler = Arc::new(MissionScriptActionHandler::new(
            self.mission_scripts.clone(),
        ));
        let engine = self
            .owned_script_engine
            .as_mut()
            .expect("receiving interpreter retained during detached decode");
        // reset already ran before map decoding; new_map remains at its later
        // original phase. Definition admission must not reset controls/templates.
        engine.clear_script_lists();
        if let Some(difficulty) = difficulty {
            engine.set_global_difficulty(difficulty);
        }
        engine.set_action_handler(Some(handler));
        for (index, list) in lists.iter().enumerate() {
            if let Err(error) =
                engine.set_script_list_for_player(index, Some(Box::new(list.clone())))
            {
                log::error!("Failed to admit map script list {}: {:?}", index, error);
                return false;
            }
        }
        // Retain the original install boundary's publication of the actual
        // receiving interpreter policy before player/factory callbacks.
        self.objects_receive_difficulty_bonus =
            engine.get_objects_should_receive_difficulty_bonus();
        self.loaded_script_lists = lists;
        self.script_source_path = path;
        true
    }

    pub(in super::super) fn finish_map_script_foundation(&mut self) -> bool {
        let Some(engine) = self.owned_script_engine.as_mut() else {
            return false;
        };
        engine.new_map();
        self.objects_receive_difficulty_bonus = true;
        self.scripts_loaded = true;
        true
    }
}

#[cfg(test)]
mod vision_floor_tests {
    use super::*;

    /// Retail FactionBuilding.ini VisionRange / ShroudClearingRange pairs the
    /// host AI catalog must carry. A blind entry (sight 0) can never register
    /// a shroud looker, so with fog-of-war on every AI build dies on
    /// LBC_SHROUD and the skirmish economy freezes (live start_game evidence).
    #[test]
    fn ai_faction_templates_carry_retail_vision() {
        let cases: &[(Team, &[(&str, f32)])] = &[
            (
                Team::USA,
                &[
                    ("USA_CommandCenter", 300.0),
                    ("AmericaCommandCenter", 300.0),
                    ("USA_SupplyCenter", 200.0),
                    ("AmericaSupplyCenter", 200.0),
                    ("USA_PowerPlant", 200.0),
                    ("AmericaPowerPlant", 200.0),
                    ("USA_Barracks", 200.0),
                    ("AmericaBarracks", 200.0),
                    ("USA_WarFactory", 200.0),
                    ("AmericaWarFactory", 200.0),
                    ("USA_Ranger", 100.0),
                    ("AmericaInfantryRanger", 100.0),
                    ("USA_Humvee", 150.0),
                    ("AmericaVehicleHumvee", 150.0),
                    ("AmericaVehicleChinook", 300.0),
                    ("USA_Dozer", 200.0),
                    ("AmericaVehicleDozer", 200.0),
                ],
            ),
            (
                Team::China,
                &[
                    ("China_CommandCenter", 300.0),
                    ("ChinaCommandCenter", 300.0),
                    ("China_SupplyCenter", 200.0),
                    ("ChinaSupplyCenter", 200.0),
                    ("China_PowerPlant", 200.0),
                    ("ChinaPowerPlant", 200.0),
                    ("China_Barracks", 200.0),
                    ("ChinaBarracks", 200.0),
                    ("China_WarFactory", 200.0),
                    ("ChinaWarFactory", 200.0),
                    ("China_RedGuard", 100.0),
                    ("ChinaInfantryRedguard", 100.0),
                    ("ChinaVehicleSupplyTruck", 150.0),
                ],
            ),
            (
                Team::GLA,
                &[
                    ("GLA_CommandCenter", 300.0),
                    ("GLACommandCenter", 300.0),
                    ("GLA_SupplyStash", 200.0),
                    ("GLASupplyStash", 200.0),
                    ("GLA_ArmsDealer", 200.0),
                    ("GLAArmsDealer", 200.0),
                    ("GLA_Barracks", 200.0),
                    ("GLABarracks", 200.0),
                    ("GLA_Soldier", 150.0),
                    ("GLAInfantryRebel", 150.0),
                    ("GLA_Technical", 150.0),
                    ("GLAInfantryWorker", 100.0),
                ],
            ),
        ];
        for (team, entries) in cases {
            let mut logic = GameLogic::new();
            logic.ensure_ai_faction_templates(*team);
            for (name, vision) in *entries {
                let template = logic
                    .templates
                    .get(*name)
                    .unwrap_or_else(|| panic!("{team:?} catalog entry {name} missing"));
                assert_eq!(
                    template.sight_range, *vision,
                    "{team:?} {name} must carry its retail VisionRange"
                );
                let resolved = template.resolved_shroud_clearing_range();
                assert_eq!(
                    resolved, *vision,
                    "{team:?} {name} must clear shroud out to its vision"
                );
            }
        }
    }

    /// A curated starter that already exists keeps every host binding but, when
    /// still blind, gains the retail vision floor; an authored non-zero vision
    /// is never overridden.
    #[test]
    fn blind_existing_entry_gains_vision_floor_authored_value_wins() {
        let mut logic = GameLogic::new();
        let mut blind = ThingTemplate::new("GLACommandCenter");
        blind.set_health(1800.0);
        logic.templates.insert("GLACommandCenter".into(), blind);
        let mut authored = ThingTemplate::new("GLASupplyStash");
        authored.sight_range = 123.0;
        logic.templates.insert("GLASupplyStash".into(), authored);

        logic.ensure_ai_faction_templates(Team::GLA);

        let cc = logic.templates.get("GLACommandCenter").expect("cc");
        assert_eq!(cc.sight_range, 300.0, "blind entry gains retail vision");
        assert_eq!(cc.resolved_shroud_clearing_range(), 300.0);
        let stash = logic.templates.get("GLASupplyStash").expect("stash");
        assert_eq!(stash.sight_range, 123.0, "authored vision wins");
    }
}

#[cfg(test)]
mod map_script_difficulty_tests {
    use super::*;

    #[test]
    fn map_script_admission_preserves_actual_difficulty_before_foundation() {
        use gamelogic::player::GameDifficulty;
        let mut owner = GameLogic::new();
        owner.prepare_native_command_difficulty(0);
        owner
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_global_difficulty(GameDifficulty::Hard);
        let mut peer = GameLogic::new();
        peer.prepare_native_command_difficulty(0);
        // Admission retains the actual receiving interpreter. Its difficulty
        // deliberately differs from the session's earlier Easy selection.
        let difficulty = owner
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        let map = super::super::super::script_loader::ChunkyMap {
            source: std::path::PathBuf::from("no-authored-script-lists.map"),
            toc: std::collections::HashMap::from([(1, "UnrelatedMapData".into())]),
            body_offset: 0,
            bytes: [
                1u32.to_le_bytes().as_slice(),
                1u16.to_le_bytes().as_slice(),
                0i32.to_le_bytes().as_slice(),
            ]
            .concat(),
        };
        assert!(owner.admit_map_script_definitions_with_difficulty(
            "no-authored-script-lists.map",
            Some(&map),
            difficulty,
        ));
        assert_eq!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            GameDifficulty::Hard
        );
        assert!(owner.finish_map_script_foundation());
        assert_eq!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            GameDifficulty::Hard
        );
        assert_eq!(
            peer.owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            GameDifficulty::Easy
        );
        // Direct callers keep their own actual interpreter value too.
        owner.scripts_loaded = false;
        owner.initialize_scripts_with_map_data("no-authored-script-lists.map", Some(&map));
        assert_eq!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            GameDifficulty::Hard
        );
    }
}

#[cfg(test)]
#[path = "successful_script_owner_tests.rs"]
mod successful_script_owner_tests;

#[cfg(test)]
mod admission_lifetime_tests {
    use super::*;

    #[test]
    fn checked_public_admission_rejects_live_collision_without_resetting_query_or_score_state() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        let id = ObjectId(777);
        let mut original = Object::new(ThingTemplate::new("AdmissionOwner"), id, Team::USA);
        original.kill_score_phase = crate::game_logic::kill_credit::KillScorePhase::Complete;
        original.kill_experience_awarded = true;
        owner.try_add_object(original).unwrap();
        let query = owner.objects[&id].clone();
        let generation = owner.objects[&id].visual_object_generation;
        let next_generation = owner.next_visual_object_generation;
        let roster = owner.objects.ordered_ids().to_vec();
        let error = owner
            .try_add_object(Object::new(
                ThingTemplate::new("RejectedReplacement"),
                id,
                Team::China,
            ))
            .unwrap_err();
        assert!(error.to_string().contains("occupied"));
        assert_eq!(owner.objects[&id].template_name, "AdmissionOwner");
        assert_eq!(owner.objects[&id].visual_object_generation, generation);
        assert_eq!(owner.next_visual_object_generation, next_generation);
        assert_eq!(owner.objects.ordered_ids(), roster);
        assert_eq!(
            owner.objects[&id].kill_score_phase,
            crate::game_logic::kill_credit::KillScorePhase::Complete
        );
        assert!(owner.objects[&id].kill_experience_awarded);
        // Query/continuation clones are not implicitly reset into fresh births.
        peer.try_add_object(query).unwrap();
        assert_eq!(
            peer.objects[&id].kill_score_phase,
            crate::game_logic::kill_credit::KillScorePhase::Complete
        );
        assert!(peer.objects[&id].kill_experience_awarded);
        assert!(owner.pending_object_removal_snapshot().is_empty());
        assert!(peer.pending_object_removal_snapshot().is_empty());
    }
}
