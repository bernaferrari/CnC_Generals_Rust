use super::*;
use crate::common::{DefaultThingTemplate, KindOf};
use crate::player::{Player, PlayerList, player_list};
use crate::team::Team;

pub(crate) fn complete_dozer_for_test(
    builder: &Arc<RwLock<Object>>,
    structure: &Arc<RwLock<Object>>,
    rebuild: bool,
) {
    let mut dozer = DozerAIUpdate::new(
        game_engine::common::thing::module::ModuleDataHandle::new(DozerAIUpdateModuleData::default()),
        builder.read().unwrap().get_id(),
    );
    dozer.handle_build_completion(builder, structure, rebuild, &mut None).unwrap();
}

pub(crate) struct CompletionFixture {
    owner: crate::system::game_logic::GameLogic,
    object_ids: [crate::common::ObjectID; 2],
    previous_players: Option<PlayerList>,
    previous_objects: Option<crate::object_manager::ObjectManager>,
    previous_script: Option<crate::scripting::engine::ScriptEngine>,
    previous_names: Option<crate::scripting::events::NamedObjectTrackerState>,
    previous_ai: Option<crate::ai::integration::AiIntegrationManager>,
    pub(crate) player: Arc<RwLock<Player>>,
    pub(crate) builder: Arc<RwLock<Object>>,
    pub(crate) structure: Arc<RwLock<Object>>,
}

impl CompletionFixture {
    pub(crate) fn new(power: i32) -> Self {
        Self::with_initial_disabled(power, false)
    }

    fn with_initial_disabled(power: i32, disabled: bool) -> Self {
        // Never replace an unrelated object or team admission in the adapter.
        let mut team_id = 0xBA_0300;
        loop {
            let objects_unused = [team_id + 1, team_id + 2].into_iter().all(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .get_object(id)
                    .is_none()
            });
            let published_team_factory = crate::team::get_team_factory();
            let team_unused = published_team_factory
                .lock()
                .unwrap()
                .find_team_by_id(team_id)
                .is_none();
            if objects_unused && team_unused {
                break;
            }
            team_id += 3;
        }
        let object_ids = [team_id + 1, team_id + 2];
        let previous_objects = Some(std::mem::replace(
            &mut *crate::object_manager::get_object_manager().write().unwrap(),
            crate::object_manager::ObjectManager::new(),
        ));
        let previous_names = Some(
            crate::scripting::engine::get_named_object_tracker().take_state_for_world_boundary(),
        );
        let previous_script = std::mem::replace(
            &mut *crate::scripting::engine::get_script_engine()
                .write()
                .unwrap(),
            Some(crate::scripting::engine::ScriptEngine::new_in_published_world().unwrap()),
        );
        let previous_ai = crate::ai::integration::replace_ai_integration_for_world_boundary(Some(
            crate::ai::integration::AiIntegrationManager::new(),
        ));
        let player = Arc::new(RwLock::new(Player::new(0)));
        let mut players = PlayerList::new();
        players.add_player(player.clone());
        players.set_local_player_index(0);
        let previous_players = Some(std::mem::replace(
            &mut *player_list().write().unwrap(),
            players,
        ));
        let team = Arc::new(RwLock::new(Team::new("CompletionTeam".into(), team_id)));
        team.write().unwrap().set_controlling_player_id(Some(0));
        let mut template = DefaultThingTemplate::new("CompletionStructure".into());
        template.add_kind_of(KindOf::Structure);
        template.add_kind_of(KindOf::Score);
        template.add_kind_of(KindOf::Immobile);
        if power < 0 {
            template.add_kind_of(KindOf::Powered);
        }
        template.set_energy_production(power);
        template
            .parse_object_fields_from_ini(&std::collections::HashMap::from([(
                "BuildCost".to_owned(),
                "700".to_owned(),
            )]))
            .expect("valid template fixture");
        let builder = Arc::new(RwLock::new(Object::new_test(object_ids[0], 100.0)));
        let structure = Arc::new(RwLock::new(Object::new_test_from_template(
            object_ids[1],
            100.0,
            Arc::new(template),
        )));
        // Establish the restore guard before admission, team callbacks, or
        // assertions. Constructing this local owner publishes nothing.
        let mut fixture = Self {
            owner: crate::system::game_logic::GameLogic::new(),
            object_ids,
            previous_players,
            previous_objects,
            previous_ai,
            previous_script,
            previous_names,
            player,
            builder,
            structure,
        };
        // This control starts disabled before any controller is assigned.
        // CPP Object::onDisabledEdge intentionally has no construction guard;
        // disabling an already admitted unfinished producer is another effect,
        // not this initial-state/completion test.
        if disabled {
            fixture
                .structure
                .write()
                .unwrap()
                .set_disabled(crate::common::DisabledType::Held);
        }
        // C++ admits construction status before the first team membership:
        // unfinished producers/consumers have no energy influence yet.
        fixture.structure.write().unwrap().set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::UnderConstruction,
            ),
            true,
        );
        for object in [fixture.builder.clone(), fixture.structure.clone()] {
            fixture.owner.register_object(object.clone()).unwrap();
            let assigned = object.write().unwrap().set_team(Some(team.clone()));
            assigned.expect("fixture team assignment");
        }
        fixture
            .structure
            .write()
            .unwrap()
            .set_name("CompletionScriptName".into());
        // Admission precedes naming: human completion does not eagerly cache names.
        assert_eq!(
            crate::scripting::engine::get_named_object_tracker()
                .get_object_id("CompletionScriptName")
                .unwrap(),
            None
        );
        fixture
    }

    fn retire(&mut self) -> Result<(), String> {
        let mut admitted = Vec::new();
        for (id, expected) in self
            .object_ids
            .into_iter()
            .zip([self.builder.clone(), self.structure.clone()])
        {
            // A constructor panic may have admitted only the first object.
            if let Some(actual) = self.owner.find_object_by_id(id) {
                if !Arc::ptr_eq(&actual, &expected) {
                    return Err(format!("completion fixture {id} owner identity mismatch"));
                }
                admitted.push((id, expected));
            }
        }
        let detached = {
            let manager_handle = crate::object_manager::get_object_manager();
            let mut manager = manager_handle
                .write()
                .map_err(|_| "completion fixture manager poisoned".to_string())?;
            let mut detached = Vec::new();
            for (id, expected) in &admitted {
                if manager.get_object(*id).is_some() {
                    let slot = manager.detach_fixture_object_slot(*id, expected, &self.owner)?;
                    detached.push((*id, slot, expected.clone()));
                }
            }
            detached
        };
        // C++ Object.cpp579-607/Energy.cpp130-151: team removal must still
        // resolve the original player. No external object/player/module guard
        // spans the driving owner's synchronous deletion callbacks.
        for (id, _) in &admitted {
            self.owner.destroy_object(*id);
        }
        self.owner
            .cleanup_dead_objects()
            .map_err(|error| format!("fixture cleanup: {error:?}"))?;
        {
            let manager_handle = crate::object_manager::get_object_manager();
            let manager = manager_handle
                .read()
                .map_err(|_| "completion fixture manager poisoned".to_string())?;
            for (id, slot, expected) in detached {
                manager.finish_fixture_object_slot_retirement(id, slot, &expected, &self.owner)?;
            }
        }
        for (id, expected) in &admitted {
            crate::ai::object_registry::unregister_legacy_object(*id);
            if self.owner.find_object_by_id(*id).is_some()
                || crate::object::registry::OBJECT_REGISTRY
                    .get_object(*id)
                    .is_some()
            {
                return Err(format!("completion fixture {id} remains admitted"));
            }
            let finalized = expected
                .read()
                .map_err(|_| format!("completion fixture {id} poisoned"))?;
            if finalized.get_id() != crate::common::INVALID_ID || finalized.get_team().is_some() {
                return Err(format!("completion fixture {id} was not finalized"));
            }
        }
        Ok(())
    }

    pub(crate) fn complete(&self, rebuild: bool) {
        complete_dozer_for_test(&self.builder, &self.structure, rebuild);
    }

    pub(crate) fn frame(&self) -> u32 {
        self.owner.get_frame()
    }

    pub(crate) fn request_structure_destruction(&mut self) {
        let id = self.object_ids[1];
        let admitted = self
            .owner
            .find_object_by_id(id)
            .expect("admitted structure");
        assert!(Arc::ptr_eq(&admitted, &self.structure));
        // C++ GameLogic.cpp3935-3967 queues destruction before onDestroy.
        self.owner.destroy_object(id);
    }

    pub(crate) fn finish_structure_destruction(&mut self) {
        // C++ GameLogic.cpp2499-2506 removes the lookup before the destructor.
        self.owner
            .cleanup_dead_objects()
            .expect("structure cleanup");
        assert!(self.owner.find_object_by_id(self.object_ids[1]).is_none());
    }
}

impl Drop for CompletionFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = self.retire();
            result.expect("exact completion fixture retirement");
        }));
        *crate::scripting::engine::get_script_engine()
            .write()
            .unwrap() = self.previous_script.take();
        crate::scripting::engine::get_named_object_tracker()
            .replace_state_for_world_boundary(self.previous_names.take().unwrap());
        crate::ai::integration::replace_ai_integration_for_world_boundary(self.previous_ai.take());
        *crate::object_manager::get_object_manager().write().unwrap() =
            self.previous_objects.take().unwrap();
        *player_list().write().unwrap() = self.previous_players.take().unwrap();
        if let Err(error) = result {
            if unwinding {
                eprintln!("completion fixture retirement failed during unwind");
            } else {
                std::panic::resume_unwind(error);
            }
        }
    }
}

#[test]
fn completion_fixture_retires_retained_queries_before_restoring_player_services() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let previous = CompletionFixture::new(30);
    let (previous_id, template, position) = {
        let object = previous.structure.read().unwrap();
        (
            object.get_id(),
            object.get_template().clone(),
            *object.get_position(),
        )
    };
    let instance = crate::object_manager::GameObjectInstance::from_existing(
        previous.structure.clone(),
        Some(template),
    );
    let admitted = crate::object_manager::get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(instance, position);
    admitted.expect("actual previous manager admission");
    previous.complete(false);
    let previous_power = previous.player.read().unwrap().get_energy().production();
    let fixture = CompletionFixture::new(10);
    fixture.complete(false);
    let ids = fixture.object_ids;
    let queries = [fixture.builder.clone(), fixture.structure.clone()];
    assert!(Arc::ptr_eq(
        &fixture.owner.find_object_by_id(ids[1]).unwrap(),
        &queries[1]
    ));
    drop(fixture);

    let facts: Vec<_> = queries
        .iter()
        .map(|query| {
            let object = query.read().unwrap();
            (object.get_id(), object.get_team().is_none())
        })
        .collect();
    assert_eq!(facts, vec![(crate::common::INVALID_ID, true); 2]);
    for id in ids {
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(id)
                .is_none()
        );
    }
    assert!(Arc::ptr_eq(
        &previous.owner.find_object_by_id(previous_id).unwrap(),
        &previous.structure
    ));
    let restored_registry = crate::object::registry::OBJECT_REGISTRY
        .get_object(previous_id)
        .unwrap();
    assert!(Arc::ptr_eq(&restored_registry, &previous.structure));
    let restored_slot = {
        let manager_handle = crate::object_manager::get_object_manager();
        let manager = manager_handle.read().unwrap();
        manager.with_object(previous_id, |object| object.base())
    }
    .expect("restored previous manager slot");
    assert!(Arc::ptr_eq(&restored_slot, &previous.structure));
    assert_eq!(
        previous.player.read().unwrap().get_energy().production(),
        previous_power
    );
    drop(queries);
    assert_eq!(
        previous.player.read().unwrap().get_energy().production(),
        previous_power
    );
}

#[test]
fn dozer_completion_applies_power_without_reentering_player_lock() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(10);
    let before = fixture.player.read().unwrap().get_energy().production();
    fixture.complete(false);
    assert_eq!(
        fixture.player.read().unwrap().get_energy().production(),
        before + 10
    );
    assert!(!fixture.structure.read().unwrap().is_under_construction());
}

#[test]
fn ai_dozer_completion_finishes_build_list_without_reentering_player_lock() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    let mut info = crate::build_list_info::BuildListInfo::new();
    info.set_object_id(fixture.structure.read().unwrap().get_id());
    info.set_under_construction(true);
    info.set_building_name("FinishedAiStructure".into());
    info.set_health(75);
    fixture.player.write().unwrap().set_build_list(Some(info));
    crate::ai::integration::with_ai_integration_mut(|manager| manager.ensure_ai_player(0, false))
        .unwrap();
    fixture.complete(false);
    assert!(
        !fixture
            .player
            .read()
            .unwrap()
            .get_build_list()
            .unwrap()
            .is_under_construction()
    );
    assert_eq!(
        fixture.structure.read().unwrap().get_name().as_str(),
        "FinishedAiStructure"
    );
}

fn roundtrip_player(player: &mut Player) -> Player {
    use game_engine::system::{snapshot::Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut bytes = Vec::new();
    player
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    let mut loaded = Player::new(player.get_player_index());
    loaded
        .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    loaded
}

#[test]
fn dozer_completion_scores_notifies_scripts_and_survives_player_xfer() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    for rebuild in [false, true] {
        let fixture = CompletionFixture::new(0);
        fixture.complete(rebuild);
        let player = fixture.player.read().unwrap();
        assert_eq!(
            player.get_score_keeper().get_total_money_spent(),
            if rebuild { 0 } else { 700 }
        );
        assert_eq!(
            player.get_score_keeper().get_total_buildings_built(),
            if rebuild { 0 } else { 1 }
        );
        drop(player);
        assert_eq!(
            crate::scripting::engine::get_named_object_tracker()
                .get_object_id("CompletionScriptName")
                .unwrap(),
            None // C++ notification only invalidates frame-keyed conditions.
        );
        assert_eq!(
            crate::scripting::engine::get_script_engine()
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .get_frame_object_count_changed(),
            crate::helpers::TheGameLogic::get_frame() as u32
        );
        let mut player = fixture.player.write().unwrap();
        let loaded = roundtrip_player(&mut player);
        assert_eq!(
            loaded.get_score_keeper().get_total_money_spent(),
            if rebuild { 0 } else { 700 }
        );
        assert_eq!(
            loaded.get_score_keeper().get_total_buildings_built(),
            if rebuild { 0 } else { 1 }
        );
        assert_eq!(
            loaded.get_energy().production(),
            player.get_energy().production()
        );
        assert_eq!(
            loaded.get_energy().consumption(),
            player.get_energy().consumption()
        );
    }
}

#[test]
fn construction_influence_preserves_consumers_and_disabled_producers() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    for (power, disabled) in [(10, false), (10, true), (-10, false), (-10, true)] {
        let fixture = CompletionFixture::with_initial_disabled(power, disabled);
        let (production, consumption) = {
            let player = fixture.player.read().unwrap();
            (
                player.get_energy().production(),
                player.get_energy().consumption(),
            )
        };
        fixture.complete(true);
        let player = fixture.player.read().unwrap();
        assert_eq!(
            player.get_energy().production(),
            production + if power > 0 && !disabled { power } else { 0 }
        );
        assert_eq!(
            player.get_energy().consumption(),
            consumption + if power < 0 { -power } else { 0 }
        );
    }
}

#[test]
fn restored_ai_construction_notifies_even_without_a_builder() {
    use crate::ai::integration::{IntegratedAiPlayer, with_ai_integration_mut};
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    let mut info = crate::build_list_info::BuildListInfo::new();
    info.set_object_id(fixture.structure.read().unwrap().get_id());
    info.set_under_construction(true);
    info.set_building_name("RestoredAiStructure".into());
    info.set_health(75);
    fixture.player.write().unwrap().set_build_list(Some(info));
    with_ai_integration_mut(|manager| {
        manager.ensure_ai_player(0, false);
        manager
            .with_ai_player_mut(0, |ai| {
                let IntegratedAiPlayer::Standard(ai) = ai else {
                    panic!("expected standard AI");
                };
                ai.set_team_delay_frames(99);
                ai.set_build_delay_frames(80);
            })
            .unwrap();
    })
    .unwrap();
    let loaded = roundtrip_player(&mut fixture.player.write().unwrap());
    *fixture.player.write().unwrap() = loaded;
    fixture.structure.write().unwrap().clear_status(
        crate::common::ObjectStatusMaskType::from_status(
            crate::common::ObjectStatusTypes::UnderConstruction,
        ),
    );
    Player::on_structure_construction_complete(
        &fixture.player,
        None,
        &fixture.structure,
        true,
    ).unwrap();
    assert!(
        !fixture
            .player
            .read()
            .unwrap()
            .get_build_list()
            .unwrap()
            .is_under_construction()
    );
    assert_eq!(
        fixture.structure.read().unwrap().get_name().as_str(),
        "RestoredAiStructure"
    );
    assert_eq!(fixture.structure.read().unwrap().get_health(), 75.0);
    with_ai_integration_mut(|manager| {
        manager
            .with_ai_player_mut(0, |ai| {
                let IntegratedAiPlayer::Standard(ai) = ai else {
                    panic!("expected standard AI");
                };
                assert_eq!(ai.get_team_delay(), 0);
                assert_eq!(ai.get_build_delay(), 0);
            })
            .unwrap()
    })
    .unwrap();
    let loaded = roundtrip_player(&mut fixture.player.write().unwrap());
    assert!(!loaded.get_build_list().unwrap().is_under_construction());
    assert_eq!(loaded.get_score_keeper().get_total_money_spent(), 0);
}

#[test]
fn completed_powered_structure_brownout_releases_player_before_disabled_callbacks() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(-10);
    let (template, position) = {
        let structure = fixture.structure.read().unwrap();
        (
            structure.get_template().clone(),
            *structure.get_position(),
        )
    };
    let instance = crate::object_manager::GameObjectInstance::from_existing(
        fixture.structure.clone(),
        Some(template),
    );
    crate::object_manager::get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(instance, position)
        .unwrap();
    fixture.complete(false);
    assert!(
        fixture
            .structure
            .read()
            .unwrap()
            .is_disabled_by_type(crate::common::DisabledType::DisabledUnderpowered)
    );
}

/// C++ Player.cpp:1644-1645: completion removes then re-inserts the full
/// structure footprint on the receiving pathfinder only.
#[test]
fn construction_complete_reclassifies_footprint_on_receiving_pathfinder_only() {
    use crate::ai::Pathfinder;
    use crate::ai::pathfind_complete::PathfindCellType;
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    let center = crate::common::Coord3D::new(105.0, 105.0, 0.0);
    {
        let mut structure = fixture.structure.write().unwrap();
        structure.set_position(&center).unwrap();
        let mut geometry = crate::common::GeometryInfo::default();
        geometry.set_bounds(crate::common::AABox {
            min: crate::common::Coord3D::new(-25.0, -25.0, 0.0),
            max: crate::common::Coord3D::new(25.0, 25.0, 10.0),
        });
        structure.set_geometry_info(geometry);
    }
    let structure_id = fixture.structure.read().unwrap().get_id();
    let foreign_cell = crate::common::Coord3D::new(125.0, 105.0, 0.0);
    let rubble_cell = crate::common::Coord3D::new(95.0, 105.0, 0.0);
    let center_cell = center;
    let outside = crate::common::Coord3D::new(255.0, 255.0, 0.0);

    let owner = crate::system::engine_stores::new_for_world();
    let peer = crate::system::engine_stores::new_for_world();
    fixture.structure.write().unwrap().bind_pathfinding_owner(&owner).unwrap();
    let receiving = owner.ai().read().unwrap().pathfinder().unwrap();
    let other = peer.ai().read().unwrap().pathfinder().unwrap();
    receiving.write().unwrap().reset_with_size(40, 40);
    other.write().unwrap().reset_with_size(40, 40);
    // The foreign obstacle (a different object id) overlaps the footprint.
    let foreign = {
        let template = fixture.structure.read().unwrap().get_template().clone();
        let mut object = Object::new_test_from_template(structure_id + 100, 100.0, template);
        object.set_position(&foreign_cell).unwrap();
        let mut geometry = crate::common::GeometryInfo::default();
        geometry.set_bounds(crate::common::AABox {
            min: crate::common::Coord3D::new(-4.0, -4.0, 0.0),
            max: crate::common::Coord3D::new(4.0, 4.0, 10.0),
        });
        object.set_geometry_info(geometry);
        object
    };
    receiving.write().unwrap().add_object_to_pathfind_map(&foreign).unwrap();
    // Rubble left over from clearing the site before building.
    receiving.write().unwrap().set_cell_type_for_test(&rubble_cell, PathfindCellType::Rubble);
    // The peer's same-ID object has a different footprint.
    let peer_structure = {
        let structure = fixture.structure.read().unwrap();
        let mut object = Object::new_test_from_template(
            structure_id,
            100.0,
            structure.get_template().clone(),
        );
        object.set_position(&outside).unwrap();
        object.set_geometry_info(structure.get_geometry_info().clone());
        object.bind_pathfinding_owner(&peer).unwrap();
        Arc::new(RwLock::new(object))
    };
    let cell = |pf: &Pathfinder, pos: &crate::common::Coord3D| {
        pf.get_cell_type_at(pos)
    };
    assert_eq!(cell(&receiving.read().unwrap(), &foreign_cell), Some(PathfindCellType::Obstacle));
    assert_eq!(cell(&other.read().unwrap(), &center_cell), Some(PathfindCellType::Clear));

    // The completion caller must report a reentrant pathfinder borrow rather
    // than dropping removal and continuing with insertion or later phases.
    {
        let held = receiving.write().unwrap();
        let error = Player::on_structure_construction_complete(
            &fixture.player, None, &fixture.structure, false,
        ).unwrap_err();
        assert!(error.contains("cannot borrow construction pathfinder"));
        assert_eq!(cell(&held, &center_cell), Some(PathfindCellType::Clear));
        assert_eq!(cell(&other.read().unwrap(), &center_cell), Some(PathfindCellType::Clear));
        assert_eq!(
            fixture.player.read().unwrap().get_score_keeper().get_total_money_spent(),
            0,
            "a failed footprint removal must not reach the score phase"
        );
    }

    // Publishing a different world must not redirect the receiving callback.
    crate::system::engine_stores::with_world_services(peer.services(), || {
        Player::on_structure_construction_complete(
            &fixture.player,
            None,
            &fixture.structure,
            true,
        ).unwrap();
    });

    {
        let receiving = receiving.read().unwrap();
        let other = other.read().unwrap();
        assert_eq!(cell(&receiving, &center_cell), Some(PathfindCellType::Obstacle));
        assert_eq!(cell(&receiving, &rubble_cell), Some(PathfindCellType::Obstacle));
        assert_eq!(cell(&receiving, &foreign_cell), Some(PathfindCellType::Obstacle));
        assert_eq!(cell(&receiving, &outside), Some(PathfindCellType::Clear));
        assert_eq!(cell(&other, &center_cell), Some(PathfindCellType::Clear));
        assert_eq!(cell(&other, &rubble_cell), Some(PathfindCellType::Clear));
        // Only the foreign owner's ID may be ignored at the overlapping cell.
        let ground = crate::ai::pathfind_complete::SURFACE_GROUND;
        assert!(!receiving.valid_movement_position_for_surfaces(
            ground, false, &foreign_cell, Some(structure_id),
        ));
        assert!(receiving.valid_movement_position_for_surfaces(
            ground, false, &foreign_cell, Some(structure_id + 100),
        ));
    }
    crate::system::engine_stores::with_world_services(owner.services(), || {
        Player::on_structure_construction_complete(&fixture.player, None, &peer_structure, true).unwrap();
    });
    assert_eq!(cell(&other.read().unwrap(), &outside), Some(PathfindCellType::Obstacle));
    assert_eq!(cell(&other.read().unwrap(), &center_cell), Some(PathfindCellType::Clear));
    assert_eq!(cell(&receiving.read().unwrap(), &outside), Some(PathfindCellType::Clear));
}

#[test]
fn dozer_completion_callback_keeps_same_id_peer_structure_and_footprint_untouched() {
    use crate::ai::pathfind_complete::PathfindCellType;

    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    let driving = crate::system::engine_stores::new_for_world();
    let peer = crate::system::engine_stores::new_for_world();
    let roster = player_list();
    let driving_frame = 37;
    let peer_initial_script_frame = 174;
    for (world, frame, script_frame) in [
        (&driving, driving_frame, 13),
        (&peer, 251, peer_initial_script_frame),
    ] {
        world.game_logic_mutex().lock().unwrap().set_current_frame(frame);
        let engine = crate::scripting::engine::ScriptEngine::new_with_services(world.services())
            .unwrap();
        engine.set_frame_object_count_changed(script_frame);
        *world.services().scripts().engine().write().unwrap() = Some(engine);
    }
    let position = crate::common::Coord3D::new(105.0, 105.0, 0.0);
    let peer_position = crate::common::Coord3D::new(255.0, 255.0, 0.0);
    let mut geometry = crate::common::GeometryInfo::default();
    geometry.set_bounds(crate::common::AABox {
        min: crate::common::Coord3D::new(-25.0, -25.0, 0.0),
        max: crate::common::Coord3D::new(25.0, 25.0, 10.0),
    });
    let (id, template) = {
        let mut structure = fixture.structure.write().unwrap();
        structure.bind_rank_roster(&roster).unwrap();
        structure.bind_pathfinding_owner(&driving).unwrap();
        structure.set_position(&position).unwrap();
        structure.set_geometry_info(geometry.clone());
        structure.set_construction_percent(50.0);
        (structure.get_id(), structure.get_template().clone())
    };
    let mut other_structure = Object::new_test_from_template(id, 100.0, template);
    other_structure.bind_pathfinding_owner(&peer).unwrap();
    other_structure.set_position(&peer_position).unwrap();
    other_structure.set_geometry_info(geometry);
    other_structure.set_status(
        crate::common::ObjectStatusMaskType::from_status(
            crate::common::ObjectStatusTypes::UnderConstruction,
        ),
        true,
    );
    other_structure.set_construction_percent(25.0);
    let other_structure = Arc::new(RwLock::new(other_structure));
    let peer_logic = peer.game_logic_mutex();
    let driving_path = driving.ai().read().unwrap().pathfinder().unwrap();
    let peer_path = peer.ai().read().unwrap().pathfinder().unwrap();
    driving_path.write().unwrap().reset_with_size(40, 40);
    peer_path.write().unwrap().reset_with_size(40, 40);
    peer_path
        .write()
        .unwrap()
        .add_object_to_pathfind_map(&other_structure.read().unwrap())
        .unwrap();
    crate::system::engine_stores::with_active_stores(&peer, || {
        let mut logic = peer_logic.lock().unwrap();
        logic.register_object(other_structure.clone()).unwrap();
        assert!(Arc::ptr_eq(
            &logic.find_object_by_id(id).unwrap(),
            &other_structure,
        ));
        drop(logic);
        assert!(Arc::ptr_eq(
            &crate::object::registry::OBJECT_REGISTRY.get_object(id).unwrap(),
            &other_structure,
        ));
        // Invoke the real Dozer completion callback, with the peer's registry
        // active and containing a different structure with the identical ID.
        fixture.complete(false);
    });

    assert!(!fixture.structure.read().unwrap().is_under_construction());
    assert_eq!(
        fixture.player.read().unwrap().get_score_keeper().get_total_money_spent(),
        700
    );
    {
        let other = other_structure.read().unwrap();
        assert!(other.is_under_construction());
        assert_eq!(other.get_construction_percent(), 25.0);
    }
    assert_eq!(
        driving_path.read().unwrap().get_cell_type_at(&position),
        Some(PathfindCellType::Obstacle)
    );
    assert_eq!(
        peer_path.read().unwrap().get_cell_type_at(&position),
        Some(PathfindCellType::Clear)
    );
    assert_eq!(
        peer_path.read().unwrap().get_cell_type_at(&peer_position),
        Some(PathfindCellType::Obstacle)
    );
    let script_frame = |world: &crate::system::engine_stores::EngineStores| {
        world.services().scripts().engine().read().unwrap().as_ref().unwrap()
            .get_frame_object_count_changed()
    };
    // Read before retirement, which has its own original script invalidation.
    let observed_script_frames = (script_frame(&driving), script_frame(&peer));
    crate::system::engine_stores::with_active_stores(&peer, || {
        let mut logic = peer_logic.lock().unwrap();
        logic.destroy_object(id);
        logic.cleanup_dead_objects().unwrap();
    });
    assert_eq!(
        observed_script_frames,
        (driving_frame as u32, peer_initial_script_frame),
        "C++ completion invalidates the receiving ScriptEngine at the receiving frame",
    );
}

fn admit_vision_structure(
    world: &Arc<crate::system::engine_stores::EngineStores>,
    position: crate::common::Coord3D,
    frame: u64,
    unlook_duration: u32,
) -> Arc<RwLock<Object>> {
    use crate::helpers::TheThingFactory;
    use crate::system::engine_stores::with_active_stores;
    use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};

    with_active_stores(world, || {
        world.services().common().system.runtime_global_data.write()
            .unlook_persist_duration = unlook_duration;
        world.services().shroud().lock().unwrap().init_shroud_grid(400., 400.);
        world.ai().read().unwrap().pathfinder().unwrap().write().unwrap()
            .reset_with_size(40, 40);
        let team = Arc::new(RwLock::new(Team::new("StructureOwner".into(), 0xBA_0440)));
        team.write().unwrap().set_controlling_player_id(Some(0));
        let mut controller = Player::new(0);
        controller.set_default_team(Some(Arc::clone(&team)));
        {
            let mut players = world.services().players().write().unwrap();
            players.add_player(Arc::new(RwLock::new(controller)));
            players.add_player(Arc::new(RwLock::new(Player::new(1))));
        }

        assert!(ensure_thing_factory_exists());
        let source = "
            Object CompletionVisionStructure
                KindOf = STRUCTURE IMMOBILE
                DisplayName = OBJECT:CompletionVisionStructure
                Geometry = CYLINDER
                GeometryMajorRadius = 10
                GeometryHeight = 10
                ShroudClearingRange = 60
                ShroudRevealToAllRange = 45
                BuildCost = 700
            End
        ";
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(source), 1);
        let template = TheThingFactory::find_template("CompletionVisionStructure").unwrap();
        let structure = Arc::new(RwLock::new(Object::new_test_from_template(
            0xBA_0441, 100., template,
        )));
        {
            let mut object = structure.write().unwrap();
            object.set_construction_percent(0.);
            object.set_position(&position).unwrap();
            assert_eq!(object.get_shroud_clearing_range(), 10.);
        }
        {
            let mut logic = world.game_logic_mutex().lock().unwrap();
            logic.set_current_frame(frame);
            logic.register_object(Arc::clone(&structure)).unwrap();
        }
        {
            let mut object = structure.write().unwrap();
            object.set_team(Some(team)).unwrap();
            object.handle_partition_cell_maintenance();
        }
        world.services().shroud().lock().unwrap().process_entire_pending_undo_shroud_reveal_queue();
        structure
    })
}

#[test]
fn completed_structure_refreshes_vision_and_pending_unlook_in_its_own_world() {
    use crate::common::Coord3D;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::system::shroud_manager::ShroudState;

    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
    let driving = new_for_world();
    let peer = new_for_world();
    let receiving_position = Coord3D::new(105., 105., 0.);
    let peer_position = Coord3D::new(255., 255., 0.);
    let structure = admit_vision_structure(&driving, receiving_position, 37, 23);
    let peer_structure = admit_vision_structure(&peer, peer_position, 251, 41);
    let id = structure.read().unwrap().get_id();
    assert_eq!(peer_structure.read().unwrap().get_id(), id);
    let witness = Coord3D::new(135., 105., 0.);
    let driving_shroud = driving.services().shroud();
    let peer_shroud = peer.services().shroud();
    assert_eq!(driving_shroud.lock().unwrap().get_shroud_state(1, &witness), ShroudState::Hidden);
    let before_peer = peer_shroud.lock().unwrap().snapshot_state();

    let builder = Arc::new(RwLock::new(Object::new_test(0xBA_0442, 100.)));
    with_active_stores(&peer, || complete_dozer_for_test(&builder, &structure, false));

    let receiving_shroud = driving_shroud.lock().unwrap();
    let visibility = receiving_shroud.get_shroud_state(1, &witness);
    let receiving_state = receiving_shroud.snapshot_state();
    drop(receiving_shroud);
    let after_peer = peer_shroud.lock().unwrap().snapshot_state();
    let peer_spatial = peer.services().objects().read().unwrap()
        .find_objects_in_radius(receiving_position, 2.);
    let receiving_pending: Vec<_> = receiving_state.pending_undo_shroud_reveals.iter()
        .map(|entry| (entry.where_pos, entry.for_whom, entry.expiration_frame)).collect();
    let wrong_world_look = peer_shroud.lock().unwrap().get_shroud_state(1, &witness);
    for world in [&driving, &peer] {
        with_active_stores(world, || {
            let mut logic = world.game_logic_mutex().lock().unwrap();
            let _frame = crate::system::game_logic::enter_update_frame(logic.get_frame());
            logic.destroy_object(id);
            logic.cleanup_dead_objects().unwrap();
        });
    }
    assert_eq!((visibility, wrong_world_look), (ShroudState::Visible, ShroudState::Hidden));
    // Rust currently refreshes synchronously on the construction status edge
    // and again after Create callbacks. Both unlook phases must use frame 37 + 23.
    assert_eq!(receiving_pending, [
        (receiving_position.to_array(), 1, 60),
        (receiving_position.to_array(), 1, 60),
        (receiving_position.to_array(), 2, 60),
    ]);
    assert!(after_peer == before_peer, "the peer shroud counters and pending heap stay unchanged");
    assert!(!peer_spatial.contains(&id), "completion must not move the peer's same-ID spatial entry");
}

#[cfg(panic = "unwind")]
#[test]
fn completion_with_poisoned_body_keeps_the_installed_footprint_intact() {
    use crate::ai::Pathfinder;
    use crate::ai::pathfind_complete::{PathfindCellType, SURFACE_GROUND};
    use crate::common::Coord3D;
    use crate::system::engine_stores::{new_for_world, with_active_stores};

    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
    let driving = new_for_world();
    let peer = new_for_world();
    let center = Coord3D::new(105., 105., 0.);
    let structure = admit_vision_structure(&driving, center, 37, 23);
    let peer_structure = admit_vision_structure(&peer, center, 251, 41);
    let id = structure.read().unwrap().get_id();
    assert_eq!(peer_structure.read().unwrap().get_id(), id);
    let path = driving.ai().read().unwrap().pathfinder().unwrap();
    let peer_path = peer.ai().read().unwrap().pathfinder().unwrap();
    for (path, object) in [(&path, &structure), (&peer_path, &peer_structure)] {
        let mut path = path.write().unwrap();
        path.reset_with_size(40, 40);
        // Install a footprint with its canonical object ID as at construction start.
        path.add_object_to_pathfind_map(&object.read().unwrap()).unwrap();
    }
    let sample = |path: &Pathfinder| {
        [center, Coord3D::new(115., 105., 0.), Coord3D::new(125., 105., 0.)].map(|pos| {
            (
                path.get_cell_type_at(&pos),
                path.valid_movement_position_for_surfaces(SURFACE_GROUND, false, &pos, Some(id)),
            )
        })
    };
    let before = sample(&path.read().unwrap());
    assert_eq!(before, [
        (Some(PathfindCellType::Obstacle), true),
        (Some(PathfindCellType::Obstacle), true),
        (Some(PathfindCellType::Clear), true),
    ]);
    assert_eq!(sample(&peer_path.read().unwrap()), before);

    let player = structure.read().unwrap().get_controlling_player().unwrap();
    let body = structure.read().unwrap().get_body_module().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _body = body.lock().unwrap();
        panic!("inject a poisoned construction body lock");
    })).is_err());
    let failed = with_active_stores(&peer, || {
        Player::on_structure_construction_complete(&player, None, &structure, false)
    });
    let after = sample(&path.read().unwrap());
    let after_peer = sample(&peer_path.read().unwrap());
    let score = player.read().unwrap().get_score_keeper().get_total_money_spent();
    body.clear_poison();
    assert!(failed.unwrap_err().contains("footprint body lock poisoned"));
    assert_eq!(score, 0);
    assert_eq!(after_peer, before);
    assert_eq!(after, before, "failed footprint refresh must leave its live obstacle installed");

    with_active_stores(&peer, || {
        Player::on_structure_construction_complete(&player, None, &structure, false).unwrap();
    });
    assert_eq!(sample(&path.read().unwrap()), before);
    assert_eq!(sample(&peer_path.read().unwrap()), before);
    assert_eq!(player.read().unwrap().get_score_keeper().get_total_money_spent(), 700);
    for world in [&driving, &peer] {
        with_active_stores(world, || {
            let mut logic = world.game_logic_mutex().lock().unwrap();
            logic.destroy_object(id);
            logic.cleanup_dead_objects().unwrap();
        });
    }
}

#[test]
fn construction_message_uses_authored_display_name_for_the_local_builder_only() {
    use crate::common::Coord3D;
    use crate::helpers::TheInGameUI;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use game_engine::common::language::Language;

    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
    for local_index in [1, 0] {
        let world = new_for_world();
        with_active_stores(&world, || {
            Language::register_localized_string("OBJECT:CompletionVisionStructure", "Centrale électrique");
            Language::register_localized_string("DOZER:ConstructionComplete", "Construction terminée : %s");
        });
        let structure = admit_vision_structure(&world, Coord3D::new(105., 105., 0.), 37, 23);
        let structure_id = structure.read().unwrap().get_id();
        let team = structure.read().unwrap().get_team().unwrap();
        let builder_id = 0xBA_0442;
        let builder = Arc::new(RwLock::new(Object::new_test(builder_id, 100.)));
        with_active_stores(&world, || {
            world.services().players().write().unwrap().set_local_player_index(local_index);
            world.game_logic_mutex().lock().unwrap().register_object(Arc::clone(&builder)).unwrap();
            builder.write().unwrap().set_team(Some(team)).unwrap();
            complete_dozer_for_test(&builder, &structure, false);
        });
        let messages = TheInGameUI::drain_displayed_messages(Some(world.services()));
        with_active_stores(&world, || {
            let mut logic = world.game_logic_mutex().lock().unwrap();
            logic.destroy_object(structure_id);
            logic.destroy_object(builder_id);
            logic.cleanup_dead_objects().unwrap();
        });
        if local_index == 0 {
            assert_eq!(messages, ["Construction terminée : Centrale électrique"]);
        } else {
            assert!(messages.is_empty(), "a remote builder does not announce completion to the local player");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn admit_installed_construction_dozer(
    world: &Arc<crate::system::engine_stores::EngineStores>,
) -> Arc<RwLock<Object>> {
    use crate::common::{Coord3D, LocomotorSetType};
    use crate::helpers::TheThingFactory;
    use crate::locomotor::{LocomotorTemplate, SURFACE_GROUND};
    use crate::system::engine_stores::with_active_stores;
    use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
    use game_engine::common::thing::thing_factory::get_thing_factory;
    let existing = admit_vision_structure(world, Coord3D::new(305., 305., 0.), 37, 23);
    let team = existing.read().unwrap().get_team().unwrap();
    with_active_stores(world, || {
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let mut locomotor = LocomotorTemplate::new("CompletionDozerGround".into());
        locomotor.surfaces = SURFACE_GROUND;
        locomotor.max_speed = 3.0;
        locomotor.acceleration = 0.1;
        world.services().locomotor_store().register_template(locomotor);
        let definitions = "
            Object CompletionInstalledDozer
                KindOf = VEHICLE DOZER
                Body = ActiveBody Health
                    MaxHealth = 100
                    InitialHealth = 100
                End
                Behavior = DozerAIUpdate OrderedDock
                End
                Locomotor = SET_NORMAL CompletionDozerGround
            End
        ";
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(definitions), 1);
        let template = TheThingFactory::find_template("CompletionInstalledDozer").unwrap();
        let dozer = TheThingFactory::get().unwrap().new_object_with_team_handle(template, team).unwrap();
        let mut object = dozer.write().unwrap();
        object.set_position(&Coord3D::new(65., 105., 0.)).unwrap();
        let parent = object.get_ai_update_interface().expect("installed cached UnitAI");
        drop(object);
        let mut parent = parent.lock().unwrap();
        parent.choose_locomotor_set(LocomotorSetType::Normal).unwrap();
        let mut selected_surfaces = None;
        parent.with_cur_locomotor(&mut |locomotor| {
            selected_surfaces = Some(locomotor.get_legal_surfaces());
        });
        assert_eq!(selected_surfaces, Some(SURFACE_GROUND));
        assert_eq!(
            parent.get_dozer_ai_update_interface_mut().expect("installed Dozer behavior").get_current_task(),
            DozerTask::Invalid
        );
        dozer
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn build_assistant_assigns_the_installed_dozer_without_relocking_its_parent() {
    use crate::object::contain::transport_contain::selected_locomotor_is_airborne;
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::build_assistant_assigns_the_installed_dozer_without_relocking_its_parent");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_ASSIGNMENT_CHILD",
    ) {
        return;
    }
    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();

    let receiving = new_for_world();
    let peer = new_for_world();
    let dozer = admit_installed_construction_dozer(&receiving);
    let peer_dozer = admit_installed_construction_dozer(&peer);
    let dozer_id = dozer.read().unwrap().get_id();
    assert_eq!(peer_dozer.read().unwrap().get_id(), dozer_id);
    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "CompletionVisionStructure", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });

    let peer_target_id = place(&peer, None, 205.);
    let target_id = place(&receiving, Some(dozer_id), 105.);
    assert_eq!(peer_target_id, target_id);
    let parent = dozer.read().unwrap().get_ai_update_interface().unwrap();
    let mut parent = parent.lock().unwrap();
    let dozer = parent.get_dozer_ai_update_interface_mut().unwrap();
    assert_eq!(dozer.get_current_task(), DozerTask::Build);
    assert!(dozer.is_task_pending(DozerTask::Build));
    drop(parent);
    let other_parent = peer_dozer.read().unwrap().get_ai_update_interface().unwrap();
    let mut other_parent = other_parent.lock().unwrap();
    assert_eq!(
        other_parent.get_dozer_ai_update_interface_mut().unwrap().get_current_task(),
        DozerTask::Invalid
    );
    let target = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(target_id).unwrap();
    let other_target = peer.game_logic_mutex().lock().unwrap().find_object_by_id(target_id).unwrap();
    assert_eq!(target.read().unwrap().get_builder_id(), dozer_id);
    assert_eq!(other_target.read().unwrap().get_builder_id(), INVALID_ID);

    // BuildAssistant stamps builder provenance before calling the Dozer. An
    // unclaimed peer scaffold also exercises the Dozer's own setBuilder step.
    with_active_stores(&peer, || {
        let owner_airborne = selected_locomotor_is_airborne(&*other_parent);
        let dozer = other_parent.get_dozer_ai_update_interface_mut().unwrap();
        dozer.set_build_task(peer_target_id, 30, 100., false, owner_airborne);
        assert!(dozer.is_task_pending(DozerTask::Build));
    });
    assert_eq!(other_target.read().unwrap().get_builder_id(), dozer_id);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_dozer_enters_parent_movement_from_its_authored_start_dock() {
    use crate::ai::states::AIStateType;
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::installed_dozer_enters_parent_movement_from_its_authored_start_dock");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_MOVEMENT_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let dozer = admit_installed_construction_dozer(&receiving);
    let peer_dozer = admit_installed_construction_dozer(&peer);
    let dozer_id = dozer.read().unwrap().get_id();
    assert_eq!(peer_dozer.read().unwrap().get_id(), dozer_id);
    let peer_parent = peer_dozer.read().unwrap().get_ai_update_interface().unwrap();
    let peer_state = peer_parent.lock().unwrap().get_current_state_id();

    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        let terrain = world.services().terrain();
        terrain.write().unwrap()
            .try_restore_logic_height_map_with_effects(40, 40, &[0; 1600], &mut || {}).unwrap();
        let pathfinder = world.ai().read().unwrap().pathfinder().unwrap();
        pathfinder.write().unwrap().rebuild_from_terrain(&terrain.read().unwrap());
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "CompletionVisionStructure", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let peer_scaffold = place(&peer, None, 250.);
    let scaffold = place(&receiving, Some(dozer_id), 290.);
    assert_eq!(scaffold, peer_scaffold);
    let parent = dozer.read().unwrap().get_ai_update_interface().unwrap();
    let pathfinder = receiving.ai().read().unwrap().pathfinder().unwrap();
    let peer_pathfinder = peer.ai().read().unwrap().pathfinder().unwrap();

    with_active_stores(&receiving, || {
        let mut ai = parent.lock().unwrap();
        let (dock, moving) = ai.get_dozer_ai_update_interface_mut().unwrap().construction_move_for_test().unwrap();
        assert!(!moving);
        assert!(dock.x > 160.);
        assert!(!ai.is_waiting_for_path());
        receiving.game_logic_mutex().lock().unwrap().set_current_frame(38);
        ai.update().unwrap();

        assert_eq!(ai.get_current_state_id(), Some(AIStateType::MoveTo as u32));
        assert_eq!(ai.get_goal_position(), Some(dock));
        assert_eq!(ai.get_ignored_obstacle_id(), scaffold);
        assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAi);
        assert!(ai.is_waiting_for_path());
        assert_eq!(ai.get_dozer_ai_update_interface_mut().unwrap().construction_move_for_test(), Some((dock, true)));
        assert_eq!(pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 1);
        drop(ai);

        for _ in 0..3 {
            pathfinder.write().unwrap().process_pathfind_queue().unwrap();
        }
        assert_eq!(pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 0);
        let ai = parent.lock().unwrap();
        assert!(!ai.is_waiting_for_path());
        assert!(ai.get_path().is_some());
        assert_eq!(ai.get_path_destination(), Some(dock));
        let scaffold = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(scaffold).unwrap();
        assert_eq!(scaffold.read().unwrap().get_construction_percent(), 0.);
    });

    assert_eq!(peer_pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 0);
    let mut peer_ai = peer_parent.lock().unwrap();
    assert_eq!(peer_ai.get_current_state_id(), peer_state);
    assert_eq!(peer_ai.get_ignored_obstacle_id(), INVALID_ID);
    assert!(!peer_ai.is_waiting_for_path());
    assert!(peer_ai.get_dozer_ai_update_interface_mut().unwrap().construction_move_for_test().is_none());
}

#[cfg(not(target_arch = "wasm32"))]
fn admit_physical_construction_builder(
    world: &Arc<crate::system::engine_stores::EngineStores>,
    worker: bool,
) -> Arc<RwLock<Object>> {
    use crate::common::LocomotorSetType;
    use crate::helpers::TheThingFactory;
    use crate::locomotor::{LocomotorAppearance, LocomotorTemplate, SURFACE_GROUND};
    use crate::system::engine_stores::with_active_stores;
    use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
    use game_engine::common::thing::thing_factory::get_thing_factory;

    let existing = admit_vision_structure(world, Coord3D::new(305., 305., 0.), 37, 23);
    let team = existing.read().unwrap().get_team().unwrap();
    with_active_stores(world, || {
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let terrain = world.services().terrain();
        terrain.write().unwrap()
            .try_restore_logic_height_map_with_effects(40, 40, &[0; 1600], &mut || {}).unwrap();
        let pathfinder = world.ai().read().unwrap().pathfinder().unwrap();
        pathfinder.write().unwrap().rebuild_from_terrain(&terrain.read().unwrap());

        let (kind, behavior, appearance) = if worker {
            ("INFANTRY DOZER HARVESTER", "WorkerAIUpdate", LocomotorAppearance::TwoLegs)
        } else {
            ("VEHICLE DOZER", "DozerAIUpdate", LocomotorAppearance::Treads)
        };
        let mut ground = LocomotorTemplate::new("PhysicalConstructionGround".into());
        ground.surfaces = SURFACE_GROUND;
        ground.max_speed = 6.;
        ground.acceleration = 0.4;
        ground.max_turn_rate = 0.5;
        ground.appearance = appearance;
        world.services().locomotor_store().register_template(ground);
        let definitions = format!("
            Object PhysicalConstructor
                KindOf = {kind}
                Geometry = CYLINDER
                GeometryMajorRadius = 4
                GeometryHeight = 5
                Body = ActiveBody Health
                    MaxHealth = 100
                    InitialHealth = 100
                End
                Behavior = PhysicsBehavior Drive
                    Mass = 10.0
                End
                Behavior = {behavior} Builder
                End
                Locomotor = SET_NORMAL PhysicalConstructionGround
            End
            Object PhysicalConstructionSite
                KindOf = STRUCTURE IMMOBILE
                Geometry = CYLINDER
                GeometryMajorRadius = 8
                GeometryHeight = 9
                BuildTime = 10.0
                Body = StructureBody Health
                    MaxHealth = 100
                    InitialHealth = 100
                End
            End
        ");
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&definitions), 2);
        let template = TheThingFactory::find_template("PhysicalConstructor").unwrap();
        let actor = TheThingFactory::get().unwrap().new_object_with_team_handle(template, team).unwrap();
        let mut object = actor.write().unwrap();
        object.set_position(&Coord3D::new(65., 105., 0.)).unwrap();
        assert!(object.get_physics().is_some());
        let ai = object.get_ai_update_interface().unwrap();
        drop(object);
        let mut ai = ai.lock().unwrap();
        ai.choose_locomotor_set(LocomotorSetType::Normal).unwrap();
        assert_eq!(ai.get_worker_ai_update_interface_mut().is_some(), worker);
        assert_eq!(ai.get_dozer_ai_update_interface_mut().is_some(), !worker);
        actor
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_dozer_motive_force_wakes_physics_without_postponing_its_due_frame() {
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::installed_dozer_motive_force_wakes_physics_without_postponing_its_due_frame");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_MOTIVE_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let dozer = admit_physical_construction_builder(&receiving, false);
    let worker = admit_physical_construction_builder(&peer, true);
    let (builder_id, parent, physics, proxy) = {
        let dozer = dozer.read().unwrap();
        let physics = dozer.get_physics().unwrap();
        (dozer.get_id(), dozer.get_ai_update_interface().unwrap(), physics.clone(), physics.installed_update_proxy(&dozer).unwrap())
    };
    let (worker_physics, worker_proxy, worker_pose) = {
        let worker = worker.read().unwrap();
        assert_eq!(worker.get_id(), builder_id);
        let physics = worker.get_physics().unwrap();
        (physics.clone(), physics.installed_update_proxy(&worker).unwrap(), *worker.get_position())
    };
    peer.game_logic_mutex().lock().unwrap().register_sleepy_update_module(builder_id, worker_proxy.clone(), 90);
    let initial_pose = *dozer.read().unwrap().get_position();

    with_active_stores(&receiving, || {
        let site = GameLogicBuildAssistantBackend.build_object_now(
            Some(builder_id), "CompletionVisionStructure", &BuildPosition { x: 290., y: 105., z: 0. }, 0., 0,
        ).unwrap();
        let logic = receiving.game_logic_mutex();
        logic.lock().unwrap().register_sleepy_update_module(builder_id, proxy.clone(), 80);
        for frame in 38..=41 {
            logic.lock().unwrap().set_current_frame(frame.into());
            // AIUpdate.h:580 runs the actual cached AI in PHASE_INITIAL,
            // before GameLogic drives the installed Physics module.
            parent.lock().unwrap().update().unwrap();
            if frame >= 40 {
                assert!(physics.accumulated_force_acceleration().x > 0.);
                let state = physics.access().unwrap();
                assert!(state.is_motive_at(frame + 9).unwrap());
                assert!(!state.is_motive_at(frame + 10).unwrap());
                drop(state);
                assert_eq!(logic.lock().unwrap().sleepy_entry_for(&proxy), Some((builder_id, 41)));
            }
            logic.lock().unwrap().update(frame, || None).unwrap();
        }
        assert!(dozer.read().unwrap().get_position().x > initial_pose.x);
        let scaffold = logic.lock().unwrap().find_object_by_id(site).unwrap();
        assert_eq!(scaffold.read().unwrap().get_construction_percent(), 0.);
    });
    assert_eq!(*worker.read().unwrap().get_position(), worker_pose);
    assert_eq!(worker_physics.accumulated_force_acceleration(), Coord3D::ZERO);
    assert!(!worker_physics.access().unwrap().is_motive_at(40).unwrap());
    assert_eq!(peer.game_logic_mutex().lock().unwrap().sleepy_entry_for(&worker_proxy), Some((builder_id, 90)));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_dozer_arrives_and_contributes_at_its_ignored_scaffold() {
    use crate::test_process::{TestProcess, run_bounded};

    const NAME: &str = concat!(module_path!(), "::installed_dozer_arrives_and_contributes_at_its_ignored_scaffold");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_CONTACT_CHILD",
    ) {
        return;
    }
    assert_physical_construction_action(false);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_moves_through_action_dock_before_contributing() {
    use crate::test_process::{TestProcess, run_bounded};

    const NAME: &str = concat!(module_path!(), "::installed_worker_moves_through_action_dock_before_contributing");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_WORKER_ACTION_DOCK_CHILD",
    ) {
        return;
    }
    assert_physical_construction_action(true);
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_physical_construction_action(worker: bool) {
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    let receiving = new_for_world();
    let peer = new_for_world();
    let actor = admit_physical_construction_builder(&receiving, worker);
    let peer_actor = admit_physical_construction_builder(&peer, !worker);
    let builder = actor.read().unwrap().get_id();
    assert_eq!(peer_actor.read().unwrap().get_id(), builder);
    let peer_pose = *peer_actor.read().unwrap().get_position();
    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "PhysicalConstructionSite", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let site_id = place(&receiving, Some(builder), 290.);
    assert_eq!(place(&peer, None, 205.), site_id);
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let parent = actor.read().unwrap().get_ai_update_interface().unwrap();
    let mut parent_state = parent.lock().unwrap();
    let dock = if worker {
        parent_state.get_worker_ai_update_interface_mut().unwrap()
            .construction_order_for_test().unwrap().docks[DozerDockPoint::Action as usize].unwrap()
    } else {
        parent_state.get_dozer_ai_update_interface_mut().unwrap()
            .construction_move_for_test().unwrap().0
    };
    drop(parent_state);
    let pathfinder = receiving.ai().read().unwrap().pathfinder().unwrap();

    with_active_stores(&receiving, || {
        let logic = receiving.game_logic_mutex();
        let site = logic.lock().unwrap().find_object_by_id(site_id).unwrap();
        let mut contribution = None;
        let mut action_path_frame = None;
        for frame in 38..120 {
            logic.lock().unwrap().set_current_frame(frame.into());
            // The installed AI precedes the actual scheduler's Physics and
            // partition contact phases, as selected by its C++ update phase.
            let (build_dock, idle) = {
                let mut ai = parent.lock().unwrap();
                ai.update().unwrap();
                let build_dock = if worker {
                    ai.get_worker_ai_update_interface_mut().unwrap()
                        .construction_order_for_test().unwrap().build_sub_task
                } else {
                    ai.get_dozer_ai_update_interface_mut().unwrap().build_dock_sub_task_for_test()
                };
                (build_dock, ai.is_idle())
            };
            let progress = site.read().unwrap().get_construction_percent();
            if build_dock == DozerBuildSubTask::MovingToBuildDockLocation {
                assert_eq!(progress, 0.0);
                assert!(!idle);
                if pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count() > 0 {
                    action_path_frame.get_or_insert(frame);
                }
            }
            logic.lock().unwrap().update(frame, || None).unwrap();
            let position = *actor.read().unwrap().get_position();
            if frame == 60 {
                assert!((145.0..155.0).contains(&position.x), "{position:?}");
            }
            if progress > 0.0 {
                assert!(idle);
                assert_eq!(build_dock, DozerBuildSubTask::DoBuildAtDock);
                assert!(position.distance(dock) < 2.0, "{position:?}, dock {dock:?}");
                contribution = Some((frame, position, progress));
                break;
            }
        }
        let (frame, position, progress) = contribution.expect("installed builder did not reach its Action dock");
        assert!((75..100).contains(&frame), "frame {frame}, position {position:?}");
        let action_path_frame = action_path_frame.expect("Action dock issues its own receiving path request");
        assert!(action_path_frame > 39 && action_path_frame < frame);
        // The empty headless energy gives the source 0.01 power floor.
        assert!((progress - 100.0 / 30_000.0).abs() < 0.00001);
        assert_eq!(site.read().unwrap().get_builder_id(), builder);
    });

    assert_eq!(*peer_actor.read().unwrap().get_position(), peer_pose);
    let peer_site = peer_site.read().unwrap();
    assert_eq!(peer_site.get_builder_id(), INVALID_ID);
    assert_eq!(peer_site.get_construction_percent(), 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_dozer_finishes_powered_build_and_takes_its_end_dock_path() {
    use crate::test_process::{TestProcess, run_bounded};

    const NAME: &str = concat!(module_path!(), "::installed_dozer_finishes_powered_build_and_takes_its_end_dock_path");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_END_DOCK_CHILD",
    ) {
        return;
    }

    assert_physical_powered_construction_end(false, false);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_animates_powered_build_and_takes_its_end_dock_path() {
    use crate::test_process::{TestProcess, run_bounded};

    const NAME: &str = concat!(module_path!(), "::installed_worker_animates_powered_build_and_takes_its_end_dock_path");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_WORKER_END_DOCK_CHILD",
    ) {
        return;
    }

    assert_physical_powered_construction_end(true, false);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_dozer_keeps_live_scaffold_damage_through_completion_and_end_dock() {
    use crate::test_process::{TestProcess, run_bounded};

    const NAME: &str = concat!(module_path!(), "::installed_dozer_keeps_live_scaffold_damage_through_completion_and_end_dock");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_DOZER_LIVE_SCAFFOLD_CHILD",
    ) {
        return;
    }

    assert_physical_powered_construction_end(false, true);
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_physical_powered_construction_end(worker: bool, damage_during_construction: bool) {
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    let receiving = new_for_world();
    let peer = new_for_world();
    let actor = admit_physical_construction_builder(&receiving, worker);
    let peer_actor = admit_physical_construction_builder(&peer, !worker);
    let builder = actor.read().unwrap().get_id();
    assert_eq!(peer_actor.read().unwrap().get_id(), builder);
    let peer_pose = *peer_actor.read().unwrap().get_position();
    let drawable = Arc::new(RwLock::new(Drawable::new(
        builder, builder, "ReceivingBuilder".into(), DrawableType::Animated,
    )));
    with_active_stores(&receiving, || actor.write().unwrap().set_drawable(Some(drawable.clone())));
    let peer_drawable = Arc::new(RwLock::new(Drawable::new(
        builder, builder, "PeerBuilder".into(), DrawableType::Animated,
    )));
    with_active_stores(&peer, || peer_actor.write().unwrap().set_drawable(Some(peer_drawable.clone())));
    let peer_flags = peer_drawable.read().unwrap().get_model_conditions();
    let player = with_active_stores(&receiving, || actor.read().unwrap().get_controlling_player().unwrap());
    player.write().unwrap().get_energy_mut().add_power_production(10);
    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "PhysicalConstructionSite", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let site_id = place(&receiving, Some(builder), 290.);
    assert_eq!(place(&peer, None, 205.), site_id);
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let parent = actor.read().unwrap().get_ai_update_interface().unwrap();
    let mut parent_state = parent.lock().unwrap();
    let authored_end = if worker {
        let order = parent_state.get_worker_ai_update_interface_mut().unwrap()
            .construction_order_for_test().unwrap();
        let end = order.docks[DozerDockPoint::End as usize].unwrap();
        // WorkerAIUpdate.cpp:633–639 assigns all three docks to the order point.
        assert_eq!(order.docks, [Some(end); 3]);
        end
    } else {
        parent_state.get_dozer_ai_update_interface_mut().unwrap().build_end_dock_for_test().unwrap()
    };
    drop(parent_state);
    let pathfinder = receiving.ai().read().unwrap().pathfinder().unwrap();

    with_active_stores(&receiving, || {
        let logic = receiving.game_logic_mutex();
        let site = logic.lock().unwrap().find_object_by_id(site_id).unwrap();
        let mut started = None;
        let mut completed: Option<(Coord3D, Coord3D)> = None;
        let mut end_delivered = false;
        let mut expected_health = site.read().unwrap().get_health();
        for frame in 38..450 {
            logic.lock().unwrap().set_current_frame(frame.into());
            let mut ai = parent.lock().unwrap();
            if let Some((_, goal)) = completed {
                ai.update().unwrap();
                let worker_returned_to_idle = worker && !ai.is_waiting_for_path() && ai.is_idle();
                let dozer_path_installed = !worker && ai.get_path_destination() == Some(goal);
                drop(ai);
                logic.lock().unwrap().update(frame, || None).unwrap();
                if worker_returned_to_idle
                    || (dozer_path_installed && actor.read().unwrap().get_position().x < goal.x + 10.0)
                {
                    end_delivered = true;
                    break;
                }
                continue;
            }

            let approach = *actor.read().unwrap().get_position();
            ai.update().unwrap();
            let progress = site.read().unwrap().get_construction_percent();
            let constructing = drawable.read().unwrap().get_model_conditions()
                .contains(ModelConditionFlags::ACTIVELY_CONSTRUCTING);
            assert_eq!(constructing, progress > 0.0, "builder animation at frame {frame}");
            if progress > 0.0 && started.is_none() {
                assert!((progress - 100.0 / 300.0).abs() < 0.00001);
                started = Some(frame);
            }
            if progress > 0.0 || progress == crate::object::CONSTRUCTION_COMPLETE {
                expected_health = (expected_health + 100.0 / 300.0).min(100.0);
                let health = site.read().unwrap().get_health();
                assert!((health - expected_health).abs() < 0.00001, "construction health at frame {frame}: {health}, expected {expected_health}");
                if damage_during_construction && started.is_some_and(|start| frame == start + 3) {
                    site.write().unwrap().set_health(0.5).unwrap();
                    expected_health = 0.5;
                }
            }
            if progress == crate::object::CONSTRUCTION_COMPLETE {
                assert_eq!(frame - started.expect("build must contribute before completion") + 1, 300);
                assert_eq!(site.read().unwrap().get_health() < 100.0, damage_during_construction);
                assert!(ai.is_waiting_for_path());
                assert!(pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count() > 0);
                assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAi);
                let goal = ai.get_goal_position().expect("native retreat path goal");
                assert!(goal.distance(authored_end) < crate::path::PATHFIND_CELL_SIZE_F);
                if worker {
                    assert!(approach.distance(goal) < crate::path::PATHFIND_CELL_SIZE_F);
                    assert!(ai.get_worker_ai_update_interface_mut().unwrap().construction_order_for_test().is_none());
                } else {
                    assert!(approach.distance(goal) > 35.0);
                    assert!(!ai.get_dozer_ai_update_interface_mut().unwrap().is_task_pending(DozerTask::Build));
                }
                completed = Some((approach, goal));
            }
            drop(ai);
            logic.lock().unwrap().update(frame, || None).unwrap();
        }
        let (approach, goal) = completed.expect("powered builder must complete and set its native End goal");
        assert!(end_delivered, "installed AI must consume its End movement");
        let construction = receiving.services().logic_managers().construction.read().unwrap();
        assert!(construction.active_tasks().is_empty());
        let final_pose = *actor.read().unwrap().get_position();
        if worker {
            assert!(final_pose.distance(goal) < crate::path::PATHFIND_CELL_SIZE_F);
            assert_eq!(pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 0);
        } else {
            assert!(final_pose.distance(approach) > 30.0);
            assert!(final_pose.x < goal.x + 10.0);
        }
    });

    assert_eq!(*peer_actor.read().unwrap().get_position(), peer_pose);
    assert_eq!(peer_drawable.read().unwrap().get_model_conditions(), peer_flags);
    let peer_site = peer_site.read().unwrap();
    assert_eq!(peer_site.get_builder_id(), INVALID_ID);
    assert_eq!(peer_site.get_construction_percent(), 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_recalculates_the_receiving_build_rate_after_power_changes() {
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::installed_worker_recalculates_the_receiving_build_rate_after_power_changes");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_WORKER_BUILD_RATE_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let worker = admit_physical_construction_builder(&receiving, true);
    let peer_dozer = admit_physical_construction_builder(&peer, false);
    let builder = worker.read().unwrap().get_id();
    assert_eq!(peer_dozer.read().unwrap().get_id(), builder);
    let player = with_active_stores(&receiving, || worker.read().unwrap().get_controlling_player().unwrap());
    let peer_player = with_active_stores(&peer, || peer_dozer.read().unwrap().get_controlling_player().unwrap());
    player.write().unwrap().get_energy_mut().add_power_production(10);
    peer_player.write().unwrap().get_energy_mut().add_power_production(5);
    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "PhysicalConstructionSite", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let site_id = place(&receiving, Some(builder), 290.);
    assert_eq!(place(&peer, None, 205.), site_id);
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let parent = worker.read().unwrap().get_ai_update_interface().unwrap();

    with_active_stores(&receiving, || {
        let logic = receiving.game_logic_mutex();
        let site = logic.lock().unwrap().find_object_by_id(site_id).unwrap();
        let mut before = (0., site.read().unwrap().get_health());
        let mut contributions = 0;
        for frame in 38..120 {
            logic.lock().unwrap().set_current_frame(frame.into());
            parent.lock().unwrap().update().unwrap();
            let after = {
                let building = site.read().unwrap();
                (building.get_construction_percent(), building.get_health())
            };
            logic.lock().unwrap().update(frame, || None).unwrap();
            if after.0 == before.0 {
                continue;
            }
            // ThingTemplate.cpp:1524–1558, DozerAIUpdate.cpp:515–526.
            // Lost power uses the deliberate 0.01 default fixture floor.
            let expected = [100.0 / 300.0, 100.0 / 30_000.0, 100.0 / 300.0][contributions];
            let percent_delta = after.0 - before.0;
            let health_delta = after.1 - before.1;
            assert!((percent_delta - expected).abs() < 0.00001, "frame {frame} progress delta {percent_delta}, expected {expected}");
            assert!((health_delta - expected).abs() < 0.00001, "frame {frame} health delta {health_delta}, expected {expected}");
            before = after;
            contributions += 1;
            match contributions {
                1 => player.write().unwrap().get_energy_mut().add_power_production(-10),
                2 => player.write().unwrap().get_energy_mut().add_power_production(10),
                _ => break,
            }
        }
        assert_eq!(contributions, 3);
    });

    assert_eq!(peer_player.read().unwrap().get_energy().get_power(), 5);
    let peer_site = peer_site.read().unwrap();
    assert_eq!(peer_site.get_builder_id(), INVALID_ID);
    assert_eq!(peer_site.get_construction_percent(), 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_resumes_existing_scaffold_without_relocking_its_parent() {
    use crate::ai::{AiCommandParams, AiCommandType};
    use crate::helpers::TheThingFactory;
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::installed_worker_resumes_existing_scaffold_without_relocking_its_parent");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_WORKER_RESUME_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let actor = admit_physical_construction_builder(&receiving, true);
    let peer_actor = admit_physical_construction_builder(&peer, false);
    let builder = actor.read().unwrap().get_id();
    assert_eq!(peer_actor.read().unwrap().get_id(), builder);
    let place = |world: &Arc<EngineStores>, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            None, "PhysicalConstructionSite", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let site_id = place(&receiving, 290.);
    assert_eq!(place(&peer, 205.), site_id);
    let site = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let peer_pose = *peer_actor.read().unwrap().get_position();

    with_active_stores(&receiving, || {
        let parent = actor.read().unwrap().get_ai_update_interface().unwrap();
        let player = actor.read().unwrap().get_controlling_player().unwrap();
        player.write().unwrap().get_energy_mut().add_power_production(10);
        let team = actor.read().unwrap().get_team().unwrap();
        let template = TheThingFactory::find_template("PhysicalConstructor").unwrap();
        let other_worker = TheThingFactory::get().unwrap().new_object_with_team_handle(template, team).unwrap();
        {
            let mut other = other_worker.write().unwrap();
            other.set_position(&Coord3D::new(65., 205., 0.)).unwrap();
        }
        let other_parent = other_worker.read().unwrap().get_ai_update_interface().unwrap();
        {
            let mut scaffold = site.write().unwrap();
            assert_eq!(scaffold.get_builder_id(), INVALID_ID);
            scaffold.set_construction_percent(30.0);
            scaffold.set_health(31.0).unwrap();
            assert!(ActionManager::can_resume_construction_of(
                &actor.read().unwrap(), &scaffold, CommandSourceType::FromScript,
            ));
        }
        let mut resume = AiCommandParams::new(AiCommandType::ResumeConstruction, CommandSourceType::FromScript);
        resume.obj = Some(site_id);
        parent.lock().unwrap().execute_command(&resume).unwrap();
        assert_eq!(site.read().unwrap().get_builder_id(), builder);
        let accepted = parent.lock().unwrap().get_worker_ai_update_interface_mut().unwrap()
            .construction_order_for_test().unwrap();

        let logic = receiving.game_logic_mutex();
        let mut contributed_on = None;
        for frame in 38..120 {
            logic.lock().unwrap().set_current_frame(frame.into());
            parent.lock().unwrap().update().unwrap();
            logic.lock().unwrap().update(frame, || None).unwrap();
            if site.read().unwrap().get_construction_percent() > 30.0 {
                contributed_on = Some(frame);
                break;
            }
        }
        {
            let scaffold = site.read().unwrap();
            assert!((scaffold.get_construction_percent() - (30.0 + 100.0 / 300.0)).abs() < 0.00001);
            assert!((scaffold.get_health() - (31.0 + 100.0 / 300.0)).abs() < 0.00001);
            assert_eq!(scaffold.get_producer_id(), builder);
            for installed in [&receiving, &peer] {
                with_active_stores(installed, || {
                    assert!(!ActionManager::can_resume_construction_of(
                        &other_worker.read().unwrap(), &scaffold, CommandSourceType::FromScript,
                    ));
                });
            }
        }
        // The same installed parent already owns its AI mutex during command
        // reception. A duplicate order and another Worker must not restart it.
        parent.lock().unwrap().execute_command(&resume).unwrap();
        other_parent.lock().unwrap().execute_command(&resume).unwrap();
        let observation = parent.lock().unwrap().get_worker_ai_update_interface_mut().unwrap()
            .construction_order_for_test().unwrap();
        assert_eq!(observation.order_frame, accepted.order_frame);
        assert_eq!(observation.build_sub_task, DozerBuildSubTask::DoBuildAtDock);
        assert!(other_parent.lock().unwrap().get_worker_ai_update_interface_mut().unwrap()
            .construction_order_for_test().is_none());
        assert_eq!(site.read().unwrap().get_builder_id(), builder);
        let next = contributed_on.expect("existing scaffold must receive construction") + 1;
        logic.lock().unwrap().set_current_frame(next.into());
        parent.lock().unwrap().update().unwrap();
        let scaffold = site.read().unwrap();
        assert!((scaffold.get_construction_percent() - (30.0 + 200.0 / 300.0)).abs() < 0.00001);
        assert!((scaffold.get_health() - (31.0 + 200.0 / 300.0)).abs() < 0.00001);
    });

    assert_eq!(*peer_actor.read().unwrap().get_position(), peer_pose);
    let peer_site = peer_site.read().unwrap();
    assert_eq!(peer_site.get_builder_id(), INVALID_ID);
    assert_eq!(peer_site.get_construction_percent(), 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_workers_transfer_a_live_scaffold_without_losing_progress_or_damage() {
    use crate::ai::{AiCommandParams, AiCommandType};
    use crate::common::LocomotorSetType;
    use crate::helpers::TheThingFactory;
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::installed_workers_transfer_a_live_scaffold_without_losing_progress_or_damage");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_INSTALLED_WORKER_TRANSFER_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let worker = admit_physical_construction_builder(&receiving, true);
    let peer_actor = admit_physical_construction_builder(&peer, false);
    let builder = worker.read().unwrap().get_id();
    assert_eq!(peer_actor.read().unwrap().get_id(), builder);
    let peer_pose = *peer_actor.read().unwrap().get_position();
    let place = |world: &Arc<EngineStores>, builder, x, y| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "PhysicalConstructionSite", &BuildPosition { x, y, z: 0. }, 0., 0,
        ).unwrap()
    });
    with_active_stores(&receiving, || {
        let player = worker.read().unwrap().get_controlling_player().unwrap();
        player.write().unwrap().get_energy_mut().add_power_production(10);
    });
    let site_id = place(&receiving, Some(builder), 290., 105.);
    assert_eq!(place(&peer, None, 205., 105.), site_id);
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();

    with_active_stores(&receiving, || {
        let logic = receiving.game_logic_mutex();
        let site = logic.lock().unwrap().find_object_by_id(site_id).unwrap();
        let parent = worker.read().unwrap().get_ai_update_interface().unwrap();
        let first = (38..120).find(|&frame| {
            logic.lock().unwrap().set_current_frame(frame.into());
            parent.lock().unwrap().update().unwrap();
            logic.lock().unwrap().update(frame, || None).unwrap();
            site.read().unwrap().get_construction_percent() > 0.0
        }).expect("original Worker must contribute first");
        let original_progress = site.read().unwrap().get_construction_percent();
        site.write().unwrap().set_health(0.75).unwrap();

        let new_site_id = place(&receiving, Some(builder), 295., 285.);
        let new_site = logic.lock().unwrap().find_object_by_id(new_site_id).unwrap();
        worker.write().unwrap().set_position(&Coord3D::new(275., 285., 0.)).unwrap();
        let team = worker.read().unwrap().get_team().unwrap();
        let template = TheThingFactory::find_template("PhysicalConstructor").unwrap();
        let replacement = TheThingFactory::get().unwrap().new_object_with_team_handle(template, team).unwrap();
        let (replacement_id, replacement_parent) = {
            let mut actor = replacement.write().unwrap();
            actor.set_position(&Coord3D::new(65., 105., 0.)).unwrap();
            (actor.get_id(), actor.get_ai_update_interface().unwrap())
        };
        let mut resume = AiCommandParams::new(AiCommandType::ResumeConstruction, CommandSourceType::FromScript);
        resume.obj = Some(site_id);
        {
            let mut ai = replacement_parent.lock().unwrap();
            ai.choose_locomotor_set(LocomotorSetType::Normal).unwrap();
            ai.execute_command(&resume).unwrap();
        }
        assert_eq!(site.read().unwrap().get_builder_id(), replacement_id);

        let switched = (first + 1..220).find(|&frame| {
            logic.lock().unwrap().set_current_frame(frame.into());
            replacement_parent.lock().unwrap().update().unwrap();
            logic.lock().unwrap().update(frame, || None).unwrap();
            site.read().unwrap().get_construction_percent() > original_progress
        }).expect("replacement Worker must continue the original scaffold");
        let delta = 100.0 / 300.0;
        {
            let scaffold = site.read().unwrap();
            assert!((scaffold.get_construction_percent() - (original_progress + delta)).abs() < 0.00001);
            assert!((scaffold.get_health() - (0.75 + delta)).abs() < 0.00001);
            assert_eq!(scaffold.get_producer_id(), replacement_id);
        }
        assert!(replacement.read().unwrap().get_position().x > 260.);
        // Further damage during the new builder's work remains visible on the
        // next frame. The authored body owns HP and construction adds to it.
        site.write().unwrap().set_health(0.5).unwrap();
        let next = switched + 1;
        logic.lock().unwrap().set_current_frame(next.into());
        replacement_parent.lock().unwrap().update().unwrap();
        assert!((site.read().unwrap().get_health() - (0.5 + delta)).abs() < 0.00001);
        logic.lock().unwrap().update(next, || None).unwrap();

        for frame in next + 1..next + 60 {
            logic.lock().unwrap().set_current_frame(frame.into());
            parent.lock().unwrap().update().unwrap();
            replacement_parent.lock().unwrap().update().unwrap();
            logic.lock().unwrap().update(frame, || None).unwrap();
            if new_site.read().unwrap().get_construction_percent() > 0.0 {
                break;
            }
        }
        assert_eq!(new_site.read().unwrap().get_builder_id(), builder);
        assert!(new_site.read().unwrap().get_construction_percent() > 0.0);
        assert_eq!(site.read().unwrap().get_builder_id(), replacement_id);
        assert!(site.read().unwrap().get_construction_percent() > original_progress + delta);
        let construction = receiving.services().logic_managers().construction.read().unwrap();
        assert!(construction.active_tasks().is_empty());
    });

    assert_eq!(*peer_actor.read().unwrap().get_position(), peer_pose);
    let peer_site = peer_site.read().unwrap();
    assert_eq!(peer_site.get_builder_id(), INVALID_ID);
    assert_eq!(peer_site.get_construction_percent(), 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn construction_model_uses_the_scaffolds_world_for_its_live_builder() {
    use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::test_process::{TestProcess, run_bounded};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    const NAME: &str = concat!(module_path!(), "::construction_model_uses_the_scaffolds_world_for_its_live_builder");
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_CORE_CONSTRUCTION_MODEL_OWNER_CHILD",
    ) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let dozer = admit_physical_construction_builder(&receiving, false);
    let worker = admit_physical_construction_builder(&peer, true);
    let builder = dozer.read().unwrap().get_id();
    assert_eq!(worker.read().unwrap().get_id(), builder);
    let site_id = with_active_stores(&receiving, || {
        GameLogicBuildAssistantBackend.build_object_now(
            Some(builder), "PhysicalConstructionSite", &BuildPosition { x: 290., y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let site = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let drawable = Arc::new(RwLock::new(Drawable::new(
        site_id, site_id, "ReceivingScaffold".into(), DrawableType::Animated,
    )));
    with_active_stores(&receiving, || site.write().unwrap().set_drawable(Some(drawable.clone())));

    with_active_stores(&peer, || {
        worker.write().unwrap().kill(None, None);
        assert!(!worker.read().unwrap().is_alive());
        assert!(dozer.read().unwrap().is_alive());
        // The owning registry must read its live builder even with its own
        // GameLogic borrowed by the scheduler and the dead peer published.
        let receiving_logic = receiving.game_logic_mutex().lock().unwrap();
        site.write().unwrap().set_construction_percent(40.0);
        let flags = drawable.read().unwrap().get_model_conditions();
        assert!(flags.contains(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED));
        assert!(!flags.contains(ModelConditionFlags::PARTIALLY_CONSTRUCTED));
        drop(receiving_logic);
    });

    with_active_stores(&receiving, || dozer.write().unwrap().kill(None, None));
    with_active_stores(&peer, || {
        assert!(!dozer.read().unwrap().is_alive());
        for (percent, expected) in [
            (41.0, ModelConditionFlags::PARTIALLY_CONSTRUCTED),
            (0.0, ModelConditionFlags::AWAITING_CONSTRUCTION),
            (crate::object::CONSTRUCTION_COMPLETE, ModelConditionFlags::CONSTRUCTION_COMPLETE),
        ] {
            site.write().unwrap().set_construction_percent(percent);
            let flags = drawable.read().unwrap().get_model_conditions();
            let stage = flags & (ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED
                | ModelConditionFlags::PARTIALLY_CONSTRUCTED
                | ModelConditionFlags::AWAITING_CONSTRUCTION
                | ModelConditionFlags::CONSTRUCTION_COMPLETE);
            assert_eq!(stage, expected);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn destroying_a_looking_structure_uses_the_driving_clock_outside_update() {
    use crate::common::Coord3D;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::system::shroud_manager::ShroudState;
    use crate::test_process::{TestProcess, run_bounded};

    let name = concat!(module_path!(), "::destroying_a_looking_structure_uses_the_driving_clock_outside_update");
    if let TestProcess::ParentVerified = run_bounded(
        name.strip_prefix("gamelogic::").unwrap_or(name),
        "GENERALS_DESTROY_LOOKING_STRUCTURE_CHILD",
    ) {
        return;
    }
    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
    for publish_peer in [false, true] {
        let driving = new_for_world();
        let peer = new_for_world();
        let position = Coord3D::new(105., 105., 0.);
        let peer_position = Coord3D::new(255., 255., 0.);
        let structure = admit_vision_structure(&driving, position, 37, 23);
        let peer_structure = admit_vision_structure(&peer, peer_position, 251, 41);
        let id = structure.read().unwrap().get_id();
        assert_eq!(peer_structure.read().unwrap().get_id(), id);
        let shroud = driving.services().shroud();
        let peer_shroud = peer.services().shroud();
        assert_eq!(shroud.lock().unwrap().get_shroud_state(0, &position), ShroudState::Visible);
        let before_peer = peer_shroud.lock().unwrap().snapshot_state();

        let published = if publish_peer { &peer } else { &driving };
        with_active_stores(published, || {
            let mut logic = driving.game_logic_mutex().lock().unwrap();
            assert_eq!(logic.get_frame(), 37);
            eprintln!("calling GameLogic::destroy_object while holding its owner outside update");
            logic.destroy_object(id);
            logic.destroy_object(id);
        });
        assert!(structure.read().unwrap().is_destroyed());
        assert!(!peer_structure.read().unwrap().is_destroyed());
        assert!(peer_shroud.lock().unwrap().snapshot_state() == before_peer);
        {
            let mut shroud = shroud.lock().unwrap();
            let state = shroud.snapshot_state();
            let [pending] = state.pending_undo_shroud_reveals.as_slice() else {
                panic!("destroyObject must queue the old reveal once");
            };
            assert_eq!(pending.where_pos, position.to_array());
            assert_eq!(pending.expiration_frame, 60);
            shroud.process_pending_undo_shroud_reveals(60);
            assert_eq!(shroud.get_shroud_state(0, &position), ShroudState::Visible);
            shroud.process_pending_undo_shroud_reveals(61);
            assert_eq!(shroud.get_shroud_state(0, &position), ShroudState::Explored);
        }
        with_active_stores(&driving, || {
            driving.game_logic_mutex().lock().unwrap().cleanup_dead_objects().unwrap();
        });
        with_active_stores(&peer, || {
            let mut logic = peer.game_logic_mutex().lock().unwrap();
            logic.destroy_object(id);
            logic.cleanup_dead_objects().unwrap();
        });
    }
}

/// A real create-module callback probes both manager loans without waiting if
/// the update still holds its write guard. The resulting assertion is bounded.
pub(crate) struct ConstructionManagerReentryProbe {
    pub(crate) data: game_engine::common::thing::module::ModuleDataHandle,
    pub(crate) world: Arc<crate::system::engine_stores::WorldServices>,
    pub(crate) observations: Arc<std::sync::atomic::AtomicUsize>,
    pub(crate) building_id: ObjectID,
    pub(crate) builder_id: ObjectID,
    pub(crate) builder: Arc<RwLock<Object>>,
}

impl game_engine::common::system::Snapshotable for ConstructionManagerReentryProbe {
    fn crc(&self, _: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl game_engine::common::thing::module::CreateInterface for ConstructionManagerReentryProbe {
    fn on_create(&self) {}
    fn on_build_complete(&self) {
        use std::sync::atomic::Ordering;
        self.observations.fetch_or(1, Ordering::SeqCst);
        let construction = &self.world.logic_managers().construction;
        if let Ok(manager) = construction.try_read() {
            self.observations.fetch_or(2, Ordering::SeqCst);
            if !manager.is_under_construction(self.building_id) {
                self.observations.fetch_or(4, Ordering::SeqCst);
            }
        }
        if let Ok(mut manager) = construction.try_write() {
            // Completion must release the old dozer and building before
            // callbacks can start another real construction task.
            if manager.start_construction(self.building_id + 1, self.builder_id, 100.0, 4, false).is_ok() {
                self.observations.fetch_or(8, Ordering::SeqCst);
            }
        }
        if self.builder.try_write().is_ok() {
            self.observations.fetch_or(16, Ordering::SeqCst);
        }
    }
    fn should_do_on_build_complete(&self) -> bool {
        true
    }
}

impl game_engine::common::thing::module::Module for ConstructionManagerReentryProbe {
    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.data)
    }
    fn get_create_interface(
        &self,
    ) -> Option<&dyn game_engine::common::thing::module::CreateInterface> {
        Some(self)
    }
}

#[test]
fn dozer_update_completion_releases_manager_for_create_callback_with_same_id_peer() {
    use game_engine::common::thing::module::{BaseModuleData, ModuleDataHandle, ModuleInterfaceType};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    let builder_id = fixture.builder.read().unwrap().get_id();
    let (building_id, template) = {
        let mut structure = fixture.structure.write().unwrap();
        // Largest f32 below 100: the next actual construction tick completes.
        structure.set_construction_percent(f32::from_bits(100.0_f32.to_bits() - 1));
        (structure.get_id(), structure.get_template().clone())
    };
    let world = crate::system::engine_stores::active_services();
    let manager = &world.logic_managers().construction;
    let observations = Arc::new(AtomicUsize::new(0));
    let data = ModuleDataHandle::new(BaseModuleData::new());
    fixture.structure.write().unwrap().install_module_for_test(
        "ConstructionManagerReentryProbe",
        Box::new(ConstructionManagerReentryProbe {
            data: data.clone(),
            world: world.clone(),
            observations: observations.clone(),
            building_id,
            builder_id,
            builder: fixture.builder.clone(),
        }),
        data,
        ModuleInterfaceType::CREATE,
    );

    let peer = crate::system::engine_stores::new_for_world();
    let peer_manager = &peer.services().logic_managers().construction;
    peer_manager.write().unwrap()
        .start_construction(building_id, builder_id, 100.0, 4, false)
        .unwrap();
    assert_eq!(peer_manager.write().unwrap().update_for_dozer(builder_id), None);
    let mut other = Object::new_test_from_template(building_id, 100.0, template);
    other.bind_pathfinding_owner(&peer).unwrap();
    other.set_status(
        crate::common::ObjectStatusMaskType::from_status(
            crate::common::ObjectStatusTypes::UnderConstruction,
        ),
        true,
    );
    other.set_construction_percent(25.0);
    let other = Arc::new(RwLock::new(other));
    let mut peer_logic = crate::system::game_logic::GameLogic::new();
    crate::system::engine_stores::with_active_stores(&peer, || {
        peer_logic.register_object(other.clone()).unwrap();
    });

    let mut dozer = DozerAIUpdate::new(
        ModuleDataHandle::new(DozerAIUpdateModuleData::default()),
        builder_id,
    );
    // Enter the actual build action directly, without the legacy new_task
    // helper's unrelated target-lock reacquisition during docking admission.
    dozer.task_owner.current = DozerTask::Build;
    dozer.task_owner.requests[DozerTask::Build.as_index()].target_id = building_id;
    dozer.dozer_task = Some(DozerActionTask {
        task_type: DozerTask::Build,
        target_id: building_id,
        dock_point: None,
        failed_attempts: 0,
        build_total_frames: 1,
        build_max_health: 100.0,
        started_construction: false,
    });
    dozer.action_state = DozerActionState::DoAction;
    assert!(matches!(dozer.update(), StateReturnType::Continue));
    assert_eq!(observations.load(Ordering::SeqCst), 31);
    assert!(!fixture.structure.read().unwrap().is_under_construction());
    assert_eq!(
        fixture.player.read().unwrap().get_score_keeper().get_total_money_spent(),
        700
    );
    assert!(!manager.read().unwrap().is_under_construction(building_id));
    assert!(manager.read().unwrap().is_under_construction(building_id + 1));
    assert_eq!(peer_manager.read().unwrap().get_progress(building_id), Some(25.0));
    {
        let other = other.read().unwrap();
        assert!(other.is_under_construction());
        assert_eq!(other.get_construction_percent(), 25.0);
    }
    crate::system::engine_stores::with_active_stores(&peer, || {
        peer_logic.destroy_object(building_id);
        peer_logic.cleanup_dead_objects().unwrap();
    });
    manager.write().unwrap().cancel_dozer_construction(builder_id);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn construction_superweapon_alert_uses_structure_world_local_player() {
    const TEST: &str = "object::update::ai_update::dozer_ai_update::construction_callback_tests::construction_superweapon_alert_uses_structure_world_local_player";
    if matches!(
        crate::test_process::run_bounded(TEST, "DOZER_COMPLETION_EVA_CHILD"),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let _isolation = crate::object::registry::test_isolation_lock().lock().unwrap();
    let fixture = CompletionFixture::new(0);
    let owner = crate::system::engine_stores::new_world_services();
    let peer = crate::system::engine_stores::new_world_services();
    {
        let mut players = owner.players().write().unwrap();
        players.add_player(fixture.player.clone());
        players.add_player(Arc::new(RwLock::new(Player::new(1))));
        players.set_local_player_index(1);
    }
    {
        let mut players = peer.players().write().unwrap();
        players.add_player(Arc::new(RwLock::new(Player::new(0))));
        players.set_local_player_index(0);
    }
    let mut other = {
        let mut structure = fixture.structure.write().unwrap();
        structure.bind_owner_world_for_test(&owner);
        structure.set_special_power_available(
            crate::object::special_power_types::SpecialPowerType::NeutronMissile,
            true,
        );
        Object::new_test_from_template(structure.get_id(), 100.0, structure.get_template().clone())
    };
    other.bind_owner_world_for_test(&peer);
    other.set_special_power_available(
        crate::object::special_power_types::SpecialPowerType::NeutronMissile,
        true,
    );
    Player::on_structure_construction_complete(
        &fixture.player,
        None,
        &fixture.structure,
        true,
    )
    .unwrap();
    assert_eq!(
        crate::helpers::TheEva::drain_events_in(&owner).unwrap(),
        vec![crate::helpers::EvaEvent::SuperweaponDetectedAllyNuke]
    );
    assert!(crate::helpers::TheEva::drain_events_in(&peer).unwrap().is_empty());
    assert!(other.has_special_power(
        crate::object::special_power_types::SpecialPowerType::NeutronMissile
    ));
}
