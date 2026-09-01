use super::*;

impl CommandSystem {
    /// Create a new command system
    pub fn new() -> Self {
        Self {
            current_mode: CommandMode::Normal,
            command_queue: VecDeque::new(),
            next_command_id: 1,
            mouse_drag_start: None,
            mouse_down_time: None,
            command_history: Vec::new(),
            player_settings: HashMap::new(),
        }
    }

    /// Get (or lazily create) mutable command settings for a player
    pub(super) fn player_settings_mut(&mut self, player_id: u32) -> &mut PlayerCommandSettings {
        self.player_settings.entry(player_id).or_default()
    }

    /// Read-only view of a player's settings (creating default when missing).
    pub fn player_settings(&mut self, player_id: u32) -> PlayerCommandSettings {
        self.player_settings_mut(player_id).clone()
    }

    /// Enable or disable waypoint mode for a player.
    pub fn set_waypoint_mode_for_player(&mut self, player_id: u32, enabled: bool) {
        self.player_settings_mut(player_id).waypoint_mode = enabled;
    }

    /// Toggle auto-attack preference and return the new value.
    pub fn toggle_auto_attack(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.auto_attack = !settings.auto_attack;
        settings.auto_attack
    }

    /// Toggle whether moves should preserve formation and return the new value.
    pub fn toggle_formation_move(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.formation_move = !settings.formation_move;
        settings.formation_move
    }

    /// Toggle whether selection should attempt smart grouping and return the new value.
    pub fn toggle_smart_select(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.smart_select = !settings.smart_select;
        settings.smart_select
    }

    /// Select units matching predicate for the player and queue the selection command.
    pub fn select_units_by_predicate<F>(
        &mut self,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: &GameLogic,
        mut predicate: F,
    ) -> bool
    where
        F: FnMut(ObjectId) -> bool,
    {
        // Exact player ownership rather than faction: two USA slots must not
        // be selected together.
        if !game_logic.player_exists(player_id) {
            return false;
        }

        let units = game_logic.selectable_unit_ids_for_player_where(player_id, |id| predicate(id));

        if units.is_empty() {
            return false;
        }

        let command = self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !modifier_keys.shift,
                units: units.clone(),
            },
            &units,
            player_id,
            modifier_keys,
        );
        self.queue_command(command);
        true
    }

    /// Build a command that selects all objects matching the double-clicked target
    pub(super) fn create_select_similar_command(
        &mut self,
        target_id: ObjectId,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: Option<&GameLogic>,
        presentation_similar: &[ObjectId],
    ) -> Option<GameCommand> {
        // Wave 236: prefer presentation-frozen similar unit ids.
        if !presentation_similar.is_empty() {
            let mut units = presentation_similar.to_vec();
            if !units.contains(&target_id) {
                units.push(target_id);
            }
            let command_units = units.clone();
            return Some(self.create_command(
                CommandType::CreateSelectedGroup {
                    create_new: true,
                    units: command_units,
                },
                units.as_slice(),
                player_id,
                modifier_keys,
            ));
        }

        // Wave 245: boot residual via unit/selection probes (no get_object dual-read).
        let game_logic = game_logic?;
        let player_team = game_logic.player_team(player_id)?;
        let target_team = game_logic.unit_team(target_id)?;
        if target_team != player_team {
            return None;
        }
        let template_name = game_logic.unit_template_name(target_id)?;
        let object_type = game_logic.unit_object_type(target_id)?;
        let mut units = game_logic.selectable_similar_unit_ids(
            target_team,
            &template_name,
            object_type,
            modifier_keys.alt,
        );

        if units.is_empty() {
            return None;
        }

        if !units.contains(&target_id) {
            units.push(target_id);
        }

        let command_units = units.clone();
        Some(self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: true,
                units: command_units,
            },
            units.as_slice(),
            player_id,
            modifier_keys,
        ))
    }

    /// Set the current command mode
    pub fn set_mode(&mut self, mode: CommandMode) {
        self.current_mode = mode.clone();
        log::debug!("Command mode changed to: {:?}", mode);
    }

    /// Process mouse input and create appropriate commands
    pub fn process_mouse_input(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: Option<&GameLogic>,
    ) -> Option<GameCommand> {
        if context.is_drag {
            self.mouse_drag_start = context.drag_start.or(Some(context.screen_position));
        } else {
            self.mouse_drag_start = None;
        }

        match context.mouse_button {
            MouseButton::Left => {
                self.process_left_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Right => {
                self.process_right_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Middle => {
                self.process_middle_click(context, selected_units, player_id, game_logic)
            }
        }
    }

    /// Process left mouse click
    pub(super) fn process_left_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: Option<&GameLogic>,
    ) -> Option<GameCommand> {
        match &self.current_mode {
            CommandMode::Normal => {
                let now = Instant::now();
                let is_double_click = self
                    .mouse_down_time
                    .map(|last| now.duration_since(last) <= DOUBLE_CLICK_THRESHOLD)
                    .unwrap_or(false)
                    && self.player_settings_mut(player_id).smart_select
                    && !context.is_drag;
                self.mouse_down_time = Some(now);

                if is_double_click {
                    if let Some(target_id) = context.target_object {
                        if let Some(command) = self.create_select_similar_command(
                            target_id,
                            player_id,
                            context.modifier_keys,
                            game_logic,
                            &context.presentation_select_similar_units,
                        ) {
                            return Some(command);
                        }
                    }
                }

                if context.is_drag {
                    // Area selection
                    Some(self.create_selection_command(context, player_id, game_logic))
                } else if let Some(target_id) = context.target_object {
                    // Select single unit
                    let create_new = !context.modifier_keys.shift;
                    Some(self.create_command(
                        CommandType::CreateSelectedGroup {
                            create_new,
                            units: vec![target_id],
                        },
                        selected_units,
                        player_id,
                        context.modifier_keys,
                    ))
                } else {
                    None
                }
            }
            CommandMode::BuildMode { template_name } => {
                // Place building
                Some(self.create_command(
                    CommandType::DozerConstruct {
                        template_name: template_name.clone(),
                        location: context.world_position,
                        orientation: 0.0,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            CommandMode::SpecialPower { power_type } => {
                // C++ CommandXlat.cpp:1055-1078 NEED_TARGET_POS: always
                // MSG_DO_SPECIAL_POWER_AT_LOCATION. Unit under the cursor is
                // only the optional object-in-way id. Leftover ActionManager
                // canDoSpecialPowerAtObject is FALSE for these types.
                let target = match context.target_object {
                    Some(target_id)
                        if !leftover_special_power_is_location_target_only(power_type) =>
                    {
                        PowerTarget::Object(target_id)
                    }
                    _ => PowerTarget::Location(context.world_position),
                };

                Some(self.create_command(
                    CommandType::DoSpecialPower {
                        power_type: power_type.clone(),
                        target,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            _ => None,
        }
    }

    /// Process right mouse click - creates movement and attack commands
    pub(super) fn process_right_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: Option<&GameLogic>,
    ) -> Option<GameCommand> {
        if selected_units.is_empty() {
            return None;
        }

        let (mut waypoint_mode, auto_attack) = {
            let settings = self.player_settings_mut(player_id);
            (settings.waypoint_mode, settings.auto_attack)
        };

        if context.modifier_keys.alt {
            waypoint_mode = true;
        }

        // C++ evaluateContextCommand: isInWaypointMode first — any context
        // command including Ctrl force-attack becomes MSG_ADD_WAYPOINT
        // (CommandXlat.cpp:1458-1474).
        let mode = if waypoint_mode {
            CommandMode::Waypoint
        } else if context.modifier_keys.ctrl {
            CommandMode::ForceAttack
        } else {
            self.current_mode.clone()
        };

        let mut command_type = match &mode {
            CommandMode::ForceAttack => {
                if let Some(target_id) = context.target_object {
                    // Wave 1103: force-attack object residual fail-closed on sold
                    // (presentation pick already peels; belt-and-suspenders via hint).
                    let sold = context
                        .target_presentation
                        .as_ref()
                        .filter(|h| h.id == target_id)
                        .map(|h| h.sold || !h.is_alive)
                        .unwrap_or(false);
                    if sold {
                        CommandType::ForceAttackGround {
                            location: context.world_position,
                        }
                    } else {
                        CommandType::ForceAttackObject { target_id }
                    }
                } else {
                    CommandType::ForceAttackGround {
                        location: context.world_position,
                    }
                }
            }
            CommandMode::ForceMove => CommandType::ForceMoveTo {
                destination: context.world_position,
            },
            CommandMode::Waypoint => CommandType::AddWaypoint {
                destination: context.world_position,
            },
            _ => {
                // Context-sensitive command
                self.determine_context_command(context, selected_units, game_logic)
            }
        };

        if auto_attack {
            if let CommandType::MoveTo { destination, .. } = command_type {
                command_type = CommandType::AttackMoveTo {
                    destination,
                    max_shots: -1,
                };
            }
        }

        Some(self.create_command(
            command_type,
            selected_units,
            player_id,
            context.modifier_keys,
        ))
    }

    /// Process middle mouse click
    pub(super) fn process_middle_click(
        &mut self,
        _context: &MouseCommandContext,
        _selected_units: &[ObjectId],
        _player_id: u32,
        _game_logic: Option<&GameLogic>,
    ) -> Option<GameCommand> {
        // Middle click typically used for camera controls
        None
    }

    /// Determine context-sensitive command based on target and selected units
    pub(super) fn determine_context_command(
        &self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        game_logic: Option<&GameLogic>,
    ) -> CommandType {
        // C++ CommandXlat.cpp:1659-1684 — override dest before resume/dock.
        if selection_can_override_special_power_destination(
            &context.selected_presentation,
            selected_units,
            game_logic,
        ) {
            return CommandType::OverrideSpecialPowerDestination {
                location: context.world_position,
            };
        }
        if let Some(target_id) = context.target_object {
            // Wave 228: prefer presentation-frozen target identity when installed.
            if let Some(hint) = context
                .target_presentation
                .as_ref()
                .filter(|h| h.id == target_id)
            {
                if let Some(cmd) = self.classify_right_click_target_from_presentation(
                    selected_units,
                    &context.selected_presentation,
                    target_id,
                    hint,
                    context.world_position,
                    // Wave 541: target presentation freeze is authoritative.
                    // Empty selected_presentation fails closed for capability probes
                    // (no live GameLogic dual-read). Boot residual uses the
                    // non-presentation branch below when target_presentation is absent.
                    None,
                ) {
                    return cmd;
                }
            } else if let Some(gl) = game_logic {
                // Wave 245: boot residual via ObjectId probes (no get_object dual-read).
                if self.can_resume_construction(selected_units, target_id, gl) {
                    return CommandType::ResumeConstruction { target_id };
                }
                // C++ CommandXlat::translateMouseButton checks
                // ACTIONTYPE_DOCK_AT (SELECTION_ALL) immediately after Resume
                // Construction.  This must precede attack/capture as well as
                // repair: an enemy railed transport is still a legal Dock
                // target for a selected vehicle or infantry unit.
                if self.can_dock_at_target(selected_units, target_id, gl) {
                    return CommandType::Dock { target_id };
                }
                // A warehouse advertises the same supply-source KindOf used
                // by generic Gather.  C++ exposes it through DockUpdate, so
                // do not consume its exact dock target as a Gather command.
                if gl.unit_dock_kind(target_id) != DockKind::SupplyWarehouse
                    && self.can_gather_from_target(selected_units, target_id, gl)
                {
                    return CommandType::Gather { target_id };
                }
                if self.can_attack_target(selected_units, target_id, gl) {
                    return CommandType::AttackObject { target_id };
                }
                // C++ CommandXlat evaluates attack before its CaptureBuilding
                // special-power action.  An enemy capturable structure with a
                // legal weapon target must therefore attack; neutral/non-
                // attackable capturable structures fall through here.
                if self.can_capture_building(selected_units, target_id, gl) {
                    return CommandType::CaptureBuilding { target_id };
                }
                // C++ CommandXlat.cpp:2050-2156 after capture.
                if self.can_disable_vehicle_hack(selected_units, target_id, gl) {
                    return CommandType::DisableVehicleHack { target_id };
                }
                if self.can_steal_cash_hack(selected_units, target_id, gl) {
                    return CommandType::StealCashHack { target_id };
                }
                if self.can_hacker_disable_building(selected_units, target_id, gl) {
                    return CommandType::HackerDisableBuilding { target_id };
                }
                if self.can_repair_target(selected_units, target_id, gl) {
                    return CommandType::Repair { target_id };
                }
                if self.can_enter_target(selected_units, target_id, gl) {
                    return CommandType::Enter { target_id };
                }
                if self.can_get_serviced_at_target(selected_units, target_id, gl) {
                    if gl.unit_is_kind_of(target_id, KindOf::HealPad) {
                        return CommandType::GetHealed { target_id };
                    } else {
                        return CommandType::GetRepaired { target_id };
                    }
                }
                if gl.unit_is_kind_of(target_id, KindOf::Crate)
                    || gl.host_money_crates.contains(target_id)
                {
                    let salvage = gl
                        .host_money_crates
                        .get(target_id)
                        .is_some_and(|entry| entry.is_salvage);
                    if salvage
                        && selected_units.iter().any(|&unit_id| {
                            gl.unit_is_alive(unit_id)
                                && gl.unit_can_move(unit_id)
                                && gl.unit_is_kind_of(unit_id, KindOf::Salvager)
                        })
                    {
                        return CommandType::DoSalvage {
                            destination: context.world_position,
                        };
                    }
                    if !salvage
                        && selected_units
                            .iter()
                            .any(|&unit_id| gl.unit_is_alive(unit_id) && gl.unit_can_move(unit_id))
                    {
                        return CommandType::MoveTo {
                            destination: context.world_position,
                            waypoints: Vec::new(),
                        };
                    }
                }
            }
        }

        // C++ CommandXlat.cpp:2180-2199 — empty ground + AUTO_RALLYPOINT → SetRallyPoint.
        if context.target_object.is_none()
            && !selected_units.is_empty()
            && game_logic.is_some_and(|gl| {
                selected_units.iter().all(|&id| {
                    gl.unit_is_alive(id) && gl.unit_is_kind_of(id, KindOf::AutoRallypoint)
                })
            })
        {
            return CommandType::SetRallyPoint {
                location: context.world_position,
            };
        }

        // Default to move command (Ctrl ForceAttack handled before context path).
        CommandType::MoveTo {
            destination: context.world_position,
            waypoints: Vec::new(),
        }
    }

    /// Create area selection command from drag
    pub(super) fn create_selection_command(
        &mut self,
        context: &MouseCommandContext,
        player_id: u32,
        game_logic: Option<&GameLogic>,
    ) -> GameCommand {
        // Wave 236: prefer presentation-frozen box-select ids when provided.
        if !context.presentation_box_select_units.is_empty() {
            return self.create_command(
                CommandType::CreateSelectedGroup {
                    create_new: !context.modifier_keys.shift,
                    units: context.presentation_box_select_units.clone(),
                },
                &context.presentation_box_select_units,
                player_id,
                context.modifier_keys,
            );
        }

        let Some(game_logic) = game_logic else {
            return self.create_command(
                CommandType::CreateSelectedGroup {
                    create_new: !context.modifier_keys.shift,
                    units: Vec::new(),
                },
                &[],
                player_id,
                context.modifier_keys,
            );
        };

        if !game_logic.player_exists(player_id) {
            return self.create_command(
                CommandType::CreateSelectedGroup {
                    create_new: !context.modifier_keys.shift,
                    units: Vec::new(),
                },
                &[],
                player_id,
                context.modifier_keys,
            );
        }

        let drag_start = context.drag_start.unwrap_or(context.screen_position);
        let drag_end = context.drag_end.unwrap_or(context.screen_position);
        let viewport_size = context.viewport_size.unwrap_or(Vec2::new(800.0, 600.0));
        let world_min = context.world_min.unwrap_or(Vec3::new(-400.0, 0.0, -300.0));
        let world_max = context.world_max.unwrap_or(Vec3::new(400.0, 0.0, 300.0));
        let drag_start_world = context
            .drag_start_world
            .unwrap_or_else(|| screen_to_world(drag_start, viewport_size, world_min, world_max));
        let drag_end_world = context
            .drag_end_world
            .unwrap_or_else(|| screen_to_world(drag_end, viewport_size, world_min, world_max));

        let min_x = drag_start_world.x.min(drag_end_world.x);
        let max_x = drag_start_world.x.max(drag_end_world.x);
        let min_z = drag_start_world.z.min(drag_end_world.z);
        let max_z = drag_start_world.z.max(drag_end_world.z);

        // Exact-owner box selection. `Team` remains faction-only.
        let units = game_logic
            .selectable_unit_ids_in_bounds_for_player(player_id, min_x, max_x, min_z, max_z);

        self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !context.modifier_keys.shift,
                units: units.clone(),
            },
            &units,
            player_id,
            context.modifier_keys,
        )
    }

    /// Create a game command with metadata
    pub(super) fn create_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) -> GameCommand {
        let command = GameCommand {
            command_type,
            player_id,
            command_id: self.next_command_id,
            timestamp: SystemTime::now(),
            selected_units: selected_units.to_vec(),
            modifier_keys,
        };

        self.next_command_id += 1;
        command
    }

    /// Queue command for execution
    pub fn queue_command(&mut self, command: GameCommand) {
        log::debug!("Queuing command: {:?}", command.command_type);
        tap_host_command_for_recorder(&command);
        self.command_queue.push_back(command);
    }

    /// Create and queue a command immediately (used by keyboard shortcuts).
    pub fn queue_immediate_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) {
        let command = self.create_command(command_type, selected_units, player_id, modifier_keys);
        self.queue_command(command);
    }

    /// Process all queued commands
    pub fn process_commands(&mut self, game_logic: &mut GameLogic) -> Vec<CommandResult> {
        let mut results = Vec::new();

        while let Some(command) = self.command_queue.pop_front() {
            let result = self.execute_command(&command, game_logic);
            results.push(result);

            // Add to history for replay/undo
            self.command_history.push(command);
        }

        results
    }

    /// Execute a single command
    pub fn execute_command(
        &self,
        command: &GameCommand,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut executor =
            crate::command_executor::CommandExecutor::new(game_logic, command.player_id);
        match executor.execute_command(command.clone()) {
            Ok(result) => result,
            Err(err) => {
                log::warn!(
                    "Failed to execute command {:?} for player {}: {}",
                    command.command_type,
                    command.player_id,
                    err
                );
                CommandResult::InvalidCommand
            }
        }
    }

    /// Execute move command - core RTS functionality
    pub(super) fn execute_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        _waypoints: &[Vec3],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 230: authority mutations via GameLogic unit_command_* APIs.
        let mut all_success = true;
        for &unit_id in units {
            if game_logic.unit_command_move_to(unit_id, destination) {
                log::debug!("Unit {} moving to {:?}", unit_id.0, destination);
            } else {
                all_success = false;
            }
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::CannotMoveToLocation
        }
    }

    /// Execute attack command - core RTS functionality
    pub(super) fn execute_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 230/244: target probe via unit_exists / unit_is_dead_or_missing (no get_object).
        if !game_logic.unit_exists(target_id) {
            return CommandResult::InvalidTarget;
        }
        if game_logic.unit_is_dead_or_missing(target_id) {
            return CommandResult::TargetDestroyed;
        }
        let mut all_success = true;
        for &unit_id in units {
            if game_logic.unit_command_attack(unit_id, target_id) {
                log::debug!("Unit {} attacking target {}", unit_id.0, target_id.0);
            } else {
                all_success = false;
            }
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    /// Execute attack-move command
    pub(super) fn execute_attack_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 231: attack-move via GameLogic unit_command_attack_move_to.
        let mut all_success = true;
        for &unit_id in units {
            if game_logic.unit_command_attack_move_to(unit_id, destination) {
                log::debug!("Unit {} attack-moving to {:?}", unit_id.0, destination);
            } else {
                all_success = false;
            }
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute force attack command
    pub(super) fn execute_force_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 230: force-attack via GameLogic unit_command_force_attack.
        let mut all_success = true;
        for &unit_id in units {
            if game_logic.unit_command_force_attack(unit_id, target_id) {
                log::debug!("Unit {} force-attacking target {}", unit_id.0, target_id.0);
            } else {
                all_success = false;
            }
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    /// Execute force attack ground command
    pub(super) fn execute_force_attack_ground_command(
        &self,
        units: &[ObjectId],
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 231: force-attack ground via GameLogic unit_command_attack_ground.
        let mut all_success = true;
        for &unit_id in units {
            if game_logic.unit_command_attack_ground(unit_id, location) {
                log::debug!(
                    "Unit {} force-attacking ground at {:?}",
                    unit_id.0,
                    location
                );
            } else {
                all_success = false;
            }
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute stop command
    pub(super) fn execute_stop_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 230: stop via GameLogic unit_command_stop.
        for &unit_id in units {
            if game_logic.unit_command_stop(unit_id) {
                log::debug!("Unit {} stopped", unit_id.0);
            }
        }
        CommandResult::Success
    }

    /// Execute scatter command by pushing units away from their current positions.
    pub(super) fn execute_scatter_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 231: scatter goals via unit_position_if_movable + unit_command_move_to_moving.
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        pub(super) const BASE_DISTANCE: f32 = 25.0;
        pub(super) const DISTANCE_VARIATION: f32 = 10.0;

        let mut goals: Vec<(ObjectId, Vec3)> = Vec::new();
        for (index, &unit_id) in units.iter().enumerate() {
            let Some(origin) = game_logic.unit_position_if_movable(unit_id) else {
                continue;
            };
            let angle = ((unit_id.0 as usize + index) as f32 * 0.318_309_87) % TAU;
            let distance = BASE_DISTANCE + (index as f32 % DISTANCE_VARIATION).abs();
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * distance;
            goals.push((unit_id, origin + offset));
        }
        for (unit_id, destination) in goals {
            if game_logic.unit_command_move_to_moving(unit_id, destination) {
                log::debug!("Unit {} scattering toward {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    /// Arrange selected units into a grid formation centered around their centroid.
    pub(super) fn execute_create_formation_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 231: formation via unit_position_if_movable + unit_command_move_to_moving.
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut movable_units = Vec::new();
        for &unit_id in units {
            if let Some(pos) = game_logic.unit_position_if_movable(unit_id) {
                movable_units.push((unit_id, pos));
            }
        }

        if movable_units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut centroid = Vec3::ZERO;
        for (_, position) in &movable_units {
            centroid += *position;
        }
        centroid /= movable_units.len() as f32;

        let columns = (movable_units.len() as f32).sqrt().ceil() as usize;
        let rows = movable_units.len().div_ceil(columns);
        let spacing = 20.0;

        for (index, (unit_id, _)) in movable_units.iter().enumerate() {
            let row = (index / columns) as f32;
            let column = (index % columns) as f32;
            let offset_x = (column - (columns as f32 - 1.0) * 0.5) * spacing;
            let offset_z = (row - (rows as f32 - 1.0) * 0.5) * spacing;
            let destination = centroid + Vec3::new(offset_x, 0.0, offset_z);

            if game_logic.unit_command_move_to_moving(*unit_id, destination) {
                log::debug!("Unit {} forming up at {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    pub(super) fn execute_view_command_center(
        &self,
        player_id: u32,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 242: team via player_team probe (no &Player dual-read).
        let Some(_team) = game_logic.player_team(player_id) else {
            return CommandResult::InvalidCommand;
        };

        if let Some(position) = game_logic.player_command_center_position(player_id) {
            game_logic.request_player_camera_look_at(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Execute guard command
    pub(super) fn execute_guard_command(
        &self,
        units: &[ObjectId],
        target: &GuardTarget,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 230: guard via GameLogic unit_command_guard_*.
        for &unit_id in units {
            match target {
                GuardTarget::Position(pos) => {
                    if game_logic.unit_command_guard_position(unit_id, *pos) {
                        log::debug!("Unit {} guarding position {:?}", unit_id.0, pos);
                    }
                }
                GuardTarget::Object(target_id) => {
                    if game_logic.unit_command_guard_object(unit_id, *target_id) {
                        log::debug!("Unit {} guarding object {}", unit_id.0, target_id.0);
                    }
                }
            }
        }
        CommandResult::Success
    }

    /// Execute construction command
    pub(super) fn execute_construct_command(
        &self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let (base_cost, is_structure) = match game_logic.get_templates().get(template_name) {
            Some(t) => (
                t.build_cost,
                t.is_kind_of(crate::game_logic::KindOf::Structure),
            ),
            None => return CommandResult::InvalidCommand,
        };

        if !is_structure {
            return CommandResult::InvalidCommand;
        }

        // Find a constructor unit
        for &unit_id in units {
            // A producer/builder belongs to a player slot, not merely a
            // faction. This prevents a second USA player paying for and
            // owning a building created by the first USA player's dozer.
            let Some((player_id, _team)) = game_logic.unit_owner_if_can_construct(unit_id) else {
                continue;
            };

            let mut build_cost = base_cost;
            build_cost.supplies = game_logic.modified_build_cost_supplies(
                player_id,
                template_name,
                base_cost.supplies,
            );

            if !game_logic.try_spend_player_resources(player_id, &build_cost) {
                return CommandResult::InvalidCommand;
            }

            let created = game_logic.create_object_under_construction_for_player(
                template_name,
                player_id,
                location,
            );
            if created.is_none() {
                game_logic.player_refund_supplies(player_id, build_cost.supplies);
                return CommandResult::InvalidCommand;
            }

            // Wave 232: dozer construct last-writes via GameLogic authority API.
            if !game_logic.unit_command_begin_construct(unit_id, location) {
                game_logic.player_refund_supplies(player_id, build_cost.supplies);
                return CommandResult::InvalidCommand;
            }

            log::debug!(
                "Unit {} constructing {} at {:?}",
                unit_id.0,
                template_name,
                location
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    /// Execute selection command
    pub(super) fn execute_selection_command(
        &self,
        player_id: u32,
        create_new: bool,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Wave 242: selection last-writes via select_objects / unit_select_if_team
        // and player_* probes (no &Player / &mut Player dual-read). Wave 231 residual.
        if !game_logic.player_exists(player_id) {
            return CommandResult::InvalidCommand;
        }
        if create_new {
            game_logic.select_objects(player_id, units.to_vec());
            log::debug!(
                "Player {} selected {} units",
                player_id,
                game_logic.player_selected_count(player_id)
            );
            return CommandResult::Success;
        }
        let mut added = Vec::new();
        for &unit_id in units {
            if game_logic.unit_select_if_player(unit_id, player_id) {
                added.push(unit_id);
            }
        }
        game_logic.player_extend_selection(player_id, &added);
        log::debug!(
            "Player {} selected {} units",
            player_id,
            game_logic.player_selected_count(player_id)
        );
        CommandResult::Success
    }

    /// Validate if selected units can capture target structure residual.
    pub(super) fn can_capture_building(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        units
            .iter()
            .copied()
            .any(|unit_id| game_logic.can_unit_capture_building(unit_id, target_id, true))
    }

    fn can_disable_vehicle_hack(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        if !game_logic.unit_is_alive(target_id)
            || !game_logic.unit_is_kind_of(target_id, KindOf::Vehicle)
            || game_logic.unit_is_kind_of(target_id, KindOf::Aircraft)
        {
            return false;
        }
        let Some(target_team) = game_logic.unit_team(target_id) else {
            return false;
        };
        units.iter().copied().any(|unit_id| {
            game_logic.unit_is_alive(unit_id)
                && game_logic.unit_team(unit_id).is_some_and(|team| {
                    team != target_team && target_team != crate::game_logic::Team::Neutral
                })
                && game_logic.unit_template_name(unit_id).is_some_and(|n| {
                    crate::game_logic::host_hero_abilities::is_black_lotus_template(&n)
                })
        })
    }

    fn can_steal_cash_hack(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        let Some(target_name) = game_logic.unit_template_name(target_id) else {
            return false;
        };
        if !game_logic.unit_is_alive(target_id)
            || !game_logic.unit_is_kind_of(target_id, KindOf::Structure)
            || game_logic.unit_under_construction(target_id)
            || !crate::game_logic::host_hero_abilities::is_cash_hack_target_template(&target_name)
        {
            return false;
        }
        let Some(target_team) = game_logic.unit_team(target_id) else {
            return false;
        };
        units.iter().copied().any(|unit_id| {
            game_logic.unit_is_alive(unit_id)
                && game_logic.unit_team(unit_id).is_some_and(|team| {
                    team != target_team && target_team != crate::game_logic::Team::Neutral
                })
                && game_logic.unit_template_name(unit_id).is_some_and(|n| {
                    crate::game_logic::host_hero_abilities::is_black_lotus_template(&n)
                })
        })
    }

    fn can_hacker_disable_building(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        units
            .iter()
            .copied()
            .any(|unit_id| game_logic.can_unit_hacker_disable_building(unit_id, target_id, true))
    }

    /// Validate if selected units can attack target
    pub(super) fn can_attack_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        // Wave 244/245: unit + target probes (no &Object dual-read).
        if game_logic.unit_is_dead(target_id) || !game_logic.unit_exists(target_id) {
            return false;
        }
        for &unit_id in units {
            if game_logic.unit_can_attack(unit_id)
                && matches!(
                    game_logic.get_able_to_attack_specific_object(
                        unit_id,
                        target_id,
                        crate::game_logic::AbleToAttackType::NewTarget,
                        true,
                    ),
                    crate::game_logic::CanAttackResult::Possible
                        | crate::game_logic::CanAttackResult::PossibleAfterMoving
                )
            {
                return true;
            }
        }
        false
    }

    /// Validate if selected units can gather from a resource target

    /// Wave 228/229: RMB target classification from presentation freeze.
    /// Selected-unit capability probes prefer `selected_presentation` (Wave 229);
    /// live GameLogic unit dual-read is boot residual only.
    pub(super) fn classify_right_click_target_from_presentation(
        &self,
        units: &[ObjectId],
        selected_presentation: &[PresentationSelectedUnitHint],
        target_id: ObjectId,
        hint: &PresentationTargetHint,
        world_position: Vec3,
        game_logic: Option<&GameLogic>,
    ) -> Option<CommandType> {
        // Wave 235/541: InGame RMB classification from presentation freeze.
        // selected_presentation drives capability probes; empty ⇒ fail-closed
        // (no live GameLogic dual-read). game_logic arg retained for boot callers.
        if !hint.is_alive {
            return None;
        }
        let any_resource_collector = || {
            if !selected_presentation.is_empty() {
                return selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.is_resource_collector);
            }
            // Boot residual via unit probes (no get_object dual-read).
            game_logic.is_some_and(|gl| {
                units.iter().any(|&unit_id| {
                    gl.unit_is_alive(unit_id) && gl.unit_is_resource_collector(unit_id)
                })
            })
        };
        // C++ construction/resume authority is exactly `KINDOF_DOZER`.  A
        // `HARVESTER` is deliberately narrower and is used only by Gather;
        // the host's old Worker/name compatibility class must not authorize
        // a frozen physical RMB command.
        let any_worker = || {
            if !selected_presentation.is_empty() {
                return selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.is_worker);
            }
            game_logic.is_some_and(|gl| {
                units
                    .iter()
                    .any(|&unit_id| gl.unit_is_alive(unit_id) && gl.unit_is_dozer(unit_id))
            })
        };
        let any_attacker = || {
            if !selected_presentation.is_empty() {
                return selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.can_attack);
            }
            // Wave 244: boot residual via unit probes (no get_object dual-read).
            game_logic.is_some_and(|gl| {
                units
                    .iter()
                    .any(|&unit_id| gl.unit_is_alive(unit_id) && gl.unit_can_attack(unit_id))
            })
        };
        let any_capturer = || {
            if !selected_presentation.is_empty() {
                return selected_presentation.iter().any(|u| {
                    u.is_alive
                        && u.can_move
                        && u.capture_power != CapturePowerKind::None
                        && u.capture_power_ready
                });
            }
            // Boot residual remains at the same centralized authority boundary
            // as execution, including exact SpecialPower readiness.
            game_logic.is_some_and(|gl| {
                units
                    .iter()
                    .copied()
                    .any(|unit_id| gl.can_unit_capture_building(unit_id, target_id, true))
            })
        };
        let any_repairer = || {
            if !selected_presentation.is_empty() {
                return selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.can_repair);
            }
            // Wave 244: boot residual via unit probes (no get_object dual-read).
            game_logic.is_some_and(|gl| {
                units
                    .iter()
                    .any(|&unit_id| gl.unit_is_alive(unit_id) && gl.unit_can_repair(unit_id))
            })
        };

        // Resume construction on unfinished ally structure
        if hint.is_structure
            && hint.under_construction
            && !hint.is_enemy_of_local
            && !hint.sold
            && any_worker()
        {
            return Some(CommandType::ResumeConstruction { target_id });
        }
        // C++ CommandXlat::translateMouseButton 1721: ACTIONTYPE_DOCK_AT
        // uses SELECTION_ALL and emits MSG_DOCK before repair.
        if self.can_dock_at_target_from_presentation(units, selected_presentation, hint) {
            return Some(CommandType::Dock { target_id });
        }
        // Gather
        // Wave 1099: sold residual fail-closed on gather target.
        if hint.dock_kind != DockKind::SupplyWarehouse
            && hint.is_resource
            && !hint.sold
            && any_resource_collector()
        {
            return Some(CommandType::Gather { target_id });
        }
        // C++ CommandXlat.cpp:1758-1854 — Repair / GetRepaired / GetHealed
        // immediately after Dock, before hijack/enter/attack/capture.
        if hint.is_structure
            && hint.is_damaged
            && !hint.under_construction
            && !hint.sold
            && (hint.is_friendly_of_local || hint.is_neutral)
            && any_repairer()
        {
            return Some(CommandType::Repair { target_id });
        }
        // C++ ActionManager pairs ground vehicles with REPAIR_PAD and
        // above-terrain VEHICLE+AIRCRAFT units with FS_AIRFIELD. Keep the
        // full pairing in frozen physical input; executor repeats it.
        // Wave 1099: sold residual fail-closed on repair pad.
        if hint.is_friendly_of_local
            && !hint.sold
            && !hint.under_construction
            && hint.can_provide_service
            && (hint.provides_vehicle_repair || hint.provides_aircraft_repair)
        {
            let any_damaged_serviceable = if !selected_presentation.is_empty() {
                selected_presentation.iter().any(|u| {
                    u.is_alive
                        && u.is_damaged
                        && u.can_move
                        && u.can_request_service
                        && ((u.is_vehicle && !u.is_aircraft && hint.provides_vehicle_repair)
                            || (u.is_vehicle
                                && u.is_aircraft
                                && u.is_above_terrain
                                && hint.provides_aircraft_repair))
                })
            } else {
                false
            };
            if any_damaged_serviceable {
                return Some(CommandType::GetRepaired { target_id });
            }
        }
        // Get healed at heal pad
        // Wave 1099: sold residual fail-closed on heal pad.
        if hint.can_provide_service
            && hint.provides_heal
            && !hint.sold
            && !hint.under_construction
            && hint.is_friendly_of_local
        {
            let any_injured_infantry = if !selected_presentation.is_empty() {
                selected_presentation.iter().any(|u| {
                    u.is_alive
                        && u.is_damaged
                        && u.is_infantry
                        && u.can_move
                        && u.can_request_service
                })
            } else {
                false
            };
            if any_injured_infantry {
                return Some(CommandType::GetHealed { target_id });
            }
        }
        // C++ CommandXlat.cpp:1856-1918 — hijack / carbomb / sabotage before
        // enter (:1941) and attack (:1962). C++ emits MSG_ENTER; live Enter is
        // garrison, so issue the dedicated executor commands instead.
        if !hint.sold && hint.is_alive && !hint.under_construction {
            if selection_can_hijack_target(selected_presentation, units, hint, game_logic) {
                return Some(CommandType::Hijack { target_id });
            }
            if selection_can_convert_to_carbomb_target(
                selected_presentation,
                units,
                hint,
                game_logic,
            ) {
                return Some(CommandType::ConvertToCarbomb { target_id });
            }
            if selection_can_sabotage_target(selected_presentation, units, hint, game_logic) {
                return Some(CommandType::Sabotage { target_id });
            }
        }
        // C++ CommandXlat.cpp:1921-1937 canSelectionSalvage → MSG_DO_SALVAGE.
        if hint.is_salvage_crate && !hint.sold {
            let any_salvager = if !selected_presentation.is_empty() {
                selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.is_salvager && u.can_move)
            } else {
                game_logic.is_some_and(|gl| {
                    units.iter().any(|&unit_id| {
                        gl.unit_is_alive(unit_id)
                            && gl.unit_can_move(unit_id)
                            && gl.unit_is_kind_of(unit_id, KindOf::Salvager)
                    })
                })
            };
            if any_salvager {
                return Some(CommandType::DoSalvage {
                    destination: world_position,
                });
            }
        }
        // C++ ActionManager.cpp:552-560 — infantry + DISABLED_UNMANNED +
        // !REJECT_UNMANNED returns TRUE before contain/capacity.
        if hint.is_unmanned
            && !hint.sold
            && !hint.under_construction
            && !hint.enter_disabled_subdued
        {
            let any_recrew = if !selected_presentation.is_empty() {
                selected_presentation.iter().any(|u| {
                    u.is_alive
                        && u.can_move
                        && u.is_infantry
                        && !crate::game_logic::host_car_bomb::object_definition_has_kind(
                            &u.template_name,
                            "REJECT_UNMANNED",
                        )
                })
            } else {
                game_logic.is_some_and(|gl| {
                    units
                        .iter()
                        .copied()
                        .any(|id| gl.can_execute_infantry_unmanned_recrew(id, target_id))
                })
            };
            if any_recrew {
                return Some(CommandType::Enter { target_id });
            }
        }
        // C++ `CommandXlat::translateMouseButton`: normal Enter is evaluated
        // before attack.  A non-owner empty *or stealth-garrison-only*
        // non-faction garrison remains a valid C++ `canEnterObject` target.
        // Arrival `onCollide` then ejects those STEALTH_GARRISON occupants.

        // Wave 1099: sold residual fail-closed on enter target.
        if hint.can_be_entered
            && !hint.sold
            && !hint.under_construction
            && !hint.enter_disabled_subdued
        {
            let any_mobile = if !selected_presentation.is_empty() {
                selected_presentation.iter().any(|u| {
                    u.is_alive
                        && u.can_move
                        && u.transport_slot_count > 0
                        && (!hint.enter_uses_transport_slots
                            || u.transport_slot_count <= hint.enter_available_capacity)
                        && (!hint.enter_requires_infantry || u.is_infantry)
                        && (!hint.enter_forbids_aircraft || !u.is_aircraft)
                        && (!hint.enter_is_rider_change
                            || hint
                                .rider_change_allowed_templates
                                .iter()
                                .any(|template| template.eq_ignore_ascii_case(&u.template_name)))
                })
            } else {
                // Wave 244: boot residual via unit probes (no get_object dual-read).
                game_logic.is_some_and(|gl| {
                    units
                        .iter()
                        .copied()
                        .any(|id| gl.can_unit_enter_normal_target(id, target_id))
                })
            };
            if any_mobile {
                return Some(CommandType::Enter { target_id });
            }
        }
        // Attack: C++ WeaponSet.cpp:529-543 getCanAttackObject — player
        // attacks need ENEMIES (mine/force exceptions). Wave 1098: sold
        // residual fail-closed (presentation_target_hint also peels).
        if !hint.sold && any_attacker() {
            let legal = if !selected_presentation.is_empty() {
                selected_presentation.iter().any(|u| {
                    if !u.is_alive || !u.can_attack {
                        return false;
                    }
                    if hint.is_friendly_of_local {
                        return false;
                    }
                    if u.is_worker {
                        return hint.is_mine;
                    }
                    // C++ WeaponSet.cpp:529-543 — CMD_FROM_PLAYER non-force
                    // attack requires ENEMIES or a non-allied mine. hq-5fhuz
                    // (closed): RMB attack stays weapon-legal for neutral
                    // tech/civilian targets (oil derrick, civilian car, angry
                    // mob); only ordinary/salvage crates fall through to the
                    // move-to-crate mapping below.
                    hint.is_enemy_of_local
                        || hint.is_mine
                        || (!hint.is_crate && !hint.is_salvage_crate && !hint.is_friendly_of_local)
                })
            } else {
                game_logic.is_some_and(|gl| self.can_attack_target(units, target_id, gl))
            };
            if legal {
                return Some(CommandType::AttackObject { target_id });
            }
        }
        // C++ CommandXlat tries ordinary attack before CaptureBuilding.  The
        // frozen target carries the full non-packed capture semantic so this
        // path never falls back to a faction/unit basename.
        let capture_relation_allows =
            hint.is_enemy_of_local || (hint.capturable && !hint.is_friendly_of_local);
        let capture_garrison_clear = (!hint.capture_garrisonable
            || hint.capture_nonstealthed_garrison_count == 0)
            && hint.capture_friendly_garrison_count == 0;
        if hint.is_structure
            && !hint.under_construction
            && !hint.sold
            && !hint.immune_to_capture
            && !hint.capture_target_effectively_stealthed
            && capture_relation_allows
            && capture_garrison_clear
            && any_capturer()
        {
            return Some(CommandType::CaptureBuilding { target_id });
        }
        // C++ CommandXlat.cpp:2050-2156 — Lotus/Hacker auto-hack after capture.
        if selection_can_disable_vehicle_hack_target(selected_presentation, units, hint, game_logic)
        {
            return Some(CommandType::DisableVehicleHack { target_id });
        }
        if selection_can_steal_cash_hack_target(selected_presentation, units, hint, game_logic) {
            return Some(CommandType::StealCashHack { target_id });
        }
        if selection_can_disable_building_hack_target(
            selected_presentation,
            units,
            hint,
            game_logic,
        ) {
            return Some(CommandType::HackerDisableBuilding { target_id });
        }
        // Ordinary crates (money/heal/shroud/unit): C++ crate click is move-to-crate.
        if hint.is_crate && !hint.is_salvage_crate && !hint.sold {
            let any_mobile = if !selected_presentation.is_empty() {
                selected_presentation
                    .iter()
                    .any(|u| u.is_alive && u.can_move)
            } else {
                game_logic.is_some_and(|gl| {
                    units
                        .iter()
                        .any(|&unit_id| gl.unit_is_alive(unit_id) && gl.unit_can_move(unit_id))
                })
            };
            if any_mobile {
                return Some(CommandType::MoveTo {
                    destination: world_position,
                    waypoints: Vec::new(),
                });
            }
        }
        None
    }

    /// Main boot classifier equivalent of C++ `ActionManager::canDockAt` for
    /// the three dock families currently represented in the host.  This is
    /// deliberately `SELECTION_ALL`: a mixed selection must not turn a dock
    /// command into a partial order.
    fn can_dock_at_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        if units.is_empty()
            || !game_logic.unit_exists(target_id)
            || !game_logic.unit_is_alive(target_id)
            || game_logic.unit_under_construction(target_id)
            || game_logic.unit_is_sold(target_id)
        {
            return false;
        }

        let Some(target) = game_logic.host_object(target_id) else {
            return false;
        };
        let target_kind = target.thing.template.dock_kind;
        // Do not use faction as player authority once even one live object
        // carries owner provenance.  This mirrors the frozen-frame rule and
        // keeps boot/input classification from visibly offering a Dock order
        // which the executor must reject.
        let legacy_ownerless_world = game_logic.uses_legacy_team_ownership_fallback();
        let common_source = |unit_id: ObjectId| {
            unit_id != target_id
                && game_logic.unit_is_alive(unit_id)
                && !game_logic.unit_under_construction(unit_id)
                && game_logic.unit_can_move(unit_id)
                && !game_logic.unit_is_kind_of(unit_id, KindOf::Structure)
        };
        let supply_center_controller_matches = |unit_id: ObjectId| {
            let Some(unit) = game_logic.host_object(unit_id) else {
                return false;
            };
            match (unit.owner_player_id, target.owner_player_id) {
                (Some(unit_owner), Some(target_owner)) => unit_owner == target_owner,
                (None, None) => {
                    legacy_ownerless_world
                        && unit.team == target.team
                        && game_logic.unique_player_id_for_team(unit.team).is_some()
                }
                _ => false,
            }
        };
        let warehouse_is_not_enemy = |unit_id: ObjectId| {
            use gamelogic::common::Relationship;

            let Some(unit) = game_logic.host_object(unit_id) else {
                return false;
            };
            match (unit.owner_player_id, target.owner_player_id) {
                (Some(_), Some(_)) => {
                    game_logic.object_relationship(target, unit) != Relationship::Enemies
                }
                // An ownerless map warehouse/source is neutral to a
                // player-owned collector; this is a relationship result, not
                // a faction fallback.
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => {
                    legacy_ownerless_world
                        && (target.team == Team::Neutral
                            || (unit.team == target.team
                                && game_logic.unique_player_id_for_team(unit.team).is_some()))
                }
            }
        };

        match target_kind {
            DockKind::SupplyCenter => units.iter().copied().all(|unit_id| {
                common_source(unit_id)
                    && game_logic.unit_is_resource_collector(unit_id)
                    && game_logic.unit_stored_supplies(unit_id) > 0
                    // C++ compares `getControllingPlayer()` pointers, not
                    // faction or alliance identity.
                    && supply_center_controller_matches(unit_id)
            }),
            DockKind::SupplyWarehouse => {
                if game_logic.unit_stored_supplies(target_id) == 0 {
                    return false;
                }
                units.iter().copied().all(|unit_id| {
                    common_source(unit_id)
                        && game_logic.unit_is_resource_collector(unit_id)
                        // C++ permits a warehouse unless relationship is
                        // ENEMIES.  Owner-aware objects use the exact player
                        // relationship; legacy factions are a narrow fallback.
                        && warehouse_is_not_enemy(unit_id)
                })
            }
            DockKind::RailedTransport => units.iter().copied().all(|unit_id| {
                common_source(unit_id)
                    && (game_logic.unit_is_kind_of(unit_id, KindOf::Vehicle)
                        || game_logic.unit_is_kind_of(unit_id, KindOf::Infantry))
            }),
            DockKind::None => false,
        }
    }

    /// Presentation-only Dock classifier for physical RMB input.  Fields are
    /// frozen by `PresentationFrame`; missing selected data fails closed rather
    /// than reaching into live GameLogic while the renderer owns the snapshot.
    fn can_dock_at_target_from_presentation(
        &self,
        units: &[ObjectId],
        selected_presentation: &[PresentationSelectedUnitHint],
        hint: &PresentationTargetHint,
    ) -> bool {
        if units.is_empty()
            || selected_presentation.len() != units.len()
            || !units
                .iter()
                .all(|id| selected_presentation.iter().any(|unit| unit.id == *id))
            || hint.sold
            || hint.under_construction
        {
            return false;
        }

        let common_source = |unit: &PresentationSelectedUnitHint| {
            unit.id != hint.id && unit.is_alive && unit.can_move
        };
        match hint.dock_kind {
            DockKind::SupplyCenter => {
                // C++ SupplyCenterDockUpdate compares controlling players,
                // not faction/alliance.  Both sides of this fact are frozen:
                // old snapshots only grant it through PresentationFrame's
                // wholly-ownerless, unambiguous legacy fallback.
                hint.dock_controller_is_local
                    && selected_presentation.iter().all(|unit| {
                        common_source(unit)
                            && unit.is_resource_collector
                            && unit.stored_supplies > 0
                            && unit.is_controlled_by_local
                    })
            }
            DockKind::SupplyWarehouse => {
                !hint.is_enemy_of_local
                    && hint.stored_supplies > 0
                    && selected_presentation
                        .iter()
                        .all(|unit| common_source(unit) && unit.is_resource_collector)
            }
            DockKind::RailedTransport => selected_presentation
                .iter()
                .all(|unit| common_source(unit) && (unit.is_vehicle || unit.is_infantry)),
            DockKind::None => false,
        }
    }

    pub(super) fn can_gather_from_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        // Wave 245: target/unit probes (no &Object dual-read).
        if !game_logic.unit_is_alive(target_id) || !game_logic.unit_is_resource_target(target_id) {
            return false;
        }
        for &unit_id in units {
            if game_logic.unit_is_resource_collector(unit_id)
                && game_logic.unit_is_alive(unit_id)
                && game_logic.unit_can_move(unit_id)
                // Supply sources are normally neutral.  Ownership of the
                // selected collector is enforced by the command executor;
                // this presentation-less classifier must not reject that
                // valid neutral target by comparing the two teams.
                && game_logic.unit_team(unit_id).is_some_and(|team| team != Team::Neutral)
            {
                return true;
            }
        }
        false
    }

    /// Validate if selected dozers can resume construction on unfinished structure.
    pub(super) fn can_resume_construction(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        // Wave 245: target/unit probes (no &Object dual-read).
        if !game_logic.unit_is_kind_of(target_id, crate::game_logic::KindOf::Structure)
            || !game_logic.unit_is_alive(target_id)
            || !game_logic.unit_under_construction(target_id)
        {
            return false;
        }
        let Some(target_team) = game_logic.unit_team(target_id) else {
            return false;
        };
        for &unit_id in units {
            if game_logic.unit_can_repair(unit_id)
                && game_logic.unit_is_alive(unit_id)
                && game_logic.unit_can_move(unit_id)
                && game_logic
                    .unit_team(unit_id)
                    .is_some_and(|t| t == target_team || target_team == Team::Neutral)
            {
                return true;
            }
        }
        false
    }

    /// Validate if selected units can repair target
    pub(super) fn can_repair_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        // Wave 245: target/unit probes (no &Object dual-read).
        if !game_logic.unit_is_kind_of(target_id, crate::game_logic::KindOf::Structure)
            || !game_logic.unit_is_alive(target_id)
            || game_logic.unit_under_construction(target_id)
            || !game_logic.unit_needs_service(target_id)
        {
            return false;
        }
        for &unit_id in units {
            if game_logic.unit_can_repair(unit_id)
                && game_logic.repair_relationship_is_not_enemy(unit_id, target_id)
            {
                return true;
            }
        }
        false
    }

    /// Validate if selected units can enter target
    pub(super) fn can_enter_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        units
            .iter()
            .copied()
            .any(|unit_id| game_logic.can_unit_enter_normal_target(unit_id, target_id))
    }

    /// Validate if selected units can get services at target
    pub(super) fn can_get_serviced_at_target(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &GameLogic,
    ) -> bool {
        // Wave 245: target/unit probes (no &Object dual-read).
        if !game_logic.unit_is_alive(target_id)
            || game_logic.unit_under_construction(target_id)
            || game_logic.unit_is_sold(target_id)
            || game_logic.unit_is_contained(target_id)
        {
            return false;
        }

        // `ActionManager::canGetRepairedAt` / `canGetHealedAt` use authored
        // KindOf bits, not a host BuildingType inferred from a basename.
        let target_is_heal_pad = game_logic.unit_is_kind_of(target_id, KindOf::HealPad);
        let target_is_repair_pad = game_logic.unit_is_kind_of(target_id, KindOf::RepairPad);
        let target_is_fs_airfield = game_logic.unit_is_kind_of(target_id, KindOf::FSAirfield);

        for &unit_id in units {
            if !game_logic.service_relationship_is_allies(unit_id, target_id)
                || !game_logic.unit_is_alive(unit_id)
                || !game_logic.unit_can_move(unit_id)
                || game_logic.unit_under_construction(unit_id)
                || game_logic.unit_is_contained(unit_id)
                || !game_logic.unit_needs_service(unit_id)
            {
                continue;
            }

            let is_aircraft = game_logic.unit_is_kind_of(unit_id, KindOf::Aircraft);
            let is_vehicle = game_logic.unit_is_kind_of(unit_id, KindOf::Vehicle);
            let can_use_service = (target_is_heal_pad
                && game_logic.unit_is_kind_of(unit_id, KindOf::Infantry))
                || (is_vehicle
                    && ((is_aircraft && target_is_fs_airfield)
                        || (!is_aircraft && target_is_repair_pad))
                    && (!is_aircraft || game_logic.unit_is_above_terrain(unit_id)));

            if can_use_service {
                return true;
            }
        }
        false
    }

    /// Get current selected units for a player
    pub fn get_selected_units(&self, player_id: u32, game_logic: &GameLogic) -> Vec<ObjectId> {
        // Wave 242: selection via player_selected_objects probe.
        game_logic.player_selected_objects(player_id)
    }

    /// Clear command queue
    pub fn clear_queue(&mut self) {
        self.command_queue.clear();
    }

    /// Get command history
    pub fn get_command_history(&self) -> &[GameCommand] {
        &self.command_history
    }
}

fn selected_unit_template_names(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    game_logic: Option<&GameLogic>,
) -> Vec<String> {
    if !selected_presentation.is_empty() {
        return selected_presentation
            .iter()
            .filter(|u| u.is_alive)
            .map(|u| u.template_name.clone())
            .collect();
    }
    game_logic
        .map(|gl| {
            units
                .iter()
                .copied()
                .filter(|&id| gl.unit_is_alive(id))
                .filter_map(|id| gl.unit_template_name(id))
                .collect()
        })
        .unwrap_or_default()
}

fn is_hijacker_unit_template(name: &str) -> bool {
    name.to_ascii_lowercase().contains("hijacker")
}

fn target_is_hijackable_vehicle(hint: &PresentationTargetHint) -> bool {
    hint.is_alive
        && !hint.sold
        && hint.is_enemy_of_local
        && hint.is_vehicle
        && !hint.is_aircraft
        && !hint.is_drone
}

fn target_is_carbombable_vehicle(hint: &PresentationTargetHint) -> bool {
    // C++ ActionManager.cpp:794-825 delegates to ConvertToCarBombCrateCollide
    // wouldLikeToCollideWith == isValidToExecute
    // (ConvertToCarBombCrateCollide.cpp:36-76): the click is only a carbomb
    // conversion when the TARGET VEHICLE authors a WEAPONSET_CARBOMB
    // weapon-template set; already-bombs are rejected.  Without the weapon-set
    // probe every Terrorist RMB on an own Combat Bike would steal the click
    // from RiderChange/Enter (the retail bike authors no CARBOMB set).
    hint.is_alive
        && !hint.sold
        && hint.is_vehicle
        && !hint.is_aircraft
        && !hint.is_carbomb
        && crate::game_logic::host_car_bomb::template_has_carbomb_weapon_set(
            &hint.template_name,
        )
}

fn selection_can_hijack_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !target_is_hijackable_vehicle(hint) {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| is_hijacker_unit_template(name))
}

fn selection_can_convert_to_carbomb_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !target_is_carbombable_vehicle(hint) {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| crate::game_logic::is_terrorist_template(name))
}

fn selection_can_sabotage_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !hint.is_alive || !hint.is_enemy_of_local || !hint.is_structure || hint.sold {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| crate::game_logic::is_saboteur_template(name))
}

fn selection_can_override_special_power_destination(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    game_logic: Option<&GameLogic>,
) -> bool {
    if selected_presentation
        .iter()
        .any(|u| u.is_alive && u.can_override_special_power_destination)
    {
        return true;
    }
    game_logic.is_some_and(|gl| {
        units.iter().copied().any(|id| {
            gl.host_object(id)
                .is_some_and(|o| o.is_alive() && o.special_power_override_destination.is_some())
        })
    })
}

fn selection_can_disable_vehicle_hack_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !hint.is_alive
        || hint.sold
        || !hint.is_enemy_of_local
        || !hint.is_vehicle
        || hint.is_aircraft
    {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| crate::game_logic::host_hero_abilities::is_black_lotus_template(name))
}

fn selection_can_steal_cash_hack_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !hint.is_alive
        || hint.sold
        || !hint.is_enemy_of_local
        || !hint.is_structure
        || hint.under_construction
        || !crate::game_logic::host_hero_abilities::is_cash_hack_target_template(
            &hint.template_name,
        )
    {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| crate::game_logic::host_hero_abilities::is_black_lotus_template(name))
}

fn selection_can_disable_building_hack_target(
    selected_presentation: &[PresentationSelectedUnitHint],
    units: &[ObjectId],
    hint: &PresentationTargetHint,
    game_logic: Option<&GameLogic>,
) -> bool {
    if !hint.is_alive
        || hint.sold
        || !hint.is_enemy_of_local
        || !hint.is_structure
        || hint.under_construction
    {
        return false;
    }
    selected_unit_template_names(selected_presentation, units, game_logic)
        .iter()
        .any(|name| crate::game_logic::host_hacker_income::is_hacker_template(name))
}
