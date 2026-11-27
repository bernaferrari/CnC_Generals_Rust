//! Adapt the receiving world's projectile callback to simulation impact rules.
//! Borrows end before the world handles Body damage and detonation FX/OCL.
use super::*;
use generals_sim::{
    combat::{
        projectile_contact::{
            ProjectileCollision, ProjectileContactObject, VerticalExtents, VirtualProjectileContact,
        },
        projectile_impact::{
            ImpactCause, ProjectileAfterUpdate, ProjectileImpact, ProjectileWarhead,
        },
    },
    weapon::{WeaponAffectsMask, WeaponCollideMask},
};

#[derive(Default)]
pub(in crate::game_logic) struct PreparedProjectileImpact {
    pub damage: Vec<DamageEvent>,
    pub retired: Vec<ObjectId>,
    pub detonation_pose: Option<Vec3>,
    pub virtual_death: Option<VirtualProjectileDeath>,
}

impl Projectile {
    /// A registered actor is eligible for contact once its warhead is armed.
    pub(in crate::game_logic) fn accepts_owned_contact(
        &self,
        objects: HostObjectRead<'_>,
        target: ObjectId,
        frame: u32,
    ) -> bool {
        if self.missile_drawable_hidden() || !self.is_warhead_armed() {
            return false;
        }
        let mut collision = self.collision();
        collision.intended = collision
            .intended
            .filter(|_| self.is_homing || self.is_dumb_flight());
        collision.accepts_object(
            target,
            frame,
            |id| objects.lookup(&id),
            |projectile, other| objects.relationship(projectile, other),
        )
    }

    fn collision(&self) -> ProjectileCollision {
        ProjectileCollision {
            id: self.id,
            launcher: self.shooter_id,
            intended: self.target_id,
            mask: WeaponCollideMask::new(self.projectile_collides),
            damage_type: self.damage_type,
        }
    }

    fn find_virtual_contact(&self, objects: HostObjectRead<'_>, frame: u32) -> Option<ObjectId> {
        self.collision().find_virtual_contact(
            VirtualProjectileContact {
                position: self.position,
                vertical_extents: self.collision_geometry.as_ref().map(vertical_extents),
                launch_controller: (self.source_owner_player_id, self.source_team),
            },
            frame,
            |id| objects.lookup(&id),
            || objects.registered(),
        )
    }

    fn warhead(&self) -> ProjectileWarhead {
        ProjectileWarhead {
            projectile: self.id,
            launcher: self.shooter_id,
            authored_flight: self.flight.is_some(),
            has_registered_actor: self.projectile_actor_birth.is_some(),
            no_damage: self.no_damage,
            damage: self.damage,
            damage_type: self.damage_type,
            death_type: self.death_type,
            damage_status_type: self.damage_status_type,
            primary_radius: self.explosion_radius,
            secondary_radius: self.secondary_damage_radius,
            affects: WeaponAffectsMask::new(self.radius_damage_affects),
        }
    }
}

impl ProjectileContactObject for Object {
    fn object_id(&self) -> ObjectId {
        self.id
    }

    fn position(&self) -> Vec3 {
        self.get_position()
    }

    fn controller(&self) -> (Option<u32>, Team) {
        (self.owner_player_id, self.team)
    }

    fn contained_by(&self) -> Option<ObjectId> {
        self.contained_by
    }

    fn producer(&self) -> Option<ObjectId> {
        self.producer_id
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.is_kind_of(kind)
    }

    fn is_fence(&self) -> bool {
        self.get_template().fence_width > 0.0
    }

    fn has_parking_place(&self) -> bool {
        self.thing().template.parking_place.is_some()
    }

    fn is_burned(&self) -> bool {
        self.has_object_status_bit("BURNED")
    }

    fn is_sneaky_at(&self, frame: u32) -> bool {
        self.get_sneaky_targeting_offset(frame).is_some()
    }

    fn is_alive(&self) -> bool {
        self.is_alive()
    }

    fn is_under_construction(&self) -> bool {
        self.status.under_construction
    }

    fn selection_radius(&self) -> f32 {
        self.selection_radius
    }

    fn vertical_extents(&self) -> VerticalExtents {
        vertical_extents(self.geometry_info())
    }
}

fn vertical_extents(shape: &crate::game_logic::HostGeometryInfo) -> VerticalExtents {
    VerticalExtents {
        below: if shape.geom_type == crate::game_logic::HostGeometryType::Sphere {
            shape.major_radius
        } else {
            0.0
        },
        above: shape.max_height_above_position(),
    }
}

fn mark_missile_detonated(
    objects: &mut crate::game_logic::HostObjectStore,
    projectile: &Projectile,
    collision: bool,
) {
    if let Some(actor) = objects.get_mut(&projectile.id) {
        if projectile.projectile_actor_birth.map(|birth| birth.0)
            == Some(actor.visual_object_generation)
        {
            actor.set_status_missile_killing_self(true);
            if collision {
                actor.status.no_collisions = true;
            }
        }
    }
}

impl CombatSystem {
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
        let impact = self.prepare_projectile_impact_with_runtime(
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
        (impact.damage, impact.retired)
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
    ) -> PreparedProjectileImpact {
        let mut result = PreparedProjectileImpact::default();
        if let Some(projectile) = self.projectiles.get_mut(&proj_id) {
            if projectile.missile_motion.is_some()
                && !objects.get(&proj_id).is_some_and(|actor| {
                    projectile.projectile_actor_birth.map(|birth| birth.0)
                        == Some(actor.visual_object_generation)
                })
            {
                result.retired.push(proj_id);
                return result;
            }
            let impact = match callback {
                ProjectileCallback::ObjectContact { target, pose } => {
                    projectile.position = pose;
                    Some(ProjectileImpact::object_contact(pose, target))
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
                    if projectile.is_homing
                        && let Some(target) = target
                    {
                        projectile.target_position = projectile_goal_on_object(target);
                    }
                    let kill_target = if projectile.is_homing
                        && projectile.completed_missile_kill_move()
                    {
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
                    if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(
                        dumb,
                    )) = projectile.flight.as_ref()
                    {
                        if dumb.flight_path_adjust_per_frame > 0.0
                            && let Some(tgt) = target
                        {
                            retarget_failed =
                                !crate::game_logic::weapon_bootstrap::adjust_flight_path_end(
                                    &mut projectile.flight_runtime,
                                    dumb,
                                    dumb_projectile_target_center(tgt),
                                    |start, end| {
                                        partition.as_deref().and_then(|owner| {
                                            owner.terrain_high_along_line(start, end)
                                        })
                                    },
                                );
                            projectile.target_position = projectile.flight_runtime.path_end;
                        }
                    }
                    let step = if retarget_failed {
                        ProjectileStep::DETONATE
                    } else {
                        let actor = if projectile.missile_motion.is_some() {
                            let expected = projectile.projectile_actor_birth.map(|birth| birth.0);
                            match objects.get_mut(&proj_id) {
                                Some(actor) if Some(actor.visual_object_generation) == expected => {
                                    Some(actor)
                                }
                                _ => {
                                    result.retired.push(proj_id);
                                    return result;
                                }
                            }
                        } else {
                            None
                        };
                        projectile.update_with_terrain_runtime(
                            dt,
                            target_exists,
                            kill_target,
                            terrain,
                            actor,
                            partition.as_deref_mut(),
                            Some(frame),
                            runtime,
                        )
                    };
                    match ProjectileAfterUpdate::after_flight(
                        step,
                        projectile.position,
                        projectile.is_warhead_armed(),
                        projectile.missile_drawable_hidden(),
                        projectile.projectile_actor_birth.is_some(),
                    ) {
                        ProjectileAfterUpdate::Wait => return result,
                        ProjectileAfterUpdate::Retire => {
                            result.retired.push(proj_id);
                            return result;
                        }
                        ProjectileAfterUpdate::SearchVirtualContact => {
                            ProjectileImpact::after_virtual_step(
                                projectile.position,
                                projectile.target_position,
                                projectile.target_id.is_some(),
                                projectile.find_virtual_contact(objects.read(), frame),
                            )
                        }
                        ProjectileAfterUpdate::Detonate(impact) => Some(impact),
                    }
                }
            };
            let Some(impact) = impact else {
                return result;
            };
            let contact = impact.contacted_object();
            if let Some(target_id) = contact {
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
                        result.retired.push(proj_id);
                        return result;
                    }
                }
            }
            if matches!(
                impact.cause,
                ImpactCause::Flight {
                    retain_actor: true,
                    ..
                }
            ) {
                mark_missile_detonated(objects, projectile, false);
            }
            result.detonation_pose = Some(impact.actor_position);
            Self::maybe_record_historic_bonus(
                &mut self.historic_bonus,
                frame,
                historic_damage_limit,
                projectile,
                impact.weapon_position,
                objects.read(),
            );
            result
                .damage
                .extend(projectile.warhead().detonation_damage(&impact, |position| {
                    Self::splash_area_event(projectile, objects.read(), position, true)
                }));
            self.impact_fx.extend(ProjectileImpactFx::weapon_detonation(
                projectile,
                impact.weapon_position,
                contact.filter(|_| projectile.flight.is_none()),
                impact.pre_physics_pose(),
            ));
            result.virtual_death =
                Self::collect_virtual_projectile_death(projectile, receiving_objects);

            let retain_actor = match impact.cause {
                ImpactCause::Flight { retain_actor, .. } => retain_actor,
                ImpactCause::Object(_) | ImpactCause::Ground => {
                    let retain_actor = projectile.retain_detonated_missile();
                    if retain_actor {
                        mark_missile_detonated(objects, projectile, true);
                    }
                    retain_actor
                }
            };
            if !retain_actor {
                result.retired.push(proj_id);
            }
        }
        result
    }
}
