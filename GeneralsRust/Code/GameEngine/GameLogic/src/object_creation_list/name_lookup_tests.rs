use super::{ObjectCreationList, ObjectCreationListStore};
use std::sync::Arc;

#[test]
fn immutable_catalog_names_do_not_alias_between_threads() {
    let (store, original) = std::thread::spawn(|| {
        let mut store = ObjectCreationListStore::new();
        let original = store.register_ocl("OCL_ExactName".into(), ObjectCreationList::new());
        (Arc::new(store), original)
    })
    .join()
    .unwrap();
    std::thread::spawn(move || {
        assert!(
            store.find_object_creation_list("OCL_MissingName").is_none(),
            "a new thread's numeric key must not resolve another thread's entry"
        );
        let resolved = store.find_object_creation_list("OCL_ExactName").unwrap();
        assert!(Arc::ptr_eq(&original, &resolved));
        assert!(
            store.find_object_creation_list("ocl_exactname").is_none(),
            "C++ nameToKey preserves exact name case"
        );
        assert!(store.find_object_creation_list("None").is_none());
    })
    .join()
    .unwrap();
}

#[test]
fn moved_catalog_mutation_retains_exact_name_identity() {
    let store = std::thread::spawn(|| {
        let mut store = ObjectCreationListStore::new();
        store.register_ocl("OCL_ExactName".into(), ObjectCreationList::new());
        store
    })
    .join()
    .unwrap();
    std::thread::spawn(move || {
        let mut store = store;
        assert!(store.get_ocl_mut("OCL_MissingName").is_none());
        assert!(store.get_ocl_mut("OCL_ExactName").is_some());
        store.get_or_create_ocl("OCL_ExactName".into());
        assert_eq!(store.get_ocl_count(), 1);
        let replacement = store.register_ocl("OCL_ExactName".into(), ObjectCreationList::new());
        assert_eq!(store.get_ocl_count(), 1);
        assert!(Arc::ptr_eq(
            &replacement,
            &store.find_object_creation_list("OCL_ExactName").unwrap()
        ));
    })
    .join()
    .unwrap();
}
