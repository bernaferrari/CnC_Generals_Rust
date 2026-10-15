//! C++ W3DParticleSys.cpp:137-224 counts the ordinary accepted point group.
use super::*;
use crate::effects::particle_system::{Particle, ParticleInfo};

fn admitted_fields(
    kind: ParticleType,
    name: &str,
    count: usize,
) -> (ParticleSystemManager, ParticleSystemId) {
    admitted_with_policy(
        kind,
        name,
        count,
        ParticlePriorityType::AreaEffect,
        true,
        ParticleShaderType::Alpha,
    )
}

fn admitted_with_policy(
    kind: ParticleType,
    name: &str,
    count: usize,
    priority: ParticlePriorityType,
    ground: bool,
    shader: ParticleShaderType,
) -> (ParticleSystemManager, ParticleSystemId) {
    let mut manager = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new("FieldRenderScope".into());
    let info = definition.info_mut();
    info.particle_type = kind;
    info.shader_type = shader;
    info.particle_type_name = name.into();
    info.priority = priority;
    info.is_ground_aligned = ground;
    let definition = Arc::new(definition);
    let id = manager
        .create_particle_system_with_random(&definition, false, &mut |lo, _| lo)
        .unwrap();
    for personality in 0..count {
        let info = ParticleInfo {
            lifetime: 100,
            size: 1.0,
            ..Default::default()
        };
        manager
            .admit_particle(id, Particle::new(&info, personality as u32, 0))
            .unwrap();
    }
    (manager, id)
}

#[test]
fn field_particle_count_stops_at_cpp_512_visible_point_group() {
    let (mut manager, id) = admitted_fields(ParticleType::Particle, "Field.tga", 601);
    manager
        .find_particle_system_mut(id)
        .unwrap()
        .particles_mut()[0]
        .position
        .x = 100.0;
    manager.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 512);
    assert_eq!(manager.field_particle_count(), 512);
    let system = manager.find_particle_system(id).unwrap();
    assert_eq!(
        system.particles().iter().filter(|p| !p.is_culled).count(),
        512
    );
    assert!(
        system.particles()[0].is_culled,
        "outside particles do not consume the group budget"
    );
    assert!(!system.particles()[512].is_culled);
    assert!(system.particles()[513].is_culled);
    assert_eq!(
        manager.particle_count(),
        601,
        "render culling never deletes live particles"
    );
}

#[test]
fn field_particle_count_excludes_drawable_and_smud_prefix_paths() {
    for (kind, name, expected_culled) in [
        (ParticleType::Drawable, "FieldDrawable", 0),
        (ParticleType::Particle, "SMUDfire.tga", 0),
        (ParticleType::Smudge, "SMUDGE RESERVED", 0),
    ] {
        let (mut manager, id) = admitted_fields(kind, name, 600);
        manager.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 512);
        assert_eq!(manager.field_particle_count(), 0, "{kind:?} {name}");
        let system = manager.find_particle_system(id).unwrap();
        assert_eq!(
            system.particles().iter().filter(|p| p.is_culled).count(),
            expected_culled
        );
    }
}

#[test]
fn field_particle_count_occurs_before_ordinary_gpu_type_and_shader_dispatch() {
    // C++ excludes ONLY Drawable and the SMUD DWORD prefix before counting.
    // Nonprefix SMUDGE and other unsupported types still use this capped loop.
    for (kind, shader) in [
        (ParticleType::Streak, ParticleShaderType::Alpha),
        (ParticleType::VolumeParticle, ParticleShaderType::Alpha),
        (ParticleType::Smudge, ParticleShaderType::Alpha),
        (ParticleType::Invalid, ParticleShaderType::Alpha),
        (ParticleType::Particle, ParticleShaderType::Invalid),
    ] {
        let (mut manager, _) = admitted_with_policy(
            kind,
            "NonHeat.tga",
            600,
            ParticlePriorityType::AreaEffect,
            true,
            shader,
        );
        manager.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 512);
        assert_eq!(manager.field_particle_count(), 512, "{kind:?}");
    }
}

#[test]
fn field_particle_count_requires_area_effect_and_ground_alignment() {
    for (priority, ground) in [
        (ParticlePriorityType::Constant, true),
        (ParticlePriorityType::AreaEffect, false),
    ] {
        let (mut manager, _) = admitted_with_policy(
            ParticleType::Particle,
            "Field.tga",
            2,
            priority,
            ground,
            ParticleShaderType::Alpha,
        );
        manager.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 512);
        assert_eq!(manager.field_particle_count(), 0);
    }
}
