use super::*;
use crate::gui::font::FontLibrary;

fn independently_cached_fonts() -> (Arc<GameFont>, Arc<GameFont>) {
    let desc = FontDesc::new("Arial", 12, false);
    let mut first = FontLibrary::new();
    let mut second = FontLibrary::new();
    first.init_mut().unwrap();
    second.init_mut().unwrap();
    let first_font = first.get_font(&desc).unwrap();
    let second_font = second.get_font(&desc).unwrap();
    assert_eq!(first_font.desc, second_font.desc);
    assert!(!Arc::ptr_eq(&first_font, &second_font));
    (first_font, second_font)
}

// W3DDisplayString.cpp:259-281 compares the actual font pointer before
// updating extents. Equal authored descriptors do not mean equal sources.
#[test]
fn owned_font_replacement_uses_identity_and_invalidates_layout() {
    let (first, replacement) = independently_cached_fonts();
    let mut display = DisplayString::new();
    display.set_text("Generals");
    display.set_font(Arc::clone(&first));
    let _ = display.get_size();
    assert!(!display.dirty);
    display.set_font(Arc::clone(&first));
    assert!(!display.dirty, "the same font retains cached extents");
    display.set_font(Arc::clone(&replacement));
    assert!(Arc::ptr_eq(display.get_font().unwrap(), &replacement));
    assert!(display.dirty, "a different font recomputes extents");
}

#[test]
fn borrowed_arc_font_replacement_uses_identity_and_invalidates_layout() {
    let (first, replacement) = independently_cached_fonts();
    let mut display = DisplayString::new();
    display.set_text("Generals");
    display.set_font(&first);
    let _ = display.get_size();
    display.set_font(&first);
    assert!(!display.dirty);
    display.set_font(&replacement);
    assert!(Arc::ptr_eq(display.get_font().unwrap(), &replacement));
    assert!(display.dirty);
}
