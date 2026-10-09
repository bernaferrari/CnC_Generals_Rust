//! C++ Weapon::getPercentReadyToFire consumes the driving logic frame.
use super::*;
use crate::game_logic::object::WeaponFireStatus;
use crate::game_logic::{Object, Weapon};

fn reloading_world(frame: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.frame = frame;
    let mut object = Object::new(ThingTemplate::new("ReloadingUnit"), ObjectId(77), Team::USA);
    object.weapon = Some(Weapon {
        last_fire_time: 1.0,
        clip_reload_time: 1.0,
        clip_size: 2,
        ammo: Some(0),
        reloading_clip: true,
        ..Weapon::default()
    });
    object.weapon_fire_status = WeaponFireStatus::ReloadingClip;
    world.add_object(object);
    world
}

fn readiness(frame: &PresentationFrame) -> u32 {
    frame
        .objects
        .iter()
        .find(|object| object.id == ObjectId(77))
        .unwrap()
        .weapon_ready_percent
}

#[test]
fn frozen_weapon_readiness_uses_owning_world_frame() {
    struct RestoreFrame(u32);
    impl Drop for RestoreFrame {
        fn drop(&mut self) {
            crate::game_logic::host_historic_bonus::set_logic_frame(self.0);
        }
    }
    let _restore = RestoreFrame(crate::game_logic::host_historic_bonus::logic_frame());
    let mut first = reloading_world(45);
    crate::game_logic::host_historic_bonus::set_logic_frame(first.frame);
    let first_frame = PresentationFrame::build_from_logic(&mut first, 0);
    assert_eq!(
        readiness(&first_frame),
        50,
        "45 frames is 1.5 seconds at 30Hz"
    );

    let mut second = reloading_world(90);
    crate::game_logic::host_historic_bonus::set_logic_frame(second.frame);
    let second_frame = PresentationFrame::build_from_logic(&mut second, 0);
    assert_eq!(readiness(&second_frame), 100);
    assert_eq!(
        readiness(&first_frame),
        50,
        "the completed frame stays immutable"
    );
    let rebuilt_first = PresentationFrame::build_from_logic(&mut first, 0);
    assert_eq!(
        readiness(&rebuilt_first),
        50,
        "publishing another world's combat frame must not advance this world's clip"
    );

    first.frame = 60;
    crate::game_logic::host_historic_bonus::set_logic_frame(0);
    let advanced = PresentationFrame::build_from_logic(&mut first, 0);
    assert_eq!(
        readiness(&advanced),
        100,
        "owning 60-frame clock completes the one-second clip reload"
    );
}
