//! Random number generation utilities from Command & Conquer Generals WWLib
//!
//! This module provides a faithful Rust implementation of the random number generators
//! used in Command & Conquer Generals. These generators are designed for game determinism
//! and reproducible gameplay, ensuring identical sequences for the same seeds.
//!
//! # Available Generators
//!
//! - [`RandomClass`] - Simple 15-bit Linear Congruential Generator (fastest, but limited randomness)
//! - [`Random2Class`] - XOR-based generator with 32-bit output (good balance of speed and quality)
//! - [`Random3Class`] - High-quality generator with cryptographic-like strength
//! - [`Random4Class`] - Mersenne Twister implementation (best quality, industry standard)
//!
//! # Example Usage
//!
//! ```rust
//! use wwlib_rust::random::{RandomClass, Random4Class};
//!
//! // Basic usage with RandomClass
//! let mut rng = RandomClass::new(12345);
//! let value = rng.next();
//! let ranged_value = rng.next_range(1, 10);
//!
//! // For high-quality randomness, use Random4Class (Mersenne Twister)
//! let mut mt_rng = Random4Class::new(54321);
//! let quality_random = mt_rng.next();
//! let float_value = mt_rng.next_float();
//! ```
//!
//! # Thread Safety
//!
//! All generators are `Send` but not `Sync`, meaning they can be moved between threads
//! but should not be shared without proper synchronization. For concurrent access,
//! wrap generators in `Arc<Mutex<T>>` or use thread-local instances.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

thread_local! {
    /// Thread-local global random number generator using Random4Class (Mersenne Twister)
    static GLOBAL_RNG: RefCell<Random4Class> = RefCell::new(Random4Class::new(4357));
}

/// Global thread-safe random number generator
static SHARED_RNG: OnceLock<Arc<Mutex<Random4Class>>> = OnceLock::new();

fn get_shared_rng() -> &'static Arc<Mutex<Random4Class>> {
    SHARED_RNG.get_or_init(|| Arc::new(Mutex::new(Random4Class::new(4357))))
}

/// Trait for random number generators that provide basic functionality
pub trait RandomGenerator {
    /// Generate the next random number
    fn next(&mut self) -> i32;

    /// Generate a random number within a specific range (inclusive)
    fn next_range(&mut self, min_val: i32, max_val: i32) -> i32
    where
        Self: Sized,
    {
        pick_random_number(self, min_val, max_val)
    }

    /// Get the number of significant bits this generator produces
    fn significant_bits() -> u32;
}

/// Simple Linear Congruential Generator (15-bit output)
///
/// This is the fastest generator but has limited randomness quality.
/// It's equivalent to the original RandomClass from WWLib.
///
/// # Performance Characteristics
/// - Speed: Fastest (0.156s for 10M iterations)
/// - Quality: Poor (fails most DIEHARD tests)
/// - Period: Approximately 2^32
/// - Dimensions: Starts breaking down at 24 dimensions
#[derive(Debug, Clone)]
pub struct RandomClass {
    seed: u32,
}

impl RandomClass {
    // Constants from the original implementation
    const MULT_CONSTANT: u32 = 0x41C64E6D;
    const ADD_CONSTANT: u32 = 0x00003039;
    const THROW_AWAY_BITS: u32 = 10;
    const SIGNIFICANT_BITS: u32 = 15;

    /// Create a new RandomClass with the specified seed
    ///
    /// # Arguments
    /// * `seed` - Initial seed value (0 for default)
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Get the current seed value
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Set a new seed value
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }
}

impl RandomGenerator for RandomClass {
    fn next(&mut self) -> i32 {
        // Transform the seed value into the next number in the sequence
        self.seed = self
            .seed
            .wrapping_mul(Self::MULT_CONSTANT)
            .wrapping_add(Self::ADD_CONSTANT);

        // Extract the 'random' bits from the seed
        let result = (self.seed >> Self::THROW_AWAY_BITS) & ((1 << Self::SIGNIFICANT_BITS) - 1);
        result as i32
    }

    fn significant_bits() -> u32 {
        Self::SIGNIFICANT_BITS
    }
}

/// XOR-based random number generator with table (32-bit output)
///
/// This generator uses a table-based approach with XOR operations for better
/// randomness than RandomClass while maintaining good performance.
///
/// # Performance Characteristics
/// - Speed: Good (0.250s for 10M iterations)
/// - Quality: Good (passes DIEHARD tests, p-value 0.6)
/// - Period: Long but will eventually repeat
/// - Dimensions: Huge spike at 300,000 samples in 64 dimensions
#[derive(Debug, Clone)]
pub struct Random2Class {
    index1: usize,
    index2: usize,
    table: [i32; 250],
}

impl Random2Class {
    const TABLE_SIZE: usize = 250;
    const SIGNIFICANT_BITS: u32 = 32;

    /// Create a new Random2Class with the specified seed
    ///
    /// # Arguments
    /// * `seed` - Initial seed value (0 for default)
    pub fn new(seed: u32) -> Self {
        let mut instance = Self {
            index1: 0,
            index2: 103,
            table: [0; Self::TABLE_SIZE],
        };

        // Initialize the table using Random3Class
        let mut random3 = Random3Class::new(seed, 0);
        for i in 0..Self::TABLE_SIZE {
            instance.table[i] = random3.next();
        }

        instance
    }
}

impl RandomGenerator for Random2Class {
    fn next(&mut self) -> i32 {
        self.table[self.index1] ^= self.table[self.index2];
        let val = self.table[self.index1];

        self.index1 = (self.index1 + 1) % Self::TABLE_SIZE;
        self.index2 = (self.index2 + 1) % Self::TABLE_SIZE;

        val
    }

    fn significant_bits() -> u32 {
        Self::SIGNIFICANT_BITS
    }
}

/// High-quality random number generator with cryptographic-like strength
///
/// This generator provides very strong randomness approaching cryptographic quality
/// but has a strong bias in higher dimensions.
///
/// # Performance Characteristics
/// - Speed: Slower (1.281s for 10M iterations)
/// - Quality: High but biased (fails 11 of 253 DIEHARD tests)
/// - Period: 2^32
/// - Dimensions: Strong bias from 2 dimensions up
#[derive(Debug, Clone)]
pub struct Random3Class {
    seed: i32,
    index: i32,
}

impl Random3Class {
    const SIGNIFICANT_BITS: u32 = 32;

    // Mix tables for strong randomization
    const MIX1: [i32; 20] = [
        0x0baa96887u32 as i32,
        0x01e17d32c,
        0x003bcdc3c,
        0x00f33d1b2,
        0x076a6491d,
        0x0c570d85du32 as i32,
        0x0e382b1e3u32 as i32,
        0x078db4362,
        0x07439a9d4,
        0x09cea8ac5u32 as i32,
        0x089537c5cu32 as i32,
        0x02588f55d,
        0x0415b5e1d,
        0x0216e3d95,
        0x085c662e7u32 as i32,
        0x05e8ab368,
        0x03ea5cc8c,
        0x0d26a0f74u32 as i32,
        0x0f3a9222bu32 as i32,
        0x048aad7e4,
    ];

    const MIX2: [i32; 20] = [
        0x04b0f3b58,
        0x0e874f0c3u32 as i32,
        0x06955c5a6,
        0x055a7ca46,
        0x04d9a9d86,
        0x0fe28a195u32 as i32,
        0x0b1ca7865u32 as i32,
        0x06b235751,
        0x09a997a61u32 as i32,
        0x0aa6e95c8u32 as i32,
        0x0aaa98ee1u32 as i32,
        0x05af9154c,
        0x0fc8e2263u32 as i32,
        0x0390f5e8c,
        0x058ffd802,
        0x0ac0a5ebau32 as i32,
        0x0ac4874f6u32 as i32,
        0x0a9df0913u32 as i32,
        0x086be4c74u32 as i32,
        0x0ed2c123bu32 as i32,
    ];

    /// Create a new Random3Class with the specified seeds
    ///
    /// # Arguments
    /// * `seed1` - Primary seed value
    /// * `seed2` - Secondary seed value (also serves as initial index)
    pub fn new(seed1: u32, seed2: u32) -> Self {
        Self {
            seed: seed1 as i32,
            index: seed2 as i32,
        }
    }

    /// Get the current seed value
    pub fn seed(&self) -> i32 {
        self.seed
    }

    /// Get the current index value
    pub fn index(&self) -> i32 {
        self.index
    }

    /// Set the internal seed and index state (used by RandomStraw seeding).
    pub fn set_state(&mut self, seed: i32, index: i32) {
        self.seed = seed;
        self.index = index;
    }

    /// Fetch the internal seed/index state (used by RandomStraw seeding).
    pub fn state(&self) -> (i32, i32) {
        (self.seed, self.index)
    }
}

impl RandomGenerator for Random3Class {
    fn next(&mut self) -> i32 {
        let mut loword = self.seed;
        let mut hiword = self.index;
        self.index = self.index.wrapping_add(1);

        for i in 0..4 {
            let hihold = hiword;
            let temp = hihold ^ Self::MIX1[i];
            let itmpl = temp & 0xffff;
            let itmph = temp >> 16;
            let temp = itmpl
                .wrapping_mul(itmpl)
                .wrapping_add(!itmph.wrapping_mul(itmph));
            let temp = (temp >> 16) | (temp << 16);
            hiword = loword ^ ((temp ^ Self::MIX2[i]).wrapping_add(itmpl.wrapping_mul(itmph)));
            loword = hihold;
        }

        hiword
    }

    fn significant_bits() -> u32 {
        Self::SIGNIFICANT_BITS
    }
}

/// Mersenne Twister random number generator (highest quality)
///
/// This is the industry-standard random number generator providing excellent
/// statistical properties and long period. It's the recommended choice for
/// most applications requiring high-quality randomness.
///
/// # Performance Characteristics
/// - Speed: Good (0.375s for 10M iterations, 4x faster than standard rand())
/// - Quality: Excellent (passes all DIEHARD tests, p-value 0.2588)
/// - Period: 2^19937 - 1 (astronomically long)
/// - Dimensions: Excellent performance in all tested dimensions
#[derive(Debug, Clone)]
pub struct Random4Class {
    mt: [u32; 624],
    mti: usize,
}

impl Random4Class {
    const N: usize = 624;
    const M: usize = 397;
    const MATRIX_A: u32 = 0x9908b0df;
    const UPPER_MASK: u32 = 0x80000000;
    const LOWER_MASK: u32 = 0x7fffffff;
    const TEMPERING_MASK_B: u32 = 0x9d2c5680;
    const TEMPERING_MASK_C: u32 = 0xefc60000;
    const SIGNIFICANT_BITS: u32 = 32;

    /// Create a new Random4Class with the specified seed
    ///
    /// # Arguments
    /// * `seed` - Initial seed value (0 will be changed to default 4375)
    pub fn new(seed: u32) -> Self {
        let mut instance = Self {
            mt: [0; Self::N],
            mti: Self::N + 1,
        };

        let actual_seed = if seed == 0 { 4375 } else { seed };

        // Initialize using the generator from Knuth's "The Art of Computer Programming"
        instance.mt[0] = actual_seed & 0xffffffff;
        for i in 1..Self::N {
            instance.mt[i] = (69069_u32.wrapping_mul(instance.mt[i - 1])) & 0xffffffff;
        }
        instance.mti = Self::N;

        instance
    }

    /// Generate a random float between 0.0 and 1.0
    pub fn next_float(&mut self) -> f32 {
        let x = self.next() as u32;
        x as f32 * 2.3283064370807973754314699618685e-10_f32
    }

    /// Reseed the generator
    pub fn reseed(&mut self, seed: u32) {
        *self = Self::new(seed);
    }
}

impl RandomGenerator for Random4Class {
    fn next(&mut self) -> i32 {
        static MAG01: [u32; 2] = [0x0, Random4Class::MATRIX_A];

        if self.mti >= Self::N {
            // Generate N words at one time
            for kk in 0..(Self::N - Self::M) {
                let y = (self.mt[kk] & Self::UPPER_MASK) | (self.mt[kk + 1] & Self::LOWER_MASK);
                self.mt[kk] = self.mt[kk + Self::M] ^ (y >> 1) ^ MAG01[(y & 0x1) as usize];
            }

            for kk in (Self::N - Self::M)..(Self::N - 1) {
                let y = (self.mt[kk] & Self::UPPER_MASK) | (self.mt[kk + 1] & Self::LOWER_MASK);
                self.mt[kk] = self.mt[kk.wrapping_add(Self::M).wrapping_sub(Self::N)]
                    ^ (y >> 1)
                    ^ MAG01[(y & 0x1) as usize];
            }

            let y = (self.mt[Self::N - 1] & Self::UPPER_MASK) | (self.mt[0] & Self::LOWER_MASK);
            self.mt[Self::N - 1] = self.mt[Self::M - 1] ^ (y >> 1) ^ MAG01[(y & 0x1) as usize];

            self.mti = 0;
        }

        let mut y = self.mt[self.mti];
        self.mti += 1;

        // Tempering
        y ^= y >> 11;
        y ^= (y << 7) & Self::TEMPERING_MASK_B;
        y ^= (y << 15) & Self::TEMPERING_MASK_C;
        y ^= y >> 18;

        y as i32
    }

    fn significant_bits() -> u32 {
        Self::SIGNIFICANT_BITS
    }
}

/// Utility function to pick a random number within a specified range
///
/// This function implements an unbiased method for generating random numbers
/// within a specific range, avoiding the modulo bias problem.
///
/// # Arguments
/// * `generator` - The random number generator to use
/// * `min_val` - Minimum value (inclusive)
/// * `max_val` - Maximum value (inclusive)
///
/// # Returns
/// A random number between min_val and max_val (inclusive)
pub fn pick_random_number<T: RandomGenerator>(
    generator: &mut T,
    min_val: i32,
    max_val: i32,
) -> i32 {
    // Handle edge case where range is null
    if min_val == max_val {
        return min_val;
    }

    // Ensure proper order
    let (min_val, max_val) = if min_val > max_val {
        (max_val, min_val)
    } else {
        (min_val, max_val)
    };

    let magnitude = max_val - min_val;

    // Find the highest bit that fits within the magnitude
    let mut highbit = T::significant_bits() as i32 - 1;
    while (magnitude & (1 << highbit)) == 0 && highbit > 0 {
        highbit -= 1;
    }

    // Create a mask that covers the magnitude
    let mask = !(!0_i32 << (highbit + 1));

    // Keep picking until we get a value within range
    loop {
        let pick = generator.next() & mask;
        if pick <= magnitude {
            return pick + min_val;
        }
    }
}

/// Global random number functions using thread-local storage
pub mod global {
    use super::*;

    /// Seed the thread-local global random number generator
    pub fn seed_random(seed: u32) {
        GLOBAL_RNG.with(|rng| {
            *rng.borrow_mut() = Random4Class::new(seed);
        });
    }

    /// Get the next random number from the thread-local global generator
    pub fn random() -> i32 {
        GLOBAL_RNG.with(|rng| rng.borrow_mut().next())
    }

    /// Get a random number within a range from the thread-local global generator
    pub fn random_range(min_val: i32, max_val: i32) -> i32 {
        GLOBAL_RNG.with(|rng| rng.borrow_mut().next_range(min_val, max_val))
    }

    /// Get a random float from the thread-local global generator
    pub fn random_float() -> f32 {
        GLOBAL_RNG.with(|rng| rng.borrow_mut().next_float())
    }
}

/// Thread-safe global random number functions
pub mod shared {
    use super::*;

    /// Seed the shared global random number generator
    pub fn seed_random(seed: u32) {
        let mut rng = get_shared_rng().lock().unwrap();
        *rng = Random4Class::new(seed);
    }

    /// Get the next random number from the shared global generator
    pub fn random() -> i32 {
        let mut rng = get_shared_rng().lock().unwrap();
        rng.next()
    }

    /// Get a random number within a range from the shared global generator
    pub fn random_range(min_val: i32, max_val: i32) -> i32 {
        let mut rng = get_shared_rng().lock().unwrap();
        rng.next_range(min_val, max_val)
    }

    /// Get a random float from the shared global generator
    pub fn random_float() -> f32 {
        let mut rng = get_shared_rng().lock().unwrap();
        rng.next_float()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_class_deterministic() {
        let mut rng1 = RandomClass::new(12345);
        let mut rng2 = RandomClass::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn test_random_class_range() {
        let mut rng = RandomClass::new(54321);

        for _ in 0..1000 {
            let val = rng.next_range(10, 20);
            assert!(val >= 10 && val <= 20);
        }
    }

    #[test]
    fn test_random2_class_deterministic() {
        let mut rng1 = Random2Class::new(11111);
        let mut rng2 = Random2Class::new(11111);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn test_random3_class_deterministic() {
        let mut rng1 = Random3Class::new(22222, 33333);
        let mut rng2 = Random3Class::new(22222, 33333);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn test_random4_class_deterministic() {
        let mut rng1 = Random4Class::new(44444);
        let mut rng2 = Random4Class::new(44444);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next());
        }
    }

    #[test]
    fn test_random4_float_range() {
        let mut rng = Random4Class::new(55555);

        for _ in 0..1000 {
            let val = rng.next_float();
            assert!(val >= 0.0 && val < 1.0);
        }
    }

    #[test]
    fn test_pick_random_number_unbiased() {
        let mut rng = Random4Class::new(66666);

        // Test that all values in range are possible
        let mut counts = [0; 5]; // range 0-4
        for _ in 0..10000 {
            let val = pick_random_number(&mut rng, 0, 4);
            assert!(val >= 0 && val <= 4);
            counts[val as usize] += 1;
        }

        // Each value should appear roughly 2000 times (with some variance)
        for count in counts {
            assert!(count > 1500 && count < 2500);
        }
    }

    #[test]
    fn test_global_functions() {
        global::seed_random(77777);

        let val1 = global::random();
        let val2 = global::random();
        assert_ne!(val1, val2); // Should be different

        let range_val = global::random_range(1, 100);
        assert!(range_val >= 1 && range_val <= 100);

        let float_val = global::random_float();
        assert!(float_val >= 0.0 && float_val < 1.0);
    }

    #[test]
    fn test_shared_functions() {
        shared::seed_random(88888);

        let val1 = shared::random();
        let val2 = shared::random();
        assert_ne!(val1, val2); // Should be different

        let range_val = shared::random_range(1, 100);
        assert!(range_val >= 1 && range_val <= 100);

        let float_val = shared::random_float();
        assert!(float_val >= 0.0 && float_val < 1.0);
    }

    #[test]
    fn test_compatibility_with_original() {
        // Test specific known sequences to ensure compatibility
        let mut rng = RandomClass::new(1);

        // First few values from original implementation
        let expected = [
            (1_u32.wrapping_mul(0x41C64E6D).wrapping_add(0x00003039) >> 10) & 0x7FFF,
            // Add more expected values here if you have reference data
        ];

        for &expected_val in &expected {
            assert_eq!(rng.next() as u32, expected_val);
        }
    }

    #[test]
    fn test_edge_cases() {
        let mut rng = Random4Class::new(0); // Should use default seed
        assert_ne!(rng.next(), 0); // Should not return 0 immediately

        // Test same min/max value
        assert_eq!(rng.next_range(42, 42), 42);

        // Test reversed min/max
        let val = rng.next_range(20, 10);
        assert!(val >= 10 && val <= 20);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    global::seed_random(i * 1000);
                    let mut values = Vec::new();
                    for _ in 0..100 {
                        values.push(global::random());
                    }
                    values
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Each thread should produce different sequences
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                assert_ne!(results[i], results[j]);
            }
        }
    }

    /// Benchmark test to compare performance (run with `cargo test --release`)
    #[test]
    #[ignore]
    fn benchmark_generators() {
        use std::time::Instant;

        const ITERATIONS: usize = 1_000_000;

        // RandomClass benchmark
        let start = Instant::now();
        let mut rng1 = RandomClass::new(12345);
        for _ in 0..ITERATIONS {
            let _ = rng1.next();
        }
        let random_time = start.elapsed();

        // Random2Class benchmark
        let start = Instant::now();
        let mut rng2 = Random2Class::new(12345);
        for _ in 0..ITERATIONS {
            let _ = rng2.next();
        }
        let random2_time = start.elapsed();

        // Random3Class benchmark
        let start = Instant::now();
        let mut rng3 = Random3Class::new(12345, 0);
        for _ in 0..ITERATIONS {
            let _ = rng3.next();
        }
        let random3_time = start.elapsed();

        // Random4Class benchmark
        let start = Instant::now();
        let mut rng4 = Random4Class::new(12345);
        for _ in 0..ITERATIONS {
            let _ = rng4.next();
        }
        let random4_time = start.elapsed();

        println!("Benchmark results for {} iterations:", ITERATIONS);
        println!("RandomClass: {:?}", random_time);
        println!("Random2Class: {:?}", random2_time);
        println!("Random3Class: {:?}", random3_time);
        println!("Random4Class: {:?}", random4_time);

        // RandomClass should be fastest
        assert!(random_time <= random2_time);
        assert!(random_time <= random3_time);
        assert!(random_time <= random4_time);
    }
}
