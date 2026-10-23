//! C++ Player.cpp792-819 controller admission from frozen map definitions.
//! This is definition input for an admission, never an active player selector.
use crate::game_logic::Player;
use game_engine::common::well_known_keys::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ControllerClass {
    pub(crate) human: bool,
    pub(crate) skirmish: bool,
}
impl ControllerClass {
    pub(crate) fn ordinary(human: bool) -> Self {
        Self {
            human,
            skirmish: false,
        }
    }
    pub(crate) fn apply(self, player: &mut Player) {
        player.is_human = self.human;
        player.map_side.is_skirmish_ai = self.skirmish;
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedControllers {
    classes: HashMap<String, ControllerClass>,
}
impl PreparedControllers {
    pub(crate) fn capture(
        sides: &gamelogic::sides_list::SidesList,
        template_sides: &HashMap<String, String>,
    ) -> Self {
        let source_sides: Vec<&str> = (0..sides.get_num_skirmish_sides())
            .filter_map(|index| sides.get_skirmish_side_info(index))
            .filter_map(|source| {
                template_sides.get(&source.get_dict().get_ascii_string(key_player_faction()))
            })
            .map(String::as_str)
            .collect();
        let classes = (0..sides.get_num_sides())
            .filter_map(|index| {
                let row = sides.get_side_info(index)?.get_dict();
                let name = row.get_ascii_string(key_player_name());
                if name.is_empty() {
                    return None;
                }
                let requested = row.get_bool(key_player_is_skirmish());
                let matches_source = template_sides
                    .get(&row.get_ascii_string(key_player_faction()))
                    .is_some_and(|side| source_sides.contains(&side.as_str()));
                // Original malformed requested-skirmish fallback changes the real
                // Player category to Human, not merely its AI dispatch bit.
                let human = row.get_bool(key_player_is_human()) || (requested && !matches_source);
                Some((
                    name,
                    ControllerClass {
                        human,
                        skirmish: requested && matches_source && !human,
                    },
                ))
            })
            .collect();
        Self { classes }
    }
    pub(crate) fn for_side(&self, name: &str) -> Option<ControllerClass> {
        self.classes.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, GameMode, Player, Team};
    use game_engine::common::dict::Dict;

    fn world(catalog: &HashMap<String, String>) -> GameLogic {
        let mut world = GameLogic::new();
        world.prepare_native_command_mode(GameMode::Skirmish);
        world.add_player(Player::new(1, Team::USA, "Owner", false).with_script_identity("Owner"));
        let mut source = Dict::new();
        source.set_ascii_string(key_player_name(), "SourceAmerica");
        source.set_ascii_string(key_player_faction(), "SourceFaction");
        world.sync_legacy_sides_list_from_dicts_with_controller_catalog(
            &[source],
            &[],
            &[],
            &[],
            catalog,
        );
        world
    }
    fn catalog(source: &str) -> HashMap<String, String> {
        [
            ("FactionAmerica".into(), "America".into()),
            ("SourceFaction".into(), source.into()),
            ("FactionCivilian".into(), "Civilian".into()),
        ]
        .into_iter()
        .collect()
    }
    #[test]
    fn actual_side_admission_captures_class_and_force_human_before_native_ai_reads() {
        let mut definitions = catalog("America");
        let owner = world(&definitions);
        definitions.insert("SourceFaction".into(), "China".into());
        let mut peer = world(&definitions);
        assert!(!owner.get_player(1).unwrap().is_human);
        assert!(owner.get_player(1).unwrap().map_side.is_skirmish_ai);
        assert!(
            peer.get_player(1).unwrap().is_human,
            "CPP missing matching skirmish source forces the real Player category"
        );
        assert!(!peer.get_player(1).unwrap().map_side.is_skirmish_ai);
        peer.reset();
        assert!(owner.get_player(1).unwrap().map_side.is_skirmish_ai);
        assert!(!owner.get_player(1).unwrap().is_human);
    }
    #[test]
    fn encoded_restore_preserves_admitted_map_class_after_foreign_definition_override() {
        use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
        let definitions = catalog("America");
        let owner = world(&definitions);
        let builder = SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&owner).unwrap();
        let encoded = bincode_legacy::serialize(&saved).unwrap();
        let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
        // Actual native restore prepares the saved map with its receiving
        // immutable definitions before applying the mutable continuation.
        let mut receiving = world(&definitions);
        let mut other_definitions = catalog("China");
        let mut peer = world(&other_definitions);
        other_definitions.clear();
        builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap();
        peer.reset();
        assert!(receiving.get_player(1).unwrap().map_side.is_skirmish_ai);
        assert!(!receiving.get_player(1).unwrap().is_human);
        assert!(owner.get_player(1).unwrap().map_side.is_skirmish_ai);
    }
}
