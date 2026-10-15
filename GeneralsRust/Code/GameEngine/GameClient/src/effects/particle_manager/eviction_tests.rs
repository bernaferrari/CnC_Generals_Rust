use super::*;
use crate::effects::particle_system::{Particle, ParticleInfo};

fn template(name: &str, priority: ParticlePriorityType) -> Arc<ParticleSystemTemplate> {
    let mut template = ParticleSystemTemplate::new(name.into());
    let info = template.info_mut();
    info.priority = priority;
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
    info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
    info.alpha_keys[0].min_value = 1.0;
    info.alpha_keys[0].max_value = 1.0;
    info.alpha_keys[1].min_value = 1.0;
    info.alpha_keys[1].max_value = 1.0;
    info.alpha_keys[1].frame = 100;
    Arc::new(template)
}

fn owner() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    for (id, name) in [(77, "FifoA"), (3, "FifoB")] {
        let template = template(name, ParticlePriorityType::Critical);
        manager.register_template(template.clone());
        manager
            .create_particle_system_with_id_and_random(&template, id, false, &mut |lo, _| lo)
            .unwrap();
        manager.find_particle_system_mut(id).unwrap().stop();
    }
    manager
}

fn interleaved() -> ParticleSystemManager {
    let mut manager = owner();
    for (frame, emitting) in [(1, 77), (2, 3), (3, 77)] {
        for id in [77, 3] {
            let system = manager.find_particle_system_mut(id).unwrap();
            if id == emitting {
                system.start();
            } else {
                system.stop();
            }
        }
        manager.update(0, frame);
    }
    // Timestamps are not identity or FIFO evidence; all particles may have equal ones.
    for id in [77, 3] {
        for particle in manager
            .find_particle_system_mut(id)
            .unwrap()
            .particles_mut()
        {
            particle.create_timestamp = 0;
        }
    }
    manager
}

fn inline_owner() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    for (id, name) in [(77, "InlineMaster"), (3, "InlineSlave")] {
        let mut definition = (*template(name, ParticlePriorityType::Critical)).clone();
        definition.info_mut().burst_count = GameClientRandomVariable::new(2.0, 2.0);
        let definition = Arc::new(definition);
        manager.register_template(definition.clone());
        manager
            .create_particle_system_with_id_and_random(&definition, id, false, &mut |lo, _| lo)
            .unwrap();
    }
    manager
        .find_particle_system_mut(77)
        .unwrap()
        .set_slave(Some(3));
    manager
        .find_particle_system_mut(3)
        .unwrap()
        .set_master(Some(77));
    manager.update(0, 1);
    manager
}

#[test]
fn particle_eviction_actual_interleaved_fifo_and_peer_isolation() {
    let mut manager = inline_owner();
    let peer = inline_owner();
    assert_eq!(manager.particle_count(), 4);
    for id in [77, 3] {
        let particles = manager.find_particle_system(id).unwrap().particles();
        assert_eq!(particles.len(), 2);
        assert_eq!(particles[0].create_timestamp, 1);
        assert_eq!(particles[1].create_timestamp, 1);
        assert_eq!(particles[0].personality, 0);
        assert_eq!(particles[1].personality, 1);
    }
    assert_eq!(
        manager.remove_oldest_particles(2, ParticlePriorityType::AlwaysRender),
        2
    );
    for id in [77, 3] {
        let particles = manager.find_particle_system(id).unwrap().particles();
        assert_eq!(particles.len(), 1);
        assert_eq!(particles[0].personality, 1);
    }
    assert_eq!(manager.particle_count(), 2);
    assert_eq!(peer.find_particle_system(77).unwrap().particle_count(), 2);
    assert_eq!(peer.find_particle_system(3).unwrap().particle_count(), 2);
}

#[test]
fn particle_eviction_leaves_last_rendered_field_observation_unchanged() {
    let mut manager = interleaved();
    manager.field_particle_count = 43;
    assert_eq!(
        manager.remove_oldest_particles(2, ParticlePriorityType::AlwaysRender),
        2
    );
    assert_eq!(manager.field_particle_count(), 43);
    assert_eq!(manager.particle_count(), 1);
    manager.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 10);
    assert_eq!(manager.field_particle_count(), 0);
}

#[test]
fn particle_eviction_native_restore_admits_in_stream_order_and_failure_is_atomic() {
    let mut source = interleaved();
    let bytes = source.capture_xfer_bytes().unwrap();
    let mut restored = owner();
    restored.field_particle_count = 37;
    restored.set_on_screen_particle_count(23);
    restored.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert_eq!(restored.field_particle_count(), 37);
    assert_eq!(restored.on_screen_particle_count(), 23);
    let ordinal = restored.next_particle_ordinal;
    let before = restored.capture_xfer_bytes().unwrap();
    assert!(
        restored
            .restore_xfer_bytes(&bytes[..bytes.len() - 1])
            .is_err()
    );
    assert_eq!(restored.next_particle_ordinal, ordinal);
    assert_eq!(restored.field_particle_count(), 37);
    assert_eq!(restored.on_screen_particle_count(), 23);
    assert_eq!(restored.capture_xfer_bytes().unwrap(), before);
    // C++ re-creates particles grouped by saved system then local FIFO, not by
    // the former global interleave (which the native payload does not contain).
    assert_eq!(
        restored.remove_oldest_particles(2, ParticlePriorityType::AlwaysRender),
        2
    );
    assert_eq!(
        restored.find_particle_system(77).unwrap().particle_count(),
        0
    );
    assert_eq!(
        restored.find_particle_system(3).unwrap().particle_count(),
        1
    );
    assert_eq!(
        source.remove_oldest_particles(2, ParticlePriorityType::AlwaysRender),
        2
    );
    assert_eq!(source.find_particle_system(77).unwrap().particle_count(), 1);
    assert_eq!(source.find_particle_system(3).unwrap().particle_count(), 0);
}

#[test]
fn particle_eviction_priority_precedes_age_and_equal_cap_is_excluded() {
    let mut manager = ParticleSystemManager::new();
    for (id, priority) in [
        (77, ParticlePriorityType::Critical),
        (3, ParticlePriorityType::DustTrail),
    ] {
        let template = template("Priority", priority);
        manager
            .create_particle_system_with_id_and_random(&template, id, false, &mut |lo, _| lo)
            .unwrap();
        manager
            .admit_particle(id, Particle::new(&ParticleInfo::default(), 0, 0))
            .unwrap();
    }
    assert_eq!(
        manager.remove_oldest_particles(2, ParticlePriorityType::Critical),
        1
    );
    assert_eq!(
        manager.find_particle_system(77).unwrap().particle_count(),
        1
    );
    assert_eq!(manager.find_particle_system(3).unwrap().particle_count(), 0);
    assert_eq!(
        manager.remove_oldest_particles(1, ParticlePriorityType::Critical),
        0
    );
}

#[test]
fn particle_eviction_controller_detaches_and_destroys_child_chain_until_normal_retirement() {
    let mut manager = ParticleSystemManager::new();
    let mut parent = (*template("ControllerParent", ParticlePriorityType::Critical)).clone();
    parent.info_mut().attached_system_name = "ControllerChild".into();
    let mut child = (*template("ControllerChild", ParticlePriorityType::Critical)).clone();
    child.info_mut().burst_count = GameClientRandomVariable::new(0.0, 0.0);
    child.info_mut().slave_system_name = "ControllerSlave".into();
    let slave = template("ControllerSlave", ParticlePriorityType::Critical);
    for t in [Arc::new(parent), Arc::new(child), slave] {
        manager.register_template(t);
    }
    let id = manager
        .create_particle_system_with_random(
            &manager.find_template("ControllerParent").unwrap(),
            true,
            &mut |lo, _| lo,
        )
        .unwrap();
    manager.update(0, 1);
    let controlled = manager.find_particle_system(id).unwrap().particles()[0]
        .controlled_system
        .unwrap();
    let slave = manager
        .find_particle_system(controlled)
        .unwrap()
        .slave_system_id()
        .unwrap();
    manager.find_particle_system_mut(id).unwrap().stop();
    for id in [controlled, slave] {
        manager
            .find_particle_system_mut(id)
            .unwrap()
            .set_initial_delay(2);
    }
    manager.field_particle_count = 17;
    assert_eq!(
        manager.remove_oldest_particles(1, ParticlePriorityType::AlwaysRender),
        1
    );
    for child in [controlled, slave] {
        assert!(manager.find_particle_system(child).unwrap().is_destroyed());
    }
    assert_eq!(manager.field_particle_count(), 17);
    let child = manager.find_particle_system_mut(controlled).unwrap();
    child.set_position(Vec3::new(99.0, 0.0, 0.0));
    child.set_initial_delay(0);
    child.begin_frame_emit(0, 2);
    assert_eq!(
        child.position(),
        Vec3::new(99.0, 0.0, 0.0),
        "eviction detached the actual control pose"
    );
    child.set_initial_delay(2);
    manager.update(0, 2);
    assert!(manager.find_particle_system(controlled).is_some());
    manager.update(0, 3);
    assert!(manager.find_particle_system(controlled).is_some());
    manager.update(0, 4);
    assert!(manager.find_particle_system(controlled).is_none());
    assert!(manager.find_particle_system(slave).is_none());
    assert_eq!(manager.field_particle_count(), 17);
}

#[test]
fn particle_eviction_admission_exhaustion_rejects_without_mutation() {
    let mut manager = owner();
    manager.next_particle_ordinal = u64::MAX;
    let count = manager.find_particle_system(77).unwrap().particle_count();
    assert!(
        manager
            .admit_particle(77, Particle::new(&ParticleInfo::default(), 0, 0))
            .is_err()
    );
    assert_eq!(manager.next_particle_ordinal, u64::MAX);
    assert_eq!(manager.particle_count(), 0);
    assert_eq!(
        manager.find_particle_system(77).unwrap().particle_count(),
        count
    );
    manager.find_particle_system_mut(77).unwrap().start();
    manager.update(0, 1);
    assert_eq!(
        manager.particle_count(),
        0,
        "runtime resource exhaustion is rejected without panic"
    );
}

#[test]
fn particle_eviction_missing_integration_id_leaves_receiver_unchanged() {
    let mut manager = interleaved();
    let before = manager.capture_xfer_bytes().unwrap();
    let count = manager.particle_count();
    let ordinal = manager.next_particle_ordinal;
    assert!(matches!(
        manager.integrate_particle_system(999, 4),
        Err(ParticleSystemError::InvalidSystemId(999))
    ));
    assert_eq!(manager.capture_xfer_bytes().unwrap(), before);
    assert_eq!(manager.particle_count(), count);
    assert_eq!(manager.next_particle_ordinal, ordinal);
}
