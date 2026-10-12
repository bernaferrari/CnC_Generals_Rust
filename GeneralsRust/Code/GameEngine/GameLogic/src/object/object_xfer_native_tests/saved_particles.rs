//! Native full-Object wire admission for process-local ActiveBody particle IDs.
use super::*;

fn save_with_particle_ids(ids: &[u32]) -> (Definitions, ObjectSnapshotRecord) {
    let definitions = Definitions::new(75.0);
    let mut sender = Candidate::new();
    let source = sender.prepare_source(&definitions);
    let body_module = source
        .read()
        .unwrap()
        .modules
        .iter()
        .find(|module| module.name().as_str() == "ActiveBody")
        .unwrap()
        .clone();
    let body = source.read().unwrap().get_body_module().unwrap();
    body_module.with_module(|module| {
        crate::contain_module_overrides::with_active_mut(module, &body, |active| {
            active.set_owner_particle_ids_for_test(ids);
        });
    });
    let record = sender.operate(&definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    (definitions, record)
}

#[test]
fn native_full_object_restore_rejects_saved_live_particle_id() {
    let (definitions, record) = save_with_particle_ids(&[0x1234]);
    let mut receiver = Candidate::new();
    let error = match receiver.restore(&definitions, &[record]) {
        Ok(_) => panic!("native restore accepted a process-local particle ID"),
        Err(error) => error,
    };
    assert!(
        error.contains("receiving particle-system owner"),
        "unexpected restore error: {error}"
    );
    assert_eq!(receiver.factory.get_statistics().total_objects, 1);
}

#[test]
fn native_full_object_restore_preserves_invalid_particle_sentinel() {
    let (definitions, record) = save_with_particle_ids(&[0]);
    let mut receiver = Candidate::new();
    let restored = receiver.restore(&definitions, &[record]).unwrap();
    let body = restored[0].read().unwrap().get_body_module().unwrap();
    let object = restored[0].read().unwrap();
    let module = object
        .modules
        .iter()
        .find(|module| module.name().as_str() == "ActiveBody")
        .unwrap();
    assert_eq!(
        module.with_module(|module| crate::contain_module_overrides::with_active(
            module,
            &body,
            |active| active.first_owner_particle_id_for_test(),
        )),
        Some(0)
    );
}
