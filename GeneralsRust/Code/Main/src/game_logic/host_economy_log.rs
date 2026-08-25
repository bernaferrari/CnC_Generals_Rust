//! Frame-local host economy log for GameWorld shadow parity.
//!
//! Player cash mutations record post-change absolute supplies (and power when
//! known). End-of-tick economy authority applies SetSupplies mutations then
//! writebacks host players.

use std::cell::RefCell;

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

thread_local! {
    static LOG: RefCell<Vec<HostEconomyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEconomyEvent>> = RefCell::new(Vec::new());
    static MONEY_AUDIO: RefCell<Vec<HostMoneyAudioEvent>> = const { RefCell::new(Vec::new()) };

}

pub fn record(player_id: u32, supplies: u32, power_available: i32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEconomyEvent {
            player_id,
            supplies,
            power_available,
        });
    });
}

/// Queue MiscAudio MoneyWithdrawSound / MoneyDepositSound for presentation drain.
pub fn record_money_audio(player_id: u32, kind: HostMoneyAudio) {
    MONEY_AUDIO.with(|log| {
        log.borrow_mut()
            .push(HostMoneyAudioEvent { player_id, kind });
    });
}

/// Presentation-only drain. GameWorld does not consume these.
pub fn take_money_audio() -> Vec<HostMoneyAudioEvent> {
    MONEY_AUDIO.with(|log| std::mem::take(&mut *log.borrow_mut()))
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

pub fn has_pending(player_id: u32) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.player_id == player_id))
}

pub fn drain() -> Vec<HostEconomyEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
    MONEY_AUDIO.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostEconomyEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostEconomyEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_log_drain_and_last_snapshot() {
        clear();
        record(0, 1000, 5);
        record(1, 500, -2);
        assert_eq!(len(), 2);
        let v = drain();
        assert_eq!(v.len(), 2);
        assert!(drain().is_empty());
        assert_eq!(last_drain_snapshot().len(), 2);
        assert_eq!(last_drain_snapshot()[0].supplies, 1000);
    }

    #[test]
    fn money_audio_record_and_take() {
        clear();
        record_money_audio(1, HostMoneyAudio::Withdraw);
        record_money_audio(2, HostMoneyAudio::Deposit);
        let v = take_money_audio();
        assert_eq!(v.len(), 2);
        assert_eq!(money_audio_token(v[0].kind), "MoneyWithdrawSound");
        assert_eq!(money_audio_token(v[1].kind), "MoneyDepositSound");
        assert!(take_money_audio().is_empty());
    }
}
