//! C++ ObjectCreationUpgrade / RadarUpgrade / per-object upgrade callback order.
//! Extracted from object_queries; one receiving implementation per callback.
use super::super::*;

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
        let mut attach_slave_drone = None;

        let Some((identity, status_upgrade_first_give)) =
            self.objects.get(&object_id).map(|object| {
                (
                    self.live_draw_playback_identity(object),
                    !object.has_upgrade_tag(upgrade),
                )
            })
        else {
            return Ok(());
        };
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ UpgradeMux::isAlreadyUpgraded — first give only.
            // C++ wouldUpgrade ConflictsWith: leftover ObjectCreationUpgrade already
            // matches; host objects skip OCL when another drone tag is complete.
            let first_give = status_upgrade_first_give;
            let drone_conflicts =
                crate::game_logic::host_slave_drones::slave_drone_conflicts_with_owned(
                    upgrade,
                    obj.applied_upgrades.iter().map(String::as_str),
                );
            obj.apply_upgrade_tag(upgrade);
            if first_give {
                if !drone_conflicts {
                    attach_slave_drone =
                        crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name(
                            upgrade,
                        );
                }
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
        let status_pairs = self
            .objects
            .get(&object_id)
            .filter(|_| status_upgrade_first_give)
            .map(|object| {
                crate::game_logic::host_status_bits_upgrade::collect_status_bits_for_object(
                    upgrade, object,
                )
            })
            .unwrap_or_default();
        for (set, clear) in status_pairs {
            let set: Vec<_> = set.iter().map(String::as_str).collect();
            let clear: Vec<_> = clear.iter().map(String::as_str).collect();
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
        // C++ ObjectCreationUpgrade::upgradeImplementation — OCL on this vehicle
        // (Battle/Scout/Hellfire). Leftover module already matches C++; host
        // objects have no leftover modules, so spawn via leftover OCL store
        // (or residual attach). Object-scoped complete muxes only the producer.
        if let Some(kind) = attach_slave_drone {
            self.apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
                object_id,
                kind,
                interpreter,
                presentation_effects,
            )?;
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

    /// C++ `ObjectCreationUpgrade::upgradeImplementation` on this vehicle.
    /// Leftover OCL store first; residual attach if the list is not loaded.
    fn apply_object_creation_upgrade_ocl(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_object_creation_upgrade_ocl_with_effects(
            object_id,
            kind,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn apply_object_creation_upgrade_ocl_with_effects(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
            object_id,
            kind,
            None,
            presentation_effects,
        )
    }
    fn apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_slave_drones::{SlaveDroneKind, drone_ocl_name};

        let ctx = self.objects.get(&object_id).map(|obj| {
            (
                obj.team,
                obj.get_position(),
                obj.get_orientation(),
                obj.experience.level,
                obj.movement.velocity,
                std::sync::Arc::downgrade(&obj.visual_draw_identity_token),
            )
        });
        let Some((team, pos, orient, vet, vel, master_birth)) = ctx else {
            return Ok(());
        };
        let outcome = self.execute_parsed_weapon_ocl_with_births(
            drone_ocl_name(kind),
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
        let original_master_survives = self.objects.get(&object_id).is_some_and(|master| {
            master_birth.ptr_eq(&std::sync::Arc::downgrade(
                &master.visual_draw_identity_token,
            ))
        });
        if spawned.is_empty() {
            if let Some(error) = ocl_error {
                return Err(error);
            }
            if !original_master_survives {
                return Err(gamelogic::GameLogicError::InvalidObject(object_id.0));
            }
            let _ = self.residual_attach_slave_drone_checked(
                object_id,
                kind,
                interpreter,
                presentation_effects,
            )?;
            return Ok(());
        }
        if !original_master_survives {
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
            SlaveDroneKind::Scout => {
                self.scout_drone_residual_attaches =
                    self.scout_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Hellfire => {
                self.hellfire_drone_residual_attaches =
                    self.hellfire_drone_residual_attaches.saturating_add(1);
            }
            SlaveDroneKind::Battle => {
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
}
