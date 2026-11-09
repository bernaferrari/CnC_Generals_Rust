//! Retail-asset same-process probe. From `GeneralsRust`, run
//! `cargo test -p generals_main --test parallel_instances -- --ignored --test-threads=1`.
//! `CNC_PARALLEL_INSTANCES` (default 3, minimum 2) and `CNC_PARALLEL_FRAMES`
//! (default 60, minimum 1) control the concurrent batch and trace length.
//! The checksum and observations cover only part of the simulation; a pass is
//! not proof of complete isolation or parity with the original game.

use generals_main::parallel_harness::{
    HeadlessSkirmish, SharedSkirmishAssets, SkirmishSpec, run_headless_skirmish,
    run_parallel_skirmishes,
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

#[test]
#[ignore = "requires retail assets and runs several real skirmish startups"]
fn same_process_interleaved_and_concurrent_worlds_match_single_runs() {
    // Map loading and AI use a deeper stack than the default test thread.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let instances = setting("CNC_PARALLEL_INSTANCES", 3, 2) as usize;
            let frames = setting("CNC_PARALLEL_FRAMES", 60, 1);
            let assets = SharedSkirmishAssets::load_retail().expect("retail harness assets");
            let first = SkirmishSpec { seed: 1234, frames };
            // Exercises the signed GameInfo seed's high bit without narrowing it.
            let second = SkirmishSpec { seed: 0x8765_4321, frames };
            let baseline = run_headless_skirmish(&assets, first).expect("first baseline");
            let other_baseline = run_headless_skirmish(&assets, second).expect("second baseline");
            assert!(baseline.initial.object_count > 0, "map must contain objects");
            assert_eq!(baseline.initial.base_seed, first.seed);
            assert_eq!(other_baseline.initial.base_seed, second.seed);
            assert_ne!(baseline.initial.random_state, other_baseline.initial.random_state,
                "different seeds must reach different Rust RNG states");
            assert_eq!(baseline.frames.len(), frames as usize);
            for (index, observation) in baseline.frames.iter().enumerate() {
                assert_eq!(observation.frame, baseline.initial.frame.wrapping_add(index as u32 + 1),
                    "the first world must advance one logic frame per step");
            }

            let mut a = HeadlessSkirmish::start(&assets, first.seed).expect("first world");
            let mut b = HeadlessSkirmish::start(&assets, second.seed).expect("peer world");
            assert_eq!(a.observe(), baseline.initial);
            assert_eq!(b.observe(), other_baseline.initial);
            for index in 0..frames as usize {
                let b_before = b.observe();
                a.advance();
                assert_eq!(a.observe(), baseline.frames[index], "first frame {index}");
                assert_eq!(b.observe(), b_before, "first advance changed peer at frame {index}");
                let a_before = a.observe();
                b.advance();
                assert_eq!(b.observe(), other_baseline.frames[index], "peer frame {index}");
                assert_eq!(a.observe(), a_before, "peer advance changed first at frame {index}");
            }
            let a_before = a.observe();
            drop(b);
            assert_eq!(a.observe(), a_before, "peer teardown changed first world");
            drop(a);

            let specs: Vec<_> = (0..instances)
                .map(|index| if index % 2 == 0 { first } else { second })
                .collect();
            let parallel = run_parallel_skirmishes(&assets, &specs);
            assert_eq!(parallel.len(), instances);
            for (index, result) in parallel.into_iter().enumerate() {
                let expected = if index % 2 == 0 { &baseline } else { &other_baseline };
                assert_eq!(&result.unwrap_or_else(|error| panic!("instance {index}: {error}")),
                    expected, "parallel instance {index}");
            }
        })
        .expect("spawn probe")
        .join()
        .expect("probe panicked");
}
