//! Garrison evacuation door tests.

use crate::game_logic::{AIState, GameLogic, KindOf, ThingTemplate};

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
    assert!(logic.garrison_exit_occupant_via_door(pax, bunker));
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
        is_enclosing_container: false,
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
    assert!(logic.garrison_exit_occupant_via_door(pax, fb));
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
    assert!(logic.evacuate_container_now(fb, false));
    let p2 = logic.host_object(pax2).unwrap();
    assert!(
        (p2.get_position().y - origin.y).abs() < 0.01,
        "Fire Base evac must snap station Y to ground, got y={}",
        p2.get_position().y
    );
}
