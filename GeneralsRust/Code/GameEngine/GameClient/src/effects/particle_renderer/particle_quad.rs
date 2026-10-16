//! Coordinate-aware expansion of the particle vertex shader's quad.
use super::{Mat4, ParticleUniforms, ParticleVertex};

/// Host Y-up ground axes. Original Z-up submissions select XY explicitly.
#[must_use]
pub fn particle_ground_quad_axes() -> ([f32; 3], [f32; 3]) {
    ([1., 0., 0.], [0., 0., 1.])
}

/// Compatibility diagnostic with the default host basis and identity camera.
#[must_use]
pub fn expand_particle_world_corners(vertex: &ParticleVertex) -> [[f32; 3]; 4] {
    expand_particle_world_corners_with_uniforms(vertex, &ParticleUniforms::default())
}

/// Expand the same world corners as particle_vertex.wgsl, including view rotation.
#[must_use]
fn expand_particle_world_corners_with_uniforms(
    vertex: &ParticleVertex,
    uniforms: &ParticleUniforms,
) -> [[f32; 3]; 4] {
    let (right, up) = if vertex.billboard > 0.5 {
        let inverse_rotation = Mat4::from_cols_array_2d(&uniforms.view_matrix).transpose();
        (
            inverse_rotation.x_axis.truncate().to_array(),
            inverse_rotation.y_axis.truncate().to_array(),
        )
    } else if uniforms.source_z_up != 0 {
        ([1., 0., 0.], [0., 1., 0.])
    } else {
        particle_ground_quad_axes()
    };
    let (cos_rot, sin_rot) = (vertex.rotation.cos(), vertex.rotation.sin());
    let corners_2d = [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];
    let mut corners = [[0.; 3]; 4];
    for (i, corner) in corners_2d.iter().enumerate() {
        let rx = corner[0] * cos_rot - corner[1] * sin_rot;
        let ry = corner[0] * sin_rot + corner[1] * cos_rot;
        for axis in 0..3 {
            corners[i][axis] = vertex.position[axis]
                + right[axis] * rx * vertex.size[0]
                + up[axis] * ry * vertex.size[1];
        }
    }
    corners
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ground_and_billboard_corners_match_the_submitted_camera() {
        use glam::{Vec3, Vec4};
        let eye = Vec3::new(120., 475., -240.);
        let target = Vec3::new(170., 85., 130.);
        let axis = Mat4::from_cols(Vec4::X, Vec4::Z, Vec4::Y, Vec4::W);
        let uniforms = ParticleUniforms {
            view_matrix: (Mat4::look_at_rh(eye, target, Vec3::Y) * axis).to_cols_array_2d(),
            source_z_up: 1,
            ..ParticleUniforms::default()
        };
        let vertex = ParticleVertex {
            position: axis.transform_point3(target).to_array(),
            size: [20., 40.],
            rotation: 0.37,
            billboard: 0.,
            ..ParticleVertex::default()
        };
        let ground = expand_particle_world_corners_with_uniforms(&vertex, &uniforms);
        for corner in ground {
            assert_eq!(corner[2], 85.);
        }
        assert!(ground.iter().any(|p| (p[0] - target.x).abs() > 1.));
        assert!(ground.iter().any(|p| (p[1] - target.z).abs() > 1.));
        let billboard = ParticleVertex {
            billboard: 1.,
            rotation: 0.,
            ..vertex
        };
        let corners = expand_particle_world_corners_with_uniforms(&billboard, &uniforms);
        let source_view = Mat4::from_cols_array_2d(&uniforms.view_matrix);
        let center = source_view.transform_point3(Vec3::from_array(vertex.position));
        for (corner, expected) in corners.into_iter().zip([
            Vec3::new(-10., -20., 0.),
            Vec3::new(10., -20., 0.),
            Vec3::new(-10., 20., 0.),
            Vec3::new(10., 20., 0.),
        ]) {
            let point = source_view.transform_point3(Vec3::from_array(corner));
            assert!((point - center).abs_diff_eq(expected, 0.001));
        }
        // The default host route keeps XZ ground expansion independently.
        let host = ParticleUniforms::default();
        for corner in expand_particle_world_corners_with_uniforms(&vertex, &host) {
            assert_eq!(corner[1], vertex.position[1]);
        }
    }

    #[test]
    fn live_particle_shader_accepts_both_coordinate_domains() {
        let source = include_str!("../shaders/particle_vertex.wgsl");
        for shader in [
            source,
            include_str!("../shaders/decal.wgsl"),
            include_str!("../shaders/particle_heat_haze.wgsl"),
        ] {
            let module = naga::front::wgsl::parse_str(shader).expect("live effect shader");
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid live effect shader");
        }
        assert!(source.contains("if uniforms.source_z_up != 0u"));
        assert!(source.contains("up = vec3<f32>(0.0, 1.0, 0.0)"));
        assert!(source.contains("up = vec3<f32>(0.0, 0.0, 1.0)"));
        assert_eq!(std::mem::size_of::<ParticleUniforms>(), 160);
    }
}
