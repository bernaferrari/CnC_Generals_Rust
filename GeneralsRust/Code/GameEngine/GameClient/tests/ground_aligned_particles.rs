//! IsGroundAligned systems must lie flat on world-XZ (host is Y-up).
//!
//! C++ PointGroup `Set_Billboard(shouldBillboard())` with `shouldBillboard = !m_isGroundAligned`
//! keeps the ground-axis constant (Z-up world-XY). The live wgpu path must remap that to XZ.

use std::sync::Arc;

use game_client_rust::effects::particle_manager::{ParticleSystemTemplate, ParticleType};
use game_client_rust::effects::particle_renderer::{
    bake_particle_system_gpu_mesh, expand_particle_world_corners,
};
use game_client_rust::effects::particle_system::{Particle, ParticleInfo, ParticleSystem};
use nalgebra::Point3;

#[test]
fn collect_system_particles_emits_world_xz_quads_when_ground_aligned() {
    let mut template = ParticleSystemTemplate::new("ground".to_string());
    template.info_mut().particle_type = ParticleType::Particle;
    template.info_mut().is_ground_aligned = true;
    let mut system = ParticleSystem::new(Arc::new(template), 1, false);
    let mut info = ParticleInfo::default();
    info.position = Point3::new(10.0, 5.0, 20.0);
    info.size = 4.0;
    system.push_particle(Particle::new(&info, 0, 0));

    let vertices = bake_particle_system_gpu_mesh(&system);
    assert_eq!(vertices.len(), 1);
    assert_eq!(vertices[0].billboard, 0.0);
    assert!(!system.should_billboard());

    let corners = expand_particle_world_corners(&vertices[0]);
    for corner in corners {
        assert!(
            (corner[1] - 5.0).abs() < 1e-5,
            "ground-aligned quad must stay on world-XZ (Y-up); got {corner:?}"
        );
    }
    let spans_x = corners.iter().any(|c| (c[0] - 10.0).abs() > 0.1);
    let spans_z = corners.iter().any(|c| (c[2] - 20.0).abs() > 0.1);
    assert!(
        spans_x && spans_z,
        "quad must extend in X and Z, not stand up: {corners:?}"
    );

    let shader = include_str!("../src/effects/shaders/particle_vertex.wgsl");
    assert!(
        shader.contains("up = vec3<f32>(0.0, 0.0, 1.0)"),
        "live vertex shader must use world-Z as the ground-quad V axis (Y-up host)"
    );
    assert!(
        !shader.contains("up = vec3<f32>(0.0, 1.0, 0.0)"),
        "world-Y as V stands particles up on a Y-up host"
    );
}
