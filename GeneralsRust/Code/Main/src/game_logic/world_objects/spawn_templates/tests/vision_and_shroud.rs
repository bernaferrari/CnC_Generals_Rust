use super::*;

#[test]
fn crate_vision_uses_shroud_clearing_range_and_unlooks_on_move() {
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut tpl = ThingTemplate::new("VisionProbe");
    tpl.sight_range = 100.0;
    tpl.shroud_clearing_range = 240.0;
    logic.templates.insert("VisionProbe".into(), tpl);
    let id = logic
        .create_object_for_player("VisionProbe", 1, Vec3::new(10.0, 0.0, 20.0))
        .expect("spawn");

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    logic.update_main_crate_vision();
    let first = *logic.vision_last_looks.get(&id).expect("looker");
    assert!((first.3 - 240.0).abs() < 0.01, "look radius {}", first.3);
    logic.update_main_crate_vision();
    assert_eq!(logic.vision_last_looks.len(), 1);

    logic.set_fixture_object_position(id, Vec3::new(80.0, 0.0, 90.0));
    logic.update_main_crate_vision();
    let moved = *logic.vision_last_looks.get(&id).expect("moved looker");
    assert!((moved.0 - 80.0).abs() < 0.01);
    assert!((moved.1 - 90.0).abs() < 0.01);
    assert!((moved.3 - 240.0).abs() < 0.01);

    if let Some(obj) = logic.host_object_mut(id) {
        obj.health.current = 0.0;
    }
    logic.update_main_crate_vision();
    assert!(
        !logic.vision_last_looks.contains_key(&id),
        "death must unlook"
    );
}

/// C++ PartitionManager.cpp:1582-1688 — object FOW is footprint COI mix.
#[test]
fn object_fow_uses_coi_mix_not_vision_range_circle() {
    use gamelogic::common::types::ObjectShroudStatus;
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::China, "China", false));

    let mut looker = ThingTemplate::new("Looker");
    looker.sight_range = 100.0;
    looker.shroud_clearing_range = 80.0;
    logic.templates.insert("Looker".into(), looker);

    let mut scout = ThingTemplate::new("EnemyScout");
    scout.add_kind_of(KindOf::Infantry).set_health(80.0);
    logic.templates.insert("EnemyScout".into(), scout);

    let mut bunker = ThingTemplate::new("EnemyBunker");
    bunker
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .set_health(400.0);
    bunker.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Box,
        is_small: false,
        height: 20.0,
        major_radius: 20.0,
        minor_radius: 20.0,
        authored: true,
    };
    logic.templates.insert("EnemyBunker".into(), bunker);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let _looker_id = logic
        .create_object_for_player("Looker", 1, Vec3::new(100.0, 0.0, 100.0))
        .expect("looker");
    let near_id = logic
        .create_object_for_player("EnemyScout", 2, Vec3::new(110.0, 0.0, 100.0))
        .expect("near");
    let far_id = logic
        .create_object_for_player("EnemyScout", 2, Vec3::new(400.0, 0.0, 100.0))
        .expect("far");
    let bunker_id = logic
        .create_object_for_player("EnemyBunker", 2, Vec3::new(110.0, 0.0, 110.0))
        .expect("bunker");

    logic.update_main_crate_vision();
    {
        let shroud = logic.world_services.shroud();
        let mgr = shroud.lock().expect("shroud");
        assert_eq!(
            mgr.get_host_object_shroud_status(1, near_id.0),
            Some(ObjectShroudStatus::Clear),
            "enemy on revealed cells is CLEAR"
        );
        assert_eq!(
            mgr.get_host_object_shroud_status(1, far_id.0),
            Some(ObjectShroudStatus::Shrouded),
            "enemy on unrevealed cells is SHROUDED, not a VisionRange ghost"
        );
        assert_eq!(
            mgr.get_host_object_shroud_status(1, bunker_id.0),
            Some(ObjectShroudStatus::Clear)
        );
        assert!(mgr.host_object_ever_seen(1, bunker_id.0));
    }

    logic.set_fixture_object_position(_looker_id, Vec3::new(400.0, 0.0, 400.0));
    let moved_frame = logic.frame;
    logic.update_main_crate_vision();
    // C++ PartitionManager.cpp:3971,4026-4037 queues the old reveal for
    // frame + UnlookPersistDuration and expires it strictly afterward.
    let persist = super::host_unlook_persist_frames();
    assert_eq!(persist, 150);
    for frame in [moved_frame, moved_frame.wrapping_add(persist)] {
        logic.frame = frame;
        logic.update_main_crate_vision();
        let shroud = logic.world_services.shroud();
        let mgr = shroud.lock().expect("shroud");
        assert_eq!(
            mgr.get_host_object_shroud_status(1, near_id.0),
            Some(ObjectShroudStatus::Clear),
            "old reveal persists through its inclusive deadline"
        );
        assert_eq!(
            mgr.get_host_object_shroud_status(1, bunker_id.0),
            Some(ObjectShroudStatus::Clear)
        );
    }
    logic.frame = moved_frame.wrapping_add(persist).wrapping_add(1);
    logic.update_main_crate_vision();
    {
        let shroud = logic.world_services.shroud();
        let mgr = shroud.lock().expect("shroud");
        assert_eq!(
            mgr.get_host_object_shroud_status(1, near_id.0),
            Some(ObjectShroudStatus::Shrouded),
            "mobile enemy in fog must not linger as a fog ghost"
        );
        assert_eq!(
            mgr.get_host_object_shroud_status(1, bunker_id.0),
            Some(ObjectShroudStatus::Fogged),
            "seen immobile enemy building stays FOGGED ghost"
        );
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn host_coi_visibility_and_unlook_expiry_are_instance_owned() {
    use gamelogic::common::ObjectShroudStatus;

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "host_coi_visibility_and_unlook_expiry_are_instance_owned",
        || {
            let world = |range| {
                let mut logic = GameLogic::new();
                logic.add_player(Player::new(1, Team::USA, "Viewer", true));
                logic.add_player(Player::new(2, Team::China, "Enemy", false));
                let mut looker = ThingTemplate::new("OwnedCoiLooker");
                looker.shroud_clearing_range = range;
                logic.templates.insert(looker.name.clone(), looker);
                let mut enemy = ThingTemplate::new("OwnedCoiEnemy");
                enemy.add_kind_of(KindOf::Infantry);
                enemy.shroud_clearing_range = 0.0;
                logic.templates.insert(enemy.name.clone(), enemy);
                logic
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .init_shroud_grid(512.0, 512.0);
                let looker_id = logic
                    .create_object_for_player("OwnedCoiLooker", 1, Vec3::new(120.0, 0.0, 120.0))
                    .unwrap();
                let enemy_id = logic
                    .create_object_for_player("OwnedCoiEnemy", 2, Vec3::new(130.0, 0.0, 120.0))
                    .unwrap();
                (logic, looker_id, enemy_id)
            };
            let status = |logic: &GameLogic, id: ObjectId| {
                logic
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(1, id.0)
            };
            let (mut a, a_looker, a_enemy) = world(80.0);
            let (mut b, b_looker, b_enemy) = world(0.0);
            assert_eq!((a_looker, a_enemy), (b_looker, b_enemy));

            // Neither stamping nor footprint sampling may touch the
            // reference runtime's process partition. The exact child
            // watchdog bounds the OLD route's recursive-lock failure.
            let foreign_partition =
                gamelogic::object::collide::partition_manager::world_partition_manager();
            let _foreign_partition = foreign_partition.write().unwrap();
            a.update_main_crate_vision();
            b.update_main_crate_vision();
            assert_eq!(status(&a, a_enemy), Some(ObjectShroudStatus::Clear));
            assert_eq!(status(&b, b_enemy), Some(ObjectShroudStatus::Shrouded));

            a.set_fixture_object_position(a_looker, Vec3::new(400.0, 0.0, 400.0));
            a.update_main_crate_vision();
            let pending = a.world_services.shroud().lock().unwrap().snapshot_state();
            assert_eq!(pending.pending_undo_shroud_reveals.len(), 1);
            assert_eq!(pending.pending_undo_shroud_reveals[0].expiration_frame, 150);
            b.frame = 200;
            b.update_main_crate_vision();
            assert_eq!(status(&b, b_enemy), Some(ObjectShroudStatus::Shrouded));
            assert_eq!(
                a.world_services.shroud().lock().unwrap().snapshot_state(),
                pending,
                "B's frame cannot expire A's reveal"
            );

            // C++ PartitionManager.cpp:3971 expires only after deadline.
            a.frame = 150;
            a.update_main_crate_vision();
            assert_eq!(status(&a, a_enemy), Some(ObjectShroudStatus::Clear));
            drop(_foreign_partition);
            b.reset();
            drop(b);
            assert_eq!(
                a.world_services.shroud().lock().unwrap().snapshot_state(),
                pending,
                "foreign reset/destruction cannot alter A's counters or queue"
            );
            a.frame = 151;
            a.update_main_crate_vision();
            assert_eq!(status(&a, a_enemy), Some(ObjectShroudStatus::Shrouded));
            assert!(
                a.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state()
                    .pending_undo_shroud_reveals
                    .is_empty()
            );
        },
    );
}

/// hq-rxwoc: object FOW mix reads leftover DiscreteCircle looker cells,
/// not world_partition_manager()'s square `±ceil(r/40)` + world `r²` reject.
#[test]
fn object_fow_mix_reads_leftover_discrete_circle_not_square() {
    use gamelogic::common::types::ObjectShroudStatus;
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::China, "China", false));

    let mut looker = ThingTemplate::new("CircleLooker");
    looker.sight_range = 100.0;
    looker.shroud_clearing_range = 240.0;
    logic.templates.insert("CircleLooker".into(), looker);

    let mut scout = ThingTemplate::new("RimScout");
    scout.add_kind_of(KindOf::Infantry).set_health(80.0);
    scout.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Sphere,
        is_small: true,
        height: 2.0,
        major_radius: 1.0,
        minor_radius: 1.0,
        authored: true,
    };
    logic.templates.insert("RimScout".into(), scout);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let _looker_id = logic
        .create_object_for_player("CircleLooker", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("looker");
    // Cell (5, 3): DiscreteCircle radius 6 includes the y=3 span x=±5.
    // Square world r² from (0,0) with r=240 excludes cell-center (220,140).
    let rim_id = logic
        .create_object_for_player("RimScout", 2, Vec3::new(210.0, 0.0, 130.0))
        .expect("rim");

    logic.update_main_crate_vision();
    {
        let shroud = logic.world_services.shroud();
        let mgr = shroud.lock().expect("shroud");
        assert_eq!(
            mgr.get_host_object_shroud_status(1, rim_id.0),
            Some(ObjectShroudStatus::Clear),
            "rim cell on leftover DiscreteCircle must be CLEAR, not square-disk SHROUDED"
        );
    }
}

#[test]
fn looker_mask_uses_player_relationship_and_unlook_persist_150() {
    use glam::Vec3;

    assert_eq!(super::host_unlook_persist_frames(), 150);

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.alliance_team = 7;
    let mut china = Player::new(1, Team::China, "China", false);
    china.alliance_team = 7;
    logic.add_player(usa);
    logic.add_player(china);

    let mut tpl = ThingTemplate::new("AllyLooker");
    tpl.sight_range = 50.0;
    tpl.shroud_clearing_range = 80.0;
    logic.templates.insert("AllyLooker".into(), tpl);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let id = logic
        .create_object_for_player("AllyLooker", 0, Vec3::new(10.0, 0.0, 10.0))
        .expect("spawn");
    logic.update_main_crate_vision();
    let look = *logic.vision_last_looks.get(&id).expect("looker");
    assert_ne!(look.4 & (1u32 << 0), 0, "owner bit");
    assert_ne!(
        look.4 & (1u32 << 1),
        0,
        "script/skirmish ally must share look"
    );
}

#[test]
fn transport_passengers_stop_looking() {
    use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));

    let mut chinook = ThingTemplate::new("ChinookLook");
    chinook.shroud_clearing_range = 200.0;
    chinook.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(8),
        ..ContainModuleMetadata::default()
    };
    logic.templates.insert("ChinookLook".into(), chinook);

    let mut ranger = ThingTemplate::new("RangerLook");
    ranger.shroud_clearing_range = 150.0;
    logic.templates.insert("RangerLook".into(), ranger);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let bird = logic
        .create_object_for_player("ChinookLook", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("chinook");
    let rider = logic
        .create_object_for_player("RangerLook", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    logic.set_fixture_object_contained_by(rider, Some(bird));
    logic.update_main_crate_vision();
    assert!(
        logic.vision_last_looks.contains_key(&bird),
        "container still looks"
    );
    assert!(
        !logic.vision_last_looks.contains_key(&rider),
        "transport passenger must not look"
    );
}

#[test]
fn shroud_reveal_to_all_range_looks_for_enemies() {
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::China, "China", false));

    let mut tpl = ThingTemplate::new("StratCenter");
    tpl.shroud_clearing_range = 200.0;
    tpl.shroud_reveal_to_all_range = 50.0;
    logic.templates.insert("StratCenter".into(), tpl);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let id = logic
        .create_object_for_player("StratCenter", 0, Vec3::new(20.0, 0.0, 20.0))
        .expect("center");
    logic.update_main_crate_vision();
    let reveal = *logic.vision_last_reveal_all.get(&id).expect("reveal-all");
    assert!((reveal.3 - 50.0).abs() < 0.01);
    assert_ne!(reveal.4 & (1u32 << 1), 0, "enemy bit in reveal-all mask");
    assert_eq!(reveal.4 & (1u32 << 0), 0, "owner is not enemies/neutral");

    if let Some(obj) = logic.host_object_mut(id) {
        obj.status.stealthed = true;
        obj.status.detected = false;
        obj.status.disguised = false;
    }
    logic.update_main_crate_vision();
    assert!(
        !logic.vision_last_reveal_all.contains_key(&id),
        "stealthed-not-detected skips reveal-to-all"
    );
}

/// C++ Object.cpp:4961-4962 — vision-spied units look for the spy.
#[test]
fn crate_vision_spied_mask_makes_moving_looker() {
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::China, "China", false));

    let mut tpl = ThingTemplate::new("SpiedScout");
    tpl.sight_range = 50.0;
    tpl.shroud_clearing_range = 80.0;
    logic.templates.insert("SpiedScout".into(), tpl);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let id = logic
        .create_object_for_player("SpiedScout", 1, Vec3::new(10.0, 0.0, 20.0))
        .expect("spawn");
    if let Some((obj, partition)) = logic.host_object_and_partition_mut(id) {
        obj.set_vision_spied_by_player_with_partition_owner(0, true, partition);
    }
    logic.update_main_crate_vision();
    let look = *logic.vision_last_looks.get(&id).expect("looker");
    assert_ne!(look.4 & (1u32 << 1), 0, "owner still looks for self");
    assert_ne!(
        look.4 & (1u32 << 0),
        0,
        "vision_spied_mask must OR the spy into looking_mask"
    );
    assert!((look.3 - 80.0).abs() < 0.01);

    logic.set_fixture_object_position(id, Vec3::new(90.0, 0.0, 110.0));
    logic.update_main_crate_vision();
    let moved = *logic.vision_last_looks.get(&id).expect("moved looker");
    assert!((moved.0 - 90.0).abs() < 0.01);
    assert!((moved.1 - 110.0).abs() < 0.01);
    assert_ne!(moved.4 & (1u32 << 0), 0, "spy look follows the unit");
}

/// C++ Object.cpp:5128-5140 — UC look is bounding-circle, not stored range.
#[test]
fn crate_vision_under_construction_uses_bounding_circle() {
    use crate::game_logic::{HostGeometryInfo, HostGeometryType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));

    let mut tpl = ThingTemplate::new("WarFactoryPad");
    tpl.sight_range = 100.0;
    tpl.shroud_clearing_range = 240.0;
    tpl.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 20.0,
        major_radius: 10.0,
        minor_radius: 10.0,
        authored: true,
    };
    logic.templates.insert("WarFactoryPad".into(), tpl);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let id = logic
        .create_object_for_player("WarFactoryPad", 1, Vec3::new(40.0, 0.0, 40.0))
        .expect("spawn");
    let expected_circle = (10.0_f32 * 10.0 + 10.0 * 10.0).sqrt();
    if let Some(obj) = logic.host_object_mut(id) {
        obj.status.under_construction = true;
        obj.shroud_clearing_range = 240.0;
    }
    logic.update_main_crate_vision();
    let uc = *logic.vision_last_looks.get(&id).expect("uc looker");
    assert!(
        (uc.3 - expected_circle).abs() < 0.01,
        "UC look must be bounding circle {}, got {}",
        expected_circle,
        uc.3
    );

    if let Some(obj) = logic.host_object_mut(id) {
        obj.status.under_construction = false;
    }
    logic.update_main_crate_vision();
    let done = *logic.vision_last_looks.get(&id).expect("complete looker");
    assert!(
        (done.3 - 240.0).abs() < 0.01,
        "completed look uses stored ShroudClearingRange, got {}",
        done.3
    );
}

/// C++ Object.cpp:5045-5080 — live look applies Object::shroud cover.
#[test]
fn crate_vision_applies_object_shroud_cover_to_non_allies() {
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::China, "China", false));
    let mut ally = Player::new(2, Team::GLA, "GLA", false);
    ally.alliance_team = 0;
    if let Some(usa) = logic.players.get_mut(&0) {
        usa.alliance_team = 0;
    }
    logic.add_player(ally);

    let mut tpl = ThingTemplate::new("CoverVan");
    tpl.sight_range = 50.0;
    tpl.shroud_clearing_range = 80.0;
    logic.templates.insert("CoverVan".into(), tpl);

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.init_shroud_grid(512.0, 512.0);
    }

    let id = logic
        .create_object_for_player("CoverVan", 0, Vec3::new(30.0, 0.0, 40.0))
        .expect("spawn");
    logic.update_main_crate_vision();
    assert!(
        !logic.vision_last_shroud.contains_key(&id),
        "default ShroudRange 0 must not cover"
    );

    assert!(logic.apply_active_shroud_upgrade(id, 175.0));
    let cover = *logic
        .vision_last_shroud
        .get(&id)
        .expect("upgrade must restamp cover immediately");
    assert!((cover.0 - 30.0).abs() < 0.01);
    assert!((cover.1 - 40.0).abs() < 0.01);
    assert!((cover.3 - 175.0).abs() < 0.01);
    assert_eq!(cover.4 & (1u32 << 0), 0, "owner is Allies, not covered");
    assert_ne!(cover.4 & (1u32 << 1), 0, "enemy must be in shrouding mask");
    assert_eq!(
        cover.4 & (1u32 << 2),
        0,
        "script/skirmish ally must see through"
    );

    logic.set_fixture_object_position(id, Vec3::new(90.0, 0.0, 110.0));
    logic.update_main_crate_vision();
    let moved = *logic.vision_last_shroud.get(&id).expect("moved cover");
    assert!((moved.0 - 90.0).abs() < 0.01);
    assert!((moved.1 - 110.0).abs() < 0.01);
    assert!((moved.3 - 175.0).abs() < 0.01);
    assert_ne!(moved.4 & (1u32 << 1), 0);

    if let Some(obj) = logic.host_object_mut(id) {
        obj.status.under_construction = true;
    }
    logic.update_main_crate_vision();
    assert!(
        !logic.vision_last_shroud.contains_key(&id),
        "UNDER_CONSTRUCTION must unshroud"
    );

    if let Some(obj) = logic.host_object_mut(id) {
        obj.status.under_construction = false;
    }
    logic.update_main_crate_vision();
    assert!(logic.vision_last_shroud.contains_key(&id));

    if let Some(obj) = logic.host_object_mut(id) {
        obj.health.current = 0.0;
    }
    logic.update_main_crate_vision();
    assert!(
        !logic.vision_last_shroud.contains_key(&id),
        "death must unshroud"
    );
}
