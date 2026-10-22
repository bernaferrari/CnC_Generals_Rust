//! One installed-object authored Die traversal. Each world callback ends its
//! bounded Object borrow before the next slot resolves identities again.
use super::*;
use crate::game_logic::native_die_definition::{NativeDieKind, NativeSlowFamily};
use crate::game_logic::object::OnDieInput;
#[cfg(test)]
#[path = "authored_credit_tests.rs"]
mod credit_tests;

impl GameLogic {
    pub(super) fn run_authored_die_modules(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: OnDieInput,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.run_authored_die_modules_with_effects(
            id,
            killer,
            input,
            interpreter,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn run_authored_die_modules_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: OnDieInput,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Immutable receiving declarations, operation-local owned loan. There
        // is no per-object second order or active-world content discovery.
        let declarations = self
            .objects
            .get(&id)
            .and_then(|object| object.thing().template.native_die_declarations.clone())
            .unwrap_or_default();
        for declaration in declarations.iter() {
            let Some(object) = self.objects.get(&id) else {
                break;
            };
            if object.native_die_runtime.pending_create == Some(declaration.source_index) {
                self.apply_pending_create_object_die_with_effects(id, presentation_effects);
                continue;
            }
            let jet_ground = matches!(
                &declaration.kind,
                NativeDieKind::Slow {
                    family: NativeSlowFamily::Jet(_),
                    ..
                }
            ) && (!object.is_significantly_above_terrain()
                || object.status.deck_height_offset);
            // Jet's ground override does not enter SlowDeath's applicability
            // or AI guard; its own ground effects precede destruction.
            if !jet_ground && !declaration.gate.allows(object, input.death_type) {
                continue;
            }
            if object
                .native_die_runtime
                .completed
                .contains(&declaration.source_index)
            {
                continue;
            }
            if let Some(object) = self.objects.get_mut(&id) {
                object
                    .native_die_runtime
                    .completed
                    .insert(declaration.source_index);
            }
            match &declaration.kind {
                NativeDieKind::Fx(fx) => self.run_authored_fx_die_with_effects(
                    id,
                    input.source,
                    &mut fx.configuration(),
                    presentation_effects,
                ),
                NativeDieKind::Create { config } => {
                    if let Some(config) = config {
                        let Some(object) = self.objects.get(&id) else {
                            continue;
                        };
                        // TransferPreviousHealth intentionally uses prior Body
                        // history, unlike the current OCL damage-dealer input.
                        let maximum = object.health.maximum.max(object.max_health).max(1.0);
                        let previous = object.previous_health;
                        self.execute_create_object_die_spawns_with_effects(
                            id,
                            config.spawn_templates.clone(),
                            (maximum - previous).max(0.0),
                            config.transfer_previous_health,
                            object.subdual_damage.max(0.0),
                            object.last_damage_source,
                            object.team,
                            object.owner_player_id,
                            object.get_position(),
                            presentation_effects,
                        );
                    }
                }
                NativeDieKind::Crush => {
                    let crusher = input
                        .source
                        .and_then(|source| self.objects.get(&source))
                        .map(|source| {
                            let p = source.get_position();
                            (p.x, p.z)
                        });
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.fire_crush_die_from_input(input.damage_type, crusher);
                    }
                }
                // KeepObjectDie::onDie is an applicability-only no-op. It
                // neither requests deletion nor prevents a later DestroyDie.
                NativeDieKind::Keep => {}
                // MinefieldBehavior::onDie requests direct deletion only;
                // its damage callback already performed any virtual blasts.
                NativeDieKind::Minefield => {
                    self.destroy_object_with_effects(id, presentation_effects);
                }
                NativeDieKind::Destroy => {
                    self.request_object_destruction_with_effects(id, killer, presentation_effects);
                }
                NativeDieKind::Instant {
                    config: ini,
                    creations,
                } => {
                    let Some(object) = self.objects.get(&id) else {
                        continue;
                    };
                    if ini.required_under_construction && !object.status.under_construction {
                        continue;
                    }
                    if !self.admit_authored_death_ai(id) {
                        continue;
                    }
                    // Select immediately before each effect: an earlier
                    // callback may consume the same session random stream.
                    if let Some(fx) = self.choose_authored_die_entry(&ini.fx) {
                        self.run_authored_named_die_fx_with_effects(
                            id,
                            None,
                            &fx,
                            true,
                            presentation_effects,
                        );
                    }
                    if let Some(ocl) = self.choose_authored_die_entry(&ini.ocls) {
                        if let Some(config) = ini
                            .ocls
                            .iter()
                            .position(|name| name == &ocl)
                            .and_then(|index| creations.get(index))
                            .and_then(Option::as_ref)
                        {
                            if let Some(object) = self.objects.get(&id) {
                                self.execute_create_object_die_spawns_with_effects(
                                    id,
                                    config.spawn_templates.clone(),
                                    0.0,
                                    false,
                                    0.0,
                                    None,
                                    object.team,
                                    object.owner_player_id,
                                    object.get_position(),
                                    presentation_effects,
                                );
                            }
                        }
                    }
                    if let Some(weapon) = self.choose_authored_die_entry(&ini.weapons) {
                        let spec = crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec {
                            module_source_index: declaration.source_index as u32,
                            weapon_template_name: weapon,
                            weapon_slot: crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponSlot::Primary,
                        };
                        let _ = self.create_and_fire_temp_weapon_with_effects(
                            id,
                            &spec,
                            presentation_effects,
                        );
                    }
                    self.request_object_destruction_with_effects(id, killer, presentation_effects);
                }
                NativeDieKind::Slow { .. } => {
                    if jet_ground {
                        let deletion_queued =
                            self.objects_to_destroy.iter().any(|event| event.id == id);
                        if let Some(object) = self.objects.get_mut(&id) {
                            if let NativeDieKind::Slow {
                                family: NativeSlowFamily::Jet(fx),
                                ..
                            } = &declaration.kind
                            {
                                let _ = object.begin_jet_slow_death_with_fx(
                                    fx.clone(),
                                    true,
                                    deletion_queued,
                                );
                            }
                        }
                        let fx = self
                            .objects
                            .get_mut(&id)
                            .and_then(|object| object.pending_death_fx.take());
                        if let Some(fx) = fx {
                            self.run_authored_named_die_fx_with_effects(
                                id,
                                None,
                                &fx,
                                true,
                                presentation_effects,
                            );
                        }
                        self.apply_pending_create_object_die_with_effects(id, presentation_effects);
                        self.request_object_destruction_with_effects(
                            id,
                            killer,
                            presentation_effects,
                        );
                        continue;
                    }
                    if !self.admit_authored_death_ai(id) {
                        continue;
                    }
                    let candidates: Vec<_> = declarations
                        .iter()
                        .filter_map(|candidate| {
                            let object = self.objects.get(&id)?;
                            if !candidate.gate.allows(object, input.death_type) {
                                return None;
                            }
                            match &candidate.kind {
                                NativeDieKind::Slow {
                                    family,
                                    config,
                                    probability,
                                } => Some((family.clone(), config, (*probability).max(1) as u32)),
                                _ => None,
                            }
                        })
                        .collect();
                    let total: u32 = candidates
                        .iter()
                        .map(|(_, _, probability)| *probability)
                        .sum();
                    if total > 0 {
                        let mut roll = self.logic_random.next_u32() % total;
                        for (family, config, probability) in candidates {
                            if roll < probability {
                                let deletion_queued =
                                    self.objects_to_destroy.iter().any(|event| event.id == id);
                                if let Some(object) = self.objects.get_mut(&id) {
                                    match family {
                                        NativeSlowFamily::Jet(fx) => {
                                            // Weighted dispatch calls beginSlowDeath,
                                            // not Jet's onDie ground override.
                                            object.begin_jet_slow_death_with_fx(
                                                fx,
                                                false,
                                                deletion_queued,
                                            );
                                        }
                                        NativeSlowFamily::Helicopter(fx) => {
                                            object.begin_helicopter_slow_death_with_fx(
                                                fx,
                                                deletion_queued,
                                            );
                                        }
                                        NativeSlowFamily::Ordinary => {
                                            object.begin_slow_death_from_ini_with_removal(
                                                self.frame,
                                                config,
                                                deletion_queued,
                                            );
                                        }
                                    };
                                }
                                // INITIAL phase is observed at this exact
                                // authored slot before later Die modules.
                                let fx = self
                                    .objects
                                    .get_mut(&id)
                                    .and_then(|object| object.pending_death_fx.take());
                                if let Some(fx) = fx {
                                    self.run_authored_named_die_fx_with_effects(
                                        id,
                                        None,
                                        &fx,
                                        true,
                                        presentation_effects,
                                    );
                                }
                                break;
                            }
                            roll -= probability;
                        }
                    }
                }
                NativeDieKind::Topple | NativeDieKind::Collapse => {
                    if let Some(object) = self.objects.get_mut(&id) {
                        if object.has_ai_update_interface() {
                            object.native_die_runtime.ai_dead = true;
                        }
                    }
                    self.try_begin_structure_die_with_override_with_effects(
                        id,
                        input.source,
                        interpreter,
                        Some(matches!(&declaration.kind, NativeDieKind::Collapse)),
                        presentation_effects,
                    );
                }
                NativeDieKind::DeadWeapon => {
                    let _ = self.execute_temporary_weapon_die_slot_with_effects(
                        id,
                        declaration.source_index as usize,
                        presentation_effects,
                    );
                }
                NativeDieKind::Production => {
                    self.cancel_all_production(id);
                }
                NativeDieKind::Completion => self.maybe_notify_special_power_completion(id),
                NativeDieKind::Dam => self.maybe_apply_dam_die(id),
                NativeDieKind::Pilot => {
                    self.maybe_apply_eject_pilot_die_with_effects(id, presentation_effects)
                }
                NativeDieKind::Spawn => {
                    self.apply_spawned_require_spawner_on_die_with_effects(id, presentation_effects)
                }
                NativeDieKind::Upgrade => self.maybe_apply_upgrade_die(id),
                NativeDieKind::Rebuild => {
                    let _ = self.maybe_spawn_rebuild_hole_with_effects(id, presentation_effects);
                }
                NativeDieKind::Contain { overlord } => {
                    if *overlord {
                        let redirected = self.redirected_containment_owner(id);
                        if redirected != id {
                            if let Some(object) = self.objects.get_mut(&id) {
                                object.overlord_redirection_active = false;
                            }
                            self.apply_owned_kill_with_effects(
                                redirected,
                                crate::game_logic::combat::DamageType::Unresistable,
                                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                                presentation_effects,
                            );
                            self.remove_direct_contained_child_with_effects(
                                id,
                                redirected,
                                presentation_effects,
                            );
                        }
                    }
                    self.complete_installed_containment_death_with_effects(
                        id,
                        killer,
                        presentation_effects,
                    );
                }
                NativeDieKind::Parking => {
                    let _ = self.kill_all_parked_units_with_effects(id, presentation_effects);
                }
                NativeDieKind::Bridge => {
                    let deletion_queued =
                        self.objects_to_destroy.iter().any(|event| event.id == id);
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.convert_bridge_to_rubble_husk_with_removal(
                            self.frame,
                            deletion_queued,
                        );
                    }
                    self.apply_owned_bridge_death_links_with_effects(id, presentation_effects);
                }
            }
        }
        // Object-owned postlude remains distinct from authored module effects.
        self.admit_object_drawable_deselection(id);
        self.assault_transport_give_final_orders(id);
        self.drop_jet_targeters_on_attack_exit(id);
        self.stop_move_loop_sound(id);
        self.stop_ambient_sound(id);
        self.cancel_dock_reservation(id);
        if let Some(object) = self.objects.get_mut(&id) {
            object.fire_weapon_when_dead_fired = true;
            object.unstamp_partition_value_threat();
            if let Some(bone_fx) = object.bone_fx_damage.as_mut() {
                bone_fx.stop_all_bone_fx();
            }
        }
        // Only explicit authored deletion owns removal. Keep/Slow alone do
        // not synthesize DestroyDie. Empty effective content is retained too.
    }

    fn choose_authored_die_entry(&self, names: &[String]) -> Option<String> {
        if names.is_empty() {
            None
        } else {
            Some(names[self.logic_random.next_u32() as usize % names.len()].clone())
        }
    }

    fn admit_authored_death_ai(&mut self, id: ObjectId) -> bool {
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        if object.has_ai_update_interface() {
            if object.native_die_runtime.ai_dead {
                return false;
            }
            object.native_die_runtime.ai_dead = true;
            object.set_ai_state(AIState::Idle);
            object.target = None;
        }
        true
    }

    fn run_authored_fx_die(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
        config: &mut crate::game_logic::host_fx_list_die::HostFxListDieData,
    ) {
        self.run_authored_fx_die_with_effects(
            id,
            source,
            config,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn run_authored_fx_die_with_effects(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
        config: &mut crate::game_logic::host_fx_list_die::HostFxListDieData,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        #[cfg(feature = "game_client")]
        {
            let player = self.local_player_id();
            let secondary = source
                .and_then(|source| self.objects.get(&source))
                .map(|object| {
                    Self::observe_host_fx_object(object, player, self.world_services.shroud())
                });
            Self::with_host_fx_view_parts_and_terrain(
                &mut self.session_view,
                player,
                self.world_services.shroud(),
                presentation_effects.terrain.as_mut().map(|terrain| {
                    &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects
                }),
                |view| {
                    if let Some(object) = self.objects.get_mut(&id) {
                        object.fire_fx_list_die_config_with_fx(
                            config,
                            &mut |name, object, at_object| {
                                if at_object {
                                    let primary = Self::observe_host_fx_object(
                                        object,
                                        player,
                                        self.world_services.shroud(),
                                    );
                                    Self::dispatch_fx_list_at_object_with_view(
                                        name,
                                        &primary,
                                        secondary.as_ref(),
                                        view,
                                    )
                                } else {
                                    Self::dispatch_fx_list_at_position_with_view(
                                        name,
                                        object.get_position(),
                                        None,
                                        0.0,
                                        0.0,
                                        None,
                                        view,
                                    )
                                }
                            },
                        );
                    }
                },
            );
        }
        #[cfg(not(feature = "game_client"))]
        if let Some(object) = self.objects.get_mut(&id) {
            object.fire_fx_list_die_config_with_fx(config, &mut |name, object, at_object| {
                if at_object {
                    crate::game_logic::dispatch_fx_list_at_object(
                        name,
                        object.id.0,
                        source.map(|source| source.0),
                    )
                } else {
                    crate::game_logic::dispatch_fx_list_at_pos(name, object.get_position())
                }
            });
        }
    }

    fn run_authored_named_die_fx(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
        name: &str,
        at_object: bool,
    ) {
        self.run_authored_named_die_fx_with_effects(
            id,
            source,
            name,
            at_object,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn run_authored_named_die_fx_with_effects(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
        name: &str,
        at_object: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let mut config = crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx(name);
        config.orient_to_object = at_object;
        self.run_authored_fx_die_with_effects(id, source, &mut config, presentation_effects);
    }
}
