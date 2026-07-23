//! Host ReplaceObjectUpgrade / GrantScienceUpgrade / CommandSetUpgrade residuals.
//!
//! C++:
//! - `ReplaceObjectUpgrade::upgradeImplementation` destroys the object and spawns
//!   `ReplaceObject` at the same transform/team (GLA Fake* → real buildings).
//! - `GrantScienceUpgrade::upgradeImplementation` → `Player::grantScience`.
//! - `CommandSetUpgrade::upgradeImplementation` → `setCommandSetStringOverride`.
//!
//! Residual playability slice:
//! - FakeGLA* / Chem_/Demo_/Slth_FakeGLA* + Upgrade_BecomeRealGLA* → drop "Fake"
//! - Upgrade_AmericaMOAB → SCIENCE_MOAB grant
//! - China EMP mines / common CommandSet peels set command_set_override
//!
//! Fail-closed: not full onBuildComplete create-module cascade / ControlBar UI dirty /
//! CommandSetUpgrade alt-trigger dual CommandSet matrix / pathfinder cell marks.

use serde::{Deserialize, Serialize};

/// Retail BecomeReal upgrade name prefix residual.
pub const BECOME_REAL_PREFIX: &str = "Upgrade_BecomeReal";

/// Whether this upgrade is a ReplaceObjectUpgrade BecomeReal peel.
pub fn is_replace_object_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    n.contains("becomereal")
}

/// C++ ReplaceObject residual: FakeGLA* → GLA* (preserve Chem_/Demo_/Slth_ prefix).
///
/// `Slth_FakeGLACommandCenter` → `Slth_GLACommandCenter`
/// `FakeGLABarracks` → `GLABarracks`
pub fn replacement_template_for_fake(template_name: &str) -> Option<String> {
    let t = template_name;
    // Case-preserving "Fake" removal (first occurrence only).
    if let Some(idx) = t.find("Fake") {
        let mut out = String::with_capacity(t.len() - 4);
        out.push_str(&t[..idx]);
        out.push_str(&t[idx + 4..]);
        if out.is_empty() || out == t {
            return None;
        }
        return Some(out);
    }
    if let Some(idx) = t.to_ascii_lowercase().find("fake") {
        // odd casing
        let mut out = String::with_capacity(t.len() - 4);
        out.push_str(&t[..idx]);
        out.push_str(&t[idx + 4..]);
        return Some(out);
    }
    None
}

/// Whether template is a GLA Fake building residual host.
pub fn is_fake_gla_building(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("fake") && n.contains("gla")
}

/// GrantScienceUpgrade peels: upgrade → science name.
pub fn grant_science_for_upgrade(upgrade: &str) -> Option<&'static str> {
    let n = upgrade.to_ascii_lowercase();
    if n.contains("moab") || n == "upgrade_americamoab" {
        return Some("SCIENCE_MOAB");
    }
    None
}

/// CommandSetUpgrade residual peels: (upgrade needle, template needle) → command set.
/// Prefer more specific peels first.
pub fn command_set_override_for_upgrade(
    upgrade: &str,
    template_name: &str,
) -> Option<&'static str> {
    let u = upgrade.to_ascii_lowercase();
    let t = template_name.to_ascii_lowercase();

    // Demo suicide CommandSet already handled in host_demo_suicide_bomb — skip here
    // if that path owns the string.
    if u.contains("suicidebomb") {
        return None;
    }

    // GLA Worker real command set residual.
    if u.contains("workerrealcommandset") || u.contains("worker_real") {
        if t.contains("worker") {
            return Some("GLAWorkerCommandSet");
        }
    }

    // China EMP mines → *CommandSetUpgrade on structures.
    if u.contains("empmines") || u.contains("chinaempmines") {
        if t.contains("commandcenter") {
            return Some("ChinaCommandCenterCommandSetUpgrade");
        }
        if t.contains("airfield") {
            return Some("ChinaAirfieldCommandSetUpgrade");
        }
        if t.contains("nuclearmissile") || t.contains("nuke") && t.contains("launcher") {
            return Some("ChinaNuclearMissileCommandSetUpgrade");
        }
        if t.contains("speakertower") || t.contains("propagandatower") {
            return Some("ChinaSpeakerTowerCommandSetUpgrade");
        }
        if t.contains("powerplant") {
            return Some("ChinaPowerPlantCommandSetUpgrade");
        }
        if t.contains("supplycenter") {
            return Some("ChinaSupplyCenterCommandSetUpgrade");
        }
        if t.contains("barracks") {
            return Some("ChinaBarracksCommandSetUpgrade");
        }
        if t.contains("warfactory") {
            return Some("ChinaWarFactoryCommandSetUpgrade");
        }
        if t.contains("bunker") && !t.contains("overlord") && !t.contains("helix") {
            return Some("ChinaBunkerCommandSetUpgrade");
        }
        if t.contains("propagandacenter") {
            return Some("ChinaPropagandaCenterCommandSetUpgrade");
        }
        if t.contains("gattlingcannon") || t.contains("gatlingcannon") {
            return Some("ChinaGattlingCannonCommandSetUpgrade");
        }
    }

    // China mines → base CommandSet with mines button (pre-EMP). Fail-open name peel:
    // many buildings swap to *CommandSetUpgrade only for EMP; mines often enable via
    // same CommandSetUpgrade module TriggeredBy Upgrade_ChinaMines with different set.
    if u.contains("chinamines") && !u.contains("emp") {
        // Leave command set; mines residual may be generate-minefield path.
        return None;
    }

    // Overlord / Helix addon command sets.
    if u.contains("overlordgattling") {
        return Some("ChinaTankOverlordGattlingCannonCommandSet");
    }
    if u.contains("overlordpropaganda") {
        return Some("ChinaTankOverlordPropagandaTowerCommandSet");
    }
    if u.contains("overlordbunker") || u.contains("overlordbattlebunker") {
        return Some("ChinaTankOverlordBattleBunkerCommandSet");
    }
    if u.contains("helixgattling") {
        return Some("ChinaHelixGattlingCannonCommandSet");
    }
    if u.contains("helixpropaganda") {
        return Some("ChinaHelixPropagandaTowerCommandSet");
    }
    if u.contains("helixbunker") || u.contains("helixbattlebunker") {
        return Some("ChinaHelixBattleBunkerCommandSet");
    }

    // Internet center satellite hack command set residual.
    if u.contains("satellitehack") {
        if t.contains("internet") {
            return Some("ChinaInternetCenterCommandSetTwo");
        }
    }

    None
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostReplaceGrantCommandUpgradeLog {
    pub replace_count: u32,
    pub grant_science_count: u32,
    pub command_set_count: u32,
    pub last_replacement: String,
    pub last_science: String,
    pub last_command_set: String,
}

impl HostReplaceGrantCommandUpgradeLog {
    pub fn record_replace(&mut self, from: &str, to: &str) {
        self.replace_count = self.replace_count.saturating_add(1);
        self.last_replacement = format!("{from}->{to}");
    }
    pub fn record_science(&mut self, science: &str) {
        self.grant_science_count = self.grant_science_count.saturating_add(1);
        self.last_science = science.to_string();
    }
    pub fn record_command_set(&mut self, cs: &str) {
        self.command_set_count = self.command_set_count.saturating_add(1);
        self.last_command_set = cs.to_string();
    }
    pub fn honesty_ok(&self) -> bool {
        self.replace_count
            .saturating_add(self.grant_science_count)
            .saturating_add(self.command_set_count)
            > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_gla_replacement_strips_fake() {
        assert_eq!(
            replacement_template_for_fake("FakeGLACommandCenter").as_deref(),
            Some("GLACommandCenter")
        );
        assert_eq!(
            replacement_template_for_fake("Chem_FakeGLABarracks").as_deref(),
            Some("Chem_GLABarracks")
        );
        assert_eq!(
            replacement_template_for_fake("Slth_FakeGLAArmsDealer").as_deref(),
            Some("Slth_GLAArmsDealer")
        );
    }

    #[test]
    fn become_real_detected() {
        assert!(is_replace_object_upgrade(
            "Upgrade_BecomeRealGLACommandCenter"
        ));
        assert!(!is_replace_object_upgrade("Upgrade_ChinaEMPMines"));
    }

    #[test]
    fn moab_grants_science() {
        assert_eq!(
            grant_science_for_upgrade("Upgrade_AmericaMOAB"),
            Some("SCIENCE_MOAB")
        );
    }

    #[test]
    fn emp_mines_command_set() {
        assert_eq!(
            command_set_override_for_upgrade("Upgrade_ChinaEMPMines", "ChinaCommandCenter"),
            Some("ChinaCommandCenterCommandSetUpgrade")
        );
    }
}
