//! The live helipad state issues goals; the locomotor alone changes pose.
//! C++ JetAIUpdate.cpp:1030–1084 disables its old position interpolation and
//! advances each of the two legs at an inclusive three-unit 3D threshold.

use super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use glam::Vec3;

// Authored locomotor definitions still use a process-owned catalog. Isolate
// that fixture dependency without resetting any other test's world or rules.
fn isolated(test: &str, run: impl FnOnce()) {
    isolated_at(module_path!(), test, run);
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::game_logic) fn isolated_at(module_path: &str, test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;

    const CHILD: &str = "GENERALS_HELIPAD_MOTION_CHILD";
    let module = module_path.split_once("::").unwrap().1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn exact helipad regression");
    // Drain both pipes while the child runs; verbose failure output must not
    // fill a pipe and turn an assertion into a misleading watchdog timeout.
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).expect("drain helipad child");
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll helipad child") {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            child.kill().expect("kill stalled helipad child");
            break child.wait().expect("reap stalled helipad child");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output = readers.map(|reader| {
        String::from_utf8_lossy(&reader.join().expect("join helipad reader")).into_owned()
    });
    assert!(
        !timed_out,
        "helipad child {exact} exceeded 30 seconds: {output:?}"
    );
    assert!(
        status.success(),
        "helipad child {exact} failed: {status}: {output:?}"
    );
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact child ran no regression: {exact}: {output:?}"
    );
}

#[cfg(target_arch = "wasm32")]
pub(in crate::game_logic) fn isolated_at(_: &str, _: &str, run: impl FnOnce()) {
    run();
}

pub(in crate::game_logic) fn authored_match() -> (GameLogic, ObjectId, ObjectId) {
    use game_engine::common::ini::ini_locomotor::load_locomotors_from_str;
    assert_eq!(
        load_locomotors_from_str(
            r#"
Locomotor HqO7b46HoverLocomotor
  Surfaces = AIR
  Speed = 30
  SpeedDamaged = 30
  TurnRate = 180
  Acceleration = 100
  AccelerationDamaged = 100
  Braking = 100
  Lift = 300
  LiftDamaged = 300
  SpeedLimitZ = 3
  Appearance = HOVER
  PreferredHeight = 100
  AirborneTargetingHeight = 20
  PreferredHeightDamping = 1
  AllowAirborneMotiveForce = Yes
  ZAxisBehavior = SURFACE_RELATIVE_HEIGHT
End
"#
        )
        .expect("authored hover locomotor"),
        1
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object HqO7b46Helipad
  Type = Structure
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1000
  End
  Behavior = ParkingPlaceBehavior ModuleTag_Parking
    NumRows = 1
    NumCols = 1
    ApproachHeight = 37
    LandingDeckHeightOffset = 4
    HasRunways = No
    ParkInHangars = No
    HealAmountPerSecond = 10
  End
End
Object HqO7b46Helicopter
  KindOf = PRELOAD SELECTABLE VEHICLE AIRCRAFT PRODUCED_AT_HELIPAD
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 220
  End
  Behavior = JetAIUpdate ModuleTag_AI
    NeedsRunway = No
  End
  Locomotor = SET_NORMAL HqO7b46HoverLocomotor
End
"#,
            "hq_o7b46_helipad.ini",
        )
        .expect("authored helipad and helicopter");
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "HelipadOwner", true));
    for name in ["HqO7b46Helipad", "HqO7b46Helicopter"] {
        let template = GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).expect("authored definition"),
            None,
        );
        logic.templates.insert(name.to_owned(), template);
    }
    let pad = logic
        .create_object("HqO7b46Helipad", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("authored helipad");
    let heli = logic
        .create_object("HqO7b46Helicopter", Team::USA, Vec3::new(40.0, 100.0, 40.0))
        .expect("authored helicopter");
    let object = logic.objects.get_mut(&heli).unwrap();
    object.producer_id = Some(pad);
    assert_eq!(object.owner_player_id, Some(0));
    assert!(GameLogic::object_is_produced_at_helipad(object));
    assert_eq!(object.loco_appearance, LocomotorAppearance::Hover);
    assert!((object.movement.max_speed - 30.0).abs() < 0.001);
    assert!((object.speed_limit_z - 3.0 / 30.0).abs() < 0.0001);
    // C++ defaults this targeting classification to INT_MAX; the takeoff
    // fixture must author an air target height rather than rely on AI's
    // temporary launch flag surviving actual locomotor updates.
    assert_eq!(object.airborne_targeting_height, 20);
    let command = GameCommand {
        command_type: CommandType::ReturnToBase,
        player_id: 0,
        command_id: 74_646,
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        selected_units: vec![heli],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command)
            .unwrap(),
        CommandResult::Success
    );
    let state = logic.heli_takeoff_or_landing[&heli];
    assert!(state.landing);
    assert_eq!(state.airfield_id, pad);
    assert_eq!(state.index, 0);
    assert!((state.path[0].y - state.path[1].y - 41.0).abs() < 0.001);
    assert_eq!(
        logic.objects[&heli].movement.target_position,
        Some(state.path[0])
    );
    (logic, pad, heli)
}

#[test]
fn authored_helipad_callback_issues_goal_without_integrating_pose() {
    isolated(
        "authored_helipad_callback_issues_goal_without_integrating_pose",
        || {
            let (mut logic, _, heli) = authored_match();
            let before = logic.objects[&heli].get_position();
            let goal = logic.heli_takeoff_or_landing[&heli].path[0];
            logic.tick_airfield_parking_heal();
            let object = &logic.objects[&heli];
            assert_eq!(
                object.get_position(),
                before,
                "AI callback must not move its owner"
            );
            assert_eq!(object.movement.target_position, Some(goal));
            assert!(object.precise_z_pos && object.ultra_accurate);
            assert_eq!(object.locomotor_goal_type, LocoGoalType::PositionExplicit);
            assert!(object.contained_by.is_none());
        },
    );
}

#[test]
fn authored_helipad_leg_threshold_is_inclusive_3d_without_pose_snap() {
    isolated(
        "authored_helipad_leg_threshold_is_inclusive_3d_without_pose_snap",
        || {
            let (mut logic, _, heli) = authored_match();
            let path = logic.heli_takeoff_or_landing[&heli].path;
            let outside = path[0] + Vec3::new(0.0, 3.01, 0.0);
            logic.objects.get_mut(&heli).unwrap().set_position(outside);
            logic.step_heli_takeoff_or_landing(heli);
            assert_eq!(logic.heli_takeoff_or_landing[&heli].index, 0);
            assert_eq!(logic.objects[&heli].get_position(), outside);
            let boundary = path[0] + Vec3::new(0.0, 3.0, 0.0);
            logic.objects.get_mut(&heli).unwrap().set_position(boundary);
            logic.step_heli_takeoff_or_landing(heli);
            assert_eq!(logic.heli_takeoff_or_landing[&heli].index, 1);
            assert_eq!(
                logic.objects[&heli].get_position(),
                boundary,
                "leg transition is not a pose snap"
            );
            assert!(logic.objects[&heli].contained_by.is_none());
            assert_eq!(logic.objects[&heli].movement.target_position, Some(path[0]));
            let settled = path[1] + Vec3::new(0.0, 3.0, 0.0);
            logic.objects.get_mut(&heli).unwrap().set_position(settled);
            logic.step_heli_takeoff_or_landing(heli);
            assert!(!logic.heli_takeoff_or_landing.contains_key(&heli));
            assert_eq!(logic.objects[&heli].ai_state, AIState::Docked);
            assert_eq!(
                logic.objects[&heli].get_position(),
                settled,
                "landing completion preserves observed pose"
            );
            assert!(!logic.objects[&heli].precise_z_pos);
        },
    );
}

#[test]
fn authored_helipad_full_frame_has_only_the_driving_locomotor_motion() {
    isolated(
        "authored_helipad_full_frame_has_only_the_driving_locomotor_motion",
        || {
            let (mut actual, _, heli) = authored_match();
            let (mut control, _, control_heli) = authored_match();
            assert_eq!(heli, control_heli);
            // Both matches retain the real command's authored target, flags,
            // locomotor and pose. Consume the already-installed RTB request
            // in both, so the control cannot reconstruct the callback while
            // isolating the second integration from ordinary full-frame work.
            actual
                .objects
                .get_mut(&heli)
                .unwrap()
                .return_to_base_requested = false;
            control
                .objects
                .get_mut(&heli)
                .unwrap()
                .return_to_base_requested = false;
            control.heli_takeoff_or_landing.remove(&heli);
            let before = actual.objects[&heli].get_position();
            for _ in 0..2 {
                let frame = actual.frame;
                actual.update_with_dt(LOGIC_FRAME_TIMESTEP);
                control.update_with_dt(LOGIC_FRAME_TIMESTEP);
                assert_eq!(actual.frame, frame + 1);
                assert_eq!(control.frame, actual.frame);
                assert_eq!(
                    actual.objects[&heli].get_position(),
                    control.objects[&heli].get_position(),
                    "same locomotor input must not gain a second movement in the later AI callback"
                );
            }
            assert_ne!(
                actual.objects[&heli].get_position(),
                before,
                "actual owned frame-1 movement must progress vertically"
            );
        },
    );
}

#[test]
fn authored_helipad_callbacks_use_the_driving_same_id_owner() {
    isolated(
        "authored_helipad_callbacks_use_the_driving_same_id_owner",
        || {
            let (mut first, _, heli) = authored_match();
            let (mut second, _, other) = authored_match();
            assert_eq!(heli, other);
            let first_goal = first.heli_takeoff_or_landing[&heli].path[0];
            let second_goal = second.heli_takeoff_or_landing[&other].path[0];
            let first_pose = first_goal + Vec3::new(0.0, 3.0, 0.0);
            let second_pose = second_goal + Vec3::new(0.0, 40.0, 0.0);
            first
                .objects
                .get_mut(&heli)
                .unwrap()
                .set_position(first_pose);
            second
                .objects
                .get_mut(&other)
                .unwrap()
                .set_position(second_pose);
            first.step_heli_takeoff_or_landing(heli);
            assert_eq!(first.heli_takeoff_or_landing[&heli].index, 1);
            assert_eq!(second.heli_takeoff_or_landing[&other].index, 0);
            assert_eq!(second.objects[&other].get_position(), second_pose);
            second.step_heli_takeoff_or_landing(other);
            assert_eq!(first.objects[&heli].get_position(), first_pose);
            assert_eq!(second.objects[&other].get_position(), second_pose);
            assert_eq!(first.heli_takeoff_or_landing[&heli].index, 1);
            assert_eq!(second.heli_takeoff_or_landing[&other].index, 0);
        },
    );
}
