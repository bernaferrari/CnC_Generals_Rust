//! Wave 80: SCIENCE rank residual table completeness (Rank.ini).
//!
//! Retail `Data/INI/Rank.ini` freezes ranks **1–5** only (Zero Hour):
//! - SkillPointsNeeded: **0 / 800 / 1500 / 2500 / 5000**
//! - SciencePurchasePointsGranted: **1 / 1 / 1 / 1 / 3**
//! - SciencesGranted: SCIENCE_Rank1 … SCIENCE_Rank5
//! - RankName keys: INI:RankLevel1 … INI:RankLevel5
//!
//! Fail-closed:
//! - Not full RankInfoStore INI load / multiplayer rank override matrix
//! - Not GeneralsExperience skill-points live grant UI
//! - Shell `playable_claim` stays false; network deferred

use serde::{Deserialize, Serialize};

/// Retail Rank.ini rank count residual (Zero Hour ends at Rank 5).
pub const RETAIL_RANK_COUNT: u32 = 5;

/// Retail Rank 1 SkillPointsNeeded residual.
pub const RANK1_SKILL_POINTS_NEEDED: i32 = 0;
/// Retail Rank 2 SkillPointsNeeded residual.
pub const RANK2_SKILL_POINTS_NEEDED: i32 = 800;
/// Retail Rank 3 SkillPointsNeeded residual.
pub const RANK3_SKILL_POINTS_NEEDED: i32 = 1500;
/// Retail Rank 4 SkillPointsNeeded residual.
pub const RANK4_SKILL_POINTS_NEEDED: i32 = 2500;
/// Retail Rank 5 SkillPointsNeeded residual.
pub const RANK5_SKILL_POINTS_NEEDED: i32 = 5000;

/// Retail Rank 1–4 SciencePurchasePointsGranted residual.
pub const RANK_SCIENCE_POINTS_DEFAULT: i32 = 1;
/// Retail Rank 5 SciencePurchasePointsGranted residual (generals promotion burst).
pub const RANK5_SCIENCE_POINTS_GRANTED: i32 = 3;

/// Retail RankName residual keys (GameText / INI).
pub const RANK1_NAME_KEY: &str = "INI:RankLevel1";
pub const RANK2_NAME_KEY: &str = "INI:RankLevel2";
pub const RANK3_NAME_KEY: &str = "INI:RankLevel3";
pub const RANK4_NAME_KEY: &str = "INI:RankLevel4";
pub const RANK5_NAME_KEY: &str = "INI:RankLevel5";

/// Retail SciencesGranted residual tokens.
pub const SCIENCE_RANK1: &str = "SCIENCE_Rank1";
pub const SCIENCE_RANK2: &str = "SCIENCE_Rank2";
pub const SCIENCE_RANK3: &str = "SCIENCE_Rank3";
pub const SCIENCE_RANK4: &str = "SCIENCE_Rank4";
pub const SCIENCE_RANK5: &str = "SCIENCE_Rank5";

/// One Rank.ini residual row (1-based level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RetailRankResidual {
    pub level: u32,
    pub skill_points_needed: i32,
    pub science_purchase_points_granted: i32,
    pub rank_name_key: &'static str,
    pub science_granted: &'static str,
}

/// Complete retail Rank.ini residual table (levels 1–5).
pub const RETAIL_RANK_TABLE: [RetailRankResidual; 5] = [
    RetailRankResidual {
        level: 1,
        skill_points_needed: RANK1_SKILL_POINTS_NEEDED,
        science_purchase_points_granted: RANK_SCIENCE_POINTS_DEFAULT,
        rank_name_key: RANK1_NAME_KEY,
        science_granted: SCIENCE_RANK1,
    },
    RetailRankResidual {
        level: 2,
        skill_points_needed: RANK2_SKILL_POINTS_NEEDED,
        science_purchase_points_granted: RANK_SCIENCE_POINTS_DEFAULT,
        rank_name_key: RANK2_NAME_KEY,
        science_granted: SCIENCE_RANK2,
    },
    RetailRankResidual {
        level: 3,
        skill_points_needed: RANK3_SKILL_POINTS_NEEDED,
        science_purchase_points_granted: RANK_SCIENCE_POINTS_DEFAULT,
        rank_name_key: RANK3_NAME_KEY,
        science_granted: SCIENCE_RANK3,
    },
    RetailRankResidual {
        level: 4,
        skill_points_needed: RANK4_SKILL_POINTS_NEEDED,
        science_purchase_points_granted: RANK_SCIENCE_POINTS_DEFAULT,
        rank_name_key: RANK4_NAME_KEY,
        science_granted: SCIENCE_RANK4,
    },
    RetailRankResidual {
        level: 5,
        skill_points_needed: RANK5_SKILL_POINTS_NEEDED,
        science_purchase_points_granted: RANK5_SCIENCE_POINTS_GRANTED,
        rank_name_key: RANK5_NAME_KEY,
        science_granted: SCIENCE_RANK5,
    },
];

/// Lookup residual rank row by 1-based level.
pub fn retail_rank_for_level(level: u32) -> Option<&'static RetailRankResidual> {
    RETAIL_RANK_TABLE.iter().find(|r| r.level == level)
}

/// Highest residual rank level for a skill-point total (Rank.ini thresholds).
/// Returns **0** when points are negative (fail-closed vs rank 1 at 0).
pub fn retail_rank_level_for_skill_points(skill_points: i32) -> u32 {
    if skill_points < 0 {
        return 0;
    }
    let mut best = 0u32;
    for row in &RETAIL_RANK_TABLE {
        if skill_points >= row.skill_points_needed {
            best = row.level;
        }
    }
    best
}

/// Cumulative science purchase points granted through `level` inclusive.
pub fn retail_cumulative_science_points_through(level: u32) -> i32 {
    RETAIL_RANK_TABLE
        .iter()
        .filter(|r| r.level <= level)
        .map(|r| r.science_purchase_points_granted)
        .sum()
}

/// Wave 80 honesty: Rank.ini residual table completeness.
///
/// Fail-closed: not full RankInfoStore INI parse / GeneralsExperience live UI.
pub fn honesty_science_rank_residual_pack_wave80() -> bool {
    RETAIL_RANK_COUNT == 5
        && RETAIL_RANK_TABLE.len() == RETAIL_RANK_COUNT as usize
        && RANK1_SKILL_POINTS_NEEDED == 0
        && RANK2_SKILL_POINTS_NEEDED == 800
        && RANK3_SKILL_POINTS_NEEDED == 1500
        && RANK4_SKILL_POINTS_NEEDED == 2500
        && RANK5_SKILL_POINTS_NEEDED == 5000
        && RANK_SCIENCE_POINTS_DEFAULT == 1
        && RANK5_SCIENCE_POINTS_GRANTED == 3
        && RANK1_NAME_KEY == "INI:RankLevel1"
        && RANK5_NAME_KEY == "INI:RankLevel5"
        && SCIENCE_RANK1 == "SCIENCE_Rank1"
        && SCIENCE_RANK5 == "SCIENCE_Rank5"
        && retail_rank_for_level(1).map(|r| r.skill_points_needed) == Some(0)
        && retail_rank_for_level(5).map(|r| r.science_purchase_points_granted) == Some(3)
        && retail_rank_for_level(0).is_none()
        && retail_rank_for_level(6).is_none()
        && retail_rank_level_for_skill_points(-1) == 0
        && retail_rank_level_for_skill_points(0) == 1
        && retail_rank_level_for_skill_points(799) == 1
        && retail_rank_level_for_skill_points(800) == 2
        && retail_rank_level_for_skill_points(1499) == 2
        && retail_rank_level_for_skill_points(1500) == 3
        && retail_rank_level_for_skill_points(2499) == 3
        && retail_rank_level_for_skill_points(2500) == 4
        && retail_rank_level_for_skill_points(4999) == 4
        && retail_rank_level_for_skill_points(5000) == 5
        && retail_rank_level_for_skill_points(99999) == 5
        // Cumulative SPP: 1+1+1+1+3 = 7 through rank 5; 1 through rank 1.
        && retail_cumulative_science_points_through(1) == 1
        && retail_cumulative_science_points_through(4) == 4
        && retail_cumulative_science_points_through(5) == 7
        // Monotonic skill thresholds.
        && RETAIL_RANK_TABLE.windows(2).all(|w| {
            w[0].skill_points_needed < w[1].skill_points_needed && w[0].level + 1 == w[1].level
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_rank_residual_pack_wave80_honesty() {
        assert!(honesty_science_rank_residual_pack_wave80());
        let r3 = retail_rank_for_level(3).expect("rank3");
        assert_eq!(r3.rank_name_key, "INI:RankLevel3");
        assert_eq!(r3.science_granted, "SCIENCE_Rank3");
        assert_eq!(r3.skill_points_needed, 1500);
        assert_eq!(r3.science_purchase_points_granted, 1);
    }
}
