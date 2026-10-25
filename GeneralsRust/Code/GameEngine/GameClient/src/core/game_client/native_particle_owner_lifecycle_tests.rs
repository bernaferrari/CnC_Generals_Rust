//! Actual native Client particle admission, without a GPU or global World.
use super::*;
use crate::effects::particle_manager::{
    GameClientRandomVariable, NativeParticleObjectLifecycle, ParticleSystemManager,
    ParticleSystemTemplate,
};
#[test]
fn actual_native_client_phase_lends_receiving_object_loss_only_when_particles_are_admitted() {
    let mut client = GameClient::new().unwrap();
    client.set_frame(8);
    let mut manager = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new("NativeOwnerLoss486".to_string());
    definition.info_mut().burst_count = GameClientRandomVariable::new(0., 0.);
    let definition = std::sync::Arc::new(definition);
    let id = manager.create_particle_system(&definition, true).unwrap();
    manager
        .find_particle_system_mut(id)
        .unwrap()
        .attach_to_object(755);
    let mut losses = Vec::new();
    let actual_manager = manager.runtime_owner();
    let mut lost =
        |owner: &crate::effects::particle_manager::ParticleSystemOwner, system, object| {
            assert_eq!(owner, &actual_manager);
            losses.push((system, object));
        };
    let absent = |object| {
        assert_eq!(object, 755);
        false
    };
    let mut lifecycle = NativeParticleObjectLifecycle {
        object_pose: &|_| None,
        ground_height: &|_| None,
        object_is_live: &absent,
        attached_object_lost: &mut lost,
    };
    let frames = NativeParticleFrames {
        logic_frame: 8,
        client_frame: 8,
    };
    client
        .run_native_views_with_object_lifecycle(
            &mut manager,
            None,
            &[],
            frames,
            &[],
            &[],
            NativeDrawPhase {
                draw_drawables: false,
                update_particles: false,
            },
            Some(&mut lifecycle),
        )
        .unwrap();
    assert_eq!(
        manager.find_particle_system(id).unwrap().attached_object(),
        Some(755)
    );
    assert!(!manager.find_particle_system(id).unwrap().is_destroyed());
    client
        .run_native_views_with_object_lifecycle(
            &mut manager,
            None,
            &[],
            frames,
            &[],
            &[],
            NativeDrawPhase {
                draw_drawables: false,
                update_particles: true,
            },
            Some(&mut lifecycle),
        )
        .unwrap();
    assert!(manager.find_particle_system(id).is_none());
    drop(lifecycle);
    drop(lost);
    assert_eq!(losses, vec![(id, 755)]);
}
