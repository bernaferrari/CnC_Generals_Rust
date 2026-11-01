//! Count each receiving world's score-visible objects once for observer UI.
use crate::game_logic::GameLogic;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ObserverCounts {
    pub units: i32,
    pub buildings: i32,
}

pub(crate) fn count_observer_objects(world: &GameLogic) -> HashMap<u32, ObserverCounts> {
    const SCORE: u128 = 1u128 << 35;
    const STRUCTURE: u128 = 1u128 << 7;
    const SCORE_CREATE: u128 = 1u128 << 36;
    const SCORE_DESTROY: u128 = 1u128 << 37;

    let mut result = HashMap::<u32, ObserverCounts>::new();
    for object in world.get_objects().registered_values() {
        let Some(owner) = object
            .owner_player_id
            .filter(|id| world.get_player(*id).is_some())
        else {
            continue;
        };
        let counts = result.entry(owner).or_default();
        counts.units += i32::from(object.is_kind_of_multi(SCORE, STRUCTURE));
        // These are intentionally independent: C++ adds the three queries,
        // including objects that have multiple scoring flags.
        counts.buildings += i32::from(object.is_kind_of_multi(SCORE | STRUCTURE, 0));
        counts.buildings += i32::from(object.is_kind_of_multi(SCORE_CREATE | STRUCTURE, 0));
        counts.buildings += i32::from(object.is_kind_of_multi(SCORE_DESTROY | STRUCTURE, 0));
    }
    result
}
