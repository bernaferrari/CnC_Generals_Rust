/// C++ `TeamFactory::crc` / `TeamPrototype::crc` / `Team::crc` are empty.
fn xfer_team_factory_crc(_xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    Ok(())
}

/// C++ `TeamFactory::xfer` version 1 (Team.cpp:409): uniqueTeamID, prototype
/// count, then each prototype ID + `xferSnapshot(TeamPrototype)`.
/// Does **not** persist `uniqueTeamPrototypeID`.
fn xfer_team_factory_runtime_state(
    factory: &mut crate::team::TeamFactory,
    xfer: &mut dyn Xfer,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

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
            xfer_team_prototype_snapshot(factory, prototype.as_ref(), xfer)?;
        }
    } else {
        for prototype_id in &mut prototype_ids {
            xfer.xfer_unsigned_int(prototype_id)?;
            let Some(prototype) = factory.find_team_prototype_by_id(*prototype_id) else {
                return Err(XferStatus::InvalidData);
            };
            xfer_team_prototype_snapshot(factory, prototype.as_ref(), xfer)?;
        }
    }

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
    // CPP Team.cpp:1213 follows the admitted prototype instance list.
    let instances = factory.find_team_instances(&proto_name);
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

#[cfg(test)]
mod team_factory_xfer_tests {
    use super::*;
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use game_engine::common::thing::thing_factory::load_templates_from_ini_text;
    use game_engine::{Xfer, XferLoad, XferSave};

    const TEAM: u32 = 92_201;
    const OTHER_TEAM: u32 = 92_202;
    const MEMBER: u32 = 92_101;

    type TeamHandle = Arc<RwLock<crate::team::Team>>;

    fn fixture_teams(world: &EngineStores) -> (TeamHandle, TeamHandle) {
        let mut factory = world.services().team_factory().lock().unwrap();
        let mut instantiate = |name: &str, id| {
            let prototype = factory
                .init_team_owned(name.into(), "".into(), false, None, None)
                .unwrap();
            factory
                .create_team_on_prototype_with_id(&prototype, id)
                .unwrap()
        };
        (
            instantiate("CoreSavedTeam", TEAM),
            instantiate("CoreOtherTeam", OTHER_TEAM),
        )
    }

    #[test]
    fn team_factory_xfer_validates_members_after_receiving_objects_are_restored() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::team_factory_xfer_validates_members_after_receiving_objects_are_restored"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_CORE_TEAM_XFER_CHILD",
        ) {
            return;
        }

        let _common = game_engine::common::common_services::CommonServicesScope::new();
        let file =
            std::env::temp_dir().join(format!("generals-core-team-{}.bin", std::process::id()));
        let authored = "Object RestoredTeamMember\nEnd\n";

        for saved_object_team in [None, Some(OTHER_TEAM), Some(TEAM)] {
            let source = Arc::new(EngineStores::new_for_world());
            let (saved_team, other_team) = fixture_teams(&source);
            let mut source_logic = GameLogic::new_with_engine_stores(source.clone());
            with_active_stores(&source, || {
                assert_eq!(load_templates_from_ini_text(authored, "core-team.ini"), 1);
                let template =
                    crate::helpers::TheThingFactory::find_template("RestoredTeamMember").unwrap();
                let add_object = |logic: &mut GameLogic, id, team: &TeamHandle| {
                    let object = Object::new_with_id(
                        template.clone(),
                        id,
                        ObjectStatusMaskType::none(),
                        Some(team.clone()),
                    )
                    .unwrap();
                    logic.register_object(object).unwrap();
                };
                match saved_object_team {
                    Some(TEAM) => {
                        add_object(&mut source_logic, MEMBER, &saved_team);
                        add_object(&mut source_logic, MEMBER + 1, &saved_team);
                        assert_eq!(
                            saved_team.read().unwrap().get_members(),
                            [MEMBER + 1, MEMBER]
                        );
                    }
                    Some(OTHER_TEAM) => add_object(&mut source_logic, MEMBER, &other_team),
                    None => {}
                    _ => unreachable!(),
                }
                saved_team.write().unwrap().add_member(MEMBER);
                let mut save = XferSave::new();
                save.open(file.to_string_lossy().into_owned()).unwrap();
                TeamFactorySnapshotBridge(source.services().team_factory().clone())
                    .xfer(&mut save)
                    .unwrap();
                source_logic
                    .xfer_native_snapshot_with_campaign(&mut save, &mut Default::default())
                    .unwrap();
                save.close().unwrap();

                if saved_object_team.is_none() {
                    add_object(&mut source_logic, MEMBER, &saved_team);
                }
            });

            let receiving = Arc::new(EngineStores::new_for_world());
            let (team, other) = fixture_teams(&receiving);
            // Deliberately publish the source factory with the same TeamIDs.
            // Both Core chunks must still resolve the receiving match's handles.
            with_active_stores(&source, || {
                assert!(OBJECT_REGISTRY.get_object(MEMBER).is_some());
                let mut logic = GameLogic::new_with_engine_stores(receiving.clone());
                let mut campaign = Default::default();
                let mut world_snapshot = CampaignGameLogicSnapshot {
                    logic: &mut logic,
                    campaign: &mut campaign,
                };
                let mut teams =
                    TeamFactorySnapshotBridge(receiving.services().team_factory().clone());
                let mut load = XferLoad::new();
                load.open(file.to_string_lossy().into_owned()).unwrap();
                teams.xfer(&mut load).unwrap();
                assert_eq!(team.read().unwrap().get_member_count(), 0);
                assert_eq!(other.read().unwrap().get_member_count(), 0);
                world_snapshot.xfer(&mut load).unwrap();
                load.close().unwrap();
                teams.load_post_process().unwrap();
                let status = world_snapshot.load_post_process();
                let expected = if saved_object_team == Some(TEAM) {
                    Ok(())
                } else {
                    Err(XferStatus::InvalidData)
                };
                assert_eq!(status, expected, "saved object team: {saved_object_team:?}");
                let expected_members: &[u32] = if expected.is_ok() {
                    &[MEMBER, MEMBER + 1]
                } else {
                    &[]
                };
                assert_eq!(team.read().unwrap().get_members(), expected_members);
                assert_eq!(
                    other.read().unwrap().get_member_count(),
                    usize::from(saved_object_team == Some(OTHER_TEAM))
                );

                if let Some(id) = saved_object_team {
                    let restored = logic.find_object_by_id(MEMBER).unwrap();
                    let exact_team = if id == TEAM { &team } else { &other };
                    assert!(restored.read().unwrap().is_admitted_to_team(exact_team));
                }
            });
        }
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn core_object_xfer_rejects_duplicate_saved_identity() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::core_object_xfer_rejects_duplicate_saved_identity"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_CORE_DUPLICATE_OBJECT_XFER_CHILD",
        ) {
            return;
        }

        let _common = game_engine::common::common_services::CommonServicesScope::new();
        let source = Arc::new(EngineStores::new_for_world());
        let (team, other_team) = fixture_teams(&source);
        let file = std::env::temp_dir().join(format!(
            "generals-core-duplicate-{}.bin",
            std::process::id()
        ));
        // Registering prepends, so the second object written is the duplicate.
        const FIRST: ObjectID = 0x2f10_a157;
        const SECOND: ObjectID = 0x3f2f_b247;
        with_active_stores(&source, || {
            let authored = "Object RestoredCoreIdentity\nEnd\n";
            assert_eq!(
                load_templates_from_ini_text(authored, "core-identity.ini"),
                1
            );
            let template =
                crate::helpers::TheThingFactory::find_template("RestoredCoreIdentity").unwrap();
            let mut writer = GameLogic::new_with_engine_stores(source.clone());
            for (id, team) in [(SECOND, other_team), (FIRST, team)] {
                let object = Object::new_with_id(
                    template.clone(),
                    id,
                    ObjectStatusMaskType::none(),
                    Some(team),
                )
                .unwrap();
                writer.register_object(object).unwrap();
            }
            let mut save = XferSave::new();
            save.open(file.to_string_lossy().into_owned()).unwrap();
            writer
                .xfer_native_snapshot_with_campaign(&mut save, &mut Default::default())
                .unwrap();
            save.close().unwrap();

            let mut bytes = std::fs::read(&file).unwrap();
            let needle = SECOND.to_le_bytes();
            let mut matching = bytes
                .windows(needle.len())
                .enumerate()
                .filter(|(_, bytes)| *bytes == needle);
            let (offset, _) = matching.next().expect("second object identity in the file");
            assert!(matching.next().is_none());
            bytes[offset..offset + needle.len()].copy_from_slice(&FIRST.to_le_bytes());
            std::fs::write(&file, bytes).unwrap();

            let receiving = Arc::new(EngineStores::new_for_world());
            let (receiving_team, receiving_other) = fixture_teams(&receiving);
            let mut receiver = GameLogic::new_with_engine_stores(receiving);
            let mut load = XferLoad::new();
            load.open(file.to_string_lossy().into_owned()).unwrap();
            let status =
                receiver.xfer_native_snapshot_with_campaign(&mut load, &mut Default::default());
            load.close().unwrap();
            std::fs::remove_file(&file).unwrap();
            assert_eq!(status, Err(XferStatus::InvalidData));
            assert_eq!(receiver.iter_all_object_ids().collect::<Vec<_>>(), [FIRST]);
            assert_eq!(receiving_team.read().unwrap().get_members(), [FIRST]);
            assert!(receiving_other.read().unwrap().get_members().is_empty());
        });
    }
}
