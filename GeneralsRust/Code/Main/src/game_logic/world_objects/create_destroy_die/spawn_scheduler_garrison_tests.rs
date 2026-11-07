use super::*;

fn fixture() -> (GameLogic, ObjectId) {
    let ini = r#"
Object GarrisonSpawnOwner
 KindOf = STRUCTURE
 Behavior = GarrisonContain Cargo
  IsEnclosingContainer = No
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = GarrisonSpawnChild
  OneShot = Yes
 End
End
Object GarrisonSpawnChild
 KindOf = INFANTRY
End
"#;
    let content = crate::assets::ReceivingObjectContent::from_sources([
        ("garrison-spawn.ini".into(), ini.into()),
    ]).unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let owner = world.create_object("GarrisonSpawnOwner", Team::GLA, Vec3::new(10.0, 4.0, 20.0)).unwrap();
    (world, owner)
}

#[test]
fn authored_garrison_spawn_uses_door_in_independent_worlds() {
    let (mut world, owner) = fixture();
    let (mut peer, peer_owner) = fixture();
    assert_eq!(owner, peer_owner);
    assert!(world.reserve_garrison_spawn_exit(owner, &mut Default::default()));
    world.set_current_frame(1);
    world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    let children = world.host_object(owner).unwrap().spawn_behavior_children();
    assert_eq!(children.len(), 1);
    assert!(peer.host_object(peer_owner).unwrap().spawn_behavior_children().is_empty());
    peer.set_current_frame(1);
    peer.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    let peer_children = peer.host_object(peer_owner).unwrap().spawn_behavior_children();
    assert_eq!(peer_children.len(), 1);
    assert_eq!(children[0].0, peer_children[0].0);
    assert_eq!(world.host_object(children[0].0).unwrap().get_position(),
        peer.host_object(peer_children[0].0).unwrap().get_position());
}

#[test]
fn garrison_exit_refuses_missing_and_same_object() {
    let (mut world, owner) = fixture();
    assert!(!world.exit_garrison_spawn(owner, owner, &mut Default::default()).unwrap());
    assert!(!world.exit_garrison_spawn(ObjectId(u32::MAX), owner, &mut Default::default()).unwrap());
}

#[test]
fn enclosing_spawn_waits_for_terrain_before_birth() {
    let ini = r#"
Object TerrainGarrison
 KindOf = STRUCTURE
 Behavior = GarrisonContain Cargo
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = TerrainGarrisonChild
  OneShot = Yes
 End
End
Object TerrainGarrisonChild
 KindOf = INFANTRY
End
"#;
    let content = crate::assets::ReceivingObjectContent::from_sources([
        ("terrain-garrison.ini".into(), ini.into()),
    ]).unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let owner = world.create_object("TerrainGarrison", Team::GLA, Vec3::ZERO).unwrap();
    assert!(world.terrain_height_at(Vec3::ZERO).is_none());
    assert!(!world.reserve_garrison_spawn_exit(owner, &mut Default::default()));
    world.set_current_frame(1);
    world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    assert!(world.host_object(owner).unwrap().spawn_behavior_children().is_empty());
}


#[test]
fn pending_authored_spawn_survives_snapshot_and_stays_session_owned() {
    let (mut source, owner) = fixture();
    let (peer, peer_owner) = fixture();
    assert_eq!(owner, peer_owner);
    source.set_current_frame(0);
    source.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    assert!(source.host_object(owner).unwrap().spawn_behavior_children().is_empty());
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, restored_owner) = fixture();
    assert_eq!(owner, restored_owner);
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    for frame in 1..=16 {
        source.set_current_frame(frame);
        restored.set_current_frame(frame);
        source.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
        restored.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    }
    let source_children = source.host_object(owner).unwrap().spawn_behavior_children();
    let restored_children = restored.host_object(owner).unwrap().spawn_behavior_children();
    assert_eq!(source_children.len(), 1);
    assert_eq!(restored_children.len(), 1);
    assert_eq!(source_children[0].0, restored_children[0].0);
    assert_eq!(source.host_object(source_children[0].0).unwrap().get_position(),
        restored.host_object(restored_children[0].0).unwrap().get_position());
    assert!(peer.host_object(peer_owner).unwrap().spawn_behavior_children().is_empty());
}

mod hive_roster {
    use crate as generals_main;
    use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
    use generals_main::save_load::SnapshotBuilder;
    use generals_main::save_load::snapshot::capture_lifecycle_tail;
    use glam::Vec3;

    fn roster(world: &GameLogic, producer: ObjectId) -> Vec<u32> {
        capture_lifecycle_tail(world)
            .expect("capture lifecycle tail")
            .spawn_behavior_rosters
            .expect("new-format save carries the roster section")
            .into_iter()
            .find(|roster| roster.producer_id == producer.0)
            .expect("supported SpawnBehavior producer is persisted, including its ordered children")
            .children
            .into_iter()
            .map(|child| child.child_id)
            .collect()
    }
fn retail_bones() -> std::sync::Mutex<generals_main::assets::AssetManager> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../windows_game");
    let mut assets = generals_main::assets::AssetManager::new().unwrap();
    for archive in [
        root.join("Command & Conquer Generals/W3D.big"),
        root.join("Command & Conquer Generals Zero Hour/Data/W3DZH.big"),
    ] {
        assert!(archive.exists(), "retail W3D fixture missing: {}", archive.display());
        assets.load_user_mods("", archive.to_str().unwrap()).unwrap();
    }
    std::sync::Mutex::new(assets)
}
fn advance_spawn_updates_with_assets(
    world: &mut GameLogic,
    assets: &std::sync::Mutex<generals_main::assets::AssetManager>,
) {
    // The initial deadline is zero: frame 0 cannot drain it, and the next
    // scheduler update runs at frame 15. The completed world frame is 16.
    for _ in 0..16 {
        world.tick_logic_frame_with_script_effects(
            1.0 / 30.0,
            None,
            None,
            generals_main::game_logic::ScriptPresentationEffects {
                pristine_bones: Some(assets),
                ..Default::default()
            },
            |_| {},
            |_| {},
        );
    }
}

#[test]
fn stinger_hive_damage_uses_receiving_roster_order_and_death_deadlines() {
    use generals_main::game_logic::host_base_defense::HostHiveDamageClass;

    fn stinger_world(site_body: &str, replacement_delay_ms: u32) -> GameLogic {
        let body_fields = if site_body.eq_ignore_ascii_case("HiveStructureBody") {
            " PropagateDamageTypesToSlavesWhenExisting = SMALL_ARMS SNIPER POISON RADIATION SURRENDER MICROWAVE\n SwallowDamageTypesIfSlavesNotExisting = SNIPER POISON SURRENDER\n"
        } else {
            ""
        };
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([(
                "StingerHiveOwner.ini".into(),
                format!("Object GLAStingerSite\n KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS\n Draw = W3DModelDraw DrawTag\n DefaultConditionState\n Model = UBStingerS\n End\n End\n Body = {site_body} SiteBody\n{body_fields} MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = SpawnBehavior SiteSpawn\n OneShot = No\n SpawnNumber = 3\n SpawnReplaceDelay = {replacement_delay_ms}\n SpawnTemplateName = GLAInfantryStingerSoldier\n End\n Behavior = SpawnPointProductionExitUpdate ExitTag\n SpawnPointBoneName = SpawnPoint\n End\nEnd\nObject GLAInfantryStingerSoldier\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody SoldierBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject HiveShooter\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody ShooterBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n").into(),
            )])
            .expect("receiving Stinger and source definitions are valid");
        world.add_player(Player::new(1, Team::GLA, "HiveOwner", true));
        world
    }

    let mut first = stinger_world("HiveStructureBody", 2000);
    let mut peer = stinger_world("HiveStructureBody", 30000);
    let site = first
        .create_object_for_player("GLAStingerSite", 1, Vec3::ZERO)
        .expect("Stinger SpawnBehavior admitted");
    let peer_site = peer
        .create_object_for_player("GLAStingerSite", 1, Vec3::ZERO)
        .expect("peer Stinger admitted");
    let assets = retail_bones();
    advance_spawn_updates_with_assets(&mut first, &assets);
    advance_spawn_updates_with_assets(&mut peer, &assets);
    assert_eq!(site, peer_site, "worlds deliberately reuse object IDs");
    let first_roster = roster(&first, site);
    let peer_roster = roster(&peer, peer_site);
    assert_eq!(first_roster.len(), 3);
    assert_eq!(peer_roster, first_roster);

    let source_position = first.host_object(ObjectId(first_roster[0])).unwrap().get_position();
    let peer_source_position = peer.host_object(ObjectId(peer_roster[0])).unwrap().get_position();
    let shooter = first
        .create_object_for_player("HiveShooter", 1, source_position)
        .expect("source admitted");
    let peer_shooter = peer
        .create_object_for_player("HiveShooter", 1, peer_source_position)
        .expect("peer source admitted");
    assert_eq!(shooter, peer_shooter);
    // Position the source at the first retail bone so nearest-slave
    // selection has an unambiguous authored geometry.
    let first_health = first
        .host_object(ObjectId(first_roster[0]))
        .unwrap()
        .health
        .current;
    let second_health = first
        .host_object(ObjectId(first_roster[1]))
        .unwrap()
        .health
        .current;
    first.apply_host_hive_damage_from(
        site,
        12.0,
        HostHiveDamageClass::PropagateToSlaves,
        Some((999.0, 999.0)), // position is resolved from the actual source Object
        Some(shooter),
    );
    assert_eq!(
        first
            .host_object(ObjectId(first_roster[0]))
            .unwrap()
            .health
            .current,
        first_health - 12.0
    );
    assert_eq!(
        first
            .host_object(ObjectId(first_roster[1]))
            .unwrap()
            .health
            .current,
        second_health
    );
    assert_eq!(
        peer.host_object(ObjectId(first_roster[0]))
            .unwrap()
            .health
            .current,
        100.0,
        "the same-ID peer roster is isolated"
    );

    // C++ HiveStructureBody does not run its source/propagate/swallow branch
    // if the source Object no longer resolves; the hit falls through to the
    // structure body even when the roster has live children.
    let site_health = first.host_object(site).unwrap().health.current;
    first.apply_host_hive_damage_from(
        site,
        7.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        None,
    );
    assert_eq!(
        first.host_object(site).unwrap().health.current,
        site_health - 7.0
    );

    first.apply_host_hive_damage_from(
        site,
        1_000.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(shooter),
    );
    assert_eq!(roster(&first, site), first_roster[1..]);
    let tail = capture_lifecycle_tail(&first).expect("capture ordered replacement deadline");
    let record = tail
        .spawn_behavior_rosters
        .unwrap()
        .into_iter()
        .find(|record| record.producer_id == site.0)
        .unwrap();
    assert_eq!(record.replacement_times, vec![76]);
    assert_eq!(roster(&peer, peer_site), peer_roster);
    peer.apply_host_hive_damage_from(
        peer_site,
        1_000.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(peer_shooter),
    );
    let peer_tail = capture_lifecycle_tail(&peer).expect("capture peer replacement deadline");
    let peer_record = peer_tail
        .spawn_behavior_rosters
        .unwrap()
        .into_iter()
        .find(|record| record.producer_id == peer_site.0)
        .unwrap();
    assert_eq!(peer_record.replacement_times, vec![916]);

    // A matching Stinger name and SpawnBehavior do not change an ActiveBody
    // into HiveStructureBody. C++ virtual dispatch therefore damages the
    // structure directly instead of redirecting the hit to a child.
    let mut active_body = stinger_world("ActiveBody", 30000);
    let active_site = active_body
        .create_object_for_player("GLAStingerSite", 1, Vec3::ZERO)
        .expect("ActiveBody Stinger-like object admitted");
    let active_source = active_body
        .create_object_for_player("HiveShooter", 1, Vec3::ZERO)
        .expect("ActiveBody test source admitted");
    advance_spawn_updates_with_assets(&mut active_body, &assets);
    let active_children = roster(&active_body, active_site);
    let active_child_health = active_body
        .host_object(ObjectId(active_children[0]))
        .unwrap()
        .health
        .current;
    let active_site_health = active_body.host_object(active_site).unwrap().health.current;
    active_body.apply_host_hive_damage_from(
        active_site,
        5.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(active_source),
    );
    assert_eq!(
        active_body
            .host_object(ObjectId(active_children[0]))
            .unwrap()
            .health
            .current,
        active_child_health,
        "ActiveBody must not enter the HiveStructureBody redirection path"
    );
    assert_eq!(
        active_body.host_object(active_site).unwrap().health.current,
        active_site_health - 5.0
    );

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .expect("capture complete Stinger child/deadline continuation");
    let mut restored = stinger_world("HiveStructureBody", 2000);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore ordered child identities and replacement deadline");
    assert_eq!(roster(&restored, site), first_roster[1..]);
    let restored_tail = capture_lifecycle_tail(&restored).expect("capture restored state");
    assert_eq!(
        restored_tail.spawn_behavior_rosters.unwrap()[0].replacement_times,
        vec![76],
        "replacement timing remains part of the receiving object's continuation"
    );
}

#[test]
fn hive_damage_masks_follow_receiving_body_and_source() {
    use generals_main::game_logic::host_base_defense::HostHiveDamageClass;
    fn make_world(propagate: &str, swallow: &str) -> GameLogic {
        let mut world = GameLogic::new();
        let ini = format!(
            "Object GLAStingerSite\n KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS\n Draw = W3DModelDraw DrawTag\n DefaultConditionState\n Model = UBStingerS\n End\n End\n Body = HiveStructureBody SiteBody\n PropagateDamageTypesToSlavesWhenExisting = {propagate}\n SwallowDamageTypesIfSlavesNotExisting = {swallow}\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = SpawnBehavior SiteSpawn\n OneShot = No\n SpawnNumber = 1\n SpawnReplaceDelay = 2000\n SpawnTemplateName = GLAInfantryStingerSoldier\n End\n Behavior = SpawnPointProductionExitUpdate ExitTag\n SpawnPointBoneName = SpawnPoint\n End\nEnd\nObject GLAInfantryStingerSoldier\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody SoldierBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject HiveShooter\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody ShooterBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        );
        world
            .admit_object_ini_sources([("HiveMasks.ini".into(), ini)])
            .unwrap();
        world.add_player(Player::new(1, Team::GLA, "Owner", true));
        world
    }
    let mut first = make_world("SMALL_ARMS", "SMALL_ARMS");
    let mut peer = make_world("SNIPER", "SMALL_ARMS");
    let site = first
        .create_object_for_player("GLAStingerSite", 1, Vec3::ZERO)
        .unwrap();
    let peer_site = peer
        .create_object_for_player("GLAStingerSite", 1, Vec3::ZERO)
        .unwrap();
    let assets = retail_bones();
    advance_spawn_updates_with_assets(&mut first, &assets);
    advance_spawn_updates_with_assets(&mut peer, &assets);
    let shooter = first
        .create_object_for_player("HiveShooter", 1, Vec3::ZERO)
        .unwrap();
    let peer_shooter = peer
        .create_object_for_player("HiveShooter", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!((site, shooter), (peer_site, peer_shooter));
    let first_child = ObjectId(roster(&first, site)[0]);
    let peer_child = ObjectId(roster(&peer, peer_site)[0]);
    assert_eq!(first_child, peer_child);
    // The same SMALL_ARMS hit propagates only in the first world; the peer's
    // swallow bit cannot apply unless the propagate bit is also set.
    first.apply_host_hive_damage_from(
        site,
        1000.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(shooter),
    );
    peer.apply_host_hive_damage_from(
        peer_site,
        10.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(peer_shooter),
    );
    assert!(roster(&first, site).is_empty());
    assert_eq!(peer.host_object(peer_child).unwrap().health.current, 100.0);
    assert_eq!(first.host_object(site).unwrap().health.current, 1000.0);
    assert_eq!(peer.host_object(peer_site).unwrap().health.current, 990.0);
    // With no child, a resolved source is swallowed; a missing source falls
    // through to the body even though both authored flags are set.
    first.apply_host_hive_damage_from(
        site,
        10.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(shooter),
    );
    assert_eq!(first.host_object(site).unwrap().health.current, 1000.0);
    first.apply_host_hive_damage_from(
        site,
        10.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        None,
    );
    assert_eq!(first.host_object(site).unwrap().health.current, 990.0);
}
}