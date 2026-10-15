//! Canonical runtime info and immutable admitted definition identity.
use super::*;

#[test]
fn owned_particle_info_load_drives_actual_field_cull() {
    let mut source = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new("LoadedFieldCull".into());
    definition.info_mut().is_ground_aligned = true;
    definition.info_mut().priority = ParticlePriorityType::AreaEffect;
    let definition = Arc::new(definition);
    source.register_template(definition.clone());
    let id = source.create_particle_system(&definition, false).unwrap();
    for (index, x) in [0.0, 1.0, 100.0].into_iter().enumerate() {
        let info = ParticleInfo {
            position: Vec3::new(x, 0.0, 0.0),
            lifetime: 30,
            size: 1.0,
            ..Default::default()
        };
        source
            .find_particle_system_mut(id)
            .unwrap()
            .push_particle(Particle::new(&info, index as u32, 0));
    }
    let bytes = source.capture_xfer_bytes().unwrap();

    let mut receiving = ParticleSystemManager::new();
    let mut authored = ParticleSystemTemplate::new("LoadedFieldCull".into());
    authored.info_mut().is_ground_aligned = false;
    authored.info_mut().priority = ParticlePriorityType::DustTrail;
    let authored = Arc::new(authored);
    receiving.register_template(authored.clone());
    let receipt = receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert!(receipt.belongs_to(&receiving));
    assert!(receipt.contains_system(id));

    // W3DParticleSys.cpp:124/205 counts in-box AREA_EFFECT ground particles
    // before the per-system batch cap: both near particles count, one draws.
    receiving.cull_particles_to_visible_box([0.0; 3], [10.0; 3], 1);
    assert_eq!(receiving.field_particle_count(), 2);
    let loaded = receiving.find_particle_system(id).unwrap();
    assert!(loaded.info().is_ground_aligned);
    assert_eq!(loaded.priority(), ParticlePriorityType::AreaEffect);
    assert_eq!(loaded.particle_count(), 3);
    assert_eq!(
        loaded
            .particles()
            .iter()
            .map(|particle| particle.is_culled)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    assert!(Arc::ptr_eq(loaded.template(), &authored));
    assert!(!authored.info().is_ground_aligned);
    assert_eq!(authored.info().priority, ParticlePriorityType::DustTrail);
    assert!(Arc::ptr_eq(
        &receiving.find_template("LoadedFieldCull").unwrap(),
        &authored
    ));
}

#[test]
fn particle_system_tint_all_colors_keeps_template_source_unchanged() {
    let mut template = ParticleSystemTemplate::new("TintSource".to_string());
    template.info_mut().color_keys[0] = RGBColorKeyframe {
        color: [1.0, 0.5, 0.25],
        frame: 0,
    };
    template.info_mut().color_keys[1] = RGBColorKeyframe {
        color: [1.0, 0.5, 0.25],
        frame: 10,
    };
    let template = Arc::new(template);
    let mut system = ParticleSystem::new(template.clone(), 1, false);
    let sibling = ParticleSystem::new(template.clone(), 2, false);

    system.tint_all_colors([0.25, 0.5, 1.0]);
    system.set_lifetime_range(99.0, 99.0);
    assert!(Arc::ptr_eq(system.template(), &template));
    assert!(Arc::ptr_eq(sibling.template(), &template));
    assert_eq!(system.generate_particle_info(0, 1).unwrap().lifetime, 99);
    assert_eq!(sibling.info().lifetime.min, template.info().lifetime.min);
    assert_eq!(sibling.info().lifetime.max, template.info().lifetime.max);
    // C++ tintAllColors starts at key 1; the initial color stays authored.
    assert_eq!(system.info().color_keys[0].color, [1.0, 0.5, 0.25]);
    assert_eq!(system.info().color_keys[1].color, [0.25, 0.25, 0.25]);
    assert_eq!(system.info().color_keys[1].frame, 10);
    for source in [&template, sibling.template()] {
        assert_eq!(source.info().color_keys[0].color, [1.0, 0.5, 0.25]);
        assert_eq!(source.info().color_keys[1].color, [1.0, 0.5, 0.25]);
    }
}

#[test]
fn particle_runtime_tint_retains_admitted_template_identity() {
    let template = Arc::new(ParticleSystemTemplate::new("AdmittedIdentity".into()));
    let mut system = ParticleSystem::new_with_random(template.clone(), 1, false, &mut |lo, _| lo);
    system.tint_all_colors([0.25, 0.5, 1.0]);
    assert!(
        Arc::ptr_eq(system.template(), &template),
        "runtime tint must retain admitted source identity"
    );
    system.set_lifetime_range(99.0, 99.0);
    assert!(Arc::ptr_eq(system.template(), &template));
    assert_eq!(system.generate_particle_info(0, 1).unwrap().lifetime, 99);
}

#[test]
fn particle_runtime_load_retains_receiving_template_identity() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let source = Arc::new(ParticleSystemTemplate::new("SourceInfo".into()));
    let mut saved = ParticleSystem::new_with_random(source, 1, false, &mut |lo, _| lo);
    saved.set_lifetime_range(99.0, 99.0);
    let mut bytes = Vec::new();
    let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
    save.open("identity").unwrap();
    saved.xfer(&mut save).unwrap();
    save.close().unwrap();
    drop(save);
    let receiving = Arc::new(ParticleSystemTemplate::new("ReceivingInfo".into()));
    let mut loaded = ParticleSystem::new_with_random(receiving.clone(), 0, false, &mut |lo, _| lo);
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("identity").unwrap();
    loaded.xfer(&mut load).unwrap();
    load.close().unwrap();
    assert!(
        Arc::ptr_eq(loaded.template(), &receiving),
        "loading runtime info must retain admitted receiving identity"
    );
    assert_eq!(loaded.generate_particle_info(0, 1).unwrap().lifetime, 99);
}

#[test]
fn particle_owned_wind_native_save_preserves_future_motion() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut template = ParticleSystemTemplate::new("WindContinuation".into());
    let info = template.info_mut();
    info.wind_motion = WindMotion::PingPong;
    info.wind_motion_start_angle_min = 0.25;
    info.wind_motion_start_angle_max = 0.4;
    info.wind_motion_end_angle_min = 0.75;
    info.wind_motion_end_angle_max = 1.0;
    info.wind_angle_change = 0.02;
    info.wind_angle_change_min = 0.02;
    info.wind_angle_change_max = 0.02;
    info.wind_motion_moving_to_end_angle = true;
    info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);
    info.burst_count = GameClientRandomVariable::new(0.0, 0.0);
    info.system_lifetime = 0;
    let template = Arc::new(template);
    let mut saved = ParticleSystem::new_with_random(template.clone(), 1, false, &mut |lo, _| lo);
    for frame in 0..3 {
        saved.begin_frame_emit(0, frame);
        saved.finish_frame_integrate(frame);
    }
    let mut bytes = Vec::new();
    let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
    save.open("wind_continuation").unwrap();
    saved.xfer(&mut save).unwrap();
    save.close().unwrap();
    drop(save);

    // A fresh constructor chooses different runtime endpoints. Loading must
    // restore the saved endpoints as well as the angle/direction/increment.
    let mut loaded = ParticleSystem::new_with_random(template, 2, false, &mut |_, hi| hi);
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("wind_continuation").unwrap();
    loaded.xfer(&mut load).unwrap();
    load.close().unwrap();
    assert_eq!(saved.wind_angle(), loaded.wind_angle());
    for frame in 3..27 {
        saved.begin_frame_emit(0, frame);
        saved.finish_frame_integrate(frame);
        loaded.begin_frame_emit(0, frame);
        loaded.finish_frame_integrate(frame);
        assert_eq!(
            saved.wind_angle(),
            loaded.wind_angle(),
            "wind diverged at frame {frame}"
        );
    }
}
