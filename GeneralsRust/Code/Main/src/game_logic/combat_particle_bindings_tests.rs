use super::*;
use game_client::effects::particle_system::{Particle, ParticleInfo};
use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
use game_client::system::SubsystemInterface;
use std::sync::Arc;

fn entry(id: u32, template: &str) -> CombatParticleSystemEntry {
    CombatParticleSystemEntry {
        id,
        kind: CombatParticleKind::DeathExplosion,
        template_name: template.into(),
        position: Vec3::new(2.0, 3.0, 4.0),
        source_object: None,
        target_object: None,
        spawned_frame: 0,
        active: true,
        client_system_id: None,
        fx_list_name: String::new(),
        ocl_list_name: String::new(),
        attach_offset: Vec3::ZERO,
        attachment: None,
    }
}
fn make_registry(entries: Vec<CombatParticleSystemEntry>) -> CombatParticleRegistry {
    let mut registry = CombatParticleRegistry::new();
    registry.restore_from_snapshot(1, entries).unwrap();
    registry
}
fn manager(name: &str) -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    let mut template = ParticleSystemTemplate::new(name.into());
    template.info_mut().system_lifetime = 10;
    manager.register_template(Arc::new(template));
    manager
}

#[test]
fn receiving_particle_backfill_repeated_frames_preserve_exact_distinct_ids() {
    let mut particles = manager("ReceivingBackfill");
    let mut registry = make_registry(vec![
        entry(1, "ReceivingBackfill"),
        entry(2, "ReceivingBackfill"),
    ]);
    assert_eq!(registry.backfill_client_systems(&mut particles), 2);
    let ids = [
        registry.get(1).unwrap().client_system_id.unwrap(),
        registry.get(2).unwrap().client_system_id.unwrap(),
    ];
    assert_ne!(
        ids[0], ids[1],
        "same name and position are two distinct canonical systems"
    );
    for _ in 0..3 {
        let old_frame_rows: Vec<_> = registry
            .active_systems()
            .map(crate::presentation_frame::PresentationParticleSystem::from_combat_entry)
            .collect();
        assert_eq!(old_frame_rows.len(), 2);
        assert_eq!(registry.backfill_client_systems(&mut particles), 0);
        assert_eq!(particles.active_system_count(), 2);
        assert_eq!(registry.get(1).unwrap().client_system_id, Some(ids[0]));
        assert_eq!(registry.get(2).unwrap().client_system_id, Some(ids[1]));
    }
}

#[test]
fn receiving_particle_actual_frames_are_observations_after_canonical_admission() {
    let mut world = crate::game_logic::GameLogic::new();
    world
        .combat_particles_mut()
        .restore_from_snapshot(2, [entry(1, "FrameOwnedParticle")])
        .unwrap();
    let mut particles = manager("FrameOwnedParticle");
    assert_eq!(
        world
            .combat_particles_mut()
            .backfill_client_systems(&mut particles),
        1
    );
    let client_id = world.combat_particles().get(1).unwrap().client_system_id;
    let first = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    let second = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    assert_eq!(first.particle_systems[0].client_system_id, client_id);
    assert_eq!(second.particle_systems[0].client_system_id, client_id);
    assert_eq!(
        world
            .combat_particles_mut()
            .backfill_client_systems(&mut particles),
        0
    );
    assert_eq!(particles.active_system_count(), 1);
    world
        .combat_particles_mut()
        .clear_with_client_manager(&mut particles);
    world
        .combat_particles_mut()
        .restore_from_snapshot(2, [entry(1, "FrameOwnedParticle")])
        .unwrap();
    assert_eq!(
        world
            .combat_particles_mut()
            .backfill_client_systems(&mut particles),
        1
    );
    let new_id = world.combat_particles().get(1).unwrap().client_system_id;
    assert_ne!(new_id, client_id);
    // Reading the earlier same-host-ID snapshot cannot admit its old binding.
    assert_eq!(first.particle_systems[0].client_system_id, client_id);
    assert_eq!(
        world.combat_particles().get(1).unwrap().client_system_id,
        new_id
    );
}

#[test]
fn receiving_particle_backfill_failure_retries_once_and_keeps_attached_descriptor() {
    let mut particles = ParticleSystemManager::new();
    let mut attached = entry(1, "DelayedAttached");
    attached.kind = CombatParticleKind::ParticleSysBone;
    attached.attachment = Some(CombatParticleAttachment {
        owner: ObjectId(77),
        local_offset: Vec3::new(1.0, 2.0, 3.0),
        lifetime: Some(3),
        local_z_rotation: 0.4,
    });
    let mut registry = make_registry(vec![attached]);
    assert_eq!(registry.backfill_client_systems(&mut particles), 0);
    assert!(registry.get(1).unwrap().client_system_id.is_none());
    let mut template = ParticleSystemTemplate::new("DelayedAttached".into());
    template.info_mut().system_lifetime = 10;
    particles.register_template(Arc::new(template));
    assert_eq!(registry.backfill_client_systems(&mut particles), 1);
    let id = registry.get(1).unwrap().client_system_id.unwrap();
    let actual = particles.find_particle_system(id).unwrap();
    assert_eq!(actual.attached_object(), Some(77));
    assert_eq!(actual.position(), Vec3::new(1.0, 3.0, 2.0));
    let expected = glam::Mat3::from_rotation_z(0.4);
    assert!(actual.local_transform().abs_diff_eq(expected, 0.00001));
    assert_eq!(registry.backfill_client_systems(&mut particles), 0);
    assert_eq!(particles.active_system_count(), 1);
    assert!(!particles.find_particle_system(id).unwrap().should_remove());
    for frame in 1..=3 {
        particles.integrate_particle_system(id, frame).unwrap();
    }
    assert!(
        particles.find_particle_system(id).unwrap().should_remove(),
        "receiving actual system retains the explicit three-frame override"
    );
}

#[test]
fn receiving_particle_bindings_reject_peer_reset_clone_and_historical_ownership() {
    let mut first = manager("OwnedNamespace");
    let mut peer = manager("OwnedNamespace");
    let mut registry = make_registry(vec![entry(1, "OwnedNamespace")]);
    assert_eq!(registry.backfill_client_systems(&mut first), 1);
    let first_id = registry.get(1).unwrap().client_system_id.unwrap();
    let peer_id = peer
        .create_preset_system_at("OwnedNamespace", Vec3::ZERO)
        .unwrap();
    assert_eq!(first_id, peer_id);
    let mut query = registry.clone();
    query.clear_with_client_manager(&mut first);
    assert!(!first.find_particle_system(first_id).unwrap().is_destroyed());
    let saved = registry.systems_snapshot();
    let mut restored = make_registry(saved.clone());
    restored.deactivate_with_client_manager(1, &mut peer);
    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());
    let mut rebound = make_registry(saved);
    assert_eq!(rebound.backfill_client_systems(&mut peer), 1);
    let rebound_id = rebound.get(1).unwrap().client_system_id.unwrap();
    assert_ne!(
        rebound_id, peer_id,
        "historical numeric ID cannot claim an unrelated receiving system"
    );
    assert_eq!(rebound.backfill_client_systems(&mut peer), 0);
    rebound.clear_with_client_manager(&mut peer);
    assert!(
        peer.find_particle_system(rebound_id)
            .unwrap()
            .is_destroyed()
    );
    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());
    registry.deactivate_with_client_manager(1, &mut peer);
    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());
    assert!(!first.find_particle_system(first_id).unwrap().is_destroyed());
    assert!(registry.get(1).unwrap().active);
    assert_eq!(registry.get(1).unwrap().client_system_id, Some(first_id));
    registry.systems.insert(2, entry(2, "OwnedNamespace"));
    assert_eq!(
        registry.backfill_client_systems(&mut peer),
        0,
        "live peer mismatch preserves the original receipt"
    );
    assert!(
        registry.get(2).unwrap().client_system_id.is_none(),
        "a wrong peer cannot split unbound rows into another live manager"
    );
    assert_eq!(
        peer.active_system_count(),
        2,
        "only original peer and already-retired historical rebound exist"
    );
    registry.clear_with_client_manager(&mut peer);
    assert!(
        registry.get(1).unwrap().active,
        "clear preflight rejects the wrong live owner without partial mutation"
    );
    registry.clear_with_client_manager(&mut first);
    assert_eq!(registry.system_count(), 0);
    assert!(
        first.find_particle_system(first_id).unwrap().is_destroyed(),
        "correct owner still has the retained receipt and retires it"
    );
    assert!(!peer.find_particle_system(peer_id).unwrap().is_destroyed());

    first.reset().unwrap();
    let mut live = make_registry(vec![entry(1, "OwnedNamespace")]);
    live.backfill_client_systems(&mut first);
    let old_owner = first.runtime_owner();
    first.reset().unwrap();
    assert_ne!(old_owner, first.runtime_owner());
    assert!(
        !old_owner.is_live(),
        "receipts cannot keep a reset namespace alive"
    );
    let reused = first
        .create_preset_system_at("OwnedNamespace", Vec3::ZERO)
        .unwrap();
    live.deactivate_with_client_manager(1, &mut first);
    assert!(!first.find_particle_system(reused).unwrap().is_destroyed());
    assert_eq!(
        live.backfill_client_systems(&mut first),
        0,
        "deactivated entries never resurrect"
    );
}

#[test]
fn receiving_particle_manager_reset_rebinds_once_without_claiming_reused_id() {
    let mut particles = manager("ResetActual");
    let mut registry = make_registry(vec![entry(1, "ResetActual")]);
    registry.backfill_client_systems(&mut particles);
    let old_id = registry.get(1).unwrap().client_system_id.unwrap();
    particles.reset().unwrap();
    let occupant = particles
        .create_preset_system_at("ResetActual", Vec3::ZERO)
        .unwrap();
    assert_eq!(occupant, old_id);
    assert_eq!(registry.backfill_client_systems(&mut particles), 1);
    let admitted = registry.get(1).unwrap().client_system_id.unwrap();
    assert_ne!(admitted, occupant);
    assert_eq!(registry.backfill_client_systems(&mut particles), 0);
    registry.clear_with_client_manager(&mut particles);
    assert!(
        !particles
            .find_particle_system(occupant)
            .unwrap()
            .is_destroyed()
    );
    assert!(
        particles
            .find_particle_system(admitted)
            .unwrap()
            .is_destroyed()
    );
}

#[test]
fn receiving_particle_retirement_keeps_existing_particles_until_normal_fade() {
    let mut particles = manager("RetiringActual");
    let mut registry = make_registry(vec![entry(1, "RetiringActual")]);
    registry.backfill_client_systems(&mut particles);
    let id = registry.get(1).unwrap().client_system_id.unwrap();
    let mut info = ParticleInfo::default();
    info.lifetime = 3;
    particles
        .admit_particle(id, Particle::new(&info, 0, 0))
        .unwrap();
    registry.deactivate_with_client_manager(1, &mut particles);
    let actual = particles.find_particle_system(id).unwrap();
    assert!(actual.is_destroyed());
    assert_eq!(actual.particle_count(), 1);
    for frame in 1..=4 {
        particles.update(0, frame);
    }
    assert!(particles.find_particle_system(id).is_none());
    assert!(registry.get(1).unwrap().client_system_id.is_none());
}

#[test]
fn receiving_particle_fx_notification_does_not_fabricate_a_system() {
    let mut particles = manager("OnlyFxListNotification");
    let mut registry = CombatParticleRegistry::new();
    let id = registry.spawn_fx_list_marker(
        CombatParticleKind::WeaponMuzzleFlash,
        "OnlyFxListNotification",
        Vec3::ZERO,
        0,
        None,
        None,
    );
    assert_eq!(registry.backfill_client_systems(&mut particles), 0);
    assert!(registry.get(id).unwrap().client_system_id.is_none());
    assert_eq!(particles.active_system_count(), 0);
}

#[test]
fn receiving_particle_retirement_cascades_to_the_actual_slave() {
    let mut particles = manager("ReceivingSlave");
    let mut master_template = ParticleSystemTemplate::new("ReceivingMaster".into());
    master_template.info_mut().slave_system_name = "ReceivingSlave".into();
    particles.register_template(Arc::new(master_template));
    let mut registry = make_registry(vec![entry(1, "ReceivingMaster")]);
    assert_eq!(registry.backfill_client_systems(&mut particles), 1);
    let master_id = registry.get(1).unwrap().client_system_id.unwrap();
    // Exercise retirement of a real linked pair. Automatic authored slave
    // construction is an independent factory gap, not assumed by this test.
    let slave_id = particles
        .create_preset_system_at("ReceivingSlave", Vec3::ZERO)
        .unwrap();
    particles
        .find_particle_system_mut(master_id)
        .unwrap()
        .set_slave(Some(slave_id));
    particles
        .find_particle_system_mut(slave_id)
        .unwrap()
        .set_master(Some(master_id));
    assert_ne!(master_id, slave_id);
    registry.deactivate_with_client_manager(1, &mut particles);
    assert!(
        particles
            .find_particle_system(master_id)
            .unwrap()
            .is_destroyed()
    );
    assert!(
        particles
            .find_particle_system(slave_id)
            .unwrap()
            .is_destroyed()
    );
}

#[test]
fn receiving_particle_wrong_owner_restore_rejects_before_mutating_registry() {
    let mut particles = manager("RestoreOwner");
    let mut registry = make_registry(vec![entry(1, "RestoreOwner")]);
    registry.backfill_client_systems(&mut particles);
    let actual_id = registry.get(1).unwrap().client_system_id.unwrap();
    let before = registry.systems_snapshot();
    let next = registry.next_id();
    assert!(
        registry
            .restore_from_snapshot(100, [entry(99, "Replacement")])
            .is_err()
    );
    assert_eq!(registry.systems_snapshot(), before);
    assert_eq!(registry.next_id(), next);
    assert!(registry.client_bindings.contains_key(&1));
    assert!(
        !particles
            .find_particle_system(actual_id)
            .unwrap()
            .is_destroyed()
    );
    registry.clear_with_client_manager(&mut particles);
    assert!(
        particles
            .find_particle_system(actual_id)
            .unwrap()
            .is_destroyed()
    );
    registry
        .restore_from_snapshot(100, [entry(99, "Replacement")])
        .unwrap();
    assert!(registry.get(1).is_none());
    assert!(registry.get(99).is_some());
    assert_eq!(registry.next_id(), 100);
}

#[test]
fn receiving_particle_wrong_owner_world_reset_rejects_before_world_mutation() {
    let mut particles = manager("ResetGuardOwner");
    let mut world = crate::game_logic::GameLogic::new();
    world.add_object(crate::game_logic::Object::new(
        crate::game_logic::ThingTemplate::new("ResetGuardObject"),
        ObjectId(77),
        Team::USA,
    ));
    world.frame = 123;
    world
        .combat_particles_mut()
        .restore_from_snapshot(2, [entry(1, "ResetGuardOwner")])
        .unwrap();
    world
        .combat_particles_mut()
        .backfill_client_systems(&mut particles);
    let before = world.combat_particles().systems_snapshot();
    let actual_id = before[0].client_system_id.unwrap();
    let empty = crate::game_logic::GameLogic::new();
    let builder = crate::save_load::SnapshotBuilder::new();
    let replacement = builder.create_world_snapshot(&empty).unwrap();
    assert!(
        builder
            .restore_from_snapshot(&replacement, &mut world)
            .is_err()
    );
    assert_eq!(world.frame, 123);
    assert!(world.host_object(ObjectId(77)).is_some());
    assert_eq!(world.combat_particles().systems_snapshot(), before);
    let reset = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.reset()));
    assert!(
        reset.is_err(),
        "void compatibility reset explicitly rejects a different live owner"
    );
    assert_eq!(world.frame, 123);
    assert!(world.host_object(ObjectId(77)).is_some());
    assert_eq!(world.combat_particles().systems_snapshot(), before);
    assert!(
        !particles
            .find_particle_system(actual_id)
            .unwrap()
            .is_destroyed()
    );
    world
        .combat_particles_mut()
        .clear_with_client_manager(&mut particles);
    assert!(
        particles
            .find_particle_system(actual_id)
            .unwrap()
            .is_destroyed()
    );
    world.reset();
    assert_eq!(world.frame, 0);
    assert!(world.host_object(ObjectId(77)).is_none());
}

#[test]
fn receiving_particle_loaded_receipt_rejects_peer_without_mutating_host_bindings() {
    let mut first = manager("AdoptLoaded");
    let mut peer = manager("AdoptLoaded");
    let id = first
        .create_preset_system_at("AdoptLoaded", Vec3::ZERO)
        .unwrap();
    peer.create_preset_system_at("AdoptLoaded", Vec3::ZERO)
        .unwrap();
    let bytes = first.capture_xfer_bytes().unwrap();
    let receipt = first.restore_xfer_bytes(&bytes).unwrap().unwrap();
    let mut saved = entry(1, "AdoptLoaded");
    saved.client_system_id = Some(id);
    let mut registry = make_registry(vec![saved]);
    let before = registry.systems_snapshot();
    assert!(
        registry
            .adopt_loaded_client_systems(&peer, &receipt)
            .is_err()
    );
    assert_eq!(registry.systems_snapshot(), before);
    assert!(registry.client_bindings.is_empty());
    assert_eq!(
        registry
            .adopt_loaded_client_systems(&first, &receipt)
            .unwrap(),
        1
    );
    assert_eq!(registry.backfill_client_systems(&mut first), 0);
    registry.clear_with_client_manager(&mut first);
    assert!(first.find_particle_system(id).unwrap().is_destroyed());
    assert!(!peer.find_particle_system(id).unwrap().is_destroyed());
}

#[test]
fn paired_particle_sav_companion_adopts_actual_loaded_ids_without_replaying_omissions() {
    use crate::save_load::SaveFileManager;
    use game_client::effects::particle_manager::{GameClientRandomVariable, ParticlePriorityType};
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let mut world = crate::game_logic::GameLogic::new();
    world
        .combat_particles_mut()
        .restore_from_snapshot(
            4,
            [
                entry(1, "PairedArchiveParticle"),
                entry(2, "PairedArchiveParticle"),
                entry(3, "PairedArchiveParticle"),
            ],
        )
        .unwrap();
    let saved_ids = with_client_particle_manager(|manager| {
        // This real .sav test intentionally drives the existing compatibility
        // manager/companion stream; broader pending-slot isolation is separate.
        let mut template = ParticleSystemTemplate::new("PairedArchiveParticle".into());
        let info = template.info_mut();
        info.system_lifetime = 1000;
        info.priority = ParticlePriorityType::AlwaysRender;
        info.start_size = GameClientRandomVariable::new(3.0, 3.0);
        info.start_size_rate = GameClientRandomVariable::new(2.0, 2.0);
        info.size_rate = GameClientRandomVariable::new(0.0, 0.0);
        info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.lifetime = GameClientRandomVariable::new(100.0, 100.0);
        for key in &mut info.alpha_keys[..2] {
            key.min_value = 1.0;
            key.max_value = 1.0;
        }
        info.alpha_keys[1].frame = 100;
        manager.register_template(Arc::new(template));
        assert_eq!(
            world
                .combat_particles_mut()
                .backfill_client_systems(manager),
            3
        );
        let ids: Vec<_> = (1..=3)
            .map(|id| {
                world
                    .combat_particles()
                    .get(id)
                    .unwrap()
                    .client_system_id
                    .unwrap()
            })
            .collect();
        manager.update(0, 1);
        let particle = &manager.find_particle_system(ids[0]).unwrap().particles()[0];
        assert_eq!((particle.size, particle.personality), (3.0, 0));
        assert_eq!(particle.create_timestamp, 1);
        manager.destroy_particle_system(ids[1]);
        manager
            .find_particle_system_mut(ids[2])
            .unwrap()
            .set_saveable(false);
        ids
    })
    .unwrap();
    saves.quick_save(&world).unwrap();
    let mut restored = crate::game_logic::GameLogic::new();
    saves.load_game("quicksave", &mut restored).unwrap();
    with_client_particle_manager(|manager| {
        assert!(manager.find_particle_system(saved_ids[0]).is_some());
        assert!(manager.find_particle_system(saved_ids[1]).is_none());
        assert!(manager.find_particle_system(saved_ids[2]).is_none());
        assert_eq!(
            restored.combat_particles().get(1).unwrap().client_system_id,
            Some(saved_ids[0])
        );
        assert!(!restored.combat_particles().get(2).unwrap().active);
        assert!(!restored.combat_particles().get(3).unwrap().active);
        let particle = &manager
            .find_particle_system(saved_ids[0])
            .unwrap()
            .particles()[0];
        assert_eq!((particle.size, particle.personality), (3.0, 0));
        assert_eq!(particle.create_timestamp, 1);
        // C++ ParticleSys.cpp:2488 generates once on load, advancing the
        // size bonus2 ->4; forced restoration also advances store1 ->2.
        // These observations pass through the real .sav companion adoption.
        manager.update(0, 2);
        let system = manager.find_particle_system(saved_ids[0]).unwrap();
        assert_eq!(system.particle_count(), 2);
        let next = &system.particles()[1];
        assert_eq!((next.size, next.personality), (7.0, 2));
        assert_eq!(next.create_timestamp, 2);
        let count = manager.active_system_count();
        for _ in 0..3 {
            assert_eq!(
                restored
                    .combat_particles_mut()
                    .backfill_client_systems(manager),
                0
            );
            assert_eq!(manager.active_system_count(), count);
        }
        restored
            .combat_particles_mut()
            .deactivate_with_client_manager(1, manager);
        assert!(
            manager
                .find_particle_system(saved_ids[0])
                .unwrap()
                .is_destroyed()
        );
    })
    .unwrap();
}

#[test]
fn receiving_particle_native_companion_adopts_exact_loaded_namespace_and_omissions() {
    let mut source = manager("OwnedNativeCompanion");
    let mut registry = make_registry(vec![
        entry(1, "OwnedNativeCompanion"),
        entry(2, "OwnedNativeCompanion"),
        entry(3, "OwnedNativeCompanion"),
    ]);
    registry.backfill_client_systems(&mut source);
    let ids: Vec<_> = (1..=3)
        .map(|id| registry.get(id).unwrap().client_system_id.unwrap())
        .collect();
    let mut particle = ParticleInfo::default();
    particle.lifetime = 20;
    source
        .admit_particle(ids[0], Particle::new(&particle, 0, 0))
        .unwrap();
    source.destroy_particle_system(ids[1]);
    source
        .find_particle_system_mut(ids[2])
        .unwrap()
        .set_saveable(false);
    let actual_bytes = source.capture_xfer_bytes().unwrap();
    let mut receiving = manager("OwnedNativeCompanion");
    receiving
        .create_preset_system_at("OwnedNativeCompanion", Vec3::ZERO)
        .unwrap();
    let previous_owner = receiving.runtime_owner();
    let owner = receiving
        .restore_xfer_bytes(&actual_bytes)
        .unwrap()
        .unwrap();
    assert!(!previous_owner.is_live());
    assert!(owner.belongs_to(&receiving));
    assert!(!owner.belongs_to(&source));
    let mut restored = make_registry(registry.systems_snapshot());
    let before = restored.systems_snapshot();
    assert!(
        restored
            .adopt_loaded_client_systems(&source, &owner)
            .is_err()
    );
    assert_eq!(restored.systems_snapshot(), before);
    assert_eq!(
        restored
            .adopt_loaded_client_systems(&receiving, &owner)
            .unwrap(),
        1
    );
    assert_eq!(restored.get(1).unwrap().client_system_id, Some(ids[0]));
    assert!(!restored.get(2).unwrap().active);
    assert!(!restored.get(3).unwrap().active);
    assert_eq!(receiving.active_system_count(), 1);
    for _ in 0..3 {
        assert_eq!(restored.backfill_client_systems(&mut receiving), 0);
        assert_eq!(receiving.active_system_count(), 1);
    }
    assert_eq!(
        receiving
            .find_particle_system(ids[0])
            .unwrap()
            .particle_count(),
        1
    );
    restored.deactivate_with_client_manager(1, &mut receiving);
    assert!(
        receiving
            .find_particle_system(ids[0])
            .unwrap()
            .is_destroyed()
    );
    assert_eq!(
        receiving
            .find_particle_system(ids[0])
            .unwrap()
            .particle_count(),
        1,
        "adopted system retirement retains saved particles for fade"
    );
    assert!(!source.find_particle_system(ids[0]).unwrap().is_destroyed());
}

#[test]
fn receiving_particle_companion_receipt_rejects_later_created_omitted_id() {
    let mut source = manager("ReceiptMembership");
    let mut registry = make_registry(vec![
        entry(1, "ReceiptMembership"),
        entry(2, "ReceiptMembership"),
    ]);
    registry.backfill_client_systems(&mut source);
    let first_id = registry.get(1).unwrap().client_system_id.unwrap();
    let omitted_id = registry.get(2).unwrap().client_system_id.unwrap();
    source
        .find_particle_system_mut(omitted_id)
        .unwrap()
        .set_saveable(false);
    let bytes = source.capture_xfer_bytes().unwrap();
    let mut receiving = manager("ReceiptMembership");
    let receipt = receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert!(receipt.contains_system(first_id));
    assert!(!receipt.contains_system(omitted_id));
    let template = receiving.find_template("ReceiptMembership").unwrap();
    receiving
        .create_particle_system_with_id(&template, omitted_id, false)
        .unwrap();
    let mut restored = make_registry(registry.systems_snapshot());
    assert_eq!(
        restored
            .adopt_loaded_client_systems(&receiving, &receipt)
            .unwrap(),
        1
    );
    assert!(!restored.get(2).unwrap().active);
    assert_eq!(restored.backfill_client_systems(&mut receiving), 0);
    assert_eq!(receiving.active_system_count(), 2);
    restored.clear_with_client_manager(&mut receiving);
    assert!(
        receiving
            .find_particle_system(first_id)
            .unwrap()
            .is_destroyed()
    );
    assert!(
        !receiving
            .find_particle_system(omitted_id)
            .unwrap()
            .is_destroyed(),
        "a later same-numeric-ID occupant is not part of the loaded receipt"
    );
}

#[test]
fn receiving_particle_companion_rejects_same_id_wrong_template_before_mutation() {
    let mut source = manager("LoadedEffect");
    let mut source_registry =
        make_registry(vec![entry(1, "LoadedEffect"), entry(2, "LoadedEffect")]);
    assert_eq!(source_registry.backfill_client_systems(&mut source), 2);
    let id = source_registry.get(1).unwrap().client_system_id.unwrap();
    let bytes = source.capture_xfer_bytes().unwrap();
    let mut receiving = manager("LoadedEffect");
    receiving.register_template(Arc::new(ParticleSystemTemplate::new("OtherEffect".into())));
    let receipt = receiving.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert!(receipt.contains_system(id));

    let mut wrong_entry = entry(1, "OtherEffect");
    wrong_entry.client_system_id = Some(id);
    let mut wrong = make_registry(vec![wrong_entry]);
    let before = wrong.systems_snapshot();
    let next = wrong.next_id();
    assert!(
        wrong
            .adopt_loaded_client_systems(&receiving, &receipt)
            .is_err()
    );
    assert_eq!(wrong.systems_snapshot(), before);
    assert_eq!(wrong.next_id(), next);
    assert!(wrong.client_bindings.is_empty());
    assert_eq!(receiving.active_system_count(), 2);
    assert!(!receiving.find_particle_system(id).unwrap().is_destroyed());
    wrong.clear_with_client_manager(&mut receiving);
    assert!(
        !receiving.find_particle_system(id).unwrap().is_destroyed(),
        "rejected adoption grants no retirement authority"
    );

    let mut valid = make_registry(source_registry.systems_snapshot());
    assert_eq!(
        valid
            .adopt_loaded_client_systems(&receiving, &receipt)
            .unwrap(),
        2
    );
    let second_id = valid.get(2).unwrap().client_system_id.unwrap();
    valid.systems.get_mut(&2).unwrap().template_name = "OtherEffect".into();
    let before = valid.systems_snapshot();
    assert!(
        valid
            .adopt_loaded_client_systems(&receiving, &receipt)
            .is_err()
    );
    assert_eq!(valid.systems_snapshot(), before);
    assert_eq!(valid.client_bindings.len(), 2);
    assert_eq!(valid.client_bindings.get(&1).unwrap().id, id);
    assert_eq!(valid.client_bindings.get(&2).unwrap().id, second_id);
    valid.systems.get_mut(&2).unwrap().template_name = "MissingEffect".into();
    let before = valid.systems_snapshot();
    assert!(
        valid
            .adopt_loaded_client_systems(&receiving, &receipt)
            .is_err()
    );
    assert_eq!(valid.systems_snapshot(), before);
    assert_eq!(valid.client_bindings.len(), 2);
    valid.systems.get_mut(&2).unwrap().template_name = "LoadedEffect".into();
    assert_eq!(valid.backfill_client_systems(&mut receiving), 0);
    assert_eq!(receiving.active_system_count(), 2);
    valid.deactivate_with_client_manager(1, &mut receiving);
    assert!(receiving.find_particle_system(id).unwrap().is_destroyed());
    assert!(!source.find_particle_system(id).unwrap().is_destroyed());
}
