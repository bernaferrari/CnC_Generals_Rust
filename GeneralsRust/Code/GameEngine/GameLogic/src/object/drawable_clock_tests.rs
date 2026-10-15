//! Original Drawable expiry observes Logic time; flash scheduling observes Client time.
use super::*;

fn receiver(id: u32) -> Drawable {
    let mut drawable = Drawable::new(1, id, "ClockModel".into(), DrawableType::Static);
    drawable.set_expiration_date(4);
    drawable.set_flash_count(4);
    drawable
}

#[test]
fn drawable_expiration_and_flash_observe_independent_operation_clocks() {
    let mut drawable = receiver(77);
    let mut peer = receiver(77);
    drawable.update(0., 30, 3).unwrap();
    assert_eq!(drawable.last_update_frame, 30);
    assert_eq!(
        drawable.get_flash_count(),
        3,
        "Client30 pulses while Logic3 has not reached expiration4"
    );
    assert_eq!(peer.last_update_frame, 0);
    assert_eq!(peer.get_flash_count(), 4);
    drawable.update(0., 45, 4).unwrap();
    assert_eq!(drawable.last_update_frame, 45);
    assert_eq!(
        drawable.get_flash_count(),
        3,
        "Logic4 expires before a second Client45 flash pulse"
    );
    peer.update(0., 3, 30).unwrap();
    assert_eq!(peer.last_update_frame, 3);
    assert_eq!(
        peer.get_flash_count(),
        4,
        "a small Client clock cannot postpone an already reached Logic deadline"
    );
}

#[test]
fn drawable_restored_expiration_keeps_logic_deadline_and_client_pulse_schedule() {
    let mut drawable = receiver(3);
    drawable.update(0., 30, 3).unwrap();
    let mut bytes = Vec::new();
    crate::common::Snapshot::xfer(
        &mut drawable,
        &mut game_engine::common::system::xfer_save::XferSave::new(
            std::io::Cursor::new(&mut bytes),
            1,
        ),
    );
    let mut restored = receiver(3);
    crate::common::Snapshot::xfer(
        &mut restored,
        &mut game_engine::common::system::xfer_load::XferLoad::new(std::io::Cursor::new(bytes), 1),
    );
    assert_eq!(restored.expiration_date(), 4);
    assert_eq!(restored.get_flash_count(), 3);
    restored.update(0., 60, 4).unwrap();
    assert_eq!(restored.last_update_frame, 60);
    assert_eq!(
        restored.get_flash_count(),
        3,
        "restored Logic deadline returns before the receiving Client60 pulse"
    );
    let mut never_expires = receiver(4);
    never_expires.set_expiration_date(0);
    never_expires.update(0., 30, u32::MAX).unwrap();
    assert_eq!(never_expires.get_flash_count(), 3);
}

#[test]
fn drawable_client_frame_wrap_preserves_frequency_and_duplicate_admission() {
    let mut drawable = receiver(8);
    drawable.set_expiration_date(0);
    drawable.update(0., u32::MAX, 3).unwrap();
    assert_eq!(drawable.last_update_frame, u32::MAX);
    assert_eq!(drawable.get_flash_count(), 3);
    drawable.update(0., 0, 3).unwrap();
    assert_eq!(drawable.last_update_frame, 0);
    assert_eq!(
        drawable.get_flash_count(),
        2,
        "wrapped Client0 is a new admitted flash frame, not subtraction overflow"
    );
    drawable.update(0., 0, 3).unwrap();
    assert_eq!(
        drawable.get_flash_count(),
        2,
        "repeating wrapped Client0 retains the existing frequency policy"
    );
    drawable.update_frequency = 2;
    drawable.update(0., 1, 3).unwrap();
    assert_eq!(drawable.last_update_frame, 0);
    drawable.update(0., 2, 3).unwrap();
    assert_eq!(drawable.last_update_frame, 2);
}
