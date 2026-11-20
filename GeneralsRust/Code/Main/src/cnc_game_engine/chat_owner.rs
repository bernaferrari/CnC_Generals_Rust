//! InGameChat.cpp reads from the receiving native match, never the Core roster.
use super::*;
#[cfg(feature = "game_client")]
impl CnCGameEngine {
    pub(super) fn receiving_chat_world_facts(
        &self,
    ) -> Option<game_client::gui::callbacks::ingame_callbacks::ChatWorldFacts> {
        // Only the roster's multiplayer flag is read here; building the
        // full roster counted every object on each menu-facts query.
        chat_world_facts_from_world(&self.game_logic, self.receiving_observer_multiplayer())
    }
}
#[cfg(feature = "game_client")]
pub(super) fn chat_world_facts_from_world(
    world: &crate::game_logic::GameLogic,
    multiplayer: bool,
) -> Option<game_client::gui::callbacks::ingame_callbacks::ChatWorldFacts> {
    use game_client::gui::callbacks::ingame_callbacks::{ChatSlotFacts, ChatWorldFacts};
    let local_id = world.local_player_id()?;
    let local = world.get_players().get(&local_id)?;
    let admission = world.initial_skirmish_admission.as_deref();
    let source_info = admission.and_then(|admission| admission.source.as_ref());
    let admitted_local_slot = admission
        .and_then(|admission| admission.local_slot)
        .and_then(|slot| u8::try_from(slot).ok())
        .filter(|slot| *slot < 8);
    let mut slots = [None; 8];
    // C++ InGameChat.cpp:198-223 resolves named player0..player7.
    // Display names, faction, raw player ID and hash-map order are not slots.
    for player in world.get_players().values() {
        let Some(suffix) = player.map_side.map_player_name.strip_prefix("player") else {
            continue;
        };
        let [digit @ b'0'..=b'7'] = suffix.as_bytes() else {
            continue;
        };
        let slot = usize::from(*digit - b'0');
        if slots[slot].is_some() {
            continue;
        }
        let is_local = player.id == local_id;
        let mutually_allied = world.player_relationship(local_id, player.id)
            == gamelogic::common::Relationship::Allies
            && world.player_relationship(player.id, local_id)
                == gamelogic::common::Relationship::Allies;
        // A source GameInfo retains its exact slot mute. Direct non-network
        // admissions have no mutable lobby; GameSlot.cpp's constructor is unmuted.
        let muted = source_info.is_some_and(|info| info.slots[slot].is_muted);
        slots[slot] = Some(ChatSlotFacts {
            is_local,
            mutually_allied,
            muted,
        });
    }
    let local_slot = admitted_local_slot.or_else(|| {
        slots
            .iter()
            .position(|slot| slot.is_some_and(|facts| facts.is_local))
            .map(|slot| slot as u8)
    });
    Some(ChatWorldFacts {
        multiplayer,
        local_player_active: local.is_alive && !local.is_observer,
        local_slot,
        slots,
    })
}
#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Team};
    use game_client::gui::callbacks::ingame_callbacks::InGameChatType;
    #[test]
    fn named_slots_and_relationships_come_from_each_actual_world() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        for (world, active) in [(&mut owner, true), (&mut peer, false)] {
            let mut local = Player::new(17, Team::USA, "arbitrary display", true);
            local.is_alive = active;
            local.map_side.map_player_name = "player3".into();
            let mut other = Player::new(91, Team::China, "same display", false);
            other.map_side.map_player_name = "player0".into();
            local.set_map_relationship(91, gamelogic::common::Relationship::Allies);
            other.set_map_relationship(
                17,
                if active {
                    gamelogic::common::Relationship::Allies
                } else {
                    gamelogic::common::Relationship::Enemies
                },
            );
            world.add_player(local);
            world.add_player(other);
            for (id, name) in [(92, "Player1"), (93, "player06"), (94, "player8")] {
                let mut decoy = Player::new(id, Team::USA, "display", false);
                decoy.map_side.map_player_name = name.into();
                world.add_player(decoy);
            }
        }
        let first = chat_world_facts_from_world(&owner, true).unwrap();
        let second = chat_world_facts_from_world(&peer, true).unwrap();
        assert_eq!(first.local_slot, Some(3));
        assert_eq!(second.local_slot, Some(3));
        assert!(first.local_player_active);
        assert!(!second.local_player_active);
        assert_eq!(
            first.slots.map(|slot| slot.is_some()),
            [true, false, false, true, false, false, false, false]
        );
        assert_eq!(first.player_mask(&InGameChatType::Allies), (1 << 3) | 1);
        assert_eq!(second.player_mask(&InGameChatType::Allies), 1 << 3);
        peer.reset();
        assert_eq!(chat_world_facts_from_world(&owner, true), Some(first));
    }
}
