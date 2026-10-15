use super::*;
use crate::effects::particle_manager::ParticleSystemTemplate;
use crate::effects::particle_system::ParticleInfo;
use std::sync::Arc;

#[test]
fn particle_integration_final_destroyed_particle_skips_system_lifetime_tick() {
    let mut template = ParticleSystemTemplate::new("DestroyedLifetimePhase".into());
    template.info_mut().system_lifetime = 9;
    let mut system = ParticleSystem::new_with_random(Arc::new(template), 1, false, &mut |lo, _| lo);
    let mut info = ParticleInfo::default();
    info.lifetime = 1;
    info.alpha_keys[0].value = 1.0;
    system.push_particle(Particle::new(&info, 0, 0));
    system.destroy();
    assert_eq!(system.system_lifetime_left, 9);
    system.finish_frame_integrate(1);
    assert_eq!(system.particle_count(), 0);
    assert!(system.should_remove());
    assert_eq!(
        system.system_lifetime_left, 9,
        "C++ returns before finite lifetime after destroyed final particle removal"
    );
}

#[test]
fn particle_admission_wrap_binds_new_identity_not_repeated_personality() {
    use crate::effects::particle_manager::{
        GameClientRandomVariable, ParticlePriorityType, ParticleSystemManager,
    };
    let mut manager = ParticleSystemManager::new();
    for (name, count, attached) in [
        ("WrappingParent", 1.0, "WrappingChild"),
        ("WrappingChild", 0.0, ""),
    ] {
        let mut definition = ParticleSystemTemplate::new(name.into());
        let info = definition.info_mut();
        info.priority = ParticlePriorityType::AlwaysRender;
        info.burst_count = GameClientRandomVariable::new(count, count);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
        info.attached_system_name = attached.into();
        info.alpha_keys[0].min_value = 1.0;
        info.alpha_keys[0].max_value = 1.0;
        info.alpha_keys[1].min_value = 1.0;
        info.alpha_keys[1].max_value = 1.0;
        info.alpha_keys[1].frame = 100;
        manager.register_template(Arc::new(definition));
    }
    let template = manager.find_template("WrappingParent").unwrap();
    let parent = manager
        .create_particle_system_with_random(&template, false, &mut |lo, _| lo)
        .unwrap();
    manager.update(0, 1);
    let old_child = manager.find_particle_system(parent).unwrap().particles()[0]
        .controlled_system
        .unwrap();
    manager
        .find_particle_system_mut(parent)
        .unwrap()
        .particles_mut()[0]
        .personality = u32::MAX;
    // Repeat the saved unsigned personality while its older particle remains alive.
    // Runtime receiver identity must still bind the new particle, not the old one.
    manager
        .find_particle_system_mut(parent)
        .unwrap()
        .personality_counter = u32::MAX;
    manager.update(0, 2);
    let system = manager.find_particle_system(parent).unwrap();
    assert_eq!(system.personality_counter(), 0);
    let particles = system.particles();
    assert_eq!(particles.len(), 2);
    assert_eq!(particles[0].personality, u32::MAX);
    assert_eq!(particles[1].personality, u32::MAX);
    assert_eq!(particles[0].controlled_system, Some(old_child));
    let new_child = particles[1]
        .controlled_system
        .expect("new admission has its own attached child");
    assert_ne!(old_child, new_child);
    assert_eq!(
        manager
            .all_particle_systems()
            .map(|system| system.system_id())
            .collect::<Vec<_>>(),
        vec![parent, old_child, new_child]
    );
    for child in [old_child, new_child] {
        assert_eq!(
            manager
                .find_particle_system(child)
                .unwrap()
                .template()
                .name(),
            "WrappingChild"
        );
    }
}
