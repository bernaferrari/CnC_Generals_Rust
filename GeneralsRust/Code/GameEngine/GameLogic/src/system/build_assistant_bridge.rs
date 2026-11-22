//! GameLogic bridge for the Common build assistant backend.

use std::sync::Arc;

use crate::common::Coord3D as LogicCoord3D;
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::object::contain::transport_contain::selected_locomotor_is_airborne;
use crate::object::production::construction::FoundationValidator;
use crate::object::update::ai_update::dozer_ai_update::DozerTask;
use crate::player::player_list;
use game_engine::common::system::build_assistant::{
    BuildAssistantBackend, Coord3D, LegalBuildCode, LocalLegalToBuildOptions, ObjectID,
    set_build_assistant_backend,
};

mod dozer_dock;

#[derive(Debug, Default)]
pub struct GameLogicBuildAssistantBackend;

impl BuildAssistantBackend for GameLogicBuildAssistantBackend {
    fn build_object_now(
        &self,
        builder_id: Option<ObjectID>,
        template_name: &str,
        pos: &Coord3D,
        angle: f32,
        owning_player: u32,
    ) -> Option<ObjectID> {
        let template = TheThingFactory::find_template(template_name)?;
        let builder = builder_id.and_then(TheGameLogic::find_object_by_id);

        let team = if builder_id.is_some() {
            builder.as_ref()?.read().ok()?.get_team()
        } else {
            let __player_list_arc = player_list();
        if let Ok(list) = __player_list_arc.read() {
                if let Some(player) = list.get_player(owning_player as i32) {
                    if let Ok(player_guard) = player.read() {
                        player_guard.get_default_team()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }?;

        let factory = TheThingFactory::get().ok()?;
        // C++ BuildAssistant.cpp:361-368 — structures spawn UNDER_CONSTRUCTION
        // so onCreate does not fire finished-building hooks (GrantUpgrade, power).
        let new_object = if template.is_kind_of(crate::common::KindOf::Structure) {
            factory.new_construction_scaffold(Arc::clone(&template), team).ok()?
        } else {
            let team = team.read().ok()?;
            factory.new_object(Arc::clone(&template), &team).ok()?
        };

        let (placement_world, build_max_health) = {
            let object = new_object.read().ok()?;
            let max_health = object.get_body_module()
                .and_then(|body| body.lock().ok().map(|body| body.get_max_health()))
                .unwrap_or(0.0);
            (object.owning_world_services(), max_health)
        };

        if let Ok(mut guard) = new_object.write() {
            let _ = guard.set_position(&LogicCoord3D::new(pos.x, pos.y, pos.z));
            let _ = guard.set_orientation(angle as f32);
            if let Some(builder) = &builder
                && let Ok(builder_guard) = builder.read()
            {
                guard.set_producer(Some(&*builder_guard));
                guard.set_builder(Some(&*builder_guard));
            }
            guard.set_construction_percent(0.0);
            if build_max_health > 0.0 {
                let _ = guard.set_health(1.0);
            }
        }

        // C++ DozerAIUpdate.cpp:1692-1696 / WorkerAIUpdate.cpp:404-408 —
        // flattenTerrain then snap Z to getGroundHeight before pathfind add.
        let terrain = placement_world.as_ref()
            .map(|world| Arc::clone(world.terrain()))
            .unwrap_or_else(crate::terrain::get_terrain_logic);
        let ground_height = terrain.write().ok().map(|mut terrain| {
            terrain.flatten_terrain(&new_object);
            terrain.get_ground_height(pos.x, pos.y, None)
        });
        // The position callback reads terrain to update the object's area and height.
        if let Some(z) = ground_height
            && let Ok(mut guard) = new_object.write()
        {
            let _ = guard.set_position(&LogicCoord3D::new(pos.x, pos.y, z));
        }

        // DozerAIUpdate.cpp:1699 and WorkerAIUpdate.cpp:411 classify the
        // footprint from the snapped pose before the builder receives the task.
        let placed = new_object.read().ok()?;
        if let Some(owner) = placed.classified_pathfinding_owner()
            && let Err(error) = owner.classify_owned_object_footprint(&placed, true)
        {
            let id = placed.get_id();
            let world = Arc::clone(owner.services());
            drop(placed);
            log::error!("cannot classify placed construction {id}: {error}");
            let _ = world.destroy_logic_object(id);
            return None;
        }
        let receiving_core = placed.classified_pathfinding_owner().cloned();
        let scaffold_id = placed.get_id();
        drop(placed);

        if let Some(builder) = builder
            && let Ok(builder_guard) = builder.read()
        {
            let ai = builder_guard.get_ai_update_interface();
            let builder_id = builder_guard.get_id();
            let position = *builder_guard.get_position();
            let player_id = builder_guard.get_controlling_player_id();
            let services = builder_guard.owning_world_services();
            let core = receiving_core.filter(|core| {
                services.as_ref().is_some_and(|world| Arc::ptr_eq(core.services(), world))
            });
            drop(builder_guard);

            let roster = services.as_ref()
                .map(|world| Arc::clone(world.players()))
                .unwrap_or_else(player_list);
            let player = player_id.and_then(|id| roster.read().ok()?.get_player(id as i32).cloned());
            let player = player.as_ref().and_then(|player| player.read().ok());
            let total_build_frames = template
                .calc_time_to_build(player.as_deref().map(|player| player as &dyn std::any::Any))
                .max(1) as u32;
            drop(player);
            let frame = core.as_ref().map(|core| core.services().logic_frame());
            if let Some(ai) = ai
                && let Ok(mut ai) = ai.try_lock()
            {
                let airborne = selected_locomotor_is_airborne(&*ai);
                if let Some(worker) = ai.get_worker_ai_update_interface_mut() {
                    worker.set_build_task(scaffold_id, total_build_frames, build_max_health, false, airborne);
                } else {
                    let surfaces = core.as_ref().and_then(|_| ai.get_locomotor_set_clone())
                        .map(|locomotors| locomotors.get_valid_surfaces());
                    if let Some(dozer) = ai.get_dozer_ai_update_interface_mut() {
                        if let (Some(core), Some(frame), Some(surfaces)) = (core, frame, surfaces)
                            && !dozer.is_any_task_pending()
                            && dozer.get_current_task() == DozerTask::Invalid
                        {
                            let source = dozer_dock::DozerDockSource { id: builder_id, position, airborne, surfaces };
                            let placed_order = dozer_dock::find_build_dock(&core, &source, &new_object)
                                .and_then(|dock| {
                                    let scaffold = new_object.try_read().map_err(|_| "placed scaffold borrowed")?;
                                    dozer.admit_placed_build(&scaffold, dock, total_build_frames, build_max_health, frame)
                                });
                            if let Err(error) = placed_order {
                                log::error!("cannot admit placed Dozer construction {scaffold_id}: {error}");
                                drop(ai);
                                if let Err(error) = core.services().destroy_logic_object(scaffold_id) {
                                    log::error!("cannot retire rejected scaffold {scaffold_id}: {error}");
                                }
                                return None;
                            }
                        } else {
                            dozer.set_build_task(scaffold_id, total_build_frames, build_max_health, false, airborne);
                        }
                    }
                }
            }
        }

        Some(scaffold_id)
    }

    fn is_location_legal_to_build(
        &self,
        world_pos: &Coord3D,
        template_name: &str,
        angle: f32,
        _options: LocalLegalToBuildOptions,
        _builder_id: Option<ObjectID>,
        player_id: Option<u32>,
    ) -> LegalBuildCode {
        let validator = FoundationValidator::from_build_options(_options);
        let logic_pos = LogicCoord3D::new(world_pos.x, world_pos.y, world_pos.z);
        let owner = player_id.unwrap_or(0);
        match validator.validate_placement(&logic_pos, template_name, angle, owner) {
            Ok(()) => LegalBuildCode::Ok,
            Err(err) => {
                let msg = err.to_ascii_lowercase();
                if msg.contains("shroud") {
                    LegalBuildCode::Shroud
                } else if msg.contains("object") {
                    LegalBuildCode::ObjectsInTheWay
                } else if msg.contains("path") {
                    LegalBuildCode::NoClearPath
                } else if msg.contains("flat") {
                    LegalBuildCode::NotFlatEnough
                } else if msg.contains("cliff")
                    || msg.contains("underwater")
                    || msg.contains("bridge")
                    || msg.contains("terrain")
                {
                    LegalBuildCode::RestrictedTerrain
                } else {
                    LegalBuildCode::GenericFailure
                }
            }
        }
    }

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        crate::helpers::TheTerrainLogic::get()
            .map(|terrain| terrain.get_ground_height(x, y, None))
            .unwrap_or(0.0)
    }

    fn on_selling(&self, id: ObjectID) {
        let Some(obj) = TheGameLogic::find_object_by_id(id) else {
            return;
        };
        let contain = match obj.read() {
            Ok(guard) => guard.get_contain(),
            Err(_) => return,
        };
        if let Some(contain) = contain {
            use crate::modules::ContainModuleInterfaceExt;
            let _ = contain.on_selling();
        }
    }
}

pub fn install_build_assistant_backend() {
    set_build_assistant_backend(Arc::new(GameLogicBuildAssistantBackend::default()));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::ai::Pathfinder;
    use crate::ai::pathfind_complete::{PathfindCellType, SURFACE_GROUND};
    use crate::object::Object;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::team::Team;
    use std::sync::RwLock;

    fn place(world: &Arc<EngineStores>, center: LogicCoord3D, angle: f32) -> ObjectID {
        use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};

        with_active_stores(world, || {
            assert!(ensure_thing_factory_exists());
            world.services().terrain().write().unwrap().try_restore_logic_height_map_with_effects(
                40, 40, &[100; 1600], &mut || {},
            ).unwrap();
            let definitions = "
                Object FootprintScaffold
                    KindOf = STRUCTURE IMMOBILE
                    Geometry = BOX
                    GeometryMajorRadius = 18
                    GeometryMinorRadius = 4
                    GeometryHeight = 12
                End
                Object ForeignFootprint
                    KindOf = STRUCTURE IMMOBILE
                    Geometry = CYLINDER
                    GeometryMajorRadius = 3
                    GeometryHeight = 8
                End
            ";
            assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(definitions), 2);

            let team = Arc::new(RwLock::new(Team::new("PlacementTeam".into(), 71)));
            team.write().unwrap().set_controlling_player_id(Some(0));
            let mut player = crate::player::Player::new(0);
            player.set_default_team(Some(Arc::clone(&team)));
            world.services().players().write().unwrap().add_player(Arc::new(RwLock::new(player)));

            let builder = Arc::new(RwLock::new(Object::new_test(700, 100.)));
            let foreign = Arc::new(RwLock::new(Object::new_test_from_template(
                701,
                100.,
                TheThingFactory::find_template("ForeignFootprint").unwrap(),
            )));
            foreign.write().unwrap().set_position(&(center + LogicCoord3D::new(10., 0., 0.))).unwrap();
            {
                let mut logic = world.game_logic_mutex().lock().unwrap();
                logic.register_object(Arc::clone(&builder)).unwrap();
                logic.register_object(Arc::clone(&foreign)).unwrap();
            }
            builder.write().unwrap().set_team(Some(team)).unwrap();
            {
                let pathfinder = world.ai().read().unwrap().pathfinder().unwrap();
                let mut pathfinder = pathfinder.write().unwrap();
                pathfinder.reset_with_size(40, 40);
                pathfinder.add_object_to_pathfind_map(&foreign.read().unwrap()).unwrap();
            }

            GameLogicBuildAssistantBackend
                .build_object_now(
                    Some(700),
                    "FootprintScaffold",
                    &Coord3D {x: center.x, y: center.y, z: 12.},
                    angle,
                    0,
                )
                .expect("actual build assistant must place the authored scaffold")
        })
    }

    #[test]
    fn ordered_scaffold_placement_stamps_its_owned_final_footprint() {
        use crate::test_process::{TestProcess, run_bounded};
        const NAME: &str = concat!(module_path!(), "::ordered_scaffold_placement_stamps_its_owned_final_footprint");
        if let TestProcess::ParentVerified = run_bounded(
            NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
            "GENERALS_CORE_SCAFFOLD_FOOTPRINT_CHILD",
        ) {
            return;
        }

        let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
        let owning = new_for_world();
        let peer = new_for_world();
        let placed = LogicCoord3D::new(105., 105., 0.);
        let peer_placed = LogicCoord3D::new(205., 205., 0.);
        let peer_angle = std::f32::consts::FRAC_PI_2;
        let peer_id = place(&peer, peer_placed, peer_angle);
        let peer_pathfinder = peer.ai().read().unwrap().pathfinder().unwrap();
        let owned_cells = |path: &Pathfinder, position: LogicCoord3D, id: ObjectID| {
            (
                path.get_cell_type_at(&position),
                path.valid_movement_position_for_surfaces(SURFACE_GROUND, false, &position, Some(id)),
            )
        };
        let before_peer = owned_cells(&peer_pathfinder.read().unwrap(), peer_placed, peer_id);
        let id = place(&owning, placed, 0.);
        assert_eq!(id, peer_id);
        let ground_z = owning.services().terrain().read().unwrap().get_ground_height(placed.x, placed.y, None);
        assert!(ground_z > 10.);
        {
            let logic = owning.game_logic_mutex().lock().unwrap();
            let ordinary = logic.find_object_by_id(700).unwrap();
            let ordinary = ordinary.read().unwrap();
            assert!(ordinary.owning_pathfinding_runtime().is_some());
            assert!(ordinary.classified_pathfinding_owner().is_none());
            let structure = logic.find_object_by_id(id).unwrap();
            let structure = structure.read().unwrap();
            assert_eq!(*structure.get_position(), LogicCoord3D::new(placed.x, placed.y, ground_z));
            assert_eq!(structure.get_orientation(), 0.);
            assert_eq!(structure.get_height_above_terrain(), 0.);
            assert_eq!(structure.get_builder_id(), 700);
            assert!(structure.is_under_construction());
            assert!(Arc::ptr_eq(&structure.owning_pathfinding_runtime().unwrap(), &owning));
        }
        {
            let logic = peer.game_logic_mutex().lock().unwrap();
            let structure = logic.find_object_by_id(peer_id).unwrap();
            assert_eq!(structure.read().unwrap().get_orientation(), peer_angle);
        }
        assert_eq!(owned_cells(&peer_pathfinder.read().unwrap(), peer_placed, peer_id), before_peer);
        let receiving = owning.ai().read().unwrap().pathfinder().unwrap();
        let receiving = receiving.read().unwrap();
        let origin = LogicCoord3D::new(5., 5., 0.);
        let foreign = LogicCoord3D::new(115., 105., 0.);
        let observed = [
            owned_cells(&receiving, origin, id),
            owned_cells(&receiving, placed, id),
            owned_cells(&receiving, LogicCoord3D::new(95., 105., 0.), id),
            owned_cells(&receiving, foreign, 701),
            owned_cells(&receiving, foreign, id),
        ];
        assert_eq!(observed, [
            (Some(PathfindCellType::Clear), true),
            (Some(PathfindCellType::Obstacle), true),
            (Some(PathfindCellType::Obstacle), true),
            (Some(PathfindCellType::Obstacle), true),
            (Some(PathfindCellType::Obstacle), false),
        ]);
        assert_eq!(before_peer, (Some(PathfindCellType::Obstacle), true));
        {
            let peer_pathfinder = peer_pathfinder.read().unwrap();
            assert_eq!(owned_cells(&peer_pathfinder, placed, id), (Some(PathfindCellType::Clear), true));
            assert_eq!(owned_cells(&peer_pathfinder, peer_placed + LogicCoord3D::Y * 10., id), (Some(PathfindCellType::Obstacle), true));
            assert_eq!(owned_cells(&peer_pathfinder, peer_placed - LogicCoord3D::X * 10., id), (Some(PathfindCellType::Clear), true));
        }
        drop(receiving);

        with_active_stores(&owning, || {
            let (builder, structure) = {
                let logic = owning.game_logic_mutex().lock().unwrap();
                (logic.find_object_by_id(700).unwrap(), logic.find_object_by_id(id).unwrap())
            };
            crate::object::update::ai_update::dozer_ai_update::construction_callback_tests::complete_dozer_for_test(
                &builder, &structure, false,
            );
            assert!(!structure.read().unwrap().is_under_construction());
            let mut logic = owning.game_logic_mutex().lock().unwrap();
            logic.destroy_object(id);
            logic.cleanup_dead_objects().unwrap();
        });
        let receiving = owning.ai().read().unwrap().pathfinder().unwrap();
        let receiving = receiving.read().unwrap();
        assert_eq!(owned_cells(&receiving, placed, id), (Some(PathfindCellType::Clear), true));
        assert_eq!(owned_cells(&receiving, foreign, 701), (Some(PathfindCellType::Obstacle), true));
        assert_eq!(owned_cells(&peer_pathfinder.read().unwrap(), peer_placed, peer_id), before_peer);
    }
}
