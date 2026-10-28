use game_client_rust::gui::control_bar::ControlBar;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::science::{ScienceDefinition, ScienceNameFacts, ScienceStore};
use game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions;
use std::sync::Arc;

fn catalog(label: &str, extra: bool) -> (Arc<ReceivingActionDefinitions>, i32) {
    let mut store = ScienceStore::new_owned();
    if extra {
        store.ingest_definition(ScienceDefinition {
            name: "SCIENCE_Extra".into(),
            ..Default::default()
        });
    }
    store.ingest_definition(ScienceDefinition {
        name: "SCIENCE_Shared".into(),
        ..Default::default()
    });
    let facts = ScienceNameFacts::from_store(&store);
    let science = facts.science("SCIENCE_Shared").unwrap();
    let mut actions = ReceivingActionDefinitions::default();
    actions.apply_source_with_load_type(&format!(
        "SpecialPower SharedPower\n Enum = SPECIAL_SPY_SATELLITE\n RequiredScience = SCIENCE_Shared\nEnd\n\
         CommandButton SharedButton\n Command = SPECIAL_POWER\n SpecialPower = SharedPower\n Science = SCIENCE_Shared\n Options = NEED_SPECIAL_POWER_SCIENCE OK_FOR_MULTI_SELECT\n TextLabel = {label}\nEnd\n\
         CommandButton SharedPurchase\n Command = PURCHASE_SCIENCE\n Science = SCIENCE_Shared\n ButtonImage = RankedImage\n TextLabel = RankedLabel\nEnd\n\
         CommandButton ExitRider\n Command = EXIT_CONTAINER\n ButtonImage = EmptySeat\nEnd\n\
         CommandSet Rank1\n 1 = SharedPurchase\nEnd\nCommandSet Rank3\nEnd\nCommandSet Rank8\nEnd\n\
         CommandSet SharedSet\n 1 = SharedButton\n 2 = ExitRider\nEnd\n"
    ), &facts, INILoadType::Overwrite).unwrap();
    (Arc::new(actions), science)
}

#[test]
fn two_control_bars_populate_commands_and_required_science_from_their_own_sessions() {
    let (first_actions, first_science) = catalog("FirstButton", false);
    let (peer_actions, peer_science) = catalog("PeerButton", true);
    assert_ne!(first_science, peer_science);
    let upgrades = gamelogic::upgrade::center::UpgradeCenter::new();
    let mut first = ControlBar::new();
    let mut peer = ControlBar::new();
    first.set_receiving_upgrade_definitions(&upgrades, Some(first_actions));
    peer.set_receiving_upgrade_definitions(&upgrades, Some(peer_actions));
    first.apply_presentation_science_ids(Some(&[first_science]));
    peer.apply_presentation_science_ids(Some(&[]));
    first.sync_command_set_from_presentation(Some("SharedSet"));
    peer.sync_command_set_from_presentation(Some("SharedSet"));
    assert_eq!(
        first.get_context().available_commands[0].text_label,
        "FirstButton"
    );
    assert!(!first.get_context().available_commands[0].button_hidden);
    assert_eq!(
        peer.get_context().available_commands[0].text_label,
        "PeerButton"
    );
    assert!(peer.get_context().available_commands[0].button_hidden);

    peer.apply_presentation_science_ids(Some(&[peer_science]));
    peer.sync_multi_select_command_sets_from_presentation(&[
        "SharedSet".into(),
        "SharedSet".into(),
    ]);
    assert!(!peer.get_context().available_commands[0].button_hidden);
    assert_eq!(
        first.get_context().available_commands[0].text_label,
        "FirstButton"
    );

    // C++ populateCommand reads the selected object's controller, not the
    // local player: opposite memberships must not select the other owner.
    use game_client_rust::gui::control_bar::control_bar::SelectedPlayerFacts;
    let sets = ["Rank1".into(), "Rank3".into(), "Rank8".into()];
    first.apply_presentation_science_ids(Some(&[]));
    first.apply_presentation_selected_player(Some(SelectedPlayerFacts {
        science_ids: vec![first_science],
        purchase_command_sets: sets.clone(),
        beacon_template: String::new(),
    }));
    first.sync_command_set_from_presentation(Some("SharedSet"));
    let first_command = &first.get_context().available_commands[0];
    assert!(!first_command.button_hidden);
    assert_eq!(first_command.button_image, "RankedImage");
    assert_eq!(first_command.text_label, "RankedLabel");

    peer.apply_presentation_science_ids(Some(&[peer_science]));
    peer.apply_presentation_selected_player(Some(SelectedPlayerFacts {
        science_ids: Vec::new(),
        purchase_command_sets: sets,
        beacon_template: String::new(),
    }));
    peer.sync_command_set_from_presentation(Some("SharedSet"));
    assert!(peer.get_context().available_commands[0].button_hidden);
    assert!(!first.get_context().available_commands[0].button_hidden);

    use game_client_rust::gui::control_bar::{SelectedContextFacts, StructureInventoryOccupant};
    first.apply_presentation_selected_context(SelectedContextFacts {
        controllable: true,
        ..Default::default()
    });
    first.sync_presentation_occupants(vec![StructureInventoryOccupant {
        object_id: 731,
        button_image: "RiderImage".into(),
        overlay_image: None,
    }]);
    first.sync_structure_context_from_presentation(2, 1, false, 0.0);
    first.sync_command_set_from_presentation(Some("SharedSet"));
    let mut client = game_client_rust::core::GameClient::new().unwrap();
    client
        .set_in_game_ui_subsystem(game_client_rust::core::subsystems::InGameUISubsystem::default());
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            first
                .update_for_selection_owned(vec![730], manager, effects)
                .unwrap();
        });
    });
    let exit = &first.get_context().available_commands[1];
    assert_eq!(exit.exit_object_id, Some(731));
    assert_eq!(exit.button_image, "RiderImage");
    assert!(exit.button_enabled);
}

#[test]
fn process_command_uses_each_receiving_control_bar_upgrade_catalog() {
    use game_client_rust::gui::control_bar::{
        CommandSourceType, HostControlBarInputProvenance, SelectedContextFacts,
    };
    use game_engine::common::ascii_string::AsciiString;
    use gamelogic::upgrade::center::UpgradeCenter;
    use std::sync::Mutex;

    const PLAYER_ID: i32 = 0;
    const UPGRADE: &str = "Upgrade_ReceivingControlBarCommand";
    const BUTTON: &str = "Command_ReceivingControlBarUpgrade";
    const SET: &str = "ReceivingControlBarUpgradeSet";
    static SERIAL: Mutex<()> = Mutex::new(());
    let _serial = SERIAL.lock().expect("serialize command queue fixture");

    let source = format!(
        "CommandButton {BUTTON}\n Command = PLAYER_UPGRADE\n Upgrade = {UPGRADE}\nEnd\n\
         CommandSet {SET}\n 1 = {BUTTON}\nEnd\n"
    );
    let science_store = ScienceStore::new_owned();
    let facts = ScienceNameFacts::from_store(&science_store);
    let mut actions = ReceivingActionDefinitions::default();
    actions
        .apply_source_with_load_type(&source, &facts, INILoadType::Overwrite)
        .expect("parse receiving upgrade command");
    let actions = Arc::new(actions);

    let mut owner_catalog = UpgradeCenter::new();
    owner_catalog.new_upgrade(AsciiString::from(UPGRADE));
    let peer_catalog = UpgradeCenter::new();
    let mut owner = ControlBar::new();
    let mut peer = ControlBar::new();
    owner.set_receiving_upgrade_definitions(&owner_catalog, Some(Arc::clone(&actions)));
    peer.set_receiving_upgrade_definitions(&peer_catalog, Some(actions));
    owner.apply_presentation_selected_context(SelectedContextFacts {
        controllable: true,
        ..Default::default()
    });
    peer.apply_presentation_selected_context(SelectedContextFacts {
        controllable: true,
        ..Default::default()
    });
    owner.sync_command_set_from_presentation(Some(SET));
    peer.sync_command_set_from_presentation(Some(SET));

    // Public owned update creates the enabled button state consumed by
    // process_command. No internal fixture access or production API change.
    let mut emitted = Vec::new();
    let mut owner_client = game_client_rust::core::GameClient::new().unwrap();
    let mut peer_client = game_client_rust::core::GameClient::new().unwrap();
    owner_client.set_in_game_ui_subsystem(Default::default());
    peer_client.set_in_game_ui_subsystem(Default::default());
    owner_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            owner.update_for_selection_owned(vec![41], manager, effects)
                .expect("update owner control bar");
        });
    });
    peer_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            peer.update_for_selection_owned(vec![41], manager, effects)
                .expect("update peer control bar");
        });
    });
    let queue_manager = gamelogic::commands::get_command_queue_manager();
    {
        let mut manager = queue_manager.lock().expect("command queue manager");
        manager.clear_all();
        if manager.get_player_stats(PLAYER_ID).is_none() {
            manager
                .initialize_player(PLAYER_ID)
                .expect("initialize test player queue");
        }
    }
    let mut owner_ui = game_client_rust::core::subsystems::InGameUISubsystem::default();
    let mut peer_ui = game_client_rust::core::subsystems::InGameUISubsystem::default();
    assert!(
        owner
            .process_command(
                BUTTON,
                CommandSourceType::FromUser,
                &mut emitted,
                &mut owner_ui,
                HostControlBarInputProvenance::InjectedOrUnknown,
            )
            .expect("process owner command")
    );
    let queued_after_owner = queue_manager
        .lock()
        .expect("command queue manager")
        .get_player_stats(PLAYER_ID)
        .expect("owner player queue")
        .pending_count;
    assert_eq!(queued_after_owner, 1);

    assert!(
        peer.process_command(
            BUTTON,
            CommandSourceType::FromUser,
            &mut emitted,
            &mut peer_ui,
            HostControlBarInputProvenance::InjectedOrUnknown,
        )
        .expect("process peer command")
    );
    let queued_after_peer = queue_manager
        .lock()
        .expect("command queue manager")
        .get_player_stats(PLAYER_ID)
        .expect("peer player queue")
        .pending_count;
    assert_eq!(
        queued_after_peer, queued_after_owner,
        "peer must not resolve the owner's same-named upgrade"
    );
}
