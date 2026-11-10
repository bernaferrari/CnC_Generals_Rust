//! Retail-asset same-process probe. From `GeneralsRust`, run
//! `cargo test -p generals_main --test parallel_instances -- --ignored --test-threads=1`.
//! `CNC_PARALLEL_INSTANCES` (default 3, minimum 2) and `CNC_PARALLEL_FRAMES`
//! (default 60, minimum 2) control the concurrent batch and trace length.
//! Worlds play real input: the human USA slot gets scripted orders (queue a
//! Dozer, build a Power Plant) through the UI command path while the GLA slot
//! runs its skirmish AI. The checksum, world digest and observations cover
//! only part of the simulation; a pass is not proof of complete isolation or
//! parity with the original game.

use generals_main::parallel_harness::{
    HeadlessSkirmish, HumanOrders, SharedSkirmishAssets, SkirmishObservation, SkirmishSpec,
    SkirmishTrace, run_headless_skirmish, run_parallel_skirmishes,
};

fn setting(name: &str, default: u32, minimum: u32) -> u32 {
    match std::env::var(name) {
        Ok(value) => {
            let parsed: u32 = value.parse().unwrap_or_else(|_| panic!("invalid {name}: {value}"));
            assert!(parsed >= minimum, "{name} must be at least {minimum}");
            parsed
        }
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("cannot read {name}: {error}"),
    }
}

fn supplies(observation: &SkirmishObservation, player: u32) -> u32 {
    observation
        .players
        .iter()
        .find(|(id, _, _)| *id == player)
        .map(|(_, supplies, _)| *supplies)
        .unwrap_or_else(|| panic!("player {player} missing from {observation:?}"))
}

fn start(assets: &SharedSkirmishAssets, spec: SkirmishSpec) -> HeadlessSkirmish {
    let mut game = HeadlessSkirmish::start(assets, spec.seed).expect("world start");
    if spec.human_orders {
        game.issue_human_orders().expect("human orders");
    }
    game
}

/// Drive `a` and `b` alternately on this thread; each frame must match its
/// solo trace and leave the other world untouched.
fn interleave(a: &mut HeadlessSkirmish, a_trace: &SkirmishTrace, a_from: usize,
    b: &mut HeadlessSkirmish, b_trace: &SkirmishTrace, b_from: usize, steps: usize, label: &str) {
    for step in 0..steps {
        let b_before = b.observe();
        a.advance();
        assert_eq!(a.observe(), a_trace.frames[a_from + step], "{label}: A frame {}", a_from + step);
        assert_eq!(b.observe(), b_before, "{label}: A's advance changed B at step {step}");
        let a_before = a.observe();
        b.advance();
        assert_eq!(b.observe(), b_trace.frames[b_from + step], "{label}: B frame {}", b_from + step);
        assert_eq!(a.observe(), a_before, "{label}: B's advance changed A at step {step}");
    }
}

#[test]
#[ignore = "requires retail assets and runs several real skirmish startups"]
fn same_process_interleaved_and_concurrent_worlds_match_single_runs() {
    // Map loading and AI use a deeper stack than the default test thread.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let instances = setting("CNC_PARALLEL_INSTANCES", 3, 2) as usize;
            let frames = setting("CNC_PARALLEL_FRAMES", 60, 2);
            let assets = SharedSkirmishAssets::load_retail().expect("retail harness assets");
            // Same seed with and without human input, plus a second seed that
            // exercises the signed GameInfo seed's high bit.
            let quiet = SkirmishSpec { seed: 1234, frames, human_orders: false };
            let ordered = SkirmishSpec { seed: 1234, frames, human_orders: true };
            let other = SkirmishSpec { seed: 0x8765_4321, frames, human_orders: true };
            let quiet_trace = run_headless_skirmish(&assets, quiet).expect("quiet baseline");
            let ordered_trace = run_headless_skirmish(&assets, ordered).expect("ordered baseline");
            let other_trace = run_headless_skirmish(&assets, other).expect("other baseline");

            assert!(quiet_trace.initial.object_count > 0, "map must contain objects");
            assert_eq!(quiet_trace.initial.base_seed, quiet.seed);
            assert_eq!(other_trace.initial.base_seed, other.seed);
            assert_ne!(quiet_trace.initial.random_state, other_trace.initial.random_state,
                "different seeds must reach different Rust RNG states");
            assert_eq!(quiet_trace.frames.len(), frames as usize);
            for (index, observation) in quiet_trace.frames.iter().enumerate() {
                assert_eq!(observation.frame, quiet_trace.initial.frame.wrapping_add(index as u32 + 1),
                    "the first world must advance one logic frame per step");
            }
            // The orders are real gameplay: the Dozer purchase is charged and
            // the world diverges from the same-seed run without input.
            assert_eq!(ordered_trace.initial, quiet_trace.initial, "orders are issued after the start snapshot");
            let human = HumanOrders::PLAYER;
            let last = frames as usize - 1;
            assert!(supplies(&ordered_trace.frames[last], human) < supplies(&quiet_trace.frames[last], human),
                "queuing a {} must charge the human player", HumanOrders::PRODUCED_UNIT);
            assert_ne!(ordered_trace.frames[last].world_digest, quiet_trace.frames[last].world_digest,
                "human orders must change the world");
            assert_eq!(run_headless_skirmish(&assets, ordered).expect("ordered rerun"), ordered_trace,
                "ordered run must be deterministic");

            // Same seed, different input, one thread: shared RNG, ids or
            // command state would leak across.
            let mut a = start(&assets, quiet);
            let mut b = start(&assets, ordered);
            assert_eq!(a.observe(), quiet_trace.initial);
            assert_eq!(b.observe(), ordered_trace.initial);
            let half = frames as usize / 2;
            interleave(&mut a, &quiet_trace, 0, &mut b, &ordered_trace, 0, half, "quiet/ordered");

            // A third world loads its map mid-match; the running worlds must not notice.
            let (a_before, b_before) = (a.observe(), b.observe());
            let mut c = start(&assets, other);
            assert_eq!(c.observe(), other_trace.initial, "late world start");
            assert_eq!(a.observe(), a_before, "late start changed quiet world");
            assert_eq!(b.observe(), b_before, "late start changed ordered world");
            interleave(&mut b, &ordered_trace, half, &mut c, &other_trace, 0, frames as usize - half,
                "ordered/late");
            // Quiet world catches up beside the late world's remaining frames.
            let late_done = frames as usize - half;
            let together = (frames as usize - half).min(frames as usize - late_done);
            interleave(&mut a, &quiet_trace, half, &mut c, &other_trace, late_done, together,
                "quiet/late");
            for step in half + together..frames as usize {
                let c_before = c.observe();
                a.advance();
                assert_eq!(a.observe(), quiet_trace.frames[step], "quiet frame {step} after late start");
                assert_eq!(c.observe(), c_before, "quiet advance changed late world at {step}");
            }
            for step in late_done + together..frames as usize {
                let a_before = a.observe();
                c.advance();
                assert_eq!(c.observe(), other_trace.frames[step], "late frame {step}");
                assert_eq!(a.observe(), a_before, "late advance changed quiet world at {step}");
            }
            let a_before = a.observe();
            drop(b);
            drop(c);
            assert_eq!(a.observe(), a_before, "peer teardown changed first world");
            drop(a);

            let rotation = [(quiet, &quiet_trace), (ordered, &ordered_trace), (other, &other_trace)];
            let specs: Vec<_> = (0..instances).map(|index| rotation[index % 3].0).collect();
            let parallel = run_parallel_skirmishes(&assets, &specs);
            assert_eq!(parallel.len(), instances);
            for (index, result) in parallel.into_iter().enumerate() {
                assert_eq!(&result.unwrap_or_else(|error| panic!("instance {index}: {error}")),
                    rotation[index % 3].1, "parallel instance {index}");
            }
        })
        .expect("spawn probe")
        .join()
        .expect("probe panicked");
}

/// C++ `ThingTemplate::calcTimeToBuild` for a full-handicap player without a
/// faction ProductionTimeChange, from the world's authored inputs.
fn cpp_build_frames(game: &HeadlessSkirmish, player: u32, name: &str) -> u32 {
    let seconds = game.template_build_seconds(name).expect("retail template");
    let (penalty, min_speed, max_speed) = game.energy_rules();
    let energy = game.energy_supply_ratio(player).expect("player").min(1.0);
    let mut rate = (1.0 - (1.0 - energy) * penalty).max(min_speed);
    if energy < 1.0 {
        rate = rate.min(max_speed);
    }
    if rate <= 0.0 {
        rate = 0.01;
    }
    ((seconds * 30.0) as u32 as f32 / rate) as u32
}

#[test]
#[ignore = "requires retail assets and plays a long skirmish opening"]
fn human_power_plant_completes_in_its_cpp_build_time_while_worlds_take_turns() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let assets = SharedSkirmishAssets::load_retail().expect("retail harness assets");
            let specs = [
                SkirmishSpec { seed: 1234, frames: 0, human_orders: true },
                SkirmishSpec { seed: 0x8765_4321, frames: 0, human_orders: true },
            ];
            let mut worlds: Vec<_> = specs.iter().map(|&spec| start(&assets, spec)).collect();
            let human = HumanOrders::PLAYER;
            let expected = cpp_build_frames(&worlds[0], human, HumanOrders::STRUCTURE);
            assert_eq!(cpp_build_frames(&worlds[1], human, HumanOrders::STRUCTURE), expected,
                "both worlds start from the same authored build-time inputs");
            assert!(expected >= 30, "retail Power Plant build time: {expected} frames");
            // The ordered Dozer and its approach budget: C++ newTask stores
            // the build dock from findGoodBuildOrRepairPosition, outside the
            // scaffold, and the Dozer drives its first planned path there at
            // locomotor speed. Budget = frames until that path exists + its
            // length at max speed + APPROACH_SLACK (acceleration, turning and
            // the MoveToActionPos arrival tolerance).
            const APPROACH_SLACK: u32 = 30;
            let order_frame = worlds[0].frame();
            let dozers: Vec<_> = worlds
                .iter_mut()
                .map(|world| world.owned_ids(human, "AmericaVehicleDozer")[0])
                .collect();
            let mut approach_budget = [None; 2];
            // Approach and docking, then exactly one percent step per frame.
            let limit = expected + 600;
            let mut started = [None; 2];
            let mut finished = [None; 2];
            for _ in 0..limit {
                for (index, world) in worlds.iter_mut().enumerate() {
                    if finished[index].is_some() {
                        continue;
                    }
                    world.advance();
                    let dozer = dozers[index];
                    if approach_budget[index].is_none() {
                        if let Some((_, Some(dock), speed, Some(path))) = world.dozer_approach(dozer) {
                            let site = world.owned_ids(human, HumanOrders::STRUCTURE)[0];
                            assert_eq!(world.footprint_contains(site, dock), Some(false),
                                "world {index}: build dock {dock:?} must lie outside the scaffold");
                            assert!(speed > 0.0, "world {index}: dozer locomotor speed");
                            // Locomotor turning slows the Dozer at each path
                            // corner: allow 10% over the full-speed drive.
                            let drive = (path / (speed / 30.0) * 1.1).ceil() as u32;
                            approach_budget[index] = Some(world.frame() - order_frame + drive + APPROACH_SLACK);
                        }
                    }
                    let sites = world.construction(human, HumanOrders::STRUCTURE);
                    assert_eq!(sites.len(), 1, "world {index}: one scaffold, got {sites:?}");
                    let (_, under_construction, percent) = sites[0];
                    if started[index].is_none() && percent > 0.0 {
                        started[index] = Some(world.frame());
                    }
                    if !under_construction {
                        assert_eq!(percent, 1.0, "world {index}: completed at 100%");
                        finished[index] = Some(world.frame());
                    }
                }
                if finished.iter().all(Option::is_some) {
                    break;
                }
            }
            for index in 0..2 {
                let start = started[index].unwrap_or_else(|| panic!("world {index}: Dozer never started building"));
                let budget = approach_budget[index].unwrap_or_else(|| panic!("world {index}: no build dock"));
                eprintln!("world {index}: approach {} frames (budget {budget})", start - order_frame);
                assert!(start - order_frame <= budget,
                    "world {index}: Dozer took {} frames to reach the scaffold, its planned path allows {budget}",
                    start - order_frame);
                let end = finished[index].unwrap_or_else(|| panic!("world {index}: Power Plant unfinished after {limit} frames"));
                // The first contributing frame already adds 1/frames.
                let building = end - start + 1;
                assert!((expected..=expected + 1).contains(&building),
                    "world {index}: built in {building} frames, C++ calcTimeToBuild gives {expected}");
                // The finished plant now powers its own world only.
                assert_eq!(worlds[index].energy_supply_ratio(human).map(|ratio| ratio > 0.0), Some(true),
                    "world {index}: completed Power Plant must supply energy");
            }
        })
        .expect("spawn probe")
        .join()
        .expect("probe panicked");
}

#[test]
#[ignore = "requires retail assets"]
fn retail_game_data_rules_reach_each_world_like_the_app_boot_while_worlds_take_turns() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let assets = SharedSkirmishAssets::load_retail().expect("retail harness assets");
            let mut worlds = [
                start(&assets, SkirmishSpec { seed: 7, frames: 0, human_orders: false }),
                start(&assets, SkirmishSpec { seed: 8, frames: 0, human_orders: true }),
            ];
            for step in 0..30 {
                for (index, world) in worlds.iter_mut().enumerate() {
                    world.advance();
                    // C++ GameEngine::init loads GameData/Armor/Terrain/Roads/
                    // OCL once; every headless world must hold that content.
                    assert_eq!(world.retail_rule_mismatches(&assets), Vec::<&str>::new(),
                        "world {index} at step {step}");
                }
            }
        })
        .expect("spawn probe")
        .join()
        .expect("probe panicked");
}
