//! Garrison evacuation door tests.

use crate::game_logic::{AIState, GameLogic, KindOf, ThingTemplate};

#[test]
fn rappel_garrison_holds_rider_before_center_pose_in_each_world() {
    use crate::fixture_content::{admit_fixture_content, FixtureObject};
    use crate::game_logic::pathfinding::{GridPos, PathfindingSystem};
    use crate::game_logic::{Player, Team};
    use glam::Vec3;

    let setup = || {
        let mut world = GameLogic::new();
        admit_fixture_content(
            &mut world,
            &[
                FixtureObject::new("RappelPositionRider")
                    .kind_of("INFANTRY SELECTABLE")
                    .health(100.0)
                    .walker(),
                FixtureObject::new("RappelPositionBunker")
                    .kind_of("STRUCTURE")
                    .health(1000.0)
                    .module("Behavior = GarrisonContain Cargo\n ContainMax = 5\n AllowInsideKindOf = INFANTRY\nEnd"),
            ],
            &[],
        );
        world.add_player(Player::new(1, Team::USA, "Owner", true));
        world.pathfinding_system = PathfindingSystem::new(400.0, 400.0);
        world.pathfinding_system.map_ready = true;
        let building = world.create_object(
            "RappelPositionBunker", Team::USA, Vec3::new(100.0, 0.0, 100.0),
        ).unwrap();
        let rider = world.create_object(
            "RappelPositionRider", Team::USA, Vec3::new(30.0, 0.0, 100.0),
        ).unwrap();
        let actor = world.host_object_mut(rider).unwrap();
        actor.cur_locomotor_name = Some("ground".into());
        actor.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_GROUND;
        assert!(world.set_registered_object_position(rider, Vec3::new(40.0, 0.0, 100.0)));
        (world, building, rider)
    };
    let (mut world, building, rider) = setup();
    let (mut peer, peer_building, peer_rider) = setup();
    assert_eq!((building, rider), (peer_building, peer_rider));
    assert_eq!(world.pathfinding_ground_position(GridPos::new(4, 10)), Some(rider));
    world.rappel_add_to_contain(rider, building).unwrap();
    assert!(world.host_object(rider).unwrap().status.disabled_held);
    assert_eq!(world.host_object(rider).unwrap().get_position(), Vec3::new(100.0, 0.0, 100.0));
    assert_eq!(world.pathfinding_ground_position(GridPos::new(4, 10)), None);
    assert_eq!(world.pathfinding_ground_position(GridPos::new(10, 10)), None);
    assert_eq!(peer.pathfinding_ground_position(GridPos::new(4, 10)), Some(peer_rider));
    peer.rappel_add_to_contain(peer_rider, peer_building).unwrap();
    assert_eq!(peer.pathfinding_ground_position(GridPos::new(10, 10)), None);
}

#[test]
fn garrison_exit_occupant_via_door_bursts_from_center() {
    let mut logic = GameLogic::new();
    let mut bunker_t = ThingTemplate::new("GARR_DOOR");
    bunker_t.add_kind_of(KindOf::Structure).set_health(1000.0);
    bunker_t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Garrison,
        slots: Some(5),
        is_enclosing_container: true,
        ..Default::default()
    };
    bunker_t.geometry_info.authored = true;
    bunker_t.geometry_info.major_radius = 20.0;
    bunker_t.geometry_info.minor_radius = 10.0;
    logic.templates.insert("GARR_DOOR".into(), bunker_t);
    let mut pax_t = ThingTemplate::new("GARR_DOOR_P");
    pax_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GARR_DOOR_P".into(), pax_t);
    let origin = glam::Vec3::new(10.0, 0.0, 20.0);
    let bunker = logic
        .create_object("GARR_DOOR", crate::game_logic::Team::USA, origin)
        .unwrap();
    let pax = logic
        .create_object(
            "GARR_DOOR_P",
            crate::game_logic::Team::USA,
            glam::Vec3::new(12.0, 0.0, 20.0),
        )
        .unwrap();
    assert!(logic.garrison_exit_occupant_via_door(pax, bunker).unwrap());
    let p = logic.host_object(pax).unwrap();
    assert_eq!(p.ai_state(), AIState::Moving);
    let dest = p.movement.target_position.expect("door dest");
    assert!(
        (dest - origin).length() > 8.0,
        "burst dest must leave origin, not a 6-unit ring: dest={dest:?}"
    );
    assert!((p.get_position() - origin).length() < 2.0);
    assert_eq!(
            p.safe_occlusion_frame,
            logic.frame
                + crate::game_logic::host_gamedata_lobby_residual::DEFAULT_OCCLUSION_DELAY_FRAMES_RESIDUAL
                    as u32,
            "GarrisonContain::onRemoving must stamp OcclusionDelay"
        );
}

/// hq-ppag1: Fire Base exit snaps station height to Patch 1.01 ground Z.
#[test]
fn firebase_exit_snaps_station_height_to_ground() {
    let mut logic = GameLogic::new();
    let mut fb_t = ThingTemplate::new("AmericaFireBase");
    fb_t.add_kind_of(KindOf::Structure).set_health(1000.0);
    fb_t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Garrison,
        slots: Some(4),
        // GarrisonContain::exitObjectViaDoor (GarrisonContain.cpp:1555-1559)
        // snaps the station point to ground only for an enclosing container;
        // a non-enclosing one just walks the occupant away from where it is.
        is_enclosing_container: true,
        ..Default::default()
    };
    logic.templates.insert("AmericaFireBase".into(), fb_t);
    let mut pax_t = ThingTemplate::new("FB_PAX");
    pax_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("FB_PAX".into(), pax_t);
    let origin = glam::Vec3::new(10.0, 0.0, 20.0);
    let fb = logic
        .create_object("AmericaFireBase", crate::game_logic::Team::USA, origin)
        .unwrap();
    let pax = logic
        .create_object(
            "FB_PAX",
            crate::game_logic::Team::USA,
            glam::Vec3::new(12.0, 8.0, 22.0),
        )
        .unwrap();
    {
        let (p, fixture_partition_p) = logic.host_object_and_partition_mut(pax).unwrap();
        p.set_contained_by_with_partition_owner(Some(fb), true, fixture_partition_p);
        p.set_position_with_partition_owner(
            glam::Vec3::new(12.0, 8.0, 22.0),
            0,
            &mut Default::default(),
            fixture_partition_p,
        );
    }
    assert!(logic.garrison_exit_occupant_via_door(pax, fb).unwrap());
    let p = logic.host_object(pax).unwrap();
    assert!(
        (p.get_position().y - origin.y).abs() < 0.01,
        "Fire Base exit must snap station Y to ground, got y={}",
        p.get_position().y
    );

    let pax2 = logic
        .create_object(
            "FB_PAX",
            crate::game_logic::Team::USA,
            glam::Vec3::new(11.0, 9.0, 21.0),
        )
        .unwrap();
    assert!(logic.host_object_mut(fb).unwrap().add_occupant(pax2));
    if let Some((p, partition)) = logic.host_object_and_partition_mut(pax2) {
        p.set_contained_by_with_partition_owner(Some(fb), true, partition);
        p.set_position_with_partition_owner(
            glam::Vec3::new(11.0, 9.0, 21.0),
            0,
            &mut Default::default(),
            partition,
        );
    }
    assert!(logic.evacuate_container_now(fb, false).unwrap());
    let p2 = logic.host_object(pax2).unwrap();
    assert!(
        (p2.get_position().y - origin.y).abs() < 0.01,
        "Fire Base evac must snap station Y to ground, got y={}",
        p2.get_position().y
    );
}
