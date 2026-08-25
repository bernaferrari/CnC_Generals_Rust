use super::*;
use gamelogic::player::ThePlayerList;

/// Command Translator - converts raw input into game commands
pub struct CommandTranslator {
    // State for tracking mouse operations
    pub(super) mouse_down_position: Option<ICoord2D>,
    pub(super) drag_threshold: i32,
    pub(super) mouse_down_modifiers: u32,
    pub(super) right_click_anchor: Option<ICoord2D>,
    pub(super) right_click_lift: Option<ICoord2D>,
    pub(super) right_click_down_time: u32,
    pub(super) right_click_up_time: u32,

    // State for selection operations
    pub(super) current_selection: HashSet<ObjectID>,
    pub(super) selection_anchor: Option<ICoord2D>,

    // Mode flags
    pub(super) force_attack_mode: bool,
    pub(super) force_move_mode: bool,
    pub(super) waypoint_mode: bool,
    pub(super) path_build_mode: bool,
    pub(super) prefer_selection_mode: bool,
}

impl CommandTranslator {
    pub fn new() -> Self {
        Self {
            mouse_down_position: None,
            drag_threshold: 5, // pixels
            mouse_down_modifiers: 0,
            right_click_anchor: None,
            right_click_lift: None,
            right_click_down_time: 0,
            right_click_up_time: 0,
            current_selection: HashSet::new(),
            selection_anchor: None,
            force_attack_mode: false,
            force_move_mode: false,
            waypoint_mode: false,
            path_build_mode: false,
            prefer_selection_mode: false,
        }
    }

    pub(super) fn max_select_count() -> i32 {
        TheInGameUI::get_max_select_count()
    }

    pub(super) fn selection_limit_reached(&self) -> bool {
        Self::selection_count_limit_reached_for(
            Self::max_select_count(),
            self.current_selection.len(),
        )
    }

    pub(super) fn selection_count_limit_reached(count: usize) -> bool {
        Self::selection_count_limit_reached_for(Self::max_select_count(), count)
    }

    pub(super) fn selection_count_limit_reached_for(max: i32, count: usize) -> bool {
        max > 0 && count >= max as usize
    }

    /// Evaluate a context-sensitive command against the current selection state.
    ///
    /// This keeps the command translator itself as the source of truth for context evaluation,
    /// matching the C++ `GameClient` hookup where the registered command translator is also the
    /// object consulted by context selection logic.
    pub fn evaluate_context_command(
        &mut self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: ClientCommandEvaluateType,
    ) -> GameMessageResult<GameMessageType> {
        self.sync_selection_from_logic();

        // C++ parity: "null out draw/obj" forces position-based evaluation.
        let mut evaluate_as_position = false;

        if let Some(obj_id) = drawable.get_object_id() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(guard) = obj.read() {
                    let is_masked = guard
                        .get_status_bits()
                        .contains(LogicObjectStatusMaskType::MASKED);
                    if is_masked
                        && !guard.is_kind_of(KindOf::Shrubbery)
                        && !guard.is_kind_of(KindOf::ForceAttackable)
                    {
                        evaluate_as_position = true;
                    }

                    if !evaluate_as_position
                        && guard.is_kind_of(KindOf::Mine)
                        && guard.is_locally_controlled()
                    {
                        evaluate_as_position = true;
                    }

                    if !evaluate_as_position
                        && guard.is_locally_controlled()
                        && TheInGameUI::is_in_prefer_selection_mode()
                    {
                        return Ok(GameMessageType::Invalid);
                    }
                }
            }
        } else {
            evaluate_as_position = true;
        }

        if self.force_move_mode || TheInGameUI::is_in_force_move_to_mode() {
            evaluate_as_position = true;
        }

        let result = match cmd_type {
            ClientCommandEvaluateType::Context
            | ClientCommandEvaluateType::Primary
            | ClientCommandEvaluateType::Secondary => {
                if evaluate_as_position || drawable.get_object_id().is_none() {
                    self.handle_mouseover_location_hint(position)
                        .into_iter()
                        .next()
                        .unwrap_or(GameMessageType::Invalid)
                } else {
                    self.handle_mouseover_drawable_hint(drawable.get_id().0)
                        .into_iter()
                        .next()
                        .unwrap_or(GameMessageType::Invalid)
                }
            }
        };

        Ok(result)
    }

    /// Process mouse button down events
    pub(super) fn handle_mouse_button_down(
        &mut self,
        position: &ICoord2D,
        button: MouseButton,
        modifiers: u32,
        time: u32,
    ) -> Vec<GameMessageType> {
        debug!("Mouse button {:?} down at {:?}", button, position);

        match button {
            MouseButton::Left => {
                self.mouse_down_position = Some(position.clone());
                self.selection_anchor = Some(position.clone());
                self.mouse_down_modifiers = modifiers;
                vec![]
            }
            MouseButton::Right => {
                // Mirrors C++ right-button click bookkeeping used by click/drag gating.
                self.right_click_anchor = Some(position.clone());
                self.right_click_down_time = time;
                vec![]
            }
            MouseButton::Middle => {
                vec![]
            }
        }
    }

    pub(super) fn sync_selection_from_logic(&mut self) {
        let local_player = get_local_player_id();
        if local_player < 0 {
            return;
        }

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return;
        };

        let Some(selection) = manager.get_player_selection_ref(local_player) else {
            return;
        };

        self.current_selection.clear();
        self.current_selection
            .extend(selection.get_selected_objects());
    }

    pub(super) fn clear_targeting_modes(&mut self) {
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_force_attack_mode(false);
        TheInGameUI::set_force_move_to_mode(false);
        TheInGameUI::set_prefer_selection_mode(false);
        self.force_attack_mode = false;
        self.force_move_mode = false;
        self.prefer_selection_mode = false;
    }

    pub(super) fn pick_context_target(
        &self,
        region: &IRegion2D,
        local_player: Option<u32>,
    ) -> Option<ObjectID> {
        pub(super) const PICK_RADIUS_WORLD: f32 = 10.0;
        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        let profile = context_pick_profile(force_attack_active, &self.current_selection);
        let (mut mine, mut other) =
            collect_selectable_objects(region, true, PICK_RADIUS_WORLD, local_player, profile);
        let mine_pick = pick_closest(&mut mine);
        let other_pick = pick_closest(&mut other);

        match (mine_pick, other_pick) {
            (Some(mine_id), Some(other_id)) => {
                let mine_dist = mine
                    .iter()
                    .find(|(id, _)| *id == mine_id)
                    .map(|(_, d)| *d)
                    .unwrap_or(f32::MAX);
                let other_dist = other
                    .iter()
                    .find(|(id, _)| *id == other_id)
                    .map(|(_, d)| *d)
                    .unwrap_or(f32::MAX);
                if mine_dist <= other_dist {
                    Some(mine_id)
                } else {
                    Some(other_id)
                }
            }
            (Some(id), None) | (None, Some(id)) => Some(id),
            (None, None) => None,
        }
    }

    pub(super) fn resolve_pending_command_click(
        &mut self,
        local_player: i32,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: &Coord3D,
    ) -> Vec<GameMessageType> {
        let Some(pending) = TheInGameUI::get_pending_command() else {
            return Vec::new();
        };

        if let Some(object_id) = target {
            if pending_command_accepts_object(pending.options)
                && pending_command_target_allowed(pending.options, local_player, object_id)
                && pending_command_selection_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                )
            {
                if let Some(message) = pending_command_for_object(&pending, object_id) {
                    play_voice_for_command(self.current_selection.iter().copied(), &message);
                    self.clear_targeting_modes();
                    return vec![message];
                }
            }

            if pending_command_accepts_position(pending.options) {
                if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                    if let Ok(obj_guard) = obj.read() {
                        let position = logic_to_message_coord(obj_guard.get_position());
                        if pending_command_position_valid(
                            &pending,
                            local_player_u32,
                            &self.current_selection,
                            &position,
                            Some(object_id),
                        ) {
                            let messages = pending_command_messages_for_position(
                                &pending,
                                position,
                                &self.current_selection,
                                Some(object_id),
                            );
                            if !messages.is_empty() {
                                play_voice_for_command(
                                    self.current_selection.iter().copied(),
                                    &messages[0],
                                );
                                self.clear_targeting_modes();
                                return messages;
                            }
                        }
                    }
                }
            }
        } else if pending_command_accepts_position(pending.options)
            && pending_command_position_valid(
                &pending,
                local_player_u32,
                &self.current_selection,
                world,
                None,
            )
        {
            let messages = pending_command_messages_for_position(
                &pending,
                world.clone(),
                &self.current_selection,
                None,
            );
            if !messages.is_empty() {
                play_voice_for_command(self.current_selection.iter().copied(), &messages[0]);
                self.clear_targeting_modes();
                return messages;
            }
        }

        if pending_command_accepts_object(pending.options)
            || pending_command_accepts_position(pending.options)
        {
            vec![GameMessageType::InvalidGUICommandHint]
        } else {
            Vec::new()
        }
    }

    pub(super) fn resolve_move_command(&self, world: Coord3D) -> GameMessageType {
        if self.waypoint_mode {
            GameMessageType::AddWaypoint(world)
        } else if TheInGameUI::is_in_attack_move_to_mode() {
            GameMessageType::DoAttackMoveTo(world)
        } else if self.force_move_mode || TheInGameUI::is_in_force_move_to_mode() {
            GameMessageType::DoForceMoveTO(world)
        } else {
            GameMessageType::DoMoveTo(world)
        }
    }

    pub(super) fn resolve_move_hint(&self, world: Coord3D) -> GameMessageType {
        if !selection_has_quick_path_to(&self.current_selection, &world) {
            return GameMessageType::DoInvalidHint;
        }

        if self.waypoint_mode {
            GameMessageType::AddWaypointHint(world)
        } else if TheInGameUI::is_in_attack_move_to_mode() {
            GameMessageType::DoAttackMoveToHint(world)
        } else {
            GameMessageType::DoMoveToHint(world)
        }
    }

    pub(super) fn evaluate_force_attack_command(
        &self,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if let Some(target_id) = target {
            return match selection_force_attack_object_result(
                local_player_u32,
                &self.current_selection,
                target_id,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                    Some(GameMessageType::DoForceAttackObject(target_id))
                }
                // C++ DO_COMMAND force-attack path does not emit invalid hint messages.
                CanAttackResult::InvalidShot | CanAttackResult::NotPossible => None,
            };
        }

        match selection_force_attack_position_result(
            local_player_u32,
            &self.current_selection,
            &world,
        ) {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                Some(GameMessageType::DoForceAttackGround(world))
            }
            CanAttackResult::InvalidShot | CanAttackResult::NotPossible => None,
        }
    }

    pub(super) fn evaluate_force_attack_hint(
        &self,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if let Some(target_id) = target {
            return match selection_force_attack_object_result(
                local_player_u32,
                &self.current_selection,
                target_id,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                    Some(GameMessageType::DoForceAttackObjectHint(target_id))
                }
                CanAttackResult::InvalidShot => Some(GameMessageType::ImpossibleAttackHint),
                CanAttackResult::NotPossible => None,
            };
        }

        match selection_force_attack_position_result(
            local_player_u32,
            &self.current_selection,
            &world,
        ) {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                Some(GameMessageType::DoForceAttackGroundHint(world))
            }
            CanAttackResult::InvalidShot => Some(GameMessageType::ImpossibleAttackHint),
            CanAttackResult::NotPossible => None,
        }
    }

    pub(super) fn try_double_click_guard_command(
        &self,
        region: &IRegion2D,
        right_click: bool,
    ) -> Option<GameMessageType> {
        if region.width != 0 || region.height != 0 {
            return None;
        }

        if !is_double_click_attack_move_enabled() {
            return None;
        }

        let alternate_mouse = is_alternate_mouse_enabled();
        let should_issue_guard = if right_click {
            alternate_mouse
        } else {
            !alternate_mouse
        };
        if !should_issue_guard {
            return None;
        }

        let click_pos = ICoord2D::new(region.x, region.y);
        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        Self::record_double_click_attack_move_order_given();
        TheInGameUI::trigger_double_click_attack_move_guard_hint();
        Some(GameMessageType::DoGuardPosition(world, 0))
    }

    fn record_double_click_attack_move_order_given() {
        let Ok(list) = ThePlayerList().read() else {
            return;
        };
        let Some(player) = list.get_local_player() else {
            return;
        };
        if let Ok(mut player) = player.write() {
            player
                .get_academy_stats_mut()
                .record_double_click_attack_move_order_given();
        }
    }

    pub(super) fn evaluate_context_action(
        &self,
        _local_player: i32,
        local_player_u32: Option<u32>,
        target_id: ObjectID,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return Some(GameMessageType::DoSpecialPowerOverrideDestination(
                world,
                SPECIAL_POWER_INVALID,
                gamelogic::common::INVALID_ID,
            ));
        }

        if selection_can_resume_construction_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ResumeConstruction(target_id));
        }

        if selection_can_dock_at_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Dock(target_id));
        }

        if selection_can_repair_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DoRepair(target_id));
        }

        if selection_can_get_repaired_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetRepaired(target_id));
        }

        if selection_can_get_healed_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetHealed(target_id));
        }

        if selection_can_hijack_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        if selection_can_convert_to_carbomb_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ConvertToCarbomb(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_sabotage_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        if let Some(dest) =
            selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoMoveTo(dest));
        }

        if let Some(dest) =
            selection_can_salvage_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoSalvage(dest));
        }

        if selection_can_enter_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        let attack_result =
            selection_attack_result(local_player_u32, &self.current_selection, target_id);
        match attack_result {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                return Some(GameMessageType::DoAttackObject(target_id));
            }
            // C++ evaluateContextCommand emits MSG_IMPOSSIBLE_ATTACK_HINT for invalid shots.
            CanAttackResult::InvalidShot => return Some(GameMessageType::ImpossibleAttackHint),
            CanAttackResult::NotPossible => {}
        }

        if selection_can_capture_building_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::CaptureBuilding(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_disable_vehicle_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::DisableVehicleHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_steal_cash_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::StealCashHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_disable_building_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::DisableBuildingHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if let Some(dest) =
            selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoMoveTo(dest));
        }

        None
    }

    pub(super) fn evaluate_context_hint(
        &self,
        _local_player: i32,
        local_player_u32: Option<u32>,
        target_id: ObjectID,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return Some(GameMessageType::DoSpecialPowerOverrideDestinationHint(
                world,
            ));
        }

        let attack_result =
            selection_attack_result(local_player_u32, &self.current_selection, target_id);

        if selection_can_resume_construction_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ResumeConstructionHint(target_id));
        }

        if selection_can_dock_at_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DockHint(target_id));
        }

        if selection_can_repair_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DoRepairHint(target_id));
        }

        if selection_can_get_repaired_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetRepairedHint(target_id));
        }

        if selection_can_get_healed_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetHealedHint(target_id));
        }

        if selection_can_hijack_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::HijackHint(target_id));
        }

        if selection_can_convert_to_carbomb_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ConvertToCarbombHint(target_id));
        }

        if selection_can_sabotage_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::SabotageHint(target_id));
        }

        if selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
            .is_some()
        {
            return Some(self.resolve_move_hint(world));
        }

        if let Some(dest) =
            selection_can_salvage_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoSalvageHint(dest));
        }

        if selection_can_enter_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::EnterHint(target_id));
        }

        match attack_result {
            CanAttackResult::Possible => {
                return Some(GameMessageType::DoAttackObjectHint(target_id));
            }
            CanAttackResult::PossibleAfterMoving => {
                return Some(GameMessageType::DoAttackObjectAfterMovingHint(target_id));
            }
            CanAttackResult::InvalidShot | CanAttackResult::NotPossible => {}
        }

        if selection_can_capture_building_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::CaptureBuildingHint(target_id));
        }

        if selection_can_disable_vehicle_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) || selection_can_steal_cash_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) || selection_can_disable_building_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::HackHint(target_id));
        }

        if attack_result == CanAttackResult::InvalidShot {
            return Some(GameMessageType::ImpossibleAttackHint);
        }

        None
    }

    pub(super) fn handle_point_click(
        &mut self,
        region: &IRegion2D,
        right_click: bool,
    ) -> Vec<GameMessageType> {
        if region.width != 0 || region.height != 0 {
            return Vec::new();
        }

        if right_click && is_alternate_mouse_enabled() && !self.right_click_is_click_gesture() {
            return Vec::new();
        }

        let click_pos = ICoord2D::new(region.x, region.y);
        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let alternate_mouse = is_alternate_mouse_enabled();
        let pending_command_active = TheInGameUI::get_pending_command().is_some();
        let target = self
            .pick_context_target(region, local_player_u32)
            .filter(|object_id| !is_locally_controlled_mine_target(*object_id));

        // In C++, right-click in alternate mouse mode still evaluates context/pending
        // commands; non-alternate right-click cancels pending targeting.
        if right_click && pending_command_active && !alternate_mouse {
            self.clear_targeting_modes();
            TheInGameUI::clear_attack_move_to_mode();
            return Vec::new();
        }

        if !point_click_is_actionable(right_click, alternate_mouse, pending_command_active) {
            return Vec::new();
        }

        // C++ pending GUI command execution happens on left-click paths; right-click
        // is used to cancel GUI mode by SelectionXlat.
        let should_resolve_pending = pending_command_active && !right_click;
        if should_resolve_pending {
            let messages =
                self.resolve_pending_command_click(local_player, local_player_u32, target, &world);
            if !messages.is_empty() {
                TheInGameUI::clear_attack_move_to_mode();
                return messages;
            }
            // Targeting mode stays active until fulfilled/cancelled.
            return Vec::new();
        }

        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        let command = if force_attack_active {
            self.evaluate_force_attack_command(local_player_u32, target, world.clone())
        } else if let Some(target_id) = target {
            self.evaluate_context_action(local_player, local_player_u32, target_id, world.clone())
        } else if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            Some(GameMessageType::DoSpecialPowerOverrideDestination(
                world.clone(),
                SPECIAL_POWER_INVALID,
                gamelogic::common::INVALID_ID,
            ))
        } else if selection_can_set_rally_point(&self.current_selection) {
            None
        } else {
            Some(self.resolve_move_command(world.clone()))
        };

        if command.is_none()
            && target.is_none()
            && !force_attack_active
            && selection_can_set_rally_point(&self.current_selection)
        {
            let mut messages = Vec::new();
            let mut ids: Vec<ObjectID> = self.current_selection.iter().copied().collect();
            ids.sort_unstable();
            for id in ids {
                messages.push(GameMessageType::SetRallyPoint(id, world.clone()));
            }
            TheInGameUI::clear_attack_move_to_mode();
            return messages;
        }

        TheInGameUI::clear_attack_move_to_mode();

        if command.is_none() {
            if let Some(target_id) = target {
                if !force_attack_active
                    && selection_attack_result(local_player_u32, &self.current_selection, target_id)
                        == CanAttackResult::InvalidShot
                {
                    return vec![GameMessageType::ImpossibleAttackHint];
                }
                return Vec::new();
            }
        }

        if let Some(msg) = command.as_ref() {
            let mut info = VoicePlayInfo {
                air: false,
                target_id: target,
            };
            if let Some(target_id) = target {
                if let Some(target_obj) = OBJECT_REGISTRY.get_object(target_id) {
                    if let Ok(target_guard) = target_obj.read() {
                        info.air = target_guard.is_using_airborne_locomotor();
                    }
                }
            }
            pick_and_play_unit_voice_response(self.current_selection.iter().copied(), msg, &info);
        }

        command.map(|msg| vec![msg]).unwrap_or_default()
    }

    pub(super) fn handle_mouseover_location_hint(&self, pos: &Coord3D) -> Vec<GameMessageType> {
        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };

        if let Some(pending) = TheInGameUI::get_pending_command() {
            if pending_command_accepts_position(pending.options) {
                if !pending_command_position_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    pos,
                    None,
                ) {
                    return vec![GameMessageType::InvalidGUICommandHint];
                }
                if let Some(hint) = pending_command_hint_for_position(&pending, pos.clone()) {
                    return vec![hint];
                }
                return vec![GameMessageType::ValidGUICommandHint];
            }
            if pending_command_accepts_object(pending.options) {
                return vec![GameMessageType::InvalidGUICommandHint];
            }
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        if force_attack_active {
            return self
                .evaluate_force_attack_hint(local_player_u32, None, pos.clone())
                .map(|hint| vec![hint])
                .unwrap_or_default();
        }

        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return vec![GameMessageType::DoSpecialPowerOverrideDestinationHint(
                pos.clone(),
            )];
        }

        if selection_can_set_rally_point(&self.current_selection) {
            return vec![GameMessageType::SetRallyPointHint(pos.clone())];
        }

        vec![self.resolve_move_hint(pos.clone())]
    }

    pub(super) fn handle_mouseover_drawable_hint(
        &self,
        drawable: DrawableID,
    ) -> Vec<GameMessageType> {
        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let target_id = drawable as ObjectID;
        let world = OBJECT_REGISTRY
            .get_object(target_id)
            .and_then(|obj| {
                obj.read()
                    .ok()
                    .map(|guard| logic_to_message_coord(guard.get_position()))
            })
            .unwrap_or_default();

        // C++ evaluateContextCommand treats locally controlled mines as position
        // interactions instead of object-target interactions.
        if is_locally_controlled_mine_target(target_id) {
            return self.handle_mouseover_location_hint(&world);
        }

        if let Some(pending) = TheInGameUI::get_pending_command() {
            if pending_command_accepts_object(pending.options) {
                if pending_command_target_allowed(pending.options, local_player, target_id)
                    && pending_command_selection_valid(
                        &pending,
                        local_player_u32,
                        &self.current_selection,
                        target_id,
                    )
                {
                    // C++ GUI context-command hover uses generic valid/invalid GUI
                    // command hints rather than per-command hint message variants.
                    return vec![GameMessageType::ValidGUICommandHint];
                }
                return vec![GameMessageType::InvalidGUICommandHint];
            }

            if pending_command_accepts_position(pending.options) {
                if !pending_command_position_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    &world,
                    Some(target_id),
                ) {
                    return vec![GameMessageType::InvalidGUICommandHint];
                }
                if let Some(hint) = pending_command_hint_for_position(&pending, world.clone()) {
                    return vec![hint];
                }
                return vec![GameMessageType::ValidGUICommandHint];
            }
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        if force_attack_active {
            return self
                .evaluate_force_attack_hint(local_player_u32, Some(target_id), world)
                .map(|hint| vec![hint])
                .unwrap_or_default();
        }

        if let Some(hint) =
            self.evaluate_context_hint(local_player, local_player_u32, target_id, world.clone())
        {
            return vec![hint];
        }

        vec![self.resolve_move_hint(world)]
    }

    /// Process mouse button up events
    pub(super) fn handle_mouse_button_up(
        &mut self,
        position: &ICoord2D,
        button: MouseButton,
        modifiers: u32,
        time: u32,
    ) -> Vec<GameMessageType> {
        debug!("Mouse button {:?} up at {:?}", button, position);

        match button {
            MouseButton::Left => {
                let mut messages = Vec::new();

                if let Some(down_pos) = &self.mouse_down_position {
                    let dx = (position.x - down_pos.x) as f32;
                    let dy = (position.y - down_pos.y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();

                    let key_mods = KeyModifiers::from_bits_truncate(modifiers as u8);
                    if distance < self.drag_threshold as f32 {
                        let region = IRegion2D {
                            x: position.x,
                            y: position.y,
                            width: 0,
                            height: 0,
                        };
                        messages.extend(self.handle_selection_region(&region, key_mods));
                    } else if let Some(anchor) = &self.selection_anchor {
                        let region = build_region(anchor, position);
                        messages.extend(self.handle_selection_region(&region, key_mods));
                    }
                }

                self.mouse_down_position = None;
                self.selection_anchor = None;
                self.mouse_down_modifiers = 0;
                messages
            }
            MouseButton::Right => {
                // C++ raw right-button-up only updates click/drag bookkeeping and does not
                // directly issue command messages; context commands are generated on click events.
                self.right_click_lift = Some(position.clone());
                self.right_click_up_time = time;
                let had_pending_place_source =
                    TheInGameUI::get_pending_place_source_object_id() != 0;
                // C++ parity (CommandXlat.cpp MSG_RAW_MOUSE_RIGHT_BUTTON_UP):
                // right-click click gesture cancels pending build-placement mode.
                if self.right_click_is_click_gesture() {
                    TheInGameUI::place_build_available(None, None);
                    if TheInGameUI::get_pending_command().is_none()
                        && (!is_alternate_mouse_enabled() || had_pending_place_source)
                        && !self.current_selection.is_empty()
                    {
                        self.current_selection.clear();
                        return vec![GameMessageType::CreateSelectedGroup(true, Vec::new())];
                    }
                }
                vec![]
            }
            MouseButton::Middle => {
                vec![]
            }
        }
    }

    pub(super) fn right_click_is_click_gesture(&self) -> bool {
        let (Some(anchor), Some(lift)) = (&self.right_click_anchor, &self.right_click_lift) else {
            return false;
        };
        let dx = (anchor.x - lift.x).abs();
        let dy = (anchor.y - lift.y).abs();
        let dt = self
            .right_click_up_time
            .wrapping_sub(self.right_click_down_time);

        // C++ Mouse::isClick parity: movement within drag tolerance and short click duration.
        dx <= self.drag_threshold && dy <= self.drag_threshold && dt <= 250
    }

    pub(super) fn handle_selection_region(
        &mut self,
        region: &IRegion2D,
        modifiers: KeyModifiers,
    ) -> Vec<GameMessageType> {
        pub(super) const PICK_RADIUS_WORLD: f32 = 10.0;

        let is_point = region.width == 0 && region.height == 0;
        let allow_add = modifiers.contains(KeyModifiers::SHIFT) || self.prefer_selection_mode;
        let allow_toggle = modifiers.contains(KeyModifiers::CTRL);

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };

        let (mut mine, mut other) = collect_selectable_objects(
            region,
            is_point,
            PICK_RADIUS_WORLD,
            local_player_u32,
            ContextPickProfile::default(),
        );

        if is_point {
            let picked_object = pick_closest(&mut mine).or_else(|| pick_closest(&mut other));

            if TheInGameUI::get_pending_command().is_some() {
                let world =
                    screen_to_terrain(&ICoord2D::new(region.x, region.y)).unwrap_or(Coord3D {
                        x: region.x as f32,
                        y: region.y as f32,
                        z: 0.0,
                    });
                let messages = self.resolve_pending_command_click(
                    local_player,
                    local_player_u32,
                    picked_object,
                    &world,
                );
                if !messages.is_empty() {
                    return messages;
                }
                // Targeting mode active: ignore selection changes until command is fulfilled/cancelled.
                return Vec::new();
            }

            let Some(object_id) = picked_object else {
                // C++ SelectionXlat leaves blank point clicks in the stream so CommandXlat can
                // issue the terrain/context command for the current selection.
                return Vec::new();
            };

            let (
                current_count_mine,
                current_count_mine_infantry,
                current_count_mine_buildings,
                current_count_other,
            ) = selection_counts(local_player_u32, &self.current_selection);

            // C++ SelectionInfo.cpp: context sensitive selection never applies in force-attack or
            // force-move modes.
            let allow_context = !self.force_attack_mode
                && !self.force_move_mode
                && current_count_other == 0
                && current_count_mine > 0;

            if allow_context {
                // Enemy click becomes an action (typically attack) rather than selecting the enemy.
                if is_enemy_target(local_player, object_id)
                    && selection_can_attack_target(
                        local_player_u32,
                        &self.current_selection,
                        object_id,
                    )
                {
                    return vec![GameMessageType::DoAttackObject(object_id)];
                }

                // Clicking a garrison/transport-capable container with infantry selected issues Enter.
                if current_count_mine_infantry > 0
                    && selection_can_enter_target(
                        local_player_u32,
                        &self.current_selection,
                        object_id,
                    )
                {
                    return vec![GameMessageType::Enter(0, object_id)];
                }

                // Clicking a damaged friendly object with a dozer selected issues DoRepair.
                if selection_can_repair_target(local_player_u32, &self.current_selection, object_id)
                {
                    return vec![GameMessageType::DoRepair(object_id)];
                }

                if selection_can_resume_construction_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::ResumeConstruction(object_id)];
                }

                if selection_can_dock_at_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::Dock(object_id)];
                }

                if let Some(dest) = selection_can_pickup_crate_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::DoMoveTo(dest)];
                }

                // Salvage (hulks): C++ issues MSG_DO_SALVAGE with the target's position.
                if let Some(dest) = selection_can_salvage_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::DoSalvage(dest)];
                }
            }

            // SelectionXlat.cpp: prefer-selection mode appends/removes, but selecting enemies,
            // friends, civilians, or buildings forces a replace selection.
            let mut add_to_group = allow_add;
            if current_count_mine_buildings > 0 || current_count_other > 0 {
                add_to_group = false;
            }

            if allow_toggle {
                if self.current_selection.remove(&object_id) {
                    return vec![GameMessageType::RemoveFromSelectedGroup(vec![object_id])];
                }
                if self.selection_limit_reached() {
                    return Vec::new();
                }
                self.current_selection.insert(object_id);
                return vec![GameMessageType::CreateSelectedGroup(false, vec![object_id])];
            }

            if add_to_group {
                if self.current_selection.contains(&object_id) {
                    self.current_selection.remove(&object_id);
                    return vec![GameMessageType::RemoveFromSelectedGroup(vec![object_id])];
                }
                if self.selection_limit_reached() {
                    return Vec::new();
                }
                self.current_selection.insert(object_id);
                return vec![GameMessageType::CreateSelectedGroup(false, vec![object_id])];
            }

            self.current_selection.clear();
            self.current_selection.insert(object_id);
            return vec![GameMessageType::CreateSelectedGroup(true, vec![object_id])];
        }

        // Region selection: C++ selection prefers locally controlled units; buildings can be
        // selected when no units are selectable in the region.
        mine.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected_ids = Vec::new();
        let mut building_ids = Vec::new();
        for (id, _) in mine.into_iter() {
            if Self::selection_count_limit_reached(selected_ids.len()) {
                break;
            }

            let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };

            if guard.is_kind_of(KindOf::Structure) || guard.is_kind_of(KindOf::Building) {
                building_ids.push(id);
                continue;
            }

            selected_ids.push(id);
        }

        if selected_ids.is_empty() && building_ids.len() == 1 {
            selected_ids.push(building_ids[0]);
        }

        if selected_ids.is_empty() {
            return Vec::new();
        }

        if allow_add {
            let mut new_ids = Vec::new();
            for id in selected_ids {
                if self.selection_limit_reached() {
                    break;
                }
                if self.current_selection.insert(id) {
                    new_ids.push(id);
                }
            }
            if new_ids.is_empty() {
                Vec::new()
            } else {
                vec![GameMessageType::CreateSelectedGroup(false, new_ids)]
            }
        } else {
            self.current_selection.clear();
            self.current_selection.extend(selected_ids.iter().copied());
            vec![GameMessageType::CreateSelectedGroup(true, selected_ids)]
        }
    }

    /// Process keyboard events
    pub(super) fn handle_keyboard(&mut self, key: u32, down: bool) -> Vec<GameMessageType> {
        debug!("Key {} {}", key, if down { "down" } else { "up" });

        let mut messages = Vec::new();

        match key {
            // Meta commands mapped to keys
            0x53 => {
                // 'S' key - stop
                if down {
                    messages.push(GameMessageType::MetaStop);
                }
            }
            0x41 => {
                // 'A' key - attack move
                if down {
                    messages.push(GameMessageType::MetaToggleAttackMove);
                }
            }
            0x47 => {
                // 'G' key - guard
                if down && !self.current_selection.is_empty() {
                    // Guard current position
                    let first = *self.current_selection.iter().next().unwrap();
                    let pos = OBJECT_REGISTRY
                        .get_object(first)
                        .and_then(|obj| {
                            obj.read()
                                .ok()
                                .map(|guard| logic_to_message_coord(guard.get_position()))
                        })
                        .unwrap_or_default();
                    messages.push(GameMessageType::DoGuardPosition(pos, 0));
                }
            }
            0x48 => {
                // 'H' key - halt/stop
                if down {
                    messages.push(GameMessageType::MetaStop);
                }
            }
            0x20 => {
                // Spacebar - scatter
                if down {
                    messages.push(GameMessageType::MetaScatter);
                }
            }
            // Control key modifiers
            0x11 => {
                // Ctrl key
                if down {
                    self.force_attack_mode = true;
                    TheInGameUI::set_force_attack_mode(true);
                    messages.push(GameMessageType::MetaBeginForceAttack);
                } else {
                    self.force_attack_mode = false;
                    TheInGameUI::set_force_attack_mode(false);
                    messages.push(GameMessageType::MetaEndForceAttack);
                }
            }
            0x12 => {
                // Alt key
                if down {
                    self.waypoint_mode = true;
                    messages.push(GameMessageType::MetaBeginWaypoints);
                } else {
                    self.waypoint_mode = false;
                    messages.push(GameMessageType::MetaEndWaypoints);
                }
            }
            0x10 => {
                // Shift key
                if down {
                    self.prefer_selection_mode = true;
                    TheInGameUI::set_prefer_selection_mode(true);
                    messages.push(GameMessageType::MetaBeginPreferSelection);
                } else {
                    self.prefer_selection_mode = false;
                    TheInGameUI::set_prefer_selection_mode(false);
                    messages.push(GameMessageType::MetaEndPreferSelection);
                }
            }
            _ => {}
        }

        messages
    }

    /// Update current selection
    pub(super) fn update_selection(&mut self, objects: HashSet<ObjectID>) {
        debug!("Updating selection with {} objects", objects.len());
        self.current_selection = objects;
    }
}

impl Default for CommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}
