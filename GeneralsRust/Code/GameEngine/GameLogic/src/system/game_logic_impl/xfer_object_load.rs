/// C++ `GameLogic::xfer` load (GameLogic.cpp:4732-4785):
/// Resolve the TOC ID, skip missing templates, create and restore the object,
/// xferSnapshot, then `addWallPiece` for `KINDOF_WALK_ON_TOP_OF_WALL`.
fn xfer_game_logic_objects_load(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    object_count: UnsignedInt,
) -> Result<(), XferStatus> {
    if object_count == 0 {
        return Ok(());
    }
    let world = logic.owning_world_services().ok_or(XferStatus::InvalidData)?;
    let core = logic.pathfinding_owner.clone();
    // GameState loads Teams and Players before this block. Object::Object
    // requires the receiving world's neutral default team during birth.
    let neutral = world
        .players()
        .read()
        .map_err(|_| XferStatus::InvalidData)?
        .get_neutral_player()
        .ok_or_else(|| {
            warn!("Core Object snapshot requires a receiving neutral player");
            XferStatus::InvalidData
        })?;
    let default_team = neutral
        .read()
        .map_err(|_| XferStatus::InvalidData)?
        .get_default_team()
        .ok_or_else(|| {
            warn!("Core Object snapshot requires the receiving neutral player's default team");
            XferStatus::InvalidData
        })?;
    let default_team_id = default_team
        .read()
        .map_err(|_| XferStatus::InvalidData)?
        .get_id();
    let registered_team = world
        .team_factory()
        .with_read_without_flushing(|factory| factory.find_team_by_id(default_team_id))
        .map_err(|_| XferStatus::InvalidData)?;
    if !registered_team.is_some_and(|team| Arc::ptr_eq(&team, &default_team)) {
        warn!(
            "Core Object snapshot neutral team {default_team_id} is outside the receiving factory"
        );
        return Err(XferStatus::InvalidData);
    }

    for _ in 0..object_count {
        let mut toc_id: UnsignedShort = 0;
        xfer.xfer_unsigned_short(&mut toc_id)?;
        let Some(entry) = logic.find_toc_entry_by_id(toc_id) else {
            warn!("Core Object snapshot refers to unknown TOC id {toc_id}");
            return Err(XferStatus::InvalidData);
        };
        let block_size = xfer.begin_block()?;
        if block_size < 0 {
            warn!(
                "Core Object snapshot for {} has negative block size {block_size}",
                entry.name
            );
            return Err(XferStatus::InvalidData);
        }

        let Some(template) =
            crate::helpers::TheThingFactory::find_template_in(world.common(), &entry.name)
        else {
            warn!(
                "Core Object snapshot is missing template {}, skipping {block_size} bytes",
                entry.name
            );
            xfer.skip(block_size)?;
            xfer.end_block()?;
            continue;
        };

        // Read the header before any constructor or admission callback can run.
        let id = Object::xfer_identity(INVALID_ID, &mut CommonXferBridge { inner: xfer }).map_err(
            |error| {
                warn!("Core Object snapshot failed: {error}");
                XferStatus::ReadError
            },
        )?;
        if id == INVALID_ID || logic.objects.contains_key(&id) {
            warn!("Core Object snapshot has invalid or duplicate identity {id}");
            return Err(XferStatus::InvalidData);
        }

        let restore =
            || restore_core_neutral_object(logic, xfer, &world, default_team.clone(), template, id);
        match core.as_ref() {
            Some(core) => crate::system::engine_stores::with_active_stores(core, restore),
            None => crate::system::engine_stores::with_world_services(&world, restore),
        }?;
    }
    Ok(())
}

/// ThingFactory.cpp:283–319 and Object.cpp:2729–2749. Run birth on a
/// provisional actor, then move the same actor before applying its saved pose.
fn restore_core_neutral_object(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    world: &Arc<crate::system::engine_stores::WorldServices>,
    team: Arc<RwLock<crate::team::Team>>,
    template: Arc<dyn crate::common::ThingTemplate>,
    id: ObjectID,
) -> Result<(), XferStatus> {
    let invalid = |error: &dyn std::fmt::Display| {
        warn!("Core restore of Object {id}: {error}");
        XferStatus::InvalidData
    };
    let factory = &world.logic_managers().object_factory;
    let factory_has_id = factory
        .read()
        .map_err(|error| invalid(&error))?
        .get_object(id)
        .is_some();
    let manager_has_id = world
        .objects()
        .read()
        .map_err(|error| invalid(&error))?
        .get_object(id)
        .is_some();
    if factory_has_id || manager_has_id || world.object_registry().has_indexed_identity(id)
        .map_err(|error| invalid(&error))?
    {
        return Err(invalid(&"saved identity already exists in receiving object indexes"));
    }
    world.logic_managers().collision.read().map_err(|error| invalid(&error))?
        .check_restored_birth_identity(id).map_err(|error| invalid(&error))?;

    let template =
        crate::helpers::TheThingFactory::resolve_build_variation_in(world.common(), template);
    let provisional = logic.allocate_object_id();
    let actor = crate::object::object_factory::ObjectFactory::assemble_core_restore(
        template,
        provisional,
        team,
        logic.frame,
        world,
    )
    .map_err(|error| invalid(&error))?;
    logic
        .admit_object_impl(actor.clone(), false)
        .map_err(|error| invalid(&error))?;
    actor
        .write()
        .map_err(|error| invalid(&error))?
        .run_on_create();
    logic.register_object_with_partition(provisional, &actor);
    actor
        .write()
        .map_err(|error| invalid(&error))?
        .init_core_restored_birth(logic);

    rekey_core_neutral_birth(logic, world, &actor, provisional, id)
        .map_err(|error| invalid(&error))?;
    world
        .legacy_object_registry()
        .register_restored_object(id, &actor);
    factory
        .write()
        .map_err(|error| invalid(&error))?
        .record_core_restored_object(id, &actor)
        .map_err(|error| invalid(&error))?;

    let (drawable, wall, name, pose) = {
        let mut actor = actor.write().map_err(|error| invalid(&error))?;
        let drawable = actor.get_drawable().and_then(|drawable| {
            let birth_id = drawable.read().ok()?.get_drawable_id();
            Some((drawable, birth_id))
        });
        let transferred = xfer_object_snapshot(
            &mut actor,
            xfer,
            logic.frame,
            Some(CoreObjectLoad {
                id,
                teams: Some(world.team_factory()),
            }),
            &mut logic.objects_changed_trigger_areas,
            &mut logic.frame_objects_changed_trigger_areas,
        );
        if let Err(error) = transferred {
            if let Some((drawable, birth_id)) = drawable {
                if let Ok(mut drawable) = drawable.write() {
                    drawable.set_drawable_id(birth_id);
                }
            }
            return Err(error);
        }
        (
            drawable,
            actor.is_kind_of(KindOf::WalkOnTopOfWall),
            actor.get_name().to_string(),
            *actor.get_position(),
        )
    };
    let bridge_layer = {
        let actor = actor.read().map_err(|error| invalid(&error))?;
        if actor.is_kind_of(KindOf::Bridge) {
            let mut terrain = world.terrain().write().map_err(|error| invalid(&error))?;
            actor
                .bind_restored_bridge(&mut terrain)
                .map_err(|error| invalid(&error))?
        } else {
            None
        }
    };
    // Overflow spans use Ground and have no dedicated pathfinder layer.
    if let Some(layer) = bridge_layer
        && layer != crate::path::PathfindLayerEnum::Ground
        && let Some(owner) = logic.pathfinding_owner.as_ref()
    {
        let pathfinder = owner
            .ai()
            .read()
            .map_err(|error| invalid(&error))?
            .pathfinder()
            .ok_or_else(|| invalid(&"receiving pathfinder is missing"))?;
        if !pathfinder
            .write()
            .map_err(|error| invalid(&error))?
            .bind_restored_bridge(layer, id)
        {
            return Err(invalid(&"admitted bridge layer is missing"));
        }
    }
    if let Some((drawable, birth_id)) = drawable {
        TheGameClient::rekey_restored_drawable(world, &drawable, birth_id)
            .map_err(|error| invalid(&error))?;
    }
    if !name.is_empty() {
        world
            .scripts()
            .named_objects()
            .register_named_object(name, id)
            .map_err(|error| invalid(&error))?;
    }
    logic
        .partition_manager_mut()
        .add_object(id, (pose.x, pose.y, pose.z));
    world
        .objects()
        .write()
        .map_err(|error| invalid(&error))?
        .record_core_restored_object(id, actor)
        .map_err(|error| invalid(&error))?;
    xfer.end_block()?;
    if wall {
        add_loaded_wall_piece(logic, id);
    }
    Ok(())
}

fn rekey_core_neutral_birth(
    logic: &mut GameLogic,
    world: &crate::system::engine_stores::WorldServices,
    actor: &Arc<RwLock<Object>>,
    provisional: ObjectID,
    saved: ObjectID,
) -> Result<(), &'static str> {
    if provisional == saved {
        return Ok(());
    }
    let position = logic
        .all_objects
        .iter()
        .position(|id| *id == provisional)
        .ok_or("provisional Object absent from object list")?;
    world
        .object_registry()
        .rekey_restored_birth(provisional, saved, actor)?;
    {
        let mut ghosts = world
            .w3d_ghost_objects()
            .write()
            .map_err(|_| "birth ghosts poisoned")?;
        world
            .logic_managers()
            .collision
            .write()
            .map_err(|_| "birth collision poisoned")?
            .rekey_restored_birth(provisional, saved, &mut ghosts)?;
    }
    {
        let mut actor = actor.write().map_err(|_| "birth Object poisoned")?;
        actor.rekey_core_restored_birth(saved)?;
        if let Some(previous) = actor.get_prev_object_id() {
            logic.objects[&previous]
                .write()
                .map_err(|_| "previous Object poisoned")?
                .set_next_object_id(Some(saved));
        }
        if let Some(next) = actor.get_next_object_id() {
            logic.objects[&next]
                .write()
                .map_err(|_| "next Object poisoned")?
                .set_prev_object_id(Some(saved));
        }
    }
    logic.all_objects[position] = saved;
    let actor = logic
        .objects
        .remove(&provisional)
        .expect("validated provisional Object");
    logic.objects.insert(saved, actor);
    for queued in &mut logic.objects_changed_trigger_areas {
        if *queued == provisional {
            *queued = saved;
        }
    }
    if let Some(GameEvent::ObjectCreated(id)) = logic
        .event_queue
        .iter_mut()
        .rev()
        .find(|event| matches!(event, GameEvent::ObjectCreated(id) if *id == provisional))
    {
        *id = saved;
    }
    Ok(())
}

fn add_loaded_wall_piece(logic: &GameLogic, object_id: ObjectID) {
    let ai_store = logic
        .pathfinding_owner
        .as_ref()
        .map(|owner| Arc::clone(owner.ai()))
        .unwrap_or_else(the_ai);
    if let Ok(ai) = ai_store.read() {
        if let Some(pathfinder) = ai.pathfinder() {
            if let Ok(mut pf) = pathfinder.write() {
                pf.add_wall_piece(object_id);
            }
        }
    }
}

fn pathfinder_new_map_after_polygon_load(logic: &GameLogic) -> Result<(), XferStatus> {
    // C++ GameLogic.cpp:4880 runs newMap after trigger restore.
    let Some(owner) = logic.pathfinding_owner.as_ref() else {
        // The standalone compatibility object registration is still paired
        // with its legacy footprint retirement.
        if let Ok(ai) = the_ai().read()
            && let Some(pathfinder) = ai.pathfinder()
            && let Ok(mut pathfinder) = pathfinder.write()
            && let Ok(terrain) = get_terrain_logic().read()
        {
            pathfinder.rebuild_from_terrain(&terrain);
        }
        return Ok(());
    };

    let pathfinder = owner.ai().read().map_err(|_| XferStatus::InvalidData)?
        .pathfinder().ok_or(XferStatus::InvalidData)?;
    let mut pathfinder = pathfinder.write().map_err(|_| XferStatus::InvalidData)?;
    let terrain = owner.services().terrain().read().map_err(|_| XferStatus::InvalidData)?;
    pathfinder.rebuild_from_terrain_after_load(
        &terrain,
        logic.get_all_object_ids().iter().map(|id| &logic.objects[id]),
        |id| logic.find_object_by_id(id).map(|wall| {
            let wall = wall.try_read().map_err(|error| format!("loaded wall {id}: {error}"))?;
            Ok(crate::path::pathfind_layer_classify::WallFootprint::from_object(&wall))
        }).transpose(),
    ).map_err(|error| {
        log::error!("cannot rebuild the loaded pathfinding map: {error}");
        XferStatus::InvalidData
    })
}

#[cfg(test)]
fn add_fixture_neutral_player(
    world: &Arc<crate::system::engine_stores::WorldServices>,
    team: Arc<RwLock<crate::team::Team>>,
) -> Arc<RwLock<crate::player::Player>> {
    use crate::player::{Player, PlayerType};

    let mut neutral = Player::new_for_world(0, world);
    neutral.init_from_dict_defaults();
    neutral.set_player_type(PlayerType::Neutral, false);
    neutral.set_default_team(Some(team));
    let neutral = Arc::new(RwLock::new(neutral));
    world.players().write().unwrap().add_player(neutral.clone());
    neutral
}

#[cfg(test)]
fn admit_fixture_neutral_player(world: &Arc<crate::system::engine_stores::WorldServices>) {
    let team = {
        let mut factory = world.team_factory().lock().unwrap();
        let prototype = factory
            .init_team_owned("CoreXferNeutral".into(), "".into(), false, None, Some(0))
            .unwrap();
        factory
            .create_team_on_prototype_with_id(&prototype, 94_000)
            .unwrap()
    };
    add_fixture_neutral_player(world, team);
}

#[cfg(test)]
mod object_block_tests {
    use super::*;
    use game_engine::common::system::snapshot::Snapshotable;
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use game_engine::{XferLoad, XferSave};
    use game_engine::common::thing::thing_factory::load_templates_from_ini_text;

    #[test]
    fn core_neutral_restore_runs_birth_and_rekeys_in_the_receiving_world() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::core_neutral_restore_runs_birth_and_rekeys_in_the_receiving_world"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_CORE_NEUTRAL_RESTORE_OWNER_CHILD",
        ) {
            return;
        }

        const ID: ObjectID = 91_304;
        const BIRTH: ObjectID = 4_117;
        const DRAW: u32 = 820;
        const BIRTH_DRAW: u32 = 26;
        const FRAME: u32 = 123;
        let pose = Coord3D::new(400.0, 280.0, 0.0);
        let source = Arc::new(EngineStores::new_for_world());
        let receiving = Arc::new(EngineStores::new_for_world());
        let peer = Arc::new(EngineStores::new_for_world());
        let seed = |core: &Arc<EngineStores>, first_draw| {
            with_active_stores(core, || {
                core.services()
                    .session_flags()
                    .next_drawable_id
                    .store(first_draw, Ordering::Relaxed);
                crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
                core.services().scripts().initialize_engine(core.services()).unwrap();
                assert_eq!(
                    load_templates_from_ini_text(
                        "Object CoreRestoreWarehouse\n KindOf = IMMOBILE\n Behavior = SupplyWarehouseCreate ModuleTag_Warehouse\n End\nEnd\n",
                        "core-restore-warehouse.ini",
                    ),
                    1
                );
                let (team, prototype) = {
                    let mut teams = core.services().team_factory().lock().unwrap();
                    let prototype = teams
                        .init_team_owned(
                            "CoreRestoreNeutral".into(),
                            "".into(),
                            false,
                            None,
                            Some(0),
                        )
                        .unwrap();
                    let team = teams
                        .create_team_on_prototype_with_id(&prototype, 91_302)
                        .unwrap();
                    (team, prototype)
                };
                let neutral = add_fixture_neutral_player(core.services(), team.clone());
                neutral.write().unwrap().add_team_to_list(prototype.clone());
                (team, neutral, prototype)
            })
        };
        let (source_team, source_neutral, _) = seed(&source, DRAW);
        let (team, neutral, prototype) = seed(&receiving, BIRTH_DRAW);
        let (_, peer_neutral, _) = seed(&peer, DRAW);
        let path = std::env::temp_dir().join(format!(
            "generals-core-neutral-restore-{}.bin",
            std::process::id()
        ));
        with_active_stores(&source, || {
            let template =
                crate::helpers::TheThingFactory::find_template("CoreRestoreWarehouse").unwrap();
            let source_actor = Object::new_with_id(
                template,
                ID,
                ObjectStatusMaskType::none(),
                Some(source_team),
            )
            .unwrap();
            {
                let mut actor = source_actor.write().unwrap();
                actor.set_name("CoreSavedWarehouse".into());
                actor.set_position(&pose).unwrap();
            }
            let mut logic = GameLogic::new_with_engine_stores(source.clone());
            logic.frame = FRAME;
            logic.register_object(source_actor).unwrap();
            let mut xfer = XferSave::new();
            xfer.open(path.to_string_lossy().into_owned()).unwrap();
            Snapshotable::xfer(
                &mut *source_neutral.write().unwrap(),
                &mut CommonXferBridge { inner: &mut xfer },
            )
            .unwrap();
            logic
                .xfer_native_snapshot_with_campaign(&mut xfer, &mut Default::default())
                .unwrap();
            xfer.close().unwrap();
            logic.clear_all_objects();
        });

        let receiving_logic = with_active_stores(&receiving, get_game_logic);
        let mut logic = receiving_logic.lock().unwrap();
        logic.next_object_id = BIRTH;
        with_active_stores(&peer, || {
            let peer_logic = get_game_logic();
            let template =
                crate::helpers::TheThingFactory::find_template("CoreRestoreWarehouse").unwrap();
            let peer_actor =
                Object::new_with_id(template, ID, ObjectStatusMaskType::none(), None).unwrap();
            peer_logic
                .lock()
                .unwrap()
                .register_object(peer_actor.clone())
                .unwrap();

            let mut xfer = XferLoad::new();
            xfer.open(path.to_string_lossy().into_owned()).unwrap();
            {
                let mut player = neutral.write().unwrap();
                Snapshotable::xfer(&mut *player, &mut CommonXferBridge { inner: &mut xfer })
                    .unwrap();
                assert!(Arc::ptr_eq(&player.get_default_team().unwrap(), &team));
                assert!(Arc::ptr_eq(&player.get_player_team_prototypes()[0], &prototype));
            }
            logic
                .xfer_native_snapshot_with_campaign(&mut xfer, &mut Default::default())
                .unwrap();
            xfer.close().unwrap();
            std::fs::remove_file(&path).unwrap();
            let actor = logic.find_object_by_id(ID).unwrap();
            let world = receiving.services();
            assert!(Arc::ptr_eq(
                &actor,
                &world.object_registry().get_object(ID).unwrap()
            ));
            assert!(world.object_registry().get_object(BIRTH).is_none());
            assert_eq!(logic.iter_all_object_ids().collect::<Vec<_>>(), [ID]);
            assert_eq!(logic.next_object_id, BIRTH + 1);
            assert_eq!(
                world.scripts().engine().read().unwrap().as_ref().unwrap().get_frame_object_count_changed(),
                FRAME
            );
            assert_eq!(team.read().unwrap().get_members(), [ID]);
            assert!(actor.read().unwrap().is_admitted_to_team(&team));
            assert_eq!(
                neutral
                    .read()
                    .unwrap()
                    .get_resource_manager()
                    .unwrap()
                    .get_supply_warehouses(),
                &[BIRTH]
            );
            let factory = world.logic_managers().object_factory.read().unwrap();
            assert!(Arc::ptr_eq(
                &actor,
                &factory.get_object(ID).unwrap().get_base_object().unwrap()
            ));
            assert!(factory.get_object(BIRTH).is_none());
            assert!(Arc::ptr_eq(
                &actor,
                &world
                    .objects()
                    .read()
                    .unwrap()
                    .with_object(ID, |object| object.base())
                    .unwrap()
            ));
            assert_eq!(
                logic
                    .partition_manager()
                    .host
                    .find_objects_in_radius(pose, 1.0),
                [ID]
            );
            assert_eq!(
                logic
                    .partition_manager()
                    .find_objects_in_radius(&pose, 1.0, &[]),
                [ID]
            );
            assert!(!logic.partition_manager().contains_object(BIRTH));
            assert_eq!(
                world.w3d_ghost_objects().read().unwrap().used()[0].parent_object_id(),
                Some(ID)
            );
            let drawable = TheGameClient::get_drawable_arc_in(world, DRAW).unwrap();
            assert_eq!(drawable.read().unwrap().get_object_id(), ID);
            assert!(TheGameClient::get_drawable_arc_in(world, BIRTH_DRAW).is_none());
            assert_eq!(
                world
                    .scripts()
                    .named_objects()
                    .get_object_id("CoreSavedWarehouse")
                    .unwrap(),
                Some(ID)
            );
            assert_eq!(
                peer_logic
                    .lock()
                    .unwrap()
                    .iter_all_object_ids()
                    .collect::<Vec<_>>(),
                [ID]
            );
            assert!(Arc::ptr_eq(
                &peer_actor,
                &OBJECT_REGISTRY.get_object(ID).unwrap()
            ));
            assert!(
                peer_neutral
                    .read()
                    .unwrap()
                    .get_resource_manager()
                    .unwrap()
                    .get_supply_warehouses()
                    .is_empty()
            );
        });
    }

    #[test]
    fn core_object_restore_requires_the_receiving_neutral_team() {
        const ID: ObjectID = 91_360;
        const BIRTH: ObjectID = 416;
        let receiving = Arc::new(EngineStores::new_for_world());
        let peer = Arc::new(EngineStores::new_for_world());
        let authored = "Object RestoreRequiresNeutral\n KindOf = IMMOBILE\nEnd\n";
        let team = |owner: &EngineStores| {
            let mut factory = owner.services().team_factory().lock().unwrap();
            let prototype = factory
                .init_team_owned("SnapshotTeam".into(), "".into(), false, None, None)
                .unwrap();
            factory.create_team_on_prototype_with_id(&prototype, 91_361).unwrap()
        };

        let receiving_team = team(&receiving);
        with_active_stores(&receiving, || {
            assert_eq!(load_templates_from_ini_text(authored, "receiving.ini"), 1);
        });
        let saved_file = std::env::temp_dir().join(format!(
            "generals-core-load-without-neutral-{}.bin",
            std::process::id()
        ));

        let mut receiver = GameLogic::new_with_engine_stores(receiving.clone());
        receiver.next_object_id = BIRTH;
        with_active_stores(&peer, || {
            assert_eq!(load_templates_from_ini_text(authored, "peer.ini"), 1);
            let template =
                crate::helpers::TheThingFactory::find_template("RestoreRequiresNeutral").unwrap();
            let peer_team = team(&peer);
            add_fixture_neutral_player(peer.services(), peer_team.clone());
            let actor = Object::new_with_id(
                template,
                ID,
                ObjectStatusMaskType::none(),
                Some(peer_team.clone()),
            )
            .unwrap();
            let mut writer = GameLogic::new_with_engine_stores(peer.clone());
            writer.register_object(actor.clone()).unwrap();
            let mut save = XferSave::new();
            save.open(saved_file.to_string_lossy().into_owned()).unwrap();
            writer
                .xfer_native_snapshot_with_campaign(&mut save, &mut Default::default())
                .unwrap();
            save.close().unwrap();

            let mut assert_rejected = || {
                let mut load = XferLoad::new();
                load.open(saved_file.to_string_lossy().into_owned()).unwrap();
                let result =
                    receiver.xfer_native_snapshot_with_campaign(&mut load, &mut Default::default());
                load.close().unwrap();
                assert_eq!(result, Err(XferStatus::InvalidData));
            };
            assert_rejected();

            let neutral = add_fixture_neutral_player(receiving.services(), receiving_team.clone());
            neutral.write().unwrap().set_default_team(None);
            assert_rejected();

            neutral.write().unwrap().set_default_team(Some(peer_team));
            assert_rejected();

            std::fs::remove_file(&saved_file).unwrap();
            assert_eq!(receiver.get_object_count(), 0);
            assert_eq!(receiver.next_object_id, BIRTH);
            assert!(receiving_team.read().unwrap().get_members().is_empty());
            assert!(receiving.services().object_registry().get_object(ID).is_none());
            assert!(Arc::ptr_eq(
                &actor,
                &peer.services().object_registry().get_object(ID).unwrap(),
            ));
        });
    }

    #[test]
    fn core_object_load_distinguishes_bad_toc_from_missing_template() {
        let world = Arc::new(EngineStores::new_for_world());
        admit_fixture_neutral_player(world.services());
        with_active_stores(&world, || {
            // Seed the catalog so a deliberate miss cannot trigger disk initialization.
            assert_eq!(
                load_templates_from_ini_text("Object PresentTemplate\nEnd\n", "core-toc.ini"),
                1
            );
            let mut logic = GameLogic::new_with_engine_stores(world.clone());
            let missing_template_id: UnsignedShort = 1;
            let unknown_toc_id: UnsignedShort = 2;
            logic.add_toc_entry("MissingTemplate".into(), missing_template_id);
            let path = std::env::temp_dir().join(format!(
                "generals-core-object-toc-{}.bin",
                std::process::id()
            ));
            let next_value = 0x1201_a901u32;
            for (id, block_size, payload, expected) in [
                (unknown_toc_id, 0i32, &[][..], Err(XferStatus::InvalidData)),
                (
                    missing_template_id,
                    -1,
                    &[][..],
                    Err(XferStatus::InvalidData),
                ),
                (missing_template_id, 3, &[1, 2, 3][..], Ok(())),
            ] {
                let mut bytes = id.to_le_bytes().to_vec();
                bytes.extend_from_slice(&block_size.to_le_bytes());
                bytes.extend_from_slice(payload);
                bytes.extend_from_slice(&next_value.to_le_bytes());
                std::fs::write(&path, bytes).unwrap();

                let mut xfer = XferLoad::new();
                xfer.open(path.to_string_lossy().into_owned()).unwrap();
                let status = xfer_game_logic_objects_load(&mut logic, &mut xfer, 1);
                if expected.is_ok() {
                    let mut next = 0;
                    xfer.xfer_unsigned_int(&mut next).unwrap();
                    assert_eq!(next, next_value);
                }
                xfer.close().unwrap();
                assert_eq!(status, expected, "TOC id {id}, block size {block_size}");
                assert_eq!(logic.get_object_count(), 0);
            }
            std::fs::remove_file(path).unwrap();
        });
    }
}
