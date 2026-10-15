use super::*;
use crate::effects::particle_system::{Particle, ParticleInfo};
use std::io::Write;

fn populated() -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    let template = Arc::new(ParticleSystemTemplate::new("SavedParticles".into()));
    manager
        .templates
        .insert(template.name().into(), template.clone());
    for id in [19, 3] {
        manager
            .create_particle_system_with_id_and_random(&template, id, false, &mut |lo, _| lo)
            .unwrap();
        let system = manager.find_particle_system_mut(id).unwrap();
        system.set_position(Vec3::new(id as f32, 2.0, 4.0));
        let info = ParticleInfo {
            lifetime: 24,
            ..Default::default()
        };
        system.push_particle(Particle::new(&info, id, 0));
    }
    manager
}

fn encode(manager: &mut ParticleSystemManager) -> Vec<u8> {
    let mut bytes = Vec::new();
    manager
        .xfer_owned_state(&mut CommonXferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

fn decode(manager: &mut ParticleSystemManager, bytes: &[u8], runtime: bool) -> Result<(), String> {
    if runtime {
        // Exercise the actual native file decoder and its production adapter,
        // independently of the Common memory decoder.
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file.flush().unwrap();
        let mut native = game_engine::System::XferLoad::new();
        native.open(file.path().to_string_lossy().into()).unwrap();
        let result = manager.xfer_owned_state(
            &mut crate::core::game_client::RuntimeCommonXferAdapter::new(&mut native),
        );
        native.close().unwrap();
        result
    } else {
        manager.xfer_owned_state(&mut CommonXferLoad::new(Cursor::new(bytes.to_vec()), 1))
    }
}

fn assert_unchanged_on_error(bytes: &[u8]) {
    for runtime in [false, true] {
        let mut live = populated();
        let owner = live.runtime_owner();
        let before = encode(&mut live);
        let counts = (live.next_system_id, live.particle_count, live.system_count);
        assert!(
            decode(&mut live, bytes, runtime).is_err(),
            "runtime={runtime}"
        );
        assert_eq!(
            encode(&mut live),
            before,
            "failed load changed live systems"
        );
        assert_eq!(
            live.runtime_owner(),
            owner,
            "failed load replaced runtime ownership"
        );
        assert_eq!(
            (live.next_system_id, live.particle_count, live.system_count),
            counts
        );
        assert_eq!(live.active_system_ids_in_order(), [19, 3]);
    }
}

#[test]
fn failed_particle_load_keeps_live_owner_allocator_particles_and_order() {
    let bytes = encode(&mut populated());
    for end in [0, 1, 4, 8, bytes.len() / 2, bytes.len() - 1] {
        assert_unchanged_on_error(&bytes[..end]);
    }
}

fn payload_with_second_entry(duplicate: bool) -> Vec<u8> {
    let mut source = populated();
    let mut bytes = Vec::new();
    let mut xfer = CommonXferSave::new(Cursor::new(&mut bytes), 1);
    xfer.xfer_version(&mut 1, 1).unwrap();
    xfer.xfer_unsigned_int(&mut 999).unwrap();
    xfer.xfer_unsigned_int(&mut 2).unwrap();
    let system = source.find_particle_system_mut(19).unwrap();
    let mut name = system.template().name().to_string();
    xfer.xfer_ascii_string(&mut name).unwrap();
    Snapshotable::xfer(system, &mut xfer).unwrap();
    if duplicate {
        xfer.xfer_ascii_string(&mut name).unwrap();
        Snapshotable::xfer(system, &mut xfer).unwrap();
    } else {
        xfer.xfer_ascii_string(&mut "MissingSavedTemplate".into())
            .unwrap();
    }
    bytes
}

#[test]
fn late_unknown_template_or_duplicate_id_cannot_partially_admit_a_particle_list() {
    assert_unchanged_on_error(&payload_with_second_entry(false));
    assert_unchanged_on_error(&payload_with_second_entry(true));
}

#[test]
fn both_particle_decoders_admit_the_complete_native_payload_in_creation_order() {
    let bytes = encode(&mut populated());
    for runtime in [false, true] {
        let mut destination = populated();
        let owner = destination.runtime_owner();
        decode(&mut destination, &bytes, runtime).unwrap();
        assert_ne!(
            destination.runtime_owner(),
            owner,
            "successful replacement owns new IDs"
        );
        assert_eq!(destination.active_system_ids_in_order(), [19, 3]);
        assert_eq!(destination.particle_count, 2);
        assert_eq!(destination.system_count, 2);
        assert_eq!(
            encode(&mut destination),
            bytes,
            "native system payloads survive both adapters"
        );
    }
}

#[test]
fn native_load_receipt_covers_only_its_admitted_ids_and_live_namespace() {
    let bytes = encode(&mut populated());
    let mut destination = populated();
    let receipt = destination.restore_xfer_bytes(&bytes).unwrap().unwrap();
    assert!(receipt.belongs_to(&destination));
    assert!(receipt.contains_system(19));
    assert!(receipt.contains_system(3));
    let template = destination.find_template("SavedParticles").unwrap();
    destination
        .create_particle_system_with_id_and_random(&template, 42, false, &mut |lo, _| lo)
        .unwrap();
    assert!(
        !receipt.contains_system(42),
        "later-created systems were not loaded by this receipt"
    );
    assert!(
        !receipt.belongs_to(&populated()),
        "same IDs in a peer are not the loaded owner"
    );
    destination.reset().unwrap();
    assert!(
        !receipt.belongs_to(&destination),
        "reset invalidates the receipt before IDs are reused"
    );
}
