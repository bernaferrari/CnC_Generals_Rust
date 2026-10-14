//! Real receiving-factory controls for PartitionManager's Dozer query.
use super::*;
use crate::common::GeometryInfo;
use crate::object::update::ai_update::dozer_ai_update::DozerAIUpdateModuleData;
use game_engine::common::system::GeometryType as EngineGeometryType;

#[derive(Debug)]
struct PositionTarget {
    base: DefaultThingTemplate,
    geometry: GeometryInfo,
}
impl ThingTemplate for PositionTarget {
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn get_name(&self) -> &crate::common::AsciiString {
        self.base.get_name()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.geometry.clone()
    }
    fn get_template_geometry_type(&self) -> Option<EngineGeometryType> {
        Some(self.geometry.get_geometry_type())
    }
}
fn world(target_x: f32) -> NativeMatch {
    let mut source = DefaultThingTemplate::new("PositionDozer".into());
    source.add_kind_of(KindOf::Vehicle);
    source.add_kind_of(KindOf::Dozer);
    let mut data = DozerAIUpdateModuleData::default();
    data.base.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    let _ = infantry_template();
    let source = Arc::new(EntryTemplate {
        base: source,
        modules: vec![TemplateModuleInfo {
            name: "DozerAIUpdate".into(),
            module_tag: "PositionDozerAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    });
    let mut base = DefaultThingTemplate::new("TallPositionTarget".into());
    base.add_kind_of(KindOf::Structure);
    let mut geometry = GeometryInfo::default();
    geometry
        .shape
        .set(EngineGeometryType::Box, false, 100.0, 10.0, 10.0);
    let mut game = NativeMatch::with_source_template(
        target_x,
        Arc::new(PositionTarget { base, geometry }),
        false,
        source,
    );
    register_actual_partition_objects(&mut game);
    game
}
fn register_actual_partition_objects(game: &mut NativeMatch) {
    for object in [&game.source, &game.target] {
        let object = object.read().unwrap();
        let pose = object.get_position();
        game.collision
            .partition_manager_mut()
            .register_object_oriented(
                object.get_id(),
                crate::object::collide::Coord3D::new(pose.x, pose.y, pose.z),
                crate::object::collide::collision_geometry::GeometryInfo::from_engine_geometry(
                    &object.get_geometry_info().shape,
                ),
                object.get_orientation(),
            )
            .unwrap();
    }
}
fn expected(game: &NativeMatch) -> Coord3D {
    let owner = game.source.read().unwrap();
    let target = game.target.read().unwrap();
    let center = target.get_position();
    let approach = (owner.get_position().y - center.y).atan2(owner.get_position().x - center.x);
    let radius = target.get_geometry_info().get_bounding_sphere_radius();
    Coord3D::new(
        center.x + radius * approach.cos(),
        center.y + radius * approach.sin(),
        0.0,
    )
}
fn query(game: &mut NativeMatch) -> Result<Coord3D, String> {
    let source = game.source.clone();
    let target = game.target.clone();
    game.operate(70, |world| world.dozer_action_position(&source, &target))
}

#[test]
fn fixed_sphere_candidate_uses_receiving_pose_geometry_and_no_peer_borrows() {
    let mut first = world(220.0);
    let peer = world(250.0);
    let expected = expected(&first);
    let _peer_source = peer.source.write().unwrap();
    let _peer_target = peer.target.write().unwrap();
    assert_eq!(query(&mut first).unwrap(), expected);
    let owner = first.source.clone();
    assert_eq!(
        first
            .operate(71, |world| world.dozer_action_distance_squared(
                &owner,
                &Coord3D::new(60.0, 50.0, 999.0)
            ))
            .unwrap(),
        (10.0 - 2.0_f32.sqrt()).powi(2),
        "CPP boundary 2D ignores Z and subtracts the actor's radius, not target sphere"
    );
}

#[test]
fn impassable_approach_falls_back_to_target_without_searching_another_side() {
    let mut first = world(220.0);
    let mut peer = world(250.0);
    let candidate = expected(&first);
    let peer_candidate = expected(&peer);
    first.pathfinder.set_cell_type_at(
        &candidate,
        crate::ai::pathfind_astar::PathfindCellType::Impassable,
    );
    assert_eq!(
        query(&mut first).unwrap(),
        *first.target.read().unwrap().get_position()
    );
    assert_eq!(query(&mut peer).unwrap(), peer_candidate);
}

#[test]
fn foreign_same_id_target_rejects_before_query_and_off_map_goal_keeps_its_height() {
    let mut first = world(220.0);
    let peer = world(250.0);
    let source = first.source.clone();
    let foreign = peer.target.clone();
    assert!(
        first
            .operate(70, |world| world.dozer_action_position(&source, &foreign))
            .is_err()
    );
    let target = first.target.clone();
    let off_map = Coord3D::new(400.0, 50.0, 77.0);
    first
        .operate(71, |world| {
            world.set_object_position_handle(&target, &off_map)
        })
        .unwrap();
    assert_eq!(query(&mut first).unwrap(), off_map);
}

#[test]
fn exposed_unregistered_target_rejects_instead_of_becoming_empty_space() {
    let mut game = world(220.0);
    let mut peer = world(220.0);
    let target_id = game.target.read().unwrap().get_id();
    game.collision
        .partition_manager_mut()
        .unregister_object(target_id)
        .unwrap();
    let source_before = *game.source.read().unwrap().get_position();
    let target_before = *game.target.read().unwrap().get_position();
    let error = query(&mut game).unwrap_err();
    assert!(error.contains("missing an exposed object"), "{error}");
    assert_eq!(*game.source.read().unwrap().get_position(), source_before);
    assert_eq!(*game.target.read().unwrap().get_position(), target_before);
    assert_eq!(query(&mut peer).unwrap(), expected(&peer));
    register_actual_partition_objects(&mut game);
    assert_eq!(query(&mut game).unwrap(), expected(&game));
}
