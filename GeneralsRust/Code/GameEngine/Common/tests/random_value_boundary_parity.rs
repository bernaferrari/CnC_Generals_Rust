//! Boundary parity for the integer RNG streams (C++ RandomValue.cpp).
//!
//! C++ `GameLogicRandomValue` (:189-196): `UnsignedInt delta = hi - lo + 1;`
//! — the `Int` expression wraps on the selected original compiler (MSVC x86,
//! two's complement) before the bits are stored as `UnsignedInt`; `delta == 0`
//! returns `hi` BEFORE consuming a draw; the result is
//! `((Int)(draw % delta)) + lo` with a signed wrap on the final add.
//! The Rust implementation mirrors this with explicit wrapping arithmetic;
//! these tests pin value determinism, draw counts, and the wrapped-delta
//! domain against the public API so the checks run in the supported build
//! (the crate's `cfg(test)` modules are not compiled into any test target).

use game_engine::common::random_value::{
    get_game_audio_random_value, get_game_client_random_value,
    get_game_logic_random_seed_state, get_game_logic_random_value,
    init_game_logic_random, init_random_with_seed,
};
use std::sync::Mutex;

/// The three RNG streams are process-global; serialize every test.
static RNG_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn max_range_does_not_panic_and_replays_deterministically() {
    let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (0, i32::MAX): delta wraps to 0x80000000 (C++ RandomValue.cpp:189).
    // Checked-arithmetic builds must not panic, and the same seed must
    // reproduce the exact value (one draw consumed per call, C++ :196).
    init_game_logic_random(0x5EED_0001);
    let first = get_game_logic_random_value(0, i32::MAX);
    init_game_logic_random(0x5EED_0001);
    let second = get_game_logic_random_value(0, i32::MAX);
    assert_eq!(first, second, "same seed must replay the wrapped-delta draw");
}

#[test]
fn delta_zero_range_returns_hi_without_consuming_a_draw() {
    let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (5, 4): hi - lo + 1 == 0 mod 2^32 => C++ :193-194 returns hi (4)
    // BEFORE the randomValue() call — the stream state must be untouched.
    init_game_logic_random(0x5EED_0002);
    let before = get_game_logic_random_seed_state();
    assert_eq!(get_game_logic_random_value(5, 4), 4);
    assert_eq!(
        get_game_logic_random_seed_state(),
        before,
        "delta==0 must not consume a draw (C++ RandomValue.cpp:193-194)"
    );
    // The extreme wrap pair (i32::MIN, i32::MAX) is also a delta==0 range.
    init_game_logic_random(0x5EED_0003);
    let before = get_game_logic_random_seed_state();
    assert_eq!(get_game_logic_random_value(i32::MIN, i32::MAX), i32::MAX);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn equal_bounds_return_lo_and_consume_exactly_one_draw() {
    let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (10, 10): delta == 1 (C++ :196, NOT the delta==0 branch) — one draw.
    init_game_logic_random(0x5EED_0004);
    let before = get_game_logic_random_seed_state();
    assert_eq!(get_game_logic_random_value(10, 10), 10);
    assert_ne!(
        get_game_logic_random_seed_state(),
        before,
        "lo==hi consumes exactly one draw"
    );
}

#[test]
fn wrapped_results_stay_inside_the_msvc_delta_domain() {
    let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // For every non-delta==0 pair the result satisfies
    // (result - lo) as u32 < delta, with delta computed in wrapped arithmetic
    // and stored as UnsignedInt (C++ RandomValue.cpp:189-196). Includes the
    // full wrap-add range (i32::MAX - 1, i32::MIN) whose delta is 2^31.
    let pairs = [
        (0, i32::MAX),
        (i32::MAX - 1, i32::MIN),
        (-7, 900),
        (i32::MIN, i32::MIN + 4096),
        (-1, -1),
        (1234, 5678),
    ];
    for seed in [1u32, 0xDEAD_BEEF, 0x7FFF_FFFF] {
        for &(lo, hi) in &pairs {
            let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
            if delta == 0 {
                init_game_logic_random(seed);
                assert_eq!(get_game_logic_random_value(lo, hi), hi);
                continue;
            }
            init_game_logic_random(seed);
            let value = get_game_logic_random_value(lo, hi);
            let offset = value.wrapping_sub(lo) as u32;
            assert!(
                offset < delta,
                "seed {seed} pair ({lo},{hi}): offset {offset} outside delta {delta}"
            );
        }
    }
}

#[test]
fn client_and_audio_streams_mirror_the_logic_boundary() {
    let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // All three streams share the C++ body (RandomValue.cpp:217/:223 client,
    // :240/:246 audio) — the wrapped boundary must behave identically and
    // deterministically on each. init_random_with_seed reseeds ALL streams;
    // init_game_logic_random touches the logic stream only.
    for seed in [0x5EED_0005u32, 42] {
        init_random_with_seed(seed);
        let client_a = get_game_client_random_value(0, i32::MAX);
        init_random_with_seed(seed);
        let client_b = get_game_client_random_value(0, i32::MAX);
        assert_eq!(client_a, client_b, "client stream must replay");

        init_random_with_seed(seed);
        let audio_a = get_game_audio_random_value(0, i32::MAX);
        init_random_with_seed(seed);
        let audio_b = get_game_audio_random_value(0, i32::MAX);
        assert_eq!(audio_a, audio_b, "audio stream must replay");
    }
}
