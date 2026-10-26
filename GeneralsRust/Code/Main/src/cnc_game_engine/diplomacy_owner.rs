//! Immutable Diplomacy.cpp observations from this actual native match.
use super::*;
#[cfg(feature = "game_client")]
impl CnCGameEngine {
    pub(super) fn receiving_diplomacy_world_facts(
        &self,
    ) -> Result<game_client::gui::callbacks::diplomacy::DiplomacyWorldFacts, String> {
        let multiplayer = self
            .receiving_observer_roster()
            .ok_or("native recorder observation unavailable")?
            .multiplayer;
        use game_client::gui::callbacks::diplomacy::{
            DiplomacyWorldFacts, DiplomaticRelationship, PlayerInfo, PlayerStatus,
        };
        let chat = super::chat_owner::chat_world_facts_from_world(&self.game_logic, multiplayer)
            .ok_or("Diplomacy requires an admitted local player")?;
        let mut players = HashMap::new();
        for slot in 0..8 {
            let Some(facts) = chat.slots[slot] else {
                continue;
            };
            let name = format!("player{slot}");
            let player = self
                .game_logic
                .get_players()
                .values()
                .find(|p| p.map_side.map_player_name == name)
                .ok_or("named native player disappeared")?;
            let source = self
                .game_logic
                .initial_skirmish_admission
                .as_deref()
                .and_then(|a| a.source.as_ref())
                .map(|info| &info.slots[slot]);
            players.insert(
                slot as i32,
                PlayerInfo {
                    name: source
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| player.name.clone()),
                    side: format!("{:?}", player.team),
                    team: player.alliance_team,
                    status: if player.is_observer {
                        PlayerStatus::Observer
                    } else if player.is_alive {
                        PlayerStatus::Active
                    } else {
                        PlayerStatus::Defeated
                    },
                    relationship: if facts.is_local || facts.mutually_allied {
                        DiplomaticRelationship::Ally
                    } else {
                        DiplomaticRelationship::Neutral
                    },
                    is_muted: facts.muted,
                },
            );
        }
        Ok(DiplomacyWorldFacts {
            input_enabled: self.gameplay_ui_active(),
            intro_movie: self.game_logic.intro_movie_playing,
            loading_map: self.game_logic.is_loading_map(),
            multiplayer: multiplayer,
            local_slot: chat.local_slot.map(i32::from),
            players,
        })
    }
}
