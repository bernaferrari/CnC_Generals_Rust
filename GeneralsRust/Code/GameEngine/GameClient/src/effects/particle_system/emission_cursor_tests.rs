use super::*;

#[test]
fn particle_emission_truncates_fractional_samples_before_coefficients() {
    let mut template = ParticleSystemTemplate::new("FractionalEmission".into());
    template.info_mut().burst_count = GameClientRandomVariable::new(2.9, 2.9);
    template.info_mut().burst_delay = GameClientRandomVariable::new(2.9, 2.9);
    let mut system = ParticleSystem::new_with_random(Arc::new(template), 1, false, &mut |lo, _| lo);
    system.count_coeff = 2.0;
    system.delay_coeff = 2.0;
    system.begin_frame_emit(0, 1);
    assert_eq!(system.take_pending_emissions().len(), 4);
    assert_eq!(system.burst_delay_left, 4);
    for frame in 2..=5 {
        system.begin_frame_emit(0, frame);
        assert!(system.take_pending_emissions().is_empty());
        assert_eq!(system.burst_delay_left, 5 - frame);
    }
    system.begin_frame_emit(0, 6);
    assert_eq!(system.take_pending_emissions().len(), 4);
    assert_eq!(system.burst_delay_left, 4);
}

#[test]
fn particle_zero_burst_still_finishes_delay_before_next_attempt() {
    let mut template = ParticleSystemTemplate::new("ZeroBurstDelay".into());
    template.info_mut().burst_count = GameClientRandomVariable::new(0.9, 0.9);
    template.info_mut().burst_delay = GameClientRandomVariable::new(2.9, 2.9);
    let mut system = ParticleSystem::new_with_random(Arc::new(template), 1, false, &mut |lo, _| lo);
    system.delay_coeff = 2.0;
    system.begin_frame_emit(0, 1);
    assert!(system.take_pending_emissions().is_empty());
    assert_eq!(system.burst_delay_left, 4);
    for frame in 2..=5 {
        system.begin_frame_emit(0, frame);
        assert!(system.take_pending_emissions().is_empty());
        assert_eq!(system.burst_delay_left, 5 - frame);
    }
    system.begin_frame_emit(0, 6);
    assert!(system.take_pending_emissions().is_empty());
    assert_eq!(system.burst_delay_left, 4);
}

#[test]
fn particle_stop_start_destroy_preserve_native_saved_lifecycle_identity() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let template = Arc::new(ParticleSystemTemplate::new("LifecycleFlags".into()));
    let mut saved = ParticleSystem::new_with_random(template.clone(), 1, false, &mut |lo, _| lo);
    saved.start_timestamp = 99;
    saved.stop();
    let mut bytes = Vec::new();
    let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
    save.open("lifecycle").unwrap();
    saved.xfer(&mut save).unwrap();
    save.close().unwrap();
    drop(save);
    let mut loaded = ParticleSystem::new_with_random(template, 0, false, &mut |lo, _| lo);
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("lifecycle").unwrap();
    loaded.xfer(&mut load).unwrap();
    load.close().unwrap();
    assert!(loaded.is_stopped);
    loaded.start();
    assert!(!loaded.is_stopped);
    assert_eq!(loaded.start_frame(), 99);
    loaded.destroy();
    assert!(loaded.is_destroyed());
    assert!(
        !loaded.is_stopped,
        "destroy does not change the independent stop flag"
    );
    loaded.start();
    assert!(
        loaded.is_destroyed(),
        "start cannot resurrect a destroyed system"
    );
    assert_eq!(loaded.start_frame(), 99);
    assert!(matches!(
        loaded.begin_frame_emit(0, 100),
        FrameEmitPhase::Dead
    ));
}

#[test]
fn actual_parent_expiry_detaches_delayed_controlled_child_before_destroy() {
    let mut manager = ParticleSystemManager::new();
    let mut parent = ParticleSystemTemplate::new("ExpiringControlParent".into());
    let info = parent.info_mut();
    info.priority = ParticlePriorityType::AlwaysRender;
    info.shader_type = ParticleShaderType::Alpha;
    info.lifetime = GameClientRandomVariable::new(2.0, 2.0);
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.burst_delay = GameClientRandomVariable::new(100.0, 100.0);
    info.alpha_keys[0].min_value = 1.0;
    info.alpha_keys[0].max_value = 1.0;
    info.alpha_keys[1].min_value = 1.0;
    info.alpha_keys[1].max_value = 1.0;
    info.alpha_keys[1].frame = 100;
    info.attached_system_name = "DelayedControlChild".into();
    let mut child = ParticleSystemTemplate::new("DelayedControlChild".into());
    child.info_mut().initial_delay = GameClientRandomVariable::new(3.0, 3.0);
    manager.register_template(Arc::new(child));
    let parent_id = manager
        .create_particle_system_with_random(&Arc::new(parent), false, &mut |lo, _| lo)
        .unwrap();
    manager
        .find_particle_system_mut(parent_id)
        .unwrap()
        .set_position(Vec3::new(10.0, 20.0, 30.0));
    manager.update(0, 1);
    let child_id = manager.find_particle_system(parent_id).unwrap().particles()[0]
        .controlled_system
        .unwrap();
    let child = manager.find_particle_system(child_id).unwrap();
    assert_eq!(child.initial_delay_left(), 2);
    assert_eq!(
        child.control_particle_pos,
        Some(Vec3::new(10.0, 20.0, 30.0))
    );
    manager.destroy_particle_system(child_id);
    assert_eq!(
        manager
            .find_particle_system(child_id)
            .unwrap()
            .control_particle_pos,
        Some(Vec3::new(10.0, 20.0, 30.0)),
        "explicit destroy retains the still-live control binding"
    );
    manager.update(0, 2);
    assert_eq!(
        manager
            .find_particle_system(parent_id)
            .unwrap()
            .particle_count(),
        0
    );
    let child = manager.find_particle_system(child_id).unwrap();
    assert!(child.is_destroyed());
    assert_eq!(child.initial_delay_left(), 1);
    assert_eq!(
        child.control_particle_pos, None,
        "actual parent particle destructor detaches before marking the child destroyed"
    );
    manager.update(0, 3);
    assert_eq!(
        manager
            .find_particle_system(child_id)
            .unwrap()
            .control_particle_pos,
        None
    );
    manager.update(0, 4);
    assert!(manager.find_particle_system(child_id).is_none());
}

#[test]
fn destroyed_empty_preparation_advances_wind_before_retirement() {
    let mut template = ParticleSystemTemplate::new("RetiringWindPhase".into());
    let info = template.info_mut();
    info.wind_motion = WindMotion::PingPong;
    info.wind_motion_start_angle_min = 0.25;
    info.wind_motion_start_angle_max = 0.25;
    info.wind_motion_end_angle_min = 0.75;
    info.wind_motion_end_angle_max = 0.75;
    info.wind_motion_moving_to_end_angle = true;
    info.wind_angle_change = 0.02;
    let mut system = ParticleSystem::new_with_random(Arc::new(template), 1, false, &mut |lo, _| lo);
    system.destroy();
    assert_eq!(system.wind_angle(), 0.25);
    assert!(matches!(
        system.begin_frame_emit(0, 1),
        FrameEmitPhase::Dead
    ));
    assert_eq!(system.wind_angle(), 0.25 + 0.005);
}
