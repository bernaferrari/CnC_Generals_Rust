//! Host scripts `impl GameLogic` — `add_object_selection`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! add_object / selection / visibility / AI / initialize_scripts
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Add object to the game world
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let id = object.id;
        self.objects.insert(id, object);
        self.host_radar_add_object(id);
        id
    }

    // ====== ENHANCED RTS COMMAND SYSTEM ======

    /// Get all objects visible to a specific team (for rendering and UI)
    pub fn get_visible_objects(&self, viewing_team: Team) -> Vec<ObjectId> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
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
        viewing_team: Team,
    ) -> Vec<(ObjectId, super::super::ObjectVisualInfo)> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
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
        if let Some(player) = self.players.get_mut(&player_id) {
            let mut selected_objects = Vec::new();

            // Clear previous selection if not adding
            if !add_to_selection {
                for &old_id in &player.selected_objects {
                    if let Some(obj) = self.objects.get_mut(&old_id) {
                        obj.deselect();
                    }
                }
                player.selected_objects.clear();
            }

            // Find objects in the selection area.
            // C++ parity: uses bounding-circle intersection with the selection
            // rectangle, not just center-point containment.  This allows selecting
            // large objects whose center is outside the box but whose radius
            // overlaps it.
            for (id, obj) in &mut self.objects {
                if obj.owner_player_id == Some(player_id) && obj.is_selectable() {
                    let pos = obj.get_position();
                    let r = obj.selection_radius;
                    // Circle-vs-AABB intersection test.
                    let closest_x = pos.x.clamp(min_pos.x, max_pos.x);
                    let closest_z = pos.z.clamp(min_pos.z, max_pos.z);
                    let dist_sq = (pos.x - closest_x).powi(2) + (pos.z - closest_z).powi(2);
                    if dist_sq <= r * r {
                        obj.select();
                        selected_objects.push(*id);
                        if !player.selected_objects.contains(id) {
                            player.selected_objects.push(*id);
                        }
                    }
                }
            }

            log::trace!(
                "{} selected {} objects in area",
                player_id,
                selected_objects.len()
            );
            selected_objects
        } else {
            Vec::new()
        }
    }

    /// Select a single object by click
    pub fn select_object_at_position(
        &mut self,
        player_id: u32,
        position: Vec3,
        selection_radius: f32,
        add_to_selection: bool,
    ) -> Option<ObjectId> {
        if let Some(player) = self.players.get_mut(&player_id) {
            let team = player.team;
            // Pure residual acquire: nearest selectable friendly in click radius (3D).
            // Per-object selection_radius expands the pick disk (C++ pick residual).
            let candidates: Vec<_> = self
                .objects
                .iter()
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

            if let Some((selected_id, _)) = closest_object {
                // Clear previous selection if not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                }

                // Select the new object
                if let Some(obj) = self.objects.get_mut(&selected_id) {
                    obj.select();
                    if !player.selected_objects.contains(&selected_id) {
                        player.selected_objects.push(selected_id);
                    }
                }

                log::trace!("{} selected object {}", player_id, selected_id);
                Some(selected_id)
            } else {
                // Clear selection if clicking on empty space and not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                    log::trace!("{} cleared selection", player_id);
                }
                None
            }
        } else {
            None
        }
    }

    /// Command selected units to stop all actions
    pub fn command_stop(&mut self, player_id: u32) {
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
                if self.flight_deck_ai_do_command(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::Idle,
                    None,
                    None,
                ) {
                    continue;
                }

                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving();
                    obj.stop_attack();
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(object_id, 0);
                        crate::game_logic::host_ai_decision_log::record_stop_attack(object_id);
                    }
                }
            }
            log::trace!("{} commanded {} units to stop", player_id, selected.len());
        }
    }

    /// Command selected units to attack-move to a position (with pathfinding)
    pub fn command_attack_move(&mut self, player_id: u32, target_position: Vec3) {
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
                if self.flight_deck_ai_do_command(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackMoveToPosition,
                    None,
                    Some(target_position),
                ) {
                    continue;
                }

                let (is_mobile, can_attack) = self
                    .objects
                    .get(&object_id)
                    .map(|obj| (obj.is_mobile(), obj.can_attack() || obj.weapon.is_some()))
                    .unwrap_or((false, false));
                if is_mobile {
                    self.move_object_with_pathfinding(
                        object_id,
                        target_position,
                        Some(if can_attack {
                            AIState::AttackMoving
                        } else {
                            AIState::Moving
                        }),
                    );
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        if can_attack {
                            obj.is_attack_path = true;
                            obj.auto_acquire_when_idle = true;
                            obj.set_max_shots_to_fire(-1);
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
            ai_state: obj.ai_state.clone(),
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
        self.create_object(template_name, team, position)
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
        println!("   Total Objects: {}", self.objects.len());
        println!("   Players: {}", self.players.len());

        // Show objects by team
        for team in [Team::USA, Team::China, Team::GLA, Team::Neutral] {
            let team_objects: Vec<_> = self
                .objects
                .iter()
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
            .iter()
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
        let visual_info = self.get_visual_object_info(Team::USA);
        println!("   {} objects visible to USA team", visual_info.len());

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
                .values()
                .filter(|o| o.team == team && o.is_alive() && o.is_kind_of(KindOf::CommandCenter))
                .map(|o| o.get_position())
                .next()
                .or_else(|| {
                    self.objects
                        .values()
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
                    player.apply_supply_gain(need);
                    crate::game_logic::host_economy_log::record(
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
        let _ = super::super::locomotor_bootstrap::ensure_host_locomotor_store();
        fn structure(name: &str, kinds: &[KindOf], hp: f32, cost: u32) -> ThingTemplate {
            let mut t = ThingTemplate::new(name);
            t.set_health(hp);
            t.set_cost(cost, 0);
            t.build_time = 0.05;
            for k in kinds {
                t.add_kind_of(*k);
            }
            t
        }
        fn unit(name: &str, kinds: &[KindOf], hp: f32, cost: u32) -> ThingTemplate {
            let mut t = structure(name, kinds, hp, cost);
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
        ) -> ThingTemplate {
            let mut t = unit(name, kinds, hp, cost);
            // These are the retail Object INI values.  The loaded Object
            // catalog wins via entry().or_insert_with below; this only keeps
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
                ),
                structure(
                    "AmericaCommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
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
                ),
                structure(
                    "USA_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "AmericaPowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "USA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                ),
                structure(
                    "AmericaBarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
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
                ),
                unit(
                    "USA_Ranger",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    120.0,
                    100,
                ),
                unit(
                    "AmericaInfantryRanger",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    120.0,
                    100,
                ),
                unit(
                    "USA_Humvee",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    300.0,
                    400,
                ),
                unit(
                    "AmericaVehicleHumvee",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    300.0,
                    400,
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
                ),
                {
                    let mut d = unit(
                        "USA_Dozer",
                        &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
                        300.0,
                        1000,
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
                        &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
                        300.0,
                        1000,
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
                ),
                structure(
                    "ChinaCommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
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
                ),
                structure(
                    "China_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "ChinaPowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "China_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                ),
                structure(
                    "ChinaBarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
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
                ),
                unit(
                    "China_RedGuard",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
                unit(
                    "ChinaInfantryRedguard",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
                collector(
                    "ChinaVehicleSupplyTruck",
                    &[KindOf::Vehicle, KindOf::Harvester, KindOf::Selectable],
                    300.0,
                    600,
                    10.0,
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
                ),
                structure(
                    "GLA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    900.0,
                    200,
                ),
                structure(
                    "GLABarracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    900.0,
                    200,
                ),
                unit(
                    "GLA_Soldier",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
                unit(
                    "GLAInfantryRebel",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
                unit(
                    "GLA_Technical",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    250.0,
                    300,
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
            self.templates.entry(t.name.clone()).or_insert_with(|| t);
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
            self.objects.keys().copied().collect();
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
            self.objects.keys().copied().collect();
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

        // --- Initialize the gamelogic crate AI subsystem ---
        // the_ai singleton (pathfinder, groups) and the AiIntegrationManager
        // must be initialized before any AI player updates run.
        let ai_store = the_ai(); if let Ok(mut ai) = ai_store.write() {
            ai.init();
            log::info!("the_ai singleton initialized for skirmish");
        }
        if let Err(e) = initialize_ai_integration() {
            log::warn!("AiIntegrationManager init failed (non-fatal): {:?}", e);
        }

        // Add AI players for non-human players
        for player_id in 0..4 {
            if player_id == human_player_id {
                continue;
            }
            let team = self.players.get(&player_id).and_then(|p| {
                if !p.is_alive || p.name == "ReplayObserver" || p.team == Team::Neutral {
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

    /// True when this team's skirmish AI player is non-local and currently paused.
    /// Human/local teams always return false (auto-engage remains available).
    pub fn skirmish_ai_auto_engage_paused(&self, team: Team) -> bool {
        self.players.iter().any(|(&pid, player)| {
            player.team == team && !player.is_local && !self.is_host_ai_active(pid)
        })
    }

    /// Pause skirmish AI for `player_id` and clear that team's combat targets so
    /// residual unit AI does not keep counterfiring after the manager pause.
    /// Also cancels production queues on that team so barracks/factories do not
    /// keep spawning units during a golden map clear while AI is paused.
    /// Used by golden map clear (AI rebuild off + no structure auto-engage).
    pub fn pause_skirmish_ai_and_clear_combat(&mut self, player_id: u32) {
        self.set_ai_active(player_id, false);
        let team = self.players.get(&player_id).map(|p| p.team);
        let Some(team) = team else {
            return;
        };
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.team == team && o.is_alive())
            .map(|o| o.id)
            .collect();
        // Cancel production first (needs building_data); then clear combat state.
        for id in ids.iter().copied() {
            let _ = self.cancel_all_production(id);
        }
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.stop_attack();
                obj.target = None;
                obj.target_location = None;
                obj.set_status_force_attack(false);
                // Halt workers/dozers mid-rebuild so paused AI does not finish distant CCs.
                if obj.is_kind_of(KindOf::Worker) || obj.template_name.contains("Dozer") {
                    obj.stop_moving();
                    obj.movement.target_position = None;
                    if matches!(
                        obj.ai_state,
                        AIState::Moving | AIState::Constructing | AIState::Repairing
                    ) {
                        obj.set_ai_state(AIState::Idle);
                    }
                }
                if matches!(
                    obj.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                ) {
                    obj.set_ai_state(AIState::Idle);
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
        if self.scripts_loaded {
            return; // Already initialized
        }

        // Live C++ path is ScriptEngine + executor (GameLogic.cpp:3600).
        // Leftover ScriptingEngine / ActionRegistry / Rhai / TriggerSystem is
        // leftover-only (hq-8ta4n): do not construct a second script brain.
        {
            let handler: Arc<dyn ScriptActionHandler> = Arc::new(MissionScriptActionHandler::new(
                self.mission_scripts.clone(),
            ));
            let _ = gamelogic::scripting::engine::initialize_script_engine();
            if let Ok(mut live_guard) = gamelogic::scripting::engine::get_script_engine().write() {
                if let Some(live) = live_guard.as_mut() {
                    live.set_action_handler(Some(handler));
                }
            }
        }

        match super::super::script_loader::load_map_scripts(map_name) {
            Ok(Some(result)) => {
                self.loaded_script_lists = result.script_lists;
                self.script_source_path = Some(result.source_path);
                self.scripts_loaded = true;
                self.mission_scripts
                    .install_lists(&self.loaded_script_lists);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();
                self.eva_superweapon_science_hidden.clear();

                // Install decoded per-player lists into the crate ScriptEngine.
                // Live once-per-frame walk is ScriptEngine::update (C++
                // GameLogic.cpp:3600).  MissionScriptRuntime stays for
                // CALL_SUBROUTINE / action-handler queues, not a second walk.
                let _ = gamelogic::scripting::engine::initialize_script_engine();
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        // C++ ScriptEngine::newMap() resets transient script runtime
                        // then starts the 33-frame FADE_MULTIPLY fade-in from black.
                        engine.reset();
                        engine.new_map();
                        for (idx, list) in self.loaded_script_lists.iter().enumerate() {
                            let _ = engine
                                .set_script_list_for_player(idx, Some(Box::new(list.clone())));
                        }
                    }
                }

                log::info!(
                    "Loaded {} mission scripts for '{}'",
                    result.total_scripts,
                    map_name
                );
            }
            Ok(None) => {
                self.loaded_script_lists.clear();
                self.script_source_path = None;
                self.scripts_loaded = true;
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();
                self.eva_superweapon_science_hidden.clear();

                // Ensure the legacy ScriptEngine doesn't keep running scripts from a previous map.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                        engine.new_map();
                    }
                }

                log::warn!("No mission scripts found for '{}'", map_name);
            }
            Err(err) => {
                log::error!(
                    "Failed to decode mission scripts for '{}': {}",
                    map_name,
                    err
                );
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();
                self.eva_superweapon_science_hidden.clear();

                // On load failures, clear any previously loaded scripts for safety.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                        engine.new_map();
                    }
                }
            }
        }
    }
}
