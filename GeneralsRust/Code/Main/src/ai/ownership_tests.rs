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

#[test]
fn strategic_ai_queries_and_repair_requests_use_the_driving_world() {
    // AIPlayer.cpp repairStructure consults the actual object body and appends only
    // a damaged body's ID. Identical IDs in another world must not answer that query.
    let player_id = 934204;
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    for world in [&mut first, &mut second] {
        let mut player = Player::new(player_id, Team::USA, "AI", false);
        player.resources.supplies = 10_000;
        world.add_player(player);
        let mut template = ThingTemplate::new("OwnedRepairPlan");
        template.add_kind_of(KindOf::Structure);
        world.templates.insert(template.name.clone(), template);
    }
    let first_id = first
        .create_object("OwnedRepairPlan", Team::USA, Vec3::ZERO)
        .unwrap();
    let second_id = second
        .create_object("OwnedRepairPlan", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(first_id, second_id);
    first.host_object_mut(first_id).unwrap().health.current = 20.0;
    first.host_object_mut(first_id).unwrap().health.maximum = 100.0;
    first.host_object_mut(first_id).unwrap().body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
    second.host_object_mut(second_id).unwrap().health.current = 100.0;
    second.host_object_mut(second_id).unwrap().health.maximum = 100.0;

    let mut first_ai = AIPlayer::new(player_id, Team::USA, AIDifficulty::Medium);
    let mut second_ai = AIPlayer::new(player_id, Team::USA, AIDifficulty::Medium);
    first_ai.repair_structure(&first, first_id);
    second_ai.repair_structure(&second, second_id);
    assert_eq!(first_ai.structures_to_repair, vec![first_id]);
    assert!(second_ai.structures_to_repair.is_empty());

    // A new borrowed decision view sees prior synchronous changes, not a stale snapshot.
    second.host_object_mut(second_id).unwrap().health.current = 40.0;
    second.host_object_mut(second_id).unwrap().body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged;
    second_ai.repair_structure(&second, second_id);
    assert_eq!(second_ai.structures_to_repair, vec![second_id]);
    assert_eq!(first.host_object(first_id).unwrap().health.current, 20.0);
    first.reset();
    first_ai.structures_to_repair.clear();
    first_ai.repair_structure(&first, first_id);
    assert!(first_ai.structures_to_repair.is_empty());
    assert_eq!(second.host_object(second_id).unwrap().health.current, 40.0);
}
