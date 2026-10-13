//! Real particle storage, supplied terrain and attachment; no runtime publication.
use super::*;
use crate::system::SubsystemInterface;

fn manager() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    manager.register_template(Arc::new(ParticleSystemTemplate::new("OwnedImpact".into())));
    manager
}

#[test]
fn same_id_particle_worlds_use_their_own_terrain_and_storage() {
    let mut first = manager();
    let mut peer = manager();
    let nugget = ParticleSystemFXNugget {
        create_at_ground_height: true,
        offset: Vec3::new(2.0, 3.0, 4.0),
        delay: GameClientRandomVariable::new(100.0, 100.0),
        ..ParticleSystemFXNugget::new("OwnedImpact".into())
    };
    let first_ids = nugget.do_fx_with_services(
        Vec3::new(10.0, 20.0, 30.0),
        None,
        None,
        0.0,
        &mut first,
        &mut |low, _| low,
        &mut |_| Some(71.0),
    );
    let peer_ids = nugget.do_fx_with_services(
        Vec3::new(100.0, 200.0, 300.0),
        None,
        None,
        0.0,
        &mut peer,
        &mut |low, _| low,
        &mut |_| Some(83.0),
    );
    assert_eq!(first_ids.len(), 1);
    assert_eq!(first_ids, peer_ids, "identity is allocated by each manager");
    let first_system = first.find_particle_system(first_ids[0]).unwrap();
    assert_eq!(first_system.position(), Vec3::new(12.0, 23.0, 71.0));
    assert_eq!(first_system.initial_delay_left(), 3);
    assert_eq!(
        peer.find_particle_system(peer_ids[0]).unwrap().position(),
        Vec3::new(102.0, 203.0, 83.0)
    );
    peer.reset().unwrap();
    assert_eq!(first.active_system_count(), 1);
    assert_eq!(
        first.find_particle_system(first_ids[0]).unwrap().position(),
        Vec3::new(12.0, 23.0, 71.0)
    );
}

#[test]
fn supplied_particle_object_attachment_retains_actual_transform_and_delay() {
    let mut manager = manager();
    let matrix = Mat3::from_rotation_z(0.5);
    let nugget = ParticleSystemFXNugget {
        attach_to_object: true,
        orient_to_object: true,
        delay: GameClientRandomVariable::new(34.0, 34.0),
        ..ParticleSystemFXNugget::new("OwnedImpact".into())
    };
    let ids = nugget.do_fx_with_services(
        Vec3::new(50.0, 60.0, 70.0),
        Some(&matrix),
        Some(900),
        0.0,
        &mut manager,
        &mut |low, _| low,
        &mut |_| None,
    );
    let system = manager.find_particle_system(ids[0]).unwrap();
    assert_eq!(system.attached_object_id(), 900);
    assert_eq!(system.local_transform(), matrix);
    assert_eq!(system.initial_delay_left(), 2);
    assert!(!system.is_stopped());
}
