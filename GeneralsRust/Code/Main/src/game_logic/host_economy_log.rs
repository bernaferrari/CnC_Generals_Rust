//! Completed canonical cash observations and Money audio owned by one match.
//! These receipts never reconcile or write player balances.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostEconomyEvent {
    pub player_id: u32,
    /// Absolute supplies after the host mutation.
    pub supplies: u32,
    /// Absolute power_available after the host mutation (best-effort).
    pub power_available: i32,
}

/// C++ `Money::withdraw` / `Money::deposit` MiscAudio (Money.cpp:32-56).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMoneyAudio {
    Withdraw,
    Deposit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostMoneyAudioEvent {
    pub player_id: u32,
    pub kind: HostMoneyAudio,
}

#[derive(Debug, Default)]
pub struct EconomyEvents {
    pending: Vec<HostEconomyEvent>,
    completed: Vec<HostEconomyEvent>,
    money_audio: Vec<HostMoneyAudioEvent>,
}
impl EconomyEvents {
    pub fn record(&mut self, player_id: u32, supplies: u32, power_available: i32) {
        self.pending.push(HostEconomyEvent {
            player_id,
            supplies,
            power_available,
        });
    }
    pub fn record_money_audio(&mut self, player_id: u32, kind: HostMoneyAudio) {
        self.money_audio
            .push(HostMoneyAudioEvent { player_id, kind });
    }
    pub(crate) fn complete_boundary(&mut self) {
        self.completed.append(&mut self.pending);
    }
    pub(crate) fn take_completed(&mut self) -> Vec<HostEconomyEvent> {
        std::mem::take(&mut self.completed)
    }
    pub(crate) fn take_money_audio(&mut self) -> Vec<HostMoneyAudioEvent> {
        std::mem::take(&mut self.money_audio)
    }
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
        self.money_audio.clear();
    }
}

pub fn money_audio_token(kind: HostMoneyAudio) -> &'static str {
    match kind {
        HostMoneyAudio::Withdraw => "MoneyWithdrawSound",
        HostMoneyAudio::Deposit => "MoneyDepositSound",
    }
}

/// Resolve MiscAudio.ini playable name; fall back to the INI token.
///
/// C++ `TheAudio->getMiscAudio()->m_*` then `addAudioEvent` of that event
/// (`INI::parseAudioEventRTS` stores the value token as the event name).
pub fn resolve_misc_audio_event(token: &str) -> String {
    let Some(misc) = game_engine::common::ini::ini_misc_audio::get_misc_audio() else {
        return token.to_string();
    };
    let misc = misc.read();
    let Some(event) = misc.get_audio_event(token) else {
        return token.to_string();
    };
    let name = event.playable_event_name();
    if name.is_empty() {
        token.to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod owned_receipt_tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Resources, Team};
    use crate::presentation_frame::{PresentationEvent, PresentationFrame};

    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "CashOwner", true);
        player.resources.supplies = 1_000;
        world.add_player(player);
        world
    }
    fn withdraw(world: &mut GameLogic, amount: u32) -> bool {
        let (player, output) = world.player_and_economy_events_mut(0).unwrap();
        player.spend_resources(
            &Resources {
                supplies: amount,
                power: 0,
            },
            output,
        )
    }
    fn credit(world: &mut GameLogic, amount: u32) {
        let (player, output) = world.player_and_economy_events_mut(0).unwrap();
        player.credit_supplies(amount, output);
    }
    #[test]
    fn real_money_receipts_keep_receiver_fifo_across_peer_reset_restore_and_freeze() {
        let mut owner = world();
        let mut peer = world();
        assert!(withdraw(&mut owner, 100));
        owner.economy_events.complete_boundary();
        assert!(withdraw(&mut peer, 200));
        peer.economy_events.complete_boundary();
        let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
        assert_eq!(
            peer_frame.money_audio_receipts,
            vec![HostMoneyAudioEvent {
                player_id: 0,
                kind: HostMoneyAudio::Withdraw
            }]
        );
        peer.reset();
        assert_eq!(owner.get_player(0).unwrap().resources.supplies, 900);
        credit(&mut owner, 25);
        owner.economy_events.complete_boundary();
        let snapshot = crate::save_load::SnapshotBuilder::new()
            .create_world_snapshot(&owner)
            .unwrap();
        let mut peer = world();
        credit(&mut peer, 50);
        crate::save_load::SnapshotBuilder::new()
            .restore_from_snapshot(&snapshot, &mut peer)
            .unwrap();
        assert!(peer.economy_events.take_completed().is_empty());
        assert!(peer.economy_events.take_money_audio().is_empty());
        assert_eq!(peer.get_player(0).unwrap().resources.supplies, 925);
        let frame = PresentationFrame::build_from_logic(&mut owner, 0);
        let cash = frame
            .events
            .iter()
            .filter_map(|event| match event {
                PresentationEvent::EconomyChanged {
                    player_id: 0,
                    supplies,
                    ..
                } => Some(*supplies),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(cash, vec![900, 925]);
        assert_eq!(
            frame.money_audio_receipts,
            vec![
                HostMoneyAudioEvent {
                    player_id: 0,
                    kind: HostMoneyAudio::Withdraw
                },
                HostMoneyAudioEvent {
                    player_id: 0,
                    kind: HostMoneyAudio::Deposit
                },
            ]
        );
        // Collecting a frozen frame cannot consume another world's sound.
        credit(&mut peer, 7);
        let _ = frame.collect_audio_events();
        assert_eq!(peer.economy_events.take_money_audio().len(), 1);
        let again = PresentationFrame::build_from_logic(&mut owner, 0);
        assert!(again.money_audio_receipts.is_empty());
        assert!(
            !again
                .events
                .iter()
                .any(|event| matches!(event, PresentationEvent::EconomyChanged { .. }))
        );
        assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 925);
    }
    #[test]
    fn actual_positive_bounty_requests_cpp_deposit_before_award_observation() {
        let mut owner = world();
        owner.get_player_mut(0).unwrap().cash_bounty_percent = 0.125;
        let (player, output) = owner.player_and_economy_events_mut(0).unwrap();
        assert_eq!(player.do_bounty_for_kill(101, output), 13);
        assert_eq!(player.resources.supplies, 1_013);
        assert_eq!(player.statistics.money_earned, 13);
        owner.economy_events.complete_boundary();
        assert_eq!(
            owner.economy_events.take_money_audio(),
            vec![HostMoneyAudioEvent {
                player_id: 0,
                kind: HostMoneyAudio::Deposit
            }]
        );
        assert_eq!(
            owner.economy_events.take_completed(),
            vec![HostEconomyEvent {
                player_id: 0,
                supplies: 1_013,
                power_available: 0
            }]
        );
    }
    #[test]
    fn cpp_unsigned_deposit_routes_wrap_the_actual_balance_and_preserve_save_future() {
        for route in 0..5 {
            let mut owner = world();
            owner.get_player_mut(0).unwrap().resources.supplies = u32::MAX - 2;
            let (player, output) = owner.player_and_economy_events_mut(0).unwrap();
            match route {
                0 => player.apply_supply_gain(5, output),
                1 => player.add_resources(
                    &Resources {
                        supplies: 5,
                        power: 0,
                    },
                    output,
                ),
                2 => player.credit_supplies(5, output),
                3 => player.refund_production_cost(
                    &Resources {
                        supplies: 5,
                        power: 0,
                    },
                    output,
                ),
                4 => {
                    player.cash_bounty_percent = 1.0;
                    assert_eq!(player.do_bounty_for_kill(5, output), 5);
                }
                _ => unreachable!(),
            }
            assert_eq!(
                owner.get_player(0).unwrap().resources.supplies,
                2,
                "route {route}"
            );
            assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 2);
            assert!(
                !withdraw(&mut owner, 3),
                "post-overflow affordability is canonical"
            );
            owner.economy_events.complete_boundary();
            assert_eq!(
                owner
                    .economy_events
                    .take_completed()
                    .last()
                    .unwrap()
                    .supplies,
                2
            );
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&owner).unwrap();
            let mut restored = world();
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            assert_eq!(restored.get_player(0).unwrap().resources.supplies, 2);
            credit(&mut owner, u32::MAX);
            credit(&mut restored, u32::MAX);
            assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1);
            assert_eq!(restored.get_player(0).unwrap().resources.supplies, 1);
        }
    }

    #[test]
    fn receiving_refund_deposits_before_observation_and_wraps_without_income_credit() {
        let mut owner = world();
        owner.get_player_mut(0).unwrap().resources.supplies = u32::MAX - 2;
        let collected = owner.get_player(0).unwrap().statistics.resources_collected;
        owner.player_refund_supplies(0, 5);
        assert_eq!(owner.get_player(0).unwrap().resources.supplies, 2);
        assert_eq!(
            owner.get_player(0).unwrap().statistics.resources_collected,
            collected
        );
        owner.economy_events.complete_boundary();
        assert_eq!(
            owner
                .economy_events
                .take_completed()
                .last()
                .unwrap()
                .supplies,
            2
        );
        assert_eq!(
            owner.economy_events.take_money_audio(),
            vec![HostMoneyAudioEvent {
                player_id: 0,
                kind: HostMoneyAudio::Deposit
            }]
        );
        owner.player_refund_supplies(0, 0);
        assert!(owner.economy_events.take_money_audio().is_empty());
    }

    #[test]
    fn rejected_and_zero_money_operations_leave_owner_and_output_unchanged() {
        let mut owner = world();
        assert!(!withdraw(&mut owner, 1_001));
        assert!(withdraw(&mut owner, 0));
        credit(&mut owner, 0);
        owner.economy_events.complete_boundary();
        assert_eq!(owner.get_player(0).unwrap().resources.supplies, 1_000);
        assert!(owner.economy_events.take_money_audio().is_empty());
        let values = owner.economy_events.take_completed();
        assert!(values.iter().all(|event| event.supplies == 1_000));
    }
}
