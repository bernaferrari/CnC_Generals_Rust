//! Retail-asset probe of same-process worlds. Run explicitly with
//! `cargo test -p generals_main --test parallel_instances -- --ignored --test-threads=1`.
//! The checksum covers only part of the simulation; a pass is not proof of
//! complete instance isolation or parity with the original game.

use generals_main::parallel_harness::{
    HeadlessSkirmish, SharedSkirmishAssets, SkirmishSpec, run_headless_skirmish,
    run_parallel_skirmishes,
};

#[test]
#[ignore = "requires retail assets and runs several real skirmish startups"]
fn same_process_interleaved_and_concurrent_worlds_match_single_runs() {
    // Map loading and AI use a deeper stack than the default test thread.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let assets = SharedSkirmishAssets::load_retail().expect("retail harness assets");
            let first = SkirmishSpec { seed: 1234, frames: 8 };
            let second = SkirmishSpec { seed: 5678, frames: 8 };
            let baseline = run_headless_skirmish(&assets, first).expect("first baseline");
            let other_baseline = run_headless_skirmish(&assets, second).expect("second baseline");
            assert!(baseline.objects_at_start > 0, "map must contain objects");
            assert_eq!(baseline.frame_crcs.len(), first.frames as usize);

            let mut a = HeadlessSkirmish::start(&assets, first.seed).expect("first world");
            let mut b = HeadlessSkirmish::start(&assets, second.seed).expect("peer world");
            assert_eq!(a.object_count(), baseline.objects_at_start);
            assert_eq!(b.object_count(), other_baseline.objects_at_start);
            for index in 0..first.frames as usize {
                let b_before = (b.frame(), b.logic_crc(), b.player_summary());
                assert_eq!(a.step(), baseline.frame_crcs[index], "first frame {index}");
                assert_eq!((b.frame(), b.logic_crc(), b.player_summary()), b_before,
                    "advancing first world changed peer at frame {index}");
                let a_before = (a.frame(), a.logic_crc(), a.player_summary());
                assert_eq!(b.step(), other_baseline.frame_crcs[index], "peer frame {index}");
                assert_eq!((a.frame(), a.logic_crc(), a.player_summary()), a_before,
                    "advancing peer changed first world at frame {index}");
            }
            assert_eq!(a.object_count(), baseline.objects_at_end);
            assert_eq!(b.object_count(), other_baseline.objects_at_end);
            assert_eq!(a.frame(), baseline.final_frame);
            assert_eq!(b.frame(), other_baseline.final_frame);
            drop((a, b));

            let parallel = run_parallel_skirmishes(&assets, &[first, second, first]);
            let mut parallel = parallel.into_iter();
            assert_eq!(parallel.next().unwrap().expect("parallel first"), baseline);
            assert_eq!(parallel.next().unwrap().expect("parallel peer"), other_baseline);
            assert_eq!(parallel.next().unwrap().expect("parallel duplicate"), baseline);
        })
        .expect("spawn probe")
        .join()
        .expect("probe panicked");
}
