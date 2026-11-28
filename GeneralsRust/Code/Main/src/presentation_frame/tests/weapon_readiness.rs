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
    let mut first = reloading_world(45);

    let first_frame = PresentationFrame::try_build_from_logic(&mut first, 0).unwrap();
    assert_eq!(
        readiness(&first_frame),
        50,
        "45 frames is 1.5 seconds at 30Hz"
    );

    let mut second = reloading_world(90);

    let second_frame = PresentationFrame::try_build_from_logic(&mut second, 0).unwrap();
    assert_eq!(readiness(&second_frame), 100);
    assert_eq!(
        readiness(&first_frame),
        50,
        "the completed frame stays immutable"
    );
    let rebuilt_first = PresentationFrame::try_build_from_logic(&mut first, 0).unwrap();
    assert_eq!(
        readiness(&rebuilt_first),
        50,
        "another world's combat frame must not advance this world's clip"
    );

    first.frame = 60;

    let advanced = PresentationFrame::try_build_from_logic(&mut first, 0).unwrap();
    assert_eq!(
        readiness(&advanced),
        100,
        "owning 60-frame clock completes the one-second clip reload"
    );
}

#[test]
fn frozen_weapon_readiness_uses_the_receiving_combat_rules() {
    fn world(rate_of_fire_percent: u32) -> GameLogic {
        let mut world = GameLogic::new();
        world
            .admit_combat_bonus_ini_sources([format!(
                "GameData\n WeaponBonus = SOLO_HUMAN_EASY RATE_OF_FIRE {rate_of_fire_percent}%\nEnd\n"
            )])
            .unwrap();
        world.frame = 45;
        let mut object = Object::new(ThingTemplate::new("CoolingDown"), ObjectId(77), Team::USA);
        object.weapon = Some(Weapon {
            reload_time: 2.0,
            last_fire_time: 1.0,
            ..Weapon::default()
        });
        object.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
        object.weapon_bonus_solo = 16;
        world.add_object(object);
        world
    }

    let mut normal = world(100);
    let mut faster = world(200);
    let frozen = PresentationFrame::try_build_from_logic(&mut normal, 0).unwrap();
    let other = PresentationFrame::try_build_from_logic(&mut faster, 0).unwrap();
    assert_eq!(
        (readiness(&frozen), readiness(&other)),
        (25, 50),
        "the receiving match's rate-of-fire bonus must set HUD readiness"
    );
    faster.frame = 60;
    let advanced = PresentationFrame::try_build_from_logic(&mut faster, 0).unwrap();
    assert_eq!(readiness(&advanced), 100);
    assert_eq!(readiness(&frozen), 25);
}
