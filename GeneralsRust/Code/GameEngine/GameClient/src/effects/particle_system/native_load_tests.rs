//! C++2488 generates once on native Load, before forcecreating saved particles.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;

fn owner() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new("NativeLoadGeneration".into());
    let info = definition.info_mut();
    info.priority = ParticlePriorityType::AlwaysRender;
    info.start_size = GameClientRandomVariable::new(3.0, 3.0);
    info.start_size_rate = GameClientRandomVariable::new(2.0, 2.0);
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
    info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
    info.alpha_keys[0].min_value = 1.0;
    info.alpha_keys[0].max_value = 1.0;
    info.alpha_keys[1].min_value = 1.0;
    info.alpha_keys[1].max_value = 1.0;
    info.alpha_keys[1].frame = 100;
    manager.register_template(Arc::new(definition));
    manager
}

fn authored_payload(count: usize, identity: bool, bonus: f32, store: u32) -> Vec<u8> {
    let mut source = owner();
    let definition = source.find_template("NativeLoadGeneration").unwrap();
    let id = source
        .create_particle_system_with_random(&definition, false, &mut |lo, _| lo)
        .unwrap();
    for index in 0..count {
        let info = ParticleInfo {
            position: Vec3::new(index as f32, 2.0, 3.0),
            lifetime: 50,
            size: 31.0 + index as f32,
            ..Default::default()
        };
        source
            .admit_particle(id, Particle::new(&info, 7 + index as u32, 9))
            .unwrap();
    }
    let system = source.find_particle_system_mut(id).unwrap();
    // These are independent authored native fields, not a decoded expected result.
    system.personality_counter = store;
    system.accumulated_size_bonus = bonus;
    system.position = Vec3::new(10.0, 20.0, 30.0);
    system.last_position = Vec3::new(-1.0, -2.0, -3.0);
    system.transform_translation = Vec3::new(10.0, 20.0, 30.0);
    system.is_identity = identity;
    system.is_first_pos = true;
    system.is_stopped = true;
    system.delay_left = 3;
    source.capture_xfer_bytes().unwrap()
}

#[test]
fn native_particle_load_generation_side_effects_and_actual_next_admission() {
    let payload = authored_payload(3, false, 5.0, u32::MAX - 1);
    let mut receiving = owner();
    let definition = receiving.find_template("NativeLoadGeneration").unwrap();
    receiving.restore_xfer_bytes(&payload).unwrap().unwrap();
    let system = receiving.find_particle_system_mut(1).unwrap();
    assert_eq!(system.accumulated_size_bonus, 7.0);
    assert!(!system.is_first_pos);
    assert_eq!(system.last_position, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(system.personality_counter, 1);
    assert_eq!(
        system.delay_left, 3,
        "load generation is not an update tick"
    );
    assert!(system.is_stopped);
    assert!(Arc::ptr_eq(system.template(), &definition));
    for (index, particle) in system.particles().iter().enumerate() {
        assert_eq!(particle.personality, 7 + index as u32);
        assert_eq!(particle.position, Vec3::new(index as f32, 2.0, 3.0));
        assert_eq!(particle.size, 31.0 + index as f32);
        assert_eq!(particle.lifetime, 50);
        assert_eq!(particle.create_timestamp, 9);
    }
    system.set_initial_delay(0);
    system.start();
    receiving.update(0, 1);
    let system = receiving.find_particle_system(1).unwrap();
    let particle = system.particles().back().unwrap();
    assert_eq!(particle.personality, 1);
    assert_eq!(particle.size, 10.0, "base3 plus load-advanced bonus7");
    assert_eq!(system.personality_counter, 2);
    assert_eq!(system.accumulated_size_bonus, 9.0);
}

#[test]
fn native_particle_load_generation_side_effects_include_zero_count_and_identity() {
    for identity in [false, true] {
        let mut receiving = owner();
        receiving
            .restore_xfer_bytes(&authored_payload(0, identity, 5.0, 13))
            .unwrap()
            .unwrap();
        let system = receiving.find_particle_system(1).unwrap();
        assert_eq!(system.particle_count(), 0);
        assert_eq!(system.personality_counter, 13);
        assert_eq!(
            system.accumulated_size_bonus, 7.0,
            "one generation even when the saved list is empty"
        );
        assert_eq!(system.is_first_pos, identity);
        assert_eq!(
            system.last_position,
            if identity {
                Vec3::new(-1.0, -2.0, -3.0)
            } else {
                Vec3::new(10.0, 20.0, 30.0)
            }
        );
        assert_eq!(system.delay_left, 3);
        assert!(system.is_stopped);
    }
}

#[test]
fn native_particle_load_generation_side_effects_cap_and_repeated_load_are_explicit() {
    let mut receiving = owner();
    receiving
        .restore_xfer_bytes(&authored_payload(0, false, 49.5, 13))
        .unwrap()
        .unwrap();
    assert_eq!(
        receiving
            .find_particle_system(1)
            .unwrap()
            .accumulated_size_bonus,
        50.0
    );
    let bytes = receiving.capture_xfer_bytes().unwrap();
    receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert_eq!(
        receiving
            .find_particle_system(1)
            .unwrap()
            .accumulated_size_bonus,
        50.0
    );
    assert_eq!(
        receiving
            .find_particle_system(1)
            .unwrap()
            .personality_counter,
        13
    );
}

#[test]
fn native_particle_load_generation_side_effects_failed_candidate_does_not_publish() {
    let mut receiving = owner();
    receiving
        .restore_xfer_bytes(&authored_payload(3, false, 5.0, u32::MAX - 1))
        .unwrap()
        .unwrap();
    let before = receiving.capture_xfer_bytes().unwrap();
    let before_system = receiving.find_particle_system(1).unwrap();
    let before_bonus = before_system.accumulated_size_bonus;
    let before_store = before_system.personality_counter;
    let owner = receiving.runtime_owner();
    let truncated = authored_payload(2, true, 20.0, 100);
    assert!(
        receiving
            .restore_xfer_bytes(&truncated[..truncated.len() - 1])
            .is_err()
    );
    assert_eq!(receiving.capture_xfer_bytes().unwrap(), before);
    assert_eq!(receiving.runtime_owner(), owner);
    let system = receiving.find_particle_system(1).unwrap();
    assert_eq!(system.accumulated_size_bonus, before_bonus);
    assert_eq!(system.personality_counter, before_store);
}
