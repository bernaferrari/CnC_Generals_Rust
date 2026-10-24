//! Production/placement policy observes only the receiving Player and science catalog.
use super::*;
use game_engine::common::rts::science::{
    SCIENCE_INVALID, ScienceInfo, ScienceNameFacts, ScienceStore,
};
use game_engine::common::thing::{
    module_factory::ModuleFactory, receiving_module_content::ReceivingModuleContent,
};
use std::sync::{Arc, RwLock};

fn world(science_name: &str, human: bool) -> (GameLogic, game_engine::common::rts::ScienceType) {
    let mut definitions = ScienceStore::new();
    definitions.add_science(ScienceInfo::new(701, science_name));
    let facts = Arc::new(ScienceNameFacts::from_store(&definitions));
    let raw = facts.science(science_name).unwrap();
    let mut modules = ModuleFactory::new();
    modules.replace_receiving_science_facts(facts);
    let mut world = GameLogic::new();
    world.receiving_module_content = Some(ReceivingModuleContent::new(modules));
    let mut player = Player::new(0, Team::USA, "player0", human);
    player.is_local = !human; // locality cannot substitute for the controller type.
    world.add_player(player);
    (world, raw)
}

#[test]
fn receiving_human_policy_never_uses_foreign_same_slot_or_local_selection() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_human_policy_never_uses_foreign_same_slot_or_local_selection",
        || {
            let (owner, _) = world("SCIENCE_OwnerPolicy", false);
            let (peer, _) = world("SCIENCE_PeerPolicy", true);
            let mut foreign = gamelogic::player::Player::new(0);
            foreign.set_player_name_key(
                game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("player0"),
            );
            foreign.set_player_type(gamelogic::player::PlayerType::Human, false);
            let mut list = gamelogic::player::PlayerList::new();
            list.add_player(Arc::new(RwLock::new(foreign)));
            *gamelogic::player::player_list().write().unwrap() = list;
            assert!(!owner.player_is_human(0));
            assert!(peer.player_is_human(0));
            assert!(!owner.player_is_human(99), "a foreign-only slot is absent");
        },
    );
}

#[test]
fn prerequisite_science_uses_receiving_raw_name_pair_and_membership_even_with_foreign_grant() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "prerequisite_science_uses_receiving_raw_name_pair_and_membership_even_with_foreign_grant",
        || {
            let (mut owner, raw) = world("SCIENCE_OwnerPolicy", false);
            let (mut peer, peer_raw) = world("SCIENCE_PeerPolicy", false);
            assert_eq!(raw, peer_raw);
            peer.get_player_mut(0)
                .unwrap()
                .unlocked_sciences
                .insert("SCIENCE_PeerPolicy".into());
            let mut foreign = gamelogic::player::Player::new(0);
            foreign.set_player_name_key(
                game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("player0"),
            );
            assert!(foreign.add_science(raw));
            let mut list = gamelogic::player::PlayerList::new();
            list.add_player(Arc::new(RwLock::new(foreign)));
            *gamelogic::player::player_list().write().unwrap() = list;
            assert!(!owner.host_player_has_prereq_science(Some(0), Some(Team::USA), raw));
            assert!(peer.host_player_has_prereq_science(Some(0), Some(Team::USA), raw));
            owner
                .get_player_mut(0)
                .unwrap()
                .unlocked_sciences
                .insert("SCIENCE_OwnerPolicy".into());
            assert!(owner.host_player_has_prereq_science(Some(0), Some(Team::USA), raw));
            assert!(
                !owner.host_player_has_prereq_science(Some(99), Some(Team::USA), raw),
                "explicit missing owner cannot select a faction peer"
            );
            owner.receiving_module_content = None;
            assert!(
                !owner.host_player_has_prereq_science(Some(0), Some(Team::USA), raw),
                "missing own catalog is authoritative"
            );
            assert!(
                owner.host_player_has_prereq_science(Some(99), None, SCIENCE_INVALID),
                "original no-prerequisite sentinel is unconditional"
            );
        },
    );
}
