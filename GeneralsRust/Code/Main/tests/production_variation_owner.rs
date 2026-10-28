use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

const PRODUCER: &str = "AmericaBarracks";
const BASE: &str = "AmericaInfantryRanger";

fn world(variation_one: &str, variation_two: &str) -> (GameLogic, ObjectId) {
    let object_ini = format!(
        "Object {PRODUCER}\n\
         KindOf = STRUCTURE FS_BARRACKS SELECTABLE\n\
         BuildCost = 500\n\
         Body = ActiveBody BarracksBody\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\n\
         Object {BASE}\n\
         KindOf = INFANTRY SELECTABLE SCORE\n\
         BuildCost = 100\n BuildTime = 0.1\n\
         BuildVariations = {variation_one} {variation_two}\n\
         Body = ActiveBody RangerBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
         Object {variation_one}\n\
         KindOf = INFANTRY SELECTABLE SCORE\n\
         BuildCost = 100\n BuildTime = 0.1\n\
         Body = ActiveBody VariationOneBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
         Object {variation_two}\n\
         KindOf = INFANTRY SELECTABLE SCORE\n\
         BuildCost = 100\n BuildTime = 0.1\n\
         Body = ActiveBody VariationTwoBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    );
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([("Object.ini".into(), object_ini)])
        .expect("admit per-world Object INI");
    logic.start_new_game(GameMode::SinglePlayer);
    let mut player = Player::new(0, Team::USA, "Owner", true);
    player.resources.supplies = 5000;
    logic.add_player(player);
    let producer = logic
        .create_object_for_player(PRODUCER, 0, Vec3::ZERO)
        .expect("create barracks");
    let building = logic.host_object_mut(producer).expect("producer");
    building.set_status_under_construction(false);
    building.construction_percent = 1.0;
    (logic, producer)
}

fn queue_ready_unit(logic: &mut GameLogic, producer: ObjectId) {
    assert!(logic.enqueue_production(producer, BASE.into()));
    let head = logic
        .host_object_mut(producer)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap()
        .production_queue
        .first_mut()
        .unwrap();
    head.progress = head.total_time;
    head.construction_frames = 100_000;
}

fn complete(logic: &mut GameLogic, producer: ObjectId) {
    // Public fixed-step entry; ordinary post-command updates collect and
    // complete the queued unit through Main's production owner. A producer
    // with a door first opens it, then releases the completed unit on a later
    // logic frame, so keep stepping until the bounded door cycle finishes.
    for _ in 0..8 {
        logic.update_with_dt(1.0 / 30.0);
        let output_count = logic
            .host_objects()
            .registered_values()
            .filter(|object| object.id != producer)
            .count();
        if output_count != 0 {
            break;
        }
    }
    assert_eq!(
        logic
            .host_objects()
            .registered_values()
            .filter(|object| object.id != producer)
            .count(),
        1,
        "one queued unit must complete through the producer door cycle"
    );
    assert!(
        logic
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty(),
        "the completed unit's queue entry must be consumed"
    );
}

fn produced_name(logic: &GameLogic, producer: ObjectId) -> String {
    let outputs: Vec<_> = logic
        .host_objects()
        .registered_values()
        .filter(|object| object.id != producer)
        .map(|object| object.template_name.clone())
        .collect();
    assert_eq!(outputs.len(), 1, "one queued unit must complete");
    outputs.into_iter().next().unwrap()
}

#[test]
fn public_production_completion_uses_each_same_id_worlds_variation_catalog() {
    let (mut owner, owner_factory) = world("OwnerRangerVariantA", "OwnerRangerVariantB");
    let (mut peer, peer_factory) = world("PeerRangerVariantA", "PeerRangerVariantB");
    assert_eq!(
        owner_factory, peer_factory,
        "fixture reuses producer ObjectId"
    );
    queue_ready_unit(&mut owner, owner_factory);
    queue_ready_unit(&mut peer, peer_factory);

    complete(&mut peer, peer_factory);
    complete(&mut owner, owner_factory);

    let owner_output = produced_name(&owner, owner_factory);
    let peer_output = produced_name(&peer, peer_factory);
    assert!(
        ["OwnerRangerVariantA", "OwnerRangerVariantB"].contains(&owner_output.as_str()),
        "owner produced foreign/global variation {owner_output}"
    );
    assert!(
        ["PeerRangerVariantA", "PeerRangerVariantB"].contains(&peer_output.as_str()),
        "peer produced foreign/global variation {peer_output}"
    );
}
