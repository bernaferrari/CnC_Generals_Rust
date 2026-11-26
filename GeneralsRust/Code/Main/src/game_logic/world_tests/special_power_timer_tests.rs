//! Object and Player cooldowns keep their independent logic clocks.
use crate::game_logic::{GameLogic, Team};
use crate::presentation_frame::PresentationFrame;

#[test]
fn object_cooldown_ticks_and_emp_freezes_it() {
    use crate::game_logic::{KindOf, ThingTemplate};
    let mut logic = GameLogic::new();

    {
        let mut t = ThingTemplate::new("SoleTickSp");
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("SoleTickSp".into(), t);
    }
    let oid = logic
        .create_object("SoleTickSp", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 4.0;
        o.special_power_ready = false;
    }

    logic.host_object_mut(oid).unwrap().tick_timers(
        1.5,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let unfrozen = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let actor = unfrozen.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(actor.special_power_cooldown_remaining, 2.5);
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .special_power_cooldown_remaining,
        2.5
    );
    logic.host_object_mut(oid).unwrap().status.disabled_emp = true;
    logic.host_object_mut(oid).unwrap().tick_timers(
        1.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .special_power_cooldown_remaining,
        2.5
    );
    let emp = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let actor = emp.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(actor.special_power_cooldown_remaining, 2.5);
}

#[test]
fn shared_special_power_timer_belongs_to_its_player() {
    use crate::command_system::SpecialPowerType;

    let mut logic = GameLogic::new();

    logic.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "OwnedCooldown",
        true,
    ));
    let pid = 0u32;
    let p = logic.get_player_mut(pid).unwrap();
    p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 5.0);
    p.reset_shared_special_power_timer(&SpecialPowerType::ParticleCannon, 55.0);
    p.reset_shared_special_power_timer(&SpecialPowerType::ScudStorm, 10.0);

    let mut peer = GameLogic::new();
    peer.add_player(crate::game_logic::Player::new(pid, Team::USA, "Peer", true));
    peer.get_player_mut(pid)
        .unwrap()
        .reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 9.0);
    logic.tick_shared_special_power_timers(2.0);
    let p = logic.get_player(pid).expect("player");
    let rem = p
        .shared_special_power_cooldowns
        .get(&SpecialPowerType::Airstrike)
        .copied()
        .unwrap_or(-1.0);
    assert!(
        (rem - 3.0).abs() < 1e-3,
        "expected ~3.0 remaining after 2s tick, got {rem}"
    );

    let _frame = PresentationFrame::try_build_from_logic(&mut logic, pid).unwrap();
    let timers = &logic
        .get_player(pid)
        .unwrap()
        .shared_special_power_cooldowns;
    assert_eq!(timers[&SpecialPowerType::ParticleCannon], 53.0);
    assert_eq!(timers[&SpecialPowerType::ScudStorm], 8.0);
    assert_eq!(
        peer.get_player(pid)
            .unwrap()
            .shared_special_power_cooldowns
            .len(),
        1
    );
    let player = logic.get_player_mut(pid).unwrap();
    player.reset_shared_special_power_timer(&SpecialPowerType::ParticleCannon, 3.0);
    assert_eq!(
        player.shared_special_power_cooldowns[&SpecialPowerType::ParticleCannon],
        3.0
    );
    assert_eq!(
        logic
            .get_player(pid)
            .unwrap()
            .shared_special_power_cooldowns[&SpecialPowerType::Airstrike],
        3.0
    );
    assert_eq!(
        peer.get_player(pid).unwrap().shared_special_power_cooldowns[&SpecialPowerType::Airstrike],
        9.0
    );
}
