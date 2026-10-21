//! Actual receiving factory locomotors and owned logical terrain, not an
//! injected predicate or presentation-only altitude flag.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};

fn world(raw_ground: u8, water: Option<i32>) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "FollowOwner", true));
    terrain(&world, raw_ground, water);
    let mut parser = crate::assets::IniParser::new();
    let source_text = r#"
Object GroundFollow
 KindOf = VEHICLE SELECTABLE
 Locomotor = SET_NORMAL CrusaderLocomotor
 Behavior = AIUpdateInterface GroundAI
 End
End
Object AirFollow
 KindOf = VEHICLE SELECTABLE
 Locomotor = SET_NORMAL ComancheLocomotor
 Behavior = AIUpdateInterface AirAI
 End
End
"#;
    parser
        .parse_ini_content(source_text, "follow-airborne.ini")
        .unwrap();
    for name in ["GroundFollow", "AirFollow"] {
        let template = GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).unwrap(),
            None,
        );
        assert_eq!(template.authored_ai_update_interface(), Some(true));
        world.templates.insert(name.into(), template);
    }
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "follow-airborne.ini".into(),
                source_text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    let position = Vec3::new(40., 37., 40.);
    let ground = world
        .create_object_for_player("GroundFollow", 0, position)
        .unwrap();
    let air = world
        .create_object_for_player("AirFollow", 0, position)
        .unwrap();
    // Deliberately misleading targeting flags must not select camera heading.
    let object = world.host_object_mut(ground).unwrap();
    object.set_orientation(0.4 - std::f32::consts::PI);
    object.status.airborne_target = true;
    assert!(object.selected_locomotor_definition.is_some());
    assert!(!object.selected_locomotor_is_airborne());
    let object = world.host_object_mut(air).unwrap();
    object.set_orientation(0.4);
    object.status.airborne_target = false;
    assert!(object.selected_locomotor_definition.is_some());
    assert!(object.selected_locomotor_is_airborne());
    world.session_view.set_camera_follow(Some(air));
    (world, ground, air)
}
fn terrain(world: &GameLogic, raw_ground: u8, water: Option<i32>) {
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 16;
    map.height = 16;
    map.heightmap = vec![raw_ground; 256];
    let owner = world.world_services.terrain();
    let mut terrain = owner.write().unwrap_or_else(|error| error.into_inner());
    assert!(terrain.load_map_geometry(map).is_empty());
    let mut grid = gamelogic::terrain_water::WaterGridState::default();
    if let Some(height) = water {
        grid.enabled = true;
        grid.set_resolution(16., 16., 10.);
        grid.set_height(height as f32);
    }
    // Actual owned restore admission; no global water hooks or area tracker.
    terrain.restore_water_grid_state(grid);

    assert_eq!(
        terrain.get_ground_height(40., 40., None),
        f32::from(raw_ground) * 0.625
    );
}
fn altitude(world: &mut GameLogic, id: ObjectId, height: f32) {
    world
        .host_object_mut(id)
        .unwrap()
        .set_position(Vec3::new(40., height, 40.));
}
#[test]
fn actual_follow_freeze_uses_selected_air_surface_and_exact_colocated_identity() {
    let (mut a, ground, air) = world(32, None);
    let (mut peer, peer_ground, peer_air) = world(80, None);
    assert_eq!((ground, air), (peer_ground, peer_air));
    // Source raw height32*.625 = 20; equal peer ID/pose uses terrain50.
    let heading = a.host_object(air).unwrap().get_orientation();
    let facts = freeze(&a).unwrap();
    assert_eq!(facts.id, air);
    assert_eq!(facts.position, Vec3::new(40., 37., 40.));
    assert_eq!(facts.airborne_orientation, Some(heading));
    let frame = PresentationFrame::build_from_logic(&mut a, 0);
    assert_eq!(
        frame.camera_follow_facts,
        Some(facts),
        "production frame builder freezes the exact receiving owner facts"
    );
    assert_eq!(freeze(&peer).unwrap().airborne_orientation, None);
    // Both exact IDs occupy the same coordinate with opposite headings. The
    // ground locomotor must not acquire orientation from its airborne neighbor.
    a.session_view.set_camera_follow(Some(ground));
    assert_eq!(freeze(&a).unwrap().id, ground);
    assert_eq!(freeze(&a).unwrap().airborne_orientation, None);
    a.session_view.set_camera_follow(Some(air));
    assert_eq!(freeze(&a).unwrap().airborne_orientation, Some(heading));
    // Source isAboveTerrainOrWater is strict >0, including exact zero.
    for height in [19., 20.] {
        altitude(&mut a, air, height);
        assert_eq!(freeze(&a).unwrap().airborne_orientation, None);
    }
    altitude(&mut a, air, 20.25);
    assert_eq!(freeze(&a).unwrap().airborne_orientation, Some(heading));
    // A later peer terrain mutation changes only the peer's frozen predicate.
    terrain(&peer, 0, None);
    // The actual setter also invalidates C++ Thing's altitude cache after
    // terrain replacement; do not assert a different cache lifetime.
    altitude(&mut peer, peer_air, 37.);
    assert_eq!(
        freeze(&peer).unwrap().airborne_orientation,
        Some(peer.host_object(peer_air).unwrap().get_orientation())
    );
    assert_eq!(freeze(&a).unwrap().airborne_orientation, Some(heading));
    assert_eq!(
        facts.position,
        Vec3::new(40., 37., 40.),
        "earlier immutable facts do not follow later setters"
    );
}
#[test]
fn actual_follow_freeze_uses_water_only_above_ground_and_keeps_peer_surface_owned() {
    let (mut a, _, air) = world(32, Some(30)); // water30 above ground20
    let (mut peer, _, peer_air) = world(32, Some(10)); // water below ground: use terrain20
    let heading = a.host_object(air).unwrap().get_orientation();
    for height in [25., 30.] {
        altitude(&mut a, air, height);
        assert_eq!(freeze(&a).unwrap().airborne_orientation, None);
    }
    altitude(&mut peer, peer_air, 25.);
    assert!(freeze(&peer).unwrap().airborne_orientation.is_some());
    altitude(&mut a, air, 30.25);
    assert_eq!(freeze(&a).unwrap().airborne_orientation, Some(heading));
    // Equal dry-ground surface is still not airborne in the peer world.
    altitude(&mut peer, peer_air, 20.);
    assert_eq!(freeze(&peer).unwrap().airborne_orientation, None);
    assert_eq!(freeze(&a).unwrap().airborne_orientation, Some(heading));
}

#[cfg(feature = "game_client")]
#[path = "follow_debug_owner_tests.rs"]
mod debug_owner;
