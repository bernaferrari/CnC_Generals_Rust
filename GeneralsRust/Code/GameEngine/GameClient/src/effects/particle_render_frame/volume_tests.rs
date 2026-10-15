//! Actual CPU collection against C++ W3D source-group/layer-loop observations.
//! No GPU image equivalence or global heat-manager fixture is involved.
use super::*;
use crate::effects::particle_system::ParticleInfo;

fn frame() -> ParticleRenderFrame {
    ParticleRenderFrame {
        uniforms: ParticleUniforms::default(),
        batches: Vec::new(),
        particles_rendered: 0,
        preparation_time: std::time::Duration::ZERO,
        heat_smudges: Vec::new(),
    }
}

fn add_system(manager: &mut ParticleSystemManager, id: u32, ground: bool, depth: u32) {
    let mut definition = ParticleSystemTemplate::new(format!("Volume{id}"));
    let info = definition.info_mut();
    info.particle_type = ParticleType::VolumeParticle;
    info.particle_type_name = "SharedVolume.tga".into();
    info.shader_type = ParticleShaderType::Alpha;
    info.priority = ParticlePriorityType::AreaEffect;
    info.is_ground_aligned = ground;
    info.volume_particle_depth = depth;
    manager
        .create_particle_system_with_id_and_random(
            &Arc::new(definition),
            id,
            false,
            &mut |lo, _| lo,
        )
        .unwrap();
}

fn admit(manager: &mut ParticleSystemManager, id: u32, position: Vec3, size: f32, alpha: f32) {
    let mut info = ParticleInfo {
        position,
        size,
        lifetime: 100,
        ..Default::default()
    };
    info.alpha_keys[0].value = alpha;
    manager
        .admit_particle(id, Particle::new(&info, 17, 0))
        .unwrap();
}

#[test]
fn prepared_volume_group_limits_512_sources_not_the_3072_layer_instances() {
    for apply_visible_cull in [false, true] {
        let mut manager = ParticleSystemManager::new();
        add_system(&mut manager, 19, true, 0);
        for index in 0..513 {
            admit(
                &mut manager,
                19,
                Vec3::new(index as f32, 0.0, 0.0),
                1.0,
                0.75,
            );
        }
        if apply_visible_cull {
            manager.cull_particles_to_visible_box([0.0; 3], [1000.0; 3], 512);
            assert_eq!(manager.field_particle_count(), 512);
            assert!(manager.find_particle_system(19).unwrap().particles()[512].is_culled);
        }
        let mut prepared = frame();
        prepared.collect_system_particles(manager.find_particle_system(19).unwrap(), [0.0; 3]);
        assert_eq!(prepared.batches.len(), 1);
        let vertices = &prepared.batches[0].vertices;
        assert_eq!(vertices.len(), 3072, "512 sources on all six layers");
        assert_eq!(
            prepared.particles_rendered, 3072,
            "metric counts expanded instances"
        );
        for layer in vertices.chunks_exact(512) {
            for (index, vertex) in layer.iter().enumerate() {
                assert_eq!(vertex.position, [index as f32, 0.0, 0.0]);
                assert_eq!(vertex.alpha, 0.75);
            }
        }
        assert_eq!(
            manager.find_particle_system(19).unwrap().particle_count(),
            513
        );
        assert_eq!(
            manager.particle_count(),
            513,
            "render budget cannot delete simulation particles"
        );
    }
}

#[test]
fn prepared_volume_layers_keep_cpp_order_first_visible_size_shift_and_full_alpha() {
    let mut manager = ParticleSystemManager::new();
    add_system(&mut manager, 19, false, 0);
    admit(&mut manager, 19, Vec3::new(100.0, 0.0, 0.0), 900.0, 0.2);
    admit(&mut manager, 19, Vec3::new(0.0, 0.0, 10.0), 6.0, 0.8);
    admit(&mut manager, 19, Vec3::new(10.0, 0.0, 0.0), 30.0, 0.4);
    manager
        .find_particle_system_mut(19)
        .unwrap()
        .particles_mut()[0]
        .is_culled = true;
    let mut prepared = frame();
    prepared.collect_system_particles(manager.find_particle_system(19).unwrap(), [0.0; 3]);
    let vertices = &prepared.batches[0].vertices;
    assert_eq!(vertices.len(), 12);
    // pointgr.cpp:1734-1764: shift=t*FIRST current_size*(0.1/depth),
    // toward camera, without centering or attenuation. Outer loop is layer.
    for (layer, expected_coordinate) in [10.0, 9.9, 9.8, 9.7, 9.6, 9.5].into_iter().enumerate() {
        let a = &vertices[layer * 2];
        let b = &vertices[layer * 2 + 1];
        assert_eq!(a.position, [0.0, 0.0, expected_coordinate]);
        assert_eq!(b.position, [expected_coordinate, 0.0, 0.0]);
        assert_eq!(a.size, [6.0; 2]);
        assert_eq!(b.size, [30.0; 2]);
        assert_eq!(a.alpha, 0.8);
        assert_eq!(b.alpha, 0.4);
    }
}

#[test]
fn prepared_volume_ground_layers_and_camera_coincidence_do_not_invent_offsets() {
    for (ground, camera) in [(true, [0.0; 3]), (false, [1.0, 2.0, 3.0])] {
        let mut manager = ParticleSystemManager::new();
        add_system(&mut manager, 19, ground, 0);
        admit(&mut manager, 19, Vec3::new(1.0, 2.0, 3.0), 6.0, 0.75);
        let mut prepared = frame();
        prepared.collect_system_particles(manager.find_particle_system(19).unwrap(), camera);
        assert_eq!(prepared.batches[0].vertices.len(), 6);
        for vertex in &prepared.batches[0].vertices {
            assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
            assert_eq!(vertex.alpha, 0.75);
            assert_eq!(vertex.billboard, if ground { 0.0 } else { 1.0 });
        }
    }
}

#[test]
fn prepared_volume_uses_fixed_cpp_render_depth_without_mutating_native_metadata() {
    for saved_depth in [0, 1, 6, 16, u32::MAX] {
        let mut manager = ParticleSystemManager::new();
        add_system(&mut manager, 19, true, saved_depth);
        admit(&mut manager, 19, Vec3::ZERO, 6.0, 0.75);
        let mut prepared = frame();
        let system = manager.find_particle_system(19).unwrap();
        prepared.collect_system_particles(system, [0.0; 3]);
        assert_eq!(prepared.batches[0].vertices.len(), 6);
        assert_eq!(system.info().volume_particle_depth, saved_depth);
    }
}

#[test]
fn prepared_volume_finishes_all_layers_before_the_next_actual_system() {
    let mut manager = ParticleSystemManager::new();
    for (id, x) in [(19, 1.0), (3, 2.0)] {
        add_system(&mut manager, id, true, 0);
        admit(&mut manager, id, Vec3::new(x, 0.0, 0.0), 6.0, 0.75);
    }
    let mut prepared = frame();
    for system in manager.draw_particle_systems() {
        prepared.collect_system_particles(system, [0.0; 3]);
    }
    assert_eq!(
        prepared.batches.len(),
        2,
        "equal texture/shader systems stay ordered submissions"
    );
    for (batch, x) in prepared.batches.iter().zip([1.0, 2.0]) {
        assert_eq!(batch.texture_name, "SharedVolume.tga");
        assert_eq!(batch.vertices.len(), 6);
        assert!(
            batch
                .vertices
                .iter()
                .all(|vertex| vertex.position == [x, 0.0, 0.0])
        );
    }
    let frozen = prepared.clone();
    manager
        .find_particle_system_mut(19)
        .unwrap()
        .particles_mut()[0]
        .position
        .x = 99.0;
    manager.destroy_particle_system(3);
    assert_eq!(
        prepared, frozen,
        "prepared layers cannot follow later live owner changes"
    );
}
