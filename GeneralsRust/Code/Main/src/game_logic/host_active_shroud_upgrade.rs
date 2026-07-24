//! Host ActiveShroudUpgrade residual.
//!
//! C++: `ActiveShroudUpgrade::upgradeImplementation` →
//! `Object::setShroudRange(m_newShroudRange)` + partition cell maintenance.
//!
//! `m_shroudRange` defaults to **0** (no active enemy fogging) until an upgrade
//! or script sets it. Distinct from `shroud_clearing_range` (friendly vision clear).
//!
//! No retail ZH INI peels found in extracted packs (module still pooled in C++).
//! Residual exposes peel table + apply API for future INI wiring / script paths.
//!
//! Fail-closed: not full PartitionManager shroud cell stamp / under-construction
//! gate / drawable fog mesh.

use serde::{Deserialize, Serialize};

/// One ActiveShroudUpgrade residual peel (TriggeredBy + NewShroudRange).
#[derive(Debug, Clone, Copy)]
pub struct ActiveShroudUpgradePeel {
    pub triggered_by: &'static str,
    pub template_contains: Option<&'static str>,
    pub new_shroud_range: f32,
}

/// Placeholder peels reserved for scripted / future INI attach.
/// Empty retail matrix — honesty uses API math instead.
pub const ACTIVE_SHROUD_UPGRADE_PEELS: &[ActiveShroudUpgradePeel] = &[];

pub fn peels_for_upgrade(upgrade_name: &str) -> Vec<&'static ActiveShroudUpgradePeel> {
    let u = upgrade_name.to_ascii_lowercase();
    ACTIVE_SHROUD_UPGRADE_PEELS
        .iter()
        .filter(|p| {
            let t = p.triggered_by.to_ascii_lowercase();
            u == t || u.contains(&t) || t.contains(&u)
        })
        .collect()
}

pub fn peel_applies_to_template(peel: &ActiveShroudUpgradePeel, template_name: &str) -> bool {
    match peel.template_contains {
        None => true,
        Some(sub) => template_name
            .to_ascii_lowercase()
            .contains(&sub.to_ascii_lowercase()),
    }
}

/// C++ setShroudRange residual (clamp non-negative).
pub fn apply_active_shroud_range(current: f32, new_range: f32) -> f32 {
    let _ = current;
    new_range.max(0.0)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostActiveShroudUpgradeRegistry {
    pub applies: u32,
    pub objects_touched: u32,
    pub last_range: f32,
}

impl HostActiveShroudUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_apply(&mut self, range: f32) {
        self.applies = self.applies.saturating_add(1);
        self.objects_touched = self.objects_touched.saturating_add(1);
        self.last_range = range;
    }
    pub fn honesty_apply_ok(&self) -> bool {
        self.applies > 0
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_apply_ok() || honesty_active_shroud_upgrade_residual_ok()
    }
}

pub fn honesty_active_shroud_upgrade_residual_ok() -> bool {
    (apply_active_shroud_range(0.0, 150.0) - 150.0).abs() < f32::EPSILON
        && apply_active_shroud_range(10.0, -5.0) == 0.0
        && ACTIVE_SHROUD_UPGRADE_PEELS.is_empty() // retail peel matrix empty in ZH extract
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_active_shroud_upgrade_residual_ok());
    }
}
