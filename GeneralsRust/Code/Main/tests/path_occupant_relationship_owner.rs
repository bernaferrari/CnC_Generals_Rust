//! C++ checkForMovement / checkDestination resolve every occupant on each check
//! (`TheGameLogic->findObjectByID(posUnit)`) and ask the seeking object
//! `obj->getRelationship(unit)` (AIPathfind.cpp:4943-4948, 5009-5063).
//! Object::getRelationship applies the undetected-defector asymmetry before
//! any player or team relation (Object.cpp:1548-1568): a defecting source sees
//! NEUTRAL, a defecting target is seen as ALLIES.
use generals_main::game_logic::host_defection_helper::HostDefectionHelperData;
use generals_main::game_logic::pathfinding::{GridPos, PathPositionFootprint, PathfindingGrid};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

/// `3*COST_DIAGONAL`: one fixed ally in the footprint (AIPathfind.cpp:6278-6290).
const ALLY_FIXED_COST: u32 = 42;
/// The grid's enemyFixed sentinel.
const ENEMY_FIXED: u32 = u32::MAX / 8;

struct Session {
    world: GameLogic,
    grid: PathfindingGrid,
    seeker: ObjectId,
    occupant: ObjectId,
}

fn session() -> Session {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Seeker", true));
    world.add_player(Player::new(1, Team::GLA, "Occupant", false));
    let mut template = ThingTemplate::new("RelationshipActor");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("RelationshipActor".into(), template);
    let seeker = world
        .create_object_for_player("RelationshipActor", 0, Vec3::new(95.0, 0.0, 95.0))
        .unwrap();
    let occupant = world
        .create_object_for_player("RelationshipActor", 1, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    for id in [seeker, occupant] {
        let actor = world.host_object_mut(id).unwrap();
        actor.selection_radius = 1.0;
        actor.crushable_level = 255;
        actor.crusher_level = 0;
    }
    // The idle occupant owns its position and goal cell (updatePos/updateGoal).
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let position = world.host_object(occupant).unwrap().get_position();
    grid.update_ground_goal_cell(occupant.0, 1.0, false, (-1, -1), position);
    grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: occupant.0,
            radius: 1.0,
            layer: 0,
            on_ground: true,
        },
        (-1, -1),
        position,
    );
    Session {
        world,
        grid,
        seeker,
        occupant,
    }
}

impl Session {
    /// The occupied cell's movement cost as `observer` sees it right now.
    fn cost_seen_by(&mut self, observer: ObjectId) -> u32 {
        self.grid
            .bind_query_occupants(self.world.host_objects(), Some(observer));
        self.grid.occupancy_extra_cost(GridPos::new(5, 5), false)
    }

    fn set_defector(&mut self, id: ObjectId, on: bool) {
        let mut helper = HostDefectionHelperData::default();
        helper.set_undetected_defector(on);
        self.world.host_object_mut(id).unwrap().defection_helper = Some(helper);
    }
}

#[test]
fn undetected_defector_target_is_seen_as_ally_per_world_and_same_frame() {
    let mut defecting = session();
    let mut peer = session();
    assert_eq!(defecting.seeker, peer.seeker);
    assert_eq!(defecting.occupant, peer.occupant);
    assert_eq!(defecting.cost_seen_by(defecting.seeker), ENEMY_FIXED);

    let occupant = defecting.occupant;
    defecting.set_defector(occupant, true);
    // No grid event happened: the next check resolves the live object.
    assert_eq!(defecting.cost_seen_by(defecting.seeker), ALLY_FIXED_COST);
    assert_eq!(
        peer.cost_seen_by(peer.seeker),
        ENEMY_FIXED,
        "the same-ID peer world keeps its own relationship"
    );

    defecting.set_defector(occupant, false);
    assert_eq!(defecting.cost_seen_by(defecting.seeker), ENEMY_FIXED);
}

#[test]
fn undetected_defector_source_sees_neutral_not_ally() {
    let mut session = session();
    // Register the seeker's own fixed cell over the occupant's square so the
    // occupant, searching, observes the seeker there.
    let seeker = session.seeker;
    let occupant = session.occupant;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let position = Vec3::new(55.0, 0.0, 55.0);
    grid.update_ground_goal_cell(seeker.0, 1.0, false, (-1, -1), position);
    grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: seeker.0,
            radius: 1.0,
            layer: 0,
            on_ground: true,
        },
        (-1, -1),
        position,
    );
    session.grid = grid;
    // Players 0 and 1 share an alliance, so each sees the other as ALLIES.
    for player in [0, 1] {
        session.world.get_player_mut(player).unwrap().alliance_team = 7;
    }
    assert_eq!(session.cost_seen_by(occupant), ALLY_FIXED_COST);
    // A defecting source treats everyone as NEUTRAL (Object.cpp:1550-1553).
    session.set_defector(occupant, true);
    assert_eq!(session.cost_seen_by(occupant), ENEMY_FIXED);
    // The asymmetry: the ally still sees the defector as an ally.
    session.grid = {
        let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
        grid.update_ground_goal_cell(occupant.0, 1.0, false, (-1, -1), position);
        grid.update_path_position_cell(
            &PathPositionFootprint {
                unit_id: occupant.0,
                radius: 1.0,
                layer: 0,
                on_ground: true,
            },
            (-1, -1),
            position,
        );
        grid
    };
    assert_eq!(session.cost_seen_by(seeker), ALLY_FIXED_COST);
}
