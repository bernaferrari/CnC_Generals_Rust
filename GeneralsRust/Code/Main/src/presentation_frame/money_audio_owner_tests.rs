use super::*;
use crate::game_logic::host_economy_log::{
    HostMoneyAudio, money_audio_token, resolve_misc_audio_event,
};
use crate::game_logic::{GameLogic, Player, Team};

#[test]
fn frozen_money_audio_preserves_cpp_player_identity_for_native_locality() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "local", true));
    world.add_player(Player::new(7, Team::GLA, "other", false));
    let (player, output) = world.player_and_economy_events_mut(7).unwrap();
    player.credit_supplies(25, output);
    let frame = PresentationFrame::build_from_logic(&mut world, 0);
    let name = resolve_misc_audio_event(money_audio_token(HostMoneyAudio::Deposit));
    let requests = frame.collect_audio_events();
    let deposits: Vec<_> = requests
        .iter()
        .filter(|request| request.event_type == name)
        .collect();
    assert_eq!(deposits.len(), 1);
    assert_eq!(deposits[0].player_index, Some(7));
    assert!(deposits[0].position.is_none());
    assert!(!deposits[0].is_looping);
    assert!(
        PresentationFrame::build_from_logic(&mut world, 0)
            .money_audio_receipts
            .is_empty()
    );
}
