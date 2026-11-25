//! Object and Player upgrade entry points and per-source cost contributions.
use super::super::*;
use game_engine::common::thing::module::{ModuleData, ModuleDataHandle, ModuleDataView};

fn upgrade_data_view<T: ModuleData + 'static>(
    handle: &ModuleDataHandle,
) -> Option<ModuleDataView<T>> {
    if !handle.is::<T>() {
        return None;
    }
    Some(ModuleDataView::new(handle.clone(), |data| {
        data.as_any()
            .downcast_ref::<T>()
            .expect("upgrade module data type")
    }))
}

mod behavior_order;
mod ordered_mux;
mod spawn_effects;

pub(in crate::game_logic) struct AuthoredCostModifierUpgrade {
    state_index: usize,
    data: ModuleDataView<
        gamelogic::object::upgrade::cost_modifier_upgrade::CostModifierUpgradeModuleData,
    >,
}

impl AuthoredCostModifierUpgrade {
    fn from_data(state_index: usize, data: &ModuleDataHandle) -> Option<Self> {
        Some(Self {
            state_index,
            data: upgrade_data_view(data)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpgradeGrantOrigin {
    Object,
    PlayerResearch,
    PlayerResearchResiduals,
}

impl GameLogic {
    /// StealthUpgrade.cpp sets CAN_STEALTH on the owner and its current weapon spawns.
    fn apply_stealth_upgrade_effect(&mut self, object_id: ObjectId) {
        let Some(object) = self.objects.get_mut(&object_id) else {
            return;
        };
        object.innate_stealth = true;
        if !object.is_kind_of(KindOf::SpawnsAreTheWeapons) {
            return;
        }
        for (child_id, birth) in object.spawn_behavior_children() {
            if let Some(child) = self.objects.get_mut(&child_id).filter(|child| {
                birth != 0 && child.visual_object_generation == birth
            }) {
                child.innate_stealth = true;
            }
        }
    }

    pub(in crate::game_logic) fn object_birth_matches(
        &self,
        object_id: ObjectId,
        birth: &std::sync::Weak<()>,
    ) -> bool {
        self.objects.get(&object_id).is_some_and(|object| {
            birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        })
    }

    /// CostModifierUpgrade::onCapture, at this module's slot in Object::onCapture.
    pub(in crate::game_logic) fn capture_cost_modifier_slot(
        &mut self,
        object_id: ObjectId,
        module: &AuthoredCostModifierUpgrade,
        old_owner: Option<u32>,
        new_owner: Option<u32>,
    ) {
        let Some(executed) = self
            .objects
            .get_mut(&object_id)
            .and_then(|object| {
                object
                    .cost_modifier_upgrade_executed
                    .get_mut(module.state_index)
            })
            .filter(|executed| **executed)
        else {
            return;
        };
        let data = module.data.read();
        if let Some(player) = old_owner.and_then(|id| self.players.get_mut(&id)) {
            player
                .remove_authored_kind_of_production_cost_change(data.kind_of(), data.percentage());
            *executed = false;
        }
        if let Some(player) = new_owner.and_then(|id| self.players.get_mut(&id)) {
            player.add_authored_kind_of_production_cost_change(data.kind_of(), data.percentage());
            *executed = true;
        }
    }

    pub(super) fn delete_cost_modifier_slot(
        &mut self,
        object_id: ObjectId,
        module: &AuthoredCostModifierUpgrade,
    ) {
        let owner = self
            .objects
            .get(&object_id)
            .and_then(|object| self.player_owner_for_host_object(object));
        let Some(executed) = self
            .objects
            .get_mut(&object_id)
            .and_then(|object| {
                object
                    .cost_modifier_upgrade_executed
                    .get_mut(module.state_index)
            })
            .filter(|executed| **executed)
        else {
            return;
        };
        if let Some(player) = owner.and_then(|id| self.players.get_mut(&id)) {
            let data = module.data.read();
            player
                .remove_authored_kind_of_production_cost_change(data.kind_of(), data.percentage());
        }
        *executed = false;
    }

    pub(in crate::game_logic) fn destroy_object_if_birth_matches_with_effects(
        &mut self,
        object_id: ObjectId,
        birth: &std::sync::Weak<()>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if !self.object_birth_matches(object_id, birth) {
            return Ok(false);
        }
        self.destroy_object_with_effects(object_id, presentation_effects)?;
        Ok(true)
    }

    /// Apply an upgrade tag to an object.
    /// Mirrors C++ behavior where upgrades are persistent object state, not display-name edits.
    pub(crate) fn apply_upgrade_to_object(&mut self, object_id: ObjectId, upgrade: &str) {
        self.apply_upgrade_to_object_with_effects(
            object_id,
            upgrade,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_upgrade_to_object_with_effects(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_upgrade_to_object_with_effects_checked(object_id, upgrade, presentation_effects)
            .expect("owned upgrade callback admission must complete");
    }

    pub(crate) fn apply_upgrade_to_object_with_effects_checked(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_upgrade_to_object_with_interpreter_checked(
            object_id,
            upgrade,
            None,
            presentation_effects,
        )
    }
    pub(crate) fn apply_upgrade_to_object_with_interpreter_checked(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_upgrade_to_object_with_interpreter_and_origin_checked(
            object_id,
            upgrade,
            interpreter,
            presentation_effects,
            UpgradeGrantOrigin::Object,
        )
    }

    pub(crate) fn apply_player_research_to_object_with_interpreter_checked(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_upgrade_to_object_with_interpreter_and_origin_checked(
            object_id,
            upgrade,
            interpreter,
            presentation_effects,
            UpgradeGrantOrigin::PlayerResearch,
        )
    }

    /// Birth/construction replay runs the ordered mux once with the combined
    /// mask, after replaying families not yet admitted to that traversal.
    pub(crate) fn apply_player_research_residuals_with_interpreter_checked(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_upgrade_to_object_with_interpreter_and_origin_checked(
            object_id,
            upgrade,
            interpreter,
            presentation_effects,
            UpgradeGrantOrigin::PlayerResearchResiduals,
        )
    }

    fn apply_upgrade_to_object_with_interpreter_and_origin_checked(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        origin: UpgradeGrantOrigin,
    ) -> gamelogic::GameLogicResult<()> {
        let mut satellite_hack_activate = None;
        let Some((identity, status_upgrade_first_give, object_birth)) =
            self.objects.get(&object_id).map(|object| {
                (
                    self.live_draw_playback_identity(object),
                    !object.has_upgrade_tag(upgrade),
                    std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                )
            })
        else {
            return Ok(());
        };
        let has_admitted_upgrade_roster = self.has_admitted_upgrade_roster(object_id);
        let raw_stealth_triggered = !has_admitted_upgrade_roster
            && self.objects.get(&object_id).is_some_and(|obj| {
                let player = obj.owner_player_id.and_then(|id| self.players.get(&id));
                let had = |name: &str| {
                    obj.has_upgrade_tag(name)
                        || (!name.eq_ignore_ascii_case(upgrade)
                            && player.is_some_and(|player| player.has_unlocked_upgrade(name)))
                };
                let has = |name: &str| had(name) || name.eq_ignore_ascii_case(upgrade);
                obj.thing()
                    .template
                    .stealth_upgrades
                    .iter()
                    .any(|rules| !rules.satisfied_by(&had) && rules.satisfied_by(&has))
            });
        // C++ updateUpgradeModules tests every module against the combined
        // controlling-player/object mask. Snapshot the exact admitted module
        // declarations and current mask before any callbacks mutate either.
        let authored_pass = if origin == UpgradeGrantOrigin::PlayerResearchResiduals {
            None
        } else {
            self.prepare_authored_upgrade_pass(object_id, Some(upgrade))
        };
        let object_completion_name = if origin == UpgradeGrantOrigin::Object {
            let Some(template) = self.upgrade_template(upgrade) else {
                // Object.cpp:3559-3567 ignores a missing UpgradeTemplate on
                // map grants; Object::giveUpgrade likewise does nothing for
                // a null template. Keep unknown map/OCL upgrade tokens inert.
                return Ok(());
            };
            Some(template.get_name().as_str().to_owned())
        } else {
            None
        };
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ UpgradeMux::isAlreadyUpgraded — first give only.
            // C++ wouldUpgrade ConflictsWith: leftover ObjectCreationUpgrade already
            // matches; host objects skip OCL when another drone tag is complete.
            let first_give = status_upgrade_first_give;
            obj.apply_upgrade_tag(upgrade);
            if let Some(name) = &object_completion_name {
                // Object::giveUpgrade records any supplied template mask,
                // regardless of the template's declared UpgradeType.
                obj.completed_object_upgrades.insert(name.clone());
            }
            if first_give {
                if let Some(spec) =
                    crate::game_logic::host_satellite_hack::satellite_hack_spy_spec(upgrade)
                {
                    if crate::game_logic::host_satellite_hack::object_authors_spy_vision_update(
                        &obj.template_name,
                    ) || obj.is_kind_of(KindOf::FSInternetCenter)
                    {
                        satellite_hack_activate = Some(spec);
                    }
                }
            }
        }
        if let Some(pass) = authored_pass {
            self.execute_authored_ordered_upgrade_modules(pass, interpreter, presentation_effects)?;
        }
        if !self
            .objects
            .get(&object_id)
            .is_some_and(|object| self.live_draw_playback_identity(object) == identity)
        {
            return Ok(());
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if !has_admitted_upgrade_roster {
                let tags = obj.applied_upgrades.clone();
                let shown = obj.run_raw_template_visual_upgrades(|name| {
                    name.eq_ignore_ascii_case(upgrade)
                        || tags.iter().any(|tag| tag.eq_ignore_ascii_case(name))
                });
                if !shown.is_empty() {
                    let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
                    self.sub_objects_upgrades.record(&shown);
                }
            }
            if !has_admitted_upgrade_roster {
                obj.run_raw_template_stealth_upgrades(|name| name.eq_ignore_ascii_case(upgrade));
                let flags =
                    obj.apply_raw_template_flag_upgrades(|name| name.eq_ignore_ascii_case(upgrade));
                if let Some(command_set) = flags.command_set.as_deref() {
                    self.replace_grant_command_upgrades
                        .record_command_set(command_set);
                }
                if flags.armor_set {
                    self.upgrade_module_residuals.record_armor_set(upgrade);
                }
            }
            // C++ LocomotorSetUpgrade::upgradeImplementation only — not a
            // name-sniff of Heroic rank or AutoLoader research.
            if crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade_in(
                obj,
                upgrade,
                self.locomotor_definitions.receiving(),
            ) {
                self.upgrade_module_residuals.record_locomotor_set(upgrade);
            }
            // C++ UnpauseSpecialPowerUpgrade::upgradeImplementation: every
            // authored module triggered by this upgrade unpauses the power
            // its SpecialPowerTemplate names (capture, radar scan, booby
            // trap, helix bombs). Capture's three faction powers are
            // separate modules, each unpaused by its own module.
            let unpaused = super::authored_upgrade_modules::authored_unpause_powers(
                &obj.thing().template,
                |name| name.eq_ignore_ascii_case(upgrade),
            );
            for power in &unpaused {
                obj.pause_special_power_countdown(power, false);
            }
            if !unpaused.is_empty() {
                self.upgrade_module_residuals.record_unpause(upgrade);
            }
        }
        if raw_stealth_triggered {
            self.apply_stealth_upgrade_effect(object_id);
        }

        if !has_admitted_upgrade_roster {
            self.apply_authored_power_plant_upgrades(object_id, |name| {
                name.eq_ignore_ascii_case(upgrade)
            });
        }

        // C++ SpyVisionUpdate::upgradeImplementation — activate on upgrade.
        if let Some(spec) = satellite_hack_activate {
            let _ = self.activate_satellite_hack_spy_vision(object_id, spec);
        }

        // C++ GenerateMinefieldBehavior::upgradeImplementation residual.
        let _mines = self.apply_structure_minefield_upgrade_with_interpreter_and_effects(
            object_id,
            upgrade,
            interpreter,
            presentation_effects,
        );

        // C++ GrantScienceUpgrade residual.
        if let Some(science) =
            crate::game_logic::host_replace_object_upgrade::grant_science_for_upgrade(upgrade)
                .filter(|_| status_upgrade_first_give)
        {
            let owner = self
                .objects
                .get(&object_id)
                .filter(|object| self.live_draw_playback_identity(object) == identity)
                .and_then(|object| object.owner_player_id);
            if let Some(owner) = owner {
                if let Some(player) = self.players.get_mut(&owner) {
                    if player.unlock_science(science) {
                        self.replace_grant_command_upgrades.record_science(science);
                    }
                }
            }
        }

        // Compatibility for manually constructed templates without admitted
        // behavior data. An admitted template is governed by its mux alone.
        if status_upgrade_first_give
            && crate::game_logic::host_replace_object_upgrade::is_replace_object_upgrade(upgrade)
        {
            let legacy_replacement = self.objects.get(&object_id).and_then(|object| {
                let source = object.get_template();
                if source.receiving_client_template.is_some() {
                    return None;
                }
                let target =
                    crate::game_logic::host_replace_object_upgrade::replacement_template_for_fake(
                        &object.template_name,
                    )?;
                Some((object.template_name.clone(), target))
            });
            if let Some((source, target)) = legacy_replacement {
                if !self.templates.contains_key(&target) {
                    if let Some(source) = self.templates.get(&source).cloned() {
                        let mut legacy_target = source;
                        legacy_target.name = target.clone();
                        self.templates.insert(target.clone(), legacy_target);
                    }
                }
                return self.replace_object_with_effects(
                    object_id,
                    &object_birth,
                    &target,
                    true,
                    interpreter,
                    presentation_effects,
                );
            }
        }
        self.admit_host_radar_provider_with_effects(object_id, presentation_effects);
        Ok(())
    }

    /// Admitted objects grant upgrades only through their ordered behavior roster.
    pub(super) fn has_admitted_upgrade_roster(&self, object_id: ObjectId) -> bool {
        self.objects.get(&object_id).is_some_and(|object| {
            self.receiving_module_content
                .as_ref()
                .is_some_and(|content| content.admitted_template(&object.template_name).is_some())
        })
    }
}

#[cfg(test)]
mod armor_upgrade_tests;
#[cfg(test)]
mod command_set_tests;
#[cfg(test)]
mod cost_modifier_receiving_owner_tests;
#[cfg(test)]
mod replace_position_owner_tests;
#[cfg(test)]
mod visual_upgrade_tests;
#[cfg(test)]
mod stealth_upgrade_tests;
