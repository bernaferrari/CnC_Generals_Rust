use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
use glam::Vec3;

const PARENT: &str = "StealthUpgradeNest";
const CHILD: &str = "StealthUpgradeSpawn";
const GRANT: &str = "Upgrade_StealthFixture";
const PROBE: &str = "Upgrade_StealthProbe";
const SPAWN: &str = "
 Behavior = DefaultProductionExitUpdate Exit
 End
 Behavior = SpawnBehavior Spawn
  OneShot = Yes
  SpawnNumber = 1
  SpawnTemplateName = StealthUpgradeSpawn
 End
";
const STEALTH: &str = "
 Behavior = StealthUpgrade Stealth
  TriggeredBy = Upgrade_StealthFixture
  RemovesUpgrades = Upgrade_StealthProbe
 End
";
const RADAR: &str = "
 Behavior = RadarUpgrade Radar
  TriggeredBy = Upgrade_StealthFixture
  RemovesUpgrades = Upgrade_StealthFixture
 End
";

fn world(behaviors: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    for name in [GRANT, PROBE] {
        register_test_upgrade(&mut world, name, "OBJECT", 0, 1);
    }
    world
        .admit_object_ini_sources([(
            "stealth-upgrade.ini".into(),
            format!(
                "Object {PARENT}\n KindOf = STRUCTURE SPAWNS_ARE_THE_WEAPONS\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n{SPAWN}{behaviors}End\n\
                 Object {CHILD}\n KindOf = INFANTRY\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n\
                 Behavior = StealthUpdate Camouflage\n InnateStealth = No\n StealthDelay = 0\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world
}

fn spawn(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let parent = world
        .create_object_for_player(PARENT, 1, Vec3::ZERO)
        .unwrap();
    world.frame += 1;
    world
        .update_spawn_behavior_with_effects(&mut Default::default())
        .unwrap();
    let children = world.host_object(parent).unwrap().spawn_behavior_children();
    assert_eq!(children.len(), 1);
    (parent, children[0].0)
}

fn can_stealth(world: &GameLogic, id: ObjectId) -> bool {
    world.host_object(id).unwrap().innate_stealth
}

#[test]
fn stealth_callback_applies_and_resets_in_authored_order() {
    let mut stealth_first = world(&format!("{STEALTH}{RADAR}"));
    let mut radar_first = world(&format!("{RADAR}{STEALTH}"));
    let (parent, child) = spawn(&mut stealth_first);
    assert_eq!(spawn(&mut radar_first), (parent, child));

    for world in [&mut stealth_first, &mut radar_first] {
        assert!(!can_stealth(world, child));
        world.apply_upgrade_to_object(parent, PROBE);
        world.apply_upgrade_to_object(parent, GRANT);
        assert!(
            !world
                .host_object(parent)
                .unwrap()
                .has_object_upgrade_complete(PROBE)
        );
        assert!(
            !world
                .host_object(parent)
                .unwrap()
                .has_object_upgrade_complete(GRANT)
        );
        assert!(can_stealth(world, parent));
        assert!(can_stealth(world, child));
        assert!(!world.host_object(child).unwrap().status.stealthed);
    }
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&radar_first).unwrap();
    let mut restored = world(&format!("{RADAR}{STEALTH}"));
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();

    // A separate status change may clear capability. Only a reset
    // module runs again when the Object re-acquires the trigger.
    for world in [&mut stealth_first, &mut radar_first, &mut restored] {
        world.host_object_mut(child).unwrap().innate_stealth = false;
        world.apply_upgrade_to_object(parent, PROBE);
        world.apply_upgrade_to_object(parent, GRANT);
    }
    assert!(can_stealth(&stealth_first, child));
    assert!(!can_stealth(&radar_first, child));
    assert!(!can_stealth(&restored, child));
    assert!(
        radar_first
            .host_object(parent)
            .unwrap()
            .has_object_upgrade_complete(PROBE)
    );
}

#[test]
fn stealth_capability_restores_after_trigger_removal() {
    let mut source = world(STEALTH);
    let mut peer = world(STEALTH);
    let (parent, child) = spawn(&mut source);
    assert_eq!(spawn(&mut peer), (parent, child));
    source.apply_upgrade_to_object(parent, GRANT);
    assert!(source.remove_object_upgrade_with_receiving_rules(parent, GRANT));
    assert!(can_stealth(&source, parent));
    assert!(can_stealth(&source, child));
    assert!(!can_stealth(&peer, child));

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world(STEALTH);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(
        !restored
            .host_object(parent)
            .unwrap()
            .has_object_upgrade_complete(GRANT)
    );
    assert!(can_stealth(&restored, parent));
    assert!(can_stealth(&restored, child));
    assert!(!restored.host_object(child).unwrap().status.stealthed);
    assert_eq!(
        restored
            .host_object(parent)
            .unwrap()
            .spawn_behavior_children()[0]
            .0,
        child
    );
    assert!(!can_stealth(&peer, child));
}

#[test]
fn stealth_save_reads_the_framed_record_when_its_timer_contains_the_tag() {
    let mut source = world(STEALTH);
    let (_, child) = spawn(&mut source);
    let grant = u32::from_le_bytes(*b"STLG");
    source
        .host_object_mut(child)
        .unwrap()
        .stealth_frames_granted = grant;
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = world(STEALTH);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(child).unwrap().stealth_frames_granted,
        grant
    );
}
