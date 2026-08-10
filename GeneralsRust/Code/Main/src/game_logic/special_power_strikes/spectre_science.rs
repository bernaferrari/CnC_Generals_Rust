//! SpectreGunship science-tier OrbitTime residual.
use super::types::*;
use super::spectre::*;
// --- Spectre science-tier OrbitTime residual ---

/// Residual Spectre Gunship science tier (OrbitTime 10s / 15s / 20s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SpectreGunshipScienceTier {
    /// Airforce LEVEL1 OrbitTime = 10000 ms → 300 frames.
    Level1,
    #[default]
    /// Default / LEVEL2 OrbitTime = 15000 ms → 450 frames.
    Level2,
    /// Airforce LEVEL3 OrbitTime = 20000 ms → 600 frames.
    Level3,
}

impl SpectreGunshipScienceTier {
    /// Retail OrbitTime residual in logic frames for this science tier.
    pub fn orbit_duration_frames(self) -> u32 {
        match self {
            SpectreGunshipScienceTier::Level1 => 300,
            SpectreGunshipScienceTier::Level2 => SPECTRE_ORBIT_DURATION_FRAMES,
            SpectreGunshipScienceTier::Level3 => 600,
        }
    }

    /// Retail AttackAreaRadius residual for this science tier.
    ///
    /// AirF LEVEL1/2/3 and default USA Spectre all use AttackAreaRadius **200**
    /// (OrbitTime is the only science-tier orbit residual that changes).
    pub fn attack_area_radius(self) -> f32 {
        let _ = self;
        SPECTRE_ORBIT_RADIUS
    }

    /// Map SCIENCE_SpectreGunship1/2/3 (or AirF / Solo residual) to tier.
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if n.contains("spectregunship3") {
            Some(SpectreGunshipScienceTier::Level3)
        } else if n.contains("spectregunship2") {
            Some(SpectreGunshipScienceTier::Level2)
        } else if n.contains("spectregunship1")
            || n.contains("spectregunshipsolo")
            || n.contains("spectregunship")
        {
            Some(SpectreGunshipScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked Spectre science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = SpectreGunshipScienceTier::Level2; // retail default OrbitTime 15s
        let mut found = false;
        for s in sciences {
            if let Some(t) = Self::from_science_name(s) {
                if !found {
                    best = t;
                    found = true;
                } else {
                    best = match (best, t) {
                        (_, SpectreGunshipScienceTier::Level3)
                        | (SpectreGunshipScienceTier::Level3, _) => {
                            SpectreGunshipScienceTier::Level3
                        }
                        (_, SpectreGunshipScienceTier::Level2)
                        | (SpectreGunshipScienceTier::Level2, _) => {
                            SpectreGunshipScienceTier::Level2
                        }
                        _ => SpectreGunshipScienceTier::Level1,
                    };
                }
            }
        }
        best
    }
}
