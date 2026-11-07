//! Drawable.cpp uses Logic time for expiry and Client time for flash pulses.
use super::*;

fn receiver(id: u32) -> Drawable {
    let mut drawable = Drawable::new(1, id, "ClockModel".into(), DrawableType::Static);
    drawable.set_expiration_date(4);
    drawable.set_flash_count(4);
    drawable
}

#[test]
fn every_call_pulses_even_for_repeated_client_frames_and_isolated_receivers() {
    let mut drawable = receiver(77);
    let mut peer = receiver(77);
    drawable.update(0., 0, 0).unwrap();
    drawable.update(0., 0, 0).unwrap();
    assert_eq!(drawable.get_flash_count(), 2);
    assert_eq!(peer.get_flash_count(), 4);
    peer.update(0., 30, 3).unwrap();
    assert_eq!(peer.get_flash_count(), 3);
    drawable.update(0., 15, 3).unwrap();
    assert_eq!(drawable.get_flash_count(), 1);
    drawable.update(0., 15, 4).unwrap();
    assert_eq!(drawable.get_flash_count(), 1, "Logic expiry precedes Client flash");
    peer.update(0., 30, 4).unwrap();
    assert_eq!(peer.get_flash_count(), 3);
}

#[test]
fn expiration_follows_fade_and_stops_later_effects_on_a_repeated_frame() {
    let mut drawable = receiver(9);
    drawable.fade_in(4);
    drawable.animation_clips.insert(
        "move".into(),
        AnimationClip {
            name: "move".into(),
            duration: 10.0,
            loop_animation: false,
            keyframes: Vec::new(),
            events: Vec::new(),
        },
    );
    drawable.play_animation("move", false, 0.0).unwrap();
    drawable.update(1.0, 15, 3).unwrap();
    assert_eq!(drawable.time_elapsed_fade(), 1);
    assert_eq!(drawable.animation_time, 1.0);
    assert_eq!(drawable.get_flash_count(), 3);
    drawable.update(1.0, 15, 4).unwrap();
    assert_eq!(drawable.time_elapsed_fade(), 2);
    assert_eq!(drawable.animation_time, 1.0);
    assert_eq!(drawable.get_flash_count(), 3);
}

#[test]
fn restored_drawable_keeps_deadline_and_updates_on_same_client_frame() {
    let mut drawable = receiver(3);
    drawable.update(0., 30, 3).unwrap();
    let mut bytes = Vec::new();
    crate::common::Snapshot::xfer(
        &mut drawable,
        &mut game_engine::common::system::xfer_save::XferSave::new(
            std::io::Cursor::new(&mut bytes), 1,
        ),
    );
    let mut restored = receiver(3);
    crate::common::Snapshot::xfer(
        &mut restored,
        &mut game_engine::common::system::xfer_load::XferLoad::new(std::io::Cursor::new(bytes), 1),
    );
    assert_eq!(restored.expiration_date(), 4);
    assert_eq!(restored.get_flash_count(), 3);
    drawable.update(0., 30, 3).unwrap();
    restored.update(0., 30, 3).unwrap();
    assert_eq!(drawable.get_flash_count(), 2);
    assert_eq!(restored.get_flash_count(), 2);
    restored.update(0., 30, 4).unwrap();
    assert_eq!(restored.get_flash_count(), 2);
}
