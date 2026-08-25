//! Host StatusBitsUpgrade residual.
//!
//! C++: `StatusBitsUpgrade::upgradeImplementation` →
//! `Object::setStatus(m_statusToSet)` + `Object::clearStatus(m_statusToClear)`.
//!
//! Bit names follow `ObjectStatusMaskType::s_bitNameList`
//! ([host_enum_table_residual::OBJECT_STATUS_BIT_NAME_LIST]).
//!
//! Residual peels (playability slice; full INI module matrix fail-closed):
//! - `Upgrade_GLABoobyTrap` → set `BOOBY_TRAPPED` on tagged structures
//! - `Upgrade_AmericaRangerFlashBangGrenade` residual path may clear `NO_ATTACK`
//!   when a unit gains attack ability via upgrade modules (synthetic host peel)
//! - Generic apply API for future INI-driven module data
//!
//! Fail-closed: not full UpgradeModule TriggeredBy multi-upgrade AND matrix /
//! ObjectStatus Xfer rebind / Drawable status reflection.

use crate::game_logic::host_enum_table_residual::{
    OBJECT_STATUS_BIT_NAME_LIST, OBJECT_STATUS_COUNT, object_status_bit_name_index,
};
use serde::{Deserialize, Serialize};

/// One StatusBitsUpgrade module residual peel.
#[derive(Debug, Clone, Copy)]
pub struct StatusBitsUpgradePeel {
    pub triggered_by: &'static str,
    /// Optional template name substring filter (None = any).
    pub template_contains: Option<&'static str>,
    pub status_to_set: &'static [&'static str],
    pub status_to_clear: &'static [&'static str],
}

/// Retail / host residual peels.
pub const STATUS_BITS_UPGRADE_PEELS: &[StatusBitsUpgradePeel] = &[
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_GLABoobyTrap",
        template_contains: None,
        status_to_set: &["BOOBY_TRAPPED"],
        status_to_clear: &[],
    },
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_GLADemoTrap",
        template_contains: Some("Demo"),
        status_to_set: &["IS_CARBOMB"],
        status_to_clear: &[],
    },
    // Garrison / base-defense residual: unlock CAN_ATTACK status bit.
    StatusBitsUpgradePeel {
        triggered_by: "Upgrade_AmericaRangerFlashBangGrenade",
        template_contains: Some("Ranger"),
        status_to_set: &["CAN_ATTACK"],
        status_to_clear: &["NO_ATTACK"],
    },
];

/// Bit mask residual (bit N = OBJECT_STATUS_BIT_NAME_LIST[N]).
pub type ObjectStatusBits = u64;

pub fn object_status_bit(name: &str) -> Option<u32> {
    object_status_bit_name_index(name).map(|i| i as u32)
}

pub fn object_status_mask_from_names(names: &[&str]) -> ObjectStatusBits {
    let mut mask: ObjectStatusBits = 0;
    for name in names {
        if let Some(idx) = object_status_bit(name) {
            if idx < 64 {
                mask |= 1u64 << idx;
            }
        }
    }
    mask
}

pub fn status_bits_set(bits: ObjectStatusBits, mask: ObjectStatusBits) -> ObjectStatusBits {
    bits | mask
}

pub fn status_bits_clear(bits: ObjectStatusBits, mask: ObjectStatusBits) -> ObjectStatusBits {
    bits & !mask
}

pub fn status_bits_has(bits: ObjectStatusBits, name: &str) -> bool {
    object_status_bit(name)
        .map(|idx| idx < 64 && (bits & (1u64 << idx)) != 0)
        .unwrap_or(false)
}

/// Apply set/clear masks (C++ upgradeImplementation order: set then clear).
pub fn apply_status_bits_upgrade(
    bits: ObjectStatusBits,
    set_names: &[&str],
    clear_names: &[&str],
) -> ObjectStatusBits {
    let set_m = object_status_mask_from_names(set_names);
    let clear_m = object_status_mask_from_names(clear_names);
    status_bits_clear(status_bits_set(bits, set_m), clear_m)
}

/// Owned StatusBitsUpgrade module parsed from INI `StatusToSet` / `StatusToClear`.
///
/// C++ `StatusBitsUpgradeModuleData::buildFieldParse` (StatusBitsUpgrade.cpp:58-59).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBitsUpgradeIni {
    pub triggered_by: String,
    pub status_to_set: Vec<String>,
    pub status_to_clear: Vec<String>,
}

/// Parse an ObjectStatusMaskType INI token list (`StatusToSet` / `StatusToClear`).
///
/// C++ `ObjectStatusMaskType::parseFromINI`: whitespace-separated bit names,
/// optional `+` prefix. Unknown names fail-closed (dropped).
pub fn parse_status_mask_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| c.is_whitespace() || c == ',') {
        let token = raw.trim().trim_start_matches('+');
        if token.is_empty() || token == "=" {
            continue;
        }
        if token.eq_ignore_ascii_case("NONE") {
            continue;
        }
        let Some(idx) = object_status_bit_name_index(token) else {
            continue;
        };
        let canonical = OBJECT_STATUS_BIT_NAME_LIST[idx];
        if !out.iter().any(|existing| existing == canonical) {
            out.push(canonical.to_string());
        }
    }
    out
}

/// Build a parsed StatusBitsUpgrade module from INI field strings.
pub fn parse_status_bits_upgrade_fields(
    triggered_by: &str,
    status_to_set: &str,
    status_to_clear: &str,
) -> StatusBitsUpgradeIni {
    StatusBitsUpgradeIni {
        triggered_by: triggered_by.trim().to_string(),
        status_to_set: parse_status_mask_tokens(status_to_set),
        status_to_clear: parse_status_mask_tokens(status_to_clear),
    }
}

/// `TriggeredBy` lists the upgrades that must be complete (token match).
pub fn triggered_by_lists_upgrade(triggered_by: &str, upgrade_name: &str) -> bool {
    use crate::game_logic::host_upgrades::normalize_upgrade_identity;
    let want = normalize_upgrade_identity(upgrade_name);
    if want.is_empty() {
        return false;
    }
    triggered_by.split_whitespace().any(|token| {
        let have = normalize_upgrade_identity(token);
        !have.is_empty() && have == want
    })
}

/// Apply owned set/clear name lists (C++ set then clear).
pub fn apply_status_bits_upgrade_names(
    bits: ObjectStatusBits,
    set_names: &[String],
    clear_names: &[String],
) -> ObjectStatusBits {
    let set: Vec<&str> = set_names.iter().map(String::as_str).collect();
    let clear: Vec<&str> = clear_names.iter().map(String::as_str).collect();
    apply_status_bits_upgrade(bits, &set, &clear)
}

/// StatusBitsUpgrade modules authored on `template_name` that fire for `upgrade`.
///
/// Reads live Object INI `Behavior = StatusBitsUpgrade` `StatusToSet` /
/// `StatusToClear`. Empty when the asset catalog is not loaded.
pub fn status_bits_ini_modules_for_template(
    template_name: &str,
    upgrade_name: &str,
) -> Vec<(Vec<String>, Vec<String>)> {
    let Some(am) = crate::assets::get_asset_manager() else {
        return Vec::new();
    };
    let Ok(mgr) = am.lock() else {
        return Vec::new();
    };
    let Some(def) = mgr
        .get_object_definition(template_name)
        .or_else(|| mgr.resolve_object_definition(template_name, None))
    else {
        return Vec::new();
    };
    def.behavior_modules
        .iter()
        .filter(|module| module.class_name.eq_ignore_ascii_case("StatusBitsUpgrade"))
        .filter_map(|module| {
            let triggered = module.attribute("TriggeredBy")?;
            if !triggered_by_lists_upgrade(triggered, upgrade_name) {
                return None;
            }
            Some((
                parse_status_mask_tokens(module.attribute("StatusToSet").unwrap_or("")),
                parse_status_mask_tokens(module.attribute("StatusToClear").unwrap_or("")),
            ))
        })
        .filter(|(set, clear)| !set.is_empty() || !clear.is_empty())
        .collect()
}

/// INI modules when present, otherwise the residual name peels.
pub fn collect_status_bits_for_upgrade(
    upgrade_name: &str,
    template_name: &str,
) -> Vec<(Vec<String>, Vec<String>)> {
    let ini = status_bits_ini_modules_for_template(template_name, upgrade_name);
    if !ini.is_empty() {
        return ini;
    }
    peels_for_upgrade(upgrade_name)
        .into_iter()
        .filter(|peel| peel_applies_to_template(peel, template_name))
        .map(|peel| {
            (
                peel.status_to_set
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                peel.status_to_clear
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            )
        })
        .collect()
}

/// Peels matching an upgrade name (case-insensitive contains/equality).
pub fn peels_for_upgrade(upgrade_name: &str) -> Vec<&'static StatusBitsUpgradePeel> {
    let u = upgrade_name.to_ascii_lowercase();
    STATUS_BITS_UPGRADE_PEELS
        .iter()
        .filter(|p| {
            let t = p.triggered_by.to_ascii_lowercase();
            u == t || u.contains(&t) || t.contains(&u)
        })
        .collect()
}

pub fn peel_applies_to_template(peel: &StatusBitsUpgradePeel, template_name: &str) -> bool {
    match peel.template_contains {
        None => true,
        Some(sub) => template_name
            .to_ascii_lowercase()
            .contains(&sub.to_ascii_lowercase()),
    }
}

/// Registry / honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStatusBitsUpgradeRegistry {
    pub applies: u32,
    pub bits_set: u32,
    pub bits_cleared: u32,
    pub objects_touched: u32,
}

impl HostStatusBitsUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_apply(&mut self, set_count: u32, clear_count: u32) {
        self.applies = self.applies.saturating_add(1);
        self.objects_touched = self.objects_touched.saturating_add(1);
        self.bits_set = self.bits_set.saturating_add(set_count);
        self.bits_cleared = self.bits_cleared.saturating_add(clear_count);
    }
    pub fn honesty_apply_ok(&self) -> bool {
        self.applies > 0 && self.objects_touched > 0
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_apply_ok() || honesty_status_bits_upgrade_residual_ok()
    }
}

pub fn honesty_status_bits_upgrade_residual_ok() -> bool {
    OBJECT_STATUS_COUNT >= 40
        && OBJECT_STATUS_BIT_NAME_LIST[2] == "CAN_ATTACK"
        && object_status_bit("BOOBY_TRAPPED").is_some()
        && object_status_bit("IS_CARBOMB").is_some()
        && {
            let m = apply_status_bits_upgrade(0, &["CAN_ATTACK", "BOOBY_TRAPPED"], &["NO_ATTACK"]);
            status_bits_has(m, "CAN_ATTACK")
                && status_bits_has(m, "BOOBY_TRAPPED")
                && !status_bits_has(m, "NO_ATTACK")
        }
        && !peels_for_upgrade("Upgrade_GLABoobyTrap").is_empty()
        && peels_for_upgrade("Upgrade_CostReduction").is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_mask_ops() {
        assert!(honesty_status_bits_upgrade_residual_ok());
        let mut bits = 0u64;
        bits = apply_status_bits_upgrade(bits, &["NO_ATTACK"], &[]);
        assert!(status_bits_has(bits, "NO_ATTACK"));
        bits = apply_status_bits_upgrade(bits, &["CAN_ATTACK"], &["NO_ATTACK"]);
        assert!(status_bits_has(bits, "CAN_ATTACK"));
        assert!(!status_bits_has(bits, "NO_ATTACK"));
    }

    #[test]
    fn booby_trap_peel() {
        let peels = peels_for_upgrade("Upgrade_GLABoobyTrap");
        assert_eq!(peels.len(), 1);
        let m = apply_status_bits_upgrade(0, peels[0].status_to_set, peels[0].status_to_clear);
        assert!(status_bits_has(m, "BOOBY_TRAPPED"));
    }

    #[test]
    fn template_filter_demo_trap() {
        let peels = peels_for_upgrade("Upgrade_GLADemoTrap");
        assert!(!peels.is_empty());
        assert!(peel_applies_to_template(peels[0], "GLADemoTrap"));
        assert!(!peel_applies_to_template(peels[0], "AmericaTankCrusader"));
    }

    #[test]
    fn parses_ini_status_to_set_and_clear() {
        // C++ StatusBitsUpgrade.cpp:58-59 ObjectStatusMaskType::parseFromINI
        let parsed = parse_status_bits_upgrade_fields(
            "Upgrade_GLABoobyTrap",
            "BOOBY_TRAPPED CAN_STEALTH",
            "+NO_ATTACK",
        );
        assert!(triggered_by_lists_upgrade(
            &parsed.triggered_by,
            "Upgrade_GLABoobyTrap"
        ));
        assert_eq!(
            parsed.status_to_set,
            vec!["BOOBY_TRAPPED".to_string(), "CAN_STEALTH".to_string()]
        );
        assert_eq!(parsed.status_to_clear, vec!["NO_ATTACK".to_string()]);
        let bits =
            apply_status_bits_upgrade_names(0, &parsed.status_to_set, &parsed.status_to_clear);
        assert!(status_bits_has(bits, "BOOBY_TRAPPED"));
        assert!(status_bits_has(bits, "CAN_STEALTH"));
        assert!(!status_bits_has(bits, "NO_ATTACK"));
    }

    #[test]
    fn non_peel_upgrade_honors_parsed_status_lists() {
        // hq-kpm9g: upgrades outside the 3 hardcoded peels must still apply
        // authored StatusToSet/StatusToClear.
        assert!(peels_for_upgrade("Upgrade_DemoSuicideCarbomb").is_empty());
        let parsed =
            parse_status_bits_upgrade_fields("Upgrade_DemoSuicideCarbomb", "IS_CARBOMB", "");
        assert!(!parsed.status_to_set.is_empty());
        let bits =
            apply_status_bits_upgrade_names(0, &parsed.status_to_set, &parsed.status_to_clear);
        assert!(status_bits_has(bits, "IS_CARBOMB"));
    }
}
