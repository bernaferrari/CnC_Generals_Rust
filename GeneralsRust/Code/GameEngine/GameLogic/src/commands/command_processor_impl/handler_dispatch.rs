impl CommandHandler for DefaultCommandHandler {
    fn execute_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let start_time = Instant::now();

        // Update statistics
        self.stats.commands_processed += 1;

        let result = match command.command.get_type() {
            CommandType::ClearGameData => self.execute_clear_game_data(),
            CommandType::NewGame => self.execute_new_game(command),
            CommandType::DoMoveTo
            | CommandType::DoAttackMoveTo
            | CommandType::DoForceMoveTo
            | CommandType::AddWaypoint
            | CommandType::DoSalvage => self.execute_move_command(command, context),
            CommandType::DoAttackObject | CommandType::DoForceAttackObject => {
                self.execute_attack_command(command, context)
            }
            CommandType::DoForceAttackGround => self.execute_force_attack_ground(command, context),
            CommandType::Enter => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Enter,
                "enter",
            ),
            CommandType::DoRepair => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Repair,
                "repair",
            ),
            CommandType::Dock => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Dock,
                "dock",
            ),
            CommandType::GetRepaired => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::GetRepaired,
                "get repaired",
            ),
            CommandType::GetHealed => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::GetHealed,
                "get healed",
            ),
            CommandType::ResumeConstruction => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::ResumeConstruction,
                "resume construction",
            ),
            CommandType::DozerConstruct | CommandType::DozerConstructLine => {
                self.execute_build_command(command, context)
            }
            CommandType::Sell => self.execute_sell_command(command, context),
            CommandType::SetRallyPoint => self.execute_set_rally_point(command, context),
            CommandType::SetMineClearingDetail => {
                self.execute_set_mine_clearing_detail(command, context)
            }
            CommandType::DoStop => self.execute_stop_command(command, context),
            CommandType::DoScatter => self.execute_scatter_command(command, context),
            CommandType::DoSpecialPower
            | CommandType::DoSpecialPowerAtLocation
            | CommandType::DoSpecialPowerAtObject
            | CommandType::DoSpecialPowerOverrideDestination => {
                self.execute_special_power(command, context)
            }
            CommandType::Evacuate => self.execute_evacuate_command(context),
            CommandType::Exit => self.execute_exit_command(command, context),
            CommandType::ExecuteRailedTransport => self.execute_selected_ai_command(
                context,
                crate::ai::AiCommandType::ExecuteRailedTransport,
            ),
            CommandType::InternetHack => self.execute_internet_hack_command(context),
            CommandType::CombatDropAtLocation | CommandType::CombatDropAtObject => {
                self.execute_combat_drop_command(command, context)
            }
            CommandType::DoWeapon
            | CommandType::DoWeaponAtLocation
            | CommandType::DoWeaponAtObject => self.execute_weapon_target_command(command, context),
            CommandType::DoGuardPosition => self.execute_guard_position(command, context),
            CommandType::DoGuardObject => self.execute_guard_object(command, context),
            CommandType::DoCheer => self.execute_cheer(command, context),
            CommandType::ToggleOvercharge => self.execute_overcharge_toggle(command, context),
            CommandType::SwitchWeapons => self.execute_switch_weapons(command, context),
            CommandType::ConvertToCarbomb => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Enter,
                "convert to carbomb",
            ),
            CommandType::CaptureBuilding => self.execute_capture_building(command, context),
            CommandType::DisableVehicleHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialBlackLotusDisableVehicleHack,
                |obj, target, source| {
                    TheActionManager::can_disable_vehicle_via_hacking(obj, target, source, true)
                },
                "disable vehicle via hacking",
            ),
            CommandType::StealCashHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialBlackLotusStealCashHack,
                TheActionManager::can_steal_cash_via_hacking,
                "steal cash via hacking",
            ),
            CommandType::DisableBuildingHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialHackerDisableBuilding,
                TheActionManager::can_disable_building_via_hacking,
                "disable building via hacking",
            ),
            CommandType::SnipeVehicle => self.execute_snipe_vehicle(command, context),
            CommandType::EnableRetaliationMode => self.execute_enable_retaliation(command, context),
            CommandType::PurchaseScience => self.execute_purchase_science(command, context),
            CommandType::QueueUpgrade => self.execute_queue_upgrade_command(command, context),
            CommandType::CancelUpgrade => self.execute_cancel_upgrade_command(command, context),
            CommandType::QueueUnitCreate => {
                self.execute_queue_unit_create_command(command, context)
            }
            CommandType::CancelUnitCreate => {
                self.execute_cancel_unit_create_command(command, context)
            }
            CommandType::DozerCancelConstruct => {
                self.execute_dozer_cancel_construct_command(command, context)
            }
            CommandType::CreateFormation => self.execute_create_formation(command, context),
            CommandType::SelfDestruct => self.execute_self_destruct(command, context),
            CommandType::PlaceBeacon => self.execute_place_beacon(command, context),
            CommandType::RemoveBeacon => self.execute_remove_beacon(command, context),
            CommandType::SetBeaconText => self.execute_set_beacon_text(command, context),
            CommandType::ClearInGamePopupMessage => {
                TheInGameUI::request_popup_message_clear();
                CommandExecutionResult::Success
            }
            CommandType::DoAttackSquad | CommandType::SetReplayCamera | CommandType::LogicCrc => {
                CommandExecutionResult::Success
            }
            CommandType::MetaBeginPathBuild => self.execute_begin_path_build(),
            CommandType::MetaEndPathBuild => self.execute_end_path_build(context),
            _ => CommandExecutionResult::Failed(AsciiString::from(&format!(
                "Unhandled command type: {:?}",
                command.command.get_type()
            ))),
        };

        // Update execution time statistics
        let execution_time = start_time.elapsed().as_millis() as f64;
        self.stats.average_execution_time_ms = (self.stats.average_execution_time_ms
            * (self.stats.commands_processed - 1) as f64
            + execution_time)
            / self.stats.commands_processed as f64;

        // Update result statistics
        match &result {
            CommandExecutionResult::Success => self.stats.commands_succeeded += 1,
            CommandExecutionResult::Failed(_)
            | CommandExecutionResult::InvalidCommand
            | CommandExecutionResult::InvalidGameState => self.stats.commands_failed += 1,
            CommandExecutionResult::Deferred => self.stats.commands_deferred += 1,
        }

        result
    }

    fn can_handle(&self, command_type: CommandType) -> bool {
        matches!(
            command_type,
            CommandType::ClearGameData
                | CommandType::NewGame
                | CommandType::DoMoveTo
                | CommandType::DoAttackMoveTo
                | CommandType::DoForceMoveTo
                | CommandType::AddWaypoint
                | CommandType::DoSalvage
                | CommandType::DoAttackObject
                | CommandType::DoForceAttackObject
                | CommandType::DoForceAttackGround
                | CommandType::Enter
                | CommandType::DoRepair
                | CommandType::Dock
                | CommandType::GetRepaired
                | CommandType::GetHealed
                | CommandType::ResumeConstruction
                | CommandType::DozerConstruct
                | CommandType::DozerConstructLine
                | CommandType::Sell
                | CommandType::SetRallyPoint
                | CommandType::SetMineClearingDetail
                | CommandType::DoStop
                | CommandType::DoScatter
                | CommandType::DoSpecialPower
                | CommandType::DoSpecialPowerAtLocation
                | CommandType::DoSpecialPowerAtObject
                | CommandType::DoSpecialPowerOverrideDestination
                | CommandType::Evacuate
                | CommandType::Exit
                | CommandType::ExecuteRailedTransport
                | CommandType::InternetHack
                | CommandType::CombatDropAtLocation
                | CommandType::CombatDropAtObject
                | CommandType::DoWeapon
                | CommandType::DoWeaponAtLocation
                | CommandType::DoWeaponAtObject
                | CommandType::DoGuardPosition
                | CommandType::DoGuardObject
                | CommandType::DoCheer
                | CommandType::ToggleOvercharge
                | CommandType::SwitchWeapons
                | CommandType::ConvertToCarbomb
                | CommandType::CaptureBuilding
                | CommandType::DisableVehicleHack
                | CommandType::StealCashHack
                | CommandType::DisableBuildingHack
                | CommandType::SnipeVehicle
                | CommandType::EnableRetaliationMode
                | CommandType::PurchaseScience
                | CommandType::QueueUpgrade
                | CommandType::CancelUpgrade
                | CommandType::QueueUnitCreate
                | CommandType::CancelUnitCreate
                | CommandType::DozerCancelConstruct
                | CommandType::CreateFormation
                | CommandType::SelfDestruct
                | CommandType::PlaceBeacon
                | CommandType::RemoveBeacon
                | CommandType::SetBeaconText
                | CommandType::ClearInGamePopupMessage
                | CommandType::DoAttackSquad
                | CommandType::SetReplayCamera
                | CommandType::LogicCrc
                | CommandType::MetaBeginPathBuild
                | CommandType::MetaEndPathBuild
        )
    }

    fn get_priority(&self) -> i32 {
        100 // Default priority
    }
}

