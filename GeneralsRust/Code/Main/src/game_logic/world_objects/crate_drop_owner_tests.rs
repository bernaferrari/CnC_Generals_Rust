use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::native_lifecycle::NativeLifecycleState;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{GameLogic, ObjectId, Team};
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use generals_sim::pathfinding::PathfindLayerEnum;
use glam::Vec3;

const FRAME: u32 = 75;
const POSITION: Vec3 = Vec3::new(25.0, 0.0, 40.0);
const FIRST: &str = "WitnessFirstCrate";
const SECOND: &str = "WitnessSecondCrate";
const PEER: &str = "WitnessPeerCrate";

fn admitted_world(second_crate: &str) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object CrateWitnessVictim\n\
         KindOf = IMMOBILE\n\
         Body = ActiveBody VictimBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
         Behavior = CreateCrateDie Drops\n\
         CrateData = MissingCrateRow\nCrateData = DeniedCrateRow\n\
         CrateData = FirstCrateRow\nCrateData = SecondCrateRow\nEnd\n\
         Behavior = KeepObjectDie Keep\nEnd\nEnd\n\
         Object {FIRST}\nKindOf = CRATE\n\
         Behavior = DeletionUpdate Expiration\nMinLifetime = 1000\nMaxLifetime = 1000\nEnd\nEnd\n\
         Object {SECOND}\nKindOf = CRATE\nEnd\n\
         Object {PEER}\nKindOf = CRATE\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("crate-drop-witness.ini".into(), source)])
        .unwrap();
    world
        .world_services
        .common()
        .ini
        .crate_system
        .write()
        .apply_mixed_source(&format!(
            "CrateData DeniedCrateRow\nCreationChance = 1\nVeterancyLevel = HEROIC\nCrateObject = {PEER} 1\nEnd\n\
             CrateData FirstCrateRow\nCreationChance = 1\nCrateObject = {FIRST} 1\nEnd\n\
             CrateData SecondCrateRow\nCreationChance = 1\nCrateObject = {second_crate} 1\nEnd\n"
        ))
        .unwrap();
    world.frame = FRAME;
    let victim = world
        .create_object("CrateWitnessVictim", Team::USA, POSITION)
        .unwrap();
    world.set_owned_pathfinder_layer(victim, PathfindLayerEnum::Top as u8);
    (world, victim)
}

#[test]
fn authored_crate_data_keeps_draw_order_and_receiving_catalog_after_save() {
    let (mut original, victim) = admitted_world(SECOND);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, restored_victim) = admitted_world(SECOND);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let (mut peer, peer_victim) = admitted_world(PEER);
    assert_eq!((restored_victim, peer_victim), (victim, victim));

    let kill_and_observe = |world: &mut GameLogic, final_crate: &str| {
        let mut expected = world.logic_random.snapshot();
        // The unknown row uses no draw. The known but denied row rolls only
        // chance. Successful rows each roll chance, pick and orientation.
        expected.next_real(0., 1.);
        expected.next_real(0., 1.);
        expected.next_real(0., 1.);
        let expiry = FRAME + expected.next_int(30, 30) as u32;
        let first_angle = expected.next_real(0., std::f32::consts::TAU);
        expected.next_real(0., 1.);
        expected.next_real(0., 1.);
        let second_angle = expected.next_real(0., std::f32::consts::TAU);

        world
            .apply_owned_damage(
                victim,
                1000.0,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
            .unwrap();
        assert_eq!(world.logic_random.seed_words(), expected.seed_words());
        let mut drops: Vec<_> = world
            .host_objects()
            .registered_values()
            .filter(|object| [FIRST, SECOND, PEER].contains(&object.template_name.as_str()))
            .collect();
        drops.sort_by_key(|object| object.id);
        let [first, second] = drops.as_slice() else {
            panic!("expected exactly two authored drops, got {}", drops.len());
        };
        for (object, template, orientation) in [
            (first, FIRST, first_angle),
            (second, final_crate, second_angle),
        ] {
            assert_eq!(object.template_name, template);
            assert_eq!(object.get_position(), POSITION);
            assert_eq!(object.pathfind_layer, PathfindLayerEnum::Top as u8);
            assert!((object.get_orientation() - orientation).abs() < 0.000001);
        }
        assert_eq!(
            first.native_lifecycle.as_ref().unwrap().states.as_ref(),
            &[NativeLifecycleState::Deletion {
                die_frame: expiry,
                active: true
            }],
        );
        (
            [first.id, second.id],
            expected.seed_words(),
            [first_angle, second_angle],
        )
    };

    let uninterrupted = kill_and_observe(&mut original, SECOND);
    assert_eq!(kill_and_observe(&mut peer, PEER), uninterrupted);
    assert_eq!(kill_and_observe(&mut restored, SECOND), uninterrupted);
}
