//! Host SubObjectsUpgrade residual (show/hide drawable sub-object peels).
//!
//! C++: `SubObjectsUpgrade::upgradeImplementation` → Drawable show/hide subobjects
//! when TriggeredBy upgrades complete (with ConflictsWith / RequiresAllTriggers).
//!
//! Residual playability slice:
//! - GLA BombTruck Bio / HE / Bio+HE → Bombload02/03/04
//! - China Helix NapalmBomb → BombWing
//! - Tracks shown/hidden names on object for presentation honesty
//!
//! Fail-closed: not full W3D mesh toggle GPU path / multi-object SharedNSync matrix.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Active sub-object visibility residual for one object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSubObjectVisibility {
    pub shown: BTreeSet<String>,
    pub hidden: BTreeSet<String>,
}

impl HostSubObjectVisibility {
    pub fn apply_show_hide(&mut self, show: &[&str], hide: &[&str]) {
        for s in show {
            self.shown.insert((*s).to_string());
            self.hidden.remove(*s);
        }
        for h in hide {
            self.hidden.insert((*h).to_string());
            self.shown.remove(*h);
        }
    }

    pub fn is_shown(&self, name: &str) -> bool {
        self.shown.contains(name)
    }
}

/// Result of applying SubObjectsUpgrade for an upgrade on a template.
#[derive(Debug, Clone, Default)]
pub struct SubObjectsUpgradeApply {
    pub show: Vec<&'static str>,
    pub hide: Vec<&'static str>,
    pub matched: bool,
}

/// C++ SubObjectsUpgrade peels keyed by upgrade + unit family.
pub fn sub_objects_for_upgrade(upgrade: &str, template_name: &str) -> SubObjectsUpgradeApply {
    let u = upgrade.to_ascii_lowercase();
    let t = template_name.to_ascii_lowercase();
    let mut out = SubObjectsUpgradeApply::default();

    // Helix Napalm BombWing residual.
    if u.contains("helixnapalm") || u.contains("helix_napalm") {
        if t.contains("helix") {
            out.show.push("BombWing");
            out.matched = true;
            return out;
        }
    }

    // Bomb truck load peels.
    let is_bomb_truck = t.contains("bombtruck") || t.contains("bomb_truck");
    if !is_bomb_truck {
        return out;
    }

    let bio = u.contains("biobomb") || u.contains("bio_bomb") || u.contains("anthrax");
    // High explosive naming peels.
    let he = u.contains("highexplosive")
        || u.contains("high_explosive")
        || u.contains("hebomb")
        || (u.contains("explosive") && u.contains("bomb") && !bio);

    // When a single upgrade is applied we don't know the other flag yet — callers
    // pass combined upgrade tags via `sub_objects_for_upgrade_tags`.
    if bio && he {
        out.show.push("Bombload04");
        out.hide.extend(["Bombload01", "Bombload02", "Bombload03"]);
        out.matched = true;
    } else if he {
        out.show.push("Bombload03");
        out.hide.extend(["Bombload01", "Bombload02", "Bombload04"]);
        out.matched = true;
    } else if bio {
        out.show.push("Bombload02");
        out.hide.extend(["Bombload01", "Bombload03", "Bombload04"]);
        out.matched = true;
    }

    out
}

/// Resolve bomb-truck subobjects from the full set of applied upgrade tags.
pub fn sub_objects_for_upgrade_tags(
    tags: &std::collections::HashSet<String>,
    template_name: &str,
) -> SubObjectsUpgradeApply {
    let t = template_name.to_ascii_lowercase();
    let mut out = SubObjectsUpgradeApply::default();

    // Helix
    if t.contains("helix") {
        let has_napalm = tags.iter().any(|x| {
            let n = x.to_ascii_lowercase();
            n.contains("helixnapalm") || n.contains("helix_napalm")
        });
        if has_napalm {
            out.show.push("BombWing");
            out.matched = true;
        }
        return out;
    }

    if !(t.contains("bombtruck") || t.contains("bomb_truck")) {
        return out;
    }

    let bio = tags.iter().any(|x| {
        let n = x.to_ascii_lowercase();
        n.contains("biobomb") || n.contains("bio_bomb")
    });
    let he = tags.iter().any(|x| {
        let n = x.to_ascii_lowercase();
        n.contains("highexplosive")
            || n.contains("high_explosive")
            || (n.contains("explosive") && n.contains("bomb") && !n.contains("bio"))
    });

    if bio && he {
        out.show.push("Bombload04");
        out.hide.extend(["Bombload01", "Bombload02", "Bombload03"]);
        out.matched = true;
    } else if he {
        out.show.push("Bombload03");
        out.hide.extend(["Bombload01", "Bombload02", "Bombload04"]);
        out.matched = true;
    } else if bio {
        out.show.push("Bombload02");
        out.hide.extend(["Bombload01", "Bombload03", "Bombload04"]);
        out.matched = true;
    }
    out
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSubObjectsUpgradeLog {
    pub applications: u32,
    pub last_show: String,
}

impl HostSubObjectsUpgradeLog {
    pub fn record(&mut self, show: &[&str]) {
        self.applications = self.applications.saturating_add(1);
        self.last_show = show.join(",");
    }
    pub fn honesty_ok(&self) -> bool {
        self.applications > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn bomb_truck_bio_shows_load02() {
        let a = sub_objects_for_upgrade("Upgrade_GLABombTruckBioBomb", "GLAVehicleBombTruck");
        assert!(a.matched);
        assert!(a.show.contains(&"Bombload02"));
        assert!(a.hide.contains(&"Bombload01"));
    }

    #[test]
    fn bomb_truck_both_shows_load04() {
        let mut tags = HashSet::new();
        tags.insert("Upgrade_GLABombTruckBioBomb".into());
        tags.insert("Upgrade_GLABombTruckHighExplosiveBomb".into());
        let a = sub_objects_for_upgrade_tags(&tags, "GLAVehicleBombTruck");
        assert_eq!(a.show, vec!["Bombload04"]);
    }

    #[test]
    fn helix_napalm_shows_bomb_wing() {
        let a = sub_objects_for_upgrade("Upgrade_HelixNapalmBomb", "ChinaVehicleHelix");
        assert_eq!(a.show, vec!["BombWing"]);
    }
}
