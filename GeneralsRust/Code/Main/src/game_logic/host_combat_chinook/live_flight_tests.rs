//! Real command and owned-frame regressions for Chinook flight/locomotor ownership.

use super::flight_fixture::{authored_match, combat_drop};
use super::{HostChinookAIState, HostChinookFlightStatus};
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{AIState, LocoGoalType, Team};
use glam::Vec3;

// The existing authored locomotor catalog is process-owned. Each regression
// owns a fresh child process, so its unique catalog definition never escapes
// into another test. Mutable match state below remains owned by GameLogic.
#[cfg(not(target_arch = "wasm32"))]
fn isolated(test: &str, run: impl FnOnce()) {
    const CHILD: &str = "GENERALS_CHINOOK_FLIGHT_CHILD";
    if std::env::var(CHILD).ok().as_deref() == Some(test) {
        run();
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let exact = format!("{module}::{test}");
    use std::io::Read;
    use std::process::Stdio;
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, test)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn exact Chinook regression");
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll child") {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().expect("reap stalled Chinook child");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output = readers.map(|reader| {
        String::from_utf8_lossy(&reader.join().expect("drain child output")).into_owned()
    });
    assert!(
        !timed_out,
        "Chinook child {exact} exceeded 30 seconds: {output:?}"
    );
    assert!(
        status.success(),
        "Chinook child {exact} failed: {status}: {output:?}"
    );
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact Chinook child must run one regression: {output:?}"
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_combat_drop_full_frame_obeys_one_locomotor_step() {
    isolated(
        "authored_combat_drop_full_frame_obeys_one_locomotor_step",
        || {
            let (mut logic, transport, passenger) = authored_match();
            combat_drop(&mut logic, transport);
            let before = logic.host_object(transport).unwrap().get_position();
            let frame = logic.frame;
            logic.update_with_dt(1.0 / 30.0);
            assert_eq!(logic.frame, frame + 1, "actual full logic frame advanced");
            let object = logic.host_object(transport).unwrap();
            let after = object.get_position();
            let horizontal = Vec3::new(after.x - before.x, 0.0, after.z - before.z).length();
            assert!(
                horizontal <= 30.0 / 30.0 + 0.001,
                "one authored locomotor step, not a second hardcoded flight move: {before:?} -> {after:?}"
            );
            assert_eq!(
                logic.host_object(passenger).unwrap().contained_by,
                Some(transport)
            );
            assert_eq!(
                object.chinook_ai.as_ref().unwrap().state,
                HostChinookAIState::MoveToCombatDrop
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_flight_callback_observes_pose_without_integrating_again() {
    isolated(
        "live_flight_callback_observes_pose_without_integrating_again",
        || {
            let (mut a, transport, passenger) = authored_match();
            let (mut b, other, _) = authored_match();
            assert_eq!(
                transport, other,
                "two matches intentionally reuse external IDs"
            );
            combat_drop(&mut a, transport);
            combat_drop(&mut b, other);
            let before_a = a.host_object(transport).unwrap().get_position();
            let before_b = b.host_object(other).unwrap().get_position();
            a.tick_chinook_ai(1.0 / 30.0);
            b.tick_chinook_ai(1.0);
            a.tick_chinook_ai(0.0);
            assert_eq!(a.host_object(transport).unwrap().get_position(), before_a);
            assert_eq!(b.host_object(other).unwrap().get_position(), before_b);
            assert_eq!(
                a.host_object(passenger).unwrap().contained_by,
                Some(transport)
            );
            drop(b);
            a.tick_chinook_ai(1.0 / 30.0);
            assert_eq!(a.host_object(transport).unwrap().get_position(), before_a);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_takeoff_publishes_explicit_goal_and_preserves_pose() {
    isolated(
        "live_takeoff_publishes_explicit_goal_and_preserves_pose",
        || {
            let (mut logic, transport, _) = authored_match();
            let before = logic.host_object(transport).unwrap().get_position();
            let object = logic.host_object_mut(transport).unwrap();
            let ai = object.chinook_ai.as_mut().unwrap();
            ai.state = HostChinookAIState::TakingOff;
            ai.flight_status = HostChinookFlightStatus::TakingOff;
            ai.dest = [before.x, before.z, before.y + 20.0];
            logic.tick_chinook_ai(1.0 / 30.0);
            let object = logic.host_object(transport).unwrap();
            assert_eq!(object.get_position(), before);
            assert_eq!(object.locomotor_goal_type, LocoGoalType::PositionExplicit);
            assert_eq!(
                object.movement.target_position,
                Some(before + Vec3::Y * 20.0)
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_takeoff_exit_arms_original_position_path_without_integrating() {
    isolated(
        "live_takeoff_exit_arms_original_position_path_without_integrating",
        || {
            let (mut logic, transport, _) = authored_match();
            let object = logic.host_object_mut(transport).unwrap();
            let ai = object.chinook_ai.as_mut().unwrap();
            let original = [-40.0, 40.0, 100.0];
            ai.original_pos = original;
            ai.begin_takeoff_and_exit();
            // The real locomotor has reached the precise takeoff goal.
            // Retain its MOVING flag to check that the new state replaces the
            // old goal even before the movement phase clears that flag.
            let arrival = Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]);
            object.set_position(arrival);
            object.status.moving = true;
            object.set_locomotor_goal_position_explicit(arrival);
            logic.tick_chinook_ai(1.0 / 30.0);
            let object = logic.host_object(transport).unwrap();
            assert_eq!(object.get_position(), arrival);
            assert_eq!(
                object.chinook_ai.as_ref().unwrap().state,
                HostChinookAIState::HeadOffMap
            );
            let goal = Vec3::new(original[0], original[2], original[1]);
            assert_eq!(object.movement.path.last(), Some(&goal));
            assert_eq!(object.movement.target_position, Some(goal));
            assert_eq!(object.locomotor_goal_type, LocoGoalType::PositionOnPath);
            assert!(object.allow_invalid_position);
            assert!(!object.precise_z_pos && !object.ultra_accurate);
            logic.tick_chinook_ai(1.0);
            assert_eq!(
                logic.host_object(transport).unwrap().get_position(),
                arrival
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_evacuation_retains_passenger_through_takeoff_and_landing() {
    isolated(
        "authored_evacuation_retains_passenger_through_takeoff_and_landing",
        || {
            let (mut logic, transport, passenger) = authored_match();
            let second = logic
                .create_object("Hq46k12Rappeller", Team::USA, Vec3::new(35.0, 0.0, 40.0))
                .unwrap();
            assert!(logic
                .host_object_mut(transport)
                .unwrap()
                .add_occupant(second));
            let rider = logic.host_object_mut(second).unwrap();
            rider.set_contained_by(Some(transport));
            rider.set_ai_state(AIState::Docked);
            // The initial condition is a landed, loaded transport. Every
            // subsequent pose comes from the actual full-frame locomotor.
            let object = logic.host_object_mut(transport).unwrap();
            let landed = Vec3::new(40.0, 0.0, 40.0);
            object.set_position(landed);
            // removeAllContained(FALSE) is immediate even when normal
            // passenger ExitBusy/ExitDelay streaming is unavailable.
            object.frame_exit_not_busy = 10_000;
            let ai = object.chinook_ai.as_mut().unwrap();
            ai.pos = [landed.x, landed.z, landed.y];
            ai.state = HostChinookAIState::Idle;
            ai.flight_status = HostChinookFlightStatus::Landed;
            let destination = Vec3::new(100.0, 0.0, 40.0);
            let command = GameCommand {
                command_type: CommandType::MoveToAndEvacuate {
                    destination,
                    and_exit: false,
                },
                player_id: 0,
                command_id: 46_013,
                timestamp: std::time::SystemTime::UNIX_EPOCH,
                selected_units: vec![transport],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                CommandExecutor::new(&mut logic, 0)
                    .execute_command(command)
                    .unwrap(),
                CommandResult::Success
            );
            let ai = logic
                .host_object(transport)
                .unwrap()
                .chinook_ai
                .as_ref()
                .unwrap();
            assert_eq!(ai.state, HostChinookAIState::TakingOff);
            assert_eq!(
                ai.pending_evac_dest,
                Some([destination.x, destination.z, destination.y])
            );
            logic.update_with_dt(1.0 / 30.0);
            let object = logic.host_object(transport).unwrap();
            assert_eq!(
                object.chinook_ai.as_ref().unwrap().state,
                HostChinookAIState::TakingOff
            );
            assert_eq!(
                logic.host_object(passenger).unwrap().contained_by,
                Some(transport)
            );

            let mut took_off = false;
            let mut landed_after_flight = false;
            let mut exited = false;
            let mut accepted_landing_destination = None;
            for _ in 0..480 {
                logic.update_with_dt(1.0 / 30.0);
                let object = logic.host_object(transport).unwrap();
                let ai = object.chinook_ai.as_ref().unwrap();
                let y = object.get_position().y;
                if ai.state == HostChinookAIState::LandAndEvac
                    && accepted_landing_destination.is_none()
                {
                    // The move state must reach the requested XZ first. C++
                    // landing onEnter may then choose a nearby clear landing
                    // position, so the final precise arrival uses that goal.
                    let p = object.get_position();
                    let horizontal =
                        Vec3::new(p.x - destination.x, 0.0, p.z - destination.z).length();
                    assert!(horizontal <= 3.0,
                        "the evacuation approach must reach its requested destination: {p:?} -> {destination:?}");
                    accepted_landing_destination =
                        Some(Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]));
                }
                took_off |= y >= 90.0;
                landed_after_flight |= took_off && y <= 3.0;
                if logic.host_object(passenger).unwrap().contained_by.is_none() {
                    assert!(
                        logic.host_object(second).unwrap().contained_by.is_none(),
                        "the same entry action removes every passenger"
                    );
                    assert!(object.contained_units().is_empty());
                    let p = object.get_position();
                    let landing_destination = accepted_landing_destination
                        .expect("actual flight must approach before precise landing and release");
                    let horizontal = Vec3::new(
                        p.x - landing_destination.x,
                        0.0,
                        p.z - landing_destination.z,
                    )
                    .length();
                    assert!(horizontal <= 3.0,
                        "release must occur at the accepted precise landing destination: {p:?} -> {landing_destination:?}");
                    assert!(took_off, "passenger cannot exit before real takeoff");
                    assert!(
                        landed_after_flight,
                        "passenger cannot exit before real landing: {y}, {:?}",
                        ai.state
                    );
                    exited = true;
                    break;
                }
                assert_eq!(
                    logic.host_object(passenger).unwrap().contained_by,
                    Some(transport)
                );
            }
            assert!(
                took_off,
                "authored lift must move the owner to its takeoff height"
            );
            assert!(
                landed_after_flight,
                "the actual locomotor must return to the landing height"
            );
            assert!(exited, "the actual evacuation must release its passenger");
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_explicit_vertical_goal_does_not_add_maintain_height_force() {
    isolated(
        "authored_explicit_vertical_goal_does_not_add_maintain_height_force",
        || {
            let (mut logic, transport, _) = authored_match();
            let object = logic.host_object_mut(transport).unwrap();
            let start = Vec3::new(40.0, 0.0, 40.0);
            object.set_position(start);
            object.set_precise_z_and_ultra_accurate(true);
            object.set_locomotor_goal_position_explicit(start + Vec3::Y * 100.0);
            assert_eq!(object.physics_accel.y, 0.0);
            // Exercise the real owned movement phase without a Chinook
            // callback. It already consumes lift/gravity in its height Euler
            // step, so a later maintain-position call must not add another
            // height force targeting the just-captured maintain pose.
            logic.update_movement_for_test(&[transport], 1.0 / 30.0);
            let object = logic.host_object(transport).unwrap();
            assert!(object.get_position().y > start.y);
            assert!(object.movement.velocity.y > 0.0);
            assert!(object.physics_accel.y.abs() < 0.00001,
                "one explicit-goal height update must consume its force, without a second maintain-height force: {}", object.physics_accel.y);
        },
    );
}

/// hq-0xpfm: actual authored aircraft publish separate occupied landing goals.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_host_chinooks_unstack_landing_dest() {
    isolated("live_host_chinooks_unstack_landing_dest", || {
        let (mut logic, a, _) = authored_match();
        let position = logic.host_object(a).unwrap().get_position();
        let b = logic
            .create_object("Hq46k12CombatChinook", Team::USA, position)
            .expect("second authored Chinook");
        for id in [a, b] {
            let object = logic.host_object_mut(id).unwrap();
            assert!(object.cur_locomotor_name.is_some());
            object.pending_evacuate_on_stop = true;
        }
        logic.tick_chinook_ai(1.0 / 30.0);
        let destinations = [a, b].map(|id| {
            let object = logic.host_object(id).unwrap();
            let ai = object.chinook_ai.as_ref().unwrap();
            assert_eq!(ai.flight_status, HostChinookFlightStatus::Landing);
            assert_eq!(object.locomotor_goal_type, LocoGoalType::PositionExplicit);
            assert!(object.precise_z_pos && object.ultra_accurate);
            assert_eq!(
                object.movement.target_position,
                Some(Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]))
            );
            ai.dest
        });
        let [da, db] = destinations;
        let stacked = (da[0] - db[0]).abs() < 1.0 && (da[1] - db[1]).abs() < 1.0;
        assert!(
            !stacked,
            "chinooks must not share one LZ da={da:?} db={db:?}"
        );
    });
}
