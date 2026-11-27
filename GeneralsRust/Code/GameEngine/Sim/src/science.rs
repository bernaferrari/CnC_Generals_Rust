//! Science command names and fallback data for sessions without a matching definition.

/// C++ Player::setScienceAvailability modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScienceAvailabilityType {
    Available,
    Disabled,
    Hidden,
}

impl ScienceAvailabilityType {
    /// Parse the authored names accepted by Player::getScienceAvailabilityTypeFromString.
    pub fn from_script_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("Available") {
            Some(Self::Available)
        } else if name.eq_ignore_ascii_case("Disabled") {
            Some(Self::Disabled)
        } else if name.eq_ignore_ascii_case("Hidden") {
            Some(Self::Hidden)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallbackScience {
    name: &'static str,
    point_cost: i32,
    pub prerequisites: [&'static str; 2],
    pub grantable: bool,
}

/// Science definitions used when the active catalogs have no entry.
const FALLBACK_SCIENCES: &[FallbackScience] = &[
    FallbackScience {
        name: "SCIENCE_AMERICA",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_CHINA",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_GLA",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Rank1",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_PaladinTank",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_DaisyCutter",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_MOAB",
        point_cost: 0, // not purchasable (upgrade path residual)
        prerequisites: ["SCIENCE_AMERICA", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ClusterMines",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ArtilleryBarrage1",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_CashBounty1",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank3"],
        grantable: true,
    },
    // Retail Science.ini CashBounty chain: 1 → 2 → 3 (each purchase cost 1,
    // level N requires level N-1 + faction + Rank3). CashBounty3 below already
    // requires CashBounty2, so the middle link must be purchasable too.
    FallbackScience {
        name: "SCIENCE_CashBounty2",
        point_cost: 1,
        prerequisites: ["SCIENCE_CashBounty1", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_CashBounty3",
        point_cost: 1,
        prerequisites: ["SCIENCE_CashBounty2", "SCIENCE_Rank3"],
        grantable: true,
    },
    // Rank.ini SciencesGranted — cost 0 (not purchasable).
    FallbackScience {
        name: "SCIENCE_Rank2",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Rank3",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Rank4",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Rank5",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: true,
    },
    // America Rank1 purchasables (Science.ini + PlayerTemplate command set).
    FallbackScience {
        name: "SCIENCE_StealthFighter",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Pathfinder",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_SpyDrone",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank1"],
        grantable: true,
    },
    // America Rank3 purchasables.
    FallbackScience {
        name: "SCIENCE_A10ThunderboltMissileStrike1",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_A10ThunderboltMissileStrike2",
        point_cost: 1,
        prerequisites: ["SCIENCE_A10ThunderboltMissileStrike1", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_A10ThunderboltMissileStrike3",
        point_cost: 1,
        prerequisites: ["SCIENCE_A10ThunderboltMissileStrike2", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_EmergencyRepair1",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_Paradrop1",
        point_cost: 1,
        prerequisites: ["SCIENCE_AMERICA", "SCIENCE_Rank3"],
        grantable: true,
    },
    // China Rank1 / Rank3 purchasables.
    FallbackScience {
        name: "SCIENCE_RedGuardTraining",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_BattlemasterTraining",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ArtilleryTraining",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_NukeCannon",
        point_cost: 1,
        prerequisites: ["SCIENCE_CHINA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ArtilleryBarrage2",
        point_cost: 1,
        prerequisites: ["SCIENCE_ArtilleryBarrage1", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ArtilleryBarrage3",
        point_cost: 1,
        prerequisites: ["SCIENCE_ArtilleryBarrage2", "SCIENCE_Rank3"],
        grantable: true,
    },
    // GLA Rank1 / Rank3 purchasables.
    FallbackScience {
        name: "SCIENCE_RebelAmbush1",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_MarauderTank",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_ScudLauncher",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank1"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_AnthraxBomb",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank3"],
        grantable: true,
    },
    FallbackScience {
        name: "SCIENCE_SneakAttack",
        point_cost: 1,
        prerequisites: ["SCIENCE_GLA", "SCIENCE_Rank3"],
        grantable: true,
    },
    // C++ ScienceInfo::m_grantable = No — PLAYER_GRANT_SCIENCE must refuse.
    FallbackScience {
        name: "SCIENCE_NonGrantableTest",
        point_cost: 0,
        prerequisites: ["", ""],
        grantable: false,
    },
];

/// Resolve supported command aliases to their science definition names.
pub fn normalize_science_name(science_name: &str) -> String {
    let raw = science_name.trim();
    if raw.is_empty() {
        return String::new();
    }
    // Already a SCIENCE_ / faction-prefix science token.
    let lower = raw.to_ascii_lowercase().replace('-', "_");
    if lower.starts_with("science_")
        || lower.starts_with("airf_science_")
        || lower.starts_with("nuke_science_")
        || lower.starts_with("early_science_")
        || lower.starts_with("supw_science_")
        || lower.starts_with("chem_science_")
        || lower.starts_with("slth_science_")
        || lower.starts_with("infa_science_")
        || lower.starts_with("tank_science_")
    {
        // Preserve original casing style for SCIENCE_ tokens.
        if raw.starts_with("SCIENCE_") || raw.contains("_SCIENCE_") {
            return raw.to_string();
        }
        // Rebuild SCIENCE_ prefix from lower form when possible.
        if lower.starts_with("science_") {
            return format!(
                "SCIENCE_{}",
                &raw[raw.find('_').map(|i| i + 1).unwrap_or(0)..]
            );
        }
        return raw.to_string();
    }
    // Compact aliases accepted by science commands.
    match lower.as_str() {
        "a10strike1" | "a10_strike_1" | "a10thunderboltmissilestrike1" => {
            "SCIENCE_A10ThunderboltMissileStrike1".into()
        }
        "a10strike2" | "a10_strike_2" => "SCIENCE_A10ThunderboltMissileStrike2".into(),
        "a10strike3" | "a10_strike_3" => "SCIENCE_A10ThunderboltMissileStrike3".into(),
        "daisycutter" | "daisy_cutter" => "SCIENCE_DaisyCutter".into(),
        "spydrone" | "spy_drone" => "SCIENCE_SpyDrone".into(),
        "cashbounty1" | "cash_bounty_1" | "cashbounty" => "SCIENCE_CashBounty1".into(),
        "cashbounty2" | "cash_bounty_2" => "SCIENCE_CashBounty2".into(),
        "cashbounty3" | "cash_bounty_3" => "SCIENCE_CashBounty3".into(),
        "paladintank" | "paladin_tank" => "SCIENCE_PaladinTank".into(),
        "clustermines" | "cluster_mines" => "SCIENCE_ClusterMines".into(),
        "emergencyrepair1" | "emergency_repair_1" | "emergencyrepair" => {
            "SCIENCE_EmergencyRepair1".into()
        }
        "paradrop1" | "paradrop" => "SCIENCE_Paradrop1".into(),
        "spectregunship" | "spectre_gunship" => "SCIENCE_SpectreGunshipSolo".into(),
        "fuelairbomb" | "fuel_air_bomb" | "moab" => "SCIENCE_DaisyCutter".into(), // MOAB upgrade path
        other => {
            // Convert a bare token to a SCIENCE_ name.
            let mut out = String::from("SCIENCE_");
            let mut cap = true;
            for ch in other.chars() {
                if ch == '_' {
                    out.push('_');
                    cap = true;
                } else if cap {
                    out.extend(ch.to_uppercase());
                    cap = false;
                } else {
                    out.push(ch);
                }
            }
            out
        }
    }
}

fn is_science_token_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("science_")
        || n.starts_with("airf_science_")
        || n.starts_with("nuke_science_")
        || n.starts_with("early_science_")
        || n.starts_with("supw_science_")
        || n.starts_with("chem_science_")
        || n.starts_with("slth_science_")
        || n.starts_with("infa_science_")
        || n.starts_with("tank_science_")
}

pub fn fallback_science(name: &str) -> Option<&'static FallbackScience> {
    FALLBACK_SCIENCES
        .iter()
        .find(|definition| definition.name == name)
}

/// Match the science store's zero cost for an unknown token.
pub fn fallback_purchase_point_cost(name: &str) -> Option<i32> {
    fallback_science(name)
        .map(|definition| definition.point_cost)
        .or_else(|| is_science_token_name(name).then_some(0))
}
