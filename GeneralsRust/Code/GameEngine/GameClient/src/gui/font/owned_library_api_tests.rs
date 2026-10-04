// Child of gui/font.rs; no ambient library or fake FontData.
use super::*;

#[test]
fn owned_font_library_preserves_cache_identity_traversal_and_cleanup() {
    let mut library = FontLibrary::new();
    library.init_mut().unwrap();
    let first_desc = FontDesc::new("Arial", 12, false);
    let second_desc = FontDesc::new("Arial", 13, true);
    let first = library.get_font(&first_desc).unwrap();
    let second = library.get_font(&second_desc).unwrap();
    assert_eq!(library.get_count(), 2);
    assert_eq!(library.get_cache_stats(), (0, 2));
    let hit = library.get_font(&first_desc).unwrap();
    assert!(Arc::ptr_eq(&first, &hit));
    assert_eq!(library.get_cache_stats(), (1, 2));
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &second));
    assert!(Arc::ptr_eq(
        &library.next_font(&second_desc).unwrap(),
        &first
    ));
    assert!(library.next_font(&first_desc).is_none());
    drop(hit);
    drop(first);
    library.cleanup_cache();
    assert_eq!(library.get_count(), 1);
    assert!(Arc::ptr_eq(&library.first_font().unwrap(), &second));
    assert!(library.next_font(&second_desc).is_none());
}

#[test]
fn owned_font_libraries_reset_and_statistics_do_not_cross_instances() {
    let mut first = FontLibrary::new();
    let mut second = FontLibrary::new();
    first.init_mut().unwrap();
    second.init_mut().unwrap();
    let desc = FontDesc::new("Arial", 12, false);
    let first_font = first.get_font(&desc).unwrap();
    let second_font = second.get_font(&desc).unwrap();
    assert!(!Arc::ptr_eq(&first_font, &second_font));
    first.get_font(&desc).unwrap();
    assert_eq!(first.get_cache_stats(), (1, 1));
    assert_eq!(second.get_cache_stats(), (0, 1));
    first.reset_mut().unwrap();
    assert_eq!(first.get_count(), 0);
    assert_eq!(first.get_cache_stats(), (0, 0));
    assert_eq!(second.get_count(), 1);
    assert_eq!(second.get_cache_stats(), (0, 1));
    assert!(Arc::ptr_eq(&second.get_font(&desc).unwrap(), &second_font));
    first.shutdown().unwrap();
    assert!(matches!(
        first.get_font(&desc),
        Err(FontError::NotInitialized)
    ));
    assert_eq!(first_font.desc, desc); // Existing retained-font lifetime remains.
}
