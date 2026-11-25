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

/// Persistent overrides in W3DModelDraw's first-insertion order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSubObjectVisibility {
    overrides: Vec<SubObjectOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SubObjectOverride {
    name: String,
    hidden: bool,
}

impl HostSubObjectVisibility {
    /// W3DModelDraw::showSubObject matches case-insensitively and updates the
    /// existing slot. A parent override can affect later child directives.
    pub fn set_shown(&mut self, name: &str, shown: bool) {
        if name.is_empty() {
            return;
        }
        if let Some(existing) = self
            .overrides
            .iter_mut()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            existing.hidden = !shown;
        } else {
            self.overrides.push(SubObjectOverride {
                name: name.to_owned(),
                hidden: !shown,
            });
        }
    }

    pub fn apply_show_hide(&mut self, show: &[&str], hide: &[&str]) {
        for name in show {
            self.set_shown(name, true);
        }
        for name in hide {
            self.set_shown(name, false);
        }
    }

    pub fn is_shown(&self, name: &str) -> bool {
        self.overrides
            .iter()
            .any(|entry| !entry.hidden && entry.name.eq_ignore_ascii_case(name))
    }

    pub fn is_hidden(&self, name: &str) -> bool {
        self.overrides
            .iter()
            .any(|entry| entry.hidden && entry.name.eq_ignore_ascii_case(name))
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Visit drawable directives in the order they must be applied.
    pub fn directives(&self) -> impl Iterator<Item = (&str, bool)> {
        self.overrides
            .iter()
            .map(|entry| (entry.name.as_str(), entry.hidden))
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
pub fn sub_objects_for_upgrade_tags<'a>(
    tags: impl IntoIterator<Item = &'a String>,
    template_name: &str,
) -> SubObjectsUpgradeApply {
    let mut out = SubObjectsUpgradeApply::default();
    let mut tags = tags.into_iter().peekable();
    if tags.peek().is_none() {
        return out;
    }
    let t = template_name.to_ascii_lowercase();

    // Helix
    if t.contains("helix") {
        let has_napalm = tags.any(|x| {
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

    let mut bio = false;
    let mut he = false;
    for x in tags {
        let n = x.to_ascii_lowercase();
        bio |= n.contains("biobomb") || n.contains("bio_bomb");
        he |= n.contains("highexplosive")
            || n.contains("high_explosive")
            || (n.contains("explosive") && n.contains("bomb") && !n.contains("bio"));
        if bio && he {
            break;
        }
    }

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
    fn sub_object_overrides_keep_first_insertion_order_when_case_aliases_update() {
        let mut visibility = HostSubObjectVisibility::default();
        visibility.set_shown("ZParent", false);
        visibility.set_shown("AChild", false);
        visibility.set_shown("zparent", true);
        visibility.set_shown("", true);
        assert_eq!(
            visibility.directives().collect::<Vec<_>>(),
            [("ZParent", false), ("AChild", true)]
        );
        assert!(visibility.is_shown("ZPARENT"));
    }

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
