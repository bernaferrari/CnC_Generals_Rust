use super::*;

fn admit(manager: &mut ParticleSystemManager) -> ParticleSystemId {
    let mut definition = ParticleSystemTemplate::new("ResetClock".into());
    let info = definition.info_mut();
    info.priority = ParticlePriorityType::AlwaysRender;
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
    info.lifetime = GameClientRandomVariable::new(30.0, 30.0);
    for key in &mut info.alpha_keys {
        key.min_value = 1.0;
        key.max_value = 1.0;
    }
    let definition = Arc::new(definition);
    manager.register_template(definition.clone());
    manager.create_particle_system(&definition, false).unwrap()
}

#[test]
fn receiving_reset_clock_runs_frame_zero_once_and_keeps_peer_independent() {
    let mut a = ParticleSystemManager::new();
    let original = admit(&mut a);
    a.update(0, 0);
    assert_eq!(
        a.find_particle_system(original).unwrap().particle_count(),
        0,
        "C++ constructor starts at frame zero and skips it"
    );
    SubsystemInterface::reset(&mut a).unwrap();
    let id = admit(&mut a);
    a.update(0, 0);
    assert_eq!(a.find_particle_system(id).unwrap().particle_count(), 1);
    a.update(0, 0);
    assert_eq!(a.find_particle_system(id).unwrap().particle_count(), 1);
    let mut b = ParticleSystemManager::new();
    SubsystemInterface::reset(&mut b).unwrap();
    let peer = admit(&mut b);
    b.update(0, 0);
    assert_eq!(id, peer);
    assert_eq!(b.find_particle_system(peer).unwrap().particle_count(), 1);
    SubsystemInterface::reset(&mut b).unwrap();
    a.update(0, 0);
    assert_eq!(a.find_particle_system(id).unwrap().particle_count(), 1);
    a.update(0, 1);
    assert_eq!(a.find_particle_system(id).unwrap().particle_count(), 2);
    assert_eq!(b.active_system_count(), 0);
}
