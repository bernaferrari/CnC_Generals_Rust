// Exercises the actual populated shortcut path. Core Player/template/CommandSet
// metadata remains an explicit prerequisite; only the completed-upgrade reader
// is receiving-owned in this bounded migration.
#[test]
fn actual_shortcut_population_observes_each_receiving_upgrade_operation() {
    let owner = crate::gui::window_manager::WindowManagerOwner::new();
    owner.with_scope(|| {
        use game_engine::common::ini::{ini_command_button, ini_command_set};
        use game_engine::common::thing::receiving_module_content::{
            UpgradeTooltipObservation, UpgradeTooltipSource,
        };
        use gamelogic::player::PlayerArcExt;
        const TEST: &str = "actual_shortcut_population_observes_each_receiving_upgrade_operation";
        const UPGRADE: &str = "Upgrade_ActualReceivingShortcut";
        const BUTTON: &str = "Command_ActualReceivingShortcut";
        const SET: &str = "ActualReceivingShortcutSet";
        if matches!(
            crate::test_process::run_bounded(module_path!(), TEST),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        // The retained legacy metadata must really resolve this upgrade; otherwise
        // the original optional-template branch would skip NEED_UPGRADE entirely.
        let upgrade = gamelogic::upgrade::center::with_upgrade_center_mut(|center| {
            center.new_upgrade(UPGRADE.into())
        });
        ini_command_button::initialize_control_bar();
        {
            let mut definitions = ini_command_button::get_control_bar_mut().unwrap();
            let button = definitions.new_command_button(BUTTON.to_owned());
            button.command = "STOP".to_owned();
            button.upgrade = UPGRADE.to_owned();
            button.options_bits = CommandOption::NeedUpgrade as u32;
        }
        ini_command_set::initialize_command_set_manager();
        ini_command_set::get_command_set_manager_mut()
            .unwrap()
            .new_command_set(SET.to_owned())
            .set_button_at_position(0, BUTTON.to_owned())
            .unwrap();
        gamelogic::control_bar::refresh_control_bar_bridge_from_common().unwrap();
        {
            let bridge = gamelogic::control_bar::get_control_bar_bridge().unwrap();
            let button = bridge.find_command_button_by_name(BUTTON).unwrap();
            assert!(button.get_upgrade_template().is_some());
            assert_ne!(
                button.get_options_bits() & CommandOption::NeedUpgrade as u32,
                0
            );
        }
        let mut template = gamelogic::player::PlayerTemplate::new("ShortcutMetadata".to_owned());
        template.special_power_shortcut_command_set = SET.to_owned();
        let mut local = gamelogic::player::Player::new(7);
        local.init(Arc::new(template));
        let local = Arc::new(RwLock::new(local));
        {
            let mut players = logic_player_list().write().unwrap();
            *players = gamelogic::player::PlayerList::new();
            players.add_player(Arc::clone(&local));
            players.set_local_player_index(7);
        }
        assert!(!local.has_upgrade_complete(&upgrade));
        // Install the existing shortcut row inventory and actual named windows.
        // No WND asset/device is required to call the real population algorithm.
        fn bar(layout: &str) -> ControlBar {
            let mut bar = ControlBar::new();
            bar.special_power_shortcut_layout = layout.to_owned();
            bar.special_power_shortcut_count = 1;
            bar.special_power_shortcuts.push(SpecialPowerShortcutState {
                command_name: String::new(),
                availability: CommandAvailability::Hidden,
                multiplier_count: 1,
                is_hidden: true,
            });
            for name in [
                bar.leftover_shortcut_button_name(0),
                bar.leftover_shortcut_parent_name(0),
            ] {
                with_window_manager(|manager| {
                    let window = manager.create_window(None, 0, 0, 32, 32).unwrap();
                    window.borrow_mut().set_name(&name);
                    window.borrow_mut().hide(true).unwrap();
                });
            }
            bar
        }
        fn assert_row(bar: &ControlBar, visible: bool) {
            let row = &bar.special_power_shortcuts[0];
            assert_eq!(row.is_hidden, !visible);
            assert_eq!(row.command_name, if visible { BUTTON } else { "" });
            for name in [
                bar.leftover_shortcut_button_name(0),
                bar.leftover_shortcut_parent_name(0),
            ] {
                let window = leftover_find_window(&name).expect("real installed shortcut window");
                assert_eq!(window.borrow().is_hidden(), !visible, "{name}");
            }
        }
        let observation =
            |completed: bool, present: bool, object_completed: bool| UpgradeTooltipObservation {
                command: None,
                template: present.then(|| Arc::clone(&upgrade)),
                completed,
                object_completed,
                object_affected: Some(false),
            };
        let completed = |command: &str, name: &str| {
            assert!(command.is_empty());
            assert_eq!(name, UPGRADE);
            observation(true, true, false)
        };
        let peer_incomplete = |_: &str, _: &str| observation(false, true, true);
        let missing = |_: &str, _: &str| observation(true, false, true);
        let mut owner = bar("ReceivingOwner.wnd");
        let mut peer = bar("ReceivingPeer.wnd");
        let forbidden = |_: &str, _: &str| -> UpgradeTooltipObservation {
            panic!("ending shortcut reveal must stop before definition observation");
        };
        owner.show_special_power_shortcut_receiving(
            UpgradeTooltipSource::Receiving(&forbidden),
            true,
        );
        assert_row(&owner, false);
        assert_row(&peer, false);
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
                &completed,
            ))
            .unwrap();
        assert_row(&owner, true); // Core is incomplete: no Core fallback can supply this row.
        peer.populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
            &peer_incomplete,
        ))
        .unwrap();
        assert_row(&peer, false); // Object completion is intentionally irrelevant to this Player gate.
        assert_row(&owner, true); // Another bar cannot rewrite this installed row/window.
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
                &missing,
            ))
            .unwrap();
        assert_row(&owner, false); // An old populated row is removed at this operation.
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
                &completed,
            ))
            .unwrap();
        assert_row(&owner, true);
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Standalone)
            .unwrap();
        assert_row(&owner, false);
        local.add_upgrade(&upgrade, gamelogic::upgrade::UpgradeStatus::Complete, None);
        assert!(local.has_upgrade_complete(&upgrade));
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Standalone)
            .unwrap();
        assert_row(&owner, true); // Explicit standalone still uses its real Core Player.
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
                &missing,
            ))
            .unwrap();
        assert_row(&owner, false); // Receiving Missing must reject even this completed Core sentinel.
        peer.populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
            &peer_incomplete,
        ))
        .unwrap();
        assert_row(&peer, false);
        owner
            .populate_special_power_shortcut_with_upgrade_source(UpgradeTooltipSource::Receiving(
                &completed,
            ))
            .unwrap();
        assert_row(&owner, true);
    });
}
