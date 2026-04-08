use rand::Rng;
use rand_distr::{Distribution, Normal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionType {
    Constant = 0,
    Uniform = 1,
    Gaussian = 2,
    Triangular = 3,
    LowBias = 4,
    HighBias = 5,
}

pub const DISTRIBUTION_TYPE_NAMES: [&str; 6] = [
    "CONSTANT",
    "UNIFORM",
    "GAUSSIAN",
    "TRIANGULAR",
    "LOW_BIAS",
    "HIGH_BIAS",
];

#[derive(Debug, Clone, Copy)]
pub struct GameClientRandomVariable {
    distribution_type: DistributionType,
    low: f32,
    high: f32,
}

impl Default for GameClientRandomVariable {
    fn default() -> Self {
        Self {
            distribution_type: DistributionType::Uniform,
            low: 0.0,
            high: 0.0,
        }
    }
}

impl GameClientRandomVariable {
    pub fn new(low: f32, high: f32) -> Self {
        let mut value = Self::default();
        value.set_range(low, high, DistributionType::Uniform);
        value
    }

    pub fn set_range(&mut self, low: f32, high: f32, distribution_type: DistributionType) {
        self.low = low;
        self.high = high;
        self.distribution_type = distribution_type;
    }

    pub fn get_value(&self) -> f32 {
        let min = self.low.min(self.high);
        let max = self.low.max(self.high);
        if (max - min).abs() <= f32::EPSILON {
            return min;
        }

        let mut rng = rand::thread_rng();
        match self.distribution_type {
            DistributionType::Constant => min,
            DistributionType::Uniform => rng.gen_range(min..=max),
            DistributionType::Gaussian => {
                let mean = (min + max) * 0.5;
                let std_dev = ((max - min) / 6.0).max(f32::EPSILON);
                Normal::new(mean, std_dev)
                    .map(|normal| normal.sample(&mut rng).clamp(min, max))
                    .unwrap_or(mean)
            }
            DistributionType::Triangular => {
                let a = rng.gen_range(min..=max);
                let b = rng.gen_range(min..=max);
                (a + b) * 0.5
            }
            DistributionType::LowBias => {
                let sample = rng.gen_range(0.0_f32..=1.0_f32);
                min + (max - min) * sample * sample
            }
            DistributionType::HighBias => {
                let sample = rng.gen_range(0.0_f32..=1.0_f32);
                min + (max - min) * (1.0 - (1.0 - sample) * (1.0 - sample))
            }
        }
    }

    pub fn get_minimum_value(&self) -> f32 {
        self.low
    }

    pub fn get_maximum_value(&self) -> f32 {
        self.high
    }

    pub fn get_distribution_type(&self) -> DistributionType {
        self.distribution_type
    }
}

pub fn get_game_client_random_value(lo: i32, hi: i32, _file: &str, _line: u32) -> i32 {
    let min = lo.min(hi);
    let max = lo.max(hi);
    rand::thread_rng().gen_range(min..=max)
}

pub fn get_game_client_random_value_real(lo: f32, hi: f32, _file: &str, _line: u32) -> f32 {
    let min = lo.min(hi);
    let max = lo.max(hi);
    rand::thread_rng().gen_range(min..=max)
}
