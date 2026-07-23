//! Host DamDie residual (enable water waveguides when dam dies).
//!
//! C++: `DamDie::onDie` walks all objects; for each KINDOF_WAVEGUIDE clears
//! DISABLED_DEFAULT so flood wave objects start moving.
//!
//! Residual playability slice:
//! - Dam template death enables WAVEGUIDE-kind objects (clear disabled_default)
//! - WaveGuide objects begin as disabled until dam dies
//!
//! Fail-closed: not full WaveGuideUpdate hydrodynamics / terrain flood mesh.

use serde::{Deserialize, Serialize};

/// C++ DISABLED_DEFAULT residual bit (DamDie / WaveGuide startup).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDamDieState {
    /// True once dam onDie residual has fired.
    pub dam_died: bool,
}

/// Template name peel for the map dam object.
pub fn is_dam_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Exact-ish peels: avoid "damage" false positives.
    n == "dam"
        || n.ends_with("dam")
        || n.contains("bigdam")
        || n.contains("waterdam")
        || n == "civiandam"
        || n.contains("civiliandam")
}

/// Template / kind peel for waveguide objects.
pub fn is_wave_guide_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("waveguide") || n.contains("waterwave") || n.contains("floodwave")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dam_name_match() {
        assert!(is_dam_template("Dam"));
        assert!(is_dam_template("CivilianDam"));
        assert!(!is_dam_template("AmericaTankCrusader"));
        assert!(!is_dam_template("DamageRegion"));
    }

    #[test]
    fn waveguide_name_match() {
        assert!(is_wave_guide_template("WaveGuide1"));
        assert!(is_wave_guide_template("WaterWaveA"));
        assert!(!is_wave_guide_template("AmericaJetRaptor"));
    }
}
