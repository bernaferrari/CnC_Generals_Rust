use super::*;

#[test]
fn receiving_logic_expiry_keeps_client_flash_clock_independent() {
    let mut drawable = BasicDrawable::new(DrawableId(90_101));
    drawable.set_expiration_frame(4);
    drawable.color_flash(Vector3::new(1.0, 0.0, 0.0), 2);
    drawable.fade_out(8);
    drawable.set_current_frame(30);
    drawable.update(0.0, 3);
    assert_eq!(drawable.flash_count, 1);
    assert_eq!(drawable.time_elapsed_fade(), 1);
    assert!(!drawable.is_expired(3));

    drawable.set_current_frame(45);
    drawable.update(0.0, 4);
    assert_eq!(
        drawable.flash_count, 1,
        "Logic expiry returns before the scheduled Client flash"
    );
    assert_eq!(
        drawable.time_elapsed_fade(),
        2,
        "fade still precedes expiry"
    );
    assert!(drawable.is_expired(4));

    let mut peer = BasicDrawable::new(DrawableId(90_101));
    peer.set_expiration_frame(4);
    peer.color_flash(Vector3::new(1.0, 0.0, 0.0), 2);
    peer.set_current_frame(3);
    peer.update(0.0, 30);
    assert_eq!(peer.flash_count, 2);
    assert!(peer.is_expired(30));
    assert_eq!(drawable.flash_count, 1);
}

#[test]
fn receiving_expiry_preserves_raw_unsigned_deadline_semantics() {
    let mut drawable = BasicDrawable::new(DrawableId(90_102));
    drawable.set_expiration_frame(0);
    assert!(!drawable.is_expired(0));
    assert!(!drawable.is_expired(u32::MAX));
    drawable.set_expiration_frame(u32::MAX);
    assert!(!drawable.is_expired(u32::MAX - 1));
    assert!(drawable.is_expired(u32::MAX));
    assert!(!drawable.is_expired(u32::MAX.wrapping_add(1)));
    assert!(!Drawable::is_expired(&drawable, 0));
    assert!(Drawable::is_expired(&drawable, u32::MAX));
}
