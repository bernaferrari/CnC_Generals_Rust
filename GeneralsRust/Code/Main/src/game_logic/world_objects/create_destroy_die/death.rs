//! Main GameLogic death dispatch, retained death phases, and destruction requests.
//! C++ Object/Die modules: preserve their synchronous ordering on the driving owner.
use super::*;
#[path = "death/authored.rs"]
mod authored;

#[cfg(all(test, feature = "game_client"))]
#[path = "structure_topple_fx_tests.rs"]
mod structure_topple_fx_tests;
#[path = "structure_topple_owned.rs"]
mod structure_topple_owned;

impl GameLogic {
    /// Damage carried onto debris/replacement objects when a dying object's
    /// death transfers combat damage. Shared by the debris and respawn paths.
    fn apply_death_transfer_damage(
        &mut self,
        id: &ObjectId,
        subdual: f32,
        transfer_dmg: f32,
        source: Option<ObjectId>,
    ) {
        self.apply_death_transfer_damage_with_effects(
            id,
            subdual,
            transfer_dmg,
            source,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn apply_death_transfer_damage_with_effects(
        &mut self,
        id: &ObjectId,
        subdual: f32,
        transfer_dmg: f32,
        source: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(receiving_damage_id) = self.objects.get(id).map(|object| object.id) else {
            return;
        };
        if subdual > 0.0 {
            let _ = self.apply_residual_damage_owned_with_effects(
                receiving_damage_id,
                subdual,
                None,
                crate::game_logic::combat::DamageType::SubdualUnresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                    crate::game_logic::combat::DamageType::SubdualUnresistable,
                ),
                None,
                presentation_effects,
            );
        }
        if transfer_dmg > 0.0 {
            let _ = self.apply_residual_damage_owned_with_effects(
                receiving_damage_id,
                transfer_dmg,
                source,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                    crate::game_logic::combat::DamageType::Unresistable,
                ),
                None,
                presentation_effects,
            );
        }
    }

    /// C++ FireWeaponWhenDeadBehavior::onDie leftover — death weapon splash.
    /// Specialized leftovers (bomb truck / toxin / nuke / demo) own exclusive modules.
    pub(in super::super::super) fn apply_fire_weapon_when_dead(&mut self, dying_id: ObjectId) {
        self.apply_fire_weapon_when_dead_with_effects(
            dying_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn apply_fire_weapon_when_dead_with_effects(
        &mut self,
        dying_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_demo_suicide_bomb::{
            has_demo_suicide_bomb_upgrade, is_demo_suicide_bomb_eligible_template,
        };
        use crate::game_logic::host_fire_weapon_when_dead::{
            death_weapon_for_dying_object, splash_damage_at_distance,
        };

        let Some(obj) = self.objects.get(&dying_id) else {
            return;
        };
        if obj.fire_weapon_when_dead_fired {
            return;
        }
        if obj.status.under_construction {
            return;
        }
        if is_demo_suicide_bomb_eligible_template(&obj.template_name)
            && has_demo_suicide_bomb_upgrade(&obj.applied_upgrades)
        {
            return;
        }
        let Some(splash) = death_weapon_for_dying_object(&obj.template_name, obj.status.death_type)
        else {
            return;
        };
        let pos = obj.get_position();
        let team = obj.team;
        let max_r = splash.primary_radius.max(splash.secondary_radius);

        let is_helix_napalm_bomb = obj.helix_napalm_bomb_projectile;
        let napalm_source = obj.producer_id;
        let black_napalm_bomb = obj.template_name.to_ascii_lowercase().contains("black");

        // Mark fired
        if let Some(obj) = self.objects.get_mut(&dying_id) {
            obj.fire_weapon_when_dead_fired = true;
        }

        let victims: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if *id == dying_id || !o.is_alive() {
                    return None;
                }
                let p = o.get_position();
                let dx = p.x - pos.x;
                let dz = p.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= max_r { Some(*id) } else { None }
            })
            .collect();

        let mut destroy_ids = Vec::new();
        for vid in victims {
            let Some(v) = self.objects.get_mut(&vid) else {
                continue;
            };
            let p = v.get_position();
            let dx = p.x - pos.x;
            let dz = p.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let dmg = splash_damage_at_distance(&splash, dist);
            if dmg <= 0.0 {
                continue;
            }
            let receiving_damage_target_id = v.id;
            let destroyed = self.apply_residual_damage_owned_with_effects(
                receiving_damage_target_id,
                dmg,
                Some(dying_id),
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                    crate::game_logic::combat::DamageType::Unresistable,
                ),
                None,
                presentation_effects,
            );
            if destroyed {
                destroy_ids.push(vid);
            }
        }
        // Presentation residual: death explosion particle at epicenter.
        let _ = self.combat_particles.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
            pos,
            self.frame,
            Some(dying_id),
            None,
        );
        if is_helix_napalm_bomb {
            // Honesty: HeightDie detonation residual counted as blast path.
            self.helix_napalm.blast_hits = self
                .helix_napalm
                .blast_hits
                .saturating_add(destroy_ids.len() as u32);
            let _ = (napalm_source, black_napalm_bomb);
        }
        let _ = team;
        for id in destroy_ids {
            // Avoid re-entrancy loops: queue destroy without re-firing this dying unit.
            if id != dying_id {
                self.queue_after_death_completion(id, Some(team));
            }
        }
    }

    /// Wave 752: lethal finish that respects damage-authority HP last-write.
    /// Prefer this over direct host HP zeroing for production destroy residual.
    #[allow(dead_code)]
    pub(crate) fn host_lethal_finish_object(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
    ) -> bool {
        let Some(o) = self.objects.get_mut(&id) else {
            return false;
        };
        {
            o.internal_change_health(-o.health.current);
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
        true
    }

    /// Return the subset of a selected EjectPilot OCL that this live host can
    /// reproduce without substituting a name-shaped effect.
    ///
    /// `EjectPilotDie` owns only the typed ground/air OCL selection.  In
    /// particular, its own `InvulnerableTime` field is not consumed by the
    /// C++ `onDie`; `GenericObjectCreationNugget::InvulnerableTime` on the
    /// selected OCL is the source of the spawned pilot's protection.  Keep
    /// that distinction here so a module default of zero never becomes a
    /// fabricated 2000 ms grant.
    fn parsed_eject_pilot_ocl_plan(
        creation_list: crate::game_logic::EjectPilotCreationList,
    ) -> Option<(bool, u32)> {
        use crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;
        use gamelogic::object_creation_list::{
            DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget,
        };

        // These are the only two OCL identities the typed parser admits.  The
        // enum is deliberately not a free-form INI name, so this lookup cannot
        // make an arbitrary creation list act like EjectPilotDie.
        let (ocl_name, parachute_ocl, expected_container, min_force, max_force) =
            match creation_list {
                crate::game_logic::EjectPilotCreationList::OnGround => {
                    ("OCL_EjectPilotOnGround", false, "", 2.0, 3.0)
                }
                crate::game_logic::EjectPilotCreationList::ViaParachute => (
                    "OCL_EjectPilotViaParachute",
                    true,
                    "AmericaParachute",
                    10.0,
                    12.0,
                ),
            };
        let ocl =
            gamelogic::helpers::TheObjectCreationListStore::lookup_object_creation_list(ocl_name)?;
        let [nugget] = ocl.nuggets() else {
            return None;
        };
        let generic = nugget
            .as_any()
            .downcast_ref::<GenericObjectCreationNugget>()?;

        // The existing ejection/parachute host represents precisely the two
        // retail OCL shapes below.  Refuse a changed/mixed OCL rather than
        // silently issuing only its familiar-looking pilot portion.
        let supported_shape = generic.name_are_objects
            && generic.debris_to_generate == 1
            && generic.names.len() == 1
            && generic.names[0].eq_ignore_ascii_case(EJECT_PILOT_TEMPLATE)
            && generic.ignore_primary_obstacle
            && generic.inherit_veterancy
            && generic.disposition == DebrisDisposition::new(DebrisDisposition::RANDOM_FORCE)
            && generic.min_mag == min_force
            && generic.max_mag == max_force
            // `parse_angle_real` stores the authored degree literals in the
            // engine's radians representation.
            && generic.min_pitch == 50.0_f32.to_radians()
            && generic.max_pitch == 60.0_f32.to_radians()
            && generic.spin_rate == 0.0
            && generic.requires_live_player
            && generic
                .put_in_container
                .eq_ignore_ascii_case(expected_container)
            && !generic.contain_inside_source_object
            && !generic.skip_if_significantly_airborne
            && !generic.dies_on_bad_land
            && !generic.spread_formation
            && !generic.fade_in
            && !generic.fade_out;
        supported_shape.then_some((parachute_ocl, generic.invulnerable_time))
    }

    /// Wave 754: C++ EjectPilotDie::onDie residual at death start (mark_object),
    /// not only final process_destroy remove. SlowDeath defers remove and must
    /// not suppress pilot spawn / honesty residual.
    pub(crate) fn maybe_apply_eject_pilot_die(&mut self, id: ObjectId) {
        self.maybe_apply_eject_pilot_die_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn maybe_apply_eject_pilot_die_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_usa_pilot::{
            EJECT_PILOT_TEMPLATE, HostDeathType, PILOT_EJECT_AUDIO, PILOT_SOUND_EJECT_AUDIO,
            air_eject_spawn_height, is_significantly_above_terrain,
        };

        let (
            metadata,
            pilot_team,
            pilot_owner_player_id,
            death_pos,
            veterancy,
            death_type,
            is_hijacked,
            dying_template,
        ) = {
            let Some(obj) = self.objects.get(&id) else {
                return;
            };
            if obj.eject_pilot_die_applied {
                return;
            }
            let Some(metadata) = obj.thing().template.eject_pilot_die else {
                // Module presence, not an object basename, is the C++ die
                // authority.  A name-shaped vehicle with no parsed module is
                // intentionally inert here.
                return;
            };
            (
                metadata,
                obj.team,
                obj.owner_player_id,
                obj.get_position(),
                obj.experience.level,
                obj.status.death_type,
                obj.status.hijacked,
                obj.thing().template.name.clone(),
            )
        };

        // C++ invokes a DieModule once per death.  The host may visit this
        // object again while SlowDeath unwinds, so record the attempt before
        // any supported filter/OCL can decline it.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.eject_pilot_die_applied = true;
        }

        let death_is_crushed_or_splatted =
            matches!(death_type, HostDeathType::Crushed | HostDeathType::Splatted);
        let veterancy_is_regular = matches!(veterancy, VeterancyLevel::Rookie);
        let death_types_gate = match metadata.death_types {
            EjectPilotDeathTypes::All => true,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted => !death_is_crushed_or_splatted,
            EjectPilotDeathTypes::Unsupported => false,
        };
        let veterancy_gate = match metadata.veterancy_levels {
            EjectPilotVeterancyLevels::All => true,
            EjectPilotVeterancyLevels::AllExceptRegular => !veterancy_is_regular,
            EjectPilotVeterancyLevels::Unsupported => false,
        };
        let exempt_status_gate = match metadata.exempt_status {
            EjectPilotExemptStatus::None => true,
            EjectPilotExemptStatus::Hijacked => !is_hijacked,
            EjectPilotExemptStatus::Unsupported => false,
        };

        // Preserve the existing observability counters, but now only when the
        // corresponding parsed DieMux clause actually owns the block.
        if matches!(
            metadata.veterancy_levels,
            EjectPilotVeterancyLevels::AllExceptRegular
        ) && death_types_gate
            && exempt_status_gate
            && !veterancy_gate
        {
            self.usa_pilot.record_eject_veterancy_block();
        }
        if matches!(
            metadata.death_types,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted
        ) && veterancy_gate
            && exempt_status_gate
            && !death_types_gate
        {
            self.usa_pilot.record_eject_death_type_block();
        }
        if matches!(metadata.exempt_status, EjectPilotExemptStatus::Hijacked)
            && veterancy_gate
            && death_types_gate
            && !exempt_status_gate
        {
            self.usa_pilot.record_eject_hijacked_block();
        }

        if !metadata.allows_supported_death(
            death_is_crushed_or_splatted,
            veterancy_is_regular,
            is_hijacked,
        ) {
            return;
        }

        // C++ `EjectPilotDie::onDie` selects the OCL solely from
        // `Object::isSignificantlyAboveTerrain()`.  `airborne_target` is not
        // an alternate authorization route.
        let terrain_y = self.terrain_height_at(death_pos).unwrap_or(0.0);
        let significantly_above_terrain = is_significantly_above_terrain(death_pos.y - terrain_y);
        let Some(creation_list) = metadata.creation_list_for_air_path(significantly_above_terrain)
        else {
            // A null/unsupported selected OCL is C++'s no-op `ejectPilot`.
            return;
        };
        let Some((parachute_ocl, invulnerable_frames)) =
            Self::parsed_eject_pilot_ocl_plan(creation_list)
        else {
            return;
        };

        // `RequiresLivePlayer = Yes` is part of both retail OCLs.  C++ rejects
        // a missing source controller as well as a defeated one; do not let a
        // team-only fallback create a useful pilot for an ownerless wreck.
        let Some(pilot_owner_player_id) = pilot_owner_player_id.filter(|player_id| {
            self.players
                .get(player_id)
                .is_some_and(|player| player.is_alive && player.team == pilot_team)
        }) else {
            return;
        };
        if !self.templates.contains_key(EJECT_PILOT_TEMPLATE) {
            // OCL_EjectPilot* names this exact retail object.  Do not inject
            // a synthetic pilot when the authored template cannot be loaded:
            // missing Object INI data must not turn a source name into a live
            // ejection action.
            let Some(pilot_tpl) = Self::build_template_from_asset_definition(EJECT_PILOT_TEMPLATE)
            else {
                return;
            };
            self.templates
                .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
        }
        // Offset slightly so pilot is not buried under death debris residual.
        // The chosen OCL (not vehicle kind/name) controls whether the live
        // host applies the existing AmericaParachute residual.
        let spawn_pos = if parachute_ocl {
            glam::Vec3::new(
                death_pos.x + 2.0,
                air_eject_spawn_height(death_pos.y),
                death_pos.z + 2.0,
            )
        } else {
            death_pos + glam::Vec3::new(2.0, 0.0, 2.0)
        };
        if let Some(pilot_id) = self.create_object_for_player_with_effects(
            EJECT_PILOT_TEMPLATE,
            pilot_owner_player_id,
            spawn_pos,
            None,
            presentation_effects,
        ) {
            self.usa_pilot.record_ejection();
            if parachute_ocl {
                self.usa_pilot.record_air_ejection();
            }
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                if invulnerable_frames > 0 {
                    pilot.apply_eject_invulnerable_at(
                        self.frame.saturating_add(invulnerable_frames),
                        self.frame,
                    );
                }
                if parachute_ocl {
                    let raw_y = pilot.get_position().y;
                    pilot.apply_eject_parachuting();
                    if crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged(
                        raw_y, 0.0,
                    ) {
                        self.usa_pilot.record_parachute_open_fudge();
                    }
                }
                // OCL_EjectPilot* has `InheritsVeterancy = Yes`.
                pilot.experience.level = veterancy;
            }
            if invulnerable_frames > 0 {
                self.usa_pilot.record_invulnerable_grant();
            }
            // C++ EjectPilotDie::ejectPilot playObjectSounds: VoiceEject (pos+player)
            // then SoundEject (pos). Resolve from the dying vehicle, not the slot key.
            self.queue_resolved_per_unit_sound_named(
                &dying_template,
                PILOT_EJECT_AUDIO,
                None,
                Some(death_pos),
                Some(pilot_owner_player_id as i32),
                170,
            );
            self.queue_resolved_per_unit_sound_named(
                &dying_template,
                PILOT_SOUND_EJECT_AUDIO,
                None,
                Some(death_pos),
                None,
                170,
            );
            let _ = pilot_id;
        }
    }

    /// True when the object is a DAM (battle dam) template residual.
    fn dam_template_at(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false)
    }

    pub(crate) fn mark_object_for_destruction(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> gamelogic::GameLogicResult<()> {
        self.mark_object_for_destruction_with_effects(
            id,
            killer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn mark_object_for_destruction_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // This existing adapter has no accepted Body input. Do not fabricate
        // one from preferred history; migrating its callers remains explicit.
        self.begin_object_death_with_effects(id, killer, None, None, presentation_effects)
    }

    pub(in crate::game_logic) fn mark_object_for_destruction_with_topple(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: crate::game_logic::object::OnDieInput,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.mark_object_for_destruction_with_topple_with_effects(
            id,
            killer,
            input,
            topple_interpreter,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn mark_object_for_destruction_with_topple_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: crate::game_logic::object::OnDieInput,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.begin_object_death_with_effects(
            id,
            killer,
            Some(input),
            topple_interpreter,
            presentation_effects,
        )
    }

    /// C++ GameLogic::destroyObject requests deletion independently of body
    /// death. Already-started death retains its pending completion effects;
    /// a live direct request never manufactures an onDie operation.
    pub fn destroy_object(&mut self, id: ObjectId) -> gamelogic::GameLogicResult<()> {
        self.destroy_object_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn destroy_object_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.request_object_destruction_with_effects(id, None, presentation_effects)
    }

    /// A synchronous authored Die callback retains its admitted death credit.
    /// Public direct deletion carries none and never manufactures a death.
    pub(in crate::game_logic) fn request_object_destruction(
        &mut self,
        id: ObjectId,
        death_credit: Option<Team>,
    ) -> gamelogic::GameLogicResult<()> {
        self.request_object_destruction_with_effects(
            id,
            death_credit,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn request_object_destruction_with_effects(
        &mut self,
        id: ObjectId,
        death_credit: Option<Team>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        if self.objects_to_destroy.iter().any(|event| event.id == id) {
            return Ok(());
        }
        if object.status.on_die_started {
            if let Some(object) = self.objects.get_mut(&id) {
                object.set_locomotor_goal_none();
                object.movement.path.clear();
            }
            self.enqueue_object_destruction_with_effects(id, death_credit, presentation_effects);
            self.apply_object_delete_callbacks_with_effects(id, presentation_effects)?;
        } else {
            self.destroy_live_object_with_effects(id, presentation_effects)?;
        }
        Ok(())
    }

    pub(in crate::game_logic) fn queue_after_death_completion(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) {
        if self.objects.contains_key(&id)
            && !self.objects_to_destroy.iter().any(|event| event.id == id)
        {
            self.objects_to_destroy
                .push_back(DestructionEvent::after_death(id, killer));
        }
    }
    pub(crate) fn pending_object_removal_snapshot(
        &self,
    ) -> Vec<crate::game_logic::kill_credit::PendingObjectRemoval> {
        self.objects_to_destroy
            .iter()
            .map(
                |event| crate::game_logic::kill_credit::PendingObjectRemoval {
                    id: event.id,
                    killer: event.killer,
                    after_death: event.needs_death_effects(),
                },
            )
            .collect()
    }
    pub(crate) fn restore_pending_object_removals(
        &mut self,
        records: Vec<crate::game_logic::kill_credit::PendingObjectRemoval>,
    ) {
        self.objects_to_destroy.clear();
        for record in records {
            self.objects_to_destroy.push_back(if record.after_death {
                DestructionEvent::after_death(record.id, record.killer)
            } else {
                DestructionEvent::direct(record.id)
            });
        }
    }

    fn enqueue_object_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        self.enqueue_object_destruction_with_effects(
            id,
            killer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn enqueue_object_destruction_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Timed kernels may already stamp destroyed before reporting final
        // completion. Queue identity, not that bit, proves admission here.
        if !self.objects.contains_key(&id)
            || self.objects_to_destroy.iter().any(|event| event.id == id)
        {
            return;
        }
        self.apply_pending_create_object_die_with_effects(id, presentation_effects);
        self.queue_after_death_completion(id, killer);
        if let Some(object) = self.objects.get_mut(&id) {
            object.status.destroyed = true;
        }
    }

    fn begin_object_death(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: Option<crate::game_logic::object::OnDieInput>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.begin_object_death_with_effects(
            id,
            killer,
            input,
            topple_interpreter,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn begin_object_death_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: Option<crate::game_logic::object::OnDieInput>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let mut receiving_effects = presentation_effects.reborrow();
        if let Some(interpreter) = topple_interpreter {
            receiving_effects.script_events =
                Some(crate::game_logic::ScriptEventRecipient::new(interpreter));
        }
        let presentation_effects = &mut receiving_effects;

        self.object_lifecycle_callback_depth += 1;
        let result = self.begin_object_death_callbacks_with_effects(
            id,
            killer,
            input,
            topple_interpreter,
            presentation_effects,
        );
        self.object_lifecycle_callback_depth -= 1;
        result
    }

    fn begin_object_death_callbacks_with_effects(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        input: Option<crate::game_logic::object::OnDieInput>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if self
            .objects_to_destroy
            .iter()
            .any(|event| event.id == id && !event.needs_death_effects())
        {
            return Ok(());
        }
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.status.on_die_started)
        {
            return Ok(());
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.status.on_die_started = true;
        }
        if input.is_some()
            && self
                .objects
                .get(&id)
                .is_some_and(|object| object.thing().template.native_die_declarations.is_some())
        {
            self.run_authored_die_modules_with_effects(
                id,
                killer,
                input.expect("checked incoming death"),
                topple_interpreter,
                presentation_effects,
            )?;
            return Ok(());
        }
        let fx_source = match input {
            Some(input) => input.source,
            None => self
                .objects
                .get(&id)
                .and_then(|object| object.last_damage_source),
        };
        self.fire_owned_fx_list_die_from_source_with_effects(id, fx_source, presentation_effects);
        if let Some(object) = self.objects.get_mut(&id) {
            object.fire_create_object_die();
        }
        self.apply_pending_create_object_die_with_effects(id, presentation_effects);
        self.admit_object_drawable_deselection(id);
        self.assault_transport_give_final_orders(id);
        // C++ AIUpdate dtor / setCurrentVictim(NULL) + turret nuke on death.
        self.drop_jet_targeters_on_attack_exit(id);
        self.stop_move_loop_sound(id);
        self.stop_ambient_sound(id);

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.unstamp_partition_value_threat_with_partition_owner(&mut self.partition_manager);
            // C++ BoneFXUpdate dtor / onDelete: killRunningParticleSystems.
            if let Some(bfx) = obj.bone_fx_damage.as_mut() {
                bfx.stop_all_bone_fx_with_owner(presentation_effects);
            }
        }
        // C++ BridgeTowerBehavior::onDie kills the span; BridgeBehavior::onDie
        // kills towers. Keep the husk so rubble stays repairable.
        let is_bridge_member = self.objects.get(&id).is_some_and(|obj| {
            obj.is_kind_of(KindOf::Bridge)
                || obj.is_kind_of(KindOf::BridgeTower)
                || crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                    &obj.template_name,
                )
        });
        if is_bridge_member {
            if let Some(obj) = self.objects.get_mut(&id) {
                if !obj.status.keep_as_rubble {
                    obj.convert_bridge_to_rubble_husk(self.frame);
                }
            }
            self.apply_owned_bridge_death_links_with_effects(id, presentation_effects);
            return Ok(());
        }

        // C++ AIDockState::onExit → AIDockMachine::halt → cancelDock on death.
        self.cancel_dock_reservation(id);

        // CPP OverlordContain200: deactivate before synchronous child kill;
        // the child's callbacks still resolve the installed outer object.
        let redirected = self.redirected_containment_owner(id);
        if redirected != id {
            if let Some(outer) = self.objects.get_mut(&id) {
                outer.overlord_redirection_active = false;
            }
            self.apply_owned_kill_with_effects(
                redirected,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                presentation_effects,
            );
            // Base TransportContain::onDie now sees the physical payload in
            // the hull's literal roster, not the redirected infantry list.
            self.remove_direct_contained_child_with_effects(id, redirected, presentation_effects)?;
        }

        // All contained effects complete on the installed owner before later
        // InstantDeath, SlowDeath or retained-body callbacks can observe it.
        self.complete_installed_containment_death_with_effects(
            id,
            killer,
            input,
            presentation_effects,
        )?;

        // C++ ProductionUpdate cancelAndRefund on death start (before topple/slow-death deferral).
        self.cancel_all_production(id);
        // C++ SpecialPowerCompletionDie::onDie residual.
        self.maybe_notify_special_power_completion(id, presentation_effects);
        // C++ DamDie::onDie residual fires with other die modules at death start.
        self.maybe_apply_dam_die(id);
        // Wave 754: C++ EjectPilotDie::onDie at death start (before SlowDeath defer).
        self.maybe_apply_eject_pilot_die_with_effects(id, presentation_effects);
        // C++ SpawnBehavior::onDie SpawnedRequireSpawner — kill remaining slaves.
        self.apply_spawned_require_spawner_on_die_with_effects(id, presentation_effects);
        // C++ OCL ApplyRandomForceNugget residual (air-death toss before debris).
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        // C++ RebuildHoleExposeDie::onDie at death start (before topple/slow-death).
        // WorkerRespawnDelay starts here, not after collapse Done.
        let _ = self.maybe_spawn_rebuild_hole_with_effects(id, presentation_effects);
        // C++ InstantDeathBehavior::onDie — FX/OCL/Weapon then destroyObject.
        let death_type = input
            .map(|input| input.death_type)
            .or_else(|| self.objects.get(&id).map(|object| object.status.death_type));
        if death_type.is_some_and(|death_type| {
            self.try_apply_instant_death_with_effects(id, death_type, presentation_effects)
        }) {
            self.queue_after_death_completion(id, killer);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.status.destroyed = true;
            }

            return Ok(());
        }
        let source = match input {
            Some(input) => input.source,
            None => self
                .objects
                .get(&id)
                .and_then(|object| object.last_damage_source),
        };
        let crusher_xz = source.and_then(|src| {
            let crusher = self.objects.get(&src)?;
            let p = crusher.get_position();
            Some((p.x, p.z))
        });
        if let Some(obj) = self.objects.get_mut(&id) {
            if !obj.front_crushed && !obj.back_crushed {
                if let Some(input) = input {
                    obj.fire_crush_die_from_input(input.damage_type, crusher_xz);
                } else {
                    obj.fire_crush_die_from_crusher(crusher_xz);
                }
            }
        }
        // Wave 482: BuildAssistant sell finish removes the object immediately.
        // Do not defer into StructureTopple/Collapse / SlowDeath / KeepObjectDie —
        // those combat-death peels left sold structures alive forever in host-only tests.
        let (sold, under_construction, is_rebuild_hole) = self
            .objects
            .get(&id)
            .map(|o| {
                (
                    o.status.sold,
                    o.status.under_construction,
                    o.is_rebuild_hole,
                )
            })
            .unwrap_or((false, false, false));
        // Wave 715: MSG_DOZER_CANCEL_CONSTRUCT / unfinished builds remove immediately.
        // Do not defer into StructureTopple — cancel would leave the shell alive a frame+.
        // Rebuild holes are already craters (no StructureToppleUpdate in C++).
        // C++ LandMineInterface::disarm destroys mines immediately: KINDOF_DEMOTRAP
        // mines are not buildings, so they must never defer into StructureTopple/
        // Collapse / SlowDeath / KeepObjectDie residuals even when their residual
        // template carries KindOf::Structure.
        // Body death follows its authored delayed or retained death behavior.
        let is_mine = self.objects.get(&id).is_some_and(|o| o.mine_data.is_some());
        // Derived containment callbacks have already completed while installed.
        // An authored delayed death may retain the empty owner afterward.
        let defer_death_animations = !sold && !under_construction && !is_rebuild_hole && !is_mine;
        if defer_death_animations {
            // C++ StructureTopple/Collapse residual: buildings fall/sink before remove.
            let source = match input {
                Some(input) => input.source,
                None => self
                    .objects
                    .get(&id)
                    .and_then(|object| object.last_damage_source),
            };
            if self.try_begin_structure_topple_with_override_with_effects(
                id,
                source,
                topple_interpreter,
                presentation_effects,
            ) {
                return Ok(());
            }
            // C++ SlowDeathBehavior residual: infantry/vehicles delay destroy + sink.
            if self.try_begin_slow_death_instead_of_destroy(id, presentation_effects)? {
                return Ok(());
            }
            // C++ KeepObjectDie residual: leave rubble, do not DestroyDie-remove.
            if self
                .try_begin_keep_object_die_instead_of_destroy_with_effects(id, presentation_effects)
            {
                return Ok(());
            }
        }
        self.enqueue_object_destruction_with_effects(id, killer, presentation_effects);
        Ok(())
    }

    /// C++ InstantDeathBehavior::onDie residual.
    pub(in super::super::super) fn try_apply_instant_death(
        &mut self,
        id: ObjectId,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        self.try_apply_instant_death_with_effects(
            id,
            death_type,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn try_apply_instant_death_with_effects(
        &mut self,
        id: ObjectId,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let extra_owned = {
            let Some(obj) = self.objects.get(&id) else {
                return false;
            };
            obj.owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .map(|p| p.completed_upgrades.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let fired = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return false;
            };
            if !obj.fire_instant_death_for(
                death_type,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            ) {
                false
            } else {
                if !extra_owned.is_empty() {
                    let _ = extra_owned;
                }
                true
            }
        };
        if !fired {
            return false;
        }
        self.apply_pending_create_object_die_with_effects(id, presentation_effects);
        if let Some(wpn) = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.pending_instant_death_weapon.take())
        {
            // C++ InstantDeathBehavior.cpp:141-147 creates, fires and deletes
            // a temporary Weapon from the authored template. This request
            // has no persistent module key or snapshot state.
            let spec = crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec {
                module_source_index: 0,
                weapon_template_name: wpn.clone(),
                weapon_slot: crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponSlot::Primary,
            };
            if self
                .create_and_fire_temp_weapon_with_effects(id, &spec, presentation_effects)
                .is_none()
            {
                let _ = self.apply_fire_weapon_when_damaged_named_with_effects(
                    id,
                    &wpn,
                    presentation_effects,
                );
            }
        }
        true
    }

    /// C++ KeepObjectDie residual: convert to lasting rubble, skip remove.
    pub(in super::super::super) fn try_begin_keep_object_die_instead_of_destroy(
        &mut self,
        id: ObjectId,
    ) -> bool {
        self.try_begin_keep_object_die_instead_of_destroy_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn try_begin_keep_object_die_instead_of_destroy_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Wave 775: StructureCollapse/Topple already ran their presentation; after Done
        // allow normal destroy instead of KeepObjectDie forever-defer (civilian barns).
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        if obj.status.keep_as_rubble {
            return true;
        }
        if !obj.begin_keep_object_die(
            frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        ) {
            return false;
        }
        // Death FX / OCL peels without world removal.
        self.fire_owned_fx_list_die_with_effects(id, presentation_effects);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.fire_create_object_die();
        }
        self.apply_pending_create_object_die_with_effects(id, presentation_effects);
        if self.dam_template_at(id) {
            self.apply_dam_die_enable_waveguides();
        }
        true
    }

    /// C++ DamDie::onDie residual — enable KINDOF_WAVEGUIDE objects.
    /// C++ UpgradeDie::onDie residual.
    pub(in super::super::super) fn maybe_apply_upgrade_die(&mut self, id: ObjectId) {
        let (producer, upgrade) = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            let Some(ud) = obj.upgrade_die.as_mut() else {
                return;
            };
            if ud.fired {
                return;
            }
            ud.fired = true;
            (obj.producer_id, ud.upgrade_to_remove.clone())
        };
        let Some(pid) = producer else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        let Some(master) = self.objects.get_mut(&pid) else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        if master.remove_upgrade_tag(&upgrade) {
            self.upgrade_die_reg.record_removal();
        } else {
            self.upgrade_die_reg.record_missing_upgrade();
        }
    }

    pub(in super::super::super) fn maybe_apply_dam_die(&mut self, id: ObjectId) {
        if self.dam_template_at(id) {
            self.apply_dam_die_enable_waveguides();
        }
    }

    pub(in super::super::super) fn apply_dam_die_enable_waveguides(&mut self) {
        let frame = self.frame;
        for obj in self.objects.registered_values_mut() {
            let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                || crate::game_logic::host_dam_die::is_wave_guide_template(&obj.template_name)
                || crate::game_logic::host_wave_guide::is_wave_guide_template(&obj.template_name);
            if is_wg {
                obj.status.disabled_default = false;
                if obj.wave_guide_data.is_none() {
                    let mut wg = crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                    wg.facing = obj.get_orientation();
                    wg.ensure_active(frame.max(1));
                    obj.wave_guide_data = Some(wg);
                } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                    wg.ensure_active(frame.max(1));
                }
            }
        }
    }

    pub(in super::super::super) fn try_begin_slow_death_instead_of_destroy(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let frame = self.frame;
        // Jet crash residual. Ground/deck explode must not fall through to heli/slow.
        let is_jet = self.objects.get(&id).is_some_and(|obj| {
            crate::game_logic::host_jet_slow_death::is_jet_slow_death_template(&obj.template_name)
                || obj.jet_slow_death.is_some()
        });
        if is_jet {
            let Some(obj) = self.objects.get(&id) else {
                return Ok(false);
            };
            if obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false) {
                return Ok(false);
            }
            if obj
                .jet_slow_death
                .as_ref()
                .map(|j| j.is_active())
                .unwrap_or(false)
            {
                return Ok(true);
            }
            let start = self
                .objects
                .get_mut(&id)
                .map(|obj| {
                    obj.begin_jet_slow_death(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ))
                })
                .unwrap_or_default();
            self.commit_slow_death_idle_owned(id, start, effects)?;
            return Ok(start.defer_destruction);
        }
        // Helicopter spiral crash residual.
        let Some(obj) = self.objects.get(&id) else {
            return Ok(false);
        };
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.done)
            .unwrap_or(false)
        {
            return Ok(false);
        }
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            return Ok(true);
        }
        let start = self
            .objects
            .get_mut(&id)
            .map(|obj| {
                obj.begin_helicopter_slow_death(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ))
            })
            .unwrap_or_default();
        self.commit_slow_death_idle_owned(id, start, effects)?;
        if start.defer_destruction {
            return Ok(true);
        }
        let Some(obj) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        // Already finished slow death → allow destroy.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_done())
            .unwrap_or(false)
        {
            return Ok(false);
        }
        // Mid slow death → keep deferring.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            return Ok(true);
        }
        if obj.begin_slow_death(
            frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        ) {
            return Ok(true);
        }
        Ok(false)
    }

    fn commit_slow_death_idle_owned(
        &mut self,
        id: ObjectId,
        start: crate::game_logic::object::SlowDeathStart,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if !start.enter_idle {
            return Ok(());
        }
        self.set_ai_state_decision_aware_with_effects(id, AIState::Idle, effects)?;
        // C++ beginSlowDeath clears the target after setState(Idle) onExit.
        if let Some(object) = self.objects.get_mut(&id) {
            object.target = None;
        }
        Ok(())
    }

    /// CPP doDamageLine fires each real weapon before that point's FX. The
    /// immutable template is resolved before lending the owner to callbacks.
    pub(crate) fn apply_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
    ) -> Result<(), String> {
        self.apply_structure_topple_crush_samples_with_effects(
            building_id,
            samples,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_structure_topple_crush_samples_with_effects(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(weapon) = self.preflight_owned_structure_topple_crushing(building_id)? else {
            return Ok(());
        };
        self.apply_prepared_structure_topple_crush_samples_with_effects(
            building_id,
            samples,
            &weapon,
            presentation_effects,
        )
    }

    pub(super) fn preflight_owned_structure_topple_crushing(
        &self,
        building_id: ObjectId,
    ) -> Result<Option<std::sync::Arc<gamelogic::weapon::WeaponTemplate>>, String> {
        let Some(source) = self.objects.get(&building_id) else {
            return Ok(None);
        };
        let Some(data) = source.structure_topple_data.as_ref() else {
            return Ok(None);
        };
        let name = data.crushing_weapon_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("NONE") {
            return Ok(None);
        }
        let template = crate::game_logic::weapon_bootstrap::with_host_weapon_store(|store| {
            store.find_weapon_template(name).cloned()
        })
        .map_err(|error| format!("StructureTopple {building_id:?} weapon {name}: {error}"))?;
        // CPP findWeaponTemplate(NULL) returns without firing or advancing the
        // sweep. A missing definition is not an invented default weapon.
        let Some(template) = template else {
            return Ok(None);
        };
        let authored_name =
            |name: &str| !name.trim().is_empty() && !name.eq_ignore_ascii_case("NONE");
        let unsupported = if source.contained_by.is_some() {
            Some("contained firing source")
        } else if authored_name(&template.projectile_name)
            || authored_name(&template.projectile_stream_name)
        {
            Some("projectile/stream birth")
        } else if authored_name(&template.laser_name) {
            Some("laser birth")
        } else if template.damage_dealt_at_self_position {
            Some("DamageDealtAtSelfPosition")
        } else if template.damage_status_type.bits() != 0 {
            Some("DamageStatusType")
        } else if template.scatter_radius != 0.0 || !template.scatter_targets.is_empty() {
            Some("scatter targeting")
        } else if template.historic_bonus_count > 0 {
            Some("historic bonus weapon")
        } else if template.radius_damage_angle < std::f32::consts::PI {
            Some("directional radius damage")
        } else if template.affects_mask.bits()
            & crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF
            != 0
        {
            Some("KillsSelf")
        } else if template.fire_fx.iter().any(Option::is_some)
            || template.fire_ocl.iter().any(Option::is_some)
            || template
                .fire_ocl_names
                .iter()
                .flatten()
                .any(|name| authored_name(name))
            || !template.fire_sound.is_empty()
        {
            Some("weapon fire-time FX/OCL/audio")
        } else if !template.weapon_speed.is_finite() || template.weapon_speed <= 0.0 {
            Some("invalid weapon speed")
        } else {
            None
        };
        if let Some(reason) = unsupported {
            return Err(format!(
                "StructureTopple {building_id:?} weapon {name} does not yet admit {reason}"
            ));
        }
        // Geometric admission of the complete eventual sweep happens before
        // any real phase mutation. This owned copy derives points only; its
        // state/cursor is never replayed or written back to the Object.
        let facing_width = data.crush_facing_width();
        let rows = (data.building_height as f64 / 25.0).ceil() + 1.0;
        let columns = (2.0 * facing_width as f64 / 25.0).ceil() + 1.0;
        if !source.get_position().is_finite()
            || !data.building_height.is_finite()
            || data.building_height < 0.0
            || !facing_width.is_finite()
            || facing_width < 0.0
            || rows * columns > 262_144.0
        {
            return Err(format!(
                "StructureTopple {building_id:?} weapon {name} has invalid or unsupported sweep geometry"
            ));
        }
        let mut point_plan = data.clone();
        point_plan.state =
            crate::game_logic::host_structure_topple::HostStructureToppleState::WaitingForDone;
        point_plan.last_crushed_location = 0.0;
        let position = source.get_position();
        let points = point_plan.take_crush_sweep_samples(position.x, position.z);
        self.validate_structure_topple_crush_points(building_id, &points, &template)?;
        Ok(Some(template))
    }

    pub(super) fn validate_structure_topple_crush_points(
        &self,
        building_id: ObjectId,
        samples: &[crate::game_logic::host_structure_topple::StructureToppleCrushSample],
        weapon: &gamelogic::weapon::WeaponTemplate,
    ) -> Result<(), String> {
        let Some(source) = self.objects.get(&building_id) else {
            return Ok(());
        };
        for s in samples {
            let height = self
                .terrain_ground_height_at(glam::Vec3::new(s.x, 0.0, s.z))
                .unwrap_or(0.0);
            let impact = glam::Vec3::new(s.x, height, s.z);
            let delay = source.get_position().distance(impact) / weapon.weapon_speed;
            if !impact.is_finite() || !delay.is_finite() || delay >= 1.0 {
                return Err(format!(
                    "StructureTopple {building_id:?} weapon {} requires unsupported delayed/invalid positional damage",
                    weapon.name
                ));
            }
        }
        Ok(())
    }

    pub(super) fn apply_prepared_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
        weapon: &gamelogic::weapon::WeaponTemplate,
    ) -> Result<(), String> {
        self.apply_prepared_structure_topple_crush_samples_with_effects(
            building_id,
            samples,
            weapon,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn apply_prepared_structure_topple_crush_samples_with_effects(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
        weapon: &gamelogic::weapon::WeaponTemplate,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        self.validate_structure_topple_crush_points(building_id, &samples, weapon)?;
        for s in samples {
            // Release the source loan before damage/onDie. Each following
            // point rereads this match's source and bonuses after callbacks.
            let Some(source) = self.objects.get(&building_id) else {
                break;
            };
            let team = source.team;
            let (damage_bonus, _, _, _, radius_bonus) =
                source.weapon_bonus_fields_for_named_weapon_with_solo_set(
                    &weapon.name,
                    &self.solo_weapon_bonus_set,
                );
            let crushing_fx = source
                .structure_topple_data
                .as_ref()
                .map(|data| data.crushing_fx.clone())
                .unwrap_or_default();
            let height = self
                .terrain_ground_height_at(glam::Vec3::new(s.x, 0.0, s.z))
                .unwrap_or(0.0);
            let impact = glam::Vec3::new(s.x, height, s.z);
            // CPP Weapon.cpp1293-1470: the shared actual Main weapon route
            // includes structures, authored affects/armor/death, and finishes
            // each victim's onDie before visiting the next candidate.
            self.apply_instant_hit_splash_at_with_effects(
                impact,
                weapon.primary_damage * damage_bonus,
                weapon.secondary_damage * damage_bonus,
                weapon.primary_damage_radius * radius_bonus,
                weapon.secondary_damage_radius * radius_bonus,
                building_id,
                team,
                ObjectId(0), // positional shot has no primary victim
                Some(&weapon.name),
                presentation_effects,
            );
            // CPP doDamageLine407-429: even an empty/no-damage weapon shot is
            // followed by its CrushingFX at this exact terrain point.
            if !crushing_fx.is_empty()
                && self
                    .objects
                    .get(&building_id)
                    .is_some_and(|source| source.structure_topple_fx_is_allowed())
            {
                self.dispatch_fx_list_at_host_position_with_effects(
                    &crushing_fx,
                    impact,
                    presentation_effects,
                );
            }
        }
        Ok(())
    }

    /// C++ CreateObjectDie::onDie residual — spawn OCL templates at dying object.
    pub fn apply_pending_create_object_die(&mut self, dying_id: ObjectId) {
        self.apply_pending_create_object_die_with_effects(
            dying_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_pending_create_object_die_with_effects(
        &mut self,
        dying_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let (spawns, transfer_dmg, transfer, subdual, source, team, owner_player_id, pos) = {
            let Some(o) = self.objects.get_mut(&dying_id) else {
                return;
            };
            let (spawns, dmg, transfer, subdual, source) =
                o.take_pending_create_object_die_spawns();
            if let Some(index) = o.native_die_runtime.pending_create.take() {
                o.native_die_runtime.completed.insert(index);
            }
            (
                spawns,
                dmg,
                transfer,
                subdual,
                source,
                o.team,
                o.owner_player_id,
                o.get_position(),
            )
        };
        self.execute_create_object_die_spawns_with_effects(
            dying_id,
            spawns,
            transfer_dmg,
            transfer,
            subdual,
            source,
            team,
            owner_player_id,
            pos,
            presentation_effects,
        );
    }

    fn execute_create_object_die_spawns(
        &mut self,
        dying_id: ObjectId,
        spawns: Vec<String>,
        transfer_dmg: f32,
        transfer: bool,
        subdual: f32,
        source: Option<ObjectId>,
        team: Team,
        owner_player_id: Option<u32>,
        pos: Vec3,
    ) {
        self.execute_create_object_die_spawns_with_effects(
            dying_id,
            spawns,
            transfer_dmg,
            transfer,
            subdual,
            source,
            team,
            owner_player_id,
            pos,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn execute_create_object_die_spawns_with_effects(
        &mut self,
        dying_id: ObjectId,
        spawns: Vec<String>,
        transfer_dmg: f32,
        transfer: bool,
        subdual: f32,
        source: Option<ObjectId>,
        team: Team,
        owner_player_id: Option<u32>,
        pos: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if spawns.is_empty() {
            return;
        }
        let mut spawned_ids: Vec<ObjectId> = Vec::new();
        for tmpl in spawns {
            let tl = tmpl.to_ascii_lowercase();
            if tl.contains("debris") || tl.contains("barrel") {
                use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
                let plan = if tl.contains("barrel") {
                    HostOclCreateDebrisPlan::damaged_barrel()
                } else {
                    let mut p = HostOclCreateDebrisPlan::generic_tank_debris();
                    p.model_or_template = tmpl.clone();
                    p
                };
                let inherit = self
                    .objects
                    .get(&dying_id)
                    .map(|o| o.movement.velocity)
                    .unwrap_or(Vec3::ZERO);
                let ids = self.spawn_ocl_create_debris_with_effects(
                    &plan,
                    team,
                    pos,
                    inherit,
                    owner_player_id,
                    presentation_effects,
                );
                if transfer {
                    for id in &ids {
                        self.apply_death_transfer_damage_with_effects(
                            id,
                            subdual,
                            transfer_dmg,
                            source,
                            presentation_effects,
                        );
                    }
                }
                spawned_ids.extend(ids);
                continue;
            }
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.set_health(100.0);
                if tmpl.to_ascii_lowercase().contains("tunnel")
                    || tmpl.to_ascii_lowercase().contains("network")
                {
                    t.add_kind_of(KindOf::Structure);
                }
                self.templates.insert(tmpl.clone(), t);
            }
            let Some(new_id) = self.create_object_for_owner_or_team_with_effects(
                &tmpl,
                team,
                owner_player_id,
                pos,
                None,
                presentation_effects,
            ) else {
                continue;
            };
            if let Some(dying) = self.objects.get(&dying_id) {
                let yaw = dying.get_orientation();
                if let Some(n) = self.objects.get_mut(&new_id) {
                    n.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                    n.producer_id = Some(dying_id);
                }
            }
            if let Some(n) = self.objects.get_mut(&new_id) {
                n.ensure_fuel_air_gas_slow_death(self.frame);
                if n.fuel_air_gas_slow_death.is_some() {
                    self.fuel_air_gas_reg.record_install();
                }
            }
            if transfer {
                self.apply_death_transfer_damage_with_effects(
                    &new_id,
                    subdual,
                    transfer_dmg,
                    source,
                    presentation_effects,
                );
            }
            spawned_ids.push(new_id);
        }
        if transfer {
            for new_id in spawned_ids {
                let _ = self.transfer_attack(dying_id, new_id);
            }
        }
    }

    pub(in super::super::super) fn apply_fire_weapon_when_damaged_named(
        &mut self,
        source_id: ObjectId,
        weapon_name: &str,
    ) -> u32 {
        self.apply_fire_weapon_when_damaged_named_with_effects(
            source_id,
            weapon_name,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn apply_fire_weapon_when_damaged_named_with_effects(
        &mut self,
        source_id: ObjectId,
        weapon_name: &str,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let (pos, team) = match self.objects.get(&source_id) {
            Some(o) => (o.get_position(), o.team),
            None => return 0,
        };
        let (pd, pr, sd, sr) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_weapon_splash(
                weapon_name,
            );
        // Intended = self so splash doesn't skip others incorrectly... API skips intended_id.
        // Pass a dummy non-existent intended so all in radius can be hit except we should not hit self.
        // apply_instant_hit_splash_at skips intended_id only — use source as intended to skip self.
        self.apply_instant_hit_splash_at_with_effects(
            pos,
            pd,
            sd,
            pr,
            sr,
            source_id,
            team,
            source_id,
            Some(weapon_name),
            presentation_effects,
        )
    }

    pub(in super::super::super) fn try_begin_structure_topple_instead_of_destroy(
        &mut self,
        id: ObjectId,
    ) -> bool {
        self.try_begin_structure_topple_instead_of_destroy_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn try_begin_structure_topple_instead_of_destroy_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let source = self
            .objects
            .get(&id)
            .and_then(|object| object.last_damage_source);
        self.try_begin_structure_topple_with_override_with_effects(
            id,
            source,
            None,
            presentation_effects,
        )
    }

    fn try_begin_structure_topple_with_override(
        &mut self,
        id: ObjectId,
        incoming_source: Option<ObjectId>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> bool {
        self.try_begin_structure_topple_with_override_with_effects(
            id,
            incoming_source,
            topple_interpreter,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn try_begin_structure_topple_with_override_with_effects(
        &mut self,
        id: ObjectId,
        incoming_source: Option<ObjectId>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        self.try_begin_structure_die_with_override_with_effects(
            id,
            incoming_source,
            topple_interpreter,
            None,
            presentation_effects,
        )
    }

    fn try_begin_structure_die_with_override(
        &mut self,
        id: ObjectId,
        incoming_source: Option<ObjectId>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        collapse_only: Option<bool>,
    ) -> bool {
        self.try_begin_structure_die_with_override_with_effects(
            id,
            incoming_source,
            topple_interpreter,
            collapse_only,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn try_begin_structure_die_with_override_with_effects(
        &mut self,
        id: ObjectId,
        incoming_source: Option<ObjectId>,
        topple_interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        collapse_only: Option<bool>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();

        let attacker_pos = {
            incoming_source.and_then(|sid| {
                self.objects.get(&sid).map(|s| {
                    let p = s.get_position();
                    (p.x, p.z)
                })
            })
        };
        let frame = self.frame;
        // Read at the actual Topple admission, after prior onDie callbacks.
        // An explicitly supplied interpreter remains authoritative even when
        // its named override is absent; no process owner or mirror is selected.
        let override_direction = self
            .host_object(id)
            .and_then(|object| {
                topple_interpreter
                    .or(self.owned_script_engine.as_ref())
                    .and_then(|engine| engine.topple_direction_for_name(&object.name))
            })
            .map(|direction| (direction.x, direction.y));
        #[cfg(feature = "game_client")]
        let fx_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Already finished collapse or topple → allow normal destroy.
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if match collapse_only {
            Some(true) => collapse_done,
            Some(false) => topple_done,
            None => collapse_done || topple_done,
        } {
            return false;
        }
        let collapse_active = obj
            .structure_collapse_data
            .as_ref()
            .is_some_and(|data| data.is_active());
        let topple_active = obj
            .structure_topple_data
            .as_ref()
            .is_some_and(|data| data.is_active());
        if match collapse_only {
            Some(true) => collapse_active,
            Some(false) => topple_active,
            None => collapse_active || topple_active,
        } {
            return true;
        }
        // Prefer StructureCollapse for civilian/prop peels; else StructureTopple.
        #[cfg(feature = "game_client")]
        let collapse_started = Self::with_object_fx_services(
            &mut self.session_view,
            self.queued_audio_events.policy().clone(),
            fx_player,
            self.world_services.shroud(),
            presentation_effects,
            &fx_operation_environment,
            self.world_services.terrain(),
            &self.logic_random,
            |view| {
                if collapse_only == Some(false) {
                    return false;
                }
                let Some((started, retired)) = obj.prepare_structure_collapse(frame) else {
                    return false;
                };
                if !started {
                    return true;
                }
                view.retire_particles(retired);
                obj.dispatch_pending_collapse_fx_with(&mut |name, object, at_object| {
                    Self::dispatch_object_phase_fx_with_view(
                        name,
                        object,
                        at_object,
                        fx_player,
                        self.world_services.shroud(),
                        view,
                    );
                });
                true
            },
        );
        #[cfg(not(feature = "game_client"))]
        let collapse_started = collapse_only != Some(false) && obj.begin_structure_collapse(frame);
        if collapse_started || collapse_only == Some(true) {
            return collapse_started;
        }
        if !obj.thing().template.authored_structure_topple {
            return false;
        }
        // Original StructureToppleUpdate::onDie gates before AI/deselect,
        // direction/delay selection, Start effects or runtime installation.
        if !obj.structure_topple_die_is_applicable() {
            return false;
        }
        let delay = obj
            .thing()
            .template
            .structure_topple_definition
            .as_ref()
            .and_then(|definition| definition.as_ref().ok())
            .map(|definition| {
                let hi = definition.max_delay.max(definition.min_delay);
                let span = hi.wrapping_sub(definition.min_delay).wrapping_add(1);
                if span == 0 {
                    hi
                } else {
                    definition
                        .min_delay
                        .wrapping_add(self.logic_random.next_u32() % span)
                }
            })
            .unwrap_or(0);
        let position = obj.get_position();
        let radius =
            ((*obj.geometry_info()).major_radius + (*obj.geometry_info()).minor_radius) * 0.5 * 0.9;
        let random_unit = self.logic_random.next_u32() as f32 / u32::MAX as f32;
        let angle = match attacker_pos {
            Some((x, z)) => {
                (position.z - z).atan2(position.x - x)
                    + (random_unit * 2.0 - 1.0) * std::f32::consts::PI / 8.0
            }
            None => random_unit * std::f32::consts::TAU,
        };
        let original_direction = (angle.cos(), angle.sin());
        // The named script override changes the fall direction, but C++ keeps
        // the pre-override angle for the delay-burst position.
        let direction = override_direction.or(Some(original_direction));
        let mut burst = glam::Vec3::new(
            position.x + radius * original_direction.0,
            0.0,
            position.z + radius * original_direction.1,
        );
        let _ = obj;
        burst.y = self.terrain_ground_height_at(burst).unwrap_or(0.0);
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        let prepared = self.objects.get_mut(&id).is_some_and(|object| {
            object.prepare_structure_topple_with_direction(
                frame,
                attacker_pos,
                direction,
                delay,
                Some(burst),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            )
        });
        if prepared {
            self.dispatch_owned_structure_topple_fx_with_effects(id, presentation_effects);
        }
        prepared
    }
}
