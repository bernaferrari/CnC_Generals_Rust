//! Renderer-independent values for HeightMap's selected and revealed paths.
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId};
use game_engine::common::game_common::Relationship;
use glam::Vec3;

/// W3dWaypointBuffer.cpp:61,177–184. Every goal receives a node.
pub(crate) const MAX_RIBBON_GOALS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaypointPathStyle {
    Selected,
    RevealedEnemy,
}

impl WaypointPathStyle {
    pub(crate) fn width(self) -> f32 {
        match self {
            Self::Selected => 1.5,
            Self::RevealedEnemy => 3.0,
        }
    }

    pub(crate) fn rgb(self) -> [f32; 3] {
        match self {
            Self::Selected => [0.25, 0.5, 1.0],
            Self::RevealedEnemy => [0.95, 0.5, 0.0],
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct WaypointOverlayPath {
    pub(crate) origin: Vec3,
    pub(crate) goals: Vec<Vec3>,
    pub(crate) style: WaypointPathStyle,
}

impl WaypointOverlayPath {
    pub(crate) fn ribbon_points(&self) -> impl Iterator<Item = Vec3> + '_ {
        std::iter::once(self.origin).chain(self.goals.iter().take(MAX_RIBBON_GOALS).copied())
    }
}

fn active_goals(object: &Object) -> Option<&[Vec3]> {
    if !object.has_active_selected_follow_path() {
        return None;
    }
    let runtime = &object.unit_ai_runtime;
    let index = runtime.follow_path_index()? as usize;
    let goals = runtime.machine().goals.path.get(index..)?;
    (!goals.is_empty()).then_some(goals)
}

/// Capture only the paths consumed this draw. The presentation input already
/// uses Main's Y-up coordinates, and the local selection is the C++ push-front list.
pub(crate) fn capture(
    world: &GameLogic,
    in_waypoint_mode: bool,
    hovered_actor: Option<ObjectId>,
) -> Vec<WaypointOverlayPath> {
    if in_waypoint_mode {
        world
            .selected_objects
            .iter()
            .filter_map(|id| world.host_object(*id))
            .filter(|object| {
                object.has_ai_update_interface() && !object.is_kind_of(KindOf::IgnoredInGui)
            })
            .filter_map(|object| {
                Some(WaypointOverlayPath {
                    origin: object.get_position(),
                    goals: active_goals(object)?.to_vec(),
                    style: WaypointPathStyle::Selected,
                })
            })
            .collect()
    } else {
        revealed_enemy_path(world, hovered_actor)
            .into_iter()
            .collect()
    }
}

fn revealed_enemy_path(
    world: &GameLogic,
    hovered_actor: Option<ObjectId>,
) -> Option<WaypointOverlayPath> {
    let enemy = world.host_object(hovered_actor?)?;
    // The original breaks at the first locally controlled revealer, even if
    // that revealer cannot see the enemy. A later outpost may not take over.
    let revealer = world
        .selected_objects
        .iter()
        .filter_map(|id| world.host_object(*id))
        .find(|object| {
            object.is_kind_of(KindOf::RevealsEnemyPaths)
                && world.is_object_locally_controlled(object.id)
        })?;
    let in_vision_range =
        enemy.get_position().distance(revealer.get_position()) <= revealer.vision_range;
    if !enemy.has_ai_update_interface()
        || world.object_relationship(enemy, revealer) != Relationship::Enemies
        || !in_vision_range
    {
        return None;
    }

    let goals = if let Some(path) = active_goals(enemy) {
        path.to_vec()
    } else {
        // AIUpdateInterface::getGoalPosition is the state-machine goal,
        // even while the locomotor follows a different local path node.
        let goal = enemy.unit_ai_runtime.machine().goals.position;
        (goal.length() > 1.0).then_some(vec![goal])?
    };
    Some(WaypointOverlayPath {
        origin: enemy.get_position(),
        goals,
        style: WaypointPathStyle::RevealedEnemy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};

    fn actor(
        world: &mut GameLogic,
        id: u32,
        team: Team,
        owner: u32,
        pos: Vec3,
        kinds: &[KindOf],
    ) -> ObjectId {
        let mut template = ThingTemplate::new("WaypointOverlayFixture");
        template.set_health(100.0);
        for kind in kinds {
            template.add_kind_of(*kind);
        }
        let id = ObjectId(id);
        let mut object = Object::new(template, id, team);
        object.owner_player_id = Some(owner);
        object.vision_range = 60.0;
        world.add_object(object);
        world.set_fixture_object_position(id, pos);
        id
    }

    #[test]
    fn selected_waypoint_overlay_uses_current_authored_suffix_and_caps_only_ribbon() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Local", true));
        let selected = actor(&mut world, 7, Team::USA, 0, Vec3::Y, &[KindOf::Vehicle]);
        let ignored = actor(
            &mut world,
            8,
            Team::USA,
            0,
            Vec3::ZERO,
            &[KindOf::Vehicle, KindOf::IgnoredInGui],
        );
        let goals: Vec<_> = (0..520)
            .map(|index| Vec3::new(20.0 + index as f32, 6.0, 5.0))
            .collect();
        world
            .host_object_mut(selected)
            .unwrap()
            .unit_ai_runtime
            .start_follow_path(goals.clone());
        let runtime = &mut world.host_object_mut(selected).unwrap().unit_ai_runtime;
        runtime
            .consume_follow_path_move_result(Vec3::ZERO, false)
            .unwrap();
        world
            .host_object_mut(ignored)
            .unwrap()
            .unit_ai_runtime
            .start_follow_path(vec![Vec3::X]);
        world.selected_objects = vec![ignored, selected];

        let mut peer = GameLogic::new();
        peer.add_player(Player::new(0, Team::USA, "Peer", true));
        let peer_selected = actor(&mut peer, 7, Team::USA, 0, Vec3::Z, &[KindOf::Vehicle]);
        assert_eq!(peer_selected, selected);
        let peer_goal = Vec3::new(9.0, 5.0, 42.0);
        peer.host_object_mut(peer_selected)
            .unwrap()
            .unit_ai_runtime
            .start_follow_path(vec![peer_goal]);
        peer.selected_objects = vec![peer_selected];

        let paths = peer.with_own_services(|| capture(&world, true, None));
        let [path] = paths.as_slice() else {
            panic!("exactly one selected path");
        };
        assert_eq!(path.style, WaypointPathStyle::Selected);
        assert_eq!(path.origin, Vec3::Y);
        assert_eq!(path.goals, goals[1..]);
        let ribbon: Vec<_> = path.ribbon_points().collect();
        assert_eq!(ribbon.len(), 513);
        assert_eq!(ribbon[512], goals[512]);
        assert_eq!(path.goals.last(), goals.last());
        assert_eq!(capture(&peer, true, None)[0].goals, vec![peer_goal]);
    }

    #[test]
    fn revealed_waypoint_overlay_uses_first_local_revealer_and_enemy_machine_goal() {
        let mut world = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "Local", true);
        local.alliance_team = 0;
        let mut enemy_player = Player::new(1, Team::China, "Enemy", false);
        enemy_player.alliance_team = 1;
        world.add_player(local);
        world.add_player(enemy_player);
        assert!(world.select_local_player(Some(0)));
        let near = actor(
            &mut world,
            7,
            Team::USA,
            0,
            Vec3::ZERO,
            &[KindOf::Structure, KindOf::RevealsEnemyPaths],
        );
        let high = actor(
            &mut world,
            8,
            Team::USA,
            0,
            Vec3::new(0.0, 100.0, 0.0),
            &[KindOf::Structure, KindOf::RevealsEnemyPaths],
        );
        let enemy = actor(&mut world, 9, Team::China, 1, Vec3::X, &[KindOf::Vehicle]);
        let machine_goal = Vec3::new(64.0, 3.0, 49.0);
        world
            .host_object_mut(enemy)
            .unwrap()
            .unit_ai_runtime
            .set_machine_goal_position(machine_goal);

        world.selected_objects = vec![high, near];
        assert!(capture(&world, false, Some(enemy)).is_empty());
        world.selected_objects = vec![near, high];
        let paths = capture(&world, false, Some(enemy));
        assert_eq!(
            paths,
            vec![WaypointOverlayPath {
                origin: Vec3::X,
                goals: vec![machine_goal],
                style: WaypointPathStyle::RevealedEnemy,
            }]
        );
        assert!(capture(&world, true, Some(enemy)).is_empty());
    }
}
