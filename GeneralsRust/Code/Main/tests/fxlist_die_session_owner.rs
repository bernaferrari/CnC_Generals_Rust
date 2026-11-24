use game_engine::common::common_services::CommonServicesScope;
use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::game_logic::{GameLogic, Player, Team};
use glam::Vec3;

#[test]
fn missing_session_death_fx_does_not_play_a_foreign_global_sound() {
    let _services = CommonServicesScope::new();
    // Offer a same-named Sound list through the GameClient store accessor.
    // C++ FXListDie.cpp:41-70 only invokes its authored list; an absent
    // receiving list must never synthesize a sound from any other catalog.
    game_client::fx_list::with_fx_list_store_mut(|s| {
        s.apply_source("FXList FX_ForeignDieSession\n Sound\n Name = ForeignDieSound\n End\nEnd\n")
            .expect("foreign global FX list");
    });

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Attacker", true));
    world.add_player(Player::new(1, Team::China, "Victim", false));
    world.admit_object_ini_sources([("FXDieOwner.ini".into(),
        "Object FXDieOwner\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FXListDie DieFx\n DeathFX = FX_ForeignDieSession\n End\nEnd\n\nObject FXDieAttacker\n KindOf = INFANTRY\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n WeaponSet\n Conditions = None\n Weapon = PRIMARY FXDieLethalShot\n End\nEnd\n".into(),
    )]).unwrap();
    const LETHAL_SHOT: &str = "FXDieLethalShot";
    gamelogic::initialize_weapon_store().unwrap();
    assert_eq!(
        register_weapons_from_ini_text(
            "Weapon FXDieLethalShot\n PrimaryDamage = 200\n AttackRange = 100\n DamageType = EXPLOSION\n AntiGround = Yes\n WeaponSpeed = 999999\n ProjectileObject = NONE\nEnd\n",
        ),
        1
    );
    let weapon = gamelogic::weapon::with_weapon_store(|store| {
        (**store.find_weapon_template(LETHAL_SHOT).unwrap()).clone()
    })
    .unwrap();
    world
        .admit_receiving_weapon_snapshot(vec![(LETHAL_SHOT.into(), weapon)])
        .unwrap();
    let source = world
        .create_object_for_player("FXDieAttacker", 0, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object_for_player("FXDieOwner", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let shot = world.attack_fire_weapon_update(source, victim, 1.0);
    let object = world
        .host_object(victim)
        .expect("onDie retains the victim until cleanup");
    assert!(
        !object.is_alive(),
        "the actual lethal shot must reach the victim, got {shot:?}"
    );
    assert!(
        object
            .pending_transition_damage_fx
            .iter()
            .all(|event| event.audio_name.as_deref() != Some("ForeignDieSound"))
    );
    assert!(
        world
            .queued_audio_events
            .iter()
            .all(|event| event.event_type != "ForeignDieSound")
    );
}

#[test]
fn authored_object_fx_delivers_each_owned_sound_in_order() {
    let _services = CommonServicesScope::new();
    use game_client::display::view::View;
    use game_client::effects::particle_manager::ParticleSystemManager;
    use game_client::fx_list::{DisplayLightScene, FXListStore, FxExecution, FxInvocation};
    use game_client::terrain::scorch_mesh::TerrainScorchBuffer;
    use game_engine::common::audio::audio_event_rts::AudioEventInfo;
    use game_engine::common::audio::game_audio::{
        AudioLocalityRelationship, AudioLocalityResolver, AudioManager,
    };
    use game_engine::common::global_data::GlobalData;
    use game_engine::common::random_value::RandomState;
    use gamelogic::common::{Coord3D, Matrix3D};
    use gamelogic::helpers::HostFxObjectPose;

    struct Players;
    impl AudioLocalityResolver for Players {
        fn get_local_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn is_player_active(&self, _: i32) -> bool {
            true
        }
        fn player_exists(&self, _: i32) -> bool {
            true
        }
        fn has_default_team(&self, _: i32) -> bool {
            true
        }
        fn get_relationship_to_local_team(&self, _: i32, _: i32) -> AudioLocalityRelationship {
            AudioLocalityRelationship::Allies
        }
    }
    let mut catalog = FXListStore::new();
    catalog.apply_source("FXList FX_DeathOwned\n Sound\n Name = DeathFirst\n End\n Sound\n Name = DeathSecond\n End\nEnd\n").unwrap();
    let mut audio = AudioManager::new();
    audio.init();
    for name in ["DeathFirst", "DeathSecond"] {
        audio.register_audio_event_info(AudioEventInfo {
            audio_name: name.into(),
            sounds: vec![format!("{name}.wav")],
            ..Default::default()
        });
    }
    let mut particles = ParticleSystemManager::new();
    let mut logic_random = RandomState::default();
    let mut audio_random = RandomState::default();
    let mut client_random = RandomState::default();
    let mut view = View::new();
    let rules = GlobalData::default();
    let mut scorches = TerrainScorchBuffer::new();
    let mut lights = DisplayLightScene::default();
    let mut ground = |_| Some(0.0);
    let audio_instance = game_engine::common::audio::AudioInstance::default();
    let mut execution = FxExecution {
        catalog: &catalog,
        particles: &mut particles,
        audio: &mut audio,
        audio_instance: &audio_instance,
        audio_locality: &Players,
        logic_random: &mut logic_random,
        audio_random: &mut audio_random,
        client_random: &mut client_random,
        ground_height: &mut ground,
        view: &mut view,
        rules: &rules,
        scorches: &mut scorches,
        lights: &mut lights,
        drawables: None,
        frame: 10,
    };
    let pose = |id, x| HostFxObjectPose {
        id,
        position: Coord3D::new(x, 0.0, 0.0),
        transform: Matrix3D::IDENTITY,
        player_index: 0,
        bounding_circle_radius: 1.0,
        is_shrouded: false,
    };
    execution
        .execute_named(
            "FX_DeathOwned",
            &FxInvocation::on_object(Some(pose(7, 1.0)), Some(pose(8, 2.0))),
        )
        .unwrap();
    let mut shrouded = pose(7, 1.0);
    shrouded.is_shrouded = true;
    execution
        .execute_named(
            "FX_DeathOwned",
            &FxInvocation::on_object(Some(shrouded), Some(pose(8, 2.0))),
        )
        .unwrap();
    let sounds: Vec<_> = audio
        .pending_play_events()
        .map(|event| event.get_event_name().to_owned())
        .collect();
    assert_eq!(sounds, ["DeathFirst", "DeathSecond"]);
}
