//! Real FX list definitions and mutable presentation owners, no global fixtures.
use super::*;
use game_engine::common::audio::audio_event_rts::AudioEventInfo;
use game_engine::common::audio::game_audio::AudioLocalityRelationship;

struct Players;
impl AudioLocalityResolver for Players {
    fn get_local_player_index(&self) -> Option<i32> {
        Some(3)
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

fn execute(
    catalog: &FXListStore,
    audio: &mut AudioManager,
    particles: &mut ParticleSystemManager,
    lights: &mut DisplayLightScene,
    scorches: &mut TerrainScorchBuffer,
) -> Result<(), String> {
    let mut logic = RandomState::default();
    let mut audio_random = RandomState::default();
    let mut client = RandomState::default();
    let mut view = View::new();
    let rules = GlobalData::default();
    let mut ground = |_| Some(91.0);
    let mut fx = FxExecution {
        catalog,
        particles,
        audio,
        audio_locality: &Players,
        logic_random: &mut logic,
        audio_random: &mut audio_random,
        client_random: &mut client,
        ground_height: &mut ground,
        view: &mut view,
        rules: &rules,
        scorches,
        lights,
        drawables: None,
        frame: 20,
    };
    fx.execute_named(
        "OwnedImpact",
        &FxInvocation {
            primary: Some(Coord3D::new(10.0, 20.0, 30.0)),
            matrix: Some(Matrix3D::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            secondary: None,
            primary_speed: 0.0,
            override_radius: 0.0,
            object: None,
            visible: true,
        },
    )
}

fn list(sound: &str) -> FXList {
    let mut list = FXList::new();
    list.add_fx_nugget(Box::new(ParticleSystemWrapper {
        nugget: ParticleSystemFXNugget {
            create_at_ground_height: true,
            offset: Vec3::new(2.0, 0.0, 0.0),
            ..ParticleSystemFXNugget::new("OwnedParticle".into())
        },
    }));
    list.add_fx_nugget(Box::new(LightPulseFXNugget {
        radius: 40.0,
        color: Vec3::ONE,
        ..Default::default()
    }));
    list.add_fx_nugget(Box::new(TerrainScorchFXNugget {
        radius: 4.0,
        scorch: 0,
    }));
    list.add_fx_nugget(Box::new(SoundFXNugget {
        sound_name: sound.into(),
    }));
    list
}

#[test]
fn actual_fx_list_creates_receiving_particle_light_scorch_and_sound() {
    let mut catalog = FXListStore::new();
    catalog.add_fx_list("OwnedImpact".into(), list("OwnedSound"));
    let mut particles = ParticleSystemManager::new();
    particles.register_template(Arc::new(
        crate::effects::particle_manager::ParticleSystemTemplate::new("OwnedParticle".into()),
    ));
    let mut audio = AudioManager::new();
    audio.init();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedSound".into(),
        sounds: vec!["owned_sound".into()],
        ..Default::default()
    });
    let mut lights = DisplayLightScene::default();
    let mut scorches = TerrainScorchBuffer::new();
    execute(
        &catalog,
        &mut audio,
        &mut particles,
        &mut lights,
        &mut scorches,
    )
    .unwrap();
    assert_eq!(particles.active_system_count(), 1);
    assert_eq!(
        particles.find_particle_system(1).unwrap().position(),
        Vec3::new(10.0, 22.0, 91.0)
    );
    assert_eq!(lights.lights().len(), 1);
    assert_eq!(lights.lights()[0].pos, [10.0, 20.0, 30.0]);
    assert_eq!(scorches.len(), 1);
    assert_eq!(audio.pending_play_request_count_for("OwnedSound"), 1);
}

#[test]
fn missing_late_sound_preflights_before_any_fx_owner_mutates() {
    let mut catalog = FXListStore::new();
    catalog.add_fx_list("OwnedImpact".into(), list("MissingSound"));
    let mut particles = ParticleSystemManager::new();
    particles.register_template(Arc::new(
        crate::effects::particle_manager::ParticleSystemTemplate::new("OwnedParticle".into()),
    ));
    let mut audio = AudioManager::new();
    let mut lights = DisplayLightScene::default();
    let mut scorches = TerrainScorchBuffer::new();
    assert!(
        execute(
            &catalog,
            &mut audio,
            &mut particles,
            &mut lights,
            &mut scorches
        )
        .unwrap_err()
        .contains("MissingSound")
    );
    assert_eq!(particles.active_system_count(), 0);
    assert!(lights.lights().is_empty());
    assert!(scorches.is_empty());
    assert_eq!(audio.pending_play_request_count(), 0);
}
