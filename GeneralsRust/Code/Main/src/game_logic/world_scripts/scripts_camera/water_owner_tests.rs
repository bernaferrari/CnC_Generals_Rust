//! Actual ScriptEngine effects and terrain phases on the driving Main owner.
use super::*;
use crate::game_logic::pathfinding::GridPos;
use gamelogic::ai::pathfind_astar::PathfindCellType;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{ScriptEngine, ScriptWaterRequest};
use gamelogic::terrain::TerrainDynamicWaterSnapshotEntry;

fn owner_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.pathfinding_system.grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    world.terrain = Some(crate::game_logic::terrain::TerrainData::flat(
        glam::Vec3::ZERO,
        glam::Vec3::new(200.0, 0.0, 200.0),
    ));
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 20;
    map.height = 20;
    map.heightmap = vec![0; 400];
    let mut water = gamelogic::polygon_trigger::PolygonTrigger::new(7, "Lake".into(), Vec::new());
    water.set_water_area(true);
    for (x, y) in [(0, 0), (80, 0), (80, 80), (0, 80)] {
        water.add_point(ICoord3D::new(x, y, 0));
    }
    map.polygon_triggers.push(water);
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.add_player(Player::new(1, Team::USA, "WaterOwner", false));
    let mut template = ThingTemplate::new("WaterBody");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("WaterBody", 1, glam::Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    (world, id)
}

fn action(height: f32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::WaterChangeHeight);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Lake".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Real, height))
        .unwrap();
    action
}

struct ObserveActions<'a> {
    actual: HostScriptExecutionDriver<'a>,
    id: ObjectId,
    observations: Vec<(PathfindCellType, f32)>,
}
impl ScriptExecutionDriver for ObserveActions<'_> {
    fn water(&mut self, request: ScriptWaterRequest<'_>) -> Option<gamelogic::GameLogicResult<()>> {
        self.actual.water(request)
    }
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        self.actual.after_action()?;
        let world = &self.actual.world;
        self.observations.push((
            world.pathfinding_system.grid.cell_type(GridPos::new(2, 2)),
            world.host_object(self.id).unwrap().health.current,
        ));
        Ok(())
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn script_water_classifies_and_damages_before_next_action_with_foreign_ai_held() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "script_water_classifies_and_damages_before_next_action_with_foreign_ai_held",
        || {
            let (mut first, id) = owner_world();
            let (second, other_id) = owner_world();
            assert_eq!(id, other_id);
            let foreign_object = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(id.0, 150.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign_object);
            let foreign_body = foreign_object.read().unwrap().get_body_module().unwrap();
            let foreign_health = foreign_body.lock().unwrap().get_health();
            let engine = ScriptEngine::new().unwrap();
            let foreign = gamelogic::ai::the_ai();
            let _foreign_held = foreign.write().unwrap();
            let mut actions = action(5.0);
            actions.next_action = Some(Box::new(action(0.0)));
            let mut observer = ObserveActions {
                actual: HostScriptExecutionDriver::new(&mut first),
                id,
                observations: Vec::new(),
            };
            engine.friend_execute_action_with_driver(
                &actions,
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut observer,
            );
            assert_eq!(observer.observations.len(), 2);
            assert_eq!(
                observer.observations[0].0,
                PathfindCellType::Water,
                "classification completes during the raising action"
            );
            assert!(
                observer.observations[0].1 <= 0.0,
                "DAMAGE_WATER completes before the next action"
            );
            assert_eq!(
                observer.observations[1].0,
                PathfindCellType::Clear,
                "lowering is also immediately visible"
            );
            assert_eq!(
                foreign_body.lock().unwrap().get_health(),
                foreign_health,
                "same-ID Core body is never the water receiver"
            );
            assert_eq!(second.host_object(id).unwrap().health.current, 100.0);
            assert!(
                !second
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_underwater(20.0, 20.0, None, None)
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn dynamic_water_uses_owned_frame_fractional_duration_and_terminal_damage() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "dynamic_water_uses_owned_frame_fractional_duration_and_terminal_damage",
        || {
            let (mut world, id) = owner_world();
            world
                .apply_owned_script_water(ScriptWaterRequest::OverTime {
                    name: "Lake",
                    height: 2.0,
                    seconds: 0.05,
                    damage: 10.0,
                })
                .unwrap();
            assert_eq!(
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .dynamic_water_count(),
                1
            );
            world.frame = 1;
            world.update_owned_water();
            assert_eq!(
                world.host_object(id).unwrap().health.current,
                100.0,
                "frame1 intermediate transition does not damage"
            );
            let handle = world.world_services.terrain().clone();
            {
                let terrain = handle.read().unwrap();
                assert_eq!(
                    terrain
                        .get_water_handle_by_name(&"Lake".into())
                        .unwrap()
                        .get_current_height(),
                    1.0,
                    "actual polygon Z truncates the 1.333 float accumulator"
                );
                assert_eq!(
                    terrain.dynamic_water_count(),
                    1,
                    "0.05 seconds is 1.5 frames, not truncated to one"
                );
            }
            world.frame = 2;
            world.update_owned_water();
            assert_eq!(
                world.host_object(id).unwrap().health.current,
                90.0,
                "terminal transition damages even outside the thirty-frame cadence"
            );
            assert_eq!(handle.read().unwrap().dynamic_water_count(), 0);
            // A zero-duration request schedules infinity and completes at the next
            // owned terrain phase, not in the script action.
            world
                .apply_owned_script_water(ScriptWaterRequest::OverTime {
                    name: "Lake",
                    height: 4.0,
                    seconds: 0.0,
                    damage: 5.0,
                })
                .unwrap();
            assert_eq!(world.host_object(id).unwrap().health.current, 90.0);
            assert_eq!(handle.read().unwrap().dynamic_water_count(), 1);
            world.frame = 3;
            world.update_owned_water();
            assert_eq!(world.host_object(id).unwrap().health.current, 85.0);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn grid_water_constructor_reset_and_queries_are_instance_owned() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "grid_water_constructor_reset_and_queries_are_instance_owned",
        || {
            let (first, id) = owner_world();
            let handle = first.world_services.terrain().clone();
            {
                let mut terrain = handle.write().unwrap();
                let grid = terrain.water_grid_state_mut();
                grid.enabled = true;
                grid.resolution = (8.0, 8.0, 10.0);
                grid.set_height(12.0);
            }
            let (second, other_id) = owner_world();
            assert_eq!(id, other_id);
            assert!(handle.read().unwrap().is_underwater(20.0, 20.0, None, None));
            assert!(
                !second
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_underwater(20.0, 20.0, None, None)
            );
            second.world_services.terrain().write().unwrap().reset();
            assert_eq!(
                handle
                    .read()
                    .unwrap()
                    .water_grid_state()
                    .height_at(20.0, 20.0),
                Some(12.0)
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn water_admission_failure_preserves_existing_transition_and_continuation() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "water_admission_failure_preserves_existing_transition_and_continuation",
        || {
            let (mut world, id) = owner_world();
            world
                .apply_owned_script_water(ScriptWaterRequest::OverTime {
                    name: "Lake",
                    height: 6.0,
                    seconds: 0.1,
                    damage: 10.0,
                })
                .unwrap();
            world.frame = 1;
            world.update_owned_water();
            let handle = world.world_services.terrain().clone();
            let saved = handle.read().unwrap().snapshot_dynamic_water_entries();
            let mut malformed = saved.clone();
            malformed[0].current_height = 100.0;
            malformed.push(TerrainDynamicWaterSnapshotEntry {
                trigger_id: 999,
                water_name: AsciiString::new(),
                change_per_frame: 1.0,
                target_height: 7.0,
                damage_amount: 10.0,
                current_height: 2.0,
            });
            assert!(
                handle
                    .write()
                    .unwrap()
                    .restore_dynamic_water_entries(malformed)
                    .is_err()
            );
            let retained = handle.read().unwrap().snapshot_dynamic_water_entries();
            assert_eq!(retained.len(), saved.len());
            assert_eq!(retained[0].current_height, saved[0].current_height);
            let (mut restored, restored_id) = owner_world();
            {
                let mut terrain = restored.world_services.terrain().write().unwrap();
                terrain
                    .mutate_named_water_height(&"Lake".into(), saved[0].current_height, 0.0, false)
                    .unwrap();
                terrain.restore_dynamic_water_entries(saved).unwrap();
            }
            for frame in [2, 3] {
                world.frame = frame;
                restored.frame = frame;
                world.update_owned_water();
                restored.update_owned_water();
                assert_eq!(
                    world.host_object(id).unwrap().health.current,
                    restored.host_object(restored_id).unwrap().health.current
                );
            }
            assert_eq!(world.host_object(id).unwrap().health.current, 90.0);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn water_mesh_xfer_admits_only_the_validated_owner_and_preserves_cpp_fields() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "water_mesh_xfer_admits_only_the_validated_owner_and_preserves_cpp_fields",
        || {
            let (first, _) = owner_world();
            let (second, _) = owner_world();
            let first_handle = first.world_services.terrain().clone();
            let second_handle = second.world_services.terrain().clone();
            {
                let mut terrain = first_handle.write().unwrap();
                let grid = terrain.water_grid_state_mut();
                grid.enabled = true;
                grid.resolution = (2.0, 2.0, 10.0);
                grid.height_deltas.insert((1, 1), 3.5);
                grid.mesh_motion.insert(
                    (1, 1),
                    gamelogic::terrain_water::WaterGridMeshMotion {
                        velocity: 2.25,
                        status: 7,
                        preferred_height: 9,
                    },
                );
            }
            second_handle
                .write()
                .unwrap()
                .water_grid_state_mut()
                .resolution = (2.0, 2.0, 10.0);
            let bytes =
                game_client::terrain::terrain_visual_xfer::capture_live_terrain_visual_for_terrain(
                    &first_handle.read().unwrap(),
                )
                .unwrap();
            // C++ layout: W3D v3, base v1, enabled, WaterRenderObj v1, dimensions.
            assert_eq!(&bytes[..12], &[3, 1, 1, 1, 2, 0, 0, 0, 2, 0, 0, 0]);
            let before = second_handle.read().unwrap().water_grid_state().clone();
            let mut broken = bytes.clone();
            broken.truncate(20);
            assert!(
                game_client::terrain::terrain_visual_xfer::restore_live_terrain_visual_for_terrain(
                    &mut second_handle.write().unwrap(),
                    &broken
                )
                .is_err()
            );
            assert_eq!(second_handle.read().unwrap().water_grid_state(), &before);
            game_client::terrain::terrain_visual_xfer::restore_live_terrain_visual_for_terrain(
                &mut second_handle.write().unwrap(),
                &bytes,
            )
            .unwrap();
            let second = second_handle.read().unwrap();
            assert_eq!(second.water_grid_state().height_deltas[&(1, 1)], 3.5);
            let motion = second.water_grid_state().mesh_motion[&(1, 1)];
            assert_eq!(
                (motion.velocity, motion.status, motion.preferred_height),
                (2.25, 7, 9)
            );
            assert_eq!(
                first_handle
                    .read()
                    .unwrap()
                    .water_grid_state()
                    .height_deltas[&(1, 1)],
                3.5
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn script_water_damage_uses_original_diagonal_range_and_no_projectile_skip() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "script_water_damage_uses_original_diagonal_range_and_no_projectile_skip",
        || {
            let (mut world, inside) = owner_world();
            let mut other =
                gamelogic::polygon_trigger::PolygonTrigger::new(8, "OtherLake".into(), Vec::new());
            other.set_water_area(true);
            for (x, y) in [(80, 80), (200, 80), (200, 200), (80, 200)] {
                other.add_point(ICoord3D::new(x, y, 10));
            }
            world
                .world_services
                .terrain()
                .write()
                .unwrap()
                .add_trigger_area(other);
            let near = world
                .create_object_for_player("WaterBody", 1, glam::Vec3::new(90.0, 0.0, 90.0))
                .unwrap();
            let far = world
                .create_object_for_player("WaterBody", 1, glam::Vec3::new(190.0, 0.0, 190.0))
                .unwrap();
            let mut projectile = ThingTemplate::new("WaterProjectileBody");
            projectile.add_kind_of(KindOf::Projectile).set_health(100.0);
            world.templates.insert(projectile.name.clone(), projectile);
            let projectile = world
                .create_object_for_player(
                    "WaterProjectileBody",
                    1,
                    glam::Vec3::new(30.0, 0.0, 30.0),
                )
                .unwrap();
            let engine = ScriptEngine::new().unwrap();
            engine.friend_execute_action_with_driver(
                &action(5.0),
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::new(&mut world),
            );
            assert!(world.host_object(inside).unwrap().health.current <= 0.0);
            assert!(
                world.host_object(near).unwrap().health.current <= 0.0,
                "CPP scans the full diagonal radius and checks any water, not just the edited polygon AABB"
            );
            assert_eq!(
                world.host_object(far).unwrap().health.current,
                100.0,
                "another lake outside the edited table's range is untouched"
            );
            assert!(
                world.host_object(projectile).unwrap().health.current <= 0.0,
                "CPP TerrainLogic never exempts a projectile with a damageable body"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_snapshot_restore_rejects_unresolved_water_trigger_without_touching_running_owner() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_snapshot_restore_rejects_unresolved_water_trigger_without_touching_running_owner",
        || {
            let (running, id) = owner_world();
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let mut saved = builder.create_world_snapshot(&running).unwrap();
            saved.persist_v18.water_updates.push(
                crate::save_load::snapshot::persist_v18::WaterUpdatePersist {
                    trigger_id: 999,
                    change_per_frame: 1.0,
                    target_height: 9.0,
                    damage_amount: 10.0,
                    current_height: 3.0,
                },
            );
            let (mut candidate, other_id) = owner_world();
            assert_eq!(id, other_id);
            assert!(
                builder
                    .restore_from_snapshot(&saved, &mut candidate)
                    .is_err(),
                "CPP TerrainLogic::xfer throws on an unresolved water handle"
            );
            assert_eq!(running.host_object(id).unwrap().health.current, 100.0);
            let terrain = running.world_services.terrain().read().unwrap();
            assert_eq!(terrain.dynamic_water_count(), 0);
            assert!(!terrain.is_underwater(20.0, 20.0, None, None));
        },
    );
}
