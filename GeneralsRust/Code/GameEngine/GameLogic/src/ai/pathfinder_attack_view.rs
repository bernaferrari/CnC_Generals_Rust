//! C++ AIPathfind.cpp:9278-9429 — attack LOS against this Pathfinder's cells.

use super::*;
use crate::ai::pathfind_astar::{GridCoord, PathfindLayerEnum};

impl Pathfinder {
    /// The caller supplies its AI rule and the actual terrain/weapon LOS query.
    /// Terrain is queried only at the original C++ point, before cell traversal.
    pub(crate) fn is_attack_view_blocked_with_observations(
        &self,
        attacker: &Object,
        attacker_pos: &Coord3D,
        victim: Option<&Object>,
        victim_pos: &Coord3D,
        attack_uses_los: bool,
        clear_terrain: &mut dyn FnMut() -> Result<bool, String>,
    ) -> Result<bool, String> {
        resolve_attack_view(
            attack_uses_los,
            attacker.is_kind_of(KindOf::AttackNeedsLineOfSight),
            attacker.is_kind_of(KindOf::Immobile),
            attacker.get_current_weapon().is_some(),
            clear_terrain,
            || {
                let trace = AttackViewTrace {
                    attacker: attacker.get_id(),
                    attacker_layer: PathfindLayerEnum::from_u32(attacker.get_layer() as u32),
                    attacker_container: attacker.get_contained_by().unwrap_or(INVALID_ID),
                    attacker_slaver: Self::object_slaver_id(attacker),
                    victim: victim.map_or(INVALID_ID, Object::get_id),
                    victim_layer: victim.map_or(PathfindLayerEnum::Ground, |victim| {
                        PathfindLayerEnum::from_u32(victim.get_layer() as u32)
                    }),
                    victim_slaver: victim.map_or(INVALID_ID, Self::object_slaver_id),
                };
                self.trace_attack_view(&trace, attacker_pos, victim_pos)
            },
        )
    }

    fn trace_attack_view(
        &self,
        trace: &AttackViewTrace,
        attacker_pos: &Coord3D,
        victim_pos: &Coord3D,
    ) -> bool {
        // C++ captures the victim cell before substituting an elevated attacker
        // layer for a ground victim. Keep the two cell observations distinct.
        let victim_cell_obstacle = self
            .inner
            .pathfinder
            .obstacle_observation_at(GridCoord::from_world(victim_pos), trace.victim_layer)
            .and_then(|(_, obstacle, _)| obstacle);
        let mut layer = trace.victim_layer;
        let mut skip_count = 0;
        if trace.attacker_layer != PathfindLayerEnum::Ground {
            skip_count = 3;
            if layer == PathfindLayerEnum::Ground {
                layer = trace.attacker_layer;
            }
        }

        self.inner.iterate_cells_along_line_world(
            attacker_pos,
            victim_pos,
            layer,
            |_from, to, _x, _y| {
                if skip_count > 0 {
                    skip_count -= 1;
                    return 0;
                }
                let Some((cell_type, obstacle, transparent)) =
                    self.inner.pathfinder.obstacle_observation_at(to, layer)
                else {
                    return 0;
                };
                if cell_type != PathfindCellType::Obstacle {
                    return 0;
                }
                // PathfindCell::isObstaclePresent never matches INVALID_ID.
                let present = |id| id != INVALID_ID && obstacle == Some(id);
                if present(trace.attacker)
                    || present(trace.victim)
                    || present(trace.victim_slaver)
                    || present(trace.attacker_container)
                    || present(trace.attacker_slaver)
                    || transparent
                    || victim_cell_obstacle.is_some_and(present)
                {
                    return 0;
                }
                // The original TODO does not ignore the victim's container.
                1
            },
        ) != 0
    }
}

// Preserve the original short-circuit order without eager native terrain work.
fn resolve_attack_view(
    uses_los: bool,
    needs_los: bool,
    immobile: bool,
    has_weapon: bool,
    clear_terrain: &mut dyn FnMut() -> Result<bool, String>,
    trace: impl FnOnce() -> bool,
) -> Result<bool, String> {
    if !uses_los || !needs_los {
        return Ok(false);
    }
    if !immobile && has_weapon && !clear_terrain()? {
        return Ok(true);
    }
    Ok(trace())
}

struct AttackViewTrace {
    attacker: ObjectID,
    attacker_layer: PathfindLayerEnum,
    attacker_container: ObjectID,
    attacker_slaver: ObjectID,
    victim: ObjectID,
    victim_layer: PathfindLayerEnum,
    victim_slaver: ObjectID,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Pathfinder {
        let mut grid = Pathfinder::new();
        grid.reset_with_size(12, 3);
        grid.inner.new_map();
        grid
    }

    fn trace() -> AttackViewTrace {
        AttackViewTrace {
            attacker: 10,
            attacker_layer: PathfindLayerEnum::Ground,
            attacker_container: 11,
            attacker_slaver: 12,
            victim: 20,
            victim_layer: PathfindLayerEnum::Ground,
            victim_slaver: 21,
        }
    }

    fn position(x: i32) -> Coord3D {
        GridCoord::new(x, 1).to_world(0.0)
    }

    #[test]
    fn attack_view_short_circuits_before_native_terrain_and_grid() {
        for (uses_los, needs_los) in [(false, true), (true, false)] {
            assert_eq!(
                resolve_attack_view(
                    uses_los,
                    needs_los,
                    false,
                    true,
                    &mut || panic!("terrain must not execute"),
                    || panic!("grid must not execute"),
                ),
                Ok(false),
            );
        }
        assert_eq!(
            resolve_attack_view(true, true, false, true, &mut || Ok(false), || {
                panic!("blocked terrain must precede grid")
            }),
            Ok(true),
        );
        assert_eq!(
            resolve_attack_view(
                true,
                true,
                false,
                true,
                &mut || Err("terrain unavailable".into()),
                || panic!("failed terrain must precede grid"),
            ),
            Err("terrain unavailable".into()),
        );
    }

    #[test]
    fn attack_view_immobile_and_weaponless_skip_only_terrain() {
        for (immobile, has_weapon) in [(true, true), (false, false)] {
            assert_eq!(
                resolve_attack_view(
                    true,
                    true,
                    immobile,
                    has_weapon,
                    &mut || panic!("C++ has no terrain query here"),
                    || true,
                ),
                Ok(true),
            );
        }
    }

    #[test]
    fn attack_view_uses_actual_grid_obstacles_and_cpp_identity_exclusions() {
        let mut grid = grid();
        let trace = trace();
        let cell = GridCoord::new(5, 1);
        for ignored in [10, 11, 12, 20, 21] {
            grid.inner
                .pathfinder
                .set_cell_obstacle_id(cell, ignored, false, false);
            assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
        }
        // A victim container is not one of C++'s ignored identities.
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(cell, 22, false, false);
        assert!(grid.trace_attack_view(&trace, &position(0), &position(10)));
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(cell, 22, false, true);
        assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(cell, 22, false, false);
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(GridCoord::new(10, 1), 22, false, false);
        assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
    }

    #[test]
    fn attack_view_elevated_skip_three_uses_selected_cell_and_ground_fallback() {
        let mut grid = grid();
        let mut trace = trace();
        trace.attacker_layer = PathfindLayerEnum::Top;
        let early = GridCoord::new(2, 1);
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(early, 99, false, false);
        assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
        let later = GridCoord::new(5, 1);
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(later, 99, false, false);
        assert!(grid.trace_attack_view(&trace, &position(0), &position(10)));
        grid.inner.pathfinder.set_cell_obstacle_id_on_layer(
            later,
            PathfindLayerEnum::Top,
            trace.attacker,
            false,
            false,
        );
        assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
        grid.inner.pathfinder.set_cell_type_on_layer(
            later,
            PathfindLayerEnum::Top,
            PathfindCellType::Impassable,
        );
        assert!(grid.trace_attack_view(&trace, &position(0), &position(10)));
    }

    #[test]
    fn attack_view_victim_cell_is_observed_before_attacker_layer_substitution() {
        let mut grid = grid();
        let mut trace = trace();
        trace.attacker_layer = PathfindLayerEnum::Top;
        grid.inner
            .pathfinder
            .set_cell_obstacle_id(GridCoord::new(10, 1), 77, false, false);
        grid.inner.pathfinder.set_cell_obstacle_id_on_layer(
            GridCoord::new(5, 1),
            PathfindLayerEnum::Top,
            77,
            false,
            false,
        );
        assert!(!grid.trace_attack_view(&trace, &position(0), &position(10)));
        grid.inner.pathfinder.set_cell_obstacle_id_on_layer(
            GridCoord::new(10, 1),
            PathfindLayerEnum::Top,
            88,
            false,
            false,
        );
        assert!(grid.trace_attack_view(&trace, &position(0), &position(10)));
    }
}
