//! Actual production completion must resolve BuildVariations and consume RNG
//! from the receiving Main world, even when another world reuses its IDs.
use super::*;
use crate::game_logic::{GameLogic, Player, ThingTemplate};

fn world(seed: u32, variations: [&str; 2]) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Owner", true));
    for name in [
        "VariationOwnerProducer",
        "VariationBuildBase",
        variations[0],
        variations[1],
    ] {
        logic
            .templates
            .insert(name.to_string(), ThingTemplate::new(name));
    }
    logic
        .templates
        .get_mut("VariationBuildBase")
        .unwrap()
        .build_variations = variations.iter().map(|name| (*name).to_string()).collect();
    logic.logic_random.seed_random(seed);
    let producer = logic
        .create_object_for_player("VariationOwnerProducer", 0, Vec3::ZERO)
        .expect("create production owner");
    (logic, producer)
}

fn complete(logic: &mut GameLogic, producer: ObjectId) {
    logic
        .host_apply_unit_production_completions_with_effects(
            vec![(
                Team::USA,
                "VariationBuildBase".to_string(),
                Vec3::new(25.0, 0.0, 40.0),
                None,
                producer,
            )],
            &mut Default::default(),
        )
        .expect("production completion");
}

#[test]
fn same_id_production_completion_uses_receiving_template_variations_and_rng() {
    let mut owner = world(101, ["OwnerVariationOne", "OwnerVariationTwo"]);
    let mut peer = world(809, ["PeerVariationOne", "PeerVariationTwo"]);
    assert_eq!(
        owner.1, peer.1,
        "worlds intentionally reuse the producer ID"
    );

    let mut expected_owner = game_engine::common::random_value::SessionRandom::default();
    expected_owner.seed_random(101);
    let owner_index = expected_owner.next_int(0, 1) as usize;
    let peer_before = peer.0.logic_random.seed_words();

    complete(&mut owner.0, owner.1);
    assert_eq!(
        owner.0.logic_random.seed_words(),
        expected_owner.seed_words()
    );
    assert_eq!(peer.0.logic_random.seed_words(), peer_before);
    let owner_output = owner
        .0
        .host_objects()
        .registered_values()
        .find(|object| object.id != owner.1)
        .expect("production output");
    assert_eq!(
        owner_output.template_name,
        ["OwnerVariationOne", "OwnerVariationTwo"][owner_index]
    );

    let mut expected_peer = game_engine::common::random_value::SessionRandom::default();
    expected_peer.seed_random(809);
    let peer_index = expected_peer.next_int(0, 1) as usize;
    complete(&mut peer.0, peer.1);
    assert_eq!(peer.0.logic_random.seed_words(), expected_peer.seed_words());
    let peer_output = peer
        .0
        .host_objects()
        .registered_values()
        .find(|object| object.id != peer.1)
        .expect("production output");
    assert_eq!(
        peer_output.template_name,
        ["PeerVariationOne", "PeerVariationTwo"][peer_index]
    );
}

#[test]
fn missing_case_sensitive_build_variation_falls_back_after_one_draw() {
    let (mut logic, _) = world(541, ["OwnerVariationOne", "OwnerVariationTwo"]);
    logic
        .templates
        .get_mut("VariationBuildBase")
        .unwrap()
        .build_variations = vec!["ownervariationone".to_string()];
    let mut expected = game_engine::common::random_value::SessionRandom::default();
    expected.seed_random(541);
    let _ = expected.next_int(0, 0);

    assert_eq!(
        resolve_production_build_variation(&logic, "VariationBuildBase"),
        "VariationBuildBase",
        "missing exact-case selected template falls back to its source"
    );
    assert_eq!(logic.logic_random.seed_words(), expected.seed_words());

    assert_eq!(
        resolve_production_build_variation(&logic, "variationbuildbase"),
        "variationbuildbase",
        "source lookup is exact case-sensitive and consumes no draw"
    );
    assert_eq!(logic.logic_random.seed_words(), expected.seed_words());
}
