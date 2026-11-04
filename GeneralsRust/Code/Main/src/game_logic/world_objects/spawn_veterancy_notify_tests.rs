//! C++ SpawnBehavior::computeAggregateStates (SpawnBehavior.cpp:855-897):
//! `vetLevel` is captured once; each spawn in m_spawnIDs order is compared to
//! that captured rank and the exact `setVeterancyLevel` (provideFeedback=TRUE)
//! notifies whichever side is lower.
use crate::game_logic::{GameLogic, ObjectId, Player, Team, VeterancyLevel};
use glam::Vec3;

fn session() -> (GameLogic, ObjectId, Vec<ObjectId>) {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([("SpawnVet.ini".into(),
        "Object Nest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = Child\n End\nEnd\nObject Child\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into())]).unwrap();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    let parent = world
        .create_object_for_player("Nest", 1, Vec3::ZERO)
        .unwrap();
    let children: Vec<ObjectId> = world
        .host_object(parent)
        .unwrap()
        .spawn_behavior_children()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(children.len(), 2);
    (world, parent, children)
}

fn level(world: &GameLogic, id: ObjectId) -> VeterancyLevel {
    world.host_object(id).unwrap().experience.level
}

#[test]
fn spawn_veterancy_compares_each_child_to_captured_parent_rank_like_cpp() {
    let (mut world, parent, children) = session();
    world
        .objects
        .get_mut(&children[0])
        .unwrap()
        .experience
        .level = VeterancyLevel::Elite;
    world.sync_spawn_behavior_veterancy(parent, "Child");
    assert_eq!(level(&world, parent), VeterancyLevel::Elite);
    // Rookie child equals the captured Rookie vetLevel: untouched this pass.
    assert_eq!(level(&world, children[1]), VeterancyLevel::Rookie);
    // Next aggregate pass captures Elite and promotes the lagging child.
    world.sync_spawn_behavior_veterancy(parent, "Child");
    assert_eq!(level(&world, children[1]), VeterancyLevel::Elite);
}

#[test]
fn spawn_veterancy_uses_exact_setter_in_roster_order_like_cpp() {
    let (mut world, parent, children) = session();
    world
        .objects
        .get_mut(&children[0])
        .unwrap()
        .experience
        .level = VeterancyLevel::Elite;
    world
        .objects
        .get_mut(&children[1])
        .unwrap()
        .experience
        .level = VeterancyLevel::Veteran;
    world.sync_spawn_behavior_veterancy(parent, "Child");
    // Both exceed captured Rookie: Elite then exact setVeterancyLevel(Veteran).
    assert_eq!(level(&world, parent), VeterancyLevel::Veteran);
}
