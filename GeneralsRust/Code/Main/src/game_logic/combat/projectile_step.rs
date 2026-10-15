//! One projectile callback; borrows end before the receiving world's Body effects.
//! This supplies the Body boundary for CPP GameLogic.cpp3698-3736.
//! Detonation FX/OCL and complete UpdateModule ordering remain separate work.
use super::*;

impl CombatSystem {
    pub(in crate::game_logic) fn prepare_projectile_impact(
        &mut self,
        proj_id: ObjectId,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        historic_damage_limit: u32,
    ) -> (Vec<DamageEvent>, Vec<ObjectId>) {
        let mut damage_events = Vec::new();
        let mut projectiles_to_remove = Vec::new();
        if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
            if let Some(reg) = countermeasures.as_mut() {
                apply_countermeasure_report_and_decoy(projectile, proj_id, objects, reg, frame);
            }
            let target_exists = projectile
                .target_id
                .map(|target_id| objects.contains_key(&target_id))
                .unwrap_or(true);
            // CPP getGoalObject uses registry presence even for a dead object.
            // Refresh the registered target pose before this callback steps.
            // C++ AIMoveToState::onEnter lifts projectile goals by target height.
            if projectile.is_homing {
                if let Some(tid) = projectile.target_id {
                    if let Some(tgt) = objects.get(&tid) {
                        projectile.target_position = projectile_goal_on_object(tgt);
                    }
                }
            }
            if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb)) =
                projectile.flight.as_ref()
            {
                if dumb.flight_path_adjust_per_frame > 0.0 {
                    if let Some(tid) = projectile.target_id {
                        if let Some(tgt) = objects.get(&tid) {
                            crate::game_logic::weapon_bootstrap::adjust_flight_path_end(
                                &mut projectile.flight_runtime,
                                dumb,
                                tgt.get_position(),
                            );
                            projectile.target_position = projectile.flight_runtime.path_end;
                        }
                    }
                }
            }
            match projectile.update_with_terrain(dt, target_exists, terrain) {
                ProjectileStep::Alive => {
                    if !projectile.is_warhead_armed() {
                        return (damage_events, projectiles_to_remove);
                    }
                }
                ProjectileStep::Hold => {
                    // C++ MissileAIUpdate::KILL_SELF holds for the parsed
                    // contrail delay, with no impact FX/OCL or damage.
                    return (damage_events, projectiles_to_remove);
                }
                ProjectileStep::Remove => {
                    projectiles_to_remove.push(proj_id);
                    return (damage_events, projectiles_to_remove);
                }
                step @ (ProjectileStep::Detonate | ProjectileStep::DetonateAndHold) => {
                    // C++ DumbProjectileBehavior lifespan and
                    // MissileAIUpdate DetonateOnNoFuel both call the
                    // detonation weapon at the projectile's current pose.
                    // A zero-radius weapon has no guessed direct victim.
                    let impact = projectile.position;
                    Self::maybe_record_historic_bonus(
                        &mut self.historic_bonus,
                        frame,
                        historic_damage_limit,
                        projectile,
                        impact,
                        objects,
                    );
                    if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                        damage_events.push(Self::splash_area_event(projectile, objects, impact));
                    }
                    if !projectile.detonation_fx_name.is_empty()
                        || !projectile.detonation_ocl_name.is_empty()
                    {
                        self.impact_fx.push(ProjectileImpactFx {
                            position: impact,
                            shooter_id: projectile.shooter_id,
                            target_id: None,
                            detonation_fx_name: projectile.detonation_fx_name.clone(),
                            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                            source_team: projectile.source_team,
                            source_veterancy: projectile.source_veterancy,
                            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                            source_velocity: projectile.velocity,
                        });
                    }
                    Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                    if step == ProjectileStep::Detonate {
                        projectiles_to_remove.push(proj_id);
                    }
                    return (damage_events, projectiles_to_remove);
                }
            }

            // Intervening structure residual: ballistic shells impact the first
            // constructed building whose footprint contains the projectile.
            // Gated by Weapon.ini ProjectileCollidesWith leftover mask.
            // C++ Weapon.cpp:716-721 — same-controller structures only if
            // CONTROLLED_STRUCTURES; STRUCTURES is everyone else.
            // Skip intended target (handled below) and shooter.
            let mut hit_structure: Option<ObjectId> = None;
            let collides_structures =
                crate::game_logic::weapon_bootstrap::projectile_collides_structures(
                    projectile.projectile_collides,
                );
            if collides_structures {
                let shooter = projectile.shooter_id;
                let intended = projectile.target_id;
                let pos = projectile.position;
                let sneak_now = frame;
                let shooter_obj = objects.get(&shooter);
                let proj_owner = shooter_obj
                    .and_then(|s| s.owner_player_id)
                    .or(projectile.source_owner_player_id);
                let proj_team = shooter_obj
                    .map(|s| s.team)
                    .unwrap_or(projectile.source_team);
                // C++ Weapon.cpp:663-666 — never collide with the launcher's container.
                let launcher_contained_by = shooter_obj.and_then(|s| s.contained_by);
                // C++ Weapon.cpp:670-677 — Flame/ParticleBeam skip already-burned.
                let skip_burned = matches!(
                    projectile.damage_type,
                    DamageType::Flame | DamageType::ParticleBeam
                );
                for (&oid, obj) in objects.iter() {
                    if oid == shooter || Some(oid) == intended {
                        continue;
                    }
                    if launcher_contained_by == Some(oid) {
                        continue;
                    }
                    if skip_burned && obj.has_object_status_bit("BURNED") {
                        continue;
                    }
                    if obj.get_sneaky_targeting_offset(sneak_now).is_some() {
                        continue;
                    }
                    if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                        continue;
                    }
                    if obj.status.under_construction {
                        continue;
                    }
                    let same_controller =
                        crate::game_logic::weapon_bootstrap::projectile_structure_same_controller(
                            proj_owner,
                            obj.owner_player_id,
                            obj.team == proj_team,
                        );
                    if !crate::game_logic::weapon_bootstrap::projectile_collides_with_structure(
                        projectile.projectile_collides,
                        same_controller,
                    ) {
                        continue;
                    }
                    // C++ Weapon.cpp:679-699 — only skip KINDOF_FS_AIRFIELD
                    // when the intended victim reserved a parking space.
                    // Other buildings still detonate AA / airborne shots.
                    if obj.is_kind_of(KindOf::FSAirfield)
                        && obj.thing().template.parking_place.is_some()
                    {
                        if let Some(tid) = intended {
                            if let Some(t) = objects.get(&tid) {
                                if t.producer_id == Some(oid) || t.contained_by == Some(oid) {
                                    continue;
                                }
                            }
                        }
                    }
                    let radius = obj.selection_radius.max(8.0);
                    // Horizontal (XZ) distance — tall buildings block regardless of Y.
                    let op = obj.get_position();
                    let dx = pos.x - op.x;
                    let dz = pos.z - op.z;
                    if (dx * dx + dz * dz).sqrt() <= radius {
                        hit_structure = Some(oid);
                        break;
                    }
                }
            }
            if let Some(sid) = hit_structure {
                if let Some(flight) = projectile.flight.as_ref() {
                    if crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                        objects,
                        sid,
                        projectile.shooter_id,
                        flight,
                        ownership_events,
                    )
                    .is_some()
                    {
                        Self::play_garrison_hit_kill_fx(
                            objects,
                            sid,
                            flight.garrison_hit_kill_fx(),
                        );
                        projectiles_to_remove.push(proj_id);
                        return (damage_events, projectiles_to_remove);
                    }
                }
                let impact = projectile.position;
                Self::maybe_record_historic_bonus(
                    &mut self.historic_bonus,
                    frame,
                    historic_damage_limit,
                    projectile,
                    impact,
                    objects,
                );
                if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                    damage_events.push(Self::splash_area_event(projectile, objects, impact));
                } else if !projectile.no_damage {
                    damage_events.push(DamageEvent::Direct {
                        target_id: sid,
                        position: impact,
                        damage: projectile.damage,
                        damage_type: projectile.damage_type,
                        death_type: projectile.death_type,

                        shooter_id: projectile.shooter_id,
                    });
                }
                if !projectile.detonation_fx_name.is_empty()
                    || !projectile.detonation_ocl_name.is_empty()
                {
                    self.impact_fx.push(ProjectileImpactFx {
                        position: impact,
                        shooter_id: projectile.shooter_id,
                        target_id: Some(sid),
                        detonation_fx_name: projectile.detonation_fx_name.clone(),
                        detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                        source_team: projectile.source_team,
                        source_veterancy: projectile.source_veterancy,
                        source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                        source_velocity: projectile.velocity,
                    });
                }
                Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                projectiles_to_remove.push(proj_id);
                return (damage_events, projectiles_to_remove);
            }

            // Check for hits
            if let Some(target_id) = projectile.target_id {
                if let Some(target) = objects.get(&target_id) {
                    let distance = projectile.position.distance(target.get_position());
                    if distance <= 5.0 {
                        if let Some(flight) = projectile.flight.as_ref() {
                            if crate::game_logic::weapon_bootstrap::apply_garrison_hit_kill(
                                objects,
                                target_id,
                                projectile.shooter_id,
                                flight,
                                ownership_events,
                            )
                            .is_some()
                            {
                                Self::play_garrison_hit_kill_fx(
                                    objects,
                                    target_id,
                                    flight.garrison_hit_kill_fx(),
                                );
                                projectiles_to_remove.push(proj_id);
                                return (damage_events, projectiles_to_remove);
                            }
                        }
                        let impact = projectile.position;
                        Self::maybe_record_historic_bonus(
                            &mut self.historic_bonus,
                            frame,
                            historic_damage_limit,
                            projectile,
                            impact,
                            objects,
                        );
                        if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                            // C++ Weapon.cpp:1438: primary inside primaryRadius, else secondary.
                            damage_events
                                .push(Self::splash_area_event(projectile, objects, impact));
                        } else if !projectile.no_damage {
                            damage_events.push(DamageEvent::Direct {
                                target_id,
                                position: impact,
                                damage: projectile.damage,
                                damage_type: projectile.damage_type,
                                death_type: projectile.death_type,

                                shooter_id: projectile.shooter_id,
                            });
                        }
                        if !projectile.detonation_fx_name.is_empty()
                            || !projectile.detonation_ocl_name.is_empty()
                        {
                            self.impact_fx.push(ProjectileImpactFx {
                                position: impact,
                                shooter_id: projectile.shooter_id,
                                target_id: Some(target_id),
                                detonation_fx_name: projectile.detonation_fx_name.clone(),
                                detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                source_team: projectile.source_team,
                                source_veterancy: projectile.source_veterancy,
                                source_orientation: projectile
                                    .velocity
                                    .z
                                    .atan2(projectile.velocity.x),
                                source_velocity: projectile.velocity,
                            });
                        }
                        Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                        projectiles_to_remove.push(proj_id);
                    }
                }
            } else {
                // Check ground impact
                let distance = projectile.position.distance(projectile.target_position);
                if distance <= 2.0 {
                    let impact = projectile.target_position;
                    Self::maybe_record_historic_bonus(
                        &mut self.historic_bonus,
                        frame,
                        historic_damage_limit,
                        projectile,
                        impact,
                        objects,
                    );
                    if !projectile.no_damage && projectile.explosion_radius > 0.0 {
                        damage_events.push(Self::splash_area_event(projectile, objects, impact));
                    }
                    if !projectile.detonation_fx_name.is_empty()
                        || !projectile.detonation_ocl_name.is_empty()
                    {
                        self.impact_fx.push(ProjectileImpactFx {
                            position: impact,
                            shooter_id: projectile.shooter_id,
                            target_id: None,
                            detonation_fx_name: projectile.detonation_fx_name.clone(),
                            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                            source_team: projectile.source_team,
                            source_veterancy: projectile.source_veterancy,
                            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                            source_velocity: projectile.velocity,
                        });
                    }
                    Self::queue_missile_calls_on_die(&mut self.impact_fx, projectile);

                    projectiles_to_remove.push(proj_id);
                }
            }
        }
        (damage_events, projectiles_to_remove)
    }
}
