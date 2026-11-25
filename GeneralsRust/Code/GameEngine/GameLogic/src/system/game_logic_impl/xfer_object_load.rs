/// C++ `GameLogic::xfer` load (GameLogic.cpp:4732-4785):
/// Resolve the TOC ID, skip missing templates, create and restore the object,
/// xferSnapshot, then `addWallPiece` for `KINDOF_WALK_ON_TOP_OF_WALL`.
fn xfer_game_logic_objects_load(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    object_count: UnsignedInt,
) -> Result<(), XferStatus> {
    let world = logic.owning_world_services();
    let teams = world.as_ref().map(|world| world.team_factory());
    let players = player_list();
    let default_team = players
        .read()
        .ok()
        .and_then(|list| list.get_neutral_player())
        .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));

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

        let Some(template) = crate::helpers::TheThingFactory::find_template(&entry.name) else {
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

        let arc = match default_team.as_ref() {
            Some(team) => crate::helpers::TheThingFactory::get()
                .and_then(|factory| {
                    factory.new_object_with_team_handle(template.clone(), team.clone())
                })
                .or_else(|_| {
                    Object::new_with_id(
                        template,
                        INVALID_ID,
                        ObjectStatusMaskType::none(),
                        Some(team.clone()),
                    )
                }),
            None => Object::new_with_id(template, INVALID_ID, ObjectStatusMaskType::none(), None),
        }
        .map_err(|error| {
            warn!("Core Object snapshot could not construct object {id}: {error}");
            XferStatus::InvalidData
        })?;

        {
            let mut obj = arc.write().map_err(|_| XferStatus::InvalidData)?;
            xfer_object_snapshot(
                &mut obj,
                xfer,
                logic.frame,
                Some(CoreObjectLoad { id, teams }),
                &mut logic.objects_changed_trigger_areas,
                &mut logic.frame_objects_changed_trigger_areas,
            )?;
        }
        let walk_on_wall = arc
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::WalkOnTopOfWall))
            .unwrap_or(false);
        let id = logic.register_object(arc).map_err(|error| {
            warn!("Core Object snapshot could not admit object: {error}");
            XferStatus::InvalidData
        })?;
        xfer.end_block()?;
        if walk_on_wall {
            add_loaded_wall_piece(logic, id);
        }
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
mod object_block_tests {
    use super::*;
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use game_engine::XferLoad;
    use game_engine::common::thing::thing_factory::load_templates_from_ini_text;

    #[test]
    fn core_object_load_distinguishes_bad_toc_from_missing_template() {
        let world = Arc::new(EngineStores::new_for_world());
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
