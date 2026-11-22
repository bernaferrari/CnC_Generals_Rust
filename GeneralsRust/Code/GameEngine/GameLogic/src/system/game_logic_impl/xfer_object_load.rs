/// C++ `GameLogic::xfer` load (GameLogic.cpp:4732-4785):
/// findTemplate, skip unknown, `TheThingFactory->newObject(template, defaultTeam)`,
/// xferSnapshot, then `addWallPiece` for `KINDOF_WALK_ON_TOP_OF_WALL`.
fn xfer_game_logic_objects_load(
    logic: &mut GameLogic,
    xfer: &mut dyn Xfer,
    object_count: UnsignedInt,
) -> Result<(), XferStatus> {
    let __player_list_arc = player_list();
        let default_team = __player_list_arc.read()
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

        {
            let mut obj = arc.write().map_err(|_| XferStatus::InvalidData)?;
            xfer_object_snapshot(
                &mut obj,
                xfer,
                logic.frame,
                &mut logic.objects_changed_trigger_areas,
                &mut logic.frame_objects_changed_trigger_areas,
            )?;
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
                add_loaded_wall_piece(logic, object_id);
            }
        }
        let _ = xfer.end_block();
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
    ).map_err(|error| {
        log::error!("cannot rebuild the loaded pathfinding map: {error}");
        XferStatus::InvalidData
    })
}
