//! One projectile callback; borrows end before the receiving world's Body effects.
//! This supplies the Body boundary for CPP GameLogic.cpp3698-3736.
//! Detonation FX/OCL and complete UpdateModule ordering remain separate work.
use super::*;
use gamelogic::{common::Relationship, weapon::WeaponCollideMask};

impl Projectile {
    /// Weapon.cpp:642–731. The partition has already confirmed geometric contact.
    pub(in crate::game_logic) fn accepts_missile_contact(
        &self,
        objects: HostObjectRead<'_>,
        target: ObjectId,
        frame: u32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            projectile_collides_with_structure_or_fence, projectile_structure_same_controller,
        };

        if self.missile_motion.is_none()
            || self.missile_drawable_hidden()
            || !self.is_warhead_armed()
        {
            return false;
        }
        let (Some(missile), Some(other)) = (objects.lookup(&self.id), objects.lookup(&target)) else {
            return false;
        };
        if self.is_homing && self.target_id == Some(target) {
            return true;
        }
        if target == self.shooter_id
            || objects.lookup(&self.shooter_id).and_then(|launcher| launcher.contained_by)
                == Some(target)
        {
            return false;
        }
        if matches!(self.damage_type, DamageType::Flame | DamageType::ParticleBeam)
            && other.has_object_status_bit("BURNED")
        {
            return false;
        }
        if other.is_kind_of(KindOf::FSAirfield)
            && other.thing().template.parking_place.is_some()
            && self.is_homing
            && self.target_id.and_then(|id| objects.lookup(&id)).is_some_and(|victim| {
                victim.producer_id == Some(target) || victim.contained_by == Some(target)
            })
        {
            return false;
        }
        if other.get_sneaky_targeting_offset(frame).is_some() {
            return false;
        }
        let mask = WeaponCollideMask::new(self.projectile_collides);
        let same_controller = projectile_structure_same_controller(
            missile.owner_player_id,
            other.owner_player_id,
            missile.team == other.team,
        );
        let matches_kind = projectile_collides_with_structure_or_fence(
            mask.bits(),
            other.is_kind_of(KindOf::Structure),
            other.get_template().fence_width > 0.0,
            same_controller,
        ) || (mask.contains(WeaponCollideMask::SHRUBBERY) && other.is_kind_of(KindOf::Shrubbery))
            || (mask.contains(WeaponCollideMask::PROJECTILE) && other.is_kind_of(KindOf::Projectile))
            || (mask.contains(WeaponCollideMask::SMALL_MISSILES) && other.is_kind_of(KindOf::SmallMissile))
            || (mask.contains(WeaponCollideMask::BALLISTIC_MISSILES)
                && other.is_kind_of(KindOf::BallisticMissile));
        matches_kind
            || (mask.contains(WeaponCollideMask::ALLIES | WeaponCollideMask::ENEMIES)
                && match objects.relationship(missile, other) {
                    Some(Relationship::Allies) => mask.contains(WeaponCollideMask::ALLIES),
                    Some(Relationship::Enemies) => mask.contains(WeaponCollideMask::ENEMIES),
                    _ => false,
                })
    }
}

fn find_standalone_projectile_hit(
    projectile: &Projectile,
    objects: HostObjectRead<'_>,
    frame: u32,
) -> Option<ObjectId> {
    use crate::game_logic::weapon_bootstrap::{
        projectile_collides_structures_or_fences, projectile_collides_with_structure_or_fence,
        projectile_structure_same_controller,
    };

    let mask = projectile.projectile_collides;
    let intended = projectile.target_id;
    let intended_object = intended.and_then(|id| objects.lookup(&id));
    if projectile_collides_structures_or_fences(mask) {
        let launcher = objects.lookup(&projectile.shooter_id);
        let launcher_owner = launcher
            .and_then(|object| object.owner_player_id)
            .or(projectile.source_owner_player_id);
        let launcher_team = launcher.map_or(projectile.source_team, |object| object.team);
        let launcher_container = launcher.and_then(|object| object.contained_by);
        let skip_burned =
            matches!(projectile.damage_type, DamageType::Flame | DamageType::ParticleBeam);
        for (&id, other) in objects.registered() {
            if id == projectile.id
                || id == projectile.shooter_id
                || Some(id) == intended
                || Some(id) == launcher_container
                || !other.is_alive()
                || other.status.under_construction
            {
                continue;
            }
            let same_controller = projectile_structure_same_controller(
                launcher_owner,
                other.owner_player_id,
                launcher_team == other.team,
            );
            if !projectile_collides_with_structure_or_fence(
                mask,
                other.is_kind_of(KindOf::Structure),
                other.get_template().fence_width > 0.0,
                same_controller,
            ) || !standalone_projectile_overlaps(projectile, other)
                || (skip_burned && other.has_object_status_bit("BURNED"))
                || other.get_sneaky_targeting_offset(frame).is_some()
            {
                continue;
            }
            // Weapon.cpp:679–699. The resting target belongs to this airfield.
            if other.is_kind_of(KindOf::FSAirfield)
                && other.thing().template.parking_place.is_some()
                && intended_object.is_some_and(|target| {
                    target.producer_id == Some(id) || target.contained_by == Some(id)
                })
            {
                continue;
            }
            return Some(id);
        }
    }
    let target = intended_object?;
    (projectile.position.distance(target.get_position()) <= 5.0).then_some(target.id)
}

fn standalone_projectile_overlaps(projectile: &Projectile, other: &Object) -> bool {
    use crate::game_logic::HostGeometryType;

    let pos = projectile.position;
    let other_pos = other.get_position();
    let dx = pos.x - other_pos.x;
    let dz = pos.z - other_pos.z;
    if (dx * dx + dz * dz).sqrt() > other.selection_radius.max(8.0) {
        return false;
    }
    let Some(geometry) = projectile.collision_geometry else {
        return true;
    };
    let other_geometry = other.geometry_info();
    let below = |shape: &crate::game_logic::HostGeometryInfo| {
        if shape.geom_type == HostGeometryType::Sphere {
            shape.major_radius
        } else {
            0.0
        }
    };
    let top = pos.y + geometry.max_height_above_position();
    let bottom = pos.y - below(&geometry);
    let other_top = other_pos.y + other_geometry.max_height_above_position();
    let other_bottom = other_pos.y - below(other_geometry);
    top >= other_bottom && bottom <= other_top
}

fn mark_missile_detonated(
    objects: &mut crate::game_logic::HostObjectStore,
    projectile: &Projectile,
    collision: bool,
) {
    if let Some(actor) = objects.get_mut(&projectile.id) {
        if projectile.projectile_actor_birth.map(|birth| birth.0) == Some(actor.visual_object_generation) {
            actor.set_status_missile_killing_self(true);
            if collision {
                actor.status.no_collisions = true;
            }
        }
    }
}

impl CombatSystem {
    fn detonation_damage_event(
        projectile: &Projectile,
        objects: HostObjectRead<'_>,
        position: Vec3,
        contact: Option<ObjectId>,
    ) -> Option<DamageEvent> {
        if projectile.no_damage {
            return None;
        }
        if projectile.explosion_radius > 0.0 || projectile.secondary_damage_radius > 0.0 {
            return Some(Self::splash_area_event(projectile, objects, position, true));
        }
        // Weapon.cpp:1296–1306 applies a radius-zero SUICIDE hit directly to
        // the source Object, before the launcher attribution of the area loop.
        let (target_id, shooter_id, damage) = if projectile.flight.is_some() {
            if !Projectile::warhead_kills_source(projectile.radius_damage_affects)
                || projectile.projectile_actor_birth.is_none()
            {
                return None;
            }
            (projectile.id, projectile.id, HUGE_DAMAGE_AMOUNT)
        } else {
            (contact?, projectile.shooter_id, projectile.damage)
        };
        Some(DamageEvent::Direct {
            target_id,
            position,
            damage,
            damage_type: projectile.damage_type,
            death_type: projectile.death_type,
            shooter_id,
            damage_status_type: projectile.damage_status_type,
        })
    }

    pub(in crate::game_logic) fn prepare_projectile_impact(
        &mut self,
        proj_id: ObjectId,
        dt: f32,
        objects: &mut crate::game_logic::HostObjectStore,
        countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        historic_damage_limit: u32,
    ) -> (Vec<DamageEvent>, Vec<ObjectId>) {
        self.prepare_projectile_impact_with_fx(
            proj_id,
            dt,
            objects,
            countermeasures,
            frame,
            terrain,
            ownership_events,
            historic_damage_limit,
            &mut dispatch_projectile_fx_standalone,
        )
    }

    pub(in crate::game_logic) fn prepare_projectile_impact_with_fx(
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
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
    ) -> (Vec<DamageEvent>, Vec<ObjectId>) {
        self.prepare_projectile_impact_with_fx_and_pose(
            proj_id,
            dt,
            objects,
            countermeasures,
            frame,
            terrain,
            ownership_events,
            historic_damage_limit,
            fx,
            &mut |projectile| {
                crate::game_logic::host_fx_list_dispatch::publish_host_fx_object_with_visibility(
                    projectile.id.0,
                    projectile.position,
                    projectile.velocity.z.atan2(projectile.velocity.x),
                    projectile
                        .source_owner_player_id
                        .map(|id| id as i32)
                        .unwrap_or(-1),
                    0.0,
                    crate::game_logic::host_fx_list_dispatch::host_object_is_shrouded_for_local(
                        projectile.id.0,
                    ),
                );
            },
        )
    }

    pub(in crate::game_logic) fn prepare_projectile_impact_with_fx_and_pose(
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
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
        publish: &mut dyn FnMut(&Projectile),
    ) -> (Vec<DamageEvent>, Vec<ObjectId>) {
        let (events, retired, _) = self.prepare_projectile_impact_with_runtime(
            proj_id,
            ProjectileCallback::Update(dt),
            objects,
            countermeasures,
            frame,
            terrain,
            None,
            ownership_events,
            historic_damage_limit,
            None,
            &mut ProjectileFxCallbacks { fx, pose: publish },
        );
        (events, retired)
    }
    pub(in crate::game_logic) fn prepare_projectile_impact_with_runtime(
        &mut self,
        proj_id: ObjectId,
        callback: ProjectileCallback,
        objects: &mut crate::game_logic::HostObjectStore,
        mut countermeasures: Option<
            &mut crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,
        >,
        frame: u32,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        mut partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
        ownership_events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
        historic_damage_limit: u32,
        receiving_objects: Option<&crate::assets::ReceivingObjectDefinitions>,
        runtime: &mut dyn ProjectilePresentationRuntime,
    ) -> (Vec<DamageEvent>, Vec<ObjectId>, Option<Vec3>) {
        let mut damage_events = Vec::new();
        let mut projectiles_to_remove = Vec::new();
        let mut detonation_pose = None;
        if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
            if projectile.missile_motion.is_some()
                && !objects.get(&proj_id).is_some_and(|actor| {
                    projectile.projectile_actor_birth.map(|birth| birth.0)
                        == Some(actor.visual_object_generation)
                })
            {
                projectiles_to_remove.push(proj_id);
                return (damage_events, projectiles_to_remove, detonation_pose);
            }
            let object_hit = match callback {
                ProjectileCallback::ObjectContact { target, pose } => {
                    projectile.position = pose;
                    Some(target)
                }
                ProjectileCallback::Update(dt) => {
                    if let Some(reg) = countermeasures.as_mut() {
                        apply_countermeasure_report_and_decoy(
                            projectile,
                            proj_id,
                            objects.read(),
                            reg,
                            frame,
                        );
                    }
                    let target = projectile
                        .target_id
                        .and_then(|target_id| objects.get(&target_id));
                    let target_exists = projectile.target_id.is_none() || target.is_some();
                    // CPP getGoalObject uses registry presence even for a dead object.
                    // Refresh the registered target pose before this callback steps.
                    // C++ AIMoveToState::onEnter lifts projectile goals by target height.
                    if projectile.is_homing && let Some(target) = target {
                        projectile.target_position = projectile_goal_on_object(target);
                    }
                    let kill_target = if projectile.is_homing && projectile.completed_missile_kill_move() {
                        target.zip(objects.get(&proj_id)).map(|(target, missile)| {
                            use crate::game_logic::partition_geometry::bounding_spheres_distance_squared;
                            MissileKillTarget {
                                origin: target.get_position(),
                                edge_distance_squared: bounding_spheres_distance_squared(
                                    missile.get_position(),
                                    missile.geometry_info(),
                                    target.get_position(),
                                    target.geometry_info(),
                                ),
                            }
                        })
                    } else {
                        None
                    };
                    let mut retarget_failed = false;
                    if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(dumb)) =
                        projectile.flight.as_ref()
                    {
                        if dumb.flight_path_adjust_per_frame > 0.0 && let Some(tgt) = target {
                            retarget_failed = !crate::game_logic::weapon_bootstrap::adjust_flight_path_end(
                                &mut projectile.flight_runtime,
                                dumb,
                                dumb_projectile_target_center(tgt),
                                |start, end| partition.as_deref().and_then(|owner| owner.terrain_high_along_line(start, end)),
                            );
                            projectile.target_position = projectile.flight_runtime.path_end;
                        }
                    }
                    let step = if retarget_failed {
                        ProjectileStep::Detonate
                    } else {
                        let actor = if projectile.missile_motion.is_some() {
                            let expected = projectile.projectile_actor_birth.map(|birth| birth.0);
                            match objects.get_mut(&proj_id) {
                                Some(actor) if Some(actor.visual_object_generation) == expected => Some(actor),
                                _ => {
                                    projectiles_to_remove.push(proj_id);
                                    return (damage_events, projectiles_to_remove, detonation_pose);
                                }
                            }
                        } else {
                            None
                        };
                        projectile.update_with_terrain_runtime(
                            dt, target_exists, kill_target, terrain, actor,
                            partition.as_deref_mut(), Some(frame), runtime,
                        )
                    };
                    match step {
                        ProjectileStep::Alive => {
                            if !projectile.is_warhead_armed() {
                                return (damage_events, projectiles_to_remove, detonation_pose);
                            }
                        }
                        ProjectileStep::Hold => {
                            // Detonation disables further collisions, but a missile
                            // holding after target loss remains armed and collidable.
                            if projectile.missile_drawable_hidden() || !projectile.is_warhead_armed() {
                                return (damage_events, projectiles_to_remove, detonation_pose);
                            }
                        }
                        ProjectileStep::Remove => {
                            projectiles_to_remove.push(proj_id);
                            return (damage_events, projectiles_to_remove, detonation_pose);
                        }
                        step @ (ProjectileStep::Detonate | ProjectileStep::DetonateAndHold) => {
                            if step == ProjectileStep::DetonateAndHold {
                                mark_missile_detonated(objects, projectile, false);
                            }
                            // C++ DumbProjectileBehavior lifespan and
                            // MissileAIUpdate DetonateOnNoFuel both call the
                            // detonation weapon at the projectile's current pose.
                            // A zero-radius weapon has no guessed direct victim.
                            let impact = projectile.detonation_position.take().unwrap_or(projectile.position);
                            detonation_pose = Some(impact);
                            Self::maybe_record_historic_bonus(
                                &mut self.historic_bonus,
                                frame,
                                historic_damage_limit,
                                projectile,
                                impact,
                                objects.read(),
                            );
                            damage_events.extend(Self::detonation_damage_event(
                                projectile, objects.read(), impact, None,
                            ));
                            if !projectile.detonation_fx_name.is_empty()
                                || !projectile.detonation_ocl_name.is_empty()
                            {
                                self.impact_fx.push(ProjectileImpactFx {
                                    position: impact,
                                    shooter_id: projectile.shooter_id,
                                    detonation_projectile: Some(projectile.id),
                                    target_id: None,
                                    detonation_fx_name: projectile.detonation_fx_name.clone(),
                                    detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                                    source_team: projectile.source_team,
                                    source_veterancy: projectile.source_veterancy,
                                    source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                                    source_velocity: projectile.velocity,
                                    projectile_die_effects: Vec::new(),
                                });
                            }
                            Self::queue_missile_calls_on_die(
                                &mut self.impact_fx,
                                projectile,
                                receiving_objects,
                            );

                            if step == ProjectileStep::Detonate {
                                projectiles_to_remove.push(proj_id);
                            }
                            return (damage_events, projectiles_to_remove, detonation_pose);
                        }
                    }
                    // Authored missile object contacts arrive from PartitionManager,
                    // after the movement of all actors has been observed.
                    if projectile.missile_motion.is_some() {
                        return (damage_events, projectiles_to_remove, detonation_pose);
                    }
                    find_standalone_projectile_hit(projectile, objects.read(), frame)
                }
            };
            if let Some(target_id) = object_hit {
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
                            objects.read(),
                            target_id,
                            flight.garrison_hit_kill_fx(),
                            &mut |name, object| runtime.fx(name, object),
                        );
                        projectiles_to_remove.push(proj_id);
                        return (damage_events, projectiles_to_remove, detonation_pose);
                    }
                }
                let impact = projectile.position;
                detonation_pose = Some(impact);
                Self::maybe_record_historic_bonus(
                    &mut self.historic_bonus,
                    frame,
                    historic_damage_limit,
                    projectile,
                    impact,
                    objects.read(),
                );
                damage_events.extend(Self::detonation_damage_event(
                    projectile, objects.read(), impact, Some(target_id),
                ));
                if !projectile.detonation_fx_name.is_empty()
                    || !projectile.detonation_ocl_name.is_empty()
                {
                    self.impact_fx.push(ProjectileImpactFx {
                        position: impact,
                        shooter_id: projectile.shooter_id,
                        detonation_projectile: Some(projectile.id),
                        target_id: projectile.flight.is_none().then_some(target_id),
                        detonation_fx_name: projectile.detonation_fx_name.clone(),
                        detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                        source_team: projectile.source_team,
                        source_veterancy: projectile.source_veterancy,
                        source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                        source_velocity: projectile.velocity,
                        projectile_die_effects: Vec::new(),
                    });
                }
                Self::queue_missile_calls_on_die(
                    &mut self.impact_fx,
                    projectile,
                    receiving_objects,
                );

                if !projectile.retain_detonated_missile() {
                    projectiles_to_remove.push(proj_id);
                } else {
                    mark_missile_detonated(objects, projectile, true);
                }
                return (damage_events, projectiles_to_remove, detonation_pose);
            }

            if projectile.target_id.is_none() {
                // Check ground impact
                let distance = projectile.position.distance(projectile.target_position);
                if distance <= 2.0 {
                    let impact = projectile.target_position;
                    detonation_pose = Some(projectile.position);
                    Self::maybe_record_historic_bonus(
                        &mut self.historic_bonus,
                        frame,
                        historic_damage_limit,
                        projectile,
                        impact,
                        objects.read(),
                    );
                    damage_events.extend(Self::detonation_damage_event(
                        projectile, objects.read(), impact, None,
                    ));
                    if !projectile.detonation_fx_name.is_empty()
                        || !projectile.detonation_ocl_name.is_empty()
                    {
                        self.impact_fx.push(ProjectileImpactFx {
                            position: impact,
                            shooter_id: projectile.shooter_id,
                            detonation_projectile: Some(projectile.id),
                            target_id: None,
                            detonation_fx_name: projectile.detonation_fx_name.clone(),
                            detonation_ocl_name: projectile.detonation_ocl_name.clone(),
                            source_team: projectile.source_team,
                            source_veterancy: projectile.source_veterancy,
                            source_orientation: projectile.velocity.z.atan2(projectile.velocity.x),
                            source_velocity: projectile.velocity,
                            projectile_die_effects: Vec::new(),
                        });
                    }
                    Self::queue_missile_calls_on_die(
                        &mut self.impact_fx,
                        projectile,
                        receiving_objects,
                    );

                    if !projectile.retain_detonated_missile() {
                        projectiles_to_remove.push(proj_id);
                    } else {
                        mark_missile_detonated(objects, projectile, true);
                    }
                }
            }
        }
        (damage_events, projectiles_to_remove, detonation_pose)
    }
}
