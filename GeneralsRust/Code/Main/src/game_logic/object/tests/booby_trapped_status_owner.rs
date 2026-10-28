use super::*;

#[test]
fn booby_trapped_mask_updates_canonical_status_and_query_projection() {
    use crate::game_logic::host_status_bits_upgrade::{
        object_status_mask_from_names, status_bits_has,
    };

    let mut object = make_test_object();
    let bit = object_status_mask_from_names(&["BOOBY_TRAPPED"]);

    object.apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
    assert!(object.status.booby_trapped);
    assert!(object.has_object_status_bit("BOOBY_TRAPPED"));
    assert!(status_bits_has(
        object.object_status_mask_for_queries(),
        "BOOBY_TRAPPED"
    ));
    assert_eq!(
        object.object_status_bits & bit,
        0,
        "no writable duplicate bit"
    );

    assert_eq!(object.object_status_mask_for_queries() & bit, bit);

    object.apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &["BOOBY_TRAPPED"]);
    assert!(
        !object.status.booby_trapped,
        "C++ StatusBitsUpgrade sets before clearing"
    );
    assert_eq!(object.object_status_bits & bit, 0);
    assert_eq!(object.object_status_mask_for_queries() & bit, 0);
}

#[test]
fn same_id_objects_keep_booby_trap_status_world_local() {
    let template = ThingTemplate::new("TestStructure");
    let mut first = Object::new(template.clone(), ObjectId(91), Team::GLA);
    let mut second = Object::new(template, ObjectId(91), Team::GLA);
    first.set_status_booby_trapped(true);

    assert!(first.has_object_status_bit("BOOBY_TRAPPED"));
    assert!(!second.has_object_status_bit("BOOBY_TRAPPED"));
    second.apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
    assert!(second.status.booby_trapped);
    assert!(first.status.booby_trapped);
    second.apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
    assert!(first.status.booby_trapped);
    assert!(!second.status.booby_trapped);
}
