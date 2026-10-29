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
        self.apply_grant_upgrade_creates_with_effects(object_id, grants, &mut Default::default())
    }

    pub(in super::super::super) fn apply_grant_upgrade_creates_with_effects(
        &mut self,
        object_id: ObjectId,
        grants: &[crate::game_logic::GrantUpgradeCreateMetadata],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Err(error) = self.apply_grant_upgrade_creates_with_interpreter_and_effects(
            object_id, grants, None, effects,
        ) {
            log::error!("GrantUpgradeCreate failed: {error}");
        }
    }
    pub(in super::super::super) fn apply_grant_upgrade_creates_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        grants: &[crate::game_logic::GrantUpgradeCreateMetadata],
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if grants.is_empty() {
            return Ok(());
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return Ok(());
        };
        let player_id = self.player_owner_for_host_object(obj);
        let object_birth = std::sync::Arc::downgrade(&obj.visual_draw_identity_token);
        let mut radar_upgrade = false;
        for grant in grants {
            if !self.object_birth_matches(object_id, &object_birth) {
                return Ok(());
            }
            match host_grant_upgrade_kind(self, &grant.upgrade_name) {
                Some(GrantUpgradeKind::Player) => {
                    if let Some(pid) = player_id {
                        self.add_completed_player_upgrade_with_interpreter_and_effects(
                            pid,
                            &grant.upgrade_name,
                            interpreter,
                            effects,
                        );
                    }
                }
                Some(GrantUpgradeKind::Object) => {
                    // C++ GrantUpgradeCreate::onCreate calls Object::giveUpgrade,
                    // which updates the Object completion mask and immediately
                    // dispatches UpgradeMux callbacks. Keep that operation at
                    // this authored Create-module position and retain its birth
                    // guard across reentrant callbacks.
                    self.apply_upgrade_to_object_with_interpreter_checked(
                        object_id,
                        &grant.upgrade_name,
                        interpreter,
                        effects,
                    )?;
                    if crate::game_logic::host_upgrades::HostUpgradeKind::from_name(
                        &grant.upgrade_name,
                    ) == crate::game_logic::host_upgrades::HostUpgradeKind::Radar
                    {
                        radar_upgrade = true;
                    }
                }
                None => {}
            }
            if !self.object_birth_matches(object_id, &object_birth) {
                return Ok(());
            }
        }
        self.admit_host_radar_provider_with_effects(object_id, effects);
        if radar_upgrade
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.thing().template.radar_upgrade_rules.is_none())
        {
            // C++ GrantUpgradeCreate → updateUpgradeModules → RadarUpgrade::extendRadar
            self.maybe_start_radar_extend(object_id);
        }
        Ok(())
    }
}
