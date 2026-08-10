#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_handler_accepts_area_selection_commands() {
        let handler = SelectionCommandHandler::new();

        assert!(handler.can_handle(CommandType::AreaSelection));
    }

    #[test]
    fn default_handler_accepts_build_line_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DozerConstruct));
        assert!(handler.can_handle(CommandType::DozerConstructLine));
    }

    #[test]
    fn default_handler_accepts_purchase_science_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::PurchaseScience));
    }

    #[test]
    fn default_handler_accepts_sell_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::Sell));
    }

    #[test]
    fn default_handler_accepts_set_rally_point_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::SetRallyPoint));
    }

    #[test]
    fn default_handler_accepts_mine_clearing_detail_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::SetMineClearingDetail));
    }

    #[test]
    fn default_handler_accepts_internet_hack_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::InternetHack));
    }

    #[test]
    fn default_handler_accepts_combat_drop_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::CombatDropAtLocation));
        assert!(handler.can_handle(CommandType::CombatDropAtObject));
    }

    #[test]
    fn default_handler_accepts_execute_railed_transport_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::ExecuteRailedTransport));
    }

    #[test]
    fn default_handler_accepts_exit_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::Exit));
    }

    #[test]
    fn default_handler_accepts_queue_upgrade_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::QueueUpgrade));
    }

    #[test]
    fn default_handler_accepts_cancel_upgrade_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::CancelUpgrade));
    }

    #[test]
    fn default_handler_accepts_queue_unit_create_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::QueueUnitCreate));
    }

    #[test]
    fn default_handler_accepts_cancel_unit_create_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::CancelUnitCreate));
    }

    #[test]
    fn default_handler_accepts_dozer_cancel_construct_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DozerCancelConstruct));
    }

    #[test]
    fn default_handler_accepts_targeted_weapon_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DoWeapon));
        assert!(handler.can_handle(CommandType::DoWeaponAtLocation));
        assert!(handler.can_handle(CommandType::DoWeaponAtObject));
    }

    #[test]
    fn default_handler_accepts_force_attack_ground_commands() {
        let handler = DefaultCommandHandler::new();
        assert!(handler.can_handle(CommandType::DoForceAttackGround));
    }

    #[test]
    fn default_handler_accepts_legacy_replay_and_crc_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DoAttackSquad));
        assert!(handler.can_handle(CommandType::SetReplayCamera));
        assert!(handler.can_handle(CommandType::LogicCrc));
    }

    #[test]
    fn beacon_text_prefers_selected_beacon_positions_over_default_location() {
        let mut manager = BeaconManager::new();
        let selected_position = Coord3D::new(100.0, 50.0, 0.0);
        let default_message_position = Coord3D::ZERO;
        manager.place_beacon(3, selected_position, 10);

        let updated = DefaultCommandHandler::apply_beacon_text_updates(
            &mut manager,
            &[(3, selected_position)],
            Some((3, default_message_position)),
            AsciiString::from("Alpha"),
        );

        assert!(updated);
        let snapshot = manager.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].text.as_deref(), Some("Alpha"));
        assert_eq!(snapshot[0].position, selected_position);
    }

    #[test]
    fn beacon_text_falls_back_to_message_location_without_selection() {
        let mut manager = BeaconManager::new();
        let position = Coord3D::new(25.0, 75.0, 0.0);
        manager.place_beacon(1, position, 10);

        let updated = DefaultCommandHandler::apply_beacon_text_updates(
            &mut manager,
            &[],
            Some((1, position)),
            AsciiString::from("Bravo"),
        );

        assert!(updated);
        assert_eq!(manager.snapshot()[0].text.as_deref(), Some("Bravo"));
    }

    #[test]
    fn override_destination_fallthrough_target_uses_location_x_bits() {
        let mut command = Command::new(CommandType::DoSpecialPowerOverrideDestination);
        command.set_player_index(2);
        command.append_location_argument(Coord3D::new(12.5, -4.0, 9.0));
        command.append_integer_argument(7);
        command.append_object_id_argument(1234);

        let queued = QueuedCommand::new(command, CommandPriority::Normal, 99);

        assert_eq!(
            override_destination_fallthrough_target_id(&queued),
            Some(12.5f32.to_bits())
        );
    }

    #[test]
    fn override_destination_fallthrough_target_ignores_non_override_commands() {
        let command = Command::new(CommandType::DoAttackObject);
        let queued = QueuedCommand::new(command, CommandPriority::Normal, 99);

        assert_eq!(override_destination_fallthrough_target_id(&queued), None);
    }
}
