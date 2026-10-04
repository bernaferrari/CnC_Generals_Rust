//! Actual FontLibrary regression for CPP GameFont.cpp linkFont/getFont and
//! GameFont.h firstFont/nextFont. No global library or fabricated FontData.

use super::*;

#[test]
fn cpp_font_library_head_first_traversal_tracks_actual_cache_lifetimes() {
    let mut library = FontLibrary::new();
    library.init_mut().unwrap();
    let first_desc = FontDesc::new("Arial", 12, false);
    let second_desc = FontDesc::new("Arial", 13, true);
    let third_desc = FontDesc::new("Arial", 14, false);
    assert_eq!(library.get_count(), 0);
    assert!(library.first_font().is_none());
    assert!(library.next_font(&first_desc).is_none());

    let first = library.get_font(&first_desc).unwrap();
    assert_eq!(library.get_count(), 1);
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &first));
    assert!(library.next_font(&first_desc).is_none());

    let second = library.get_font(&second_desc).unwrap();
    assert_eq!(library.get_count(), 2);
    assert!(
        Arc::ptr_eq(&library.first_font().unwrap(), &second),
        "CPP linkFont prepends the newly loaded font"
    );
    assert!(Arc::ptr_eq(
        &library.next_font(&second_desc).unwrap(),
        &first
    ));
    assert!(library.next_font(&first_desc).is_none());
    let again_first = library.get_font(&first_desc).unwrap();
    assert!(Arc::ptr_eq(&again_first, &first));
    assert!(
        Arc::ptr_eq(&library.first_font().unwrap(), &second),
        "CPP getFont returns existing identity without relinking it"
    );
    assert_eq!(library.get_count(), 2);

    let third = library.get_font(&third_desc).unwrap();
    assert_eq!(library.get_count(), 3);
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &third));
    assert!(Arc::ptr_eq(
        &library.next_font(&third_desc).unwrap(),
        &second
    ));
    assert!(Arc::ptr_eq(
        &library.next_font(&second_desc).unwrap(),
        &first
    ));
    assert!(library.next_font(&first_desc).is_none());

    // Existing Rust Weak-cache cleanup must preserve the remaining head chain.
    let old_second = Arc::downgrade(&second);
    drop(second);
    SubsystemInterface::update(&mut library).unwrap();
    assert!(old_second.upgrade().is_none());
    assert_eq!(library.get_count(), 2);
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &third));
    assert!(Arc::ptr_eq(
        &library.next_font(&third_desc).unwrap(),
        &first
    ));
    assert!(library.next_font(&first_desc).is_none());
    assert!(library.next_font(&second_desc).is_none());

    let replacement_second = library.get_font(&second_desc).unwrap();
    assert!(!std::sync::Weak::ptr_eq(
        &old_second,
        &Arc::downgrade(&replacement_second)
    ));
    assert_eq!(library.get_count(), 3);
    assert!(Arc::ptr_eq(
        &library.first_font().unwrap(),
        &replacement_second
    ));
    assert!(Arc::ptr_eq(
        &library.next_font(&second_desc).unwrap(),
        &third
    ));
    assert!(Arc::ptr_eq(
        &library.next_font(&third_desc).unwrap(),
        &first
    ));
    assert!(library.next_font(&first_desc).is_none());
    let again_third = library.get_font(&third_desc).unwrap();
    assert!(Arc::ptr_eq(&again_third, &third));
    assert!(Arc::ptr_eq(
        &library.first_font().unwrap(),
        &replacement_second
    ));

    drop(replacement_second);
    drop(again_third);
    drop(third);
    library.cleanup_cache();
    assert_eq!(library.get_count(), 1);
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &first));
    assert!(library.next_font(&first_desc).is_none());
    drop(again_first);
    drop(first);
    SubsystemInterface::update(&mut library).unwrap();
    assert_eq!(library.get_count(), 0);
    assert!(library.first_font().is_none());
    assert!(library.next_font(&first_desc).is_none());
}
