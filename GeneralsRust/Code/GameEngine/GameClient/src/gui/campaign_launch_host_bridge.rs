//! Typed single-player campaign-launch delivery for AuthorityOnly Main.
//!
//! C++ uses one GlobalData/GameInfo path from the shell into GameLogic.  Rust's
//! retail GameClient shell and Main authority are deliberately separate, so a
//! campaign or Challenge launch must carry its exact selection across that
//! boundary instead of letting Main guess from a stale HUD faction.

use std::cell::RefCell;

/// Immutable shell selection that belongs to exactly one queued `MSG_NEW_GAME`.
///
/// `player_template_*` is deliberately opaque to this bridge: Main validates
/// the template only to derive a base faction today.  The active GameLogic
/// PlayerTemplate binding is a separate authority seam, rather than silently
/// treating a General template as an ordinary Team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCampaignLaunchDescriptor {
    /// Monotonic process-local identity. Zero is never published.
    pub generation: u64,
    pub map_name: String,
    pub campaign_name: String,
    pub campaign_player_faction: String,
    pub is_challenge: bool,
    /// C++ `TheChallengeGameInfo->getStartingCash().countMoney()` captured
    /// with this launch. `Some(0)` is present GameInfo, not default cash.
    /// Ordinary campaigns have no GameInfo and carry `None`.
    pub game_info_starting_cash: Option<u32>,
    pub player_template_name: Option<String>,
    pub player_template_index: Option<i32>,
    pub game_mode_code: i32,
    pub difficulty_code: i32,
    pub rank_points: i32,
    pub max_fps: Option<i32>,
}

impl HostCampaignLaunchDescriptor {
    /// Match the exact message payload emitted immediately after this
    /// descriptor.  A mismatch is stale UI state and must never affect a later
    /// NewGame request.
    pub fn matches_new_game(
        &self,
        game_mode_code: i32,
        difficulty_code: i32,
        rank_points: i32,
        max_fps: Option<i32>,
    ) -> bool {
        self.generation != 0
            && !self.map_name.trim().is_empty()
            && self.game_mode_code == game_mode_code
            && self.difficulty_code == difficulty_code
            && self.rank_points == rank_points
            && self.max_fps == max_fps
    }
}

#[derive(Default)]
struct HostCampaignLaunchBridge {
    enabled: bool,
    next_generation: u64,
    pending: Option<HostCampaignLaunchDescriptor>,
}

/// Result of consuming the one shell descriptor associated with a NewGame
/// dispatch.  `Mismatched` is distinct from `None`: the latter is a normal
/// non-campaign launch, while the former means a campaign map/faction was
/// already mirrored into GlobalData for a different message and must be
/// rejected rather than allowed to leak into this dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCampaignLaunchDelivery {
    None,
    Matched(HostCampaignLaunchDescriptor),
    Mismatched,
}

// THREAD: main thread only — published by the GUI-thread shell menus and
// consumed by the host's NewGame dispatch on the same thread.
thread_local! {
    static HOST_CAMPAIGN_LAUNCH_BRIDGE: RefCell<HostCampaignLaunchBridge> =
        RefCell::new(HostCampaignLaunchBridge::default());
}

fn with_host_campaign_launch_bridge<R>(f: impl FnOnce(&mut HostCampaignLaunchBridge) -> R) -> R {
    HOST_CAMPAIGN_LAUNCH_BRIDGE.with_borrow_mut(f)
}

/// Enable Main-host delivery for campaign and Challenge shell launch state.
///
/// Standalone GameClient preserves its C++ singleton/message-stream path when
/// disabled. Changing owner discards only an unconsumed descriptor.
pub fn set_host_campaign_launch_bridge_enabled(enabled: bool) {
    with_host_campaign_launch_bridge(|bridge| {
        if bridge.enabled != enabled {
            bridge.enabled = enabled;
            bridge.pending = None;
        }
    });
}

/// Publish the exact shell selection immediately before its `MSG_NEW_GAME`.
///
/// Returns `false` when standalone GameClient remains the owner, so callers
/// retain their legacy behavior without a Main-only conditional path.
pub fn publish_host_campaign_launch(mut descriptor: HostCampaignLaunchDescriptor) -> bool {
    with_host_campaign_launch_bridge(|bridge| {
        if !bridge.enabled || descriptor.map_name.trim().is_empty() {
            return false;
        }

        bridge.next_generation = bridge.next_generation.wrapping_add(1).max(1);
        descriptor.generation = bridge.next_generation;
        bridge.pending = Some(descriptor);
        true
    })
}

/// Consume the pending descriptor for an exact matching NewGame message.
///
/// A mismatch also consumes the descriptor, but reports `Mismatched` rather
/// than collapsing it to `None`: Main must clear the paired GlobalData map
/// mirrors instead of treating it as an ordinary launch.
pub fn take_host_campaign_launch_for_new_game(
    game_mode_code: i32,
    difficulty_code: i32,
    rank_points: i32,
    max_fps: Option<i32>,
) -> HostCampaignLaunchDelivery {
    with_host_campaign_launch_bridge(|bridge| {
        let Some(descriptor) = bridge.pending.take() else {
            return HostCampaignLaunchDelivery::None;
        };
        if descriptor.matches_new_game(game_mode_code, difficulty_code, rank_points, max_fps) {
            HostCampaignLaunchDelivery::Matched(descriptor)
        } else {
            HostCampaignLaunchDelivery::Mismatched
        }
    })
}

/// Invalidate a descriptor at a host world/session boundary.
pub fn clear_host_campaign_launch_descriptor() {
    with_host_campaign_launch_bridge(|bridge| bridge.pending = None);
}

#[cfg(test)]
pub(crate) struct HostCampaignLaunchBridgeTestGuard;

#[cfg(test)]
pub(crate) fn acquire_host_campaign_launch_bridge_test_guard() -> HostCampaignLaunchBridgeTestGuard
{
    // The bridge is TLS on the driving GUI thread. Each fixture resets only
    // that thread's state; a process-wide test mutex has no shared owner here.
    clear_host_campaign_launch_descriptor();
    set_host_campaign_launch_bridge_enabled(false);
    HostCampaignLaunchBridgeTestGuard
}

#[cfg(test)]
impl Drop for HostCampaignLaunchBridgeTestGuard {
    fn drop(&mut self) {
        clear_host_campaign_launch_descriptor();
        set_host_campaign_launch_bridge_enabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> HostCampaignLaunchDescriptor {
        HostCampaignLaunchDescriptor {
            generation: 0,
            map_name: "Maps/GC_TankGeneral/GC_TankGeneral.map".into(),
            campaign_name: "CHALLENGE_1".into(),
            campaign_player_faction: String::new(),
            is_challenge: true,
            game_info_starting_cash: Some(0),
            player_template_name: Some("FactionChinaTankGeneral".into()),
            player_template_index: Some(7),
            game_mode_code: 0,
            difficulty_code: 2,
            rank_points: 300,
            max_fps: Some(30),
        }
    }

    #[test]
    fn descriptor_publishes_only_for_enabled_owner_and_matches_exact_new_game() {
        let _guard = acquire_host_campaign_launch_bridge_test_guard();
        assert!(!publish_host_campaign_launch(descriptor()));
        assert_eq!(
            take_host_campaign_launch_for_new_game(0, 2, 300, Some(30)),
            HostCampaignLaunchDelivery::None
        );

        set_host_campaign_launch_bridge_enabled(true);
        assert!(publish_host_campaign_launch(descriptor()));
        let HostCampaignLaunchDelivery::Matched(delivered) =
            take_host_campaign_launch_for_new_game(0, 2, 300, Some(30))
        else {
            panic!("matching MSG_NEW_GAME consumes the campaign descriptor");
        };
        assert_ne!(delivered.generation, 0);
        assert_eq!(
            delivered.player_template_name.as_deref(),
            Some("FactionChinaTankGeneral")
        );
    }

    #[test]
    fn mismatched_new_game_discards_stale_campaign_selection() {
        let _guard = acquire_host_campaign_launch_bridge_test_guard();
        set_host_campaign_launch_bridge_enabled(true);
        assert!(publish_host_campaign_launch(descriptor()));
        assert_eq!(
            take_host_campaign_launch_for_new_game(2, 2, 300, Some(30)),
            HostCampaignLaunchDelivery::Mismatched
        );
        assert_eq!(
            take_host_campaign_launch_for_new_game(0, 2, 300, Some(30)),
            HostCampaignLaunchDelivery::None
        );
    }

    #[test]
    fn launch_transport_preserves_present_zero_unsigned_and_absent_cash() {
        // This bridge belongs to the current GUI thread; this fixture needs
        // no process-global state or serialization with other GUI threads.
        let _guard = acquire_host_campaign_launch_bridge_test_guard();
        set_host_campaign_launch_bridge_enabled(true);
        for cash in [Some(0), Some(12_500), Some(u32::MAX), None] {
            let mut source = descriptor();
            source.is_challenge = cash.is_some();
            source.game_info_starting_cash = cash;
            assert!(publish_host_campaign_launch(source.clone()));
            source.game_info_starting_cash = Some(91);
            assert_eq!(source.game_info_starting_cash, Some(91));
            let HostCampaignLaunchDelivery::Matched(delivered) =
                take_host_campaign_launch_for_new_game(0, 2, 300, Some(30))
            else {
                panic!("matching NewGame must consume the owned launch input");
            };
            assert_eq!(delivered.game_info_starting_cash, cash);
        }
    }
}
