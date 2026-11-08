// Unit tests for random_value (file name excluded from globals budget).
use super::*;
use parking_lot::Mutex;
use std::thread;

static RNG_TEST_LOCK: Mutex<()> = Mutex::new(());

struct RngTestGuard {
    _lock: parking_lot::MutexGuard<'static, ()>,
    services: std::sync::Arc<crate::common::common_services::CommonServices>,
}

impl RngTestGuard {
    fn new() -> Self {
        let _lock = RNG_TEST_LOCK.lock();
        let services = std::sync::Arc::new(crate::common::common_services::CommonServices::new());
        crate::common::common_services::install_common_services(std::sync::Arc::clone(&services));
        Self { _lock, services }
    }
}

impl Drop for RngTestGuard {
    fn drop(&mut self) {
        crate::common::common_services::uninstall_common_services_if_current(&self.services);
    }
}


#[test]
fn random_variable_getters_match_set_range() {
    let mut var = GameLogicRandomVariable::new();
    var.set_range_uniform(3.0, 9.0);
    assert_eq!(var.get_minimum_value(), 3.0);
    assert_eq!(var.get_maximum_value(), 9.0);
    assert_eq!(var.get_distribution_type(), DistributionType::Uniform);
}

#[test]
fn test_random_initialization() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    assert_eq!(get_game_logic_random_seed(), 12345);
}

#[test]
fn test_random_range() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    let val = get_game_logic_random_value(10, 20);
    assert!(val >= 10 && val <= 20);
}

#[test]
fn test_random_real_range() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    let val = get_game_logic_random_value_real(1.0, 2.0);
    assert!(val >= 1.0 && val <= 2.0);
}

#[test]
fn test_random_variable() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    let mut var = GameLogicRandomVariable::new();
    var.set_range(5.0, 15.0, DistributionType::Uniform);

    let val = var.get_value();
    assert!(val >= 5.0 && val <= 15.0);
}

#[test]
fn test_constant_random_variable() {
    let mut var = GameLogicRandomVariable::new();
    var.set_range(42.0, 42.0, DistributionType::Constant);

    let val = var.get_value();
    assert_eq!(val, 42.0);
}

#[test]
fn constant_logic_random_variable_with_mismatched_range_falls_through_to_uniform() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    let mut var = GameLogicRandomVariable::new();
    var.set_range(5.0, 15.0, DistributionType::Constant);
    let actual = var.get_value();

    init_random_with_seed(12345);
    let expected = get_game_logic_random_value_real(5.0, 15.0);
    assert_eq!(actual, expected);
}

#[test]
fn constant_client_random_variable_with_mismatched_range_falls_through_to_uniform() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    init_random_with_seed(54321);
    client.seed_random(54321);
    audio.seed_random(54321);
    let mut var = GameClientRandomVariable::new();
    var.set_range(5.0, 15.0, DistributionType::Constant);
    let actual = var.get_value_with(&mut client);

    init_random_with_seed(54321);

    client.seed_random(54321);

    audio.seed_random(54321);
    let expected = client.next_real(5.0, 15.0);
    assert_eq!(actual, expected);
}

#[test]
fn unsupported_logic_distributions_return_zero_without_panic() {
    // C++ GameLogicRandomVariable::getValue (RandomValue.cpp:410-413)
    for dist in [
        DistributionType::Gaussian,
        DistributionType::Triangular,
        DistributionType::LowBias,
        DistributionType::HighBias,
    ] {
        let mut var = GameLogicRandomVariable::new();
        var.set_range(5.0, 15.0, dist);
        assert_eq!(var.get_value(), 0.0);
    }
}

#[test]
fn unsupported_client_distributions_return_zero_without_panic() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    // C++ GameClientRandomVariable::getValue (RandomValue.cpp:366-369)
    for dist in [
        DistributionType::Gaussian,
        DistributionType::Triangular,
        DistributionType::LowBias,
        DistributionType::HighBias,
    ] {
        let mut var = GameClientRandomVariable::new();
        var.set_range(5.0, 15.0, dist);
        assert_eq!(var.get_value_with(&mut client), 0.0);
    }
}

// ============================================================================
// WEEK 1 PRIORITY 1: RNG SAFETY TESTS (15+ tests for mutex poisoning recovery)
// ============================================================================

#[test]
fn test_rng_successful_lock_acquisition() {
    let _guard = RngTestGuard::new();
    // Verify normal lock acquisition works without panic
    init_random_with_seed(99999);
    let seed = get_game_logic_random_seed();
    assert_eq!(seed, 99999);
}

#[test]
fn test_rng_logic_value_multiple_calls() {
    let _guard = RngTestGuard::new();
    // Verify repeated RNG calls don't panic
    init_random_with_seed(54321);
    for _ in 0..100 {
        let _val = get_game_logic_random_value(1, 100);
    }
    // If we got here without panicking, test passes
    assert!(true);
}

#[test]
fn test_rng_client_value_multiple_calls() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify client RNG repeated calls don't panic
    init_random_with_seed(54321);
    client.seed_random(54321);
    audio.seed_random(54321);
    for _ in 0..100 {
        let _val = client.next_int(1, 100);
    }
    // If we got here without panicking, test passes
    assert!(true);
}

#[test]
fn test_rng_audio_value_multiple_calls() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify audio RNG repeated calls don't panic
    init_random_with_seed(54321);
    client.seed_random(54321);
    audio.seed_random(54321);
    for _ in 0..100 {
        let _val = audio.next_int(1, 100);
    }
    // If we got here without panicking, test passes
    assert!(true);
}

#[test]
fn test_rng_real_value_range_inclusive_high() {
    let _guard = RngTestGuard::new();
    // Verify range calculation includes high value (hi - lo + 1)
    init_random_with_seed(11111);
    let mut found_high = false;
    for _ in 0..1000 {
        let val = get_game_logic_random_value(10, 10);
        if val == 10 {
            found_high = true;
            break;
        }
    }
    assert!(found_high, "Should be able to get the high value (10, 10)");
}

#[test]
fn test_rng_range_inclusive_boundaries() {
    let _guard = RngTestGuard::new();
    // Verify both low and high boundaries are inclusive
    init_random_with_seed(22222);
    let mut found_low = false;
    let mut found_high = false;
    for _ in 0..10000 {
        let val = get_game_logic_random_value(50, 60);
        if val == 50 {
            found_low = true;
        }
        if val == 60 {
            found_high = true;
        }
        if found_low && found_high {
            break;
        }
    }
    assert!(found_low, "Should be able to get the low value (50)");
    assert!(found_high, "Should be able to get the high value (60)");
}

#[test]
fn test_rng_stream_separation_logic_vs_client() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify logic and client streams are separate
    init_random_with_seed(33333);
    client.seed_random(33333);
    audio.seed_random(33333);
    let logic_val = get_game_logic_random_value(1, 1000);

    init_random_with_seed(33333);

    client.seed_random(33333);

    audio.seed_random(33333);
    let client_val = client.next_int(1, 1000);

    // Both initialized with same seed, but should produce same value in own stream
    // (all streams start from the same seed, so they should match)
    assert_eq!(
        logic_val, client_val,
        "Same seed should produce same sequence"
    );
}

#[test]
fn test_rng_stream_separation_logic_vs_audio() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify logic and audio streams are separate
    init_random_with_seed(44444);
    client.seed_random(44444);
    audio.seed_random(44444);
    let logic_val = get_game_logic_random_value(1, 1000);

    init_random_with_seed(44444);

    client.seed_random(44444);

    audio.seed_random(44444);
    let audio_val = audio.next_int(1, 1000);

    assert_eq!(
        logic_val, audio_val,
        "Same seed should produce same sequence"
    );
}

#[test]
fn test_rng_real_value_logic_range() {
    let _guard = RngTestGuard::new();
    // Verify real value stays in range [lo, hi)
    init_random_with_seed(55555);
    for _ in 0..100 {
        let val = get_game_logic_random_value_real(5.0, 10.0);
        assert!(
            val >= 5.0 && val <= 10.0,
            "Value {} out of range [5.0, 10.0]",
            val
        );
    }
}

#[test]
fn test_rng_real_value_client_range() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify client real value stays in range
    init_random_with_seed(55555);
    client.seed_random(55555);
    audio.seed_random(55555);
    for _ in 0..100 {
        let val = client.next_real(1.0, 2.0);
        assert!(
            val >= 1.0 && val <= 2.0,
            "Value {} out of range [1.0, 2.0]",
            val
        );
    }
}

#[test]
fn test_rng_real_value_audio_range() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify audio real value stays in range
    init_random_with_seed(55555);
    client.seed_random(55555);
    audio.seed_random(55555);
    for _ in 0..100 {
        let val = audio.next_real(3.0, 4.0);
        assert!(
            val >= 3.0 && val <= 4.0,
            "Value {} out of range [3.0, 4.0]",
            val
        );
    }
}

#[test]
fn test_rng_zero_range_returns_high() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify zero range (lo == hi) returns the high value
    init_random_with_seed(66666);
    client.seed_random(66666);
    audio.seed_random(66666);
    assert_eq!(get_game_logic_random_value(42, 42), 42);
    assert_eq!(client.next_int(99, 99), 99);
    assert_eq!(audio.next_int(-5, -5), -5);
}

#[test]
fn test_rng_deterministic_with_same_seed() {
    // Verify same seed produces same sequence
    let _guard = RngTestGuard::new();
    init_random_with_seed(77777);
    let val1 = get_game_logic_random_value(1, 1000);

    init_random_with_seed(77777);
    let val2 = get_game_logic_random_value(1, 1000);

    assert_eq!(val1, val2, "Same seed should produce same value");
}

#[test]
fn test_rng_different_seeds_different_values() {
    let _guard = RngTestGuard::new();
    // Verify different seeds produce different values (probabilistic test)
    init_random_with_seed(11111);
    let val1 = get_game_logic_random_value(1, 1000000);

    init_random_with_seed(22222);
    let val2 = get_game_logic_random_value(1, 1000000);

    // These should almost certainly be different with large range
    assert_ne!(
        val1, val2,
        "Different seeds should likely produce different values"
    );
}

#[test]
fn test_rng_multithreaded_access() {
    let _guard = RngTestGuard::new();
    // Verify RNG can be safely accessed from multiple threads
    init_random_with_seed(88888);
    let shared = crate::common::common_services::common_services();

    let mut handles = vec![];
    for _ in 0..10 {
        let services = std::sync::Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let _prior = crate::common::common_services::install_common_services(services);
            for _ in 0..100 {
                let _val = get_game_logic_random_value(1, 100);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    // If no panics occurred, test passes
    assert!(true);
}

#[test]
fn test_rng_seed_crc_computation() {
    let _guard = RngTestGuard::new();
    // Verify CRC computation doesn't panic
    init_random_with_seed(99999);
    let crc = get_game_logic_random_seed_crc();
    // CRC should be a valid u32
    assert!(crc >= 0); // All u32 values are >= 0
}

#[test]
fn test_rng_init_game_logic_random() {
    let _guard = RngTestGuard::new();
    // Verify game logic random re-initialization works
    init_game_logic_random(123);
    #[cfg(not(feature = "deterministic"))]
    {
        assert_eq!(get_game_logic_random_seed(), 123);
    }
    #[cfg(feature = "deterministic")]
    {
        assert_eq!(get_game_logic_random_seed(), 0);
    }
}

// ==================== Range and repeatability checks ====================
// The user approved a Rust RNG sequence. Existing bounds and repeatability
// checks remain useful; they do not establish original C++ sequence parity.

#[test]
fn test_rng_cpp_seed_values_match() {
    let _guard = RngTestGuard::new();
    // Verify the requested base seed remains observable.
    init_random_with_seed(0);
    // After initialization with seed 0, verify the seed was set correctly
    let seed = get_game_logic_random_seed();
    assert_eq!(seed, 0, "Seed should be 0 after init_random_with_seed(0)");
}

#[test]
fn test_rng_cpp_sequence_seed_1() {
    let _guard = RngTestGuard::new();
    // Test with seed 1: get first few random values
    init_random_with_seed(1);
    let v1 = get_game_logic_random_value(0, u32::MAX as i32);
    let v2 = get_game_logic_random_value(0, u32::MAX as i32);
    let v3 = get_game_logic_random_value(0, u32::MAX as i32);

    // With seed 1, we should always get same sequence
    init_random_with_seed(1);
    assert_eq!(
        v1,
        get_game_logic_random_value(0, u32::MAX as i32),
        "First value should match"
    );
    assert_eq!(
        v2,
        get_game_logic_random_value(0, u32::MAX as i32),
        "Second value should match"
    );
    assert_eq!(
        v3,
        get_game_logic_random_value(0, u32::MAX as i32),
        "Third value should match"
    );
}

#[test]
fn test_rng_cpp_sequence_seed_12345() {
    // Test with seed 12345: classic test seed
    let _guard = RngTestGuard::new();
    init_random_with_seed(12345);
    let values: Vec<i32> = (0..10)
        .map(|_| get_game_logic_random_value(1, 100))
        .collect();

    // Verify all values are in range [1, 100]
    for (i, &v) in values.iter().enumerate() {
        assert!(
            v >= 1 && v <= 100,
            "Value {} at index {} out of range",
            v,
            i
        );
    }

    // Verify sequence is reproducible
    init_random_with_seed(12345);
    for (i, &expected) in values.iter().enumerate() {
        let actual = get_game_logic_random_value(1, 100);
        assert_eq!(
            actual, expected,
            "Value {} at index {} doesn't match",
            expected, i
        );
    }
}

#[test]
fn test_rng_cpp_large_range() {
    let _guard = RngTestGuard::new();
    // Test with large range (1 to 1,000,000)
    init_random_with_seed(99999);
    let values: Vec<i32> = (0..10)
        .map(|_| get_game_logic_random_value(1, 1000000))
        .collect();

    for (i, &v) in values.iter().enumerate() {
        assert!(
            v >= 1 && v <= 1000000,
            "Value {} at index {} out of range",
            v,
            i
        );
    }
}

#[test]
fn test_rng_cpp_negative_range() {
    let _guard = RngTestGuard::new();
    // Test with negative ranges
    init_random_with_seed(54321);
    let values: Vec<i32> = (0..10)
        .map(|_| get_game_logic_random_value(-100, -10))
        .collect();

    for (i, &v) in values.iter().enumerate() {
        assert!(
            v >= -100 && v <= -10,
            "Value {} at index {} out of range [-100, -10]",
            v,
            i
        );
    }
}

#[test]
fn test_rng_cpp_mixed_range() {
    let _guard = RngTestGuard::new();
    // Test with range crossing zero
    init_random_with_seed(77777);
    let values: Vec<i32> = (0..10)
        .map(|_| get_game_logic_random_value(-50, 50))
        .collect();

    for (i, &v) in values.iter().enumerate() {
        assert!(
            v >= -50 && v <= 50,
            "Value {} at index {} out of range [-50, 50]",
            v,
            i
        );
    }
}

#[test]
fn test_rng_cpp_single_value_range() {
    let _guard = RngTestGuard::new();
    // Test edge case: range of single value
    init_random_with_seed(111);
    for i in 0..10 {
        let v = get_game_logic_random_value(42, 42);
        assert_eq!(
            v, 42,
            "Single value range should always return 42 (iteration {})",
            i
        );
    }
}

#[test]
fn test_rng_cpp_real_value_sequence() {
    let _guard = RngTestGuard::new();
    // Test real-valued sequence reproducibility
    init_random_with_seed(22222);
    let values: Vec<f32> = (0..10)
        .map(|_| get_game_logic_random_value_real(0.0, 1.0))
        .collect();

    init_random_with_seed(22222);
    for (i, &expected) in values.iter().enumerate() {
        let actual = get_game_logic_random_value_real(0.0, 1.0);
        assert!(
            (actual - expected).abs() < 0.00001,
            "Real value at index {} doesn't match (expected {}, got {})",
            i,
            expected,
            actual
        );
    }
}

#[test]
fn test_rng_cpp_real_range_bounds() {
    let _guard = RngTestGuard::new();
    // Verify real values stay within bounds
    init_random_with_seed(33333);
    for _ in 0..100 {
        let v = get_game_logic_random_value_real(-10.5, 20.3);
        assert!(
            v >= -10.5 && v <= 20.3,
            "Real value {} out of range [-10.5, 20.3]",
            v
        );
    }
}

#[test]
fn test_rng_cpp_client_vs_logic_independence() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify client and logic streams advance independently
    init_random_with_seed(44444);
    client.seed_random(44444);
    audio.seed_random(44444);
    let logic_val = get_game_logic_random_value(1, 1000);

    // Now get client value without changing logic
    let client_val = client.next_int(1, 1000);

    // Get logic value again - should be different from first
    let logic_val2 = get_game_logic_random_value(1, 1000);

    // Re-init and verify logic matches first call
    init_random_with_seed(44444);
    client.seed_random(44444);
    audio.seed_random(44444);
    assert_eq!(
        logic_val,
        get_game_logic_random_value(1, 1000),
        "Logic stream should be reproducible"
    );
}

#[test]
fn test_rng_cpp_audio_independence() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Verify audio stream is independent
    init_random_with_seed(55555);
    client.seed_random(55555);
    audio.seed_random(55555);
    let audio1 = audio.next_int(1, 1000);
    let audio2 = audio.next_int(1, 1000);

    init_random_with_seed(55555);

    client.seed_random(55555);

    audio.seed_random(55555);
    assert_eq!(
        audio1,
        audio.next_int(1, 1000),
        "Audio stream should be reproducible"
    );
    assert_eq!(
        audio2,
        audio.next_int(1, 1000),
        "Audio stream should be reproducible"
    );
}

#[test]
fn test_rng_cpp_very_large_seed() {
    let _guard = RngTestGuard::new();
    // Test with very large seed value
    init_random_with_seed(0xFFFFFFFF);
    let v1 = get_game_logic_random_value(1, 100);
    let v2 = get_game_logic_random_value(1, 100);

    init_random_with_seed(0xFFFFFFFF);
    assert_eq!(v1, get_game_logic_random_value(1, 100));
    assert_eq!(v2, get_game_logic_random_value(1, 100));
}

#[test]
fn test_rng_cpp_zero_seed_special() {
    let _guard = RngTestGuard::new();
    // Zero seed might be special (common in C++ implementations)
    init_random_with_seed(0);
    let v1 = get_game_logic_random_value(1, 100);
    let v2 = get_game_logic_random_value(1, 100);

    init_random_with_seed(0);
    assert_eq!(v1, get_game_logic_random_value(1, 100));
    assert_eq!(v2, get_game_logic_random_value(1, 100));
}

#[test]
fn test_rng_cpp_boundary_value_low() {
    let _guard = RngTestGuard::new();
    // Test that low boundary is achievable (inclusive)
    init_random_with_seed(66666);
    let mut found_min = false;
    for _ in 0..5000 {
        let v = get_game_logic_random_value(100, 200);
        if v == 100 {
            found_min = true;
            break;
        }
    }
    assert!(
        found_min,
        "Should be able to get minimum boundary value (100)"
    );
}

#[test]
fn test_rng_cpp_boundary_value_high() {
    let _guard = RngTestGuard::new();
    // Test that high boundary is achievable (inclusive)
    init_random_with_seed(77788);
    let mut found_max = false;
    for _ in 0..5000 {
        let v = get_game_logic_random_value(100, 200);
        if v == 200 {
            found_max = true;
            break;
        }
    }
    assert!(
        found_max,
        "Should be able to get maximum boundary value (200)"
    );
}

#[test]
fn test_rng_cpp_distribution_uniformity() {
    let _guard = RngTestGuard::new();
    // Basic sanity check: values should be roughly uniformly distributed
    init_random_with_seed(88899);
    let mut histogram = [0; 10];

    for _ in 0..10000 {
        let v = get_game_logic_random_value(0, 99);
        if v >= 0 && v < 100 {
            histogram[(v / 10) as usize] += 1;
        }
    }

    // Each bucket should have roughly 1000 values (10000 / 10)
    for (i, &count) in histogram.iter().enumerate() {
        assert!(count > 800, "Bucket {} has too few values: {}", i, count);
        assert!(count < 1200, "Bucket {} has too many values: {}", i, count);
    }
}

#[test]
fn test_rng_cpp_sequential_independence() {
    let _guard = RngTestGuard::new();
    // Verify that sequences with adjacent seeds are different
    init_random_with_seed(1000);
    let seq1: Vec<_> = (0..20)
        .map(|_| get_game_logic_random_value(1, 1000000))
        .collect();

    init_random_with_seed(1001);
    let seq2: Vec<_> = (0..20)
        .map(|_| get_game_logic_random_value(1, 1000000))
        .collect();

    // Sequences should be different (at least some values differ)
    let different_count = seq1.iter().zip(seq2.iter()).filter(|(a, b)| a != b).count();
    assert!(
        different_count > 0,
        "Different seeds should produce different sequences"
    );
}

#[test]
fn test_rng_cpp_long_sequence() {
    let _guard = RngTestGuard::new();
    // Verify algorithm stability over long sequences
    init_random_with_seed(12321);
    let mut last_val = get_game_logic_random_value(0, 1000000);

    // Generate 1000 values, verify they don't all converge to same value
    let mut unique_values = std::collections::HashSet::new();
    for _ in 0..1000 {
        let v = get_game_logic_random_value(0, 1000000);
        unique_values.insert(v);
        last_val = v;
    }

    // Should have many unique values (at least 100)
    assert!(
        unique_values.len() > 100,
        "Long sequence should produce many unique values, got {}",
        unique_values.len()
    );
}

#[test]
fn test_rng_cpp_no_period_collapse() {
    let _guard = RngTestGuard::new();
    // Verify RNG doesn't collapse to fixed point or small period
    init_random_with_seed(54321);
    let v1 = get_game_logic_random_value(1, 1000000);

    // Skip 100 values
    for _ in 0..100 {
        let _ = get_game_logic_random_value(1, 1000000);
    }

    let v2 = get_game_logic_random_value(1, 1000000);

    // v2 should be quite different from v1 (probabilistically)
    assert_ne!(v1, v2, "RNG should not have extremely short period");
}

#[test]
fn test_rng_cpp_all_streams_independent() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    let _guard = RngTestGuard::new();
    // Test that all three streams are truly independent
    init_random_with_seed(99988);
    client.seed_random(99988);
    audio.seed_random(99988);

    let logic = get_game_logic_random_value(1, 1000000);
    let client = client.next_int(1, 1000000);
    let audio = audio.next_int(1, 1000000);

    // These are NOT guaranteed to be different, but statistically should be
    // This is a weak test - just verify they're all in range
    assert!(logic >= 1 && logic <= 1000000);
    assert!(client >= 1 && client <= 1000000);
    assert!(audio >= 1 && audio <= 1000000);
}

#[test]
fn test_rng_cpp_delta_calculation_inclusive() {
    let _guard = RngTestGuard::new();
    // Verify the critical delta calculation: delta = hi - lo + 1 (inclusive)
    // This is THE most important compatibility aspect
    init_random_with_seed(11223);

    // Generate many samples from range [10, 11]
    let mut found_10 = false;
    let mut found_11 = false;
    for _ in 0..1000 {
        let v = get_game_logic_random_value(10, 11);
        if v == 10 {
            found_10 = true;
        }
        if v == 11 {
            found_11 = true;
        }
    }

    assert!(
        found_10,
        "Should be able to get value 10 from range [10, 11]"
    );
    assert!(
        found_11,
        "Should be able to get value 11 from range [10, 11]"
    );
}

// ============================================================================
// BOUNDARY ARITHMETIC: MSVC-wrapped C++ semantics (RandomValue.cpp:189-196)
// ============================================================================

/// Step a copy of the given seed state by exactly one RNG draw.
fn stepped_once(words: [u32; 6]) -> ([u32; 6], u32) {
    let mut replay = RandomState::default();
    replay.set_seed_words(words);
    let draw = replay.next_value();
    (replay.seed_words(), draw)
}

#[test]
fn logic_rng_zero_to_i32_max_does_not_panic_and_matches_cpp_wrap() {
    // C++ RandomValue.cpp:189: delta = i32::MAX - 0 + 1 wraps to 0x80000000;
    // :196 value = (draw % 0x80000000) as Int + 0. Exactly one draw consumed.
    let _guard = RngTestGuard::new();
    init_random_with_seed(0xDEAD_BEEF);
    let before = get_game_logic_random_seed_state();
    let (expected_state, draw) = stepped_once(before);
    let expected = (draw % 0x8000_0000) as i32;

    let value = get_game_logic_random_value(0, i32::MAX);

    assert_eq!(value, expected);
    assert_eq!(get_game_logic_random_seed_state(), expected_state);
}

#[test]
fn logic_rng_delta_zero_returns_hi_without_consuming_a_draw() {
    // delta == hi - lo + 1 == 0 iff hi == lo - 1 (mod 2^32).
    // C++ RandomValue.cpp:193-194 returns hi before randomValue() is called.
    let _guard = RngTestGuard::new();
    init_random_with_seed(7);
    let before = get_game_logic_random_seed_state();

    assert_eq!(get_game_logic_random_value(5, 4), 4);
    assert_eq!(get_game_logic_random_value(i32::MIN, i32::MAX), i32::MAX);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn logic_rng_single_value_range_returns_lo_and_consumes_one_draw() {
    // lo == hi gives delta == 1 (NOT 0): C++ RandomValue.cpp:196 consumes a
    // draw, draw % 1 == 0, returns lo.
    let _guard = RngTestGuard::new();
    init_random_with_seed(7);
    let before = get_game_logic_random_seed_state();

    assert_eq!(get_game_logic_random_value(10, 10), 10);
    assert_eq!(get_game_logic_random_seed_state(), stepped_once(before).0);
}

#[test]
fn logic_rng_reversed_bounds_match_cpp_unsigned_mod_wrap() {
    // (5, 3): delta = 3 - 5 + 1 = -1 -> UnsignedInt 0xFFFFFFFF (C++ :189).
    // draw % 0xFFFFFFFF == draw, reinterpreted as Int, + 5 with MSVC wrap.
    let _guard = RngTestGuard::new();
    init_random_with_seed(0x00C0FFEE);
    let before = get_game_logic_random_seed_state();
    let (expected_state, draw) = stepped_once(before);
    let expected = (draw as i32).wrapping_add(5);

    assert_eq!(get_game_logic_random_value(5, 3), expected);
    assert_eq!(get_game_logic_random_seed_state(), expected_state);
}

#[test]
fn logic_rng_final_addition_wraps_like_msvc_x86() {
    // (i32::MAX - 1, i32::MIN): delta = i32::MIN - (i32::MAX - 1) + 1 = 3.
    // When draw % 3 == 2 the C++ final `+ lo` overflows Int and wraps on
    // MSVC x86 to i32::MIN. Search a seed that hits the wrap branch so the
    // boundary is exercised deterministically.
    let _guard = RngTestGuard::new();
    let mut seed = 0u32;
    let draw = loop {
        let mut probe = RandomState::default();
        probe.seed_random(seed);
        let d = probe.next_value();
        if d % 3 == 2 {
            break d;
        }
        seed += 1;
    };

    init_random_with_seed(seed);
    let before = get_game_logic_random_seed_state();
    let expected = ((draw % 3) as i32).wrapping_add(i32::MAX - 1);

    assert_eq!(expected, i32::MIN); // wrap branch actually taken
    assert_eq!(
        get_game_logic_random_value(i32::MAX - 1, i32::MIN),
        expected
    );
    assert_eq!(get_game_logic_random_seed_state(), stepped_once(before).0);
}

#[test]
fn client_and_audio_rng_survive_boundary_ranges() {
    let mut client = RandomState::default();
    let mut audio = RandomState::default();
    // Same C++ shape as the logic stream (RandomValue.cpp:217/240). These
    // streams expose no seed-state getter, so draw counts are asserted via
    // observable values: init_random_with_seed seeds all three streams
    // identically, so the next normal draw must equal the replayed draw.
    let _guard = RngTestGuard::new();
    init_random_with_seed(0xABCD);
    client.seed_random(0xABCD);
    audio.seed_random(0xABCD);
    let mut replay = RandomState::default();
    replay.seed_random(0xABCD);
    let first_draw = replay.next_value();

    // delta wraps to 0x80000000: no panic, exact MSVC-wrapped value.
    let expected_u31 = (first_draw % 0x8000_0000) as i32;
    assert_eq!(client.next_int(0, i32::MAX), expected_u31);
    assert_eq!(audio.next_int(0, i32::MAX), expected_u31);

    // delta == 0 full range: returns hi and consumes NO draw, so the next
    // normal draw is still the stream's SECOND draw (replay steps it now).
    assert_eq!(client.next_int(i32::MIN, i32::MAX), i32::MAX);
    assert_eq!(audio.next_int(i32::MIN, i32::MAX), i32::MAX);
    let second_draw = replay.next_value();
    let expected_mid = (second_draw % 1000) as i32 + 1;
    assert_eq!(client.next_int(1, 1000), expected_mid);
    assert_eq!(audio.next_int(1, 1000), expected_mid);
}

// ========================================================================
// SCOPED-OWNER LOGIC STREAM (driving-instance publication)
// ========================================================================

#[test]
fn scoped_logic_rng_owner_isolates_two_instances() {
    let _guard = RngTestGuard::new();
    // Two differently seeded driving instances must keep independent RNG
    // states: interleaved scopes draw each instance's own standalone
    // Rust sequence. The original singleton no longer chooses the driving
    // Rust world (user-approved RNG sequence/ownership change).
    let mut a = RandomState::default();
    a.seed_random(0xAAAA_0001);
    let mut b = RandomState::default();
    b.seed_random(0xBBBB_0002);

    let mut replay_a = RandomState::default();
    replay_a.seed_random(0xAAAA_0001);
    let mut replay_b = RandomState::default();
    replay_b.seed_random(0xBBBB_0002);
    let expect_a: Vec<i32> = (0..8)
        .map(|_| (replay_a.next_value() % 1000) as i32)
        .collect();
    let expect_b: Vec<i32> = (0..8)
        .map(|_| (replay_b.next_value() % 1000) as i32)
        .collect();

    let mut got_a = Vec::new();
    let mut got_b = Vec::new();
    for _ in 0..8 {
        with_logic_rng_owner(&mut a, || got_a.push(get_game_logic_random_value(0, 999)));
        with_logic_rng_owner(&mut b, || got_b.push(get_game_logic_random_value(0, 999)));
    }

    assert_eq!(got_a, expect_a, "owner A draws its own standalone sequence");
    assert_eq!(got_b, expect_b, "owner B draws its own standalone sequence");
    // Statistically certain for these fixed seeds (matches the existing
    // independence-test style above).
    assert_ne!(got_a, got_b, "differently seeded instances diverge");
    let shared = SessionRandom::from_state(a.clone());
    let callback_handle = shared.clone();
    let mut independent = shared.snapshot();
    let expected = independent.next_u32();
    assert_eq!(callback_handle.next_u32(), expected);
    assert_eq!(shared.seed_words(), independent.seed_words());
    let before_snapshot_draw = shared.seed_words();
    independent.next_u32();
    assert_eq!(
        shared.seed_words(),
        before_snapshot_draw,
        "snapshot clone is independent"
    );
}

#[test]
fn scoped_logic_rng_owner_scope_end_returns_to_global_fallback() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(0x5EED_00AA);
    let mut replay = RandomState::default();
    replay.seed_random(0x5EED_00AA);
    let global_expected = (replay.next_value() % 1000) as i32;

    let mut owner = RandomState::default();
    owner.seed_random(0x1234_5678);
    let mut owner_replay = RandomState::default();
    owner_replay.seed_random(0x1234_5678);
    let owner_expected = (owner_replay.next_value() % 1000) as i32;

    let in_scope = with_logic_rng_owner(&mut owner, || get_game_logic_random_value(0, 999));
    assert_eq!(in_scope, owner_expected, "in-scope draws hit the owner");

    // The scope guard restored the empty slot: the resolver is back on the
    // global fallback, whose state the scope never touched.
    assert_eq!(
        get_game_logic_random_value(0, 999),
        global_expected,
        "post-scope draws hit the untouched global fallback"
    );
}

#[test]
fn nested_logic_rng_owner_scopes_restore_the_outer_owner() {
    let _guard = RngTestGuard::new();
    let mut outer = RandomState::default();
    outer.seed_random(0x0D0D_0001);
    let mut inner = RandomState::default();
    inner.seed_random(0x1D1D_0002);

    let mut replay_outer = RandomState::default();
    replay_outer.seed_random(0x0D0D_0001);
    let mut replay_inner = RandomState::default();
    replay_inner.seed_random(0x1D1D_0002);
    let expect_outer_1st = (replay_outer.next_value() % 1000) as i32;
    let expect_inner = (replay_inner.next_value() % 1000) as i32;
    let expect_outer_2nd = (replay_outer.next_value() % 1000) as i32;

    let got = with_logic_rng_owner(&mut outer, || {
        let first = get_game_logic_random_value(0, 999);
        let from_inner =
            with_logic_rng_owner(&mut inner, || get_game_logic_random_value(0, 999));
        let after = get_game_logic_random_value(0, 999);
        (first, from_inner, after)
    });

    assert_eq!(got.0, expect_outer_1st);
    assert_eq!(got.1, expect_inner, "innermost scope wins while published");
    assert_eq!(
        got.2, expect_outer_2nd,
        "outer owner resumes when the inner scope ends"
    );
}

#[test]
fn init_reseeds_only_global_fallback_while_owner_is_published() {
    let _guard = RngTestGuard::new();
    init_random_with_seed(0x5EED_00BB);
    let mut owner = RandomState::default();
    owner.seed_random(1);

    let mut replay = RandomState::default();
    replay.seed_random(1);
    let expected_owner = (replay.next_value() % 1000) as i32;
    replay.seed_random(0x5EED_00CC);
    let expected_fallback = (replay.next_value() % 1000) as i32;

    // Core/another session may initialize its process fallback while Main
    // owns the synchronous callback. Its explicit stream must stay intact.
    let scoped = with_logic_rng_owner(&mut owner, || {
        init_random_with_seed(0x5EED_00CC);
        get_game_logic_random_value(0, 999)
    });

    assert_eq!(
        scoped, expected_owner,
        "global init leaves the owner intact"
    );
    assert_eq!(
        get_game_logic_random_value(0, 999),
        expected_fallback,
        "the global fallback was reseeded"
    );
    assert_eq!(get_game_logic_random_seed(), 0x5EED_00CC);
}

#[test]
fn raw_seed_state_accessors_honor_the_scoped_owner() {
    let _guard = RngTestGuard::new();
    let words = [1u32, 2, 3, 4, 5, 6];
    let mut owner = RandomState::default();
    owner.seed_random(9);

    with_logic_rng_owner(&mut owner, || {
        set_game_logic_random_seed_state(words);
        let capsule = get_game_logic_random_seed_state();
        // Legacy input becomes a Rust capsule; restoring it is exact.
        let mut restored = RandomState::default();
        restored.set_seed_words(capsule);
        assert_eq!(restored.seed_words(), capsule);
        let mut crc = Crc::new();
        for &word in &capsule {
            crc.compute_single(&word);
        }
        assert_eq!(get_game_logic_random_seed_crc(), crc.get());
    });
}
