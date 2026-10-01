/// C++ `GameLogic::xfer` load (GameLogic.cpp:4732-4785):
/// findTemplate, skip unknown, `TheThingFactory->newObject(template, defaultTeam)`,
/// xferSnapshot, then `addWallPiece` for `KINDOF_WALK_ON_TOP_OF_WALL`.
fn xfer_game_logic_objects_load(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    object_count: UnsignedInt,
) -> Result<(), XferStatus> {
    let default_team_id = player_list().read().ok().and_then(|list| {
        list.get_neutral_player()
            .and_then(|player| player.get_default_team_id())
    });

    for _ in 0..object_count {
        let mut toc_id: UnsignedShort = 0;
        xfer.xfer_unsigned_short(&mut toc_id)?;
        let block_size = xfer.begin_block()?;
        let toc_name = logic.find_toc_entry_by_id(toc_id).map(|e| e.name.clone());
        let Some(toc_name) = toc_name else {
            let _ = xfer.skip(block_size);
            let _ = xfer.end_block();
            continue;
        };

        let Some(template) = crate::helpers::TheThingFactory::find_template(&toc_name) else {
            // C++: unrecognized template → skip(block) and continue. Never stub.
            let _ = xfer.skip(block_size);
            let _ = xfer.end_block();
            continue;
        };

        let built = Object::new_with_id(
            template,
            crate::common::INVALID_ID,
            crate::common::ObjectStatusMaskType::none(),
            None,
        )
        .ok();
        let Some(mut object) = built else {
            let _ = xfer.skip(block_size);
            let _ = xfer.end_block();
            continue;
        };
        let _ = object.set_team_id(default_team_id);
        xfer_object_snapshot(&mut object, xfer);
        let wall_id = object.get_id();
        let walk_on_wall = object.is_kind_of(KindOf::WalkOnTopOfWall);
        OBJECT_REGISTRY.register_object(wall_id, object);
        let _ = logic.register_object(wall_id);
        if walk_on_wall {
            let ai_store = the_ai();
            if let Ok(ai) = ai_store.read() {
                if let Some(pathfinder) = ai.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        pf.add_wall_piece(wall_id);
                    }
                }
            }
        }
        let _ = xfer.end_block();
    }
    Ok(())
}

fn pathfinder_new_map_after_polygon_load() {
    // C++ GameLogic.cpp:4880 `TheAI->pathfinder()->newMap()` after trigger restore.
    let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
        if let Some(pathfinder) = ai.pathfinder() {
            if let Ok(mut pf) = pathfinder.write() {
                if let Ok(terrain) = get_terrain_logic().read() {
                    pf.rebuild_from_terrain(&terrain);
                }
            }
        }
    }
}
