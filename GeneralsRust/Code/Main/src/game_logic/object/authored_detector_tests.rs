//! C++ StealthDetectorUpdate is selected by the authored module list, never by
//! the template name (StealthDetectorUpdate.cpp:53-120; Object.cpp module
//! construction). Real receiving admission + object creation.
use crate::game_logic::{GameLogic, ObjectId, Team};

fn admit(world: &mut GameLogic, name: &str, modules: &str) {
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-detector.ini".into(),
                format!(
                    "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}End"
                ),
            )])
            .unwrap(),
        )
        .unwrap();
}

fn create(world: &mut GameLogic, name: &str) -> ObjectId {
    world
        .create_object(name, Team::USA, glam::Vec3::ZERO)
        .unwrap()
}

const DETECTOR: &str = "Behavior = StealthDetectorUpdate DetectorTag\n\
    DetectionRate = 900\n\
    DetectionRange = 123\n\
End\n";

#[test]
fn authored_stealth_detector_module_makes_any_template_a_detector() {
    let mut world = GameLogic::new();
    admit(&mut world, "PlainScoutCar", DETECTOR);
    let id = create(&mut world, "PlainScoutCar");
    let object = world.host_object(id).unwrap();
    assert!(object.is_detector);
    assert!((object.detection_range - 123.0).abs() < 1e-4);
    // 900 ms -> 27 logic frames (parseDurationUnsignedInt).
    assert_eq!(object.detection_rate_frames, 27);
}

#[test]
fn detector_named_template_without_module_does_not_detect() {
    let mut world = GameLogic::new();
    admit(&mut world, "AmericaInfantryPathfinder", "");
    let id = create(&mut world, "AmericaInfantryPathfinder");
    assert!(!world.host_object(id).unwrap().is_detector);
}

#[test]
fn initially_disabled_detector_does_not_scan() {
    let mut world = GameLogic::new();
    admit(
        &mut world,
        "DormantSensor",
        "Behavior = StealthDetectorUpdate DetectorTag\nDetectionRange = 50\nInitiallyDisabled = Yes\nEnd\n",
    );
    let id = create(&mut world, "DormantSensor");
    assert!(!world.host_object(id).unwrap().is_detector);
}

#[test]
fn tunnel_contain_capacity_comes_from_the_session_not_a_constant() {
    let mut world = GameLogic::new();
    world.tunnel_network.set_capacity(3);
    admit(
        &mut world,
        "PlainBurrow",
        "Behavior = TunnelContain ContainTag\nTimeForFullHeal = 5000\nEnd\n",
    );
    let id = create(&mut world, "PlainBurrow");
    let object = world.host_object(id).unwrap();
    assert!(object.is_tunnel_network_style_container());
    assert_eq!(object.garrison_capacity(), 3);
}

#[test]
fn authored_detector_constructor_staggers_first_scan() {
    use crate::game_logic::host_strategy_center::stealth_detector_scan_due;
    let mut world = GameLogic::new();
    admit(&mut world, "PlainScoutCar", DETECTOR);
    let a = create(&mut world, "PlainScoutCar");
    let b = create(&mut world, "PlainScoutCar");
    for id in [a, b] {
        let object = world.host_object(id).unwrap();
        let (next, rate) = (object.next_detection_scan_frame, object.detection_rate_frames);
        assert_eq!(rate, 27);
        // StealthDetectorUpdate ctor: GameLogicRandomValue(1, DetectionRate).
        assert!((1..=rate).contains(&next), "first scan {next} outside 1..={rate}");
        assert!(!stealth_detector_scan_due(rate, next, 0));
    }
}
