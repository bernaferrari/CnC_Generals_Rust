//! Fresh installed Body family: original Object.cpp wire and actual receiving
//! factory/team/power owners. No global admission or fixture mutex.
use super::*;
use crate::player::{OwnedPowerBrownout, Player, PlayerList, RadarEdgeServices};
use crate::team::TeamFactory;
use game_engine::common::thing::module::ModuleInterfaceType;
use game_engine::thing::thing_template::WeaponTemplateSet as EngineWeaponTemplateSet;
use std::collections::HashMap;

struct BodyTemplate {
    base: DefaultThingTemplate,
    modules: Vec<crate::common::TemplateModuleInfo>,
    weapons: Vec<EngineWeaponTemplateSet>,
}
impl fmt::Debug for BodyTemplate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BodyTemplate")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
impl ThingTemplate for BodyTemplate {
    fn get_name(&self) -> &AsciiString {
        self.base.get_name()
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
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
    fn get_behavior_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.modules
    }
    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        &self.weapons
    }
}
pub(in crate::object) struct Definitions {
    things: HashMap<String, Arc<dyn ThingTemplate>>,
    weapons: HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    locomotors: HashMap<String, Arc<crate::locomotor::LocomotorTemplate>>,
    upgrades: game_engine::common::system::upgrade::UpgradeCenter,
    bonuses: crate::weapon::WeaponBonusSet,
    body: Arc<crate::object::body::active_body::BodyDefinitionCatalog>,
    weapon_sets: Arc<ObjectWeaponSetCatalog>,
    module_factory: game_engine::common::thing::module_factory::ModuleFactory,
}
impl Definitions {
    pub(in crate::object) fn new(initial_health: f32) -> Self {
        Self::with_armor(initial_health, None)
    }
    fn with_armor(initial_health: f32, coefficient: Option<f32>) -> Self {
        Self::with_armor_and_weapon(initial_health, coefficient, None, false)
    }
    fn armed(initial_health: f32, clip_size: i32) -> Self {
        Self::with_armor_and_weapon(initial_health, None, Some(clip_size), false)
    }
    fn infantry_without_ai(initial_health: f32) -> Self {
        Self::with_armor_and_weapon(initial_health, None, None, true)
    }
    fn with_armor_and_weapon(
        initial_health: f32,
        coefficient: Option<f32>,
        clip_size: Option<i32>,
        infantry_without_ai: bool,
    ) -> Self {
        let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
        let mut armor = HashMap::new();
        if let Some(coefficient) = coefficient {
            let mut definition = crate::object::armor::ArmorTemplate::new();
            definition.set_default(coefficient);
            armor.insert("ReceivingArmor".to_string(), Arc::new(definition));
            body.default_armor_template = Some("ReceivingArmor".into());
        }
        body.max_health = 100.0;
        body.initial_health = initial_health;
        let mut weapons = HashMap::new();
        let mut sets = Vec::new();
        if let Some(clip_size) = clip_size {
            let mut weapon = crate::weapon::WeaponTemplate::new("ReceivingWeapon".to_string());
            weapon.clip_size = clip_size;
            weapons.insert("ReceivingWeapon".to_string(), Arc::new(weapon));
            let mut set = EngineWeaponTemplateSet::new();
            set.set_weapon_template_name(0, Some("ReceivingWeapon".to_string()));
            sets.push(set);
        }
        let mut base = DefaultThingTemplate::new("FreshBody".into());
        if infantry_without_ai {
            base.add_kind_of(KindOf::Infantry);
        }
        let template = Arc::new(BodyTemplate {
            base,
            modules: vec![crate::common::TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "FreshBodyState".into(),
                data: Arc::new(body),
                interface_mask: ModuleInterfaceType::BODY,
            }],
            weapons: sets.clone(),
        }) as Arc<dyn ThingTemplate>;
        let mut things = HashMap::from([("FreshBody".to_string(), template)]);
        if !sets.is_empty() {
            // A second named definition uses the same weapon but its own set
            // identity, just as two C++ ThingTemplates own distinct sets.
            things.insert(
                "ForeignBody".to_string(),
                Arc::new(BodyTemplate {
                    base: DefaultThingTemplate::new("ForeignBody".into()),
                    modules: Vec::new(),
                    weapons: sets,
                }),
            );
        }
        let mut module_factory = game_engine::common::thing::module_factory::ModuleFactory::new();
        crate::contain_module_overrides::register_native_module_overrides(&mut module_factory)
            .unwrap();
        for template in things.values() {
            for entry in template.get_behavior_module_info() {
                module_factory.add_module_internal(
                    None,
                    None,
                    game_engine::common::thing::module::ModuleType::Behavior,
                    &entry.name,
                    entry.interface_flags(),
                );
            }
        }
        let weapon_sets =
            Arc::new(ObjectWeaponSetCatalog::from_definitions(&things, &weapons).unwrap());
        Self {
            things,
            weapons,
            locomotors: HashMap::new(),
            upgrades: game_engine::common::system::upgrade::UpgradeCenter::new(),
            bonuses: Default::default(),
            body: Arc::new(
                crate::object::body::active_body::BodyDefinitionCatalog::new(
                    Some(&game_engine::common::global_data::GlobalData::default()),
                    armor,
                    HashMap::new(),
                )
                .unwrap(),
            ),
            weapon_sets,
            module_factory,
        }
    }
    fn borrowed(&self, frame: u32) -> ObjectXferDefinitions<'_> {
        ObjectXferDefinitions::with_catalog(
            &self.things,
            &self.weapons,
            &self.locomotors,
            &self.upgrades,
            &self.bonuses,
            self.body.clone(),
            self.weapon_sets.clone(),
            &self.module_factory,
            frame,
        )
        .unwrap()
    }
}

// This family's zero-energy/no-radar templates cannot issue an edge. A bad
// admission predicate must fail visibly rather than accept a fake audio effect.
struct NoRadarPayload(u32);
impl RadarEdgeServices for NoRadarPayload {
    fn has_single_player_been_defeated(&self, _: &Player) -> bool {
        false
    }
    fn client_quiet(&self) -> bool {
        true
    }
    fn in_logic_update(&self) -> bool {
        false
    }
    fn frame(&self) -> u32 {
        self.0
    }
    fn play_radar_edge(&mut self, _: crate::player::PlayerIndex, _: bool) {
        panic!("unpowered Body fixture must not request radar audio")
    }
}
pub(in crate::object) struct Candidate {
    factory: crate::object::object_factory::ObjectFactory,
    teams: TeamFactory,
    players: PlayerList,
    terrain: crate::terrain::TerrainLogic,
    collision: crate::object::collide::collision_system::CollisionSystem,
    changes: Vec<ObjectID>,
    terrain_moves: Vec<ObjectID>,
}
impl Candidate {
    pub(in crate::object) fn new() -> Self {
        let mut teams = TeamFactory::new();
        let mut players = PlayerList::new();
        for (index, name, team_id, neutral) in [
            (0, "ReceivingPlayer", 1, false),
            (1, "ReceivingNeutral", 2, true),
        ] {
            let player = Arc::new(RwLock::new(Player::new(index)));
            if neutral {
                player
                    .write()
                    .unwrap()
                    .set_player_type(crate::player::PlayerType::Neutral, false);
            }
            let prototype = teams
                .init_team_owned(
                    name.into(),
                    format!("Player{index}").into(),
                    false,
                    None,
                    Some(index as u32),
                )
                .unwrap();
            let team = teams
                .create_team_on_prototype_with_id(&prototype, team_id)
                .unwrap();
            player.write().unwrap().set_default_team(Some(team));
            players.add_player(player);
        }
        Self {
            factory: crate::object::object_factory::ObjectFactory::new(),
            teams,
            players,
            terrain: crate::terrain::TerrainLogic::new(),
            collision: crate::object::collide::collision_system::CollisionSystem::new(),
            changes: Vec::new(),
            terrain_moves: Vec::new(),
        }
    }
    pub(in crate::object) fn operate<R>(
        &mut self,
        definitions: &Definitions,
        frame: u32,
        f: impl FnOnce(&mut ObjectXferServices<'_>) -> R,
    ) -> R {
        let mut radar = NoRadarPayload(frame);
        let mut power = OwnedPowerBrownout {
            objects: crate::player::PowerObjectOwner::Factory(&self.factory),
            teams: &self.teams,
            players: &self.players,
            radar: &mut radar,
            frame,
            special_power_uses_delay: false,

            misc_audio: None,
            audio: None,
        };
        let changes = &mut self.changes;
        let terrain_moves = &mut self.terrain_moves;
        let mut changed = |id| changes.push(id);
        let mut moved = |object: &Object| terrain_moves.push(object.get_id());
        let mut services = ObjectXferServices {
            factory: &self.factory,
            teams: &self.teams,
            players: &self.players,
            terrain: &self.terrain,
            collision: &mut self.collision,
            power: &mut power,
            changed: &mut changed,
            terrain_moved: &mut moved,
            definitions: definitions.borrowed(frame),
        };
        f(&mut services)
    }
    pub(in crate::object) fn prepare_source(
        &mut self,
        definitions: &Definitions,
    ) -> Arc<RwLock<Object>> {
        self.prepare_source_with_id(definitions, 91)
    }
    pub(in crate::object) fn prepare_source_with_id(
        &mut self,
        definitions: &Definitions,
        id: ObjectID,
    ) -> Arc<RwLock<Object>> {
        let source = definitions
            .borrowed(17)
            .prepare_object(&mut self.factory, "FreshBody", id)
            .unwrap();
        let team = self
            .players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_default_team()
            .unwrap();
        self.operate(definitions, 17, |services| {
            services.admit_prepared_object(&source, team).unwrap()
        });
        source
    }
    pub(in crate::object) fn restore(
        &mut self,
        definitions: &Definitions,
        records: &[ObjectSnapshotRecord],
    ) -> Result<Vec<Arc<RwLock<Object>>>, String> {
        let prepared = definitions
            .borrowed(71)
            .prepare_objects(&mut self.factory, records)?;
        self.operate(definitions, 71, |services| services.load_objects(records))
            .map_err(|error| error.to_string())?;
        Ok(prepared)
    }
}

#[test]
fn fresh_body_object_xfer_preserves_actual_matrix_health_helpers_and_receiving_team() {
    let definitions = Definitions::new(75.0);
    let receiving = Definitions::new(4.0);
    let mut sender = Candidate::new();
    let source = sender.prepare_source(&definitions);
    let body_module = source
        .read()
        .unwrap()
        .modules
        .iter()
        .find(|module| module.name().as_str() == "ActiveBody")
        .unwrap()
        .clone();
    assert_eq!(body_module.tag().as_str(), "FreshBodyState");
    assert_eq!(
        body_module.with_module(|module| module.get_module_tag_name_key()),
        0,
        "the actual authored descriptor must resolve without name-key publication"
    );
    let matrix = Matrix3D::from_cols_array(&[
        2.0, 1.0, 3.0, 0.0, -1.0, 4.0, 5.0, 0.0, 6.0, 7.0, 8.0, 0.0, 11.0, 22.0, 33.0, 1.0,
    ]);
    sender.operate(&definitions, 17, |services| {
        let mut object = source.write().unwrap();
        object
            .set_snapshot_transform_with_services(
                &matrix,
                services.terrain,
                services.collision,
                services.definitions.frame,
                services.changed,
                services.terrain_moved,
            )
            .unwrap();
        object.set_name("SourceBody".into());
    });
    let expected_helpers = source.read().unwrap().ctor_helper_xfer_tags();
    let record = sender.operate(&definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    assert_eq!(record.bytes[0], 9);
    assert_eq!(record.bytes[5], 1);
    let mut expected = Vec::new();
    for row in 0..3 {
        for col in 0..4 {
            expected.extend_from_slice(&matrix.to_cols_array()[col * 4 + row].to_le_bytes());
        }
    }
    assert_eq!(
        &record.bytes[6..54],
        expected.as_slice(),
        "CPP xferMatrix3D 3x4 row wire"
    );
    let mut candidate = Candidate::new();
    let restored = candidate.restore(&receiving, &[record]).unwrap().remove(0);
    let object = restored.read().unwrap();
    assert_eq!(object.get_id(), 91);
    assert_eq!(object.get_name().as_str(), "SourceBody");
    assert_eq!(object.get_transform_matrix(), matrix);
    assert_eq!(*object.get_position(), Coord3D::new(11.0, 22.0, 33.0));
    assert_eq!(object.ctor_helper_xfer_tags(), expected_helpers);
    assert_eq!(
        object.behavior_module_xfer_count(),
        (expected_helpers.len() + 1) as u16
    );
    assert_eq!(
        object.get_body().unwrap().lock().unwrap().get_health(),
        75.0
    );
    assert!(Arc::ptr_eq(
        &object.thing_template,
        &receiving.things["FreshBody"]
    ));
    let team = object.get_team().unwrap();
    assert!(Arc::ptr_eq(
        &team,
        &candidate.teams.find_team_by_id(1).unwrap()
    ));
    assert!(!Arc::ptr_eq(
        &team,
        &sender.teams.find_team_by_id(1).unwrap()
    ));
    assert!(team.read().unwrap().get_members().contains(&91));
    assert!(
        !candidate
            .teams
            .find_team_by_id(2)
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .contains(&91)
    );
}

#[test]
fn failed_fresh_object_load_does_not_mutate_same_id_live_peer() {
    let definitions = Definitions::new(75.0);
    let mut live = Candidate::new();
    let owner = live.prepare_source(&definitions);
    owner.write().unwrap().set_name("LivePeer".into());
    let baseline = live.operate(&definitions, 17, |services| {
        services.save_object(&owner).unwrap()
    });
    let mut corrupt = ObjectSnapshotRecord {
        template_name: baseline.template_name.clone(),
        bytes: baseline.bytes.clone(),
    };
    corrupt.bytes.truncate(corrupt.bytes.len() - 3);
    let mut detached = Candidate::new();
    assert!(detached.restore(&definitions, &[corrupt]).is_err());
    let after = live.operate(&definitions, 17, |services| {
        services.save_object(&owner).unwrap()
    });
    assert_eq!(after.bytes, baseline.bytes);
    assert_eq!(owner.read().unwrap().get_name().as_str(), "LivePeer");
    assert_eq!(
        owner
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        75.0
    );
    assert!(
        live.teams
            .find_team_by_id(1)
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .contains(&91)
    );
}

#[test]
fn native_object_records_reject_trailing_bytes_missing_team_and_duplicate_ids() {
    let definitions = Definitions::new(75.0);
    let mut sender = Candidate::new();
    let source = sender.prepare_source(&definitions);
    let record = sender.operate(&definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    let mut trailing = ObjectSnapshotRecord {
        template_name: record.template_name.clone(),
        bytes: record.bytes.clone(),
    };
    trailing.bytes.push(1);
    assert!(
        Candidate::new()
            .restore(&definitions, &[trailing])
            .err()
            .unwrap()
            .contains("trailing Object bytes")
    );
    let mut missing = ObjectSnapshotRecord {
        template_name: record.template_name.clone(),
        bytes: record.bytes.clone(),
    };
    missing.bytes[54..58].copy_from_slice(&999u32.to_le_bytes());
    assert!(
        Candidate::new()
            .restore(&definitions, &[missing])
            .err()
            .unwrap()
            .contains("missing saved team 999")
    );
    let duplicate = ObjectSnapshotRecord {
        template_name: record.template_name.clone(),
        bytes: record.bytes.clone(),
    };
    let mut candidate = Candidate::new();
    assert!(
        candidate
            .restore(&definitions, &[record, duplicate])
            .err()
            .unwrap()
            .contains("duplicate saved Object id")
    );
    assert_eq!(candidate.factory.get_statistics().total_objects, 0);
}

#[test]
fn fresh_body_restore_resolves_same_named_armor_from_receiving_definitions() {
    let source_definitions = Definitions::with_armor(75.0, Some(0.75));
    let receiving_definitions = Definitions::with_armor(4.0, Some(0.25));
    let mut original = Candidate::new();
    let source = original.prepare_source(&source_definitions);
    let damage = crate::damage::DamageInfoInput {
        amount: 100.0,
        damage_type: crate::damage::DamageType::SmallArms,
        ..Default::default()
    };
    let body = source.read().unwrap().get_body().unwrap();
    assert_eq!(body.lock().unwrap().estimate_damage(&damage).unwrap(), 75.0);
    let record = original.operate(&source_definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    let mut receiving = Candidate::new();
    let restored = receiving
        .restore(&receiving_definitions, &[record])
        .unwrap()
        .remove(0);
    let restored_body = restored.read().unwrap().get_body().unwrap();
    // ActiveBody post-Xfer armor flags are dirty. The actual shared readonly
    // resolver must pick the receiving definition, rather than a global store
    // or the sender's cached armor pointer.
    assert_eq!(
        restored_body
            .lock()
            .unwrap()
            .estimate_damage(&damage)
            .unwrap(),
        25.0
    );
    assert_eq!(restored_body.lock().unwrap().get_health(), 75.0);
    assert_eq!(body.lock().unwrap().estimate_damage(&damage).unwrap(), 75.0);
}

#[test]
fn fresh_armed_object_restore_preserves_ammo_and_deadlines_on_next_weapon_set_update() {
    use crate::weapon::{WeaponSlotType, WeaponStatus};

    let source_definitions = Definitions::armed(75.0, 3);
    let receiving_definitions = Definitions::armed(4.0, 7);
    let mut original = Candidate::new();
    let source = original.prepare_source(&source_definitions);
    {
        let mut object = source.write().unwrap();
        let weapon = object
            .weapon_set
            .get_weapon_in_slot_mut(WeaponSlotType::Primary)
            .unwrap();
        assert_eq!(weapon.ammo_in_clip, 3);
        // These are original Weapon::xfer fields, not a second serializer or
        // a callback replay. A changed set pointer would reload all seven
        // rounds from the receiving definition on the next normal update.
        weapon.ammo_in_clip = 1;
        weapon.when_we_can_fire_again = 223;
        weapon.when_pre_attack_finished = 65;
        weapon.when_last_reload_started = 11;
        weapon.last_fire_frame = 29;
        weapon.store_status(WeaponStatus::BetweenFiringShots);
    }
    let record = original.operate(&source_definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    let mut receiving = Candidate::new();
    let restored = receiving
        .restore(&receiving_definitions, &[record])
        .unwrap()
        .remove(0);

    let observe = |object: &Object| {
        let weapon = object
            .weapon_set
            .get_weapon_in_slot(WeaponSlotType::Primary)
            .unwrap();
        assert!(Arc::ptr_eq(
            weapon.get_template(),
            &receiving_definitions.weapons["ReceivingWeapon"]
        ));
        assert_eq!(weapon.get_template().clip_size, 7);
        assert_eq!(weapon.get_remaining_ammo_at(72), 1);
        assert_eq!(weapon.get_status_at(72), WeaponStatus::BetweenFiringShots);
        assert_eq!(weapon.when_we_can_fire_again, 223);
        assert_eq!(weapon.when_pre_attack_finished, 65);
        assert_eq!(weapon.when_last_reload_started, 11);
        assert_eq!(weapon.last_fire_frame, 29);
    };
    {
        let object = restored.read().unwrap();
        observe(&object);
        let installed = object.weapon_set.installed_definition_sets();
        let selected = object
            .weapon_set
            .find_weapon_template_set(&object.cur_weapon_set_flags)
            .unwrap();
        assert!(Arc::ptr_eq(&installed[0], &selected));
        let first = receiving_definitions.borrowed(71);
        let next = receiving_definitions.borrowed(72);
        let own = first.weapon_sets("FreshBody").unwrap();
        let foreign = first.weapon_sets("ForeignBody").unwrap();
        let next_foreign = next.weapon_sets("ForeignBody").unwrap();
        assert!(Arc::ptr_eq(&installed[0], &own[0]));
        assert!(Arc::ptr_eq(&foreign[0], &next_foreign[0]));
        assert!(!Arc::ptr_eq(&own[0], &foreign[0]));
    }
    receiving.operate(&receiving_definitions, 72, |services| {
        let mut object = restored.write().unwrap();
        let id = object.get_id();
        let flags = object.cur_weapon_set_flags;
        let bonuses = object.get_weapon_bonus_condition();
        object
            .weapon_set
            .update_weapon_set_with_services(
                id,
                &flags,
                bonuses,
                services.definitions.frame,
                services.definitions.weapon_bonuses,
            )
            .unwrap();
        observe(&object);
    });
    assert_eq!(
        source
            .read()
            .unwrap()
            .weapon_set
            .get_weapon_in_slot(WeaponSlotType::Primary)
            .unwrap()
            .ammo_in_clip,
        1
    );
}

#[test]
fn fresh_infantry_without_authored_ai_preserves_no_ai_through_object_restore() {
    let peer_definitions = Definitions::new(19.0);
    let mut peer = Candidate::new();
    let peer_object = peer.prepare_source(&peer_definitions);
    peer_object.write().unwrap().set_name("SameIdPeer".into());
    let peer_before = peer.operate(&peer_definitions, 17, |services| {
        services.save_object(&peer_object).unwrap()
    });

    let source_definitions = Definitions::infantry_without_ai(75.0);
    let receiving_definitions = Definitions::infantry_without_ai(4.0);
    let assert_no_ai = |object: &Object| {
        assert!(object.is_kind_of(KindOf::Infantry));
        assert!(!object.ambient_lifecycle_admitted);
        assert!(object.get_ai_update_interface().is_none());
        assert!(object.ai.is_none());
        assert!(
            object
                .modules
                .iter()
                .all(|module| module.name().as_str() != "AIUpdateInterface")
        );
    };
    let mut source_session = Candidate::new();
    let source = source_session.prepare_source(&source_definitions);
    assert!(matches!(
        source_session.factory.get_object(91),
        Some(crate::object::object_factory::GameObjectInstance::Unit(_))
    ));
    {
        let object = source.read().unwrap();
        assert_no_ai(&object);
        assert_eq!(
            object.get_body().unwrap().lock().unwrap().get_health(),
            75.0
        );
    }
    let record = source_session.operate(&source_definitions, 17, |services| {
        services.save_object(&source).unwrap()
    });
    let mut receiving = Candidate::new();
    let restored = receiving
        .restore(&receiving_definitions, &[record])
        .unwrap()
        .remove(0);
    assert!(matches!(
        receiving.factory.get_object(91),
        Some(crate::object::object_factory::GameObjectInstance::Unit(_))
    ));
    {
        let object = restored.read().unwrap();
        assert_no_ai(&object);
        assert_eq!(
            object.get_body().unwrap().lock().unwrap().get_health(),
            75.0
        );
        assert_eq!(
            object.behavior_module_xfer_count(),
            source.read().unwrap().behavior_module_xfer_count()
        );
        assert!(!Arc::ptr_eq(&restored, &source));
        assert!(!Arc::ptr_eq(&restored, &peer_object));
    }
    let peer_after = peer.operate(&peer_definitions, 17, |services| {
        services.save_object(&peer_object).unwrap()
    });
    assert_eq!(peer_after.bytes, peer_before.bytes);
    assert_eq!(
        peer_object.read().unwrap().get_name().as_str(),
        "SameIdPeer"
    );
}

#[test]
fn native_birth_uses_exact_receiving_constructor_catalog_without_fallback() {
    let first_definitions = Definitions::new(75.0);
    let second_definitions = Definitions::new(4.0);
    let mut first = Candidate::new();
    let mut second = Candidate::new();
    let first_object = first.prepare_source(&first_definitions);
    let second_object = second.prepare_source(&second_definitions);
    assert_eq!(
        first_object.read().unwrap().get_id(),
        second_object.read().unwrap().get_id()
    );
    assert_eq!(
        first_object
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        75.0
    );
    assert_eq!(
        second_object
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        4.0
    );

    // The definition still advertises ActiveBody and has its actual typed data,
    // but this receiving catalog has no registered constructor. A fresh recipe
    // or ambient factory fallback would erroneously let this birth succeed.
    let mut missing = Definitions::new(19.0);
    missing.module_factory = game_engine::common::thing::module_factory::ModuleFactory::new();
    let mut rejected = Candidate::new();
    let error = missing
        .borrowed(17)
        .prepare_object(&mut rejected.factory, "FreshBody", 91)
        .unwrap_err();
    assert!(error.contains("ActiveBody"), "{error}");
    assert!(rejected.factory.get_object(91).is_none());
    assert_eq!(
        first_object
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        75.0
    );
    assert_eq!(
        second_object
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        4.0
    );
}

#[path = "object_xfer_native_tests/factory_identity.rs"]
mod factory_identity;
#[path = "object_xfer_native_tests/saved_particles.rs"]
mod saved_particles;
