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
pub(super) fn host_grant_upgrade_kind(
    game_logic: &crate::game_logic::GameLogic,
    name: &str,
) -> Option<GrantUpgradeKind> {
    use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
        UPGRADE_STORE_TABLE_WAVE109, UPGRADE_TYPE_OBJECT, upgrade_store_row_wave109,
    };

    if let Some(kind) = game_logic
        .upgrade_template(name)
        .map(|template| template.get_upgrade_type())
    {
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

use super::{GameLogic, ObjectId};

impl GameLogic {
    /// C++ GrantUpgradeCreate.cpp:108-117 — PLAYER vs OBJECT, never both.
    /// Missing upgrade template: C++ DEBUG_ASSERTCRASH + return (skip).
    pub(in super::super::super) fn apply_grant_upgrade_creates(
        &mut self,
        object_id: ObjectId,
        grants: &[crate::game_logic::GrantUpgradeCreateMetadata],
    ) {
        if grants.is_empty() {
            return;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        let player_id = self.player_owner_for_host_object(obj);
        let mut radar_upgrade = false;
        for grant in grants {
            match host_grant_upgrade_kind(self, &grant.upgrade_name) {
                Some(GrantUpgradeKind::Player) => {
                    if let Some(pid) = player_id {
                        if let Some(player) = self.players.get_mut(&pid) {
                            player.add_completed_upgrade(&grant.upgrade_name);
                        }
                    }
                }
                Some(GrantUpgradeKind::Object) => {
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        o.apply_upgrade_tag(&grant.upgrade_name);
                    }
                    if crate::game_logic::host_upgrades::HostUpgradeKind::from_name(
                        &grant.upgrade_name,
                    ) == crate::game_logic::host_upgrades::HostUpgradeKind::Radar
                    {
                        radar_upgrade = true;
                    }
                }
                None => {}
            }
        }
        if radar_upgrade {
            // C++ GrantUpgradeCreate → updateUpgradeModules → RadarUpgrade::extendRadar
            self.maybe_start_radar_extend(object_id);
        }
    }
}
