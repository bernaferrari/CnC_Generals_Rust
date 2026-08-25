#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

impl MeshModelClass {
    pub fn cast_ray(&self, ray: &mut RayCollisionTestClass) -> bool {
        let ray_direction = ray.line.end - ray.line.start;
        let ray_length = ray_direction.length();
        if ray_length <= RAY_EPSILON {
            return false;
        }

        let normalized_direction = ray_direction / ray_length;
        let origin = ray.line.start;

        let mut hit = false;
        let mut best_fraction = ray.result.fraction.min(1.0);
        let mut best_point = Vec3::ZERO;
        let mut best_normal = Vec3::ZERO;
        let mut best_surface = ray.result.surface_type;

        for triangle in &self.triangles {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            if let Some((distance, normal)) = ray_triangle_intersection(
                origin,
                normalized_direction,
                ray_length,
                verts[0],
                verts[1],
                verts[2],
            ) {
                let fraction = (distance / ray_length).clamp(0.0, 1.0);
                if !hit || fraction < best_fraction {
                    hit = true;
                    best_fraction = fraction;
                    best_point = origin + normalized_direction * distance;
                    best_normal = normal;
                    best_surface = triangle.attributes;
                }
            }
        }

        if hit {
            ray.result.start_bad = best_fraction == 0.0;
            ray.result.fraction = best_fraction;
            ray.result.normal = best_normal;
            ray.result.surface_type = best_surface;
            if ray.result.compute_contact_point {
                ray.result.contact_point = origin + ray_direction * best_fraction;
            } else {
                ray.result.contact_point = best_point;
            }
            return true;
        }

        false
    }

    pub fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        let mu_box = mu_aabox_from_class(&boxtest.box_obj);
        let movement = MuVec3::new(
            boxtest.move_vector.x,
            boxtest.move_vector.y,
            boxtest.move_vector.z,
        );

        let mut best: Option<MuCastResult> = None;

        for triangle in &self.triangles {
            let Some(mu_triangle) = mu_triangle_from_w3d(triangle, &self.vertices) else {
                continue;
            };

            let mut result = MuCastResult::new();
            result.compute_contact_point = true;
            if MuCollisionMath::collide_aabox_triangle(
                &mu_box,
                &movement,
                &mu_triangle,
                &mut result,
            ) {
                let replace = match &best {
                    None => true,
                    Some(existing) => {
                        if result.start_bad && !existing.start_bad {
                            true
                        } else if !result.start_bad && existing.start_bad {
                            false
                        } else {
                            result.fraction < existing.fraction
                        }
                    }
                };

                if replace {
                    best = Some(result.clone());
                }
            }
        }

        if let Some(best_hit) = best {
            let mut contact_points = Vec::new();
            if best_hit.compute_contact_point {
                contact_points.push(Vec3::new(
                    best_hit.contact_point.x,
                    best_hit.contact_point.y,
                    best_hit.contact_point.z,
                ));
            }

            boxtest.result = Some(AABoxCollisionResult {
                intersection: true,
                contact_points,
            });
            return true;
        }

        boxtest.result = Some(AABoxCollisionResult {
            intersection: false,
            contact_points: Vec::new(),
        });
        false
    }

    pub fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        let mu_box = mu_aabox_from_class(&boxtest.box_obj);
        for triangle in &self.triangles {
            let Some(mu_triangle) = mu_triangle_from_w3d(triangle, &self.vertices) else {
                continue;
            };

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &mu_triangle) {
                return true;
            }
        }
        false
    }

    /// C++ meshgeometry.cpp Cast_OBBox / CollisionMath::Collide(OBBox, move, Tri)
    pub fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool {
        let mu_box = mu_obbox_from_class(&boxtest.box_obj);
        let movement = MuVec3::new(
            boxtest.move_vector.x,
            boxtest.move_vector.y,
            boxtest.move_vector.z,
        );

        let mut best: Option<MuCastResult> = None;

        for triangle in &self.triangles {
            let Some(mu_triangle) = mu_triangle_from_w3d(triangle, &self.vertices) else {
                continue;
            };

            let mut result = MuCastResult::new();
            result.compute_contact_point = true;
            if MuCollisionMath::collide_obbox_triangle(
                &mu_box,
                &movement,
                &mu_triangle,
                &MuVec3::ZERO,
                &mut result,
            ) {
                let replace = match &best {
                    None => true,
                    Some(existing) => {
                        if result.start_bad && !existing.start_bad {
                            true
                        } else if !result.start_bad && existing.start_bad {
                            false
                        } else {
                            result.fraction < existing.fraction
                        }
                    }
                };

                if replace {
                    best = Some(result.clone());
                }
            }
        }

        if let Some(best_hit) = best {
            let mut contact_points = Vec::new();
            if best_hit.compute_contact_point {
                contact_points.push(Vec3::new(
                    best_hit.contact_point.x,
                    best_hit.contact_point.y,
                    best_hit.contact_point.z,
                ));
            }

            boxtest.result = Some(OBBoxCollisionResult {
                intersection: true,
                contact_points,
            });
            return true;
        }

        boxtest.result = Some(OBBoxCollisionResult {
            intersection: false,
            contact_points: Vec::new(),
        });
        false
    }

    pub fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        let center = MuVec3::new(
            boxtest.box_obj.center.x,
            boxtest.box_obj.center.y,
            boxtest.box_obj.center.z,
        );
        let axes = [
            MuVec3::new(
                boxtest.box_obj.basis[0].x,
                boxtest.box_obj.basis[0].y,
                boxtest.box_obj.basis[0].z,
            )
            .normalize(),
            MuVec3::new(
                boxtest.box_obj.basis[1].x,
                boxtest.box_obj.basis[1].y,
                boxtest.box_obj.basis[1].z,
            )
            .normalize(),
            MuVec3::new(
                boxtest.box_obj.basis[2].x,
                boxtest.box_obj.basis[2].y,
                boxtest.box_obj.basis[2].z,
            )
            .normalize(),
        ];
        let extent = MuVec3::new(
            boxtest.box_obj.extent.x.abs(),
            boxtest.box_obj.extent.y.abs(),
            boxtest.box_obj.extent.z.abs(),
        );
        let aligned_box = MuAABox::new(MuVec3::ZERO, extent);

        for triangle in &self.triangles {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            let mut local_vertices = [MuVec3::ZERO; 3];
            for (idx, vert) in verts.iter().enumerate() {
                let mu_vert = MuVec3::new(vert.x, vert.y, vert.z) - center;
                local_vertices[idx] = MuVec3::new(
                    mu_vert.dot(axes[0]),
                    mu_vert.dot(axes[1]),
                    mu_vert.dot(axes[2]),
                );
            }

            let mut local_triangle =
                MuTriangle::new(local_vertices[0], local_vertices[1], local_vertices[2]);
            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&aligned_box, &local_triangle) {
                return true;
            }
        }

        false
    }

    pub fn generate_rigid_apt(&self, volume: &OBBoxClass, apt: &mut Vec<u32>) {
        apt.clear();

        let center = volume.center;
        let extents = Vec3::new(
            volume.extent.x.abs(),
            volume.extent.y.abs(),
            volume.extent.z.abs(),
        );
        let axes = [
            normalize_or(volume.basis[0], Vec3::X),
            normalize_or(volume.basis[1], Vec3::Y),
            normalize_or(volume.basis[2], Vec3::Z),
        ];
        let mu_box = MuAABox::new(MuVec3::ZERO, MuVec3::new(extents.x, extents.y, extents.z));

        for (index, triangle) in self.triangles.iter().enumerate() {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            let local_vertices: [Vec3; 3] = verts
                .iter()
                .map(|v| {
                    let offset = *v - center;
                    Vec3::new(
                        offset.dot(axes[0]),
                        offset.dot(axes[1]),
                        offset.dot(axes[2]),
                    )
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or([Vec3::ZERO; 3]);

            let mut local_triangle = MuTriangle::new(
                MuVec3::new(
                    local_vertices[0].x,
                    local_vertices[0].y,
                    local_vertices[0].z,
                ),
                MuVec3::new(
                    local_vertices[1].x,
                    local_vertices[1].y,
                    local_vertices[1].z,
                ),
                MuVec3::new(
                    local_vertices[2].x,
                    local_vertices[2].y,
                    local_vertices[2].z,
                ),
            );

            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &local_triangle) {
                apt.push(index as u32);
            }
        }
    }

    pub fn generate_skin_apt(
        &self,
        world_box: &OBBoxClass,
        apt: &mut Vec<u32>,
        world_vertices: &[Vec3],
    ) {
        apt.clear();
        if world_vertices.len() < self.vertices.len() {
            return;
        }

        let center = world_box.center;
        let extents = Vec3::new(
            world_box.extent.x.abs(),
            world_box.extent.y.abs(),
            world_box.extent.z.abs(),
        );
        let axes = [
            normalize_or(world_box.basis[0], Vec3::X),
            normalize_or(world_box.basis[1], Vec3::Y),
            normalize_or(world_box.basis[2], Vec3::Z),
        ];
        let mu_box = MuAABox::new(MuVec3::ZERO, MuVec3::new(extents.x, extents.y, extents.z));

        for (index, triangle) in self.triangles.iter().enumerate() {
            let idx0 = triangle.vindex[0] as usize;
            let idx1 = triangle.vindex[1] as usize;
            let idx2 = triangle.vindex[2] as usize;
            if idx0 >= world_vertices.len()
                || idx1 >= world_vertices.len()
                || idx2 >= world_vertices.len()
            {
                continue;
            }

            let world_positions = [
                world_vertices[idx0],
                world_vertices[idx1],
                world_vertices[idx2],
            ];

            let mut local_triangle = MuTriangle::new(
                MuVec3::new(
                    (world_positions[0] - center).dot(axes[0]),
                    (world_positions[0] - center).dot(axes[1]),
                    (world_positions[0] - center).dot(axes[2]),
                ),
                MuVec3::new(
                    (world_positions[1] - center).dot(axes[0]),
                    (world_positions[1] - center).dot(axes[1]),
                    (world_positions[1] - center).dot(axes[2]),
                ),
                MuVec3::new(
                    (world_positions[2] - center).dot(axes[0]),
                    (world_positions[2] - center).dot(axes[1]),
                    (world_positions[2] - center).dot(axes[2]),
                ),
            );

            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &local_triangle) {
                apt.push(index as u32);
            }
        }
    }
}
