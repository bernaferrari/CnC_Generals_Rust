//! Source-derived native fields, not an original-engine captured save.
use super::*;

fn definition(name: &str, authored_scale: f32) -> Arc<ParticleSystemTemplate> {
    let mut template = ParticleSystemTemplate::new(name.into());
    let info = template.info_mut();
    info.particle_type_name.clear();
    info.priority = ParticlePriorityType::AlwaysRender;
    for key in &mut info.alpha_keys {
        key.min_value = 1.0;
        key.max_value = 1.0;
    }
    info.lifetime = GameClientRandomVariable::new(30.0, 30.0);
    info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
    info.burst_delay = GameClientRandomVariable::new(30.0, 30.0);
    info.color_scale = GameClientRandomVariable {
        min: authored_scale,
        max: authored_scale,
        distribution_type: 5,
    };
    Arc::new(template)
}

// Manager version/next-ID/count =9; name length/name; system version=1.
// Info prefix: version/bool/shader/type/empty-name=11; nine variables=108;
// systemLifetime=4; four variables=48; eight alpha/color keys=128 each.
// Derived independently from C++ ParticleSys.cpp768–843,3232–3323.
fn native_color_offset(name: &str) -> usize {
    9 + 1 + name.len() + 1 + 11 + 108 + 4 + 48 + 128 + 128
}

#[test]
fn authored_color_scale_runtime_normalizes_once_without_mutating_shared_definition() {
    let template = definition("NormalizedColor", 51.0);
    let mut a = ParticleSystem::new(template.clone(), 1, false);
    let b = ParticleSystem::new(template.clone(), 1, false);
    assert!(Arc::ptr_eq(a.template(), &template));
    assert!(Arc::ptr_eq(b.template(), &template));
    assert_eq!(
        (
            template.info().color_scale.min,
            template.info().color_scale.distribution_type
        ),
        (51.0, 5)
    );
    for system in [&a, &b] {
        assert_eq!(
            (
                system.info().color_scale.min,
                system.info().color_scale.max,
                system.info().color_scale.distribution_type
            ),
            (0.2, 0.2, 1)
        );
    }
    assert_eq!(a.generate_particle_info(0, 1).unwrap().color_scale, 0.2);
    assert_eq!(b.info().color_scale.min, 0.2);
    assert_eq!(template.info().color_scale.min, 51.0);
    let mut manager = ParticleSystemManager::new();
    manager.register_template(template.clone());
    manager.create_particle_system(&template, false).unwrap();
    let bytes = manager.capture_xfer_bytes().unwrap();
    let offset = native_color_offset("NormalizedColor");
    // Independently pinned little-endian C++ UNIFORM=1, bounds0.2/0.2.
    assert_eq!(
        &bytes[offset..offset + 12],
        &[1, 0, 0, 0, 205, 204, 76, 62, 205, 204, 76, 62]
    );
}

#[test]
fn source_native_uniform_color_scale_load_emits_runtime_units_and_resaves_them() {
    let template = definition("NativeColor", 51.0);
    let mut source = ParticleSystemManager::new();
    source.register_template(template.clone());
    let id = source.create_particle_system(&template, false).unwrap();
    let mut bytes = source.capture_xfer_bytes().unwrap();
    let offset = native_color_offset("NativeColor");
    // Original runtime native UNIFORM=1, bounds0.25/0.25. No writer-generated range.
    bytes[offset..offset + 12].copy_from_slice(&[1, 0, 0, 0, 0, 0, 128, 62, 0, 0, 128, 62]);
    let receiving_definition = definition("NativeColor", 204.0);
    let mut receiving = ParticleSystemManager::new();
    receiving.register_template(receiving_definition.clone());
    receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert!(Arc::ptr_eq(
        receiving.find_particle_system(id).unwrap().template(),
        &receiving_definition
    ));
    assert_eq!(
        receiving
            .find_particle_system(id)
            .unwrap()
            .info()
            .color_scale
            .min,
        0.25
    );
    let resaved = receiving.capture_xfer_bytes().unwrap();
    assert_eq!(
        &resaved[offset..offset + 12],
        &[1, 0, 0, 0, 0, 0, 128, 62, 0, 0, 128, 62]
    );
    receiving.update(0, 1);
    let system = receiving.find_particle_system(id).unwrap();
    assert_eq!(system.particle_count(), 1);
    let particle = &system.particles()[0];
    assert_eq!(particle.color_scale, 0.25);
    assert_eq!(particle.color, [0.25; 3]);
    assert_eq!(receiving_definition.info().color_scale.min, 204.0);
    let mut resumed = ParticleSystemManager::new();
    resumed.register_template(receiving_definition);
    let continuation = receiving.capture_xfer_bytes().unwrap();
    resumed.restore_xfer_bytes(&continuation).unwrap().unwrap();
    receiving.update(0, 2);
    resumed.update(0, 2);
    assert_eq!(
        receiving.find_particle_system(id).unwrap().particles()[0].color,
        [0.5; 3]
    );
    assert_eq!(
        receiving.find_particle_system(id).unwrap().particles()[0].color,
        resumed.find_particle_system(id).unwrap().particles()[0].color
    );
}
