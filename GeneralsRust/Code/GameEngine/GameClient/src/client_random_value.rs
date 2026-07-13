//! GameClient random value residual.
//!
//! Re-exports / wraps `game_engine::common::random_value` client stream so
//! GameClient consumers share the retail ADC RandomValue algorithm instead of
//! `rand::thread_rng` (non-deterministic, wrong stream).
//!
//! Fail-closed: Gaussian/Triangular/LowBias/HighBias distribution types panic
//! like C++ RandomValue.cpp unsupported paths.

use game_engine::common::random_value::{
    get_game_client_random_value as common_client_int,
    get_game_client_random_value_real as common_client_real, DistributionType as CommonDistribution,
    GameClientRandomVariable as CommonClientVar,
};

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

impl DistributionType {
    fn to_common(self) -> CommonDistribution {
        match self {
            Self::Constant => CommonDistribution::Constant,
            Self::Uniform => CommonDistribution::Uniform,
            Self::Gaussian => CommonDistribution::Gaussian,
            Self::Triangular => CommonDistribution::Triangular,
            Self::LowBias => CommonDistribution::LowBias,
            Self::HighBias => CommonDistribution::HighBias,
        }
    }
}

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
        let mut var = CommonClientVar::new();
        var.set_range(self.low, self.high, self.distribution_type.to_common());
        var.get_value()
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

/// C++ `GetGameClientRandomValue` — integer client stream residual.
pub fn get_game_client_random_value(lo: i32, hi: i32, _file: &str, _line: u32) -> i32 {
    common_client_int(lo, hi)
}

/// C++ `GetGameClientRandomValueReal` — real client stream residual.
pub fn get_game_client_random_value_real(lo: f32, hi: f32, _file: &str, _line: u32) -> f32 {
    common_client_real(lo, hi)
}

/// Convenience macros matching C++ `GameClientRandomValue` / `GameClientRandomValueReal`.
#[macro_export]
macro_rules! GameClientRandomValue {
    ($lo:expr, $hi:expr) => {
        $crate::client_random_value::get_game_client_random_value($lo, $hi, file!(), line!())
    };
}

#[macro_export]
macro_rules! GameClientRandomValueReal {
    ($lo:expr, $hi:expr) => {
        $crate::client_random_value::get_game_client_random_value_real($lo, $hi, file!(), line!())
    };
}
