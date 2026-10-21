//! CPP ParticleSys1877-1904 /2395: pending raw owner -> admitted update detach.
use super::*;
use crate::game_logic::{CombatParticleKind, Team, ThingTemplate};
use crate::save_load::SnapshotBuilder;
use game_client::effects::particle_manager::GameClientRandomVariable;
use game_client::effects::particle_system::{Particle, ParticleInfo};
use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
use std::sync::Arc;
const PARTICLE: &str = "CPATSteamVent486";
fn world_and_manager(delay: u32) -> (GameLogic, ParticleSystemManager, ObjectId, u32, u32) {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("CPATOwner486".into(), ThingTemplate::new("CPATOwner486"));
    let owner = world
        .create_object("CPATOwner486", Team::USA, Vec3::new(71., 0., 83.))
        .unwrap();
    let host = world
        .combat_particles_mut()
        .attach_named_to_object_local(
            owner,
            Vec3::new(71., 0., 83.),
            0.,
            Vec3::ZERO,
            0,
            PARTICLE,
            CombatParticleKind::ParticleSysBone,
            None,
        )
        .unwrap();
    let mut manager = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new(PARTICLE.to_string());
    definition.info_mut().burst_count = GameClientRandomVariable::new(0., 0.);
    definition.info_mut().initial_delay = GameClientRandomVariable::new(delay as f32, delay as f32);
    definition.info_mut().system_lifetime = 0;
    manager.register_template(Arc::new(definition));
    assert_eq!(
        world
            .combat_particles_mut()
            .backfill_client_systems(&mut manager),
        1
    );
    let client = world
        .combat_particles()
        .get(host)
        .unwrap()
        .client_system_id
        .unwrap();
    (world, manager, owner, host, client)
}
fn update(world: &mut GameLogic, manager: &mut ParticleSystemManager, frame: u32) {
    world.with_native_particle_object_lifecycle(|lifecycle, _pending_commands, _content| {
        manager.update_with_object_lifecycle(0, frame, frame, lifecycle)
    });
}
fn physical_remove(world: &mut GameLogic, id: ObjectId) {
    world.destroy_object(id);
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
    assert!(!world.has_pending_object_removals());
}
#[test]
fn raw_missing_owner_save_roundtrip_preserves_pending_descriptor_before_actual_update() {
    let (mut world, mut manager, owner, host, client) = world_and_manager(0);
    physical_remove(&mut world, owner);
    assert_eq!(
        manager
            .find_particle_system(client)
            .unwrap()
            .attached_object(),
        Some(owner.0)
    );
    assert!(!manager.find_particle_system(client).unwrap().is_destroyed());
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    assert_eq!(decode_and_validate(&snapshot).unwrap()[&host].owner, owner);
    let decoded: WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&snapshot).unwrap()).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = world.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(restored.host_object(owner).is_none());
    assert_eq!(
        restored
            .combat_particles()
            .get(host)
            .unwrap()
            .attachment
            .as_ref()
            .unwrap()
            .owner,
        owner
    );
    let mut restored_manager = ParticleSystemManager::new();
    let mut definition = ParticleSystemTemplate::new(PARTICLE.to_string());
    definition.info_mut().burst_count = GameClientRandomVariable::new(0., 0.);
    restored_manager.register_template(Arc::new(definition));
    assert_eq!(
        restored
            .combat_particles_mut()
            .backfill_client_systems(&mut restored_manager),
        1
    );
    update(&mut world, &mut manager, 1);
    update(&mut restored, &mut restored_manager, 1);
    for receiving in [&world, &restored] {
        let entry = receiving.combat_particles().get(host).unwrap();
        assert!(!entry.active);
        assert!(entry.attachment.is_none());
        let after = SnapshotBuilder::new()
            .create_world_snapshot(receiving)
            .unwrap();
        assert!(decode_and_validate(&after).unwrap().is_empty());
    }
    assert_eq!(manager.system_count(), 0);
    assert_eq!(restored_manager.system_count(), 0);
}
#[test]
fn actual_particle_phase_honors_delay_namespace_live_peer_and_destroy_drain() {
    let (mut world, mut manager, owner, host, client) = world_and_manager(2);
    let (mut peer, mut peer_manager, peer_owner, peer_host, peer_client) = world_and_manager(0);
    assert_eq!((owner, host, client), (peer_owner, peer_host, peer_client));
    let peer_namespace = peer_manager.runtime_owner();
    assert!(
        !world
            .combat_particles_mut()
            .admit_client_object_attachment_lost(&peer_namespace, client, owner.0)
    );
    let mut particle = ParticleInfo::default();
    particle.lifetime = 4;
    particle.alpha_keys[0].value = 1.;
    manager
        .admit_particle(client, Particle::new(&particle, 0, 0))
        .unwrap();
    physical_remove(&mut world, owner);
    // The peer's constructor published the same numeric ObjectID. The explicit
    // receiving lookup must not inherit that ambient live peer or stale own pose.
    for frame in [1, 2] {
        update(&mut world, &mut manager, frame);
        let system = manager.find_particle_system(client).unwrap();
        assert!(!system.is_destroyed());
        assert_eq!(system.attached_object(), Some(owner.0));
        assert!(world.combat_particles().get(host).unwrap().active);
    }
    update(&mut world, &mut manager, 3);
    let system = manager.find_particle_system(client).unwrap();
    assert!(system.is_destroyed());
    assert_eq!(system.attached_object(), None);
    assert_eq!(
        system.particle_count(),
        1,
        "destroy must drain particles, not delete them"
    );
    let entry = world.combat_particles().get(host).unwrap();
    assert!(!entry.active);
    assert!(entry.attachment.is_none());
    update(&mut peer, &mut peer_manager, 3);
    assert!(
        !peer_manager
            .find_particle_system(peer_client)
            .unwrap()
            .is_destroyed()
    );
    assert_eq!(
        peer_manager
            .find_particle_system(peer_client)
            .unwrap()
            .attached_object(),
        Some(peer_owner.0)
    );
    assert!(peer.combat_particles().get(peer_host).unwrap().active);
    assert!(
        peer.combat_particles()
            .get(peer_host)
            .unwrap()
            .attachment
            .is_some()
    );
    // Manager frame dedup happens BEFORE attachment lifecycle admission.
    let count = manager.particle_count();
    update(&mut world, &mut manager, 3);
    assert_eq!(manager.particle_count(), count);
    for frame in 4..12 {
        update(&mut world, &mut manager, frame);
    }
    assert_eq!(manager.system_count(), 0);
    assert_eq!(manager.particle_count(), 0);
    assert!(peer_manager.find_particle_system(peer_client).is_some());
}
