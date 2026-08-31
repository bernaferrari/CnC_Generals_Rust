//! `GrantUpgradeCreate` upgrade-kind lookup authority.

/// C++ `UpgradeType` for `GrantUpgradeCreate` branching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GrantUpgradeKind {
    Player,
    Object,
}

/// C++ `TheUpgradeCenter->findUpgrade` then `getUpgradeType()`.
/// Residual store covers tests / unloaded INI. Missing template → `None`
/// (`GrantUpgradeCreate.cpp:102-105` returns without granting).
pub(super) fn host_grant_upgrade_kind(name: &str) -> Option<GrantUpgradeKind> {
    use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
        UPGRADE_STORE_TABLE_WAVE109, UPGRADE_TYPE_OBJECT, upgrade_store_row_wave109,
    };

    if let Some(kind) = gamelogic::upgrade::center::with_upgrade_center(|center| {
        center
            .find_upgrade(name)
            .map(|template| template.get_upgrade_type())
    }) {
        return Some(match kind {
            gamelogic::upgrade::UpgradeType::Object => GrantUpgradeKind::Object,
            gamelogic::upgrade::UpgradeType::Player => GrantUpgradeKind::Player,
        });
    }
    if let Some(row) = upgrade_store_row_wave109(name).or_else(|| {
        UPGRADE_STORE_TABLE_WAVE109
            .iter()
            .find(|row| row.name.eq_ignore_ascii_case(name.trim()))
    }) {
        return Some(if row.upgrade_type == UPGRADE_TYPE_OBJECT {
            GrantUpgradeKind::Object
        } else {
            GrantUpgradeKind::Player
        });
    }
    None
}
