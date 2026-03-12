//! Parallel processing utilities for scene management
//!
//! Implements multi-threaded scene updates, culling, and physics
//! to match C++ thread pool performance

use glam::{Mat4, Vec3};
use rayon::prelude::*;
use ww3d_collision::AABTree;

/// Represents an updateable scene object
pub trait Updateable: Send + Sync {
    fn update(&mut self, delta_time: f32);
}

/// Represents an object that can be culled
pub trait Cullable: Send + Sync {
    fn get_bounding_sphere(&self) -> (Vec3, f32); // center, radius
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
}

/// Frustum for culling operations
#[derive(Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

#[derive(Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32,
}

impl Frustum {
    /// Test if a sphere intersects the frustum
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            let distance = plane.normal.dot(center) + plane.distance;
            if distance < -radius {
                return false;
            }
        }
        true
    }

    /// Extract frustum from view-projection matrix
    pub fn from_matrix(vp_matrix: Mat4) -> Self {
        let m = vp_matrix.to_cols_array_2d();

        let extract_plane = |row: usize, negate: bool| -> Plane {
            let sign = if negate { -1.0 } else { 1.0 };
            let normal = Vec3::new(
                m[0][3] + sign * m[0][row],
                m[1][3] + sign * m[1][row],
                m[2][3] + sign * m[2][row],
            );
            let distance = m[3][3] + sign * m[3][row];
            let len = normal.length();
            Plane {
                normal: if len > 0.0 { normal / len } else { normal },
                distance: if len > 0.0 { distance / len } else { distance },
            }
        };

        Self {
            planes: [
                extract_plane(0, false), // Left
                extract_plane(0, true),  // Right
                extract_plane(1, false), // Bottom
                extract_plane(1, true),  // Top
                extract_plane(2, false), // Near
                extract_plane(2, true),  // Far
            ],
        }
    }
}

/// Parallel scene update operations
pub struct ParallelSceneOps;

impl ParallelSceneOps {
    /// Update all objects in parallel
    pub fn update_objects_parallel<T: Updateable>(objects: &mut [T], delta_time: f32) {
        objects.par_iter_mut().for_each(|obj| {
            obj.update(delta_time);
        });
    }

    /// Cull objects against frustum in parallel
    pub fn cull_objects_parallel<T: Cullable>(objects: &mut [T], frustum: &Frustum) -> usize {
        objects
            .par_iter_mut()
            .map(|obj| {
                let (center, radius) = obj.get_bounding_sphere();
                let visible = frustum.intersects_sphere(center, radius);
                obj.set_visible(visible);
                if visible { 1 } else { 0 }
            })
            .sum()
    }

    /// Transform multiple points in parallel
    pub fn transform_points_parallel(
        points: &[Vec3],
        matrix: &Mat4,
        output: &mut [Vec3],
    ) {
        assert_eq!(points.len(), output.len());

        const CHUNK_SIZE: usize = 256;

        points
            .par_chunks(CHUNK_SIZE)
            .zip(output.par_chunks_mut(CHUNK_SIZE))
            .for_each(|(src_chunk, dst_chunk)| {
                for (src, dst) in src_chunk.iter().zip(dst_chunk.iter_mut()) {
                    *dst = matrix.transform_point3(*src);
                }
            });
    }

    /// Batch ray-casting in parallel
    pub fn batch_raycast_parallel<F>(
        rays: &[(Vec3, Vec3)], // (origin, direction) pairs
        test_fn: F,
    ) -> Vec<Option<f32>>
    where
        F: Fn(Vec3, Vec3) -> Option<f32> + Send + Sync,
    {
        rays.par_iter()
            .map(|&(origin, direction)| test_fn(origin, direction))
            .collect()
    }
}

/// Parallel physics operations
pub struct ParallelPhysicsOps;

impl ParallelPhysicsOps {
    /// Broad-phase collision detection in parallel
    pub fn broad_phase_parallel<T>(
        objects: &[T],
        get_bounds: impl Fn(&T) -> (Vec3, Vec3) + Send + Sync,
    ) -> Vec<(usize, usize)>
    where
        T: Send + Sync,
    {
        // Spatial hashing for broad phase
        use std::collections::HashMap;
        use std::sync::Mutex;

        let cell_size = 10.0;
        let hash_map = Mutex::new(HashMap::<(i32, i32, i32), Vec<usize>>::new());

        // Hash objects into cells
        objects.par_iter().enumerate().for_each(|(i, obj)| {
            let (min, max) = get_bounds(obj);
            let center = (min + max) * 0.5;

            let cell = (
                (center.x / cell_size).floor() as i32,
                (center.y / cell_size).floor() as i32,
                (center.z / cell_size).floor() as i32,
            );

            let mut map = hash_map.lock().unwrap();
            map.entry(cell).or_insert_with(Vec::new).push(i);
        });

        // Find potential pairs
        let pairs = Mutex::new(Vec::new());
        let map = hash_map.into_inner().unwrap();

        map.par_iter().for_each(|(_, indices)| {
            let mut local_pairs = Vec::new();
            for i in 0..indices.len() {
                for j in (i + 1)..indices.len() {
                    local_pairs.push((indices[i], indices[j]));
                }
            }
            if !local_pairs.is_empty() {
                pairs.lock().unwrap().extend(local_pairs);
            }
        });

        pairs.into_inner().unwrap()
    }

    /// Parallel contact generation
    pub fn generate_contacts_parallel<T, F>(
        pairs: &[(usize, usize)],
        objects: &[T],
        contact_fn: F,
    ) -> Vec<Contact>
    where
        T: Send + Sync,
        F: Fn(&T, &T) -> Option<Contact> + Send + Sync,
    {
        pairs
            .par_iter()
            .filter_map(|&(i, j)| contact_fn(&objects[i], &objects[j]))
            .collect()
    }
}

/// Contact information for physics
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub point: Vec3,
    pub normal: Vec3,
    pub penetration: f32,
    pub body_a: usize,
    pub body_b: usize,
}

/// Parallel animation operations
pub struct ParallelAnimationOps;

impl ParallelAnimationOps {
    /// Update multiple animation channels in parallel
    pub fn update_animations_parallel<T>(
        animations: &mut [T],
        delta_time: f32,
    ) where
        T: Updateable,
    {
        animations.par_iter_mut().for_each(|anim| {
            anim.update(delta_time);
        });
    }

    /// Blend animation poses in parallel
    pub fn blend_poses_parallel(
        pose_a: &[Mat4],
        pose_b: &[Mat4],
        weight: f32,
        output: &mut [Mat4],
    ) {
        assert_eq!(pose_a.len(), pose_b.len());
        assert_eq!(pose_a.len(), output.len());

        pose_a
            .par_iter()
            .zip(pose_b.par_iter())
            .zip(output.par_iter_mut())
            .for_each(|((a, b), out)| {
                // Simple linear blend (for demonstration)
                // In production, use proper quaternion SLERP
                let inv_weight = 1.0 - weight;
                *out = Mat4::from_cols(
                    a.x_axis * inv_weight + b.x_axis * weight,
                    a.y_axis * inv_weight + b.y_axis * weight,
                    a.z_axis * inv_weight + b.z_axis * weight,
                    a.w_axis * inv_weight + b.w_axis * weight,
                );
            });
    }
}

/// Parallel particle system operations
pub struct ParallelParticleOps;

impl ParallelParticleOps {
    /// Update particles in parallel
    pub fn update_particles_parallel<P>(
        particles: &mut [P],
        delta_time: f32,
        update_fn: impl Fn(&mut P, f32) + Send + Sync,
    ) where
        P: Send,
    {
        particles.par_iter_mut().for_each(|p| {
            update_fn(p, delta_time);
        });
    }

    /// Sort particles by depth for rendering (parallel merge sort)
    pub fn sort_particles_by_depth<P>(
        particles: &mut [P],
        camera_pos: Vec3,
        get_pos: impl Fn(&P) -> Vec3 + Send + Sync,
    ) where
        P: Send,
    {
        particles.par_sort_by(|a, b| {
            let dist_a = (get_pos(a) - camera_pos).length_squared();
            let dist_b = (get_pos(b) - camera_pos).length_squared();
            // Sort back-to-front for transparency
            dist_b.partial_cmp(&dist_a).unwrap()
        });
    }
}

/// Work-stealing task scheduler for complex operations
pub struct TaskScheduler {
    thread_count: usize,
}

impl TaskScheduler {
    /// Create scheduler with specified thread count
    pub fn new(thread_count: Option<usize>) -> Self {
        Self {
            thread_count: thread_count.unwrap_or_else(num_cpus::get),
        }
    }

    /// Execute tasks in parallel with work stealing
    pub fn execute_tasks<F>(&self, tasks: Vec<F>)
    where
        F: FnOnce() + Send,
    {
        rayon::scope(|s| {
            for task in tasks {
                s.spawn(|_| task());
            }
        });
    }

    /// Get optimal chunk size for parallel iteration
    pub fn optimal_chunk_size(&self, total_items: usize) -> usize {
        (total_items / self.thread_count).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        position: Vec3,
        velocity: Vec3,
        visible: bool,
    }

    impl Updateable for TestObject {
        fn update(&mut self, delta_time: f32) {
            self.position += self.velocity * delta_time;
        }
    }

    impl Cullable for TestObject {
        fn get_bounding_sphere(&self) -> (Vec3, f32) {
            (self.position, 1.0)
        }

        fn is_visible(&self) -> bool {
            self.visible
        }

        fn set_visible(&mut self, visible: bool) {
            self.visible = visible;
        }
    }

    #[test]
    fn test_parallel_update() {
        let mut objects = vec![
            TestObject {
                position: Vec3::ZERO,
                velocity: Vec3::X,
                visible: true,
            };
            1000
        ];

        ParallelSceneOps::update_objects_parallel(&mut objects, 1.0);

        assert!((objects[0].position.x - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_frustum_culling() {
        let frustum = Frustum::from_matrix(Mat4::IDENTITY);

        assert!(frustum.intersects_sphere(Vec3::ZERO, 1.0));
        assert!(!frustum.intersects_sphere(Vec3::new(1000.0, 0.0, 0.0), 1.0));
    }

    #[test]
    fn test_broad_phase() {
        let objects = vec![Vec3::ZERO; 100];
        let pairs = ParallelPhysicsOps::broad_phase_parallel(&objects, |v| {
            let extent = Vec3::splat(0.5);
            (*v - extent, *v + extent)
        });

        // Should find many pairs since all objects are at origin
        assert!(pairs.len() > 0);
    }

    #[test]
    fn test_particle_sorting() {
        struct Particle {
            position: Vec3,
        }

        let mut particles = vec![
            Particle {
                position: Vec3::new(0.0, 0.0, 10.0),
            },
            Particle {
                position: Vec3::new(0.0, 0.0, 5.0),
            },
            Particle {
                position: Vec3::new(0.0, 0.0, 15.0),
            },
        ];

        let camera = Vec3::ZERO;
        ParallelParticleOps::sort_particles_by_depth(&mut particles, camera, |p| p.position);

        // Should be sorted back-to-front
        assert!(particles[0].position.z > particles[1].position.z);
        assert!(particles[1].position.z > particles[2].position.z);
    }
}
