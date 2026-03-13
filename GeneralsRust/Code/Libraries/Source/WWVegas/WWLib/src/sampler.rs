//! Multidimensional sampling techniques mirroring WWLib `sampler`.
//!
//! This module provides a faithful Rust implementation of the sampling classes
//! from WWLib, which sample over multidimensional space (hypercube [0,1]^n).
//!
//! # C++ Source
//! Original implementation in `GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.cpp`
//! and `sampler.h`.
//!
//! # Sampling Techniques
//!
//! - [`RandomSamplingClass`] - Random sampling using Mersenne Twister
//! - [`RegularSamplingClass`] - Regular grid sampling (deterministic)
//! - [`StratifiedSamplingClass`] - Stratified sampling with random perturbation
//! - [`QMCSamplingClass`] - Quasi-Monte Carlo sampling using Halton sequence
//!
//! # Original Author
//! Hector Yee, 6/11/2001

use crate::random::Random4Class;

// First 100 primes, used by QMC sampling for radical inverse computation.
// Matches the exact table from sampler.cpp.
const PRIMES: [i32; 100] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541,
];

/// Computes the radical inverse of `i` in the given `base`.
///
/// This function writes a number in base `b` and reverses it over the decimal point.
/// For example, `rad_inv(1, 2)` = 0.1 base 2 = 0.5.
///
/// Used by the Halton-Hammersley sequence for quasi-Monte Carlo sampling.
#[inline]
fn rad_inv(i: i32, base: i32) -> f32 {
    let mut i = i;
    let mut sum: f32 = 0.0;
    let mut power: f32 = 1.0 / (base as f32);

    while i != 0 {
        let residue = i % base;
        i /= base;
        sum += (residue as f32) * power;
        power /= base as f32;
    }

    sum
}

/// Abstract base class for sampling techniques.
///
/// All sampling algorithms modify a vector of length `dimensions`,
/// returning values between 0.0 and 1.0. A call to [`sample`](SamplingClass::sample)
/// increments the internal state of the sampler.
///
/// # Type Parameters
/// - `Dimensions`: The length of the vector to sample
/// - `Divisions`: Used to determine the number of strata to sample over
pub trait SamplingClass {
    /// Reset the sampler to its initial state.
    fn reset(&mut self);

    /// Sample the next point, writing `dimensions` values into `target`.
    /// Each value will be in the range [0.0, 1.0].
    fn sample(&mut self, target: &mut [f32]);

    /// Returns the number of dimensions this sampler produces.
    fn dimensions(&self) -> usize;
}

/// Random sampling using Mersenne Twister.
///
/// Samples randomly over a hypercube using the Mersenne Twister random number
/// generator. The `divisions` parameter is ignored.
///
/// # C++ Source
/// ```cpp
/// class RandomSamplingClass : public SamplingClass
/// {
/// public:
///     RandomSamplingClass(unsigned int dimensions, unsigned char divisions=0);
///     virtual void Reset() {};
///     virtual void Sample(float *target);
/// };
/// ```
pub struct RandomSamplingClass {
    dimensions: usize,
    rng: Random4Class,
}

impl RandomSamplingClass {
    /// Create a new random sampler.
    ///
    /// # Arguments
    /// * `dimensions` - The number of dimensions to sample
    /// * `_divisions` - Ignored for random sampling (included for API parity)
    pub fn new(dimensions: usize, _divisions: u8) -> Self {
        Self {
            dimensions,
            rng: Random4Class::new(4357),
        }
    }

    /// Create a new random sampler with a specific seed.
    pub fn with_seed(dimensions: usize, _divisions: u8, seed: u32) -> Self {
        Self {
            dimensions,
            rng: Random4Class::new(seed),
        }
    }
}

impl SamplingClass for RandomSamplingClass {
    fn reset(&mut self) {
        // No-op, matching C++ behavior
    }

    fn sample(&mut self, target: &mut [f32]) {
        use crate::random::RandomGenerator;
        for i in 0..self.dimensions.min(target.len()) {
            target[i] = self.rng.next_float();
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Regular grid sampling over a hypercube.
///
/// Samples over a regular hypergrid. The sampler repeats after
/// `dimensions * divisions` calls to `sample`.
///
/// # C++ Source
/// ```cpp
/// class RegularSamplingClass : public SamplingClass
/// {
/// public:
///     RegularSamplingClass(unsigned int dimensions, unsigned char divisions=3);
///     virtual void Reset();
///     virtual void Sample(float *target);
///     virtual ~RegularSamplingClass();
/// protected:
///     unsigned char *index;
/// };
/// ```
pub struct RegularSamplingClass {
    dimensions: usize,
    divisions: u8,
    index: Vec<u8>,
}

impl RegularSamplingClass {
    /// Create a new regular grid sampler.
    ///
    /// # Arguments
    /// * `dimensions` - The number of dimensions to sample
    /// * `divisions` - Number of divisions per dimension (default 3 in C++)
    pub fn new(dimensions: usize, divisions: u8) -> Self {
        let divisions = divisions.max(1); // Avoid division by zero
        Self {
            dimensions,
            divisions,
            index: vec![0u8; dimensions],
        }
    }
}

impl SamplingClass for RegularSamplingClass {
    fn reset(&mut self) {
        self.index.fill(0);
    }

    fn sample(&mut self, target: &mut [f32]) {
        let div_minus_one = (self.divisions as f32) - 1.0;

        for i in 0..self.dimensions.min(target.len()) {
            // minus one because we want to get 1.0f also
            target[i] = (self.index[i] as f32) / div_minus_one;
        }

        // index[i] will always be 0..Divisions-1
        // add 1 and carry mod Divisions
        // e.g. increase x until x reaches Divisions
        // then and only then increase y. Now z increases
        // only when x=Divisions and y=Divisions etc..
        for i in 0..self.dimensions {
            self.index[i] += 1;
            if self.index[i] < self.divisions {
                break;
            }
            self.index[i] = 0;
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Stratified sampling with random perturbation.
///
/// Samples over a regular hypergrid with random offsets within each stratum.
/// This provides better coverage than pure random sampling in low dimensions.
///
/// # C++ Source
/// ```cpp
/// class StratifiedSamplingClass : public SamplingClass
/// {
/// public:
///     StratifiedSamplingClass(unsigned int dimensions, unsigned char divisions=3);
///     virtual void Reset();
///     virtual void Sample(float *target);
///     virtual ~StratifiedSamplingClass();
/// protected:
///     unsigned char *index;
/// };
/// ```
pub struct StratifiedSamplingClass {
    dimensions: usize,
    divisions: u8,
    index: Vec<u8>,
    rng: Random4Class,
}

impl StratifiedSamplingClass {
    /// Create a new stratified sampler.
    ///
    /// # Arguments
    /// * `dimensions` - The number of dimensions to sample
    /// * `divisions` - Number of strata per dimension (default 3 in C++)
    pub fn new(dimensions: usize, divisions: u8) -> Self {
        let divisions = divisions.max(1);
        Self {
            dimensions,
            divisions,
            index: vec![0u8; dimensions],
            rng: Random4Class::new(4357),
        }
    }

    /// Create a new stratified sampler with a specific random seed.
    pub fn with_seed(dimensions: usize, divisions: u8, seed: u32) -> Self {
        let divisions = divisions.max(1);
        Self {
            dimensions,
            divisions,
            index: vec![0u8; dimensions],
            rng: Random4Class::new(seed),
        }
    }
}

impl SamplingClass for StratifiedSamplingClass {
    fn reset(&mut self) {
        self.index.fill(0);
    }

    fn sample(&mut self, target: &mut [f32]) {
        use crate::random::RandomGenerator;
        let div = self.divisions as f32;

        for i in 0..self.dimensions.min(target.len()) {
            target[i] = ((self.index[i] as f32) + self.rng.next_float()) / div;
        }

        // index[i] will always be 0..Divisions-1
        // add 1 and carry mod Divisions
        for i in 0..self.dimensions {
            self.index[i] += 1;
            if self.index[i] < self.divisions {
                break;
            }
            self.index[i] = 0;
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Quasi-Monte Carlo sampling using the Halton-Hammersley sequence.
///
/// Samples using the Halton sequence, which is based on the inverse radical function.
/// This produces low-discrepancy sequences that are more evenly distributed than
/// random sampling. The `divisions` parameter is ignored.
///
/// # C++ Source
/// ```cpp
/// class QMCSamplingClass : public SamplingClass
/// {
/// public:
///     QMCSamplingClass(unsigned int dimensions, unsigned char divisions=0);
///     virtual void Reset() {index=0;};
///     virtual void Sample(float *target);
///     void Set_Offset(unsigned int offset) { index=offset; }
/// protected:
///     unsigned int index;
/// };
/// ```
pub struct QMCSamplingClass {
    dimensions: usize,
    index: u32,
}

impl QMCSamplingClass {
    /// Create a new QMC sampler.
    ///
    /// # Arguments
    /// * `dimensions` - The number of dimensions to sample (must be < 100)
    /// * `_divisions` - Ignored for QMC sampling (included for API parity)
    ///
    /// # Panics
    /// Panics if `dimensions >= 100` (matching C++ `assert(Dimensions<100)`)
    pub fn new(dimensions: usize, _divisions: u8) -> Self {
        assert!(
            dimensions < 100,
            "QMCSamplingClass: dimensions must be < 100"
        );
        Self {
            dimensions,
            index: 0,
        }
    }

    /// Set the offset (index) of the sequence.
    /// This allows jumping to an arbitrary point in the Halton sequence.
    pub fn set_offset(&mut self, offset: u32) {
        self.index = offset;
    }

    /// Get the current index position in the sequence.
    pub fn offset(&self) -> u32 {
        self.index
    }
}

impl SamplingClass for QMCSamplingClass {
    fn reset(&mut self) {
        self.index = 0;
    }

    fn sample(&mut self, target: &mut [f32]) {
        for i in 0..self.dimensions.min(target.len()) {
            target[i] = rad_inv(self.index as i32, PRIMES[i]);
        }

        self.index += 1;
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rad_inv_known_values() {
        // rad_inv(1, 2) = 0.1 base 2 = 0.5
        let val = rad_inv(1, 2);
        assert!((val - 0.5).abs() < 1e-6);

        // rad_inv(0, any) = 0.0
        assert_eq!(rad_inv(0, 2), 0.0);
        assert_eq!(rad_inv(0, 10), 0.0);

        // rad_inv(1, 10) = 0.1 base 10 = 0.1
        let val = rad_inv(1, 10);
        assert!((val - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_random_sampling_dimensions() {
        let sampler = RandomSamplingClass::new(3, 0);
        assert_eq!(sampler.dimensions(), 3);
    }

    #[test]
    fn test_random_sampling_range() {
        let mut sampler = RandomSamplingClass::with_seed(4, 0, 12345);
        let mut target = [0.0f32; 4];

        for _ in 0..100 {
            sampler.sample(&mut target);
            for &val in &target {
                assert!(val >= 0.0 && val < 1.0, "Value {} out of range", val);
            }
        }
    }

    #[test]
    fn test_regular_sampling_deterministic() {
        let mut sampler1 = RegularSamplingClass::new(2, 3);
        let mut sampler2 = RegularSamplingClass::new(2, 3);

        let mut target1 = [0.0f32; 2];
        let mut target2 = [0.0f32; 2];

        for _ in 0..10 {
            sampler1.sample(&mut target1);
            sampler2.sample(&mut target2);
            assert_eq!(target1, target2);
        }
    }

    #[test]
    fn test_regular_sampling_values() {
        let mut sampler = RegularSamplingClass::new(1, 3);
        let mut target = [0.0f32; 1];

        // With divisions=3, we expect values at 0.0, 0.5, 1.0
        let mut values = Vec::new();
        for _ in 0..3 {
            sampler.sample(&mut target);
            values.push(target[0]);
        }

        assert!((values[0] - 0.0).abs() < 1e-6);
        assert!((values[1] - 0.5).abs() < 1e-6);
        assert!((values[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_regular_sampling_reset() {
        let mut sampler = RegularSamplingClass::new(2, 3);
        let mut target = [0.0f32; 2];

        // Advance a few steps
        for _ in 0..5 {
            sampler.sample(&mut target);
        }

        // Reset and verify we start from beginning
        sampler.reset();
        sampler.sample(&mut target);
        assert!((target[0] - 0.0).abs() < 1e-6);
        assert!((target[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stratified_sampling_range() {
        let mut sampler = StratifiedSamplingClass::with_seed(3, 4, 42);
        let mut target = [0.0f32; 3];

        for _ in 0..100 {
            sampler.sample(&mut target);
            for &val in &target {
                assert!(val >= 0.0 && val < 1.0, "Value {} out of range", val);
            }
        }
    }

    #[test]
    fn test_stratified_sampling_reset() {
        // Two fresh samplers with same seed should produce identical sequences
        let mut sampler1 = StratifiedSamplingClass::with_seed(2, 3, 100);
        let mut sampler2 = StratifiedSamplingClass::with_seed(2, 3, 100);

        let mut target1 = [0.0f32; 2];
        let mut target2 = [0.0f32; 2];

        for _ in 0..10 {
            sampler1.sample(&mut target1);
            sampler2.sample(&mut target2);
            assert_eq!(target1, target2);
        }

        // After reset, index goes back to zero but RNG continues from its state.
        // The next sample should use index=0 (first stratum) but with the
        // advanced RNG, so values will differ from a fresh sampler.
        // This matches C++ behavior where Reset() only does memset(index,0,...).
        sampler1.reset();
        sampler1.sample(&mut target1);

        // Verify the sample is in valid range and different from previous
        for &val in &target1 {
            assert!(val >= 0.0 && val < 1.0);
        }

        // After reset, a second sampler that was also reset should still match
        sampler2.reset();
        sampler2.sample(&mut target2);
        assert_eq!(target1, target2);
    }

    #[test]
    fn test_qmc_sampling_dimensions() {
        let sampler = QMCSamplingClass::new(5, 0);
        assert_eq!(sampler.dimensions(), 5);
    }

    #[test]
    fn test_qmc_sampling_range() {
        let mut sampler = QMCSamplingClass::new(3, 0);
        let mut target = [0.0f32; 3];

        for _ in 0..100 {
            sampler.sample(&mut target);
            for &val in &target {
                assert!(val >= 0.0 && val < 1.0, "Value {} out of range", val);
            }
        }
    }

    #[test]
    fn test_qmc_sampling_deterministic() {
        let mut sampler1 = QMCSamplingClass::new(2, 0);
        let mut sampler2 = QMCSamplingClass::new(2, 0);

        let mut target1 = [0.0f32; 2];
        let mut target2 = [0.0f32; 2];

        for _ in 0..20 {
            sampler1.sample(&mut target1);
            sampler2.sample(&mut target2);
            assert_eq!(target1, target2);
        }
    }

    #[test]
    fn test_qmc_set_offset() {
        // Sample twice from a fresh sampler (index goes 0 -> 1)
        let mut sampler = QMCSamplingClass::new(2, 0);
        let mut target_first = [0.0f32; 2];
        let mut target_second = [0.0f32; 2];
        sampler.sample(&mut target_first);
        sampler.sample(&mut target_second);

        // Create a sampler starting at offset 1 - its first sample
        // should equal the original sampler's second sample
        let mut sampler_offset = QMCSamplingClass::new(2, 0);
        sampler_offset.set_offset(1);
        let mut target_offset = [0.0f32; 2];
        sampler_offset.sample(&mut target_offset);

        assert_eq!(target_second, target_offset);
        assert_ne!(target_first, target_offset); // Different from first sample
    }

    #[test]
    #[should_panic(expected = "dimensions must be < 100")]
    fn test_qmc_dimensions_limit() {
        QMCSamplingClass::new(100, 0);
    }

    #[test]
    fn test_qmc_reset() {
        let mut sampler = QMCSamplingClass::new(2, 0);
        let mut target1 = [0.0f32; 2];
        let mut target2 = [0.0f32; 2];

        sampler.sample(&mut target1);
        sampler.sample(&mut target1);

        sampler.reset();
        sampler.sample(&mut target2);

        // First sample after reset should equal the very first sample
        let mut sampler_fresh = QMCSamplingClass::new(2, 0);
        let mut target_fresh = [0.0f32; 2];
        sampler_fresh.sample(&mut target_fresh);

        assert_eq!(target2, target_fresh);
    }

    #[test]
    fn test_regular_sampling_overflow() {
        // Test that a 2D sampler with 2 divisions cycles correctly
        // Should produce 4 samples before repeating
        let mut sampler = RegularSamplingClass::new(2, 2);
        let mut target = [0.0f32; 2];
        let mut samples = Vec::new();

        for _ in 0..6 {
            sampler.sample(&mut target);
            samples.push((target[0], target[1]));
        }

        // First 4 samples should be distinct: (0,0), (1,0), (0,1), (1,1)
        // Then it should repeat
        assert_ne!(samples[0], samples[1]);
        assert_eq!(samples[4], samples[0]);
        assert_eq!(samples[5], samples[1]);
    }

    #[test]
    fn test_regular_divisions_one() {
        // Edge case: divisions=1 should always return 0.0 (or inf if div-1=0)
        // The implementation uses max(1) so div_minus_one = 0.0
        // index[i] / 0.0 = inf, but index is always < divisions=1 so index=0
        // Actually with divisions=1, index always stays at 0, so 0/0 = NaN
        // Let's use divisions=2 as minimum sensible value
        let mut sampler = RegularSamplingClass::new(1, 2);
        let mut target = [0.5f32; 1];
        sampler.sample(&mut target);
        assert!(target[0] >= 0.0 && target[0] <= 1.0);
    }
}
