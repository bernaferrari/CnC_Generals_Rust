//! Player.cpp:1644-1645 and AIPathfind.cpp:4107-4153 completion wall behavior.
use super::*;
use crate::common::KindOf;
use crate::object::body::active_body::ActiveBodyModuleData;
use game_engine::common::thing::module::ModuleInterfaceType;

fn wall_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeConstructionWall".into());
    base.add_kind_of(KindOf::Structure);
    base.add_kind_of(KindOf::WalkOnTopOfWall);
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "ConstructionWallBody".into(),
            data: game_engine::common::thing::module::ModuleDataHandle::new(
                ActiveBodyModuleData {
                    max_health: 100.0,
                    initial_health: 100.0,
                    ..Default::default()
                },
            ),
            interface_mask: ModuleInterfaceType::BODY,
        }],
    })
}

fn wall_occupant_template() -> Arc<dyn ThingTemplate> {
    let _ = infantry_template(); // Registers the authored ground locomotor.
    let mut base = DefaultThingTemplate::new("NativeConstructionWallOccupant".into());
    base.add_kind_of(KindOf::Infantry);
    let mut ai = crate::object::update::AIUpdateModuleData::default();
    ai.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "ConstructionWallOccupantBody".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(
                    ActiveBodyModuleData {
                        max_health: 100.0,
                        initial_health: 100.0,
                        ..Default::default()
                    },
                ),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "ConstructionWallOccupantAI".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(ai),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: "PhysicsBehavior".into(),
                module_tag: "ConstructionWallOccupantPhysics".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(
                    crate::object::behavior::physics_update::PhysicsBehaviorModuleData::default(),
                ),
                interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::COLLIDE,
            },
        ],
    })
}

fn wall_world() -> NativeMatch {
    NativeMatch::with_source_template(55.0, wall_template(), false, wall_occupant_template())
}

#[test]
fn completed_wall_splats_live_wall_occupant_before_reclassifying_receiving_layer() {
    let mut game = wall_world();
    let wall_position = Coord3D::new(55.0, 55.0, 0.0);
    let wall_id = game.target.read().unwrap().get_id();
    let occupant_id = game.source.read().unwrap().get_id();
    {
        let mut wall = game.target.write().unwrap();
        wall.set_position(&wall_position).unwrap();
    }
    {
        let mut occupant = game.source.write().unwrap();
        occupant.set_position(&wall_position).unwrap();
        occupant.set_layer(crate::common::PathfindLayerEnum::Wall);
        occupant.set_health(1.0).unwrap();
    }
    let wall_shape = {
        let wall = game.target.read().unwrap();
        crate::path::pathfind_layer_classify::WallFootprint::from_object(&wall)
    };
    {
        let pathfinder = game.pathfinder.pathfinding_system_mut();
        pathfinder.add_wall_piece(wall_id);
        pathfinder.reclassify_wall_cells_from_footprints(&[wall_shape]);
        assert_eq!(
            pathfinder.get_stored_cell_type_at_layer(
                &wall_position,
                crate::ai::pathfind_astar::PathfindLayerEnum::Wall,
            ),
            Some(crate::ai::pathfind_complete::PathfindCellType::Cliff),
            "C++ wall classification tightens this edge cell to Cliff",
        );
    }
    let wall = Arc::clone(&game.target);
    game.operate(37, |world| {
        world.refresh_native_construction_footprint(&wall).unwrap();
    });

    assert_eq!(game.source.read().unwrap().get_health(), 0.0);
    assert!(game.source.read().unwrap().is_effectively_dead());
    let pathfinder = game.pathfinder.pathfinding_system_mut();
    assert_eq!(pathfinder.wall_piece_count(), 0);
    assert_eq!(
        pathfinder.get_stored_cell_type_at_layer(
            &wall_position,
            crate::ai::pathfind_astar::PathfindLayerEnum::Wall,
        ),
        Some(crate::ai::pathfind_complete::PathfindCellType::Impassable),
        "C++ reclassifies the wall after applying falling damage",
    );
    assert_ne!(wall.read().unwrap().get_id(), occupant_id);
}
