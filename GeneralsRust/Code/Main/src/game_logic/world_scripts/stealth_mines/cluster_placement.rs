//! C++ ClusterMinesBomb field placement at the original accepted drop.
use super::*;

impl GameLogic {
    /// Resolve the bomb's authored GenerationFX from this session's admitted
    /// content. The process ThingFactory is only a legacy fallback for worlds
    /// that have not admitted a receiving catalog yet.
    fn cluster_mines_generation_fx_name(&self) -> Option<String> {
        use crate::game_logic::host_cluster_mines_flight::CLUSTER_MINES_BOMB_OBJECT;

        let Some(content) = self.receiving_module_content.as_ref() else {
            return Some(
                crate::game_logic::host_cluster_mines_flight::cluster_mines_generation_fx_name(),
            );
        };
        let template = content.admitted_template(CLUSTER_MINES_BOMB_OBJECT)?;
        for entry in template.behavior_modules().iter() {
            if !entry
                .name
                .as_str()
                .eq_ignore_ascii_case("GenerateMinefieldBehavior")
            {
                continue;
            }
            let typed_name = entry
                .data
                .downcast_ref::<gamelogic::object::behavior::GenerateMinefieldBehaviorModuleData>()
                .and_then(|data| data.generation_fx.as_deref());
            let authored_name = typed_name.or_else(|| entry.data.get_ini_field("GenerationFX"));
            return authored_name
                .map(str::trim)
                .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("none"))
                .map(ToOwned::to_owned);
        }
        None
    }

    /// Resolve the initial mine template selected by C++ while `m_upgraded`
    /// is false. The cluster residual currently has no per-bomb behavior state
    /// for the later `m_mineNameUpgraded` transition.
    fn cluster_mines_initial_mine_template_name(&self) -> Option<String> {
        use crate::game_logic::host_cluster_mines_flight::CLUSTER_MINES_BOMB_OBJECT;
        use crate::game_logic::host_mines::CLUSTER_MINES_MINE_TEMPLATE;

        let Some(content) = self.receiving_module_content.as_ref() else {
            return Some(CLUSTER_MINES_MINE_TEMPLATE.to_string());
        };
        let template = content.admitted_template(CLUSTER_MINES_BOMB_OBJECT)?;
        for entry in template.behavior_modules().iter() {
            if !entry
                .name
                .as_str()
                .eq_ignore_ascii_case("GenerateMinefieldBehavior")
            {
                continue;
            }
            let typed_name = entry
                .data
                .downcast_ref::<gamelogic::object::behavior::GenerateMinefieldBehaviorModuleData>()
                .map(|data| data.mine_name.as_str())
                .filter(|name| !name.trim().is_empty());
            return typed_name
                .or_else(|| entry.data.get_ini_field("MineName"))
                .map(str::trim)
                .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("none"))
                .map(ToOwned::to_owned);
        }
        None
    }

    /// Cluster Mines special-power residual: SmartBorder field around the drop.
    /// C++ ClusterMinesBomb GenerateMinefieldBehavior SmartBorder + DistanceAroundObject 80.
    pub fn place_cluster_mines(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        self.place_cluster_mines_with_effects(team, center, producer, &mut Default::default())
    }

    pub(crate) fn place_cluster_mines_with_effects(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            apply_cluster_mines_drop_variance, cluster_mines_drop_unit_samples,
        };
        let seed = producer.map(|p| p.0).unwrap_or(0).wrapping_add(self.frame);
        let (ux, uy) = cluster_mines_drop_unit_samples(seed);
        let center = apply_cluster_mines_drop_variance(center, ux, uy);
        self.place_cluster_mines_unvaried_with_effects(team, center, producer, None, effects)
    }

    /// SmartBorder around an already-offset drop (cargo-bomb impact).
    pub fn place_cluster_mines_unvaried(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        self.place_cluster_mines_unvaried_with_effects(
            team,
            center,
            producer,
            None,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_cluster_mines_unvaried_with_effects(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
        source_object: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::cluster_smart_border_positions;
        let Some(mine_template_name) = self.cluster_mines_initial_mine_template_name() else {
            return Vec::new();
        };
        // C++ placeMines returns before GenerationFX when the selected mine
        // template cannot be resolved.
        if self
            .receiving_module_content
            .as_ref()
            .is_some_and(|content| content.admitted_template(&mine_template_name).is_none())
        {
            return Vec::new();
        }
        let positions = cluster_smart_border_positions(center);
        let mut ids = Vec::with_capacity(positions.len());
        for pos in positions {
            if self.mine_spot_blocked(pos) {
                continue;
            }
            if let Some(id) = self.place_land_mine_named_with_effects(
                &mine_template_name,
                team,
                pos,
                producer,
                effects,
            ) {
                // C++ ClusterMinesBomb is the producer; scoot from impact, not the plane.
                self.apply_land_mine_scoot_with_effects(id, center, pos, effects);
                ids.push(id);
            }
        }
        if !ids.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new("MineFieldPlaced")
                    .with_position(center)
                    .with_priority(160),
            );
        }
        // C++ GenerateMinefieldBehavior::place_mines dispatches GenerationFX
        // only after every mine placement attempt has completed.
        if let Some(generation_fx) = self.cluster_mines_generation_fx_name() {
            if let Some(source_object) = source_object {
                let _ = self.dispatch_fx_list_at_host_object_with_effects(
                    &generation_fx,
                    source_object,
                    None,
                    effects,
                );
            } else {
                let _ = self.dispatch_fx_list_at_host_position_with_effects(
                    &generation_fx,
                    center,
                    effects,
                );
            }
        }
        ids
    }
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;
    use crate::game_logic::{NativeObjectRetirement, NativeSlaveSource, NativeViewEffects};
    use game_engine::common::ini::ini_fx_list::{FXList, FXNugget};
    use gamelogic::common::Coord3D;
    use std::sync::{Arc, Mutex};

    struct OrderedView {
        events: Arc<Mutex<Vec<&'static str>>>,
        particles: game_client::effects::particle_manager::ParticleSystemManager,
    }

    impl OrderedView {
        fn new(events: Arc<Mutex<Vec<&'static str>>>) -> Self {
            let mut particles =
                game_client::effects::particle_manager::ParticleSystemManager::new();
            particles.new_template("ClusterFxParticle".into());
            Self { events, particles }
        }
    }

    impl NativeViewEffects for OrderedView {
        fn local_player_id(&self) -> u32 {
            1
        }
        fn sample_slave(
            &mut self,
            _: &NativeSlaveSource,
            _: &str,
        ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
            crate::presentation_frame::camera::build::SlaveCameraSample::MissingObject
        }
        fn retire_physical_object_drawable(
            &mut self,
            _: &NativeObjectRetirement<'_>,
        ) -> Result<(), String> {
            Ok(())
        }
        fn host_epoch(&self) -> u64 {
            1
        }
        fn admit_factory_drawable(
            &mut self,
            _: crate::game_logic::NativeFactoryDrawableSource,
            _: game_client::core::game_client::NativeDrawableBirthContext,
            _: &mut game_engine::common::thing::module_factory::ModuleFactory,
        ) -> Result<(), String> {
            self.events.lock().unwrap().push("mine");
            Ok(())
        }
        fn particle_systems_mut(
            &mut self,
        ) -> Option<&mut game_client::effects::particle_manager::ParticleSystemManager> {
            Some(&mut self.particles)
        }
    }

    struct OrderedAudio(Arc<Mutex<Vec<&'static str>>>);
    impl crate::game_logic::NativeAudioEffects for OrderedAudio {
        fn add_audio_event(
            &mut self,
            _: &game_engine::common::audio::AudioEventRts,
            _: &game_engine::common::audio::AudioSubmissionLocality,
        ) {
        }
        fn submit_fx_sound(
            &mut self,
            name: &str,
            position: Option<&Coord3D>,
            player: Option<i32>,
            _: &game_engine::common::audio::AudioSubmissionLocality,
            _: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
            _: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
        ) -> Result<(), String> {
            assert_eq!(name, "ClusterGenerationFxSound");
            assert_eq!(player, Some(1), "object-source FX carries its owner");
            assert_eq!(
                position,
                Some(&Coord3D::new(200.0, 200.0, 3.0)),
                "object-source FX uses the generating bomb pose"
            );
            self.0.lock().unwrap().push("fx");
            Ok(())
        }
    }

    #[test]
    fn receiving_generation_fx_runs_after_mine_factory_callbacks() {
        let mut world = GameLogic::new();
        world.clear_all_players();
        world.add_player(crate::game_logic::Player::new(
            1,
            Team::China,
            "receiver",
            true,
        ));
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .init_shroud_grid(256.0, 256.0);
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(1)
            .unwrap();
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "cluster-fx.ini".into(),
                "Object ClusterMinesBomb\nBehavior = GenerateMinefieldBehavior MinefieldTag\n MineName = ReceivingClusterMine\n SmartBorder = Yes\n DistanceAroundObject = 80\n GenerationFX = FX_ReceivingClusterMine\nEnd\nEnd\nObject ReceivingClusterMine\nKindOf = LAND_MINE\nBody = ActiveBody BodyTag\nMaxHealth = 50\nInitialHealth = 50\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        ).unwrap();

        let mut definition = FXList::new("FX_ReceivingClusterMine".into());
        definition.add_nugget(FXNugget::Sound {
            name: "ClusterGenerationFxSound".into(),
        });
        definition.add_nugget(FXNugget::ParticleSystem {
            name: "ClusterFxParticle".into(),
            count: 1,
            offset: (3.0, 1.0, 0.0),
            radius: 0.0.into(),
            height: 0.0.into(),
            initial_delay: (-1.0).into(),
            rotate_x: 0.0,
            rotate_y: 0.0,
            rotate_z: 0.0,
            orient_to_object: true,
            ricochet: false,
            attach_to_object: false,
            create_at_ground_height: false,
            use_callers_radius: false,
        });
        let mut catalog = game_client::fx_list::FXListStore::new();
        catalog.add_fx_list(
            "FX_ReceivingClusterMine".into(),
            game_client::fx_list::FXList::from_definition(&definition),
        );
        world.admit_fx_catalog(catalog);

        let mut bomb = crate::game_logic::ThingTemplate::new("ClusterMinesBomb");
        bomb.set_health(50.0);
        world.templates.insert(bomb.name.clone(), bomb);
        let bomb_id = world
            .create_object_for_player("ClusterMinesBomb", 1, Vec3::new(200.0, 3.0, 200.0))
            .unwrap();
        let expected_transform;
        let (bomb, partition) = world.host_object_and_partition_mut(bomb_id).unwrap();
        bomb.cluster_mines_bomb = true;
        bomb.movement.velocity = Vec3::ZERO;
        bomb.set_orientation_with_partition_owner(0.75, partition);
        expected_transform = glam::Mat3::from_mat4(
            crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(
                bomb.get_transform_matrix(),
            ),
        );

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut view = OrderedView::new(events.clone());
        let mut audio = OrderedAudio(events.clone());
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            view: Some(&mut view),
            audio: Some(&mut audio),
            ..Default::default()
        };
        world
            .update_cluster_mines_flights_with_effects(&mut effects)
            .unwrap();
        drop(effects);
        let mine_count = world
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "ReceivingClusterMine")
            .count();
        assert!(mine_count > 0, "flight impact must create authored mines");
        let events = events.lock().unwrap();
        assert_eq!(events.last(), Some(&"fx"));
        assert!(
            events[..events.len() - 1]
                .iter()
                .all(|event| *event == "mine")
        );
        assert_eq!(events.len(), mine_count + 1);
        let particle = view
            .particles
            .find_particle_system(1)
            .expect("object FX particle nugget must run");
        assert_eq!(particle.local_transform(), expected_transform);
    }

    #[test]
    fn missing_receiving_mine_template_returns_before_generation_fx() {
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "cluster-fx-missing-mine.ini".into(),
                "Object ClusterMinesBomb\nBehavior = GenerateMinefieldBehavior MinefieldTag\n MineName = ChinaClusterMine\n SmartBorder = Yes\n DistanceAroundObject = 80\n GenerationFX = FX_ReceivingClusterMine\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        ).unwrap();

        let mut definition = FXList::new("FX_ReceivingClusterMine".into());
        definition.add_nugget(FXNugget::Sound {
            name: "ClusterGenerationFxSound".into(),
        });
        let mut catalog = game_client::fx_list::FXListStore::new();
        catalog.add_fx_list(
            "FX_ReceivingClusterMine".into(),
            game_client::fx_list::FXList::from_definition(&definition),
        );
        world.admit_fx_catalog(catalog);

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut view = OrderedView::new(events.clone());
        let mut audio = OrderedAudio(events.clone());
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            view: Some(&mut view),
            audio: Some(&mut audio),
            ..Default::default()
        };
        let ids = world.place_cluster_mines_unvaried_with_effects(
            Team::China,
            Vec3::new(200., 0., 200.),
            None,
            None,
            &mut effects,
        );
        assert!(ids.is_empty());
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn admitted_none_generation_fx_does_not_invent_the_retail_fx() {
        let mut world = GameLogic::new();
        world.clear_all_players();
        world.add_player(crate::game_logic::Player::new(
            1,
            Team::China,
            "receiver",
            true,
        ));
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .init_shroud_grid(256.0, 256.0);
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(1)
            .unwrap();
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "cluster-fx-none.ini".into(),
                "Object ClusterMinesBomb\nBehavior = GenerateMinefieldBehavior MinefieldTag\n MineName = ChinaClusterMine\n SmartBorder = Yes\n DistanceAroundObject = 80\nEnd\nEnd\nObject ChinaClusterMine\nKindOf = LAND_MINE\nBody = ActiveBody BodyTag\nMaxHealth = 50\nInitialHealth = 50\nEnd\nEnd\n".into(),
            )])
            .unwrap(),
        )
        .unwrap();

        let mut definition = FXList::new("WeaponFX_ClusterMineImpact".into());
        definition.add_nugget(FXNugget::Sound {
            name: "ClusterGenerationFxSound".into(),
        });
        let mut catalog = game_client::fx_list::FXListStore::new();
        catalog.add_fx_list(
            "WeaponFX_ClusterMineImpact".into(),
            game_client::fx_list::FXList::from_definition(&definition),
        );
        world.admit_fx_catalog(catalog);

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut view = OrderedView::new(events.clone());
        let mut audio = OrderedAudio(events.clone());
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            view: Some(&mut view),
            audio: Some(&mut audio),
            ..Default::default()
        };
        let ids = world.place_cluster_mines_unvaried_with_effects(
            Team::China,
            Vec3::new(200., 0., 200.),
            None,
            None,
            &mut effects,
        );
        assert!(!ids.is_empty());
        assert_eq!(events.lock().unwrap().len(), ids.len());
        assert!(events.lock().unwrap().iter().all(|event| *event == "mine"));
    }
}
