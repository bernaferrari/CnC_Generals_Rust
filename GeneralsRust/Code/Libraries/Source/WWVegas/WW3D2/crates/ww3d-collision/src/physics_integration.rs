//! Physics Integration System
//!
//! This module provides physics simulation with collision responses:
//! - Rigid body dynamics
//! - Collision detection and response
//! - Constraint solving
//! - Force integration
//! - Ray casting for physics queries

use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::HashMap;

type Quaternion = Quat;

/// Physics world containing all physics objects
pub struct PhysicsWorld {
    bodies: HashMap<PhysicsBodyId, RigidBody>,
    constraints: Vec<Box<dyn Constraint>>,
    gravity: Vec3,
    time_step: f32,
    sub_steps: u32,
    stats: PhysicsStats,
    contact_cache: Vec<ContactManifold>,
    collision_events: Vec<CollisionEvent>,
    collision_callback: Option<Box<dyn FnMut(&CollisionEvent) + Send>>,
}

impl PhysicsWorld {
    /// Create a new physics world
    pub fn new() -> Self {
        Self {
            bodies: HashMap::new(),
            constraints: Vec::new(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            time_step: 1.0 / 60.0, // 60 FPS
            sub_steps: 8,
            stats: PhysicsStats::default(),
            contact_cache: Vec::new(),
            collision_events: Vec::new(),
            collision_callback: None,
        }
    }

    /// Set collision callback for handling collision events
    pub fn set_collision_callback<F>(&mut self, callback: F)
    where
        F: FnMut(&CollisionEvent) + Send + 'static,
    {
        self.collision_callback = Some(Box::new(callback));
    }

    /// Get collision events from the last step
    pub fn get_collision_events(&self) -> &[CollisionEvent] {
        &self.collision_events
    }

    /// Clear collision events
    pub fn clear_collision_events(&mut self) {
        self.collision_events.clear();
    }

    /// Create a rigid body
    pub fn create_body(&mut self, desc: RigidBodyDesc) -> PhysicsBodyId {
        let id = PhysicsBodyId::new();
        let body = RigidBody::new(desc);
        self.bodies.insert(id, body);
        id
    }

    /// Remove a rigid body
    pub fn remove_body(&mut self, id: PhysicsBodyId) {
        self.bodies.remove(&id);
    }

    /// Get a rigid body (mutable access)
    pub fn get_body_mut(&mut self, id: PhysicsBodyId) -> Option<&mut RigidBody> {
        self.bodies.get_mut(&id)
    }

    /// Get a rigid body (read-only access)
    pub fn get_body(&self, id: PhysicsBodyId) -> Option<&RigidBody> {
        self.bodies.get(&id)
    }

    /// Add a constraint
    pub fn add_constraint(&mut self, constraint: Box<dyn Constraint>) {
        self.constraints.push(constraint);
    }

    /// Step the physics simulation
    pub fn step(&mut self) {
        let dt = self.time_step / self.sub_steps as f32;

        // Clear previous frame's collision events
        self.collision_events.clear();

        for _ in 0..self.sub_steps {
            // Apply forces
            self.apply_forces(dt);

            // Integrate velocities
            self.integrate_velocities(dt);

            // Detect collisions
            let collisions = self.detect_collisions();

            // Process collision events and update contact cache
            self.process_collision_events(&collisions);

            // Solve constraints
            self.solve_constraints(dt, &collisions);

            // Integrate positions
            self.integrate_positions(dt);

            // Update sleep states
            self.update_sleep_states(dt);
        }

        // Age and cleanup contact cache
        self.update_contact_cache();

        // Update statistics
        self.stats.bodies = self.bodies.len();
        self.stats.constraints = self.constraints.len();
        self.stats.sleeping_bodies = self.bodies.values().filter(|b| b.is_sleeping).count();
        self.stats.active_contacts = self.contact_cache.len();
    }

    /// Process collision events and generate enter/stay/exit events
    fn process_collision_events(&mut self, collisions: &[Collision]) {
        #[allow(dead_code)] // Contact caching threshold (future optimization)
        const MAX_CONTACT_DISTANCE: f32 = 0.1;

        // Mark all cached contacts as not updated this frame
        for contact in &mut self.contact_cache {
            contact.frame_count = 0;
        }

        // Process new collisions
        for collision in collisions {
            let pair_key = if collision.body_a.0 < collision.body_b.0 {
                (collision.body_a, collision.body_b)
            } else {
                (collision.body_b, collision.body_a)
            };

            // Check if this contact already exists in cache
            let mut found_existing = false;
            for cached_contact in &mut self.contact_cache {
                if (cached_contact.body_a, cached_contact.body_b) == pair_key {
                    // Update existing contact
                    cached_contact.contact_point = collision.contact_point;
                    cached_contact.normal = collision.normal;
                    cached_contact.penetration = collision.penetration;
                    cached_contact.frame_count = 1;
                    found_existing = true;

                    // Generate "Stay" event
                    let event = CollisionEvent {
                        event_type: CollisionEventType::Stay,
                        body_a: collision.body_a,
                        body_b: collision.body_b,
                        contact_point: collision.contact_point,
                        normal: collision.normal,
                        penetration: collision.penetration,
                    };
                    self.collision_events.push(event.clone());

                    // Call callback if set
                    if let Some(callback) = &mut self.collision_callback {
                        callback(&event);
                    }
                    break;
                }
            }

            if !found_existing {
                // New contact - add to cache
                self.contact_cache.push(ContactManifold {
                    body_a: pair_key.0,
                    body_b: pair_key.1,
                    contact_point: collision.contact_point,
                    normal: collision.normal,
                    penetration: collision.penetration,
                    frame_count: 1,
                    normal_impulse: 0.0, // Start with no cached impulse
                    friction_impulse: 0.0,
                });

                // Generate "Enter" event
                let event = CollisionEvent {
                    event_type: CollisionEventType::Enter,
                    body_a: collision.body_a,
                    body_b: collision.body_b,
                    contact_point: collision.contact_point,
                    normal: collision.normal,
                    penetration: collision.penetration,
                };
                self.collision_events.push(event.clone());

                // Call callback if set
                if let Some(callback) = &mut self.collision_callback {
                    callback(&event);
                }
            }
        }

        // Generate "Exit" events for contacts that were not updated
        // Pre-allocate based on contact cache size (worst case: all contacts exit)
        let mut exit_events = Vec::with_capacity(self.contact_cache.len());
        for contact in &self.contact_cache {
            if contact.frame_count == 0 {
                exit_events.push(CollisionEvent {
                    event_type: CollisionEventType::Exit,
                    body_a: contact.body_a,
                    body_b: contact.body_b,
                    contact_point: contact.contact_point,
                    normal: contact.normal,
                    penetration: contact.penetration,
                });
            }
        }

        for event in exit_events {
            self.collision_events.push(event.clone());
            if let Some(callback) = &mut self.collision_callback {
                callback(&event);
            }
        }
    }

    /// Update and cleanup contact cache
    fn update_contact_cache(&mut self) {
        // Remove contacts that haven't been updated (they've exited)
        self.contact_cache.retain(|contact| contact.frame_count > 0);
    }

    /// Apply forces to all bodies
    fn apply_forces(&mut self, dt: f32) {
        for body in self.bodies.values_mut() {
            if body.inv_mass > 0.0 && !body.is_sleeping {
                // Apply gravity
                body.force += self.gravity / body.inv_mass;

                // Apply damping
                body.linear_velocity *= 1.0 - body.linear_damping * dt;
                body.angular_velocity *= 1.0 - body.angular_damping * dt;
            }
        }
    }

    /// Update sleep states for all bodies
    fn update_sleep_states(&mut self, dt: f32) {
        // Thresholds are for velocity magnitude squared (note: checked with v²)
        // Original values were too high (11.0 → sqrt=3.3 units/sec) preventing sleep
        // Reduced to 2.0 (sqrt=1.41 units/sec) for reasonable sleep behavior
        const SLEEP_VELOCITY_THRESHOLD: f32 = 2.0;
        const SLEEP_ANGULAR_THRESHOLD: f32 = 2.0;
        const SLEEP_TIME_THRESHOLD: f32 = 0.5;

        for body in self.bodies.values_mut() {
            if body.inv_mass == 0.0 {
                continue; // Static bodies don't sleep
            }

            let linear_speed_sq = body.linear_velocity.length_squared();
            let angular_speed_sq = body.angular_velocity.length_squared();

            if linear_speed_sq < SLEEP_VELOCITY_THRESHOLD * SLEEP_VELOCITY_THRESHOLD
                && angular_speed_sq < SLEEP_ANGULAR_THRESHOLD * SLEEP_ANGULAR_THRESHOLD
            {
                body.sleep_timer += dt;
                if body.sleep_timer > SLEEP_TIME_THRESHOLD {
                    body.is_sleeping = true;
                    body.linear_velocity = Vec3::ZERO;
                    body.angular_velocity = Vec3::ZERO;
                }
            } else {
                body.sleep_timer = 0.0;
                body.is_sleeping = false;
            }
        }
    }

    /// Integrate velocities
    fn integrate_velocities(&mut self, dt: f32) {
        for body in self.bodies.values_mut() {
            if body.inv_mass > 0.0 && !body.is_sleeping {
                // Integrate linear velocity
                body.linear_velocity += body.force * body.inv_mass * dt;

                // Integrate angular velocity using world-space inertia tensor
                body.angular_velocity += body.inv_inertia_world * body.torque * dt;
            }

            // Clear forces
            body.force = Vec3::ZERO;
            body.torque = Vec3::ZERO;
        }
    }

    /// Integrate positions
    fn integrate_positions(&mut self, dt: f32) {
        for body in self.bodies.values_mut() {
            if body.is_sleeping {
                continue;
            }

            // Integrate position
            body.position += body.linear_velocity * dt;

            // Integrate rotation using proper angular velocity integration
            if body.angular_velocity.length_squared() > 0.0 {
                let angle = body.angular_velocity.length() * dt;
                let axis = body.angular_velocity.normalize();
                let delta_rotation = Quaternion::from_axis_angle(axis, angle);
                body.rotation = (delta_rotation * body.rotation).normalize();
            }

            // Update transform matrix and world-space inertia tensor
            body.update_transform();
            body.update_world_inertia_tensor();
        }
    }

    /// Detect collisions between all bodies
    fn detect_collisions(&self) -> Vec<Collision> {
        // Pre-allocate based on typical collision density
        // Most games have collision count ~= 0.5-2x number of bodies
        let estimated_collisions = self.bodies.len().saturating_mul(2) / 3;
        let mut collisions = Vec::with_capacity(estimated_collisions);

        // Broad phase: build spatial hash grid
        let cell_size = 5.0; // Adjust based on typical object size
        let mut spatial_grid: HashMap<(i32, i32, i32), Vec<(PhysicsBodyId, &RigidBody)>> =
            HashMap::new();

        for (id, body) in &self.bodies {
            // Calculate grid cell
            let grid_x = (body.position.x / cell_size).floor() as i32;
            let grid_y = (body.position.y / cell_size).floor() as i32;
            let grid_z = (body.position.z / cell_size).floor() as i32;

            // Insert into grid (check 27 cells for broad objects)
            // Use saturating arithmetic to prevent overflow at boundaries
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let key = (
                            grid_x.saturating_add(dx),
                            grid_y.saturating_add(dy),
                            grid_z.saturating_add(dz),
                        );
                        spatial_grid
                            .entry(key)
                            .or_insert_with(Vec::new)
                            .push((*id, body));
                    }
                }
            }
        }

        // Narrow phase: test pairs in same cells
        let mut tested_pairs = std::collections::HashSet::new();

        for cell_bodies in spatial_grid.values() {
            for i in 0..cell_bodies.len() {
                for j in (i + 1)..cell_bodies.len() {
                    let (id_a, body_a) = cell_bodies[i];
                    let (id_b, body_b) = cell_bodies[j];

                    // Skip if already tested
                    let pair = if id_a.0 < id_b.0 {
                        (id_a, id_b)
                    } else {
                        (id_b, id_a)
                    };

                    if !tested_pairs.insert(pair) {
                        continue;
                    }

                    // Skip if both bodies are static
                    if body_a.inv_mass == 0.0 && body_b.inv_mass == 0.0 {
                        continue;
                    }

                    // Skip if both bodies are sleeping (optimization)
                    if body_a.is_sleeping && body_b.is_sleeping {
                        continue;
                    }

                    // Test collision between bodies
                    if let Some(collision) = body_a.collide_with(body_b) {
                        collisions.push(Collision {
                            body_a: id_a,
                            body_b: id_b,
                            contact_point: collision.contact_point,
                            normal: collision.normal,
                            penetration: collision.penetration,
                        });
                    }
                }
            }
        }

        collisions
    }

    /// Solve constraints and handle collisions
    fn solve_constraints(&mut self, dt: f32, collisions: &[Collision]) {
        // Solve collision constraints
        for collision in collisions {
            if collision.body_a == collision.body_b {
                continue;
            }

            let body_a_id = collision.body_a;
            let body_b_id = collision.body_b;

            // SAFETY: `body_a_id` and `body_b_id` are distinct (checked above), so the
            // pointers obtained here never alias.
            if let Some(body_a) = self.bodies.get_mut(&body_a_id) {
                let body_a_ptr: *mut RigidBody = body_a as *mut _;
                if let Some(body_b) = self.bodies.get_mut(&body_b_id) {
                    let body_a_ref = unsafe { &mut *body_a_ptr };
                    Self::solve_collision(body_a_ref, body_b, collision);
                }
            }
        }

        // Solve other constraints with multiple iterations for convergence
        // Industry standard: 4 iterations provides good stability/performance tradeoff
        const CONSTRAINT_ITERATIONS: usize = 4;
        for _ in 0..CONSTRAINT_ITERATIONS {
            for constraint in &mut self.constraints {
                constraint.solve(&mut self.bodies, dt);
            }
        }
    }

    /// Solve a collision between two bodies
    fn solve_collision(body_a: &mut RigidBody, body_b: &mut RigidBody, collision: &Collision) {
        // Wake up sleeping bodies
        body_a.is_sleeping = false;
        body_a.sleep_timer = 0.0;
        body_b.is_sleeping = false;
        body_b.sleep_timer = 0.0;

        // Calculate relative velocity at contact point
        let r_a = collision.contact_point - body_a.position;
        let r_b = collision.contact_point - body_b.position;

        let vel_a = body_a.linear_velocity + body_a.angular_velocity.cross(r_a);
        let vel_b = body_b.linear_velocity + body_b.angular_velocity.cross(r_b);
        let relative_vel = vel_b - vel_a;

        // Calculate impulse
        let vel_along_normal = relative_vel.dot(collision.normal);

        if vel_along_normal > 0.0 {
            return; // Bodies are separating
        }

        // Calculate restitution and friction coefficients
        let restitution = (body_a.restitution + body_b.restitution) * 0.5;
        let friction = (body_a.friction + body_b.friction) * 0.5;

        // Calculate impulse scalar using proper inertia tensors
        let r_a_cross_n = r_a.cross(collision.normal);
        let r_b_cross_n = r_b.cross(collision.normal);

        let angular_effect_a = (body_a.inv_inertia_world * r_a_cross_n)
            .cross(r_a)
            .dot(collision.normal);
        let angular_effect_b = (body_b.inv_inertia_world * r_b_cross_n)
            .cross(r_b)
            .dot(collision.normal);

        let impulse_scalar = -(1.0 + restitution) * vel_along_normal
            / (body_a.inv_mass + body_b.inv_mass + angular_effect_a + angular_effect_b);

        // Apply impulse
        let impulse = collision.normal * impulse_scalar;

        body_a.linear_velocity -= impulse * body_a.inv_mass;
        body_a.angular_velocity -= body_a.inv_inertia_world * r_a.cross(impulse);

        body_b.linear_velocity += impulse * body_b.inv_mass;
        body_b.angular_velocity += body_b.inv_inertia_world * r_b.cross(impulse);

        // Solve penetration
        let percent = 0.8; // Penetration percentage to correct
        let slop = 0.01; // Penetration allowance

        let correction = collision.normal * (collision.penetration - slop).max(0.0) * percent
            / (body_a.inv_mass + body_b.inv_mass);

        body_a.position -= correction * body_a.inv_mass;
        body_b.position += correction * body_b.inv_mass;

        // Apply friction
        let tangent_vel = relative_vel - collision.normal * vel_along_normal;
        if tangent_vel.length_squared() > 0.001 {
            let tangent = tangent_vel.normalize();

            let r_a_cross_t = r_a.cross(tangent);
            let r_b_cross_t = r_b.cross(tangent);

            let angular_effect_a_t = (body_a.inv_inertia_world * r_a_cross_t)
                .cross(r_a)
                .dot(tangent);
            let angular_effect_b_t = (body_b.inv_inertia_world * r_b_cross_t)
                .cross(r_b)
                .dot(tangent);

            let friction_impulse_scalar = -tangent_vel.dot(tangent)
                / (body_a.inv_mass + body_b.inv_mass + angular_effect_a_t + angular_effect_b_t);

            let friction_impulse = tangent * friction_impulse_scalar.min(impulse_scalar * friction);

            body_a.linear_velocity -= friction_impulse * body_a.inv_mass;
            body_a.angular_velocity -= body_a.inv_inertia_world * r_a.cross(friction_impulse);

            body_b.linear_velocity += friction_impulse * body_b.inv_mass;
            body_b.angular_velocity += body_b.inv_inertia_world * r_b.cross(friction_impulse);
        }
    }

    /// Cast a ray through the physics world
    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayCastHit> {
        let mut closest_hit: Option<RayCastHit> = None;
        let mut closest_distance = max_distance;

        for (id, body) in &self.bodies {
            if let Some(hit) = body.ray_cast(origin, direction, closest_distance) {
                if hit.distance < closest_distance {
                    closest_hit = Some(RayCastHit {
                        body: *id,
                        point: hit.point,
                        normal: hit.normal,
                        distance: hit.distance,
                    });
                    closest_distance = hit.distance;
                }
            }
        }

        closest_hit
    }

    /// Get physics statistics
    pub fn get_stats(&self) -> &PhysicsStats {
        &self.stats
    }

    /// Detect simulation islands (connected components of rigid bodies)
    #[allow(dead_code)] // Future optimization: parallel island simulation
    fn detect_islands(&self) -> Vec<Vec<PhysicsBodyId>> {
        use std::collections::HashSet;

        let mut visited = HashSet::new();
        // Pre-allocate islands vector: typically 1-5 islands per physics world
        let mut islands = Vec::with_capacity(3);

        // Helper function to do depth-first search
        fn dfs(
            body_id: PhysicsBodyId,
            bodies: &HashMap<PhysicsBodyId, RigidBody>,
            contact_cache: &[ContactManifold],
            visited: &mut HashSet<PhysicsBodyId>,
            island: &mut Vec<PhysicsBodyId>,
        ) {
            if visited.contains(&body_id) {
                return;
            }

            visited.insert(body_id);
            island.push(body_id);

            // Find all bodies connected to this one via contacts
            for contact in contact_cache {
                if contact.body_a == body_id {
                    dfs(contact.body_b, bodies, contact_cache, visited, island);
                } else if contact.body_b == body_id {
                    dfs(contact.body_a, bodies, contact_cache, visited, island);
                }
            }
        }

        // Process each body
        for (&body_id, body) in &self.bodies {
            // Skip static bodies (they don't need simulation)
            if body.inv_mass == 0.0 {
                continue;
            }

            // Skip already visited bodies
            if visited.contains(&body_id) {
                continue;
            }

            // Create new island: pre-allocate based on typical island size (3-10 bodies)
            let mut island = Vec::with_capacity(8);
            dfs(
                body_id,
                &self.bodies,
                &self.contact_cache,
                &mut visited,
                &mut island,
            );

            if !island.is_empty() {
                islands.push(island);
            }
        }

        islands
    }

    /// Apply external force to a body
    pub fn apply_force(&mut self, body_id: PhysicsBodyId, force: Vec3) {
        if let Some(body) = self.bodies.get_mut(&body_id) {
            body.force += force;
            body.is_sleeping = false;
            body.sleep_timer = 0.0;
        }
    }

    /// Apply impulse to a body at a point
    pub fn apply_impulse_at_point(&mut self, body_id: PhysicsBodyId, impulse: Vec3, point: Vec3) {
        if let Some(body) = self.bodies.get_mut(&body_id) {
            body.linear_velocity += impulse * body.inv_mass;
            let r = point - body.position;
            body.angular_velocity += body.inv_inertia_world * r.cross(impulse);
            body.is_sleeping = false;
            body.sleep_timer = 0.0;
        }
    }

    /// Apply torque to a body
    pub fn apply_torque(&mut self, body_id: PhysicsBodyId, torque: Vec3) {
        if let Some(body) = self.bodies.get_mut(&body_id) {
            body.torque += torque;
            body.is_sleeping = false;
            body.sleep_timer = 0.0;
        }
    }

    /// Set gravity
    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.gravity = gravity;
    }

    /// Get gravity
    pub fn get_gravity(&self) -> Vec3 {
        self.gravity
    }

    /// Set time step
    pub fn set_time_step(&mut self, time_step: f32) {
        self.time_step = time_step;
    }

    /// Get time step
    pub fn get_time_step(&self) -> f32 {
        self.time_step
    }

    /// Set sub-steps
    pub fn set_sub_steps(&mut self, sub_steps: u32) {
        self.sub_steps = sub_steps;
    }

    /// Get sub-steps
    pub fn get_sub_steps(&self) -> u32 {
        self.sub_steps
    }
}

/// Unique identifier for physics bodies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsBodyId(u64);

impl PhysicsBodyId {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// Rigid body description for creation
#[derive(Debug, Clone)]
pub struct RigidBodyDesc {
    pub position: Vec3,
    pub rotation: Quaternion,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub shape: CollisionShape,
    pub restitution: f32,
    pub friction: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Default for RigidBodyDesc {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quaternion::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
            shape: CollisionShape::Sphere { radius: 1.0 },
            restitution: 0.3,
            friction: 0.5,
            linear_damping: 0.1,
            angular_damping: 0.1,
        }
    }
}

/// Collision shapes
#[derive(Debug, Clone)]
pub enum CollisionShape {
    Sphere {
        radius: f32,
    },
    Box {
        half_extents: Vec3,
    },
    Capsule {
        radius: f32,
        height: f32,
    },
    Mesh {
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
    },
}

/// Rigid body
#[derive(Debug, Clone)]
pub struct RigidBody {
    pub id: PhysicsBodyId,
    pub position: Vec3,
    pub rotation: Quaternion,
    pub transform: Mat4,

    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub force: Vec3,
    pub torque: Vec3,

    pub mass: f32,
    pub inv_mass: f32,
    pub inertia_local: Mat3,     // Local-space inertia tensor
    pub inv_inertia_local: Mat3, // Local-space inverse inertia tensor
    pub inv_inertia_world: Mat3, // World-space inverse inertia tensor (updated each frame)

    pub shape: CollisionShape,
    pub restitution: f32,
    pub friction: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub is_sleeping: bool,
    pub sleep_timer: f32,
}

impl RigidBody {
    fn new(desc: RigidBodyDesc) -> Self {
        let inv_mass = if desc.mass > 0.0 {
            1.0 / desc.mass
        } else {
            0.0
        };

        // Calculate inertia tensor based on shape (diagonal tensor in local space)
        let inertia_local = Self::calculate_inertia(&desc.shape, desc.mass);
        let inv_inertia_local = Self::invert_inertia_tensor(inertia_local);

        // Initial world-space inverse inertia (will be updated each frame)
        let rot_mat = Mat3::from_quat(desc.rotation);
        let inv_inertia_world = rot_mat * inv_inertia_local * rot_mat.transpose();

        Self {
            id: PhysicsBodyId::new(),
            position: desc.position,
            rotation: desc.rotation,
            transform: Mat4::IDENTITY,
            linear_velocity: desc.linear_velocity,
            angular_velocity: desc.angular_velocity,
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            mass: desc.mass,
            inv_mass,
            inertia_local,
            inv_inertia_local,
            inv_inertia_world,
            shape: desc.shape,
            restitution: desc.restitution,
            friction: desc.friction,
            linear_damping: desc.linear_damping,
            angular_damping: desc.angular_damping,
            is_sleeping: false,
            sleep_timer: 0.0,
        }
    }

    fn calculate_inertia(shape: &CollisionShape, mass: f32) -> Mat3 {
        if mass <= 0.0 {
            return Mat3::ZERO;
        }

        match shape {
            CollisionShape::Sphere { radius } => {
                let i = (2.0 / 5.0) * mass * radius * radius;
                Mat3::from_diagonal(Vec3::new(i, i, i))
            }
            CollisionShape::Box { half_extents } => {
                // Full extents for proper formula
                let x = half_extents.x * 2.0;
                let y = half_extents.y * 2.0;
                let z = half_extents.z * 2.0;
                let i_x = (1.0 / 12.0) * mass * (y * y + z * z);
                let i_y = (1.0 / 12.0) * mass * (x * x + z * z);
                let i_z = (1.0 / 12.0) * mass * (x * x + y * y);
                Mat3::from_diagonal(Vec3::new(i_x, i_y, i_z))
            }
            CollisionShape::Capsule { radius, height } => {
                // Approximate as cylinder
                let r2 = radius * radius;
                let h2 = height * height;
                let i_x = mass * (3.0 * r2 + h2) / 12.0;
                let i_z = i_x;
                let i_y = mass * r2 / 2.0;
                Mat3::from_diagonal(Vec3::new(i_x, i_y, i_z))
            }
            CollisionShape::Mesh { .. } => {
                // For mesh, use sphere approximation
                let radius = 1.0; // Approximate radius
                let i = (2.0 / 5.0) * mass * radius * radius;
                Mat3::from_diagonal(Vec3::new(i, i, i))
            }
        }
    }

    fn invert_inertia_tensor(inertia: Mat3) -> Mat3 {
        // For diagonal tensors, inversion is simple
        let diag = Vec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z);
        if diag.x > 0.0 && diag.y > 0.0 && diag.z > 0.0 {
            Mat3::from_diagonal(Vec3::new(1.0 / diag.x, 1.0 / diag.y, 1.0 / diag.z))
        } else {
            Mat3::ZERO
        }
    }

    pub fn update_world_inertia_tensor(&mut self) {
        let rot_mat = Mat3::from_quat(self.rotation);
        self.inv_inertia_world = rot_mat * self.inv_inertia_local * rot_mat.transpose();
    }

    pub fn update_transform(&mut self) {
        self.transform = Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation);
    }

    /// Recompute inertia tensor from current mass and shape
    /// This should be called after mass changes
    pub fn recompute_inertia_tensor(&mut self) {
        self.inertia_local = Self::calculate_inertia(&self.shape, self.mass);
        self.inv_inertia_local = Self::invert_inertia_tensor(self.inertia_local);
        self.update_world_inertia_tensor();
    }

    fn collide_with(&self, other: &RigidBody) -> Option<CollisionInfo> {
        match (&self.shape, &other.shape) {
            (CollisionShape::Sphere { radius: r1 }, CollisionShape::Sphere { radius: r2 }) => {
                self.sphere_sphere_collision(other, *r1, *r2)
            }
            (
                CollisionShape::Box { half_extents: e1 },
                CollisionShape::Box { half_extents: e2 },
            ) => self.box_box_collision(other, *e1, *e2),
            (CollisionShape::Sphere { radius }, CollisionShape::Box { half_extents }) => {
                self.sphere_box_collision(other, *radius, *half_extents)
            }
            (CollisionShape::Box { half_extents }, CollisionShape::Sphere { radius }) => other
                .sphere_box_collision(self, *radius, *half_extents)
                .map(|mut info| {
                    info.normal = -info.normal;
                    info
                }),
            (
                CollisionShape::Sphere { radius },
                CollisionShape::Capsule {
                    radius: capsule_radius,
                    height,
                },
            ) => self.sphere_capsule_collision(other, *radius, *capsule_radius, *height),
            (
                CollisionShape::Capsule {
                    radius: capsule_radius,
                    height,
                },
                CollisionShape::Sphere { radius },
            ) => other
                .sphere_capsule_collision(self, *radius, *capsule_radius, *height)
                .map(|mut info| {
                    info.normal = -info.normal;
                    info
                }),
            (
                CollisionShape::Capsule {
                    radius: r1,
                    height: h1,
                },
                CollisionShape::Capsule {
                    radius: r2,
                    height: h2,
                },
            ) => self.capsule_capsule_collision(other, *r1, *h1, *r2, *h2),
            (CollisionShape::Box { half_extents }, CollisionShape::Capsule { radius, height }) => {
                self.box_capsule_collision(other, *half_extents, *radius, *height)
            }
            (CollisionShape::Capsule { radius, height }, CollisionShape::Box { half_extents }) => {
                other
                    .box_capsule_collision(self, *half_extents, *radius, *height)
                    .map(|mut info| {
                        info.normal = -info.normal;
                        info
                    })
            }
            _ => None, // Other combinations not implemented yet
        }
    }

    fn sphere_sphere_collision(
        &self,
        other: &RigidBody,
        r1: f32,
        r2: f32,
    ) -> Option<CollisionInfo> {
        let direction = other.position - self.position;
        let distance = direction.length();
        let min_distance = r1 + r2;

        if distance < min_distance {
            let normal = direction.normalize();
            let contact_point = self.position + normal * r1;
            let penetration = min_distance - distance;

            Some(CollisionInfo {
                contact_point,
                normal,
                penetration,
            })
        } else {
            None
        }
    }

    fn box_box_collision(&self, other: &RigidBody, e1: Vec3, e2: Vec3) -> Option<CollisionInfo> {
        // OBB collision using SAT (Separating Axis Theorem)

        // Get rotation matrices
        let rot1 = Mat3::from_quat(self.rotation);
        let rot2 = Mat3::from_quat(other.rotation);

        // Box axes in world space
        let axes1 = [rot1.x_axis, rot1.y_axis, rot1.z_axis];
        let axes2 = [rot2.x_axis, rot2.y_axis, rot2.z_axis];

        // Center difference
        let t = other.position - self.position;

        // Test all 15 potential separating axes
        let mut min_penetration = f32::MAX;
        let mut best_axis = Vec3::ZERO;
        let mut best_axis_flipped = false;

        // Test box1's face normals (3 axes)
        for i in 0..3 {
            let axis = axes1[i];
            let (overlap, penetration) = self.test_obb_axis(axis, e1, e2, &axes1, &axes2, t);
            if !overlap {
                return None;
            }
            if penetration < min_penetration {
                min_penetration = penetration;
                best_axis = axis;
                best_axis_flipped = t.dot(axis) < 0.0;
            }
        }

        // Test box2's face normals (3 axes)
        for i in 0..3 {
            let axis = axes2[i];
            let (overlap, penetration) = self.test_obb_axis(axis, e1, e2, &axes1, &axes2, t);
            if !overlap {
                return None;
            }
            if penetration < min_penetration {
                min_penetration = penetration;
                best_axis = axis;
                best_axis_flipped = t.dot(axis) < 0.0;
            }
        }

        // Test edge cross products (9 axes)
        for i in 0..3 {
            for j in 0..3 {
                let axis = axes1[i].cross(axes2[j]);
                if axis.length_squared() < 0.001 {
                    continue; // Skip near-parallel axes
                }
                let axis_normalized = axis.normalize();
                let (overlap, penetration) =
                    self.test_obb_axis(axis_normalized, e1, e2, &axes1, &axes2, t);
                if !overlap {
                    return None;
                }
                if penetration < min_penetration {
                    min_penetration = penetration;
                    best_axis = axis_normalized;
                    best_axis_flipped = t.dot(axis_normalized) < 0.0;
                }
            }
        }

        // We have a collision - ensure penetration is valid
        let normal = if best_axis_flipped {
            -best_axis
        } else {
            best_axis
        };

        // Improved contact point calculation
        // Place contact point at the midpoint between boxes along the separation axis
        // Add a small offset to prevent re-penetration
        let contact_point = self.position + t * 0.5 + normal * (min_penetration * 0.5);

        Some(CollisionInfo {
            contact_point,
            normal,
            penetration: min_penetration,
        })
    }

    fn test_obb_axis(
        &self,
        axis: Vec3,
        e1: Vec3,
        e2: Vec3,
        axes1: &[Vec3; 3],
        axes2: &[Vec3; 3],
        t: Vec3,
    ) -> (bool, f32) {
        // Project half-extents onto axis
        let r1 = e1.x * axes1[0].dot(axis).abs()
            + e1.y * axes1[1].dot(axis).abs()
            + e1.z * axes1[2].dot(axis).abs();

        let r2 = e2.x * axes2[0].dot(axis).abs()
            + e2.y * axes2[1].dot(axis).abs()
            + e2.z * axes2[2].dot(axis).abs();

        let distance = t.dot(axis).abs();
        let penetration = r1 + r2 - distance;

        (penetration >= 0.0, penetration)
    }

    fn sphere_box_collision(
        &self,
        other: &RigidBody,
        radius: f32,
        half_extents: Vec3,
    ) -> Option<CollisionInfo> {
        // Transform sphere center to box local space
        let rot_mat = Mat3::from_quat(other.rotation);
        let sphere_local = rot_mat.transpose() * (self.position - other.position);

        // Find closest point on box (in local space)
        let closest = Vec3::new(
            sphere_local.x.clamp(-half_extents.x, half_extents.x),
            sphere_local.y.clamp(-half_extents.y, half_extents.y),
            sphere_local.z.clamp(-half_extents.z, half_extents.z),
        );

        let distance_vec = sphere_local - closest;
        let distance = distance_vec.length();

        if distance < radius {
            let normal = if distance > 0.0 {
                // Keep collision normals consistent with the rest of the solver:
                // `normal` points from `self` (sphere) towards `other` (box).
                // This ensures penetration correction moves bodies apart correctly.
                rot_mat * (-distance_vec.normalize())
            } else {
                // Sphere center inside box, use closest face
                Vec3::new(0.0, 1.0, 0.0)
            };

            // Contact point in world space
            let contact_point = other.position + rot_mat * closest;
            let penetration = radius - distance;

            Some(CollisionInfo {
                contact_point,
                normal,
                penetration,
            })
        } else {
            None
        }
    }

    fn sphere_capsule_collision(
        &self,
        other: &RigidBody,
        sphere_radius: f32,
        capsule_radius: f32,
        capsule_height: f32,
    ) -> Option<CollisionInfo> {
        let (cap_a, cap_b) = capsule_segment_endpoints(other, capsule_height);
        let closest = closest_point_on_segment(self.position, cap_a, cap_b);
        let sphere_to_capsule = closest - self.position;
        let distance = sphere_to_capsule.length();
        let min_distance = sphere_radius + capsule_radius;

        if distance < min_distance {
            let normal = if distance > 0.000001 {
                sphere_to_capsule / distance
            } else {
                (other.position - self.position)
                    .try_normalize()
                    .unwrap_or(Vec3::Y)
            };

            Some(CollisionInfo {
                contact_point: closest,
                normal,
                penetration: min_distance - distance,
            })
        } else {
            None
        }
    }

    fn capsule_capsule_collision(
        &self,
        other: &RigidBody,
        r1: f32,
        h1: f32,
        r2: f32,
        h2: f32,
    ) -> Option<CollisionInfo> {
        let (a0, a1) = capsule_segment_endpoints(self, h1);
        let (b0, b1) = capsule_segment_endpoints(other, h2);
        let (point_a, point_b) = closest_points_between_segments(a0, a1, b0, b1);
        let self_to_other = point_b - point_a;
        let distance = self_to_other.length();
        let min_distance = r1 + r2;

        if distance < min_distance {
            let normal = if distance > 0.000001 {
                self_to_other / distance
            } else {
                (other.position - self.position)
                    .try_normalize()
                    .unwrap_or(Vec3::Y)
            };

            Some(CollisionInfo {
                contact_point: (point_a + point_b) * 0.5,
                normal,
                penetration: min_distance - distance,
            })
        } else {
            None
        }
    }

    fn box_capsule_collision(
        &self,
        other: &RigidBody,
        half_extents: Vec3,
        capsule_radius: f32,
        capsule_height: f32,
    ) -> Option<CollisionInfo> {
        let box_rot = Mat3::from_quat(self.rotation);
        let inv_box_rot = box_rot.transpose();
        let (cap_a, cap_b) = capsule_segment_endpoints(other, capsule_height);
        let local_a = inv_box_rot * (cap_a - self.position);
        let local_b = inv_box_rot * (cap_b - self.position);
        let (local_segment_point, local_box_point) =
            closest_segment_point_to_aabb(local_a, local_b, half_extents);
        let local_delta = local_segment_point - local_box_point;
        let distance = local_delta.length();

        if distance < capsule_radius {
            let local_normal: Vec3 = if distance > 0.000001 {
                local_delta / distance
            } else {
                let local_center = inv_box_rot * (other.position - self.position);
                local_center.try_normalize().unwrap_or(Vec3::Y)
            };

            Some(CollisionInfo {
                contact_point: self.position + box_rot * local_box_point,
                normal: (box_rot * local_normal).normalize(),
                penetration: capsule_radius - distance,
            })
        } else {
            None
        }
    }

    fn ray_cast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayCastInfo> {
        match &self.shape {
            CollisionShape::Sphere { radius } => {
                self.sphere_ray_cast(origin, direction, max_distance, *radius)
            }
            CollisionShape::Box { half_extents } => {
                self.box_ray_cast(origin, direction, max_distance, *half_extents)
            }
            CollisionShape::Capsule { radius, height } => {
                self.capsule_ray_cast(origin, direction, max_distance, *radius, *height)
            }
            CollisionShape::Mesh { vertices, indices } => {
                self.mesh_ray_cast(origin, direction, max_distance, vertices, indices)
            }
        }
    }

    fn sphere_ray_cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        radius: f32,
    ) -> Option<RayCastInfo> {
        let oc = origin - self.position;
        let a = direction.dot(direction);
        let b = 2.0 * oc.dot(direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrt_disc = discriminant.sqrt();
        let t1 = (-b - sqrt_disc) / (2.0 * a);
        let t2 = (-b + sqrt_disc) / (2.0 * a);

        let t = if t1 >= 0.0 { t1 } else { t2 };

        if t >= 0.0 && t <= max_distance {
            let point = origin + direction * t;
            let normal = (point - self.position).normalize();

            Some(RayCastInfo {
                point,
                normal,
                distance: t,
            })
        } else {
            None
        }
    }

    fn box_ray_cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        half_extents: Vec3,
    ) -> Option<RayCastInfo> {
        let rot_mat = Mat3::from_quat(self.rotation);
        let inv_basis = rot_mat.transpose();
        let local_origin = inv_basis * (origin - self.position);
        let local_direction = inv_basis * direction;

        let mut tmin: f32 = 0.0;
        let mut tmax: f32 = max_distance;
        let mut hit_axis: Option<usize> = None;
        let mut hit_side = 0.0;

        for axis in 0..3 {
            let origin_axis = local_origin[axis];
            let direction_axis = local_direction[axis];
            let min_axis = -half_extents[axis];
            let max_axis = half_extents[axis];

            if direction_axis.abs() <= 0.0001 {
                if origin_axis < min_axis || origin_axis > max_axis {
                    return None;
                }
                continue;
            }

            let inv_dir = 1.0 / direction_axis;
            let t1 = (min_axis - origin_axis) * inv_dir;
            let t2 = (max_axis - origin_axis) * inv_dir;
            let (near, far, side) = if t1 < t2 {
                (t1, t2, -1.0)
            } else {
                (t2, t1, 1.0)
            };

            if near > tmin {
                tmin = near;
                hit_axis = Some(axis);
                hit_side = side;
            }

            tmax = tmax.min(far);
            if tmax <= tmin {
                return None;
            }
        }

        if tmin > max_distance {
            return None;
        }

        let point = origin + direction * tmin;
        let normal = if let Some(axis) = hit_axis {
            let mut local_normal = Vec3::ZERO;
            local_normal[axis] = hit_side;
            (rot_mat * local_normal).normalize()
        } else {
            Vec3::ZERO
        };

        Some(RayCastInfo {
            point,
            normal,
            distance: tmin,
        })
    }

    fn mesh_ray_cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        vertices: &[Vec3],
        indices: &[u32],
    ) -> Option<RayCastInfo> {
        let rot_mat = Mat3::from_quat(self.rotation);
        let inv_basis = rot_mat.transpose();
        let local_origin = inv_basis * (origin - self.position);
        let local_direction = inv_basis * direction;

        let mut closest: Option<RayCastInfo> = None;
        let mut closest_distance = max_distance;

        for tri in indices.chunks_exact(3) {
            let Some(v0) = vertices.get(tri[0] as usize).copied() else {
                continue;
            };
            let Some(v1) = vertices.get(tri[1] as usize).copied() else {
                continue;
            };
            let Some(v2) = vertices.get(tri[2] as usize).copied() else {
                continue;
            };

            if let Some((distance, local_normal)) =
                ray_triangle_intersection(local_origin, local_direction, v0, v1, v2)
            {
                if distance < closest_distance {
                    closest_distance = distance;
                    let point = origin + direction * distance;
                    let normal = (rot_mat * local_normal).normalize();
                    closest = Some(RayCastInfo {
                        point,
                        normal,
                        distance,
                    });
                }
            }
        }

        closest
    }

    fn capsule_ray_cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        radius: f32,
        height: f32,
    ) -> Option<RayCastInfo> {
        let rot_mat = Mat3::from_quat(self.rotation);
        let inv_basis = rot_mat.transpose();
        let local_origin = inv_basis * (origin - self.position);
        let local_direction = inv_basis * direction;
        let half_height = height * 0.5;

        let mut closest: Option<(f32, Vec3)> = None;

        let a = local_direction.x * local_direction.x + local_direction.z * local_direction.z;
        if a > 0.000001 {
            let b = 2.0 * (local_origin.x * local_direction.x + local_origin.z * local_direction.z);
            let c =
                local_origin.x * local_origin.x + local_origin.z * local_origin.z - radius * radius;
            let discriminant = b * b - 4.0 * a * c;

            if discriminant >= 0.0 {
                let sqrt_disc = discriminant.sqrt();
                for t in [(-b - sqrt_disc) / (2.0 * a), (-b + sqrt_disc) / (2.0 * a)] {
                    if t >= 0.0 && t <= max_distance {
                        let hit = local_origin + local_direction * t;
                        if hit.y >= -half_height && hit.y <= half_height {
                            let normal = Vec3::new(hit.x, 0.0, hit.z).normalize_or_zero();
                            update_closest_ray_hit(&mut closest, t, normal);
                        }
                    }
                }
            }
        }

        for cap_center in [
            Vec3::new(0.0, -half_height, 0.0),
            Vec3::new(0.0, half_height, 0.0),
        ] {
            if let Some((t, normal)) = ray_sphere_intersection_local(
                local_origin,
                local_direction,
                cap_center,
                radius,
                max_distance,
            ) {
                update_closest_ray_hit(&mut closest, t, normal);
            }
        }

        closest.map(|(distance, local_normal)| RayCastInfo {
            point: origin + direction * distance,
            normal: (rot_mat * local_normal).normalize(),
            distance,
        })
    }
}

fn capsule_segment_endpoints(body: &RigidBody, height: f32) -> (Vec3, Vec3) {
    let axis = body.rotation * Vec3::Y;
    let half_axis = axis * (height * 0.5);
    (body.position - half_axis, body.position + half_axis)
}

fn closest_point_on_segment(point: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let denom = ab.length_squared();
    if denom <= 0.000001 {
        return a;
    }

    let t = ((point - a).dot(ab) / denom).clamp(0.0, 1.0);
    a + ab * t
}

fn closest_point_on_aabb(point: Vec3, half_extents: Vec3) -> Vec3 {
    Vec3::new(
        point.x.clamp(-half_extents.x, half_extents.x),
        point.y.clamp(-half_extents.y, half_extents.y),
        point.z.clamp(-half_extents.z, half_extents.z),
    )
}

fn segment_aabb_distance_at(a: Vec3, b: Vec3, half_extents: Vec3, t: f32) -> (f32, Vec3, Vec3) {
    let segment_point = a + (b - a) * t;
    let box_point = closest_point_on_aabb(segment_point, half_extents);
    (
        segment_point.distance_squared(box_point),
        segment_point,
        box_point,
    )
}

fn closest_segment_point_to_aabb(a: Vec3, b: Vec3, half_extents: Vec3) -> (Vec3, Vec3) {
    if a.distance_squared(b) <= 0.000001 {
        let box_point = closest_point_on_aabb(a, half_extents);
        return (a, box_point);
    }

    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..48 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        let d1 = segment_aabb_distance_at(a, b, half_extents, m1).0;
        let d2 = segment_aabb_distance_at(a, b, half_extents, m2).0;

        if d1 < d2 {
            hi = m2;
        } else {
            lo = m1;
        }
    }

    let t = (lo + hi) * 0.5;
    let (_, segment_point, box_point) = segment_aabb_distance_at(a, b, half_extents, t);
    (segment_point, box_point)
}

fn closest_points_between_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    let (s, t) = if a <= 0.000001 && e <= 0.000001 {
        (0.0, 0.0)
    } else if a <= 0.000001 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 0.000001 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom != 0.0 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;

            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }

            (s, t)
        }
    };

    (p1 + d1 * s, p2 + d2 * t)
}

fn update_closest_ray_hit(closest: &mut Option<(f32, Vec3)>, distance: f32, normal: Vec3) {
    if normal == Vec3::ZERO {
        return;
    }

    if closest
        .as_ref()
        .map(|(closest_distance, _)| distance < *closest_distance)
        .unwrap_or(true)
    {
        *closest = Some((distance, normal));
    }
}

fn ray_sphere_intersection_local(
    origin: Vec3,
    direction: Vec3,
    center: Vec3,
    radius: f32,
    max_distance: f32,
) -> Option<(f32, Vec3)> {
    let oc = origin - center;
    let a = direction.dot(direction);
    let b = 2.0 * oc.dot(direction);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    let t = if t1 >= 0.0 { t1 } else { t2 };
    if t < 0.0 || t > max_distance {
        return None;
    }

    let point = origin + direction * t;
    Some((t, (point - center).normalize_or_zero()))
}

fn ray_triangle_intersection(
    origin: Vec3,
    direction: Vec3,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
) -> Option<(f32, Vec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let normal = edge1.cross(edge2).normalize_or_zero();
    if normal == Vec3::ZERO {
        return None;
    }

    let pvec = direction.cross(edge2);
    let det = edge1.dot(pvec);
    if det.abs() < 0.000001 {
        return None;
    }

    let inv_det = 1.0 / det;
    let tvec = origin - v0;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(edge1);
    let v = direction.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let distance = edge2.dot(qvec) * inv_det;
    if distance < 0.0 {
        return None;
    }

    Some((distance, normal))
}

/// Collision information
#[derive(Debug, Clone)]
struct CollisionInfo {
    contact_point: Vec3,
    normal: Vec3,
    penetration: f32,
}

/// Collision data
#[derive(Debug, Clone)]
pub struct Collision {
    pub body_a: PhysicsBodyId,
    pub body_b: PhysicsBodyId,
    pub contact_point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

/// Ray cast hit information
#[derive(Debug, Clone)]
pub struct RayCastHit {
    pub body: PhysicsBodyId,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

/// Ray cast internal information
#[derive(Debug, Clone)]
struct RayCastInfo {
    point: Vec3,
    normal: Vec3,
    distance: f32,
}

/// Physics constraint trait
pub trait Constraint {
    fn solve(&mut self, bodies: &mut HashMap<PhysicsBodyId, RigidBody>, dt: f32);
}

/// Distance constraint - maintains a fixed distance between two bodies
pub struct DistanceConstraint {
    pub body_a: PhysicsBodyId,
    pub body_b: PhysicsBodyId,
    pub anchor_a: Vec3, // Local space anchor on body A
    pub anchor_b: Vec3, // Local space anchor on body B
    pub rest_distance: f32,
    pub compliance: f32, // Softness (0 = rigid, higher = softer)
}

impl DistanceConstraint {
    pub fn new(
        body_a: PhysicsBodyId,
        body_b: PhysicsBodyId,
        anchor_a: Vec3,
        anchor_b: Vec3,
        rest_distance: f32,
        compliance: f32,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            rest_distance,
            compliance,
        }
    }
}

impl Constraint for DistanceConstraint {
    fn solve(&mut self, bodies: &mut HashMap<PhysicsBodyId, RigidBody>, dt: f32) {
        let body_a = bodies.get(&self.body_a);
        let body_b = bodies.get(&self.body_b);

        if body_a.is_none() || body_b.is_none() {
            return;
        }

        // Get world-space anchor positions
        let body_a = body_a.unwrap();
        let body_b = body_b.unwrap();

        let world_anchor_a = body_a.position + body_a.rotation * self.anchor_a;
        let world_anchor_b = body_b.position + body_b.rotation * self.anchor_b;

        let delta = world_anchor_b - world_anchor_a;
        let current_distance = delta.length();

        if current_distance < 0.0001 {
            return; // Avoid division by zero
        }

        let direction = delta / current_distance;
        let constraint_error = current_distance - self.rest_distance;

        // XPBD-style constraint solving with compliance
        let alpha = self.compliance / (dt * dt);

        // Calculate generalized inverse masses
        let r_a = world_anchor_a - body_a.position;
        let r_b = world_anchor_b - body_b.position;

        let w_a = body_a.inv_mass
            + (body_a.inv_inertia_world * r_a.cross(direction))
                .cross(r_a)
                .dot(direction);
        let w_b = body_b.inv_mass
            + (body_b.inv_inertia_world * r_b.cross(direction))
                .cross(r_b)
                .dot(direction);

        let w_sum = w_a + w_b;
        if w_sum < 0.0001 {
            return; // Both bodies are static
        }

        // Calculate lagrange multiplier.
        // Positive lambda pulls the bodies towards the rest distance when stretched.
        let lambda = constraint_error / (w_sum + alpha);

        // Apply position corrections
        let body_a = bodies.get_mut(&self.body_a).unwrap();
        let correction_a = direction * lambda * body_a.inv_mass;
        body_a.position += correction_a;

        let angular_correction_a = body_a.inv_inertia_world * r_a.cross(direction * lambda);
        let angle_a = angular_correction_a.length();
        if angle_a > 0.0001 {
            let axis_a = angular_correction_a / angle_a;
            let delta_rot_a = Quat::from_axis_angle(axis_a, angle_a);
            body_a.rotation = (delta_rot_a * body_a.rotation).normalize();
        }

        let body_b = bodies.get_mut(&self.body_b).unwrap();
        let correction_b = direction * lambda * body_b.inv_mass;
        body_b.position -= correction_b;

        let angular_correction_b = body_b.inv_inertia_world * r_b.cross(direction * lambda);
        let angle_b = angular_correction_b.length();
        if angle_b > 0.0001 {
            let axis_b = angular_correction_b / angle_b;
            let delta_rot_b = Quat::from_axis_angle(axis_b, -angle_b);
            body_b.rotation = (delta_rot_b * body_b.rotation).normalize();
        }
    }
}

/// Hinge constraint - restricts rotation to one axis
pub struct HingeConstraint {
    pub body_a: PhysicsBodyId,
    pub body_b: PhysicsBodyId,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub axis_a: Vec3,    // Hinge axis in body A's local space
    pub axis_b: Vec3,    // Hinge axis in body B's local space
    pub compliance: f32, // XPBD compliance for soft joint behavior
    pub damping: f32,    // Velocity damping factor (0-1, higher = more damping)
}

impl HingeConstraint {
    pub fn new(
        body_a: PhysicsBodyId,
        body_b: PhysicsBodyId,
        anchor_a: Vec3,
        anchor_b: Vec3,
        axis_a: Vec3,
        axis_b: Vec3,
    ) -> Self {
        Self {
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            axis_a,
            axis_b,
            compliance: 0.0, // Default to stiff hinge (0.0 compliance)
            damping: 0.1,    // Default moderate damping
        }
    }

    pub fn with_compliance(mut self, compliance: f32) -> Self {
        self.compliance = compliance;
        self
    }

    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.max(0.0).min(1.0);
        self
    }
}

impl Constraint for HingeConstraint {
    fn solve(&mut self, bodies: &mut HashMap<PhysicsBodyId, RigidBody>, dt: f32) {
        let body_a = bodies.get(&self.body_a);
        let body_b = bodies.get(&self.body_b);

        if body_a.is_none() || body_b.is_none() {
            return;
        }

        let body_a = body_a.unwrap();
        let body_b = body_b.unwrap();

        // Part 1: Position constraint (keep anchors together)
        let world_anchor_a = body_a.position + body_a.rotation * self.anchor_a;
        let world_anchor_b = body_b.position + body_b.rotation * self.anchor_b;

        let delta = world_anchor_b - world_anchor_a;
        let distance = delta.length();

        if distance > 0.0001 {
            let direction = delta / distance;

            let r_a = world_anchor_a - body_a.position;
            let r_b = world_anchor_b - body_b.position;

            let w_a = body_a.inv_mass
                + (body_a.inv_inertia_world * r_a.cross(direction))
                    .cross(r_a)
                    .dot(direction);
            let w_b = body_b.inv_mass
                + (body_b.inv_inertia_world * r_b.cross(direction))
                    .cross(r_b)
                    .dot(direction);

            let w_sum = w_a + w_b;
            if w_sum > 0.0001 {
                // Use compliance-based solving for position (with minimal compliance for anchor)
                let position_compliance = 0.0000001; // Very stiff position constraint
                let alpha = position_compliance / (dt * dt);
                let lambda = -distance / (w_sum + alpha);

                let body_a = bodies.get_mut(&self.body_a).unwrap();
                body_a.position += direction * lambda * body_a.inv_mass;

                let body_b = bodies.get_mut(&self.body_b).unwrap();
                body_b.position -= direction * lambda * body_b.inv_mass;
            }
        }

        // Part 2: Orientation constraint (keep hinge axes aligned)
        // This is the primary constraint for hinge behavior
        let body_a = bodies.get(&self.body_a).unwrap();
        let body_b = bodies.get(&self.body_b).unwrap();

        let world_axis_a = body_a.rotation * self.axis_a;
        let world_axis_b = body_b.rotation * self.axis_b;

        // Calculate angular error as cross product of axes
        let angle_error = world_axis_a.cross(world_axis_b);
        let error_magnitude = angle_error.length();

        if error_magnitude > 0.001 {
            let correction_axis = angle_error.normalize();

            // XPBD-style compliance for angular constraint
            // Compliance parameter controls softness of the joint
            let alpha = self.compliance / (dt * dt);

            // Calculate effective inverse inertia for the constraint
            // Use trace (sum of diagonal) as scalar approximation of rotational inertia
            // inv_inertia_world is a 3x3 matrix, so inv_effective = (inv_a + inv_b)
            let inv_inertia_a = body_a.inv_inertia_world.x_axis.x
                + body_a.inv_inertia_world.y_axis.y
                + body_a.inv_inertia_world.z_axis.z;
            let inv_inertia_b = body_b.inv_inertia_world.x_axis.x
                + body_b.inv_inertia_world.y_axis.y
                + body_b.inv_inertia_world.z_axis.z;

            let total_inv_inertia = inv_inertia_a + inv_inertia_b;
            if total_inv_inertia < 0.0001 {
                return; // Both static
            }

            // Calculate impulse with compliance damping
            let lambda = -error_magnitude / (total_inv_inertia + alpha);

            // Apply angular impulses to both bodies
            let angular_impulse = correction_axis * lambda;

            let body_a = bodies.get_mut(&self.body_a).unwrap();
            if body_a.inv_mass > 0.0 {
                let world_inertia_inv = body_a.inv_inertia_world;
                let angular_velocity = world_inertia_inv * angular_impulse;
                let angle = angular_velocity.length();
                if angle > 0.0001 {
                    let axis = angular_velocity / angle;
                    let delta_rot = Quat::from_axis_angle(axis, angle);
                    body_a.rotation = (delta_rot * body_a.rotation).normalize();
                }
            }

            let body_b = bodies.get_mut(&self.body_b).unwrap();
            if body_b.inv_mass > 0.0 {
                let world_inertia_inv = body_b.inv_inertia_world;
                let angular_velocity = world_inertia_inv * (-angular_impulse);
                let angle = angular_velocity.length();
                if angle > 0.0001 {
                    let axis = angular_velocity / angle;
                    let delta_rot = Quat::from_axis_angle(axis, angle);
                    body_b.rotation = (delta_rot * body_b.rotation).normalize();
                }
            }

            // Part 3: Velocity damping to reduce oscillation
            // Apply damping impulses proportional to relative angular velocity along constraint axis
            if self.damping > 0.0001 {
                let body_a_ref = bodies.get(&self.body_a).unwrap();
                let body_b_ref = bodies.get(&self.body_b).unwrap();

                // Get the hinge axis in world space
                let hinge_axis_a = body_a_ref.rotation * self.axis_a;
                let hinge_axis_b = body_b_ref.rotation * self.axis_b;
                // Use average of both axes for consistency
                let hinge_axis = (hinge_axis_a + hinge_axis_b).normalize();

                // Calculate relative angular velocity
                let rel_angular_vel = body_b_ref.angular_velocity - body_a_ref.angular_velocity;
                // Component along the hinge axis
                let vel_along_axis = rel_angular_vel.dot(hinge_axis);

                // Only damp if there's angular velocity along the axis
                if vel_along_axis.abs() > 0.001 {
                    // Damping impulse opposes relative velocity
                    let damping_impulse = hinge_axis * (-vel_along_axis * self.damping);

                    // Apply damping to body A
                    let body_a_mut = bodies.get_mut(&self.body_a).unwrap();
                    body_a_mut.angular_velocity += body_a_mut.inv_inertia_world * damping_impulse;

                    // Apply damping to body B (opposite direction)
                    let body_b_mut = bodies.get_mut(&self.body_b).unwrap();
                    body_b_mut.angular_velocity -= body_b_mut.inv_inertia_world * damping_impulse;
                }
            }
        }
    }
}

/// Collision callback types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionEventType {
    Enter,
    Stay,
    Exit,
}

/// Collision event
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub event_type: CollisionEventType,
    pub body_a: PhysicsBodyId,
    pub body_b: PhysicsBodyId,
    pub contact_point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
}

/// Contact manifold for persistent contacts
#[derive(Debug, Clone)]
struct ContactManifold {
    body_a: PhysicsBodyId,
    body_b: PhysicsBodyId,
    contact_point: Vec3,
    normal: Vec3,
    penetration: f32,
    frame_count: u32,
    // Warm-starting impulses for improved stability
    normal_impulse: f32,   // Last frame's normal impulse (restitution+collision)
    friction_impulse: f32, // Last frame's friction impulse
}

/// Physics statistics
#[derive(Debug, Clone, Default)]
pub struct PhysicsStats {
    pub bodies: usize,
    pub constraints: usize,
    pub sleeping_bodies: usize,
    pub collisions_resolved: u64,
    pub ray_casts_performed: u64,
    pub active_contacts: usize,
    pub islands: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_body(position: Vec3, rotation: Quat, shape: CollisionShape) -> RigidBody {
        RigidBody::new(RigidBodyDesc {
            position,
            rotation,
            shape,
            ..Default::default()
        })
    }

    #[test]
    fn test_physics_world_creation() {
        let world = PhysicsWorld::new();
        assert_eq!(world.bodies.len(), 0);
        assert_eq!(world.constraints.len(), 0);
    }

    #[test]
    fn test_rigid_body_creation() {
        let mut world = PhysicsWorld::new();
        let desc = RigidBodyDesc {
            position: Vec3::new(0.0, 5.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body_id = world.create_body(desc);
        let body = world.get_body(body_id).unwrap();

        assert_eq!(body.position, Vec3::new(0.0, 5.0, 0.0));
        assert_eq!(body.mass, 1.0);
    }

    #[test]
    fn test_sphere_sphere_collision() {
        let mut world = PhysicsWorld::new();

        let desc1 = RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let desc2 = RigidBodyDesc {
            position: Vec3::new(1.5, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body1 = world.create_body(desc1);
        let body2 = world.create_body(desc2);

        // Step physics to detect collision
        world.step();

        // Check if collision was detected and resolved
        let body1_after = world.get_body(body1).unwrap();
        let body2_after = world.get_body(body2).unwrap();

        // Bodies should have moved apart due to collision response
        let distance = (body1_after.position - body2_after.position).length();
        assert!(distance >= 1.9); // Should be at least 2.0 (radii sum) minus some tolerance
    }

    #[test]
    fn test_ray_cast() {
        let mut world = PhysicsWorld::new();

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Cast ray from left towards sphere center
        let hit = world.ray_cast(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 10.0);

        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.body, body_id);
        assert!(hit.distance > 0.0 && hit.distance < 2.0);
    }

    #[test]
    fn test_ray_cast_hits_rotated_box_outside_unrotated_aabb() {
        let mut world = PhysicsWorld::new();
        let rotation = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_4);

        let body_id = world.create_body(RigidBodyDesc {
            position: Vec3::ZERO,
            rotation,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(2.0, 0.25, 0.25),
            },
            ..Default::default()
        });

        // At z=1.0 this ray is outside the old unrotated AABB (z extent 0.25),
        // but it intersects the 45-degree oriented box, matching C++ OBBox ray tests.
        let hit = world
            .ray_cast(Vec3::new(-4.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), 10.0)
            .expect("ray should hit the rotated box");

        assert_eq!(hit.body, body_id);
        assert!((hit.distance - 2.6464467).abs() < 0.0001);
        assert!((hit.point - Vec3::new(-1.3535533, 0.0, 1.0)).length() < 0.0001);

        let expected_normal = rotation * Vec3::new(0.0, 0.0, -1.0);
        assert!(hit.normal.dot(expected_normal) > 0.999);
    }

    #[test]
    fn test_ray_cast_hits_mesh_triangle() {
        let mut world = PhysicsWorld::new();

        let body_id = world.create_body(RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Mesh {
                vertices: vec![
                    Vec3::new(-1.0, -1.0, 0.0),
                    Vec3::new(1.0, -1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                indices: vec![0, 1, 2],
            },
            ..Default::default()
        });

        let hit = world
            .ray_cast(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0), 10.0)
            .expect("ray should hit mesh triangle");

        assert_eq!(hit.body, body_id);
        assert!((hit.distance - 2.0).abs() < 0.0001);
        assert!(hit.point.length() < 0.0001);
        assert!(hit.normal.dot(Vec3::Z) > 0.999);
    }

    #[test]
    fn test_ray_cast_mesh_uses_body_transform_and_closest_triangle() {
        let mut world = PhysicsWorld::new();
        let rotation = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2);

        let body_id = world.create_body(RigidBodyDesc {
            position: Vec3::new(2.0, 0.0, 0.0),
            rotation,
            shape: CollisionShape::Mesh {
                vertices: vec![
                    Vec3::new(-1.0, -1.0, 0.0),
                    Vec3::new(1.0, -1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(-1.0, -1.0, 1.0),
                    Vec3::new(1.0, -1.0, 1.0),
                    Vec3::new(0.0, 1.0, 1.0),
                ],
                indices: vec![3, 4, 5, 0, 1, 2],
            },
            ..Default::default()
        });

        let hit = world
            .ray_cast(Vec3::new(4.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), 10.0)
            .expect("ray should hit transformed mesh");

        assert_eq!(hit.body, body_id);
        assert!((hit.distance - 1.0).abs() < 0.0001);
        assert!((hit.point - Vec3::new(3.0, 0.0, 0.0)).length() < 0.0001);
        assert!(hit.normal.dot(rotation * Vec3::Z) > 0.999);
    }

    #[test]
    fn test_ray_cast_hits_capsule_side() {
        let mut world = PhysicsWorld::new();

        let body_id = world.create_body(RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Capsule {
                radius: 0.5,
                height: 4.0,
            },
            ..Default::default()
        });

        let hit = world
            .ray_cast(Vec3::new(-2.0, 0.0, 0.0), Vec3::X, 10.0)
            .expect("ray should hit capsule side");

        assert_eq!(hit.body, body_id);
        assert!((hit.distance - 1.5).abs() < 0.0001);
        assert!((hit.point - Vec3::new(-0.5, 0.0, 0.0)).length() < 0.0001);
        assert!(hit.normal.dot(Vec3::NEG_X) > 0.999);
    }

    #[test]
    fn test_ray_cast_hits_capsule_cap_with_body_transform() {
        let mut world = PhysicsWorld::new();
        let rotation = Quat::from_axis_angle(Vec3::Z, std::f32::consts::FRAC_PI_2);

        let body_id = world.create_body(RigidBodyDesc {
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation,
            shape: CollisionShape::Capsule {
                radius: 0.5,
                height: 4.0,
            },
            ..Default::default()
        });

        let hit = world
            .ray_cast(Vec3::new(-2.0, 0.0, 0.0), Vec3::X, 10.0)
            .expect("ray should hit rotated capsule cap");

        assert_eq!(hit.body, body_id);
        assert!((hit.distance - 0.5).abs() < 0.0001);
        assert!((hit.point - Vec3::new(-1.5, 0.0, 0.0)).length() < 0.0001);
        assert!(hit.normal.dot(Vec3::NEG_X) > 0.999);
    }

    #[test]
    fn test_sphere_capsule_collision_detects_closest_segment_point() {
        let sphere = test_body(
            Vec3::new(0.8, 0.0, 0.0),
            Quat::IDENTITY,
            CollisionShape::Sphere { radius: 0.5 },
        );
        let capsule = test_body(
            Vec3::ZERO,
            Quat::IDENTITY,
            CollisionShape::Capsule {
                radius: 0.5,
                height: 2.0,
            },
        );

        let collision = sphere
            .collide_with(&capsule)
            .expect("sphere should overlap capsule");
        assert!((collision.penetration - 0.2).abs() < 0.0001);
        assert!(collision.normal.dot(Vec3::NEG_X) > 0.999);
        assert!(collision.contact_point.length() < 0.0001);
    }

    #[test]
    fn test_capsule_capsule_collision_uses_segment_closest_points() {
        let capsule_a = test_body(
            Vec3::ZERO,
            Quat::IDENTITY,
            CollisionShape::Capsule {
                radius: 0.5,
                height: 2.0,
            },
        );
        let capsule_b = test_body(
            Vec3::new(0.8, 0.0, 0.0),
            Quat::IDENTITY,
            CollisionShape::Capsule {
                radius: 0.5,
                height: 2.0,
            },
        );

        let collision = capsule_a
            .collide_with(&capsule_b)
            .expect("capsules should overlap");
        assert!((collision.penetration - 0.2).abs() < 0.0001);
        assert!(collision.normal.dot(Vec3::X) > 0.999);
        assert!((collision.contact_point.x - 0.4).abs() < 0.0001);
        assert!(collision.contact_point.y.abs() <= 1.0);
        assert!(collision.contact_point.z.abs() < 0.0001);
    }

    #[test]
    fn test_box_capsule_collision_detects_side_overlap() {
        let box_body = test_body(
            Vec3::ZERO,
            Quat::IDENTITY,
            CollisionShape::Box {
                half_extents: Vec3::ONE,
            },
        );
        let capsule = test_body(
            Vec3::new(1.4, 0.0, 0.0),
            Quat::IDENTITY,
            CollisionShape::Capsule {
                radius: 0.5,
                height: 2.0,
            },
        );

        let collision = box_body
            .collide_with(&capsule)
            .expect("box should overlap capsule");
        assert!((collision.penetration - 0.1).abs() < 0.0001);
        assert!(collision.normal.dot(Vec3::X) > 0.999);
        assert!((collision.contact_point.x - 1.0).abs() < 0.0001);
        assert!(collision.contact_point.y.abs() <= 1.0);
        assert!(collision.contact_point.z.abs() < 0.0001);
    }

    #[test]
    fn test_capsule_box_collision_inverts_normal() {
        let capsule = test_body(
            Vec3::new(1.4, 0.0, 0.0),
            Quat::IDENTITY,
            CollisionShape::Capsule {
                radius: 0.5,
                height: 2.0,
            },
        );
        let box_body = test_body(
            Vec3::ZERO,
            Quat::IDENTITY,
            CollisionShape::Box {
                half_extents: Vec3::ONE,
            },
        );

        let collision = capsule
            .collide_with(&box_body)
            .expect("capsule should overlap box");
        assert!((collision.penetration - 0.1).abs() < 0.0001);
        assert!(collision.normal.dot(Vec3::NEG_X) > 0.999);
        assert!((collision.contact_point.x - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_gravity() {
        let mut world = PhysicsWorld::new();

        let desc = RigidBodyDesc {
            position: Vec3::new(0.0, 10.0, 0.0),
            linear_velocity: Vec3::new(1.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Step physics multiple times
        for _ in 0..10 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();

        // Body should have fallen due to gravity
        assert!(body.position.y < 10.0);

        // Body should still have horizontal velocity (not affected by gravity)
        assert!(body.linear_velocity.x > 0.5);
    }

    #[test]
    fn test_rotational_dynamics() {
        let mut world = PhysicsWorld::new();

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_velocity: Vec3::new(0.0, 1.0, 0.0), // Spin around Y-axis
            shape: CollisionShape::Box {
                half_extents: Vec3::new(2.0, 0.5, 0.5),
            },
            mass: 1.0,
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Step physics
        for _ in 0..10 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();

        // Body should have rotated
        assert!(body.rotation != Quat::IDENTITY);

        // Angular velocity should be maintained (with some damping)
        assert!(body.angular_velocity.length() > 0.1);
    }

    #[test]
    fn test_obb_collision_rotated() {
        let mut world = PhysicsWorld::new();

        // Create two boxes at 45-degree angles that overlap
        let rotation1 = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_4);
        let rotation2 = Quat::from_axis_angle(Vec3::Y, -std::f32::consts::FRAC_PI_4);

        let desc1 = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: rotation1,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(1.0, 1.0, 1.0),
            },
            ..Default::default()
        };

        let desc2 = RigidBodyDesc {
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation: rotation2,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(1.0, 1.0, 1.0),
            },
            ..Default::default()
        };

        let body1 = world.create_body(desc1);
        let body2 = world.create_body(desc2);

        // Step physics a few times to allow iterative solver to push bodies apart.
        for _ in 0..5 {
            world.step();
        }

        // Bodies should have separated
        let body1_after = world.get_body(body1).unwrap();
        let body2_after = world.get_body(body2).unwrap();

        let distance = (body1_after.position - body2_after.position).length();
        // They should have moved apart
        assert!(distance > 1.0);
    }

    #[test]
    fn test_sleep_system() {
        let mut world = PhysicsWorld::new();
        world.set_gravity(Vec3::ZERO);

        // Create a body with low initial velocity
        let desc = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            linear_velocity: Vec3::new(0.01, 0.0, 0.0),
            angular_velocity: Vec3::ZERO,
            shape: CollisionShape::Sphere { radius: 1.0 },
            linear_damping: 0.9, // High damping to slow it down quickly
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Step physics many times to let body settle
        for _ in 0..200 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();

        // Body should be sleeping after settling
        assert!(body.is_sleeping);
        assert_eq!(body.linear_velocity, Vec3::ZERO);
        assert_eq!(body.angular_velocity, Vec3::ZERO);

        // Stats should show sleeping bodies
        let stats = world.get_stats();
        assert_eq!(stats.sleeping_bodies, 1);
    }

    #[test]
    fn test_sleep_wake_on_collision() {
        let mut world = PhysicsWorld::new();
        world.set_gravity(Vec3::ZERO);

        // Create a sleeping body
        let desc1 = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            linear_velocity: Vec3::ZERO,
            shape: CollisionShape::Sphere { radius: 1.0 },
            linear_damping: 0.9,
            ..Default::default()
        };

        let body1 = world.create_body(desc1);

        // Let it settle and sleep
        for _ in 0..100 {
            world.step();
        }

        let body = world.get_body(body1).unwrap();
        assert!(body.is_sleeping);

        // Create a second body moving towards the first
        let desc2 = RigidBodyDesc {
            position: Vec3::new(3.0, 0.0, 0.0),
            linear_velocity: Vec3::new(-6.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        world.create_body(desc2);

        // Step physics until collision
        for _ in 0..30 {
            world.step();
        }

        // First body should be awake now
        let body1_after = world.get_body(body1).unwrap();
        assert!(!body1_after.is_sleeping);
    }

    #[test]
    fn test_sphere_box_rotated_collision() {
        let mut world = PhysicsWorld::new();

        // Sphere approaching a rotated box
        let desc1 = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let rotation = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_4);
        let desc2 = RigidBodyDesc {
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
            },
            ..Default::default()
        };

        let body1 = world.create_body(desc1);
        let body2 = world.create_body(desc2);

        // Step physics
        world.step();

        // Collision should be detected and resolved
        let body1_after = world.get_body(body1).unwrap();
        let body2_after = world.get_body(body2).unwrap();

        let distance = (body1_after.position - body2_after.position).length();
        assert!(distance > 1.0); // Should be separated
    }

    #[test]
    fn test_inertia_tensor_updates() {
        let mut world = PhysicsWorld::new();

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_velocity: Vec3::new(1.0, 0.0, 0.0),
            shape: CollisionShape::Box {
                half_extents: Vec3::new(1.0, 2.0, 0.5),
            },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        let body_initial = world.get_body(body_id).unwrap();
        let initial_inertia = body_initial.inv_inertia_world;

        // Step physics to rotate body
        for _ in 0..10 {
            world.step();
        }

        let body_after = world.get_body(body_id).unwrap();
        let after_inertia = body_after.inv_inertia_world;

        // World-space inertia should have changed due to rotation
        assert!(initial_inertia != after_inertia);
    }

    #[test]
    fn test_spatial_grid_performance() {
        let mut world = PhysicsWorld::new();

        // Create many bodies spread out in space
        for i in 0..100 {
            let x = (i % 10) as f32 * 10.0;
            let z = (i / 10) as f32 * 10.0;

            let desc = RigidBodyDesc {
                position: Vec3::new(x, 0.0, z),
                shape: CollisionShape::Sphere { radius: 1.0 },
                ..Default::default()
            };

            world.create_body(desc);
        }

        // Step physics - should use spatial grid
        world.step();

        // All bodies should still exist
        assert_eq!(world.get_stats().bodies, 100);
    }

    #[test]
    fn test_friction_and_restitution() {
        let mut world = PhysicsWorld::new();

        // Ground plane (static box)
        let ground = RigidBodyDesc {
            position: Vec3::new(0.0, -5.0, 0.0),
            mass: 0.0, // Static
            shape: CollisionShape::Box {
                half_extents: Vec3::new(100.0, 1.0, 100.0),
            },
            friction: 0.5,
            ..Default::default()
        };

        world.create_body(ground);

        // Bouncy ball
        let ball = RigidBodyDesc {
            position: Vec3::new(0.0, 10.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            restitution: 0.8, // Very bouncy
            friction: 0.3,
            ..Default::default()
        };

        let ball_id = world.create_body(ball);

        // Step physics to let ball fall and bounce
        for _ in 0..100 {
            world.step();
        }

        let ball_after = world.get_body(ball_id).unwrap();

        // Ball should have bounced and settled near ground
        assert!(ball_after.position.y > -5.0);
        assert!(ball_after.position.y < 10.0);
    }

    #[test]
    fn test_angular_momentum_conservation() {
        let mut world = PhysicsWorld::new();

        // Create a spinning box with no external forces
        world.gravity = Vec3::ZERO; // Disable gravity

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            angular_velocity: Vec3::new(1.0, 2.0, 0.5),
            shape: CollisionShape::Box {
                half_extents: Vec3::new(1.0, 1.0, 1.0),
            },
            angular_damping: 0.0, // No damping
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        let initial_body = world.get_body(body_id).unwrap();
        let initial_angular_vel = initial_body.angular_velocity;

        // Step physics
        for _ in 0..100 {
            world.step();
        }

        let final_body = world.get_body(body_id).unwrap();
        let final_angular_vel = final_body.angular_velocity;

        // Angular velocity magnitude should be conserved (within tolerance)
        let initial_mag = initial_angular_vel.length();
        let final_mag = final_angular_vel.length();
        assert!((initial_mag - final_mag).abs() < 0.1);
    }

    #[test]
    fn test_distance_constraint() {
        let mut world = PhysicsWorld::new();
        world.set_gravity(Vec3::ZERO);

        // Create two bodies
        let desc1 = RigidBodyDesc {
            position: Vec3::new(-2.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 0.5 },
            ..Default::default()
        };

        let desc2 = RigidBodyDesc {
            position: Vec3::new(2.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 0.5 },
            ..Default::default()
        };

        let body1 = world.create_body(desc1);
        let body2 = world.create_body(desc2);

        // Add distance constraint
        let constraint = DistanceConstraint::new(
            body1,
            body2,
            Vec3::ZERO, // Local anchors at body centers
            Vec3::ZERO,
            4.0, // Match initial spacing
            0.0, // Rigid constraint
        );
        world.add_constraint(Box::new(constraint));

        // Apply force to one body
        world.apply_force(body1, Vec3::new(-100.0, 0.0, 0.0));

        // Step physics
        for _ in 0..60 {
            world.step();
        }

        // Check that distance is maintained
        let b1 = world.get_body(body1).unwrap();
        let b2 = world.get_body(body2).unwrap();
        let distance = (b2.position - b1.position).length();

        assert!((distance - 4.0).abs() < 0.1);
    }

    #[test]
    fn test_collision_callbacks() {
        use std::sync::{Arc, Mutex};

        let mut world = PhysicsWorld::new();

        // Track collision events
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        world.set_collision_callback(move |event: &CollisionEvent| {
            events_clone.lock().unwrap().push(event.event_type);
        });

        // Create two overlapping bodies
        let desc1 = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let desc2 = RigidBodyDesc {
            position: Vec3::new(1.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        world.create_body(desc1);
        world.create_body(desc2);

        // First step should generate Enter event
        world.step();

        let event_types = events.lock().unwrap();
        assert!(event_types.len() > 0);
        assert!(event_types.contains(&CollisionEventType::Enter));
    }

    #[test]
    fn test_apply_force() {
        let mut world = PhysicsWorld::new();
        world.set_gravity(Vec3::ZERO);

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Apply force
        world.apply_force(body_id, Vec3::new(10.0, 0.0, 0.0));

        // Step physics
        for _ in 0..10 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();
        // Body should have moved in the direction of force
        assert!(body.position.x > 0.0);
        assert!(body.linear_velocity.x > 0.0);
    }

    #[test]
    fn test_apply_impulse_at_point() {
        let mut world = PhysicsWorld::new();
        world.set_gravity(Vec3::ZERO);

        let desc = RigidBodyDesc {
            position: Vec3::ZERO,
            shape: CollisionShape::Box {
                half_extents: Vec3::new(1.0, 1.0, 1.0),
            },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Apply impulse at corner to induce rotation
        let impulse = Vec3::new(0.0, 10.0, 0.0);
        let point = Vec3::new(1.0, 0.0, 0.0); // Right side

        world.apply_impulse_at_point(body_id, impulse, point);

        // Step physics
        for _ in 0..10 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();
        // Body should have both linear and angular velocity
        assert!(body.linear_velocity.length() > 0.0);
        assert!(body.angular_velocity.length() > 0.0);
    }

    #[test]
    fn test_contact_caching() {
        let mut world = PhysicsWorld::new();

        // Create two bodies in contact
        let desc1 = RigidBodyDesc {
            position: Vec3::new(0.0, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            linear_velocity: Vec3::ZERO,
            ..Default::default()
        };

        let desc2 = RigidBodyDesc {
            position: Vec3::new(1.5, 0.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            linear_velocity: Vec3::ZERO,
            ..Default::default()
        };

        world.create_body(desc1);
        world.create_body(desc2);

        // Step once
        world.step();

        let stats = world.get_stats();
        let initial_contacts = stats.active_contacts;
        assert!(initial_contacts > 0);

        // Step again - contacts should be cached
        world.step();

        let stats = world.get_stats();
        assert_eq!(stats.active_contacts, initial_contacts);
    }

    #[test]
    fn test_material_properties() {
        let mut world = PhysicsWorld::new();

        // Create ground
        let ground = RigidBodyDesc {
            position: Vec3::new(0.0, -5.0, 0.0),
            mass: 0.0, // Static
            shape: CollisionShape::Box {
                half_extents: Vec3::new(100.0, 1.0, 100.0),
            },
            friction: 0.8,
            ..Default::default()
        };

        world.create_body(ground);

        // Create bouncy ball
        let ball = RigidBodyDesc {
            position: Vec3::new(0.0, 10.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            restitution: 0.9, // Very bouncy
            friction: 0.1,
            ..Default::default()
        };

        let ball_id = world.create_body(ball);

        // Let ball fall and bounce
        for _ in 0..200 {
            world.step();
        }

        let ball_after = world.get_body(ball_id).unwrap();
        // Ball should have bounced
        assert!(ball_after.position.y > -4.2);
    }

    #[test]
    fn test_multiple_substeps() {
        let mut world = PhysicsWorld::new();
        world.set_sub_steps(16); // High precision

        let desc = RigidBodyDesc {
            position: Vec3::new(0.0, 10.0, 0.0),
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        };

        let body_id = world.create_body(desc);

        // Step physics
        for _ in 0..60 {
            world.step();
        }

        let body = world.get_body(body_id).unwrap();
        // With more substeps, simulation should be more stable
        assert!(body.position.y < 10.0); // Fell
        assert!(body.position.y > -100.0); // Didn't fall through floor
    }
}
