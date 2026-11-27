use generals_sim::combat::projectile::{MissileKillTarget, ProjectileFlightWorld};
use generals_sim::combat::projectile_flight::{ProjectileStep, projectile_forward_transform};

pub use generals_sim::combat::projectile::{Projectile, ProjectileStreamBinding};

/// Event-time object borrowed only for the synchronous FX invocation.
pub(in crate::game_logic) enum ProjectileFxObject<'a> {
    Projectile(&'a Projectile),
    Object(&'a Object),
}

pub(in crate::game_logic) trait ProjectilePresentationRuntime {
    fn fx(&mut self, name: &str, object: ProjectileFxObject<'_>);
    fn pose(&mut self, projectile: &Projectile);
    fn attach_exhaust(&mut self, projectile: &Projectile) -> Option<u32>;
}
struct ProjectileFxCallbacks<'a> {
    fx: &'a mut dyn FnMut(&str, ProjectileFxObject<'_>),
    pose: &'a mut dyn FnMut(&Projectile),
}
impl ProjectilePresentationRuntime for ProjectileFxCallbacks<'_> {
    fn fx(&mut self, name: &str, object: ProjectileFxObject<'_>) {
        (self.fx)(name, object);
    }
    fn pose(&mut self, p: &Projectile) {
        (self.pose)(p);
    }
    // Explicit low-level adapter has no native particle manager recipient.
    fn attach_exhaust(&mut self, _: &Projectile) -> Option<u32> {
        None
    }
}
impl ProjectilePresentationRuntime for () {
    fn fx(&mut self, _: &str, _: ProjectileFxObject<'_>) {}
    fn pose(&mut self, _: &Projectile) {}
    fn attach_exhaust(&mut self, _: &Projectile) -> Option<u32> {
        None
    }
}

fn dispatch_projectile_fx_standalone(name: &str, object: ProjectileFxObject<'_>) {
    let (id, position, yaw, owner) = match object {
        ProjectileFxObject::Projectile(p) => (
            p.id,
            p.position,
            p.velocity.z.atan2(p.velocity.x),
            p.source_owner_player_id,
        ),
        ProjectileFxObject::Object(o) => (
            o.id,
            o.get_position(),
            o.get_orientation(),
            o.owner_player_id,
        ),
    };
    crate::game_logic::publish_host_fx_object(
        id.0,
        position,
        yaw,
        owner.map(|p| p as i32).unwrap_or(-1),
    );
    crate::game_logic::dispatch_fx_list_at_object(name, id.0, None);
}

struct ProjectileWorld<'borrow, 'terrain> {
    actor: Option<&'borrow mut Object>,
    terrain: Option<&'borrow crate::game_logic::game_logic::FlightTerrainView<'terrain>>,
    partition: Option<&'borrow mut crate::game_logic::partition_manager::PartitionManager>,
    presentation: &'borrow mut dyn ProjectilePresentationRuntime,
}

impl ProjectileWorld<'_, '_> {
    fn actor(&self) -> &Object {
        self.actor
            .as_deref()
            .expect("missile motion requires its world Object")
    }
}

impl ProjectileFlightWorld for ProjectileWorld<'_, '_> {
    fn missile_kinematics(&self) -> Option<(Vec3, Vec3)> {
        self.actor
            .as_deref()
            .map(|actor| (actor.get_position(), actor.movement.velocity))
    }

    fn missile_transform(&self) -> Option<glam::Mat4> {
        self.actor.as_deref().map(Object::get_transform_matrix)
    }

    fn missile_max_speed(&self) -> f32 {
        self.actor().movement.max_speed
    }

    fn missile_close_enough_distance(&self) -> f32 {
        self.actor().host_close_enough_dist()
    }

    fn set_missile_position(&mut self, position: Vec3, frame: u32) {
        let actor = self
            .actor
            .as_deref_mut()
            .expect("missile motion requires its world Object");
        let mut effects = crate::game_logic::ScriptPresentationEffects::default();
        if let Some(owner) = self.partition.as_deref_mut() {
            actor.set_position_with_partition_owner(position, frame, &mut effects, owner);
        } else {
            actor.set_position_with_effects(position, frame, &mut effects);
        }
    }

    fn advance_missile(
        &mut self,
        motion: &mut MissileMotion,
        goal: Vec3,
        ignited: bool,
        powered: bool,
        steering: missile_motion::MissileSteering,
        frame: u32,
    ) -> bool {
        use missile_motion::MissileMotionForObject;
        let actor = self
            .actor
            .as_deref_mut()
            .expect("missile motion requires its world Object");
        motion.advance(
            actor,
            goal,
            ignited,
            powered,
            steering,
            frame,
            self.terrain,
            self.partition.as_deref_mut(),
        )
    }

    fn advance_held_missile(&mut self, motion: &mut MissileMotion, goal: Vec3, frame: u32) -> bool {
        use missile_motion::MissileMotionForObject;
        let actor = self
            .actor
            .as_deref_mut()
            .expect("missile motion requires its world Object");
        motion.advance_held(
            actor,
            goal,
            frame,
            self.terrain,
            self.partition.as_deref_mut(),
        )
    }

    fn bridge_deck(&self, position: Vec3, old_layer: u8, armed: bool) -> (Vec3, u8) {
        self.terrain.map_or((position, 1), |terrain| {
            crate::game_logic::weapon_bootstrap::bridge_deck_detonate_pose(
                terrain, position, old_layer, armed,
            )
        })
    }

    fn publish_pose(&mut self, projectile: &Projectile) {
        self.presentation.pose(projectile);
    }

    fn ignition_fx(&mut self, name: &str, projectile: &Projectile) {
        self.presentation
            .fx(name, ProjectileFxObject::Projectile(projectile));
    }

    fn attach_exhaust(&mut self, projectile: &Projectile) -> Option<u32> {
        self.presentation.attach_exhaust(projectile)
    }
}

fn advance_projectile_flight(
    projectile: &mut Projectile,
    dt: f32,
    target_is_live: bool,
    kill_target: Option<MissileKillTarget>,
    terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    actor: Option<&mut Object>,
    partition: Option<&mut crate::game_logic::partition_manager::PartitionManager>,
    world_frame: Option<u32>,
    presentation: &mut dyn ProjectilePresentationRuntime,
) -> ProjectileStep {
    projectile.advance_flight(
        dt,
        target_is_live,
        kill_target,
        world_frame,
        &mut ProjectileWorld {
            actor,
            terrain,
            partition,
            presentation,
        },
    )
}

#[cfg(test)]
trait StandaloneProjectileFlight {
    fn update(&mut self, dt: f32, target_is_live: bool) -> ProjectileStep;
    fn update_with_terrain(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) -> ProjectileStep;
    fn update_with_terrain_and_fx(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
    ) -> ProjectileStep;
    fn update_with_terrain_fx_and_pose(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
        publish: &mut dyn FnMut(&Projectile),
    ) -> ProjectileStep;
}

#[cfg(test)]
impl StandaloneProjectileFlight for Projectile {
    fn update(&mut self, dt: f32, target_is_live: bool) -> ProjectileStep {
        self.update_with_terrain(dt, target_is_live, None)
    }

    fn update_with_terrain(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
    ) -> ProjectileStep {
        self.update_with_terrain_and_fx(
            dt,
            target_is_live,
            terrain,
            &mut dispatch_projectile_fx_standalone,
        )
    }

    fn update_with_terrain_and_fx(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
    ) -> ProjectileStep {
        self.update_with_terrain_fx_and_pose(dt, target_is_live, terrain, fx, &mut |projectile| {
            crate::game_logic::host_fx_list_dispatch::publish_host_fx_object_with_visibility(
                projectile.id.0,
                projectile.position,
                projectile.velocity.z.atan2(projectile.velocity.x),
                projectile
                    .source_owner_player_id
                    .map(|p| p as i32)
                    .unwrap_or(-1),
                0.0,
                crate::game_logic::host_fx_list_dispatch::host_object_is_shrouded_for_local(
                    projectile.id.0,
                ),
            );
        })
    }

    fn update_with_terrain_fx_and_pose(
        &mut self,
        dt: f32,
        target_is_live: bool,
        terrain: Option<&crate::game_logic::game_logic::FlightTerrainView<'_>>,
        fx: &mut dyn FnMut(&str, ProjectileFxObject<'_>),
        publish: &mut dyn FnMut(&Projectile),
    ) -> ProjectileStep {
        advance_projectile_flight(
            self,
            dt,
            target_is_live,
            None,
            terrain,
            None,
            None,
            None,
            &mut ProjectileFxCallbacks { fx, pose: publish },
        )
    }
}
