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
         CommandButton SharedButton\n Command = SPECIAL_POWER\n SpecialPower = SharedPower\n Options = NEED_SPECIAL_POWER_SCIENCE OK_FOR_MULTI_SELECT\n TextLabel = {label}\nEnd\n\
         CommandSet SharedSet\n 1 = SharedButton\nEnd\n"
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
}
