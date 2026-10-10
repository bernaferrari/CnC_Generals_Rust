//! Main GameLogic death dispatch, retained death phases, and destruction requests.
//! C++ Object/Die modules: preserve their synchronous ordering on the driving owner.
use super::*;

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
        let Some(n) = self.objects.get_mut(id) else {
            return;
        };
        if subdual > 0.0 {
            let _ = n.take_damage_from_typed_with_repulsor_policy(
                subdual,
                None,
                crate::game_logic::combat::DamageType::SubdualUnresistable,
                &mut self.health_events,
                &self.enable_repulsors,
            );
        }
        if transfer_dmg > 0.0 {
            let _ = n.take_damage_from_typed_with_repulsor_policy(
                transfer_dmg,
                source,
                crate::game_logic::combat::DamageType::Unresistable,
                &mut self.health_events,
                &self.enable_repulsors,
            );
        }
    }

    /// Wave 482: sell residual kill (parked aircraft) — queue remove without
    /// SlowDeath/Topple deferral peels used for combat deaths.
    pub(in super::super::super) fn destroy_object_for_sell_residual(&mut self, id: ObjectId) {
        self.maybe_notify_special_power_completion(id);
        self.maybe_apply_dam_die(id);
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        self.objects_to_destroy
            .push_back(DestructionEvent::after_death(id, None));
    }

    /// C++ FireWeaponWhenDeadBehavior::onDie leftover — death weapon splash.
    /// Specialized leftovers (bomb truck / toxin / nuke / demo) own exclusive modules.
    pub(in super::super::super) fn apply_fire_weapon_when_dead(&mut self, dying_id: ObjectId) {
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
            .iter()
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
            let destroyed = v.take_damage_from_immediate_with_repulsor_policy(
                dmg,
                Some(dying_id),
                &mut self.health_events,
                &self.enable_repulsors,
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
                self.objects_to_destroy
                    .push_back(DestructionEvent::after_death(id, Some(team)));
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
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            self.health_events.record_damage(id, hp, source, true);
        } else {
            o.health.current = 0.0;
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
        if let Some(pilot_id) =
            self.create_object_for_player(EJECT_PILOT_TEMPLATE, pilot_owner_player_id, spawn_pos)
        {
            self.usa_pilot.record_ejection();
            if parachute_ocl {
                self.usa_pilot.record_air_ejection();
            }
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                if invulnerable_frames > 0 {
                    pilot.apply_eject_invulnerable(self.frame.saturating_add(invulnerable_frames));
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

    pub(crate) fn mark_object_for_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        self.begin_object_death(id, killer);
    }

    /// C++ GameLogic::destroyObject requests deletion independently of body
    /// death. Already-started death retains its pending completion effects;
    /// a live direct request never manufactures an onDie operation.
    pub fn destroy_object(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if self.objects_to_destroy.iter().any(|event| event.id == id) {
            return;
        }
        if object.status.on_die_started {
            if let Some(object) = self.objects.get_mut(&id) {
                object.set_locomotor_goal_none();
                object.movement.path.clear();
            }
            self.enqueue_object_destruction(id, None);
            self.apply_object_delete_callbacks(id);
        } else {
            self.destroy_live_object(id);
        }
    }

    fn enqueue_object_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        // Timed kernels may already stamp destroyed before reporting final
        // completion. Queue identity, not that bit, proves admission here.
        if !self.objects.contains_key(&id)
            || self.objects_to_destroy.iter().any(|event| event.id == id)
        {
            return;
        }
        self.apply_pending_create_object_die(id);
        self.objects_to_destroy
            .push_back(DestructionEvent::after_death(id, killer));
        if let Some(object) = self.objects.get_mut(&id) {
            object.status.destroyed = true;
        }
        let _ = crate::gameworld_shadow::eager_mark_host_destroy_if_coupled(id);
    }

    fn begin_object_death(&mut self, id: ObjectId, killer: Option<Team>) {
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.status.on_die_started)
        {
            return;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.status.on_die_started = true;
        }
        self.assault_transport_give_final_orders(id);
        // C++ AIUpdate dtor / setCurrentVictim(NULL) + turret nuke on death.
        self.drop_jet_targeters_on_attack_exit(id);
        self.stop_move_loop_sound(id);
        self.stop_ambient_sound(id);

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.unstamp_partition_value_threat();
            // C++ BoneFXUpdate dtor / onDelete: killRunningParticleSystems.
            if let Some(bfx) = obj.bone_fx_damage.as_mut() {
                bfx.stop_all_bone_fx();
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
                    crate::game_logic::host_bridge_behavior::record_death_link(id);
                    obj.convert_bridge_to_rubble_husk();
                }
            }
            return;
        }

        // C++ AIDockState::onExit → AIDockMachine::halt → cancelDock on death.
        self.cancel_dock_reservation(id);

        // C++ ProductionUpdate cancelAndRefund on death start (before topple/slow-death deferral).
        self.cancel_all_production(id);
        // C++ SpecialPowerCompletionDie::onDie residual.
        self.maybe_notify_special_power_completion(id);
        // C++ DamDie::onDie residual fires with other die modules at death start.
        self.maybe_apply_dam_die(id);
        // Wave 754: C++ EjectPilotDie::onDie at death start (before SlowDeath defer).
        self.maybe_apply_eject_pilot_die(id);
        // C++ SpawnBehavior::onDie SpawnedRequireSpawner — kill remaining slaves.
        self.apply_spawned_require_spawner_on_die(id);
        // C++ OCL ApplyRandomForceNugget residual (air-death toss before debris).
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        // C++ RebuildHoleExposeDie::onDie at death start (before topple/slow-death).
        // WorkerRespawnDelay starts here, not after collapse Done.
        let _ = self.maybe_spawn_rebuild_hole(id);
        // C++ InstantDeathBehavior::onDie — FX/OCL/Weapon then destroyObject.
        if self.try_apply_instant_death(id) {
            self.objects_to_destroy
                .push_back(DestructionEvent::after_death(id, killer));
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.status.destroyed = true;
            }
            let _ = crate::gameworld_shadow::eager_mark_host_destroy_if_coupled(id);
            return;
        }
        let crusher_xz = self.objects.get(&id).and_then(|obj| {
            let src = obj.last_damage_source?;
            let crusher = self.objects.get(&src)?;
            let p = crusher.get_position();
            Some((p.x, p.z))
        });
        if let Some(obj) = self.objects.get_mut(&id) {
            if !obj.front_crushed && !obj.back_crushed {
                obj.fire_crush_die_from_crusher(crusher_xz);
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
        // C++ CaveContain::onDie (CaveContain.cpp:197-211) overrides
        // OpenContain::onDie with no super call: death immediately
        // unregisters the cave and runs TunnelTracker::onTunnelDestroyed —
        // the last-cave cave-in that kills the shared pool.  Retail caves
        // author no StructureToppleUpdate / StructureCollapseUpdate, so a
        // cave-style container must reach the destroy list (and its cave-in
        // branch) instead of deferring into a topple/collapse animation.
        let is_cave = self
            .objects
            .get(&id)
            .is_some_and(|o| o.is_cave_style_container());
        let defer_death_animations =
            !sold && !under_construction && !is_rebuild_hole && !is_mine && !is_cave;
        if defer_death_animations {
            // C++ StructureTopple/Collapse residual: buildings fall/sink before remove.
            if self.try_begin_structure_topple_instead_of_destroy(id) {
                return;
            }
            // C++ SlowDeathBehavior residual: infantry/vehicles delay destroy + sink.
            if self.try_begin_slow_death_instead_of_destroy(id) {
                return;
            }
            // C++ KeepObjectDie residual: leave rubble, do not DestroyDie-remove.
            if self.try_begin_keep_object_die_instead_of_destroy(id) {
                return;
            }
        }
        self.enqueue_object_destruction(id, killer);
    }

    /// C++ InstantDeathBehavior::onDie residual.
    pub(in super::super::super) fn try_apply_instant_death(&mut self, id: ObjectId) -> bool {
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
            if !obj.fire_instant_death() {
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
        self.apply_pending_create_object_die(id);
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
            if self.create_and_fire_temp_weapon(id, &spec).is_none() {
                let _ = self.apply_fire_weapon_when_damaged_named(id, &wpn);
            }
        }
        true
    }

    /// C++ KeepObjectDie residual: convert to lasting rubble, skip remove.
    pub(in super::super::super) fn try_begin_keep_object_die_instead_of_destroy(
        &mut self,
        id: ObjectId,
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
        if !obj.begin_keep_object_die(frame) {
            return false;
        }
        // Death FX / OCL peels without world removal.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.fire_fx_list_die();
            obj.fire_create_object_die();
        }
        self.apply_pending_create_object_die(id);
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
        for obj in self.objects.values_mut() {
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
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Jet crash residual. Ground/deck explode must not fall through to heli/slow.
        let is_jet =
            crate::game_logic::host_jet_slow_death::is_jet_slow_death_template(&obj.template_name)
                || obj.jet_slow_death.is_some();
        if is_jet {
            if obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false) {
                return false;
            }
            if obj
                .jet_slow_death
                .as_ref()
                .map(|j| j.is_active())
                .unwrap_or(false)
            {
                return true;
            }
            let deferred = obj.begin_jet_slow_death();
            return deferred;
        }
        // Helicopter spiral crash residual.
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.done)
            .unwrap_or(false)
        {
            return false;
        }
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            return true;
        }
        if obj.begin_helicopter_slow_death() {
            return true;
        }
        // Already finished slow death → allow destroy.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_done())
            .unwrap_or(false)
        {
            return false;
        }
        // Mid slow death → keep deferring.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            return true;
        }
        if obj.begin_slow_death(frame) {
            return true;
        }
        false
    }

    pub(crate) fn apply_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let building_team = self.objects.get(&building_id).map(|o| o.team);
        let crushing_fx = self
            .objects
            .get(&building_id)
            .and_then(|o| o.structure_topple_data.as_ref())
            .map(|d| d.crushing_fx.clone())
            .unwrap_or_default();
        if !crushing_fx.is_empty() {
            for s in &samples {
                // C++ StructureToppleUpdate.cpp:407-419 doDamageLine:
                // target.z = TheTerrainLogic->getGroundHeight(target.x, target.y).
                let sample_xz = glam::Vec3::new(s.x, 0.0, s.z);
                let height = self.terrain_height_at(sample_xz).unwrap_or(0.0);
                let _ = crate::game_logic::dispatch_fx_list_at_pos(
                    &crushing_fx,
                    glam::Vec3::new(s.x, height, s.z),
                );
            }
        }
        let mut destroy: Vec<ObjectId> = Vec::new();
        let victims: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in victims {
            if id == building_id {
                continue;
            }
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let pos = obj.get_position();
            let mut best_dmg = 0.0_f32;
            for s in &samples {
                let dx = pos.x - s.x;
                let dz = pos.z - s.z;
                let radius = s.radius.max(1.0);
                if dx * dx + dz * dz <= radius * radius {
                    best_dmg = best_dmg.max(s.damage);
                }
            }
            if best_dmg <= 0.0 {
                continue;
            }
            let killed = if let Some(obj) = self.objects.get_mut(&id) {
                // Structure topple crush residual is effectively unresistable for units
                // under the fall sweep (C++ doDamageLine lethality residual).
                let mut dead = obj.take_damage_from_typed_death_with_repulsor_policy(
                    best_dmg,
                    Some(building_id),
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                    &mut self.health_events,
                    &self.enable_repulsors,
                );
                if !dead && (obj.status.destroyed || obj.health.current <= 0.0) {
                    dead = true;
                }
                dead
            } else {
                false
            };
            if killed
                || self
                    .objects
                    .get(&id)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false)
            {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, building_team);
        }
    }

    /// C++ CreateObjectDie::onDie residual — spawn OCL templates at dying object.
    pub fn apply_pending_create_object_die(&mut self, dying_id: ObjectId) {
        let (spawns, transfer_dmg, transfer, subdual, source, team, owner_player_id, pos) = {
            let Some(o) = self.objects.get_mut(&dying_id) else {
                return;
            };
            let (spawns, dmg, transfer, subdual, source) =
                o.take_pending_create_object_die_spawns();
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
                let ids = self.spawn_ocl_create_debris(&plan, team, pos, inherit, owner_player_id);
                if transfer {
                    for id in &ids {
                        self.apply_death_transfer_damage(id, subdual, transfer_dmg, source);
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
            let Some(new_id) =
                self.create_object_for_owner_or_team(&tmpl, team, owner_player_id, pos)
            else {
                continue;
            };
            if let Some(dying) = self.objects.get(&dying_id) {
                let yaw = dying.get_orientation();
                if let Some(n) = self.objects.get_mut(&new_id) {
                    n.set_orientation(yaw);
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
                self.apply_death_transfer_damage(&new_id, subdual, transfer_dmg, source);
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
        self.apply_instant_hit_splash_at(
            pos,
            pd,
            sd,
            pr,
            sr,
            source_id,
            team,
            source_id,
            Some(weapon_name),
        )
    }

    pub(in super::super::super) fn try_begin_structure_topple_instead_of_destroy(
        &mut self,
        id: ObjectId,
    ) -> bool {
        let attacker_pos = {
            let src = self.objects.get(&id).and_then(|o| o.last_damage_source);
            src.and_then(|sid| {
                self.objects.get(&sid).map(|s| {
                    let p = s.get_position();
                    (p.x, p.z)
                })
            })
        };
        let frame = self.frame;
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
        if collapse_done || topple_done {
            return false;
        }
        // Mid-animation: keep deferring destroy.
        if obj
            .structure_collapse_data
            .as_ref()
            .map(|d| d.is_active())
            .unwrap_or(false)
            || obj
                .structure_topple_data
                .as_ref()
                .map(|d| d.is_active())
                .unwrap_or(false)
        {
            return true;
        }
        // Prefer StructureCollapse for civilian/prop peels; else StructureTopple.
        if obj.begin_structure_collapse(frame) {
            return true;
        }
        if obj.begin_structure_topple(frame, attacker_pos) {
            return true;
        }
        false
    }
}
