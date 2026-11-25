//! Archive-owned particle companions remain attached to their decoded candidate.
use super::*;
use game_client::effects::particle_archive::ParticleSystemManagerArchive;
use game_client::effects::particle_system::{Particle, ParticleInfo};
use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
use game_client::system::SubsystemInterface;
use std::sync::Arc;

pub(super) fn local_particle_archive(
    name: &str,
    position: Vec3,
) -> ParticleSystemManagerArchive {
    let mut manager = local_manager(&[name]);
    let template = manager.find_template(name).unwrap();
    let id = manager.create_particle_system(&template, true).unwrap();
    let system = manager.find_particle_system_mut(id).unwrap();
    system.set_position(position);
    manager
        .admit_particle(
            id,
            Particle::new(
                &ParticleInfo {
                    lifetime: 21,
                    ..Default::default()
                },
                17,
                0,
            ),
        )
        .unwrap();
    manager.capture_archive()
}

fn local_manager(names: &[&str]) -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    for name in names {
        let mut template = ParticleSystemTemplate::new((*name).into());
        template.info_mut().system_lifetime = 1000;
        manager.register_template(Arc::new(template));
    }
    manager
}

fn archive_key(archive: &ParticleSystemManagerArchive) -> Vec<u8> {
    bincode_legacy::serialize(archive).expect("typed particle archive is serializable")
}

fn archive(companion: Option<ParticleSystemManagerArchive>) -> Vec<u8> {
    let mut snapshot = WorldSnapshot::default();
    snapshot.particle_system_archive = companion;
    encode_snapshot_archive(&snapshot, &fixture_save_info())
}

fn parsed(bytes: &[u8]) -> WorldSnapshot {
    decode_versioned_archive(bytes).snapshot
}

fn assert_loaded(manager: &mut ParticleSystemManager, snapshot: &WorldSnapshot, expected: Vec3) {
    let receipt = manager
        .restore_archive(snapshot.particle_system_archive.as_ref().unwrap())
        .unwrap();
    assert!(receipt.belongs_to(manager));
    assert!(receipt.contains_system(1));
    assert_eq!(manager.active_system_count(), 1);
    let system = manager.find_particle_system(1).unwrap();
    assert_eq!(system.position(), expected);
    assert_eq!(system.particle_count(), 1);
    assert_eq!(system.particles()[0].lifetime, 21);
}

#[test]
fn particle_candidate_archives_interleave_without_stealing_receiving_payloads() {
    let a_position = Vec3::new(12.0, 22.0, 32.0);
    let b_position = Vec3::new(15.0, 25.0, 35.0);
    let a_archive = local_particle_archive("ParticleCandidateA", a_position);
    let b_archive = local_particle_archive("ParticleCandidateB", b_position);
    let a_key = archive_key(&a_archive);
    let b_key = archive_key(&b_archive);
    let a = parsed(&archive(Some(a_archive)));
    let b = parsed(&archive(Some(b_archive)));
    assert_eq!(
        archive_key(a.particle_system_archive.as_ref().unwrap()),
        a_key
    );
    assert_eq!(
        archive_key(b.particle_system_archive.as_ref().unwrap()),
        b_key
    );

    let mut receiving_a = local_manager(&["ParticleCandidateA"]);
    let mut receiving_b = local_manager(&["ParticleCandidateB"]);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_loaded(&mut receiving_b, &b, b_position);
    receiving_b.reset().unwrap();
    drop(receiving_b);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_eq!(
        archive_key(b.particle_system_archive.as_ref().unwrap()),
        b_key
    );
}

#[test]
fn particle_candidate_resave_preserves_the_owned_typed_archive() {
    let payload = local_particle_archive("OwnedResave", Vec3::new(7.0, 8.0, 9.0));
    let payload_key = archive_key(&payload);
    let owned = parsed(&archive(Some(payload)));
    let decoded = parsed(&archive(None));
    assert!(decoded.particle_system_archive.is_none());
    let encoded = encode_snapshot_archive(&owned, &fixture_save_info());
    let reparsed = parsed(&encoded);
    assert_eq!(
        archive_key(reparsed.particle_system_archive.as_ref().unwrap()),
        payload_key,
        "resaving uses the candidate's exact owned particle archive"
    );
}

#[test]
fn corrupt_particle_candidate_does_not_contaminate_prior_candidate() {
    let payload = local_particle_archive("CandidateSurvives", Vec3::new(3.0, 4.0, 5.0));
    let payload_key = archive_key(&payload);
    let good = parsed(&archive(Some(payload)));
    let mut corrupt = decode_versioned_archive(&archive(Some(local_particle_archive(
        "FailedCandidate",
        Vec3::ZERO,
    ))));
    corrupt.snapshot.version += 1;
    assert!(decode_versioned_save_archive(&encode_versioned_archive(&corrupt)).is_err());
    assert_eq!(
        archive_key(good.particle_system_archive.as_ref().unwrap()),
        payload_key
    );
    let mut manager = local_manager(&["CandidateSurvives"]);
    assert_loaded(&mut manager, &good, Vec3::new(3.0, 4.0, 5.0));
}
