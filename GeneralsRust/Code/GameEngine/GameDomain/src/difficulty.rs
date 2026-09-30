/// AI difficulty value carried by game metadata and frozen presentation frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AIDifficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl AIDifficulty {
    /// Get build delay modifier for this difficulty.
    pub fn get_build_delay_modifier(&self) -> f32 {
        match self {
            Self::Easy => 2.0,
            Self::Medium => 1.0,
            Self::Hard => 0.7,
            Self::Brutal => 0.5,
        }
    }

    /// Get resource bonus for this difficulty.
    pub fn get_resource_bonus(&self) -> f32 {
        match self {
            Self::Easy => 0.8,
            Self::Medium => 1.0,
            Self::Hard => 1.2,
            Self::Brutal => 1.5,
        }
    }

    /// Get aggressive behavior factor.
    pub fn get_aggression_factor(&self) -> f32 {
        match self {
            Self::Easy => 0.6,
            Self::Medium => 1.0,
            Self::Hard => 1.4,
            Self::Brutal => 1.8,
        }
    }
}
