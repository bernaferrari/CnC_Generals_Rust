//! Actual map-loading candidate uses the receiving application OCL base.
use super::*;
use crate::save_load::{GameDifficulty, SaveFileType};
const MAPPED_ARMOR_SOURCE: &str = "Object MappedArmorActor\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 4\n GeometryHeight = 6\n ArmorSet\n Conditions = NONE\n Armor = SharedArmor\n DamageFX = None\n End\n Body = ActiveBody MappedArmorBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n";

struct OclNativeStageHarness {
    #[cfg(feature = "game_client")]
    graphics: GraphicsSystem,
    #[cfg(feature = "game_client")]
    pipeline: RenderPipeline,
}
impl OclNativeStageHarness {
    fn new() -> Self {
        #[cfg(feature = "game_client")]
        {
            ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
                width: 32,
                height: 32,
                ..Default::default()
            })
            .unwrap();
            let graphics = GraphicsSystem::new(
                ww3d_engine::device().unwrap(),
                ww3d_engine::queue().unwrap(),
                ww3d_engine::color_format().unwrap(),
                ww3d_engine::depth_format().unwrap(),
            )
            .unwrap();
            let pipeline = RenderPipeline::initialize(&graphics).unwrap();
            Self { graphics, pipeline }
        }
        #[cfg(not(feature = "game_client"))]
        Self {}
    }
    fn run<R>(
        &mut self,
        run: impl FnOnce(Option<&mut dyn crate::game_logic::NativeMapAdmission>) -> R,
    ) -> R {
        #[cfg(feature = "game_client")]
        {
            use crate::game_logic::NativeMapAdmission;
            let mut candidate =
                super::super::map_native_admission::NativeMapCandidate::with_epoch(91);
            let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
            let mut attempt = candidate
                .attempt(
                    self.pipeline.borrow_native_draw_services(),
                    &mut self.graphics,
                    0,
                    super::super::native_view_effects::receiving_native_drawable_policy(),
                    &mut retirement,
                    None,
                )
                .unwrap();
            let result = run(Some(&mut attempt));
            // Staging assertions inspect the returned logical owner; this fixture
            // never commits its native world. Retire through actual services.
            attempt.discard_attempt().unwrap();
            drop(attempt);
            result
        }
        #[cfg(not(feature = "game_client"))]
        run(None)
    }
    fn finish(self) {
        drop(self);
        #[cfg(feature = "game_client")]
        ww3d_engine::shutdown().unwrap();
    }
}
fn list(model: &str) -> String {
    format!(
        "ObjectCreationList SameOCL\n CreateDebris\n ModelNames={model}\n Count=1\n End\n End\n"
    )
}
fn selected(world: &GameLogic) -> String {
    let list = world.object_creation_list_definition("SameOCL").unwrap();
    list.nuggets()[0]
        .as_any()
        .downcast_ref::<gamelogic::object_creation_list::nuggets::GenericObjectCreationNugget>()
        .unwrap()
        .names[0]
        .clone()
}
#[test]
fn actual_saved_map_ocl_candidate_uses_own_base_and_solo_without_mutating_active_or_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_saved_map_ocl_candidate_uses_own_base_and_solo_without_mutating_active_or_peer",
        || {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("SavedOCL.map");
            let mut map = game_engine::common::system::DataChunkOutput::new();
            // The real SaveFileManager stage also prepares native terrain;
            // supply a genuine small height chunk, not fallback geometry.
            map.open_data_chunk("HeightMapData", 3);
            for value in [8, 8, 0, 64] {
                map.write_int(value);
            }
            for _ in 0..64 {
                map.write_byte(40);
            }
            map.close_data_chunk();
            map.open_data_chunk("GlobalLighting", 1);
            map.write_int(2);
            for _ in 0..4 {
                for _ in 0..2 {
                    for value in [0.1, 0.2, 0.3, 0.8, 0.8, 0.8, 0., 0., -1.] {
                        map.write_real(value);
                    }
                }
            }
            map.close_data_chunk();
            std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
            std::fs::write(
                directory.path().join("Map.ini"),
                format!(
                    "{}Armor SharedArmor\n Armor = DEFAULT 50%\nEnd\n",
                    list("SavedMap")
                ),
            )
            .unwrap();
            std::fs::write(
                directory.path().join("Solo.ini"),
                format!(
                    "{}Armor SharedArmor\n Armor = DEFAULT 25%\nEnd\n",
                    list("SavedSolo")
                ),
            )
            .unwrap();
            let mut native = OclNativeStageHarness::new();
            let mut source = GameLogic::new();
            source
                .admit_receiving_module_content(
                    crate::assets::ReceivingObjectContent::from_sources([(
                        "mapped-armor-stage.ini".to_owned(),
                        MAPPED_ARMOR_SOURCE.to_owned(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            source
                .admit_ocl_application_sources(&[list("Application")])
                .unwrap();
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            let config = crate::skirmish_config::golden_skirmish_config(path.to_str().unwrap());
            crate::skirmish_config::apply_skirmish_config(&mut source, &config).unwrap();
            assert!(source.load_map(path.to_str().unwrap()));
            assert_eq!(selected(&source), "SavedSolo");
            // Exercise the same full Map.ini/Solo.ini dispatch for Armor, then
            // the real factory and later the actual encoded SaveFileManager.
            // Independent receiving definition admission, not snapshot-derived
            // metadata or manually installed Some(empty) module identities.
            let mut parser = crate::assets::IniParser::new();
            parser
                .parse_ini_content(MAPPED_ARMOR_SOURCE, "mapped-armor-stage.ini")
                .unwrap();
            let definition = parser.get_definition("MappedArmorActor").unwrap();
            let actor = GameLogic::build_template_from_object_definition(
                "MappedArmorActor",
                definition,
                None,
            );
            assert_eq!(
                actor.body_kind,
                Some(crate::game_logic::thing::BodyKind::Active)
            );
            assert!(actor.native_die_declarations.is_some());
            assert!(actor.native_lifecycle_definitions.is_some());
            assert!(actor.client_module_declarations.is_some());
            assert_eq!(actor.armor_sets[0].armor.as_deref(), Some("SharedArmor"));
            assert!(
                source.templates[&actor.name]
                    .receiving_client_template
                    .is_some(),
                "factory keeps the paired receiving definition rather than an independent projection"
            );
            let actor_id = source
                .create_object_for_player("MappedArmorActor", 0, glam::Vec3::new(30.0, 0.0, 30.0))
                .unwrap();
            fn armor(coefficient: f32) -> Arc<gamelogic::object::armor::ArmorDefinitions> {
                let mut definitions = gamelogic::object::armor::ArmorDefinitions::new();
                let mut row = gamelogic::object::armor::ArmorTemplate::new();
                row.set_default(coefficient);
                definitions.register_template("SharedArmor", row);
                Arc::new(definitions)
            }
            fn adjusted(world: &GameLogic, id: crate::game_logic::ObjectId) -> f32 {
                crate::game_logic::host_armor_residual::apply_residual_armor(
                    world.host_object(id).unwrap(),
                    crate::game_logic::combat::DamageType::Explosive,
                    100.0,
                )
            }
            assert_eq!(
                adjusted(&source, actor_id),
                25.0,
                "Solo armor overrides Map armor before factory callbacks"
            );
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let mut active = GameLogic::new();
            active
                .admit_ocl_application_sources(&[list("ActiveApplication")])
                .unwrap();
            active
                .apply_receiving_ocl_overrides(&[list("ActiveMap")])
                .unwrap();
            let mut peer = GameLogic::new();
            peer.admit_ocl_application_sources(&[list("Peer")]).unwrap();
            active.admit_armor_definitions(armor(2.0));
            peer.admit_armor_definitions(armor(2.0));
            let source_owner = source
                .get_player(
                    source
                        .host_object(actor_id)
                        .unwrap()
                        .owner_player_id
                        .unwrap(),
                )
                .unwrap();
            let mut receiving_owner = crate::game_logic::Player::new(
                source_owner.id,
                source_owner.team,
                "Peer controller",
                source_owner.is_human,
            );
            receiving_owner.map_side.map_player_name =
                source_owner.map_side.map_player_name.clone();
            receiving_owner.map_side.role = source_owner.map_side.role;
            peer.add_player(receiving_owner);
            peer.add_object(source.host_object(actor_id).unwrap().clone());
            assert_eq!(
                adjusted(&peer, actor_id),
                200.0,
                "same-ID incoming clone rebinds receiving peer armor"
            );
            let active_armor = active.current_armor_definitions().unwrap().clone();
            let peer_armor = peer.current_armor_definitions().unwrap().clone();
            let active_base = active.ocl_application_definitions();
            let peer_base = peer.ocl_application_definitions();
            let info = || SaveGameInfo {
                pristine_map_name: None,
                filename: "ocl".into(),
                display_name: "ocl".into(),
                description: "owned definitions".into(),
                map_name: path.to_str().unwrap().into(),
                campaign_side: None,
                mission_number: None,
                save_date: std::time::SystemTime::now(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: std::time::Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            let failed = native.run(|native| {
                CnCGameEngine::stage_decoded_saved_world_for_restore_with_native(
                    &snapshot,
                    info(),
                    None,
                    None,
                    None, // no paired TerrainVisual in this direct staging fixture
                    "ocl",
                    GameMode::Skirmish,
                    crate::save_load::CampaignMapContext::default(),
                    RestoreTemplateCatalog {
                        templates: &source.templates,
                        upgrade_definition_base: &source.upgrade_definition_base,
                        modules: &source.receiving_module_content,
                        player_templates: &source.player_template_definitions,
                        multiplayer_definitions: &source.multiplayer_definitions,
                        object_definitions: &source.receiving_object_definitions,
                        locomotors: &source.locomotor_definition_base,
                        baseline: &source.receiving_template_baseline,
                        lighting_baseline: source.lighting_rules_baseline(),
                        roads_baseline: source.terrain_road_baseline(),
                        combat_bonus_baseline: source.combat_bonus_baseline(),
                        tunnel_capacity_base: source.tunnel_capacity_base,
                        body_damage_baseline: source.body_damage_baseline(),
                        damage_fx_base: &source.damage_fx_base,
                        #[cfg(feature = "game_client")]
                        fx_baseline: source.fx_baseline(),
                        power_bar_baseline: source.power_bar_baseline(),
                        body_auto_particle_baseline: source.body_auto_particle_baseline(),
                        session_sell_percentage: 1.0,
                    },
                    &source.weapon_definition_base,
                    &source.projectile_definitions,
                    &active.ocl_application_definitions(),
                    source.ai_definitions.baseline(),
                    source.production_energy_baseline(),
                    source.physics_rules_baseline(),
                    source.effective_terrain_definitions(),
                    active.current_armor_definitions(),
                    active.session_view.camera.settings,
                    active.session_view.camera.clip_policy,
                    active.session_view.camera.shake_policy,
                    active.session_view.camera.movement_policy,
                    active.session_view.camera.input_policy,
                    &active.world_services,
                    |_, candidate| {
                        assert_eq!(selected(candidate), "SavedSolo");
                        assert_eq!(
                            candidate
                                .current_armor_definitions()
                                .unwrap()
                                .find_template("SharedArmor")
                                .unwrap()
                                .adjust_damage(gamelogic::damage::DamageType::Explosion, 100.0),
                            25.0
                        );
                        Err("after actual map admission".into())
                    },
                    native,
                )
            });
            assert!(failed.err().unwrap().contains("after actual map admission"));
            assert!(Arc::ptr_eq(
                active.current_armor_definitions().unwrap(),
                &active_armor
            ));
            assert!(Arc::ptr_eq(
                peer.current_armor_definitions().unwrap(),
                &peer_armor
            ));
            assert_eq!(adjusted(&source, actor_id), 25.0);
            assert_eq!(adjusted(&peer, actor_id), 200.0);
            assert_eq!(peer.host_object(actor_id).unwrap().health.current, 100.0);
            assert_eq!(selected(&active), "ActiveMap");
            assert_eq!(selected(&peer), "Peer");
            assert!(Arc::ptr_eq(
                &active_base,
                &active.ocl_application_definitions()
            ));
            assert!(Arc::ptr_eq(&peer_base, &peer.ocl_application_definitions()));
            let mut staged = native
                .run(|native| {
                    CnCGameEngine::stage_decoded_saved_world_for_restore_with_native(
                        &snapshot,
                        info(),
                        None,
                        None,
                        None, // no paired TerrainVisual in this direct staging fixture
                        "ocl",
                        GameMode::Skirmish,
                        crate::save_load::CampaignMapContext::default(),
                        RestoreTemplateCatalog {
                            templates: &source.templates,
                            upgrade_definition_base: &source.upgrade_definition_base,
                            modules: &source.receiving_module_content,
                            player_templates: &source.player_template_definitions,
                            multiplayer_definitions: &source.multiplayer_definitions,
                            object_definitions: &source.receiving_object_definitions,
                            locomotors: &source.locomotor_definition_base,
                            baseline: &source.receiving_template_baseline,
                            lighting_baseline: source.lighting_rules_baseline(),
                            roads_baseline: source.terrain_road_baseline(),
                            combat_bonus_baseline: source.combat_bonus_baseline(),
                            tunnel_capacity_base: source.tunnel_capacity_base,
                            body_damage_baseline: source.body_damage_baseline(),
                            damage_fx_base: &source.damage_fx_base,
                            #[cfg(feature = "game_client")]
                            fx_baseline: source.fx_baseline(),
                            power_bar_baseline: source.power_bar_baseline(),
                            body_auto_particle_baseline: source.body_auto_particle_baseline(),
                            session_sell_percentage: 1.0,
                        },
                        &source.weapon_definition_base,
                        &source.projectile_definitions,
                        &active.ocl_application_definitions(),
                        source.ai_definitions.baseline(),
                        source.production_energy_baseline(),
                        source.physics_rules_baseline(),
                        source.effective_terrain_definitions(),
                        active.current_armor_definitions(),
                        active.session_view.camera.settings,
                        active.session_view.camera.clip_policy,
                        active.session_view.camera.shake_policy,
                        active.session_view.camera.movement_policy,
                        active.session_view.camera.input_policy,
                        &active.world_services,
                        |snapshot, candidate| {
                            assert_eq!(selected(candidate), "SavedSolo");
                            builder
                                .restore_from_snapshot(snapshot, candidate)
                                .map_err(|error| error.to_string())
                        },
                        native,
                    )
                })
                .unwrap();
            assert_eq!(selected(&staged.logic), "SavedSolo");
            assert_eq!(adjusted(&staged.logic, actor_id), 25.0);
            assert!(Arc::ptr_eq(
                &active.ocl_application_definitions(),
                &staged.logic.ocl_application_definitions()
            ));
            staged.logic.reset_ocl_map_definitions();
            assert_eq!(selected(&staged.logic), "ActiveApplication");
            // A successfully staged candidate retains the transaction gate
            // through its combined commit/drop. Finish this candidate's real
            // assertions and retire its token before starting a second stage.
            drop(staged);
            // The complete archive route reads the real map from saved data,
            // rebuilds its definitions, and only then restores the live host.
            let saves_directory = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(saves_directory.path());
            saves.init().unwrap();
            #[cfg(feature = "game_client")]
            {
                let mut client = game_client::core::game_client::GameClient::new().unwrap();
                client.bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
                    &source.world_services,
                )));
                saves
                    .save_game_with_client_state(
                        "mapped_armor",
                        &source,
                        Default::default(),
                        &mut client,
                        &info(),
                    )
                    .unwrap();
            }
            #[cfg(not(feature = "game_client"))]
            saves.save_game("mapped_armor", &source, &info()).unwrap();
            let mut archived = native
                .run(|native| {
                    CnCGameEngine::stage_saved_world_for_restore_with_native(
                        &mut saves,
                        "mapped_armor",
                        GameMode::Skirmish,
                        crate::save_load::CampaignMapContext::default(),
                        RestoreTemplateCatalog {
                            templates: &source.templates,
                            upgrade_definition_base: &source.upgrade_definition_base,
                            modules: &source.receiving_module_content,
                            player_templates: &source.player_template_definitions,
                            multiplayer_definitions: &source.multiplayer_definitions,
                            object_definitions: &source.receiving_object_definitions,
                            locomotors: &source.locomotor_definition_base,
                            baseline: &source.receiving_template_baseline,
                            lighting_baseline: source.lighting_rules_baseline(),
                            roads_baseline: source.terrain_road_baseline(),
                            combat_bonus_baseline: source.combat_bonus_baseline(),
                            tunnel_capacity_base: source.tunnel_capacity_base,
                            body_damage_baseline: source.body_damage_baseline(),
                            damage_fx_base: &source.damage_fx_base,
                            #[cfg(feature = "game_client")]
                            fx_baseline: source.fx_baseline(),
                            power_bar_baseline: source.power_bar_baseline(),
                            body_auto_particle_baseline: source.body_auto_particle_baseline(),
                            session_sell_percentage: 1.0,
                        },
                        &source.weapon_definition_base,
                        &source.projectile_definitions,
                        &active.ocl_application_definitions(),
                        source.ai_definitions.baseline(),
                        source.production_energy_baseline(),
                        source.physics_rules_baseline(),
                        source.effective_terrain_definitions(),
                        active.current_armor_definitions(),
                        active.session_view.camera.settings,
                        active.session_view.camera.clip_policy,
                        active.session_view.camera.shake_policy,
                        active.session_view.camera.movement_policy,
                        active.session_view.camera.input_policy,
                        &active.world_services,
                        native,
                    )
                })
                .unwrap();
            assert_eq!(
                adjusted(&archived.logic, actor_id),
                25.0,
                "actual SaveFileManager stage rebinds the receiving saved-map armor"
            );
            let mut events = crate::game_logic::HostHealthEvents::default();
            archived
                .logic
                .host_object_mut(actor_id)
                .unwrap()
                .take_damage_from_typed(
                    20.0,
                    None,
                    crate::game_logic::combat::DamageType::Explosive,
                    0,
                    &mut events,
                );
            assert_eq!(
                archived.logic.host_object(actor_id).unwrap().health.current,
                95.0
            );
            assert_eq!(source.host_object(actor_id).unwrap().health.current, 100.0);
            assert_eq!(peer.host_object(actor_id).unwrap().health.current, 100.0);
            peer.host_object_mut(actor_id)
                .unwrap()
                .take_damage_from_typed(
                    20.0,
                    None,
                    crate::game_logic::combat::DamageType::Explosive,
                    0,
                    &mut events,
                );
            assert_eq!(peer.host_object(actor_id).unwrap().health.current, 60.0);
            assert_eq!(
                archived.logic.host_object(actor_id).unwrap().health.current,
                95.0
            );
            assert!(Arc::ptr_eq(
                active.current_armor_definitions().unwrap(),
                &active_armor
            ));
            assert!(Arc::ptr_eq(
                peer.current_armor_definitions().unwrap(),
                &peer_armor
            ));
            assert_eq!(selected(&active), "ActiveMap");
            assert_eq!(selected(&peer), "Peer");
            drop(archived);
            native.finish();
        },
    );
}
