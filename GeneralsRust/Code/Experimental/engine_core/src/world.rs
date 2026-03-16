use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: u64,
    pub name: String,
    pub transform: Transform,
}

#[derive(Debug, Clone)]
pub struct World {
    entities: Vec<Entity>,
    next_id: u64,
}

impl Default for World {
    fn default() -> Self {
        let mut world = Self {
            entities: Vec::new(),
            next_id: 1,
        };

        world.spawn("Camera", Transform::default());
        world.spawn(
            "Tank",
            Transform {
                position: Vec3::new(6.0, 0.0, -2.0),
                ..Transform::default()
            },
        );
        world.spawn(
            "Dozer",
            Transform {
                position: Vec3::new(-4.0, 0.0, 3.0),
                ..Transform::default()
            },
        );

        world
    }
}

impl World {
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn entities_mut(&mut self) -> &mut [Entity] {
        &mut self.entities
    }

    pub fn spawn(&mut self, name: impl Into<String>, transform: Transform) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.push(Entity {
            id,
            name: name.into(),
            transform,
        });
        id
    }

    pub fn entity_mut(&mut self, id: u64) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|entity| entity.id == id)
    }

    pub fn nearest_entity_at(&self, x: f32, z: f32, max_distance: f32) -> Option<u64> {
        let mut best: Option<(u64, f32)> = None;
        for entity in &self.entities {
            let dx = entity.transform.position.x - x;
            let dz = entity.transform.position.z - z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq <= max_distance * max_distance {
                match best {
                    Some((_, best_dist_sq)) if dist_sq >= best_dist_sq => {}
                    _ => best = Some((entity.id, dist_sq)),
                }
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn update_play(&mut self, dt: f32, simulation_seconds: f32) {
        for (index, entity) in self.entities.iter_mut().enumerate() {
            let speed = 0.5 + (index as f32 * 0.1);
            let phase = simulation_seconds * speed;
            entity.transform.rotation_degrees.y += dt * (24.0 + index as f32 * 6.0);
            entity.transform.position.x += phase.cos() * dt * 0.25;
            entity.transform.position.z += phase.sin() * dt * 0.25;
        }
    }
}
