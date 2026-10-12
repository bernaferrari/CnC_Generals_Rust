//! C++ ActionManager.cpp:502-653: REJECT_UNMANNED disables the pilot shortcut,
//! then ordinary containment admission still runs. These are admission queries
//! over actual prepared factory objects and borrowed native services, not capture
//! or PhysicsBehavior collision execution.

use super::{Object, ObjectPrivateStatusBits};
use crate::action_manager::CanEnterType;
use crate::ai::CommandSourceType;
use crate::common::{
    Coord3D, DefaultThingTemplate, DisabledType, KindOf, ObjectStatusMaskType, ObjectStatusTypes,
    Relationship, TemplateModuleInfo, ThingTemplate,
};
use crate::modules::ai_native_world::{AiNativeServices, BorrowedAiWorld};
use crate::modules::ai_world::AiWorld;
use crate::object::contain::{OpenContainModuleData, TransportContainModuleData};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, PlayerList};
use game_engine::common::audio::{AudioLocalityRelationship, AudioLocalityResolver, AudioManager};
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::{Arc, RwLock};

struct AdmissionTemplate {
    base: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
    slots: u8,
}
impl std::fmt::Debug for AdmissionTemplate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdmissionTemplate")
            .field("base", &self.base)
            .field("slots", &self.slots)
            .finish_non_exhaustive()
    }
}
impl ThingTemplate for AdmissionTemplate {
    fn get_name(&self) -> &crate::common::AsciiString {
        self.base.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.base.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_raw_transport_slot_count(&self) -> u8 {
        self.slots
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}
fn infantry(reject_unmanned: bool, slots: u8) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("UnmannedAdmissionInfantry".into());
    base.add_kind_of(KindOf::Infantry);
    if reject_unmanned {
        base.add_kind_of(KindOf::RejectUnmanned);
    }
    Arc::new(AdmissionTemplate {
        base,
        modules: Vec::new(),
        slots,
    })
}
fn carrier(module: Option<TemplateModuleInfo>) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("UnmannedAdmissionCarrier".into());
    base.add_kind_of(KindOf::Vehicle);
    Arc::new(AdmissionTemplate {
        base,
        modules: module.into_iter().collect(),
        slots: 1,
    })
}
fn open(allowed: KindOf, allow_enemies: bool) -> Arc<dyn ThingTemplate> {
    carrier(Some(TemplateModuleInfo {
        name: "OpenContain".into(),
        module_tag: "UnmannedAdmissionOpen".into(),
        data: Arc::new(
            crate::contain_module_overrides::ContainModuleDataAdapter::new(OpenContainModuleData {
                contain_max: 1,
                number_of_exit_paths: 0,
                allow_inside_kind_of: allowed.cpp_mask(),
                allow_enemies_inside: allow_enemies,
                ..Default::default()
            }),
        ),
        interface_mask: ModuleInterfaceType::CONTAIN,
    }))
}
fn transport(capacity: i32) -> Arc<dyn ThingTemplate> {
    carrier(Some(TemplateModuleInfo {
        name: "TransportContain".into(),
        module_tag: "UnmannedAdmissionTransport".into(),
        data: Arc::new(
            crate::contain_module_overrides::ContainModuleDataAdapter::new(
                TransportContainModuleData {
                    slot_capacity: capacity,
                    base: OpenContainModuleData {
                        contain_max: capacity,
                        number_of_exit_paths: 0,
                        allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ),
        interface_mask: ModuleInterfaceType::CONTAIN,
    }))
}

struct LocalPlayers<'a>(&'a PlayerList);
impl AudioLocalityResolver for LocalPlayers<'_> {
    fn get_local_player_index(&self) -> Option<i32> {
        Some(self.0.get_local_player_index())
    }
    fn is_player_active(&self, index: i32) -> bool {
        self.0
            .get_player(index)
            .is_some_and(|p| p.read().unwrap().is_player_active())
    }
    fn player_exists(&self, index: i32) -> bool {
        self.0.get_player(index).is_some()
    }
    fn has_default_team(&self, index: i32) -> bool {
        self.0
            .get_player(index)
            .is_some_and(|p| p.read().unwrap().get_default_team().is_some())
    }
    fn get_relationship_to_local_team(&self, source: i32, local: i32) -> AudioLocalityRelationship {
        let team = self
            .0
            .get_player(local)
            .unwrap()
            .read()
            .unwrap()
            .get_default_team()
            .unwrap();
        let team = team.read().unwrap();
        match self
            .0
            .get_player(source)
            .unwrap()
            .read()
            .unwrap()
            .get_relationship_with_team(&team)
        {
            Relationship::Allies => AudioLocalityRelationship::Allies,
            Relationship::Enemies => AudioLocalityRelationship::Enemies,
            _ => AudioLocalityRelationship::Neutral,
        }
    }
}

struct AdmissionMatch {
    factory: ObjectFactory,
    players: PlayerList,
    source: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
}
impl AdmissionMatch {
    fn new(reject: bool, slots: u8, target: Arc<dyn ThingTemplate>, hostile: bool) -> Self {
        let mut players = PlayerList::new();
        players.set_local_player_index(0);
        let mut teams = Vec::new();
        for index in 0..2 {
            let mut player = Player::new(index);
            player.set_player_relationship_by_index(1 - index, Relationship::Enemies);
            let team = Arc::new(RwLock::new(crate::team::Team::new(
                format!("UnmannedAdmissionTeam{index}").into(),
                index as u32 + 1,
            )));
            team.write()
                .unwrap()
                .set_controlling_player_id(Some(index as u32));
            player.set_default_team(Some(Arc::clone(&team)));
            teams.push(team);
            players.add_player(Arc::new(RwLock::new(player)));
        }
        let mut factory = ObjectFactory::new();
        let mut identity_owner = crate::system::game_logic::GameLogic::new();
        let source_id = factory
            .prepare_object(
                &mut identity_owner,
                infantry(reject, slots),
                Coord3D::ZERO,
                Some(Arc::clone(&teams[0])),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let target_id = factory
            .prepare_object(
                &mut identity_owner,
                target,
                Coord3D::new(10.0, 0.0, 0.0),
                Some(Arc::clone(&teams[usize::from(hostile)])),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let source = factory
            .get_object(source_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        // Author initial admission state without dispatching global disable callbacks.
        target
            .write()
            .unwrap()
            .disabled_mask
            .set_disabled(DisabledType::DisabledUnmanned);
        Self {
            factory,
            players,
            source,
            target,
        }
    }

    fn can_enter(&self, mode: CanEnterType) -> bool {
        let mut scheduler = crate::system::game_logic::GameLogic::new();
        let teams = crate::team::TeamFactory::new();
        let mut client_random = Default::default();
        let mut dirty_ui = Vec::new();
        let mut idle_workers = crate::helpers::IdleWorkerEvents::default();
        let terrain = crate::terrain::TerrainLogic::new();
        let mut pathfinder = crate::ai::Pathfinder::new();
        let mut collision = crate::object::collide::collision_system::CollisionSystem::new();
        let mut shroud = crate::system::shroud_manager::ShroudManager::new();
        shroud.init_shroud_grid(40.0, 40.0);
        let mut spatial = crate::object_manager::ObjectManager::new();
        let mut audio = AudioManager::new();
        let locality = LocalPlayers(&self.players);
        let mut random = Default::default();
        let rules = crate::ai::AiData::default();
        let bonuses = crate::weapon::WeaponBonusSet::default();
        let mut triggers = Vec::new();
        let mut terrain_moves = Vec::new();
        let world = BorrowedAiWorld::new(
            &self.factory,
            &self.players,
            AiNativeServices {
                groups: &[],
                disposal: None,
                current_update: None,
                update_scheduler: &mut scheduler,
                teams: &teams,
                client_random: &mut client_random,
                dirty_ui_objects: &mut dirty_ui,
                idle_worker_events: &mut idle_workers,
                terrain: &terrain,
                pathfinder: &mut pathfinder,
                collision: &mut collision,
                shroud: &mut shroud,
                spatial_objects: &mut spatial,
                audio: &mut audio,
                audio_locality: &locality,
                random: &mut random,
                rules: &rules,
                weapon_bonuses: &bonuses,
                changed_trigger_objects: &mut triggers,
                terrain_moved_objects: &mut terrain_moves,
                frame: 17,
                unlook_persist_frames: 2,
                slow_death_scale: 1.0,
                gravity: -0.1,
                movement_penalty: crate::locomotor::BodyDamageType::ReallyDamaged,
            },
        )
        .unwrap();
        let source = self.source.read().unwrap();
        let target = self.target.read().unwrap();
        assert!(Arc::ptr_eq(
            &world.object(source.get_id()).unwrap(),
            &self.source
        ));
        assert!(Arc::ptr_eq(
            &world.object(target.get_id()).unwrap(),
            &self.target
        ));
        assert!(source.get_contain().is_none());
        assert!(source.get_ai_update_interface().is_none());
        world.can_enter(&source, &target, CommandSourceType::FromScript, mode)
    }

    fn assert_query_has_no_capture_or_boarding(&self) {
        let source = self.source.read().unwrap();
        let target = self.target.read().unwrap();
        assert_eq!(source.get_contained_by(), None);
        assert!(!source.is_disabled_by_type(DisabledType::Held));
        assert!(!source.is_destroyed());
        assert!(target.is_disabled_by_type(DisabledType::DisabledUnmanned));
        assert!(!target.captured);
        if let Some(contain) = target.get_contain() {
            assert_eq!(contain.lock().unwrap().get_contained_count(), 0);
        }
    }
}

#[test]
fn reject_unmanned_falls_through_to_actual_open_and_transport_admission() {
    for target in [open(KindOf::Infantry, true), transport(1)] {
        let actual = AdmissionMatch::new(true, 1, target, false);
        assert!(actual.can_enter(CanEnterType::CheckCapacity));
        actual.assert_query_has_no_capture_or_boarding();
    }
}

#[test]
fn reject_unmanned_fallthrough_preserves_missing_contain_kind_and_relationship_rules() {
    for (target, hostile) in [
        (carrier(None), false),
        (open(KindOf::Vehicle, true), false),
        (open(KindOf::Infantry, false), true),
        (transport(1), true),
    ] {
        let actual = AdmissionMatch::new(true, 1, target, hostile);
        assert!(!actual.can_enter(CanEnterType::CheckCapacity));
        actual.assert_query_has_no_capture_or_boarding();
    }
}

#[test]
fn reject_unmanned_fallthrough_preserves_transport_capacity_and_zero_slot_rules() {
    let oversized = AdmissionMatch::new(true, 2, transport(1), false);
    assert!(!oversized.can_enter(CanEnterType::CheckCapacity));
    assert!(oversized.can_enter(CanEnterType::DontCheckCapacity));
    oversized.assert_query_has_no_capture_or_boarding();
    let zero_slot = AdmissionMatch::new(true, 0, transport(1), false);
    assert!(!zero_slot.can_enter(CanEnterType::CheckCapacity));
    assert!(!zero_slot.can_enter(CanEnterType::DontCheckCapacity));
    zero_slot.assert_query_has_no_capture_or_boarding();
}

#[test]
fn ordinary_unmanned_shortcut_requires_eligible_infantry_and_respects_prior_gates() {
    let actual = AdmissionMatch::new(false, 1, carrier(None), false);
    assert!(actual.can_enter(CanEnterType::CheckCapacity));
    actual.assert_query_has_no_capture_or_boarding();
    let rejected = AdmissionMatch::new(true, 1, carrier(None), false);
    assert!(!rejected.can_enter(CanEnterType::CheckCapacity));
    actual
        .target
        .write()
        .unwrap()
        .disabled_mask
        .clear(DisabledType::DisabledUnmanned);
    assert!(!actual.can_enter(CanEnterType::CheckCapacity));
    actual
        .target
        .write()
        .unwrap()
        .disabled_mask
        .set_disabled(DisabledType::DisabledUnmanned);
    for status in [
        ObjectStatusTypes::UnderConstruction,
        ObjectStatusTypes::Sold,
    ] {
        let mask = ObjectStatusMaskType::from_status(status);
        actual.target.write().unwrap().status |= mask;
        assert!(!actual.can_enter(CanEnterType::CheckCapacity));
        actual.target.write().unwrap().status &= !mask;
    }
    actual
        .target
        .write()
        .unwrap()
        .disabled_mask
        .set_disabled(DisabledType::DisabledSubdued);
    assert!(!actual.can_enter(CanEnterType::CheckCapacity));
    actual
        .target
        .write()
        .unwrap()
        .disabled_mask
        .clear(DisabledType::DisabledSubdued);
    actual.target.write().unwrap().private_status |= ObjectPrivateStatusBits::EffectivelyDead as u8;
    assert!(!actual.can_enter(CanEnterType::CheckCapacity));
}

#[test]
fn reject_unmanned_admission_uses_its_exact_factory_among_same_id_peers() {
    let allowed = AdmissionMatch::new(true, 1, open(KindOf::Infantry, true), false);
    let denied = AdmissionMatch::new(true, 1, open(KindOf::Vehicle, true), false);
    assert_eq!(
        allowed.source.read().unwrap().get_id(),
        denied.source.read().unwrap().get_id()
    );
    assert_eq!(
        allowed.target.read().unwrap().get_id(),
        denied.target.read().unwrap().get_id()
    );
    assert!(!Arc::ptr_eq(&allowed.source, &denied.source));
    assert!(!Arc::ptr_eq(&allowed.target, &denied.target));
    assert!(allowed.can_enter(CanEnterType::CheckCapacity));
    assert!(!denied.can_enter(CanEnterType::CheckCapacity));
    assert!(allowed.can_enter(CanEnterType::CheckCapacity));
    allowed.assert_query_has_no_capture_or_boarding();
    denied.assert_query_has_no_capture_or_boarding();
    drop(denied);
    assert!(allowed.can_enter(CanEnterType::CheckCapacity));
    allowed.assert_query_has_no_capture_or_boarding();
}
