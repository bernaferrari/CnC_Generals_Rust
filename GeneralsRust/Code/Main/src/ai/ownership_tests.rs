use super::*;
use std::sync::{Arc, RwLock};

struct RestorePlayerList(Option<gamelogic::player::PlayerList>);

impl Drop for RestorePlayerList {
    fn drop(&mut self) {
        *gamelogic::player::player_list()
            .write()
            .unwrap_or_else(|error| error.into_inner()) = self.0.take().unwrap();
    }
}

#[test]
fn ai_player_constructor_is_inert_and_owned_registration_preserves_cpp_permissions() {
    let player_id = 934203;
    let sentinel = Arc::new(RwLock::new(gamelogic::player::Player::new(
        player_id as i32,
    )));
    sentinel.write().unwrap().set_can_build_units(true);
    let mut isolated_list = gamelogic::player::PlayerList::new();
    isolated_list.add_player(Arc::clone(&sentinel));
    let original = std::mem::replace(
        &mut *gamelogic::player::player_list().write().unwrap(),
        isolated_list,
    );
    let _restore = RestorePlayerList(Some(original));

    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    first.add_player(Player::new(player_id, Team::USA, "First owned AI", false));
    second.add_player(Player::new(player_id, Team::GLA, "Second owned AI", false));
    let _candidate = AIPlayer::new(player_id, Team::USA, AIDifficulty::Medium);
    assert!(
        sentinel.read().unwrap().get_can_build_units(),
        "constructing host AI must not mutate ambient PlayerList"
    );
    assert!(first.get_player(player_id).unwrap().can_build_units);
    assert!(second.get_player(player_id).unwrap().can_build_units);

    // C++ AIPlayer base ctor disables production on its actual player.
    AIManager::apply_ctor_can_build_units(&mut first, player_id);
    assert!(!first.get_player(player_id).unwrap().can_build_units);
    assert!(second.get_player(player_id).unwrap().can_build_units);
    assert!(sentinel.read().unwrap().get_can_build_units());

    // Real skirmish registration runs base then subclass, re-enabling it.
    first.add_ai_opponent(player_id, Team::USA, AIDifficulty::Medium);
    assert!(first.get_player(player_id).unwrap().can_build_units);
    assert!(second.get_player(player_id).unwrap().can_build_units);
    assert!(sentinel.read().unwrap().get_can_build_units());
}
