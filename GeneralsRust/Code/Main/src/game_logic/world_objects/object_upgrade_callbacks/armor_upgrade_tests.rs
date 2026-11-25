use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_battlemaster::{TERRAIN_DECAL_CHEMSUIT, TERRAIN_DECAL_NONE};
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot, decode_bincode_world_snapshot};
use crate::save_load::{Snapshot, XferLoad, XferSave};
use glam::Vec3;
use std::{io::Cursor, sync::Arc};

const TARGET: &str = "ArmorUpgradeFixture";
const UPGRADE: &str = "Upgrade_ArmorFixture";
const CHEMICAL_SUITS: &str = "Upgrade_AmericaChemicalSuits";
const PROBE_UPGRADE: &str = "Upgrade_ArmorProbe";

fn world(behaviors: &str, upgraded_percent: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    for upgrade in [UPGRADE, CHEMICAL_SUITS, PROBE_UPGRADE] {
        register_test_upgrade(&mut world, upgrade, "OBJECT", 0, 1);
    }
    world.admit_armor_definitions(Arc::new(
        gamelogic::object::armor::ArmorDefinitions::load_from_str(&format!(
            "Armor FixtureArmor\n Armor = DEFAULT 100%\nEnd\n\
             Armor FixtureUpgradedArmor\n Armor = DEFAULT {upgraded_percent}%\nEnd\n"
        ))
        .unwrap(),
    ));
    world
        .admit_object_ini_sources([(
            "armor-upgrade.ini".into(),
            format!(
                "Object {TARGET}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\n End\n\
                 ArmorSet\n Conditions = None\n Armor = FixtureArmor\n End\n\
                 ArmorSet\n Conditions = PLAYER_UPGRADE\n Armor = FixtureUpgradedArmor\n End\n{behaviors}\nEnd\n"
            ),
        )])
        .unwrap();
    world
}

fn spawn(world: &mut GameLogic) -> ObjectId {
    world
        .create_object_for_player(TARGET, 1, Vec3::ZERO)
        .unwrap()
}

fn explosive_damage(world: &mut GameLogic, id: ObjectId) -> f32 {
    let frame = world.get_frame();
    let object = world.host_object_mut(id).unwrap();
    let before = object.health.current;
    object.take_damage_from_typed(
        100.0,
        None,
        DamageType::Explosive,
        frame,
        &mut crate::game_logic::HostHealthEvents::default(),
    );
    before - object.health.current
}

#[test]
fn applied_armor_survives_trigger_removal_and_encoded_restore() {
    let behavior = format!("Behavior = ArmorUpgrade Armor\n TriggeredBy = {UPGRADE}\nEnd");
    let mut source = world(&behavior, 25);
    let mut peer = world(&behavior, 60);
    let id = spawn(&mut source);
    assert_eq!(spawn(&mut peer), id);
    assert_eq!(explosive_damage(&mut source, id), 100.0);
    source.apply_upgrade_to_object(id, UPGRADE);
    assert_eq!(explosive_damage(&mut source, id), 25.0);
    assert!(source.remove_object_upgrade_with_receiving_rules(id, UPGRADE));
    assert_eq!(explosive_damage(&mut source, id), 25.0);
    assert_eq!(explosive_damage(&mut peer, id), 100.0);

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let bincode_snapshot = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut wire = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut wire)).unwrap();
    let mut xfer_snapshot = WorldSnapshot::default();
    xfer_snapshot
        .xfer(&mut XferLoad::new(Cursor::new(wire.into_inner())))
        .unwrap();
    for snapshot in [bincode_snapshot, xfer_snapshot] {
        let mut restored = world(&behavior, 25);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        assert!(
            !restored
                .host_object(id)
                .unwrap()
                .has_object_upgrade_complete(UPGRADE)
        );
        assert_eq!(explosive_damage(&mut restored, id), 25.0);
    }
    assert_eq!(explosive_damage(&mut source, id), 25.0);
    assert_eq!(explosive_damage(&mut peer, id), 100.0);
}

#[test]
fn chemical_suits_decal_uses_the_modules_authored_triggers() {
    let alternate = CHEMICAL_SUITS.to_ascii_lowercase();
    let behavior =
        format!("Behavior = ArmorUpgrade Armor\n TriggeredBy = {UPGRADE} {alternate}\nEnd");
    let plain_behavior = format!("Behavior = ArmorUpgrade Armor\n TriggeredBy = {UPGRADE}\nEnd");
    let mut source = world(&behavior, 25);
    let mut peer = world(&plain_behavior, 60);
    let id = spawn(&mut source);
    assert_eq!(spawn(&mut peer), id);
    for world in [&mut source, &mut peer] {
        world.apply_upgrade_to_object(id, UPGRADE);
    }
    assert_eq!(
        source.host_object(id).unwrap().get_terrain_decal_type(),
        TERRAIN_DECAL_CHEMSUIT
    );
    assert_eq!(
        peer.host_object(id).unwrap().get_terrain_decal_type(),
        TERRAIN_DECAL_NONE
    );
    assert!(
        !source
            .host_object(id)
            .unwrap()
            .has_object_upgrade_complete(CHEMICAL_SUITS)
    );

    let source_object = source.host_object(id).unwrap();
    assert!(source_object.terrain_decal_size > 0.0);
    assert_eq!(source_object.terrain_decal_opacity, 1.0);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world(&behavior, 25);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(id).unwrap().get_terrain_decal_type(),
        TERRAIN_DECAL_CHEMSUIT
    );
    let restored_object = restored.host_object(id).unwrap();
    assert_eq!(
        restored_object.terrain_decal_size,
        source_object.terrain_decal_size
    );
    assert_eq!(
        restored_object.terrain_decal_opacity,
        source_object.terrain_decal_opacity
    );
}

#[test]
fn armor_latch_resets_in_authored_order() {
    let armor = format!(
        "Behavior = ArmorUpgrade Armor\n TriggeredBy = {UPGRADE}\n RemovesUpgrades = {PROBE_UPGRADE}\nEnd\n"
    );
    let radar = format!(
        "Behavior = RadarUpgrade Radar\n TriggeredBy = {UPGRADE}\n RemovesUpgrades = {UPGRADE}\nEnd\n"
    );
    let mut radar_first = world(&format!("{radar}{armor}"), 25);
    let mut armor_first = world(&format!("{armor}{radar}"), 25);
    let id = spawn(&mut radar_first);
    assert_eq!(spawn(&mut armor_first), id);
    for world in [&mut radar_first, &mut armor_first] {
        world.apply_upgrade_to_object(id, PROBE_UPGRADE);
        world.apply_upgrade_to_object(id, UPGRADE);
        assert!(
            !world
                .host_object(id)
                .unwrap()
                .has_object_upgrade_complete(PROBE_UPGRADE)
        );
        assert_eq!(explosive_damage(world, id), 25.0);
        world.apply_upgrade_to_object(id, PROBE_UPGRADE);
        world.apply_upgrade_to_object(id, UPGRADE);
    }
    assert!(
        radar_first
            .host_object(id)
            .unwrap()
            .has_object_upgrade_complete(PROBE_UPGRADE)
    );
    assert!(
        !armor_first
            .host_object(id)
            .unwrap()
            .has_object_upgrade_complete(PROBE_UPGRADE)
    );
}
