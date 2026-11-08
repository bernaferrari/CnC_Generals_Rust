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
    /// MeshGeometryClass::cast_ray_brute_force delegates bounded segments to
    /// WWMath and changes the surface only when a triangle improves the result.
    pub fn cast_ray(&self, ray: &mut RayCollisionTestClass) -> bool {
        let line = MuLineSegment::new(
            MuVec3::new(ray.line.start.x, ray.line.start.y, ray.line.start.z),
            MuVec3::new(ray.line.end.x, ray.line.end.y, ray.line.end.z),
        );
        let mut result = MuCastResult {
            start_bad: ray.result.start_bad,
            fraction: ray.result.fraction,
            normal: MuVec3::new(ray.result.normal.x, ray.result.normal.y, ray.result.normal.z),
            surface_type: ray.result.surface_type,
            compute_contact_point: ray.result.compute_contact_point,
            contact_point: MuVec3::new(
                ray.result.contact_point.x,
                ray.result.contact_point.y,
                ray.result.contact_point.z,
            ),
        };
        let mut hit = false;
        for triangle in &self.triangles {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };
            // Preserve the supplied polygon normal, including its magnitude;
            // neither meshgeometry.cpp nor colmathline.cpp normalizes it here.
            let tri = MuTriangle::with_normal(
                MuVec3::new(verts[0].x, verts[0].y, verts[0].z),
                MuVec3::new(verts[1].x, verts[1].y, verts[1].z),
                MuVec3::new(verts[2].x, verts[2].y, verts[2].z),
                w3d_to_mu_vec3(&triangle.normal),
            );
            if MuCollisionMath::collide_line_triangle(&line, &tri, &mut result) {
                hit = true;
                result.surface_type = triangle.attributes;
            }
        }
        if hit {
            ray.result.fraction = result.fraction;
            ray.result.normal = Vec3::new(result.normal.x, result.normal.y, result.normal.z);
            ray.result.surface_type = result.surface_type;
            if result.compute_contact_point {
                ray.result.contact_point = Vec3::new(
                    result.contact_point.x,
                    result.contact_point.y,
                    result.contact_point.z,
                );
            }
        }
        hit
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

#[cfg(test)]
mod ray_cast_tests {
    use super::*;
    use crate::render_object_system::RenderObjClass;
    use std::sync::Arc;
    use ww3d_geometry::LineSegment;

    fn triangle_mesh() -> MeshClass {
        let mut model = MeshModelClass::new("ray contract");
        model.vertices = vec![
            W3dVectorStruct { x: -1.0, y: -1.0, z: 0.0 },
            W3dVectorStruct { x: 1.0, y: -1.0, z: 0.0 },
            W3dVectorStruct { x: 0.0, y: 1.0, z: 0.0 },
        ];
        model.triangles = vec![W3dTriangleStruct {
            vindex: [0, 1, 2],
            attributes: 37,
            // Deliberately non-unit and opposite to the winding.
            normal: W3dVectorStruct { x: 0.0, y: 0.0, z: -2.0 },
            distance: 0.0,
        }];
        let mut mesh = MeshClass::new();
        mesh.set_collision_type(1);
        mesh.model = Some(Arc::new(model));
        mesh
    }

    fn cast(mesh: &MeshClass, ray: &mut RayCollisionTestClass, through_trait: bool) -> bool {
        if through_trait {
            let object: &dyn RenderObjClass = mesh;
            object.cast_ray(ray)
        } else {
            mesh.cast_ray(ray)
        }
    }

    fn ray(start: Vec3, end: Vec3) -> RayCollisionTestClass {
        RayCollisionTestClass::new(LineSegment::new(start, end), 1)
    }

    #[test]
    fn mesh_ray_preserves_nearer_and_equal_incoming_results() {
        let mesh = triangle_mesh();
        for through_trait in [false, true] {
            for fraction in [0.25, 0.5] {
                let mut test = ray(Vec3::Z, -Vec3::Z);
                test.result.fraction = fraction;
                test.result.normal = Vec3::X;
                test.result.surface_type = 91;
                test.result.contact_point = Vec3::splat(7.0);
                test.result.compute_contact_point = true;
                test.collided_render_obj = Some(123);
                assert!(!cast(&mesh, &mut test, through_trait));
                assert_eq!(test.result.fraction, fraction);
                assert_eq!(test.result.normal, Vec3::X);
                assert_eq!(test.result.surface_type, 91);
                assert_eq!(test.result.contact_point, Vec3::splat(7.0));
                assert!(test.result.compute_contact_point);
                assert!(!test.result.start_bad);
                assert_eq!(test.collided_render_obj, Some(123));
            }
        }
    }

    #[test]
    fn mesh_ray_uses_exact_segment_and_parallel_boundaries() {
        let mesh = triangle_mesh();
        for through_trait in [false, true] {
            for (start, end, limit, expected) in [
                (Vec3::ZERO, -Vec3::Z, 1.0, Some(0.0)),
                (Vec3::Z, Vec3::ZERO, 1.0, None),
                (Vec3::Z, Vec3::ZERO, 2.0, Some(1.0)),
                (Vec3::Z, Vec3::new(0.0, 0.0, 0.000001), 2.0, None),
                (-Vec3::Z * 0.000001, -Vec3::Z, 1.0, None),
                (Vec3::ZERO, Vec3::ZERO, 1.0, None),
                (Vec3::ZERO, Vec3::X, 1.0, None),
                (
                    Vec3::new(-0.25, 0.0, 0.0000001),
                    Vec3::new(0.25, 0.0, -0.0000001),
                    1.0,
                    Some(0.5),
                ),
                (Vec3::Z * 0.0000001, -Vec3::Z * 0.0000001, 1.0, Some(0.5)),
            ] {
                let mut test = ray(start, end);
                test.result.fraction = limit;
                test.result.compute_contact_point = true;
                assert_eq!(cast(&mesh, &mut test, through_trait), expected.is_some());
                assert_eq!(test.result.fraction, expected.unwrap_or(limit));
                assert!(!test.result.start_bad);
                if expected.is_some() {
                    assert_eq!(test.result.contact_point, Vec3::ZERO);
                    assert_eq!(test.result.normal, -Vec3::Z * 2.0);
                    assert_eq!(test.result.surface_type, 37);
                }
            }
        }
    }

    #[test]
    fn mesh_ray_preserves_contact_flag_and_supplied_normal_in_world_space() {
        let mut mesh = triangle_mesh();
        mesh.transform = Mat4::from_translation(Vec3::new(3.0, 4.0, 5.0))
            * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2);
        for through_trait in [false, true] {
            for compute_contact_point in [false, true] {
                let mut test = ray(
                    mesh.transform.transform_point3(Vec3::Z),
                    mesh.transform.transform_point3(-Vec3::Z),
                );
                let sentinel = Vec3::new(11.0, 12.0, 13.0);
                test.result.contact_point = sentinel;
                test.result.compute_contact_point = compute_contact_point;
                assert!(cast(&mesh, &mut test, through_trait));
                assert_eq!(test.result.compute_contact_point, compute_contact_point);
                assert!(!test.result.start_bad);
                assert!((test.result.fraction - 0.5).abs() < 0.000001);
                assert_eq!(test.result.surface_type, 37);
                assert_eq!(
                    test.result.normal,
                    mesh.transform.transform_vector3(-Vec3::Z * 2.0),
                );
                if compute_contact_point {
                    assert!((test.result.contact_point - mesh.transform.w_axis.truncate()).length() < 0.000001);
                } else {
                    assert_eq!(test.result.contact_point, sentinel);
                }
                assert_eq!(test.collided_render_obj, Some(&mesh as *const MeshClass as usize));
            }
        }
    }

    #[test]
    fn mesh_ray_updates_surface_only_for_the_closest_accepted_triangle() {
        let mut mesh = triangle_mesh();
        let model = Arc::get_mut(mesh.model.as_mut().unwrap()).unwrap();
        for (z, attributes) in [(0.5, 52), (0.5, 99), (-0.5, 73)] {
            let first = model.vertices.len() as u32;
            model.vertices.extend([
                W3dVectorStruct { x: -1.0, y: -1.0, z },
                W3dVectorStruct { x: 1.0, y: -1.0, z },
                W3dVectorStruct { x: 0.0, y: 1.0, z },
            ]);
            model.triangles.push(W3dTriangleStruct {
                vindex: [first, first + 1, first + 2],
                attributes,
                normal: W3dVectorStruct { x: 0.0, y: 0.0, z: -2.0 },
                distance: 2.0 * z,
            });
        }
        for through_trait in [false, true] {
            let mut test = ray(Vec3::Z, -Vec3::Z);
            assert!(cast(&mesh, &mut test, through_trait));
            assert_eq!(test.result.fraction, 0.25);
            assert_eq!(test.result.surface_type, 52);
        }
    }

    #[test]
    fn mesh_ray_public_callers_reject_start_bad_without_mutation() {
        let mesh = triangle_mesh();
        for through_trait in [false, true] {
            let mut test = ray(Vec3::Z, -Vec3::Z);
            test.result.start_bad = true;
            test.result.contact_point = Vec3::X;
            test.result.surface_type = 91;
            assert!(!cast(&mesh, &mut test, through_trait));
            assert!(test.result.start_bad);
            assert_eq!(test.result.fraction, 1.0);
            assert_eq!(test.result.contact_point, Vec3::X);
            assert_eq!(test.result.surface_type, 91);
            assert_eq!(test.collided_render_obj, None);
        }
    }

    #[test]
    fn mesh_ray_public_callers_share_source_admission_rules() {
        for through_trait in [false, true] {
            let mut mesh = triangle_mesh();
            let mut test = ray(Vec3::Z, -Vec3::Z);
            test.collision_type = 2;
            assert!(!cast(&mesh, &mut test, through_trait));
            test.collision_type = 1;
            mesh.is_hidden = true;
            assert!(!cast(&mesh, &mut test, through_trait));
            test.check_hidden = true;
            assert!(cast(&mesh, &mut test, through_trait));
            test.result = RayCollisionResult::default();
            mesh.is_animation_hidden = true;
            assert!(!cast(&mesh, &mut test, through_trait));
            mesh.is_hidden = false;
            mesh.is_animation_hidden = false;
            mesh.set_presentation_opacity(0.5);
            test.check_translucent = true;
            assert!(!cast(&mesh, &mut test, through_trait));
            test.check_translucent = false;
            assert!(cast(&mesh, &mut test, through_trait));
        }
    }
}
