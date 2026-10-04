use super::{Coord3D, Path, PathMovementContext, PathfindLayerEnum};
use crate::ai::pathfind_astar::{GridCoord, PathfindCellType};
use crate::ai::pathfind_complete::{PathRequest, SURFACE_GROUND};
use crate::common::INVALID_ID;

#[test]
fn actual_grid_path_producer_returns_forward_goal_and_remaining_distance() {
    let mut owner = crate::ai::Pathfinder::new();
    owner.reset_with_size(3, 1);
    for x in 0..3 {
        owner
            .inner
            .pathfinder
            .set_cell_type(GridCoord::new(x, 0), PathfindCellType::Clear);
    }
    owner.inner.new_map_from_classified_cells();
    let result = owner.find_path_result(PathRequest::new(
        Coord3D::new(5.0, 5.0, 0.0),
        Coord3D::new(25.0, 5.0, 0.0),
        SURFACE_GROUND,
    ));
    assert!(result.success, "actual grid producer admits open ground");
    let mut path = Path::new();
    for (position, layer) in result.waypoints.iter().zip(&result.layers) {
        path.append_node(position, PathfindLayerEnum::from_u32(*layer as u32));
    }
    // The producer has already optimized these waypoints. Install their links
    // in the single owned Path; this tests the explicit geometry contract and
    // does not claim that UnitAI's live locomotor adapter is installed.
    let keys = path.ordered_keys();
    for pair in keys.windows(2) {
        path.set_opti_link(pair[0], Some(pair[1]));
    }
    struct GroundContext<'a>(&'a crate::ai::pathfind_complete::PathfindingSystem);
    impl PathMovementContext for GroundContext<'_> {
        fn object_layer(&mut self) -> PathfindLayerEnum {
            PathfindLayerEnum::Ground
        }
        fn is_line_passable(
            &mut self,
            layer: PathfindLayerEnum,
            from: &Coord3D,
            to: &Coord3D,
        ) -> bool {
            assert_eq!(layer, PathfindLayerEnum::Ground);
            self.0.is_line_passable_for_object(
                INVALID_ID,
                from,
                to,
                SURFACE_GROUND,
                false,
                None,
                true,
                false,
                0.0,
            )
        }
        fn set_debug_path_position(&mut self, _position: &Coord3D) {}
    }
    let goal = path.compute_point_on_path(
        &Coord3D::new(10.0, 5.0, 0.0),
        &mut GroundContext(&owner.inner),
    );
    assert_eq!(goal.pos_on_path, Coord3D::new(25.0, 5.0, 0.0));
    assert!((goal.dist_along_path - 15.0).abs() < 0.001);
}
