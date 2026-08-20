/// C++ `TeamFactory::crc` / `TeamPrototype::crc` / `Team::crc` are empty.
fn xfer_team_factory_crc(_xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    Ok(())
}

/// C++ `TeamFactory::xfer` version 1 (Team.cpp:409): uniqueTeamID, prototype
/// count, then each prototype ID + `xferSnapshot(TeamPrototype)`.
/// Does **not** persist `uniqueTeamPrototypeID`.
fn xfer_team_factory_runtime_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut factory = get_team_factory()
        .lock()
        .map_err(|_| XferStatus::InvalidData)?;

    let mut next_team_id = factory.get_next_team_id();
    xfer.xfer_unsigned_int(&mut next_team_id)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        factory.set_next_team_id(next_team_id);
    }

    let mut prototype_ids = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        let mut ids = factory
            .list_team_prototypes()
            .into_iter()
            .map(|prototype| prototype.get_id())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    } else {
        Vec::new()
    };
    let mut prototype_count = prototype_ids.len() as u16;
    xfer.xfer_unsigned_short(&mut prototype_count)?;
    if xfer.get_xfer_mode() == XferMode::Load
        && prototype_count as usize != factory.list_team_prototypes().len()
    {
        return Err(XferStatus::InvalidData);
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        for _ in 0..prototype_count {
            let mut prototype_id = 0u32;
            xfer.xfer_unsigned_int(&mut prototype_id)?;
            let Some(prototype) = factory.find_team_prototype_by_id(prototype_id) else {
                return Err(XferStatus::InvalidData);
            };
            xfer_team_prototype_snapshot(&mut factory, prototype.as_ref(), xfer)?;
        }
    } else {
        for prototype_id in &mut prototype_ids {
            xfer.xfer_unsigned_int(prototype_id)?;
            let Some(prototype) = factory.find_team_prototype_by_id(*prototype_id) else {
                return Err(XferStatus::InvalidData);
            };
            xfer_team_prototype_snapshot(&mut factory, prototype.as_ref(), xfer)?;
        }
    }

    Ok(())
}

fn xfer_team_factory_load_post_process() -> Result<(), XferStatus> {
    let mut factory = get_team_factory()
        .lock()
        .map_err(|_| XferStatus::InvalidData)?;
    factory.restore_unique_ids_after_load();
    Ok(())
}

/// C++ `TeamPrototype::xfer` version 2 (Team.cpp:1175).
fn xfer_team_prototype_snapshot(
    factory: &mut crate::team::TeamFactory,
    prototype: &crate::team::TeamPrototype,
    xfer: &mut dyn Xfer,
) -> Result<(), XferStatus> {
    use game_engine::common::system::snapshot::Snapshotable;

    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut owning_player_index = prototype.owning_player_index();
    xfer.xfer_int(&mut owning_player_index)?;

    let mut attack_priority_name = prototype.get_attack_priority_name().to_string();
    if version >= 2 {
        xfer.xfer_ascii_string(&mut attack_priority_name)?;
    }

    let mut always_false = prototype.production_condition_always_false();
    xfer.xfer_bool(&mut always_false)?;

    // C++ `xferSnapshot(&m_teamTemplate)` — TeamTemplateInfo::xfer v1 is only priority.
    {
        let template_version: XferVersion = 1;
        let mut tmpl_version = template_version;
        xfer.xfer_version(&mut tmpl_version, template_version)?;
        let mut production_priority = prototype.get_production_priority();
        xfer.xfer_int(&mut production_priority)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            let mut updated = prototype.clone();
            updated.set_owning_player_index(owning_player_index);
            updated.set_attack_priority_name(attack_priority_name.clone().into());
            updated.set_production_priority(production_priority);
            updated.set_production_condition_always_false(always_false);
            factory.replace_team_prototype(updated);
        }
    }

    let proto_name = prototype.get_name().to_string();
    let mut instances = factory.find_team_instances(&proto_name);
    instances.sort_by_key(|team| team.read().ok().map(|t| t.get_id()).unwrap_or(0));
    let mut instance_count = instances.len() as u16;
    xfer.xfer_unsigned_short(&mut instance_count)?;

    if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        for team_arc in &instances {
            let mut team_id = team_arc.read().map(|t| t.get_id()).unwrap_or(0);
            xfer.xfer_unsigned_int(&mut team_id)?;
            let mut team = team_arc.write().map_err(|_| XferStatus::InvalidData)?;
            let mut bridge = CommonXferBridge { inner: xfer };
            Snapshotable::xfer(&mut *team, &mut bridge).map_err(|_| XferStatus::InvalidData)?;
        }
    } else {
        let live_prototype = factory
            .find_team_prototype(&proto_name)
            .or_else(|| factory.find_team_prototype_by_id(prototype.get_id()))
            .ok_or(XferStatus::InvalidData)?;

        for _ in 0..instance_count {
            let mut team_id = 0u32;
            xfer.xfer_unsigned_int(&mut team_id)?;
            let team_arc = factory
                .find_team_by_id(team_id)
                .or_else(|| {
                    factory.create_team_on_prototype_with_id(live_prototype.as_ref(), team_id)
                })
                .ok_or(XferStatus::InvalidData)?;
            let mut team = team_arc.write().map_err(|_| XferStatus::InvalidData)?;
            let mut bridge = CommonXferBridge { inner: xfer };
            Snapshotable::xfer(&mut *team, &mut bridge).map_err(|_| XferStatus::InvalidData)?;
        }
    }

    Ok(())
}
