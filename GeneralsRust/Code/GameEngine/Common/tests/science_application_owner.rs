use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::science::ScienceNameFacts;
use game_engine::common::rts::science::{SCIENCE_INVALID, ScienceStore};
use game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions;

#[test]
fn science_names_and_map_overrides_follow_their_application_across_threads() {
    let mut first = ScienceStore::new_owned();
    let mut peer = ScienceStore::new_owned();
    let first_source = b"Science Root\n SciencePurchasePointCost = 0\nEnd\nScience Shared\n PrerequisiteSciences = Root\n SciencePurchasePointCost = 1\nEnd\n";
    let peer_source = b"Science PeerRoot\n SciencePurchasePointCost = 0\nEnd\nScience Another\n SciencePurchasePointCost = 1\nEnd\nScience Shared\n PrerequisiteSciences = PeerRoot\n SciencePurchasePointCost = 3\nEnd\n";

    NameKeyGenerator::init();
    assert_eq!(NameKeyGenerator::name_to_key("before"), 1);
    first
        .load_from_sources(&[("first", first_source, false)])
        .unwrap();
    peer.load_from_sources(&[("peer", peer_source, false)])
        .unwrap();
    assert_eq!(NameKeyGenerator::name_to_key("after"), 2);

    let first_id = first.get_science_from_internal_name("Shared");
    let peer_id = peer.get_science_from_internal_name("Shared");
    assert_ne!(first_id, SCIENCE_INVALID);
    assert_ne!(first_id, peer_id);
    assert_eq!(first.get_science_purchase_cost(first_id), 1);
    assert_eq!(peer.get_science_purchase_cost(peer_id), 3);

    let first = std::thread::spawn(move || {
        // The driving thread has an unrelated namespace with conflicting IDs.
        NameKeyGenerator::init();
        for name in ["foreign", "Shared", "Root", "MapOnly"] {
            NameKeyGenerator::name_to_key(name);
        }
        first
            .apply_override_text("Science Shared\n SciencePurchasePointCost = 7\n PrerequisiteSciences = None\nEnd\nScience MapOnly\n PrerequisiteSciences = Shared\n SciencePurchasePointCost = 2\nEnd\n")
            .unwrap();
        assert_eq!(first.get_science_from_internal_name("Shared"), first_id);
        assert_eq!(first.get_science_purchase_cost(first_id), 7);
        assert!(first.find_science_info(first_id).unwrap().prereq_sciences.is_empty());
        let map_id = first.get_science_from_internal_name("MapOnly");
        assert_ne!(map_id, first_id);
        assert_eq!(first.find_science_info(map_id).unwrap().prereq_sciences, vec![first_id]);
        first
    }).join().unwrap();

    assert_eq!(peer.get_science_purchase_cost(peer_id), 3);
    assert_eq!(
        peer.get_science_from_internal_name("MapOnly"),
        SCIENCE_INVALID
    );
    let mut reset = first;
    reset.reset();
    assert_eq!(reset.get_science_purchase_cost(first_id), 1);
    assert_eq!(
        reset.get_science_from_internal_name("MapOnly"),
        SCIENCE_INVALID
    );
}

#[test]
fn startup_command_definitions_are_base_versions_with_receiving_science_ids() {
    let mut sciences = ScienceStore::new_owned();
    sciences
        .load_from_sources(&[(
            "Science.ini",
            b"Science SCIENCE_Owned\n SciencePurchasePointCost = 1\nEnd\n",
            false,
        )])
        .unwrap();
    let facts = ScienceNameFacts::from_store(&sciences);
    let mut actions = ReceivingActionDefinitions::default();
    actions.apply_source_with_load_type(
        "CommandButton OwnedButton\n Command = PURCHASE_SCIENCE\n Science = SCIENCE_Owned\n TextLabel = Original\nEnd\nCommandSet OwnedSet\n 1 = OwnedButton\nEnd\n",
        &facts,
        INILoadType::Overwrite,
    ).unwrap();
    actions
        .apply_source_with_load_type(
            "CommandButton OwnedButton\n TextLabel = Updated\nEnd\n",
            &facts,
            INILoadType::Overwrite,
        )
        .unwrap();
    let button = actions
        .buttons
        .find_command_button_resolved("OwnedButton")
        .unwrap();
    assert!(!button.is_override());
    assert_eq!(button.text_label, "Updated");
    assert_eq!(
        button.parsed_science_required,
        vec![facts.science("SCIENCE_Owned").unwrap()]
    );
    let set = actions.sets.find_command_set_resolved("OwnedSet").unwrap();
    assert!(!set.is_override());
    assert_eq!(
        set.get_button_at_position(0).map(String::as_str),
        Some("OwnedButton")
    );
}
