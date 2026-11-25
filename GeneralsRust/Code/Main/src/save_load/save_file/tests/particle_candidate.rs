//! Archive-owned particle companions remain attached to their decoded candidate.
use super::*;
use game_client::effects::particle_system::{Particle, ParticleInfo};
use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
use game_client::system::SubsystemInterface;
use std::sync::Arc;

pub(super) fn local_particle_bytes(name: &str, position: Vec3) -> Vec<u8> {
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
    manager.capture_xfer_bytes().unwrap()
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

fn archive(companion: Option<Vec<u8>>) -> Vec<u8> {
    let mut snapshot = WorldSnapshot::default();
    snapshot.particle_system_companion = companion;
    encode_snapshot_archive(&snapshot, &fixture_save_info())
}

fn parsed(bytes: &[u8]) -> WorldSnapshot {
    decode_versioned_archive(bytes).snapshot
}

fn assert_loaded(manager: &mut ParticleSystemManager, snapshot: &WorldSnapshot, expected: Vec3) {
    let receipt = manager
        .restore_xfer_bytes(snapshot.particle_system_companion.as_deref().unwrap())
        .unwrap()
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
    let a_bytes = local_particle_bytes("ParticleCandidateA", a_position);
    let b_bytes = local_particle_bytes("ParticleCandidateB", b_position);
    let a = parsed(&archive(Some(a_bytes.clone())));
    let b = parsed(&archive(Some(b_bytes.clone())));
    assert_eq!(
        a.particle_system_companion.as_deref(),
        Some(a_bytes.as_slice())
    );
    assert_eq!(
        b.particle_system_companion.as_deref(),
        Some(b_bytes.as_slice())
    );

    let mut receiving_a = local_manager(&["ParticleCandidateA"]);
    let mut receiving_b = local_manager(&["ParticleCandidateB"]);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_loaded(&mut receiving_b, &b, b_position);
    receiving_b.reset().unwrap();
    drop(receiving_b);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_eq!(
        b.particle_system_companion.as_deref(),
        Some(b_bytes.as_slice())
    );
}

#[test]
fn particle_candidate_resave_and_null_forms_are_canonical() {
    let payload = local_particle_bytes("OwnedResave", Vec3::new(7.0, 8.0, 9.0));
    let owned = parsed(&archive(Some(payload.clone())));
    for companion in [None, Some(Vec::new()), Some(vec![1])] {
        let decoded = parsed(&archive(companion));
        assert!(
            decoded.particle_system_companion.is_none(),
            "empty and historical NullSnapshot payloads normalize to absence"
        );
    }
    let encoded = encode_snapshot_archive(&owned, &fixture_save_info());
    let reparsed = parsed(&encoded);
    assert_eq!(
        reparsed.particle_system_companion.as_deref(),
        Some(payload.as_slice()),
        "resaving uses the candidate's exact owned particle payload"
    );
}

#[test]
fn corrupt_particle_candidate_does_not_contaminate_prior_candidate() {
    let payload = local_particle_bytes("CandidateSurvives", Vec3::new(3.0, 4.0, 5.0));
    let good = parsed(&archive(Some(payload.clone())));
    let mut corrupt = decode_versioned_archive(&archive(Some(local_particle_bytes(
        "FailedCandidate",
        Vec3::ZERO,
    ))));
    corrupt.snapshot.version += 1;
    assert!(decode_versioned_save_archive(&encode_versioned_archive(&corrupt)).is_err());
    assert_eq!(
        good.particle_system_companion.as_deref(),
        Some(payload.as_slice())
    );
    let mut manager = local_manager(&["CandidateSurvives"]);
    assert_loaded(&mut manager, &good, Vec3::new(3.0, 4.0, 5.0));
}
