//! Actual immutable definition -> Object factory partition threat, not a name lookup.
use super::*;

const NAME: &str = "AmericaTankThreatOwner";
fn source(threat: u16) -> String {
    format!(
        "Object {NAME}\nKindOf = VEHICLE\nVisionRange = 80\nShroudClearingRange = 80\nThreatValue = {threat}\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n"
    )
}
fn receiving(threat: u16) -> GameLogic {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "receiving-threat.ini".to_owned(),
        source(threat),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    install_player(&mut world);
    world
}
fn install_player(world: &mut GameLogic) {
    // Prepare the receiving default team before strict TeamFactory restore,
    // as SidesList::validateSides / Player::setDefaultTeam do at new game.
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "Receiving threat owner",
        true,
    ));
}
fn reset_receiving(world: &mut GameLogic) {
    world.reset();
    install_player(world);
}
fn assert_partition_stamp(world: &GameLogic, id: ObjectId, expected: u32) {
    let object = world.host_object(id).unwrap();
    let stamp = object
        .partition_last_affect
        .expect("actual factory/maintenance partition affect stamp");
    assert_eq!(
        stamp.threat, expected,
        "actual partition affect uses receiving threat"
    );
    assert_eq!(
        stamp.mask, 1,
        "the actual controlling player owns the stamp"
    );
    assert_eq!(stamp.range, 80.0);
}
fn spawn(world: &mut GameLogic, expected: u32) -> ObjectId {
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(
        world.host_object(id).unwrap().partition_threat_value,
        expected,
        "actual Object partition threat must come from its exact receiving template version"
    );
    assert_partition_stamp(world, id, expected);
    id
}

#[test]
fn receiving_threat_factory_stamp_ignores_same_name_foreign_factory_and_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_threat_factory_stamp_ignores_same_name_foreign_factory_and_peer",
        || {
            let mut first = receiving(17);
            let mut peer = receiving(91);
            let mut zero = receiving(0);
            assert_eq!(
                game_engine::common::thing::thing_factory::load_templates_from_ini_text(
                    &source(999),
                    "foreign-threat.ini"
                ),
                1
            );
            let compatibility = ThingTemplate::new(NAME);
            assert_eq!(
                compatibility.get_threat_value(),
                999,
                "explicit None compatibility retains the actual legacy factory path"
            );
            let a = spawn(&mut first, 17);
            let b = spawn(&mut peer, 91);
            let c = spawn(&mut zero, 0);
            assert_eq!(a, b, "same object ID does not select the foreign owner");
            assert_eq!(
                a, c,
                "authored zero shares IDs but never the foreign threat"
            );
            let held = game_engine::common::thing::get_thing_factory().unwrap();
            assert_eq!(first.templates[NAME].get_threat_value(), 17);
            assert_eq!(peer.templates[NAME].get_threat_value(), 91);
            assert_eq!(zero.templates[NAME].get_threat_value(), 0);
            spawn(&mut first, 17);
            spawn(&mut zero, 0);
            drop(held);
            reset_receiving(&mut peer);
            assert_eq!(first.host_object(a).unwrap().partition_threat_value, 17);
            spawn(&mut peer, 91);
        },
    );
}

#[test]
fn receiving_threat_map_solo_zero_reset_and_encoded_continuation_use_effective_version() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_threat_map_solo_zero_reset_and_encoded_continuation_use_effective_version",
        || {
            let mut world = receiving(7);
            let mut peer = receiving(91);
            assert_eq!(
                game_engine::common::thing::thing_factory::load_templates_from_ini_text(
                    &source(999),
                    "foreign-threat-continuation.ini"
                ),
                1
            );
            spawn(&mut world, 7);
            reset_receiving(&mut world);
            let map = format!("Object {NAME}\nThreatValue = 37\nEnd\n");
            let solo = format!("Object {NAME}\nThreatValue = 0\nEnd\n");
            world
                .apply_receiving_module_overrides(std::slice::from_ref(&map))
                .unwrap();
            let nonzero = spawn(&mut world, 37);
            let builder = crate::save_load::SnapshotBuilder::new();
            let nonzero_snapshot = builder.create_world_snapshot(&world).unwrap();
            let nonzero_bytes = bincode_legacy::serialize(&nonzero_snapshot).unwrap();
            let nonzero_decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&nonzero_bytes).unwrap();
            let mut nonzero_restored = receiving(7);
            nonzero_restored
                .apply_receiving_module_overrides(std::slice::from_ref(&map))
                .unwrap();
            builder
                .restore_from_snapshot(&nonzero_decoded, &mut nonzero_restored)
                .unwrap();
            assert_eq!(
                nonzero_restored
                    .host_object(nonzero)
                    .unwrap()
                    .partition_threat_value,
                37,
                "nonzero receiving threat must be reconstructed at actual encoded restore"
            );
            nonzero_restored
                .host_object_mut(nonzero)
                .unwrap()
                .handle_partition_cell_maintenance();
            assert_partition_stamp(&nonzero_restored, nonzero, 37);
            spawn(&mut nonzero_restored, 37);
            reset_receiving(&mut world);
            world
                .apply_receiving_module_overrides(&[map.clone(), solo.clone()])
                .unwrap();
            let zero = spawn(&mut world, 0);
            assert_eq!(
                world.templates[NAME].get_threat_value(),
                0,
                "authored zero is authoritative"
            );
            spawn(&mut peer, 91);
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
            let mut restored = receiving(7);
            restored
                .apply_receiving_module_overrides(&[map, solo])
                .unwrap();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.host_object(zero).unwrap().partition_threat_value,
                0,
                "encoded restore binds the actual receiving effective template"
            );
            restored
                .host_object_mut(zero)
                .unwrap()
                .handle_partition_cell_maintenance();
            assert_partition_stamp(&restored, zero, 0);
            spawn(&mut restored, 0);
            reset_receiving(&mut world);
            spawn(&mut world, 7);
            assert_eq!(
                restored.host_object(zero).unwrap().partition_threat_value,
                0
            );
            assert_eq!(peer.templates[NAME].get_threat_value(), 91);
        },
    );
}
