//! Actual GameClient wrapper lifecycle and its exact owned FontLibrary.
//! These are integration controls, not the OLD trait-delegation regressions.

use super::*;
use crate::gui::font::{FontDesc, FontError};

#[test]
fn font_library_subsystem_lifecycle_drives_actual_inner_cache() {
    let mut wrapper = FontLibrarySubsystem::new();
    let first_desc = FontDesc::new("Arial", 12, false);
    let second_desc = FontDesc::new("Arial", 13, true);
    assert_eq!(wrapper.inner.get_count(), 0);
    assert_eq!(wrapper.inner.get_cache_stats(), (0, 0));
    assert!(matches!(
        wrapper.inner.get_font(&first_desc),
        Err(FontError::NotInitialized)
    ));
    SubsystemInterface::update(&mut wrapper).unwrap();
    assert!(matches!(
        wrapper.inner.get_font(&first_desc),
        Err(FontError::NotInitialized)
    ));
    assert_eq!(wrapper.inner.get_cache_stats(), (0, 0));

    SubsystemInterface::init(&mut wrapper).unwrap();
    let first = wrapper.inner.get_font(&first_desc).unwrap();
    assert_eq!(wrapper.inner.get_count(), 1);
    assert_eq!(wrapper.inner.get_cache_stats(), (0, 1));
    assert!(Arc::ptr_eq(&wrapper.inner.first_font().unwrap(), &first));
    assert!(wrapper.inner.next_font(&first_desc).is_none());
    let second = wrapper.inner.get_font(&second_desc).unwrap();
    assert_eq!(wrapper.inner.get_count(), 2);
    assert_eq!(wrapper.inner.get_cache_stats(), (1, 2));
    let hit = wrapper.inner.get_font(&first_desc).unwrap();
    assert!(Arc::ptr_eq(&first, &hit));
    assert_eq!(wrapper.inner.get_cache_stats(), (2, 2));

    drop(hit);
    drop(first);
    SubsystemInterface::update(&mut wrapper).unwrap();
    assert_eq!(wrapper.inner.get_count(), 1);
    assert!(Arc::ptr_eq(&wrapper.inner.first_font().unwrap(), &second));
    assert!(wrapper.inner.next_font(&second_desc).is_none());

    SubsystemInterface::reset(&mut wrapper).unwrap();
    assert_eq!(wrapper.inner.get_count(), 0);
    assert_eq!(wrapper.inner.get_cache_stats(), (0, 0));
    assert!(wrapper.inner.first_font().is_none());
    // Reset retains readiness and does not destroy externally held fonts.
    assert_eq!(second.desc, second_desc);
    let after_reset = wrapper.inner.get_font(&second_desc).unwrap();
    assert!(!Arc::ptr_eq(&second, &after_reset));
    assert_eq!(wrapper.inner.get_cache_stats(), (0, 1));
    assert_eq!(wrapper.inner.get_count(), 1);
    drop(after_reset);
    SubsystemInterface::update(&mut wrapper).unwrap();
    assert_eq!(wrapper.inner.get_count(), 0);
    assert!(wrapper.inner.first_font().is_none());
    drop(wrapper);
    assert_eq!(second.desc, second_desc);
}

#[test]
fn font_library_subsystem_instances_keep_cache_reset_and_retained_fonts_independent() {
    let mut first = FontLibrarySubsystem::new();
    let mut second = FontLibrarySubsystem::new();
    let desc = FontDesc::new("Arial", 12, false);
    SubsystemInterface::init(&mut first).unwrap();
    SubsystemInterface::init(&mut second).unwrap();
    let first_font = first.inner.get_font(&desc).unwrap();
    let second_font = second.inner.get_font(&desc).unwrap();
    assert!(!Arc::ptr_eq(&first_font, &second_font));
    assert_eq!(first.inner.get_cache_stats(), (0, 1));
    assert_eq!(second.inner.get_cache_stats(), (0, 1));

    let mut untouched = FontLibrarySubsystem::new();
    assert_eq!(untouched.inner.get_count(), 0);
    assert!(matches!(
        untouched.inner.get_font(&desc),
        Err(FontError::NotInitialized)
    ));
    assert_eq!(first.inner.get_count(), 1);
    assert_eq!(second.inner.get_count(), 1);
    let hit = first.inner.get_font(&desc).unwrap();
    assert!(Arc::ptr_eq(&hit, &first_font));
    assert_eq!(first.inner.get_cache_stats(), (1, 1));
    assert_eq!(second.inner.get_cache_stats(), (0, 1));

    SubsystemInterface::reset(&mut first).unwrap();
    assert_eq!(first.inner.get_count(), 0);
    assert_eq!(first.inner.get_cache_stats(), (0, 0));
    assert_eq!(second.inner.get_count(), 1);
    assert_eq!(second.inner.get_cache_stats(), (0, 1));
    assert!(Arc::ptr_eq(
        &second.inner.first_font().unwrap(),
        &second_font
    ));
    assert!(second.inner.next_font(&desc).is_none());
    let reloaded = first.inner.get_font(&desc).unwrap();
    assert!(!Arc::ptr_eq(&reloaded, &first_font));
    drop(hit);
    drop(reloaded);
    SubsystemInterface::update(&mut first).unwrap();
    assert_eq!(first.inner.get_count(), 0);
    assert_eq!(second.inner.get_count(), 1);
    drop(first);
    assert_eq!(first_font.desc, desc);
    assert!(Arc::ptr_eq(
        &second.inner.get_font(&desc).unwrap(),
        &second_font
    ));
    drop(second_font);
    SubsystemInterface::update(&mut second).unwrap();
    assert_eq!(second.inner.get_count(), 0);
    assert!(second.inner.first_font().is_none());
}
