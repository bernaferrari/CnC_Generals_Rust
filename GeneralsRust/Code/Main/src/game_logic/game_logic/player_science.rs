//! Science purchases and grants for players without receiving content.

use super::Player;
use generals_sim::science::{
    fallback_purchase_point_cost, fallback_science, normalize_science_name,
};
use std::collections::HashSet;

impl Player {
    /// C++ `Player::grantScience` — refuse when `ScienceStore::isScienceGrantable` is false.
    pub fn grant_science(&mut self, science_name: &str) -> bool {
        if !host_science_is_grantable(science_name) {
            return false;
        }
        self.unlock_science(science_name)
    }

    /// C++ Player::isCapableOfPurchasingScience for fallback sessions.
    pub fn is_capable_of_purchasing_science(&self, science_name: &str) -> bool {
        if self.is_science_disabled(science_name) || self.is_science_hidden(science_name) {
            return false;
        }
        if legacy_science_hidden_or_disabled(self.id, science_name) {
            return false;
        }
        can_purchase(
            &self.unlocked_sciences,
            self.science_purchase_points,
            science_name,
        )
    }

    /// C++ Player::attemptToPurchaseScience for fallback sessions.
    ///
    /// Spend science purchase points. A zero cost makes a science unpurchasable.
    pub fn attempt_to_purchase_science(&mut self, science_name: &str) -> bool {
        let canonical = normalize_science_name(science_name);
        if !self.is_capable_of_purchasing_science(&canonical) {
            return false;
        }
        let cost = purchase_point_cost(&canonical).unwrap_or(0);
        if cost <= 0 || cost > self.science_purchase_points {
            return false;
        }
        self.science_purchase_points -= cost;
        let unlocked = self.unlock_science(&canonical);
        if unlocked {
            mirror_science_to_legacy_player(self.id, &canonical);
            mirror_points_to_legacy_player(self.id, self.science_purchase_points);
        }
        unlocked
    }
}

fn purchase_point_cost(science_name: &str) -> Option<i32> {
    let canonical = normalize_science_name(science_name);
    if canonical.is_empty() {
        return None;
    }
    science_ini_purchase_cost(&canonical).or_else(|| fallback_purchase_point_cost(&canonical))
}

fn science_ini_purchase_cost(name: &str) -> Option<i32> {
    if let Some(cost) = science_ini_store_cost(name) {
        return Some(cost);
    }
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return Some(store.get_science_purchase_cost(science));
        }
    }
    None
}

fn science_ini_store_cost(name: &str) -> Option<i32> {
    let store = game_engine::common::ini::ini_science::get_science_store();
    let science = store.get_science_from_internal_name(name)?;
    Some(store.get_science_purchase_cost(science))
}

fn science_ini_prereq_names(name: &str) -> Option<Vec<String>> {
    {
        let leftover = game_engine::common::ini::ini_science::get_science_store();
        if let Some(st) = leftover.get_science_from_internal_name(name) {
            if let Some(info) = leftover.find_science_info(st) {
                return Some(
                    info.prereq_sciences
                        .iter()
                        .filter_map(|p| leftover.get_internal_name_for_science(*p))
                        .map(|a| a.to_string())
                        .collect(),
                );
            }
        }
    }
    let rts = game_engine::common::rts::get_science_store()?;
    let st = rts.get_science_from_internal_name(name);
    if st == game_engine::common::rts::SCIENCE_INVALID {
        return None;
    }
    let info = rts.find_science_info(st)?;
    Some(
        info.prereq_sciences
            .iter()
            .map(|&p| rts.get_internal_name_for_science(p).to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// C++ `ScienceStore::playerHasPrereqsForScience` (Science.cpp:257-274).
///
/// The active catalog takes precedence over the fallback definition.
fn prerequisites_met(unlocked: &HashSet<String>, science_name: &str) -> bool {
    let science_name = normalize_science_name(science_name);
    let science_name = science_name.as_str();
    let has = |req: &str| -> bool {
        if req.is_empty() {
            return true;
        }
        let r = req.to_ascii_lowercase().replace('-', "_");
        unlocked.iter().any(|u| {
            let n = u.to_ascii_lowercase().replace('-', "_");
            n == r
        })
    };
    if let Some(prereqs) = science_ini_prereq_names(science_name) {
        return prereqs.iter().all(|p| has(p));
    }
    if let Some(row) = fallback_science(science_name.trim()) {
        return row.prerequisites.into_iter().all(has);
    }
    // C++ Science.cpp:271-273 — unknown science has no prereq info → false.
    false
}

/// C++ `Player::isCapableOfPurchasingScience` for fallback definitions.
fn can_purchase(
    unlocked: &HashSet<String>,
    science_purchase_points: i32,
    science_name: &str,
) -> bool {
    let name_owned = normalize_science_name(science_name);
    let name = name_owned.as_str();
    if name.is_empty() {
        return false;
    }
    // Already owned.
    let key = name.to_ascii_lowercase().replace('-', "_");
    if unlocked.iter().any(|u| {
        let n = u.to_ascii_lowercase().replace('-', "_");
        n == key
    }) {
        return false;
    }
    let Some(cost) = purchase_point_cost(name) else {
        return false;
    };
    if cost <= 0 || cost > science_purchase_points {
        return false;
    }
    prerequisites_met(unlocked, name)
}

/// C++ `ScienceStore::isScienceGrantable` — unknown science is not grantable.
fn host_science_is_grantable(science_name: &str) -> bool {
    let name = science_name.trim();
    if name.is_empty() {
        return false;
    }
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return store.is_science_grantable(science);
        }
    }
    let leftover = game_engine::common::ini::ini_science::get_science_store();
    if let Some(st) = leftover.get_science_from_internal_name(name) {
        return leftover.is_science_grantable(st);
    }
    fallback_science(name)
        .map(|row| row.grantable)
        .unwrap_or(false)
}

/// Mirror host `Player::addScience` onto crate `Player` ScienceVec so
/// ControlBar `sciences_ids` (impl_command_context.rs) sees the purchase.
fn mirror_science_to_legacy_player(player_id: u32, science_name: &str) -> bool {
    let canonical = normalize_science_name(science_name);
    if canonical.is_empty() {
        return false;
    }
    let science = science_type_from_name(&canonical);
    if science == game_engine::common::rts::SCIENCE_INVALID {
        return false;
    }
    let player_list = gamelogic::player::ThePlayerList();

    let Ok(list) = player_list.read() else {
        return false;
    };
    let player_arc = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return false;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return false;
    };
    player.add_science(science)
}

/// Mirror host SPP onto crate Player so ControlBar remaining-points matches.
fn mirror_points_to_legacy_player(player_id: u32, spp: i32) -> bool {
    let player_list = gamelogic::player::ThePlayerList();

    let Ok(list) = player_list.read() else {
        return false;
    };
    let player_arc = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return false;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return false;
    };
    let current = player.get_science_purchase_points();
    let delta = spp - current;
    if delta != 0 {
        player.add_science_purchase_points(delta);
    }
    true
}

/// C++ `Player::isScienceDisabled` / `isScienceHidden` from leftover Player
/// written by `PLAYER_SCIENCE_AVAILABILITY` (ScriptActions).
fn legacy_science_hidden_or_disabled(player_id: u32, science_name: &str) -> bool {
    let canonical = normalize_science_name(science_name);
    if canonical.is_empty() {
        return false;
    }
    let science = science_type_from_name(&canonical);
    if science == game_engine::common::rts::SCIENCE_INVALID {
        return false;
    }
    let player_list = gamelogic::player::ThePlayerList();

    let Ok(list) = player_list.read() else {
        return false;
    };
    let Some(player_arc) = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
    else {
        return false;
    };
    drop(list);
    let Ok(player) = player_arc.read() else {
        return false;
    };
    player.is_science_disabled(science) || player.is_science_hidden(science)
}

fn science_type_from_name(name: &str) -> game_engine::common::rts::ScienceType {
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return science;
        }
    }
    if let Some(science) = game_engine::common::ini::ini_science::get_science_store()
        .get_science_from_internal_name(name)
    {
        return science.0;
    }
    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
        as game_engine::common::rts::ScienceType
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_science_ini_row_is_fail_closed() {
        // C++ Science.cpp:220-222 + 271-273: unknown science cost 0, prereqs false.
        assert_eq!(purchase_point_cost("SCIENCE_DefinitelyNotInIni"), Some(0));
        let unlocked = std::collections::HashSet::new();
        assert!(!prerequisites_met(&unlocked, "SCIENCE_DefinitelyNotInIni"));
        assert!(!can_purchase(&unlocked, 99, "SCIENCE_DefinitelyNotInIni"));
    }

    #[test]
    fn artillery_barrage3_requires_chain_not_rank1() {
        // C++ Player.cpp:2611-2635 — ArtilleryBarrage3 needs Barrage2 + Rank3.
        let mut unlocked = std::collections::HashSet::new();
        unlocked.insert("SCIENCE_CHINA".into());
        unlocked.insert("SCIENCE_Rank1".into());
        assert!(!can_purchase(&unlocked, 5, "SCIENCE_ArtilleryBarrage3"));
        unlocked.insert("SCIENCE_ArtilleryBarrage1".into());
        unlocked.insert("SCIENCE_ArtilleryBarrage2".into());
        unlocked.insert("SCIENCE_Rank3".into());
        assert!(can_purchase(&unlocked, 1, "SCIENCE_ArtilleryBarrage3"));
    }

    #[test]
    fn fallback_cash_bounty_requires_positive_cost_and_prerequisites() {
        let mut unlocked = HashSet::from(["SCIENCE_GLA".into()]);
        assert_eq!(purchase_point_cost("SCIENCE_Rank1"), Some(0));
        assert_eq!(purchase_point_cost("SCIENCE_CashBounty1"), Some(1));
        assert!(!can_purchase(&unlocked, 1, "SCIENCE_CashBounty1"));
        unlocked.insert("SCIENCE_Rank3".into());
        assert!(can_purchase(&unlocked, 1, "SCIENCE_CashBounty1"));
    }
}
