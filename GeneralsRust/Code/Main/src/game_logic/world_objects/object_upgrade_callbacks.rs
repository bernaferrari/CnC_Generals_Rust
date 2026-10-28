//! C++ ObjectCreationUpgrade / RadarUpgrade / per-object upgrade callback order.
//! Extracted from object_queries; one receiving implementation per callback.
use super::super::*;

struct AuthoredObjectCreationUpgrade {
    source_index: usize,
    state_index: usize,
    data: gamelogic::upgrade::modules::object_creation::ObjectCreationUpgradeModuleData,
}

struct AuthoredStatusBitsUpgrade {
    source_index: usize,
    state_index: usize,
    data: gamelogic::object::upgrade::status_bits_upgrade::StatusBitsUpgradeModuleData,
}

enum AuthoredOrderedUpgrade {
    ObjectCreation(AuthoredObjectCreationUpgrade),
    StatusBits(AuthoredStatusBitsUpgrade),
}

impl AuthoredOrderedUpgrade {
    fn source_index(&self) -> usize {
        match self {
            Self::ObjectCreation(module) => module.source_index,
            Self::StatusBits(module) => module.source_index,
        }
    }
}

impl GameLogic {
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
        use crate::game_logic::host_overlord_addons::{
            is_bunker_addon_upgrade, is_gattling_addon_upgrade, is_overlord_family_host,
            is_propaganda_addon_upgrade,
        };

        let mut installed_gattling = false;
        let mut installed_propaganda = false;
        let mut installed_bunker = false;
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
        // C++ updateUpgradeModules tests every module against the combined
        // controlling-player/object mask. Snapshot the exact admitted module
        // declarations and current mask before any callbacks mutate either.
        let object_creation_update_allowed = self.object_creation_upgrade_update_allowed(object_id);
        let authored_upgrade_modules = if object_creation_update_allowed {
            self.authored_ordered_upgrade_modules(object_id)
        } else {
            Vec::new()
        };
        let object_creation_key_mask =
            self.object_creation_upgrade_key_mask(object_id, Some(upgrade));
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ UpgradeMux::isAlreadyUpgraded — first give only.
            // C++ wouldUpgrade ConflictsWith: leftover ObjectCreationUpgrade already
            // matches; host objects skip OCL when another drone tag is complete.
            let first_give = status_upgrade_first_give;
            obj.apply_upgrade_tag(upgrade);
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
        if object_creation_update_allowed {
            self.execute_authored_ordered_upgrade_modules(
                object_id,
                &object_birth,
                authored_upgrade_modules,
                object_creation_key_mask,
                Some(upgrade),
                interpreter,
                presentation_effects,
            )?;
        }
        if !self
            .objects
            .get(&object_id)
            .is_some_and(|object| self.live_draw_playback_identity(object) == identity)
        {
            return Ok(());
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ SubObjectsUpgrade residual (BombTruck loads / Helix BombWing).
            {
                let applied =
                    crate::game_logic::host_sub_objects_upgrade::sub_objects_for_upgrade_tags(
                        &obj.applied_upgrades,
                        &obj.template_name,
                    );
                if applied.matched {
                    obj.sub_object_visibility
                        .apply_show_hide(&applied.show, &applied.hide);
                    self.sub_objects_upgrades.record(&applied.show);
                }
            }
            // C++ ModelConditionUpgrade residual.
            let _ = crate::game_logic::host_model_condition_upgrade::apply_model_condition_upgrade(
                &mut obj.model_condition_bits,
                upgrade,
            );
            // C++ WeaponBonusUpgrade residual.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_bonus_upgrade(upgrade) {
                obj.set_weapon_bonus_player_upgrade(true);
                self.upgrade_module_residuals.record_weapon_bonus(upgrade);
            }
            // C++ WeaponSetUpgrade residual → WEAPONSET_PLAYER_UPGRADE.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_set_upgrade(upgrade) {
                obj.set_weapon_set_flag(0, true);
                self.upgrade_module_residuals.record_weapon_set(upgrade);
            }
            // C++ ArmorUpgrade residual → ARMORSET_PLAYER_UPGRADE (+ ChemSuit decal).
            if crate::game_logic::host_upgrade_module_residuals::is_armor_upgrade(upgrade) {
                obj.set_armor_set_player_upgrade(true);
                if crate::game_logic::host_upgrade_module_residuals::is_chemical_suits_upgrade(
                    upgrade,
                ) {
                    obj.set_terrain_decal_chemsuit(true);
                }
                self.upgrade_module_residuals.record_armor_set(upgrade);
            }
            // C++ LocomotorSetUpgrade::upgradeImplementation only — not a
            // name-sniff of Heroic rank or AutoLoader research.
            if crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade_in(
                obj,
                upgrade,
                self.locomotor_definitions.as_ref(),
            ) {
                self.upgrade_module_residuals.record_locomotor_set(upgrade);
            }
            // C++ UnpauseSpecialPowerUpgrade residual.  Capture uses its
            // exact module-local `TriggeredBy` field; do not turn an upgrade
            // spelling that merely contains "capture" into an ability on a
            // Ranger/RedGuard/Rebel-named object.
            let capture_unpause = obj
                .thing()
                .template
                .capture_power
                .special_power_type()
                .filter(|_| {
                    obj.thing()
                        .template
                        .capture_upgrade_trigger
                        .as_deref()
                        .is_some_and(|trigger| trigger.trim().eq_ignore_ascii_case(upgrade.trim()))
                });
            if let Some(power) = capture_unpause {
                obj.pause_special_power_countdown(&power, false);
                self.upgrade_module_residuals.record_unpause(upgrade);
            } else if let Some(power) =
                crate::game_logic::host_upgrade_module_residuals::unpause_power_for_upgrade(upgrade)
            {
                // Capture-family values are handled only above by exact INI
                // metadata.  The remaining non-capture residual remains
                // compatible with its existing module peel.
                if crate::game_logic::CapturePowerKind::from_special_power_type(&power)
                    == crate::game_logic::CapturePowerKind::None
                {
                    for p in crate::game_logic::host_upgrade_module_residuals::unpause_power_family(
                        power,
                    ) {
                        obj.pause_special_power_countdown(&p, false);
                    }
                    self.upgrade_module_residuals.record_unpause(upgrade);
                }
            }
            // C++ CommandSetUpgrade residual.
            if let Some(cs) =
                crate::game_logic::host_replace_object_upgrade::command_set_override_for_upgrade(
                    upgrade,
                    &obj.template_name,
                )
            {
                obj.set_command_set_override(Some(cs.to_string()));
                self.replace_grant_command_upgrades.record_command_set(cs);
            }
            if is_overlord_family_host(&obj.template_name) {
                if is_gattling_addon_upgrade(upgrade) {
                    obj.install_overlord_gattling_addon();
                    installed_gattling = true;
                } else if is_propaganda_addon_upgrade(upgrade) {
                    obj.install_overlord_propaganda_addon();
                    installed_propaganda = true;
                } else if is_bunker_addon_upgrade(upgrade) {
                    // C++ ChinaTankOverlordBattleBunker TransportContain.Slots = 5.
                    // Helix bunker also uses Slots residual 5 (ChinaHelixBattleBunker).
                    obj.install_overlord_battle_bunker(5);
                    // C++ PassengersFireUpgrade TriggeredBy Upgrade_ChinaHelixBattleBunker.
                    use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
                    if should_enable_passengers_fire(upgrade, &obj.template_name)
                        || obj.is_helix_transport
                    {
                        obj.passengers_allowed_to_fire = true;

                        self.passengers_fire_upgrade_reg.record_apply(1);
                    }
                    installed_bunker = true;
                }
            }
        }

        // C++ SpyVisionUpdate::upgradeImplementation — activate on upgrade.
        if let Some(spec) = satellite_hack_activate {
            let _ = self.activate_satellite_hack_spy_vision(object_id, spec);
        }
        if installed_gattling {
            self.overlord_addons.record_gattling_install();
        }
        if installed_propaganda {
            self.overlord_addons.record_propaganda_install();
        }
        if installed_gattling || installed_propaganda || installed_bunker {
            self.ensure_overlord_portable_addon_occupant_with_interpreter_and_effects(
                object_id,
                interpreter,
                presentation_effects,
            );
        }

        // C++ CostModifierUpgrade residual — player KindOf production cost change.
        if let Some((kind, percent)) =
            crate::game_logic::host_upgrade_module_residuals::cost_modifier_for_upgrade(upgrade)
                .filter(|_| status_upgrade_first_give)
        {
            let owner = self
                .objects
                .get(&object_id)
                .filter(|object| self.live_draw_playback_identity(object) == identity)
                .and_then(|object| object.owner_player_id);
            if let Some(owner) = owner {
                if let Some(player) = self.players.get_mut(&owner) {
                    player.add_kind_of_production_cost_change(kind, percent);
                    self.upgrade_module_residuals.record_cost(upgrade);
                }
            }
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

        // ReplaceObjectUpgrade.cpp65 queues destruction; the old Object and
        // Drawable remain installed until processDestroyList's physical phase.
        if status_upgrade_first_give
            && crate::game_logic::host_replace_object_upgrade::is_replace_object_upgrade(upgrade)
        {
            let info = self.objects.get(&object_id).map(|object| {
                (
                    object.template_name.clone(),
                    object.team,
                    object.owner_player_id,
                    object.team_instance_name.clone(),
                    object.team_instance_id,
                    object.get_transform_matrix(),
                    object.get_position(),
                )
            });
            if let Some((template_name, team, owner, team_name, team_id, transform, position)) =
                info
            {
                if let Some(replacement) =
                    crate::game_logic::host_replace_object_upgrade::replacement_template_for_fake(
                        &template_name,
                    )
                {
                    if !self.templates.contains_key(&replacement) {
                        if let Some(source) = self.templates.get(&template_name).cloned() {
                            let mut target = source;
                            target.name = replacement.clone();
                            self.templates.insert(replacement.clone(), target);
                        }
                    }
                    self.destroy_object_with_effects(object_id, presentation_effects);
                    let created = if let (Some(controller), Some(team_id)) = (owner, team_id) {
                        self.create_object_on_exact_script_team_with_effects_checked(
                            &replacement,
                            controller,
                            &team_name,
                            team_id,
                            position,
                            true,
                            interpreter,
                            presentation_effects,
                        )?
                    } else {
                        self.create_object_for_owner_or_team_with_effects_checked(
                            &replacement,
                            team,
                            owner,
                            position,
                            interpreter,
                            presentation_effects,
                        )?
                    };
                    if let Some(new_id) = created {
                        if let Some(object) = self.objects.get_mut(&new_id) {
                            object.set_transform_matrix_with_effects_and_pose_owners(
                                transform,
                                self.frame,
                                &mut presentation_effects
                                    .reborrow_for_object(&mut self.partition_manager),
                                &self.host_trigger_world,
                                &self.team_factory,
                            );
                            object.status.under_construction = false;
                        }
                        self.replace_grant_command_upgrades
                            .record_replace(&template_name, &replacement);
                    }
                    return Ok(());
                }
            }
        }
        self.admit_host_radar_provider_with_effects(object_id, presentation_effects);
        // C++ RadarUpgrade::upgradeImplementation → RadarUpdate::extendRadar.
        if self
            .objects
            .get(&object_id)
            .is_some_and(|object| object.thing().template.radar_upgrade_rules.is_none())
            && crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade)
                == crate::game_logic::host_upgrades::HostUpgradeKind::Radar
        {
            self.maybe_start_radar_extend(object_id);
        }
        Ok(())
    }

    /// Read ObjectCreationUpgrade declarations from this session's exact
    /// admitted template and align the object-owned latches with their order.
    fn authored_ordered_upgrade_modules(
        &mut self,
        object_id: ObjectId,
    ) -> Vec<AuthoredOrderedUpgrade> {
        use game_engine::common::thing::module::ModuleData;
        let Some(object) = self.objects.get(&object_id) else {
            return Vec::new();
        };
        let Some(content) = self.receiving_module_content.as_ref() else {
            return Vec::new();
        };
        let Some(template) = content.admitted_template(&object.template_name) else {
            return Vec::new();
        };
        let mut object_creation_index = 0;
        let mut status_bits_index = 0;
        let modules: Vec<_> = template
            .behavior_modules()
            .iter()
            .enumerate()
            .filter_map(|(source_index, module)| {
                use game_engine::common::thing::module::ModuleData;
                if module.name.as_str().eq_ignore_ascii_case("ObjectCreationUpgrade") {
                    let data = module.data.as_any().downcast_ref::<
                        gamelogic::upgrade::modules::object_creation::ObjectCreationUpgradeModuleData,
                    >()?;
                    let state_index = object_creation_index;
                    object_creation_index += 1;
                    return Some(AuthoredOrderedUpgrade::ObjectCreation(
                        AuthoredObjectCreationUpgrade { source_index, state_index, data: data.clone() }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("StatusBitsUpgrade") {
                    let data = module.data.as_any().downcast_ref::<
                        gamelogic::object::upgrade::status_bits_upgrade::StatusBitsUpgradeModuleData,
                    >()?;
                    let state_index = status_bits_index;
                    status_bits_index += 1;
                    return Some(AuthoredOrderedUpgrade::StatusBits(
                        AuthoredStatusBitsUpgrade { source_index, state_index, data: data.clone() }
                    ));
                }
                None
            })
            .collect();
        if let Some(object) = self.objects.get_mut(&object_id) {
            object
                .object_creation_upgrade_executed
                .resize(object_creation_index, false);
            object
                .status_bits_upgrade_executed
                .resize(status_bits_index, false);
        }
        modules
    }

    fn object_creation_upgrade_update_allowed(&self, object_id: ObjectId) -> bool {
        self.objects.get(&object_id).is_some_and(|object| {
            !object.under_construction_status()
                && !object.status.destroyed
                && self.player_owner_for_host_object(object).is_some()
        })
    }

    /// Run the currently modeled typed mux families in their relative template
    /// order after capturing the combined player/object mask. This does not
    /// establish ordering against upgrade families still handled by residuals.
    fn execute_authored_ordered_upgrade_modules(
        &mut self,
        object_id: ObjectId,
        object_birth: &std::sync::Weak<()>,
        mut modules: Vec<AuthoredOrderedUpgrade>,
        key_mask: gamelogic::upgrade::UpgradeMask,
        pending_upgrade: Option<&str>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        modules.sort_by_key(AuthoredOrderedUpgrade::source_index);
        for module in modules {
            if !self.object_birth_matches(object_id, object_birth) {
                return Ok(());
            }
            let (mux, already_executed) = match &module {
                AuthoredOrderedUpgrade::ObjectCreation(module) => (
                    &module.data.upgrade_mux_data,
                    self.objects
                        .get(&object_id)
                        .and_then(|object| {
                            object
                                .object_creation_upgrade_executed
                                .get(module.state_index)
                        })
                        .copied()
                        .unwrap_or(false),
                ),
                AuthoredOrderedUpgrade::StatusBits(module) => (
                    &module.data.upgrade_mux_data,
                    self.objects
                        .get(&object_id)
                        .and_then(|object| {
                            object.status_bits_upgrade_executed.get(module.state_index)
                        })
                        .copied()
                        .unwrap_or(false),
                ),
            };
            if already_executed || !self.upgrade_mux_would_run(mux, key_mask) {
                continue;
            }
            if let Some(fx_list) = &mux.fx_list_upgrade {
                let _ = self.dispatch_fx_list_at_host_object_with_effects(
                    fx_list.name(),
                    object_id,
                    None,
                    presentation_effects,
                );
                // FX nuggets can synchronously execute gameplay callbacks.
                // Never apply this module's removals/OCL to a replacement
                // admitted under the same ObjectId.
                if !self.object_birth_matches(object_id, object_birth) {
                    return Ok(());
                }
            }
            for removed in &mux.removal_upgrade_names {
                self.remove_object_upgrade_with_receiving_rules(object_id, removed.as_str());
            }
            let result = match &module {
                AuthoredOrderedUpgrade::StatusBits(module) => {
                    let names =
                        crate::game_logic::host_enum_table_residual::OBJECT_STATUS_BIT_NAME_LIST;
                    let set: Vec<_> = names
                        .iter()
                        .enumerate()
                        .filter_map(|(i, name)| {
                            (i > 0 && module.data.status_to_set().bits() & (1u64 << i) != 0)
                                .then_some(*name)
                        })
                        .collect();
                    let clear: Vec<_> = names
                        .iter()
                        .enumerate()
                        .filter_map(|(i, name)| {
                            (i > 0 && module.data.status_to_clear().bits() & (1u64 << i) != 0)
                                .then_some(*name)
                        })
                        .collect();
                    if let Some((set_count, clear_count)) = self
                        .apply_object_status_bits_upgrade_with_effects(
                            object_id,
                            &set,
                            &clear,
                            presentation_effects,
                        )
                    {
                        self.status_bits_upgrade_reg
                            .record_apply(set_count, clear_count);
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::ObjectCreation(module) => {
                    let ocl_name = module
                        .data
                        .ocl_name
                        .as_ref()
                        .map(|name| name.as_str().trim())
                        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("None"));
                    let module_drone_kind = pending_upgrade
                        .and_then(crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name)
                        .or_else(|| module.data.upgrade_mux_data.activation_upgrade_names.iter()
                            .chain(&module.data.upgrade_mux_data.trigger_upgrade_names)
                            .find_map(|name| crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name(name.as_str())));
                    if let Some(ocl_name) = ocl_name {
                        let slave_kind = module_drone_kind.filter(|kind| {
                            ocl_name.eq_ignore_ascii_case(
                                crate::game_logic::host_slave_drones::drone_ocl_name(*kind),
                            )
                        });
                        self.apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
                            object_id,
                            ocl_name,
                            slave_kind,
                            object_birth,
                            interpreter,
                            presentation_effects,
                        )
                    } else {
                        Ok(())
                    }
                }
            };
            if self.object_birth_matches(object_id, object_birth) {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    match module {
                        AuthoredOrderedUpgrade::ObjectCreation(module) => {
                            if let Some(executed) = object
                                .object_creation_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = true;
                            }
                        }
                        AuthoredOrderedUpgrade::StatusBits(module) => {
                            if let Some(executed) = object
                                .status_bits_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = true;
                            }
                        }
                    }
                }
            }
            result?;
        }
        Ok(())
    }

    /// Re-evaluate the combined mask after construction completion. C++
    /// `updateUpgradeModules` sees object upgrades already present in the mask,
    /// even when they were granted while `UNDER_CONSTRUCTION` blocked modules.
    pub(crate) fn update_object_creation_upgrade_modules_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if !self.object_creation_upgrade_update_allowed(object_id) {
            return Ok(());
        }
        let Some(object_birth) = self
            .objects
            .get(&object_id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return Ok(());
        };
        let modules = self.authored_ordered_upgrade_modules(object_id);
        let key_mask = self.object_creation_upgrade_key_mask(object_id, None);
        self.execute_authored_ordered_upgrade_modules(
            object_id,
            &object_birth,
            modules,
            key_mask,
            None,
            interpreter,
            presentation_effects,
        )
    }

    fn object_creation_upgrade_key_mask(
        &self,
        object_id: ObjectId,
        pending_upgrade: Option<&str>,
    ) -> gamelogic::upgrade::UpgradeMask {
        let Some(object) = self.objects.get(&object_id) else {
            return gamelogic::upgrade::UpgradeMask::none();
        };
        let Some(owner_id) = self.player_owner_for_host_object(object) else {
            return gamelogic::upgrade::UpgradeMask::none();
        };
        let mut names = object
            .applied_upgrades
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if let Some(player) = self.players.get(&owner_id) {
            names.extend(player.completed_upgrades.iter().map(String::as_str));
        }
        if let Some(pending) = pending_upgrade {
            names.push(pending);
        }
        names
            .into_iter()
            .filter_map(|name| self.upgrade_template(name))
            .fold(gamelogic::upgrade::UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            })
    }

    /// Main's raw Object tag mutator has no receiving world argument. Route
    /// production removals through this owner boundary so UpgradeMux reset uses
    /// this session's definitions rather than the process UpgradeCenter.
    pub(crate) fn remove_object_upgrade_with_receiving_rules(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> bool {
        let removed_mask = self
            .upgrade_template(upgrade)
            .map(|template| template.get_mask().bits())
            .unwrap_or(0);
        let modules = self.authored_ordered_upgrade_modules(object_id);
        let activation_masks: Vec<_> = modules
            .iter()
            .map(|module| match module {
                AuthoredOrderedUpgrade::ObjectCreation(module) => {
                    self.upgrade_activation_mask(&module.data.upgrade_mux_data)
                }
                AuthoredOrderedUpgrade::StatusBits(module) => {
                    self.upgrade_activation_mask(&module.data.upgrade_mux_data)
                }
            })
            .collect();
        let mut tags_to_remove = self
            .objects
            .get(&object_id)
            .map(|object| {
                object
                    .applied_upgrades
                    .iter()
                    .filter(|name| {
                        self.upgrade_template(name.as_str())
                            .is_some_and(|template| template.get_mask().bits() & removed_mask != 0)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !tags_to_remove
            .iter()
            .any(|name| name.eq_ignore_ascii_case(upgrade))
        {
            tags_to_remove.push(upgrade.to_owned());
        }
        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let mut removed = false;
        for name in tags_to_remove {
            removed |= object.remove_upgrade_tag(&name);
        }
        if removed_mask != 0 {
            for (index, activation) in activation_masks.iter().enumerate() {
                if activation & removed_mask != 0 {
                    match modules.get(index) {
                        Some(AuthoredOrderedUpgrade::ObjectCreation(module)) => {
                            if let Some(executed) = object
                                .object_creation_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = false;
                            }
                        }
                        Some(AuthoredOrderedUpgrade::StatusBits(module)) => {
                            if let Some(executed) = object
                                .status_bits_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = false;
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        removed
    }

    fn upgrade_activation_mask(
        &self,
        data: &gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData,
    ) -> u128 {
        data.activation_upgrade_names
            .iter()
            .chain(&data.trigger_upgrade_names)
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(0u128, |mask, template| mask | template.get_mask().bits())
    }

    fn upgrade_mux_would_run(
        &self,
        data: &gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData,
        key_mask: gamelogic::upgrade::UpgradeMask,
    ) -> bool {
        use gamelogic::upgrade::UpgradeMask;
        let activation = data
            .activation_upgrade_names
            .iter()
            .chain(&data.trigger_upgrade_names)
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            });
        let conflicts = data
            .conflicting_upgrade_names
            .iter()
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            });
        activation.any()
            && key_mask.any()
            && !key_mask.test_for_any(conflicts)
            && if data.requires_all_triggers {
                key_mask.test_for_all(activation)
            } else {
                key_mask.test_for_any(activation)
            }
    }

    fn apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        ocl_name: &str,
        kind: Option<crate::game_logic::host_slave_drones::SlaveDroneKind>,
        master_birth: &std::sync::Weak<()>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let ctx = self
            .objects
            .get(&object_id)
            .filter(|obj| {
                master_birth.ptr_eq(&std::sync::Arc::downgrade(&obj.visual_draw_identity_token))
            })
            .map(|obj| {
                (
                    obj.team,
                    obj.get_position(),
                    obj.get_orientation(),
                    obj.experience.level,
                    obj.movement.velocity,
                )
            });
        let Some((team, pos, orient, vet, vel)) = ctx else {
            return Err(gamelogic::GameLogicError::InvalidObject(object_id.0));
        };
        let outcome = self.execute_parsed_weapon_ocl_with_births(
            ocl_name,
            Some(object_id),
            team,
            vet,
            orient,
            vel,
            pos,
            interpreter,
            presentation_effects,
        );
        let ocl_error = outcome.error;
        let spawned = outcome.created;
        if spawned.is_empty() {
            if let Some(error) = ocl_error {
                return Err(error);
            }
            if !self.object_birth_matches(object_id, master_birth) {
                return Err(gamelogic::GameLogicError::InvalidObject(object_id.0));
            }
            return Ok(());
        }
        let Some(kind) = kind else {
            if let Some(error) = ocl_error {
                return Err(error);
            }
            return Ok(());
        };
        if !self.object_birth_matches(object_id, master_birth) {
            let mut cleanup_error = None;
            for (drone_id, drone_birth) in spawned {
                match self.destroy_object_if_birth_matches_with_effects(
                    drone_id,
                    &drone_birth,
                    presentation_effects,
                ) {
                    Ok(_) => {}
                    Err(error) => {
                        cleanup_error.get_or_insert(error);
                    }
                }
            }
            if let Some(error) = cleanup_error {
                return Err(error);
            }
            if let Some(error) = ocl_error {
                return Err(error);
            }
            return Err(gamelogic::GameLogicError::InvalidObject(object_id.0));
        }
        // C++ SlavedUpdate startSlavedEffects + UpgradeDie on the OCL spawn.
        for (drone_id, drone_birth) in spawned {
            if !self.object_birth_matches(drone_id, &drone_birth) {
                continue;
            }
            if let Some(drone) = self.objects.get_mut(&drone_id) {
                drone.producer_id = Some(object_id);
                drone.set_status_unselectable(true);
                if drone.upgrade_die.is_none() {
                    drone.install_upgrade_die(kind.upgrade_name());
                }
            }
        }
        if let Some(error) = ocl_error {
            return Err(error);
        }
        match kind {
            crate::game_logic::host_slave_drones::SlaveDroneKind::Scout => {
                self.scout_drone_residual_attaches =
                    self.scout_drone_residual_attaches.saturating_add(1);
            }
            crate::game_logic::host_slave_drones::SlaveDroneKind::Hellfire => {
                self.hellfire_drone_residual_attaches =
                    self.hellfire_drone_residual_attaches.saturating_add(1);
            }
            crate::game_logic::host_slave_drones::SlaveDroneKind::Battle => {
                self.battle_drone_residual_attaches =
                    self.battle_drone_residual_attaches.saturating_add(1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod object_creation_upgrade_birth_tests {
    use super::*;

    const UPGRADE: &str = "Upgrade_AmericaScoutDrone";
    const SECOND_UPGRADE: &str = "Upgrade_ObjectCreationUpgradeSecondTrigger";
    const CONFLICT_UPGRADE: &str = "Upgrade_ObjectCreationUpgradeConflict";

    fn admit_upgrade(world: &mut GameLogic, name: &str) {
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(&format!("Upgrade {name}\nEnd\n"), |ini| {
            ini.read_line()?;
            world.upgrade_definitions.parse_upgrade_definition(ini)
        })
        .expect("upgrade definition should admit");
    }

    fn world(triggers: &str, ocl_name: &str, child_name: Option<&str>) -> (GameLogic, ObjectId) {
        world_with_mux(triggers, false, ocl_name, child_name)
    }

    fn world_with_mux(
        triggers: &str,
        requires_all: bool,
        ocl_name: &str,
        child_name: Option<&str>,
    ) -> (GameLogic, ObjectId) {
        world_with_mux_conflicting(triggers, requires_all, ocl_name, child_name, "")
    }

    fn world_with_mux_conflicting(
        triggers: &str,
        requires_all: bool,
        ocl_name: &str,
        child_name: Option<&str>,
        conflicts: &str,
    ) -> (GameLogic, ObjectId) {
        world_with_mux_order(
            triggers,
            requires_all,
            ocl_name,
            child_name,
            conflicts,
            true,
            "",
        )
    }

    fn world_with_mux_order(
        triggers: &str,
        requires_all: bool,
        ocl_name: &str,
        child_name: Option<&str>,
        conflicts: &str,
        status_first: bool,
        creation_removes: &str,
    ) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(crate::game_logic::Player::new(
            0,
            Team::USA,
            "ObjectCreationUpgradeOwner",
            true,
        ));
        admit_upgrade(&mut world, UPGRADE);
        admit_upgrade(&mut world, SECOND_UPGRADE);
        admit_upgrade(&mut world, CONFLICT_UPGRADE);
        let status_module = format!(
            "Behavior = StatusBitsUpgrade AuthoredStatusBitsUpgrade\n  TriggeredBy = {triggers}\n  ConflictsWith = {conflicts}\n  RequiresAllTriggers = {}\n  StatusToSet = BOOBY_TRAPPED\n End",
            if requires_all { "Yes" } else { "No" },
        );
        let creation_module = format!(
            "Behavior = ObjectCreationUpgrade AuthoredCreationUpgrade\n  TriggeredBy = {triggers}\n  ConflictsWith = {conflicts}\n  RemovesUpgrades = {creation_removes}\n  RequiresAllTriggers = {}\n  UpgradeObject = {ocl_name}\n End",
            if requires_all { "Yes" } else { "No" },
        );
        let modules = if status_first {
            format!("{status_module}\n {creation_module}")
        } else {
            format!("{creation_module}\n {status_module}")
        };
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "ObjectCreationUpgradeOwner.ini".into(),
                format!(
                    "Object AuthoredCreationUpgradeHost\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n {modules}\nEnd\n{}",
                    child_name.map_or_else(String::new, |name| {
                        format!("Object {name}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n")
                    })
                ),
            )])
            .unwrap(),
        )
        .expect("typed ObjectCreationUpgrade module should admit");
        if let Some(child_name) = child_name {
            world
                .admit_object_creation_list_sources([format!(
                    "ObjectCreationList {ocl_name}\n CreateObject\n  ObjectNames = {child_name}\n  Count = 1\n End\nEnd\n"
                )])
                .expect("receiving OCL should admit");
        }
        let source = world
            .create_object_for_player("AuthoredCreationUpgradeHost", 0, glam::Vec3::ZERO)
            .expect("upgrade source should create");
        (world, source)
    }

    fn apply(world: &mut GameLogic, source: ObjectId, upgrade: &str) {
        world
            .apply_upgrade_to_object_with_effects_checked(
                source,
                upgrade,
                &mut crate::game_logic::ScriptPresentationEffects::default(),
            )
            .expect("receiving ObjectCreationUpgrade callback should complete");
    }

    fn spawned_template_names(world: &GameLogic, source: ObjectId) -> Vec<String> {
        world
            .objects
            .registered()
            .filter(|(id, _)| **id != source)
            .map(|(_, object)| object.template_name.clone())
            .collect()
    }

    #[test]
    fn stale_ocl_child_receipt_does_not_destroy_same_id_replacement() {
        let mut world = GameLogic::new();
        let id = ObjectId(77);
        let template = ThingTemplate::new("BirthReceiptChild");
        let original = Object::new(template.clone(), id, Team::USA);
        let original_birth = std::sync::Arc::downgrade(&original.visual_draw_identity_token);
        world.objects.insert(id, original);

        let replacement = Object::new(template, id, Team::China);
        let replacement_birth = std::sync::Arc::downgrade(&replacement.visual_draw_identity_token);
        world.objects.insert(id, replacement);

        let destroyed = world
            .destroy_object_if_birth_matches_with_effects(
                id,
                &original_birth,
                &mut crate::game_logic::ScriptPresentationEffects::default(),
            )
            .expect("mismatched receipt should be a harmless no-op");
        assert!(!destroyed);
        assert!(world.object_birth_matches(id, &replacement_birth));
    }

    #[test]
    fn object_creation_upgrade_uses_each_same_id_worlds_authored_module_ocl() {
        let (mut owner, owner_source) = world(
            UPGRADE,
            "OCL_OwnerAuthoredDrone",
            Some("OwnerAuthoredDrone"),
        );
        let (mut peer, peer_source) =
            world(UPGRADE, "OCL_PeerAuthoredDrone", Some("PeerAuthoredDrone"));
        assert_eq!(
            owner_source, peer_source,
            "fixture must collide on ObjectId"
        );
        apply(&mut owner, owner_source, UPGRADE);
        apply(&mut peer, peer_source, UPGRADE);

        assert_eq!(
            spawned_template_names(&owner, owner_source),
            vec!["OwnerAuthoredDrone".to_owned()]
        );
        assert_eq!(
            spawned_template_names(&peer, peer_source),
            vec!["PeerAuthoredDrone".to_owned()]
        );
        assert!(
            owner
                .host_object(owner_source)
                .unwrap()
                .has_upgrade_tag(UPGRADE)
        );
        assert!(
            peer.host_object(peer_source)
                .unwrap()
                .has_upgrade_tag(UPGRADE)
        );
        assert!(
            owner
                .host_object(owner_source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert!(
            peer.host_object(peer_source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
    }

    #[test]
    fn missing_or_none_authored_ocl_does_not_synthesize_a_drone() {
        let (mut none, none_source) = world(UPGRADE, "None", None);
        apply(&mut none, none_source, UPGRADE);
        assert!(spawned_template_names(&none, none_source).is_empty());

        let (mut missing, missing_source) = world(UPGRADE, "OCL_NotAdmittedForThisWorld", None);
        apply(&mut missing, missing_source, UPGRADE);
        assert!(spawned_template_names(&missing, missing_source).is_empty());
    }

    #[test]
    fn object_creation_upgrade_or_triggers_execute_once_and_reset_on_trigger_removal() {
        let (mut world, source) = world(
            &format!("{UPGRADE} {SECOND_UPGRADE}"),
            "OCL_AuthoredOrTriggerChild",
            Some("AuthoredOrTriggerChild"),
        );
        apply(&mut world, source, UPGRADE);
        apply(&mut world, source, SECOND_UPGRADE);
        assert_eq!(
            spawned_template_names(&world, source),
            vec!["AuthoredOrTriggerChild".to_owned()],
            "a second OR trigger must not execute the same UpgradeMux module again"
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .object_creation_upgrade_executed,
            [true]
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .status_bits_upgrade_executed,
            [true]
        );

        assert!(world.remove_object_upgrade_with_receiving_rules(source, UPGRADE));
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .status_bits_upgrade_executed,
            [false]
        );
        apply(&mut world, source, SECOND_UPGRADE);
        assert_eq!(
            spawned_template_names(&world, source),
            vec![
                "AuthoredOrTriggerChild".to_owned(),
                "AuthoredOrTriggerChild".to_owned(),
            ],
            "C++ resetUpgrade clears the per-module latch when any activation bit is removed"
        );
    }

    #[test]
    fn both_upgrade_families_honor_conflict_masks_before_latching() {
        let (mut world, source) = world_with_mux_conflicting(
            UPGRADE,
            false,
            "OCL_AuthoredConflictChild",
            Some("AuthoredConflictChild"),
            CONFLICT_UPGRADE,
        );
        apply(&mut world, source, CONFLICT_UPGRADE);
        apply(&mut world, source, UPGRADE);
        assert!(spawned_template_names(&world, source).is_empty());
        assert!(
            !world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .object_creation_upgrade_executed,
            [false]
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .status_bits_upgrade_executed,
            [false]
        );

        assert!(world.remove_object_upgrade_with_receiving_rules(source, CONFLICT_UPGRADE));
        apply(&mut world, source, UPGRADE);
        assert_eq!(
            spawned_template_names(&world, source),
            vec!["AuthoredConflictChild"]
        );
        assert!(
            world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .object_creation_upgrade_executed,
            [true]
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .status_bits_upgrade_executed,
            [true]
        );
    }

    #[test]
    fn typed_upgrade_dispatch_accepts_either_authored_module_order() {
        for status_first in [true, false] {
            let (mut world, source) = world_with_mux_order(
                UPGRADE,
                false,
                "OCL_AuthoredOrderChild",
                Some("AuthoredOrderChild"),
                "",
                status_first,
                UPGRADE,
            );
            apply(&mut world, source, UPGRADE);
            assert!(
                world
                    .host_object(source)
                    .unwrap()
                    .has_object_status_bit("BOOBY_TRAPPED")
            );
            assert_eq!(
                spawned_template_names(&world, source),
                vec!["AuthoredOrderChild"]
            );
            assert_eq!(
                world
                    .host_object(source)
                    .unwrap()
                    .object_creation_upgrade_executed,
                [true]
            );
            assert_eq!(
                world
                    .host_object(source)
                    .unwrap()
                    .status_bits_upgrade_executed,
                [!status_first],
                "the later ObjectCreationUpgrade removal must reset an earlier StatusBitsUpgrade latch"
            );
        }
    }

    #[test]
    fn object_creation_upgrade_combines_receiving_player_and_object_masks() {
        let (mut world, source) = world_with_mux(
            &format!("{UPGRADE} {SECOND_UPGRADE}"),
            true,
            "OCL_AuthoredCombinedMaskChild",
            Some("AuthoredCombinedMaskChild"),
        );
        world
            .players
            .get_mut(&0)
            .unwrap()
            .add_completed_upgrade(UPGRADE);
        apply(&mut world, source, UPGRADE);
        assert!(
            !world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert!(
            spawned_template_names(&world, source).is_empty(),
            "RequiresAllTriggers must wait while only the player-side A mask is set"
        );
        apply(&mut world, source, SECOND_UPGRADE);
        assert!(
            world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert_eq!(
            spawned_template_names(&world, source),
            vec!["AuthoredCombinedMaskChild".to_owned()],
            "RequiresAllTriggers must combine the receiving player's A bit with this object's B bit"
        );
    }

    #[test]
    fn construction_gate_keeps_upgrade_tag_and_replays_module_after_completion() {
        let (mut world, source) = world(
            UPGRADE,
            "OCL_ConstructedCreationUpgradeChild",
            Some("ConstructedCreationUpgradeChild"),
        );
        world
            .host_object_mut(source)
            .unwrap()
            .status
            .under_construction = true;

        apply(&mut world, source, UPGRADE);
        assert!(world.host_object(source).unwrap().has_upgrade_tag(UPGRADE));
        assert!(
            !world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED"),
            "C++ updateUpgradeModules gates StatusBitsUpgrade during construction"
        );
        assert!(
            spawned_template_names(&world, source).is_empty(),
            "C++ updateUpgradeModules skips module callbacks during construction"
        );

        world
            .host_object_mut(source)
            .unwrap()
            .status
            .under_construction = false;
        world.apply_researched_player_upgrades_to_object_with_interpreter(
            source,
            None,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        );
        assert!(
            world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED"),
            "construction completion replays the typed StatusBitsUpgrade module"
        );
        assert_eq!(
            spawned_template_names(&world, source),
            vec!["ConstructedCreationUpgradeChild".to_owned()],
            "the construction-complete update must re-evaluate the existing object upgrade mask"
        );
    }
}
