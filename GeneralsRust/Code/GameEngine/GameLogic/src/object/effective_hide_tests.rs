//! C++ Drawable.h:305 keeps explicit/stealth hiding separate from visibility.
use super::*;

#[test]
fn effective_hide_truth_table_preserves_independent_visibility() {
    let _serial = crate::test_sync::lock();
    for visible in [false, true] {
        for hidden in [false, true] {
            for stealth_hidden in [false, true] {
                let mut drawable = Drawable::new(
                    0xD30_1,
                    INVALID_ID,
                    "HideFlags".into(),
                    DrawableType::Static,
                );
                drawable.set_drawable_hidden(hidden).unwrap();
                drawable.hidden_by_stealth = stealth_hidden;
                drawable.update_hidden_status();
                drawable.set_visible(visible);
                assert_eq!(
                    drawable.is_drawable_effectively_hidden(),
                    hidden || stealth_hidden,
                    "visible={visible}, hidden={hidden}, stealth_hidden={stealth_hidden}"
                );
                assert_eq!(
                    drawable.is_currently_visible(),
                    visible && !hidden && !stealth_hidden
                );
            }
        }
    }
}
