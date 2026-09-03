/// C++ `GameLogic::xfer` load (GameLogic.cpp:4732-4785):
/// findTemplate, skip unknown, `TheThingFactory->newObject(template, defaultTeam)`,
/// xferSnapshot, then `addWallPiece` for `KINDOF_WALK_ON_TOP_OF_WALL`.
fn xfer_game_logic_objects_load(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    object_count: UnsignedInt,
) -> Result<(), XferStatus> {
    let default_team = player_list()
        .read()
        .ok()
        .and_then(|list| list.get_neutral_player())
        .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));

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

        let arc = match default_team.clone() {
            Some(team) => crate::helpers::TheThingFactory::get()
                .ok()
                .and_then(|factory| {
                    factory
                        .new_object_with_team_handle(template.clone(), team.clone())
                        .ok()
                })
                .or_else(|| {
                    Object::new_with_id(
                        template.clone(),
                        crate::common::INVALID_ID,
                        crate::common::ObjectStatusMaskType::none(),
                        Some(team),
                    )
                    .ok()
                }),
            None => Object::new_with_id(
                template,
                crate::common::INVALID_ID,
                crate::common::ObjectStatusMaskType::none(),
                None,
            )
            .ok(),
        };

        let Some(arc) = arc else {
            let _ = xfer.skip(block_size);
            let _ = xfer.end_block();
            continue;
        };

        if let Ok(mut obj) = arc.write() {
            xfer_object_snapshot(&mut obj, xfer);
        }
        let wall_id = arc.read().ok().map(|obj| obj.get_id());
        let walk_on_wall = arc
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::WalkOnTopOfWall))
            .unwrap_or(false);
        let _ = logic.register_object(arc);
        if walk_on_wall {
            if let Some(object_id) = wall_id {
                let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
                    if let Some(pathfinder) = ai.pathfinder() {
                        if let Ok(mut pf) = pathfinder.write() {
                            pf.add_wall_piece(object_id);
                        }
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
