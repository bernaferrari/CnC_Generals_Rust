use super::*;
use crate::common::{DefaultThingTemplate, KindOf};
use crate::player::{Player, PlayerList, player_list};
use crate::team::Team;

pub(crate) struct CompletionFixture {
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
            Some(crate::scripting::engine::ScriptEngine::new().unwrap()),
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
        let team = Arc::new(RwLock::new(Team::new("CompletionTeam".into(), 0xBA_0300)));
        team.write().unwrap().set_controlling_player_id(Some(0));
        let mut template = DefaultThingTemplate::new("CompletionStructure".into());
        template.add_kind_of(KindOf::Structure);
        template.add_kind_of(KindOf::Score);
        if power < 0 {
            template.add_kind_of(KindOf::Powered);
        }
        template.set_energy_production(power);
        template.parse_object_fields_from_ini(&std::collections::HashMap::from([(
            "BuildCost".to_owned(),
            "700".to_owned(),
        )]));
        let builder = Arc::new(RwLock::new(Object::new_test(0xBA_0301, 100.0)));
        let structure = Arc::new(RwLock::new(Object::new_test_from_template(
            0xBA_0302,
            100.0,
            Arc::new(template),
        )));
        for object in [&builder, &structure] {
            crate::object::registry::OBJECT_REGISTRY
                .register_object(object.read().unwrap().get_id(), object);
            object.write().unwrap().set_team(Some(team.clone()));
        }
        structure
            .write()
            .unwrap()
            .set_name("CompletionScriptName".into());
        // Populate names only through the construction notification below.
        assert_eq!(
            crate::scripting::engine::get_named_object_tracker()
                .get_object_id("CompletionScriptName")
                .unwrap(),
            None
        );
        structure.write().unwrap().set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::UnderConstruction,
            ),
            true,
        );
        Self {
            previous_players,
            previous_objects,
            previous_ai,
            previous_script,
            previous_names,
            player,
            builder,
            structure,
        }
    }

    pub(crate) fn complete(&self, rebuild: bool) {
        let mut dozer = DozerAIUpdate::new(
            DozerAIUpdateData::default(),
            self.builder.read().unwrap().get_id(),
        );
        dozer.handle_build_completion(&self.builder, &self.structure, rebuild);
    }
}

impl Drop for CompletionFixture {
    fn drop(&mut self) {
        for object in [&self.builder, &self.structure] {
            let id = object.read().unwrap().get_id();
            crate::object::registry::OBJECT_REGISTRY.unregister_object(id);
            crate::ai::object_registry::unregister_legacy_object(id);
        }
        *crate::scripting::engine::get_script_engine()
            .write()
            .unwrap() = self.previous_script.take();
        crate::scripting::engine::get_named_object_tracker()
            .replace_state_for_world_boundary(self.previous_names.take().unwrap());
        crate::ai::integration::replace_ai_integration_for_world_boundary(self.previous_ai.take());
        *crate::object_manager::get_object_manager().write().unwrap() =
            self.previous_objects.take().unwrap();
        *player_list().write().unwrap() = self.previous_players.take().unwrap();
    }
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
            Some(fixture.structure.read().unwrap().get_id())
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
        let fixture = CompletionFixture::new(power);
        if disabled {
            fixture
                .structure
                .write()
                .unwrap()
                .set_disabled(crate::common::DisabledType::Held);
        }
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
    Player::on_structure_construction_complete(&fixture.player, None, &fixture.structure, true);
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
    let (template, team, position) = {
        let structure = fixture.structure.read().unwrap();
        (
            structure.get_template().clone(),
            structure.get_team(),
            *structure.get_position(),
        )
    };
    let instance = crate::object_manager::GameObjectInstance::from_existing(
        fixture.structure.clone(),
        Some(template),
        team,
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
