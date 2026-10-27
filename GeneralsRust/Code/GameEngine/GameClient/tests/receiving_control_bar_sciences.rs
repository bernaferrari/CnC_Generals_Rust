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
