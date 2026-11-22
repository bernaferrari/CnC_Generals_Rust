//! Resolve script side names and area aliases from the world running the script.
use super::HostScriptExecutionDriver;
use crate::game_logic::{GameLogic, GameMode, Player};
use game_engine::common::rts::science::{ScienceNameFacts, ScienceType};

impl HostScriptExecutionDriver<'_> {
    pub(super) fn player_script_name(player: &Player) -> &str {
        &player.map_side.map_player_name
    }

    pub(super) fn requires_prepared_player_bindings(&self) -> bool {
        // These paths still consume PlayerList side ordinals. The tech query
        // uses exact admitted Main Player names instead and does not infer IDs
        // from a side index.
        matches!(
            self.world.game_mode,
            GameMode::Skirmish
                | GameMode::Multiplayer
                | GameMode::Lan
                | GameMode::Internet
                | GameMode::Replay
        )
    }

    pub(in super::super) fn script_player_id(
        &self,
        token: &str,
        current_player: Option<&str>,
    ) -> Option<u32> {
        use gamelogic::scripting::core::{
            LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
        };
        match token {
            THIS_PLAYER => self.world.authored_script_player_id(current_player?),
            LOCAL_PLAYER => self.world.local_player_id(),
            THIS_PLAYER_ENEMY => {
                let current = self.world.authored_script_player_id(current_player?)?;
                self.world
                    .ai_manager
                    .ai_players
                    .get(&current)
                    .and_then(|ai| ai.enemy_player_id)
                    .filter(|id| self.world.players.contains_key(id))
                    .or_else(|| {
                        self.world
                            .players
                            .values()
                            .filter(|p| {
                                p.is_human
                                    && !(self.world.is_challenge_session()
                                        && Self::player_script_name(p) == THE_PLAYER)
                            })
                            .min_by_key(|p| p.id)
                            .map(|p| p.id)
                    })
            }
            // Retain the existing Challenge classification. Campaign metadata
            // ownership is separate from this query's player/geometry owner.
            THE_PLAYER if self.world.is_challenge_session() => self.world.local_player_id(),
            _ => self.world.authored_script_player_id(token),
        }
    }

    pub(super) fn query_script_player_id(
        &mut self,
        token: &str,
        current_player: Option<&str>,
    ) -> Option<u32> {
        if token == gamelogic::scripting::core::THIS_PLAYER_ENEMY
            && let Some(current) =
                current_player.and_then(|name| self.world.authored_script_player_id(name))
        {
            self.world.query_owned_ai_enemy(current);
        }
        self.script_player_id(token, current_player)
    }

    pub(super) fn query_condition_player_id(
        &mut self,
        token: &str,
        player_mask: u32,
        current_player: Option<&str>,
    ) -> Option<u32> {
        if player_mask == 0 {
            self.query_script_player_id(token, current_player)
        } else if player_mask.is_power_of_two() {
            Some(player_mask.trailing_zeros())
        } else {
            None
        }
    }

    pub(super) fn script_science_source(
        &self,
        player: &str,
        player_mask: u32,
        science: &str,
        current_player: Option<&str>,
    ) -> Option<(&Player, &ScienceNameFacts, ScienceType)> {
        // CPP playerFromParam resolves an existing single-player mask before
        // consulting the authored side name.
        let id = if player_mask == 0 {
            self.script_player_id(player, current_player)?
        } else if player_mask.is_power_of_two() {
            player_mask.trailing_zeros()
        } else {
            return None;
        };
        let player = self.world.get_player(id)?;
        let facts = self
            .world
            .receiving_module_content
            .as_ref()?
            .science_facts_ref();
        Some((player, facts, facts.science(science)?))
    }
}

impl GameLogic {
    fn authored_script_player_id(&self, name: &str) -> Option<u32> {
        // CPP Player::initFromDict stores displayName and playerName
        // separately; PlayerList::findPlayerWithNameKey selects the latter.
        if name.is_empty() {
            return None;
        }
        self.players
            .values()
            .find(|player| player.map_side.map_player_name == name)
            .map(|player| player.id)
    }

    pub(in super::super) fn qualified_script_area(
        &mut self,
        area: &str,
        current_player: Option<&str>,
    ) -> Option<String> {
        let (enemy, perimeter) = match area {
            "[Skirmish]MyInnerPerimeter" => (false, "InnerPerimeter"),
            "[Skirmish]MyOuterPerimeter" => (false, "OuterPerimeter"),
            "[Skirmish]EnemyInnerPerimeter" => (true, "InnerPerimeter"),
            "[Skirmish]EnemyOuterPerimeter" => (true, "OuterPerimeter"),
            _ => return Some(area.into()),
        };
        let current = current_player.and_then(|name| self.authored_script_player_id(name));
        let player = if enemy {
            // ScriptEngine.cpp:5908 uses Player::getCurrentEnemy here.
            // This query can advance the receiving AI's check deadline.
            current.and_then(|id| self.query_owned_ai_enemy(id))
        } else {
            Some(current?)
        };
        let index = player
            .and_then(|id| self.players.get(&id))
            .map(|p| p.start_position + 1);
        Some(format!("{perimeter}{}", index.unwrap_or(-1)))
    }
}
