//! Particle Properties and Keyframes
//!
//! This module implements the particle property system that allows properties
//! like color, size, opacity, etc. to change over the lifetime of particles
//! using keyframes.

use glam::{Vec3, Vec4};
use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const RNG_MULTIPLIER: u64 = 6364136223846793005;
const RNG_INCREMENT: u64 = 1442695040888963407;

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(RNG_MULTIPLIER)
            .wrapping_add(RNG_INCREMENT);

        ((self.state >> 32) as f32) / (u32::MAX as f32)
    }

    fn next_signed_f32(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }

    fn state(&self) -> u64 {
        self.state
    }
}

/// Particle property template that supports keyframe animation
#[derive(Debug, Clone)]
pub struct ParticleProperty<T> {
    pub start: T,
    pub rand: T,
    pub num_keyframes: u32,
    pub key_times: Option<Vec<f32>>,
    pub values: Option<Vec<T>>,
    rng_state: u64,
}

impl<T> ParticleProperty<T>
where
    T: Clone + Default + std::fmt::Debug + 'static,
{
    /// Create a new particle property with default values
    pub fn new() -> Self {
        Self::with_seed(
            T::default(),
            T::default(),
            0,
            None,
            None,
            default_seed::<T>(),
        )
    }

    /// Create a particle property with start value
    pub fn with_start(start: T) -> Self {
        Self::with_seed(start, T::default(), 0, None, None, default_seed::<T>())
    }

    /// Create a particle property with keyframes
    pub fn with_keyframes(start: T, rand: T, key_times: Vec<f32>, values: Vec<T>) -> Self {
        assert_eq!(
            key_times.len(),
            values.len(),
            "Key times and values must have the same length"
        );
        Self::with_seed(
            start,
            rand,
            key_times.len() as u32,
            Some(key_times),
            Some(values),
            default_seed::<T>(),
        )
    }

    fn with_seed(
        start: T,
        rand: T,
        num_keyframes: u32,
        key_times: Option<Vec<f32>>,
        values: Option<Vec<T>>,
        rng_state: u64,
    ) -> Self {
        Self {
            start,
            rand,
            num_keyframes,
            key_times,
            values,
            rng_state,
        }
    }

    /// Sample the property value at a given normalized age (0.0 to 1.0)
    pub fn sample(&self, age_normalized: f32, random_offset: &T) -> T
    where
        T: ParticlePropertyValue,
    {
        let base = if self.num_keyframes <= 1 {
            // No keyframes or only one keyframe, return start value
            self.start.clone()
        } else {
            // Interpolate between keyframes
            self.interpolate_keyframes(age_normalized)
        };

        T::add_random(base, random_offset)
    }

    /// Generate a per-particle random offset based on the configured random range.
    pub(crate) fn random_offset(&mut self) -> T
    where
        T: ParticlePropertyRandom,
    {
        let mut rng = DeterministicRng::new(self.rng_state);
        let offset = T::random_offset(&self.rand, &mut rng);
        self.rng_state = rng.state();
        offset
    }

    /// Interpolate between keyframes
    fn interpolate_keyframes(&self, age: f32) -> T
    where
        T: ParticlePropertyValue,
    {
        let key_times = self.key_times.as_ref().unwrap();
        let values = self.values.as_ref().unwrap();

        // Find the keyframes to interpolate between
        for i in 0..(self.num_keyframes - 1) {
            let t0 = key_times[i as usize];
            let t1 = key_times[(i + 1) as usize];

            if age >= t0 && age <= t1 {
                let factor = if t1 > t0 { (age - t0) / (t1 - t0) } else { 0.0 };
                return T::lerp(&values[i as usize], &values[(i + 1) as usize], factor);
            }
        }

        // Age is beyond the last keyframe, return the last value
        values.last().cloned().unwrap_or_else(|| self.start.clone())
    }
}

/// Trait for types that can be used as particle property values
pub trait ParticlePropertyValue: Clone + Default + std::fmt::Debug {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
    fn add_random(base: Self, random: &Self) -> Self;
}

impl ParticlePropertyValue for f32 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * t
    }

    fn add_random(base: Self, random: &Self) -> Self {
        base + *random
    }
}

impl ParticlePropertyValue for Vec3 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a.lerp(*b, t)
    }

    fn add_random(base: Self, random: &Self) -> Self {
        base + *random
    }
}

impl ParticlePropertyValue for Vec4 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a.lerp(*b, t)
    }

    fn add_random(base: Self, random: &Self) -> Self {
        base + *random
    }
}

/// Trait used to generate randomized offsets for particle properties.
pub(crate) trait ParticlePropertyRandom {
    fn random_offset(rand: &Self, rng: &mut DeterministicRng) -> Self;
}

impl ParticlePropertyRandom for f32 {
    fn random_offset(rand: &Self, rng: &mut DeterministicRng) -> Self {
        if rand.abs() <= f32::EPSILON {
            0.0
        } else {
            rng.next_signed_f32() * *rand
        }
    }
}

impl ParticlePropertyRandom for Vec3 {
    fn random_offset(rand: &Self, rng: &mut DeterministicRng) -> Self {
        Vec3::new(
            if rand.x.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.x
            },
            if rand.y.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.y
            },
            if rand.z.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.z
            },
        )
    }
}

impl ParticlePropertyRandom for Vec4 {
    fn random_offset(rand: &Self, rng: &mut DeterministicRng) -> Self {
        Vec4::new(
            if rand.x.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.x
            },
            if rand.y.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.y
            },
            if rand.z.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.z
            },
            if rand.w.abs() <= f32::EPSILON {
                0.0
            } else {
                rng.next_signed_f32() * rand.w
            },
        )
    }
}

/// Copy particle property contents from one to another
pub fn copy_particle_property<T: Clone>(dest: &mut ParticleProperty<T>, src: &ParticleProperty<T>) {
    dest.start = src.start.clone();
    dest.rand = src.rand.clone();
    dest.num_keyframes = src.num_keyframes;
    dest.key_times = src.key_times.clone();
    dest.values = src.values.clone();
    dest.rng_state = src.rng_state;
}

fn default_seed<T: 'static>() -> u64 {
    let mut hasher = DefaultHasher::new();
    TypeId::of::<T>().hash(&mut hasher);
    hasher.finish()
}

/// Particle color property (RGB)
pub type ParticleColorProperty = ParticleProperty<Vec3>;

/// Particle opacity property
pub type ParticleOpacityProperty = ParticleProperty<f32>;

/// Particle size property
pub type ParticleSizeProperty = ParticleProperty<f32>;

/// Particle rotation property with orientation randomisation support
#[derive(Debug, Clone)]
pub struct ParticleRotationProperty {
    pub base: ParticleProperty<f32>,
    pub orient_random: f32,
    orientation_rng_state: u64,
}

impl ParticleRotationProperty {
    pub fn with_start(start: f32) -> Self {
        Self {
            base: ParticleProperty::with_start(start),
            orient_random: 0.0,
            orientation_rng_state: default_seed::<(f32, u8)>(),
        }
    }

    pub fn with_start_and_orientation(start: f32, orient_random: f32) -> Self {
        let mut prop = Self::with_start(start);
        prop.orient_random = orient_random;
        prop
    }

    pub fn with_keyframes(
        start: f32,
        rand: f32,
        orient_random: f32,
        key_times: Vec<f32>,
        values: Vec<f32>,
    ) -> Self {
        Self {
            base: ParticleProperty::with_seed(
                start,
                rand,
                key_times.len() as u32,
                Some(key_times),
                Some(values),
                default_seed::<f32>(),
            ),
            orient_random,
            orientation_rng_state: default_seed::<(f32, u8)>(),
        }
    }

    pub fn set_orient_random(&mut self, orient_random: f32) {
        self.orient_random = orient_random;
        self.orientation_rng_state = default_seed::<(f32, u8)>();
    }

    pub(crate) fn random_offsets(&mut self) -> (f32, f32) {
        let rotation = self.base.random_offset();
        let mut rng = DeterministicRng::new(self.orientation_rng_state);
        let orientation = if self.orient_random.abs() <= f32::EPSILON {
            0.0
        } else {
            rng.next_signed_f32() * self.orient_random
        };
        self.orientation_rng_state = rng.state();
        (rotation, orientation)
    }

    pub fn sample(&self, age: f32, random_offset: &f32) -> f32 {
        self.base.sample(age, random_offset)
    }
}

/// Particle frame property (for texture animation)
pub type ParticleFrameProperty = ParticleProperty<f32>;

/// Particle blur time property
pub type ParticleBlurTimeProperty = ParticleProperty<f32>;
