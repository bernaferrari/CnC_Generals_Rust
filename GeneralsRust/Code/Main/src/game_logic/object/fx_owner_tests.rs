//! Execute authored client nuggets at the real native onDie/UndeadBody boundary.
use super::*;
use crate::game_logic::{DamageHitContext, GameLogic, ObjectId, Player, Team, ThingTemplate};
use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Sender, channel};

struct ObserveAudio {
    source: game_client::fx_list::FxObservation,
    secondary: Option<HostFxObjectPose>,
    events: Sender<(u8, HostFxObjectPose, Option<HostFxObjectPose>)>,
}
impl crate::game_logic::NativeAudioEffects for ObserveAudio {
    fn add_audio_event(
        &mut self,
        _: &game_engine::common::audio::AudioEventRts,
        _: &game_engine::common::audio::AudioSubmissionLocality,
    ) {
        unreachable!("FX uses explicit sound admission");
    }
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&Coord3D>,
        player: Option<i32>,
        _: &game_engine::common::audio::AudioSubmissionLocality,
        _: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        _policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let primary = self
            .source
            .object
            .expect("object callback retains source identity");
        assert_eq!(position, self.source.primary.as_ref());
        assert_eq!(player, Some(primary.player_index));
        let marker = name.strip_prefix("ObjectOwner_").unwrap().parse().unwrap();
        self.events.send((marker, primary, self.secondary)).unwrap();
        Ok(())
    }
}
fn install(catalog: &mut game_client::fx_list::FXListStore, name: &str, shake: bool, marker: u8) {
    let mut definition = FXList::new(name.into());
    if shake {
        definition.add_nugget(FXNugget::ViewShake {
            shake_type: CameraShakeType::Normal,
        });
    }
    definition.add_nugget(FXNugget::Sound {
        name: format!("ObjectOwner_{marker}").into(),
    });
    catalog.add_fx_list(
        name.into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
}
fn with_audio<R>(
    world: &mut GameLogic,
    id: ObjectId,
    events: &Sender<(u8, HostFxObjectPose, Option<HostFxObjectPose>)>,
    operation: impl FnOnce(&mut GameLogic, &mut crate::game_logic::ScriptPresentationEffects<'_>) -> R,
) -> R {
    use game_client::fx_list::{FxSource, GameClientFxRuntime};
    let secondary = world.objects[&id].last_damage_source;
    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    let source = crate::game_logic::host_fx_list_dispatch::HostFxRuntime {
        world,
        effects: &mut effects,
    }
    .observe(FxSource::Objects {
        primary: id.0,
        secondary: secondary.map(|id| id.0),
    });
    let secondary = secondary
        .and_then(|id| world.host_object(id))
        .map(|object| {
            GameLogic::observe_host_fx_object(
                object,
                world.local_player_id(),
                world.world_services.shroud(),
            )
        });
    let mut audio = ObserveAudio {
        source,
        secondary,
        events: events.clone(),
    };
    operation(
        world,
        &mut crate::game_logic::ScriptPresentationEffects {
            audio: Some(&mut audio),
            ..Default::default()
        },
    )
}
fn world(id: ObjectId, target: Vec3, catalog: &game_client::fx_list::FXListStore) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_fx_catalog(catalog.clone());
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world.session_view.target = target;
    let mut template = ThingTemplate::new("NativeDieOwner");
    template.set_health(100.);
    let mut object = Object::new(template, id, Team::USA);
    object.owner_player_id = Some(1);
    object.set_position(Vec3::new(10., 4., 12.));
    world.objects.insert(id, object);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
    world
}
fn expected_impulse(world: &GameLogic) -> f32 {
    let rules = world.session_view.camera.shake_policy.current();
    if rules.shake_normal_intensity > rules.max_shake_intensity {
        3.
    } else {
        rules.shake_normal_intensity
    }
}

#[test]
fn native_die_first_and_additional_modules_are_inline_owned_once_and_keep_killer() {
    let (sent, received) = channel();
    let mut catalog = game_client::fx_list::FXListStore::new();
    install(&mut catalog, "FX_NativeFirstDie", true, 1);
    install(&mut catalog, "FX_NativeAdditionalDie", false, 2);
    let id = ObjectId(9301);
    let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
    let range = owner
        .session_view
        .camera
        .shake_policy
        .current()
        .max_shake_range;
    let mut peer = world(id, Vec3::new(10. + range * 2., 0., 12.), &catalog);
    for world in [&mut owner, &mut peer] {
        let mut killer = Object::new(
            ThingTemplate::new("NativeKiller"),
            ObjectId(9302),
            Team::China,
        );
        killer.set_position(Vec3::new(25., 6., 50.));
        world.objects.insert(killer.id, killer);
        let object = world.objects.get_mut(&id).unwrap();
        object.last_damage_source = Some(ObjectId(9302));
        let mut die =
            crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx("FX_NativeFirstDie");
        die.more.push(
            crate::game_logic::host_fx_list_die::HostFxListDieData::with_fx(
                "FX_NativeAdditionalDie",
            ),
        );
        object.fx_list_die = Some(die);
    }
    with_audio(&mut peer, id, &sent, |world, effects| {
        world.fire_owned_fx_list_die_with_effects(id, effects)
    });
    assert_eq!(peer.session_view.impulse.intensity, 0.);
    let peer_events: Vec<_> = received.try_iter().collect();
    assert_eq!(peer_events.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2]);
    with_audio(&mut owner, id, &sent, |world, effects| {
        world.fire_owned_fx_list_die_with_effects(id, effects)
    });
    assert_eq!(
        owner.session_view.impulse.intensity,
        expected_impulse(&owner),
        "first native module cannot wait for the AI drain"
    );
    assert_eq!(
        owner.session_view.impulse.offset,
        Vec3::ZERO,
        "enqueue does not step the view"
    );
    let events: Vec<_> = received.try_iter().collect();
    assert_eq!(events.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2]);
    for (_, primary, secondary) in events {
        assert_eq!(primary.id, id.0);
        assert_eq!(primary.position, Coord3D::new(10., 12., 4.));
        let secondary = secondary.unwrap();
        assert_eq!(secondary.id, 9302);
        assert_eq!(secondary.position, Coord3D::new(25., 50., 6.));
    }
    assert!(owner.objects[&id].pending_death_fx.is_none());
    with_audio(&mut owner, id, &sent, |world, effects| {
        world.fire_owned_fx_list_die_with_effects(id, effects)
    });
    assert!(
        received.try_recv().is_err(),
        "authored DieMux once gate owns replay prevention"
    );
    let encoded = serde_json::to_vec(&owner.objects[&id]).unwrap();
    let mut restored = world(id, Vec3::ZERO, &catalog);
    restored
        .session_view
        .admit(owner.session_view.capture())
        .unwrap();
    restored
        .objects
        .insert(id, serde_json::from_slice(&encoded).unwrap());
    with_audio(&mut restored, id, &sent, |world, effects| {
        world.fire_owned_fx_list_die_with_effects(id, effects)
    });
    assert!(
        received.try_recv().is_err(),
        "restored fired modules cannot replay"
    );
    assert_eq!(
        owner.session_view.advance_impulse(),
        restored.session_view.advance_impulse()
    );
}

#[test]
fn native_second_life_start_uses_receiving_view_for_normal_and_indestructible_body() {
    let (sent, received) = channel();
    let mut catalog = game_client::fx_list::FXListStore::new();
    install(
        &mut catalog,
        crate::game_logic::host_battle_bus::BATTLE_BUS_FX_START_UNDEATH,
        true,
        7,
    );
    for indestructible in [false, true] {
        let id = ObjectId(9401);
        let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
        let range = owner
            .session_view
            .camera
            .shake_policy
            .current()
            .max_shake_range;
        let mut peer = world(id, Vec3::new(10. + range * 2., 0., 12.), &catalog);
        for world in [&mut owner, &mut peer] {
            let particle = world.combat_particles.spawn(
                crate::game_logic::CombatParticleKind::DeathSmoke,
                Vec3::ZERO,
                0,
                Some(id),
                None,
            );
            let mut transition =
                crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData::default();
            transition.enabled = true;
            transition.attached_ids[2] = vec![particle];
            let object = world.objects.get_mut(&id).unwrap();
            object.install_battle_bus_transport();
            object.indestructible = indestructible;
            object.transition_damage_fx = Some(transition);
        }
        for world in [&mut peer, &mut owner] {
            let damage = world.objects[&id].health.current + 100.;
            assert!(
                with_audio(world, id, &sent, |world, effects| world
                    .apply_owned_damage_with_killer_team_and_after_fx_with_effects(
                        id,
                        damage,
                        None,
                        crate::game_logic::combat::DamageType::Explosive,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                        None,
                        |world, victim_id| {
                            if !indestructible {
                                let victim = world.objects.get(&victim_id).unwrap();
                                assert_eq!(
                                    victim.body_damage_state,
                                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged,
                                    "the ActiveBody damage clamp leaves 1 HP before UndeadBody FULLY_HEAL"
                                );
                                let state_two_particle = victim
                                    .transition_damage_fx
                                    .as_ref()
                                    .unwrap()
                                    .attached_ids[2][0];
                                assert!(world.combat_particles.get(state_two_particle).unwrap().active);
                            }
                        },
                        effects
                    ))
                .is_some()
            );
            assert!(
                world.objects[&id]
                    .battle_bus_body
                    .as_ref()
                    .unwrap()
                    .is_in_first_death,
                "UndeadBody continuation must run after ActiveBody, indestructible={indestructible}"
            );
            assert_eq!(
                world.objects[&id].health.current,
                crate::game_logic::host_battle_bus::BATTLE_BUS_SECOND_LIFE_MAX_HEALTH
            );
            assert!(world.objects[&id].armor_set_second_life);
            let retained = world.objects[&id]
                .transition_damage_fx
                .as_ref()
                .unwrap()
                .attached_ids[2][0];
            assert!(
                world
                    .combat_particles
                    .get(retained)
                    .is_some_and(|entry| entry.active),
                "setMaxHealth(FULLY_HEAL) must not call TransitionDamageFX::onBodyDamageStateChange"
            );
        }
        assert_eq!(peer.session_view.impulse.intensity, 0.);
        assert_eq!(
            owner.session_view.impulse.intensity,
            expected_impulse(&owner)
        );
        let events: Vec<_> = received.try_iter().collect();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.0 == 7 && event.1.id == id.0)
        );
    }
}

#[test]
fn body_visual_change_replaces_auto_particles_immediately_in_receiving_world() {
    let (sent, _received) = channel();
    let catalog = game_client::fx_list::FXListStore::new();
    let id = ObjectId(9410);
    let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
    let mut peer = world(id, Vec3::new(40., 0., 12.), &catalog);
    let mut stale = Vec::new();
    for world in [&mut owner, &mut peer] {
        stale.push(world.combat_particles.spawn(
            crate::game_logic::CombatParticleKind::BodyFire,
            Vec3::ZERO,
            0,
            Some(id),
            None,
        ));
    }

    with_audio(&mut owner, id, &sent, |world, effects| {
        world
            .apply_owned_damage_with_killer_team_and_after_fx_with_effects(
                id,
                60.,
                None,
                crate::game_logic::combat::DamageType::Explosive,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
                None,
                |_, _| {},
                effects,
            )
            .unwrap();
    });

    assert_eq!(
        owner.objects[&id].body_damage_state,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
    );
    assert!(
        !owner.combat_particles.get(stale[0]).unwrap().active,
        "the receiving Body callback must retire stale auto-particles synchronously"
    );
    assert!(
        peer.combat_particles.get(stale[1]).unwrap().active,
        "a same-ID peer must retain its own body particle system"
    );
}

#[test]
fn aflame_edge_uses_object_status_when_flammable_state_already_advanced() {
    let mut object = Object::new(
        ThingTemplate::new("AflameEdgeOwner"),
        ObjectId(9411),
        Team::USA,
    );
    let mut spread = crate::game_logic::host_fire_spread::HostFireSpreadData::default();
    spread.state = crate::game_logic::host_fire_spread::HostFlammableState::Aflame;
    object.fire_spread = Some(spread);
    assert!(!object.has_object_status_bit("AFLAME"));

    let mut particle_callbacks = 0;
    object.apply_flammable_ignite_visuals_with_body_fx(None, 1, &mut |_, effect, _| {
        if matches!(
            effect,
            crate::game_logic::object::BodyStateFx::BodyParticleState
        ) {
            particle_callbacks += 1;
        }
    });

    assert_eq!(particle_callbacks, 1);
    assert!(object.has_object_status_bit("AFLAME"));
}

#[test]
fn flammable_ignition_callback_precedes_sound_spread_and_state_commit() {
    let mut template = ThingTemplate::new("IgnitionOrder");
    template.set_health(100.0);
    let mut object = Object::new(template, ObjectId(9411), Team::USA);
    let mut fire = crate::game_logic::host_fire_spread::HostFireSpreadData::tree_default();
    fire.min_spread_delay = 2;
    fire.max_spread_delay = 4;
    object.fire_spread = Some(fire);
    let mut audio = crate::game_logic::SessionAudio::default();
    let random = game_engine::common::random_value::SessionRandom::default();
    let mut body_particle_seen = false;
    let mut ignition_seen = false;

    object.apply_flammable_ignite_visuals_with_body_fx(None, 10, &mut |object, effect, _| {
        match effect {
            crate::game_logic::object::BodyStateFx::BodyParticleState => {
                assert!(!ignition_seen);
                body_particle_seen = true;
            }
            crate::game_logic::object::BodyStateFx::FlammableIgnition { frame } => {
                assert_eq!(frame, 10);
                assert!(body_particle_seen);
                assert!(object.has_object_status_bit("AFLAME"));
                let fire = object.fire_spread.as_ref().unwrap();
                assert!(matches!(
                    fire.state,
                    crate::game_logic::host_fire_spread::HostFlammableState::Normal
                ));
                assert_eq!(fire.aflame_end_frame, 0);
                ignition_seen = true;
                Object::complete_flammable_ignition(object, frame, &mut audio, &random);
            }
            effect => Object::standalone_body_fx(object, effect, None),
        }
    });

    let fire = object.fire_spread.as_ref().unwrap();
    assert!(ignition_seen);
    assert!(matches!(
        fire.state,
        crate::game_logic::host_fire_spread::HostFlammableState::Aflame
    ));
    assert_eq!(fire.aflame_end_frame, 10 + fire.aflame_duration);
    assert!(fire.next_spread_frame > 10);
    assert!(fire.burning_sound_active);
    assert_eq!(audio.len(), 1);
}

#[test]
fn direct_fire_spread_ignition_replaces_only_the_receiving_world_particles() {
    let catalog = game_client::fx_list::FXListStore::new();
    let id = ObjectId(9412);
    let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
    let mut peer = world(id, Vec3::new(40., 0., 12.), &catalog);
    let mut stale = Vec::new();
    for world in [&mut owner, &mut peer] {
        world.objects.get_mut(&id).unwrap().fire_spread =
            Some(crate::game_logic::host_fire_spread::HostFireSpreadData::tree_default());
        stale.push(world.combat_particles.spawn(
            crate::game_logic::CombatParticleKind::BodyFire,
            Vec3::ZERO,
            0,
            Some(id),
            None,
        ));
    }

    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    assert!(owner.ignite_object_fire_spread(id, &mut effects));

    assert!(owner.objects[&id].has_object_status_bit("AFLAME"));
    assert!(owner.objects[&id].fire_spread.as_ref().unwrap().is_aflame());
    assert!(!owner.combat_particles.get(stale[0]).unwrap().active);
    assert!(peer.combat_particles.get(stale[1]).unwrap().active);
}

#[test]
fn flame_damage_ignition_finishes_inline_in_the_receiving_world() {
    let catalog = game_client::fx_list::FXListStore::new();
    let id = ObjectId(9414);
    let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
    let mut peer = world(id, Vec3::new(40., 0., 12.), &catalog);
    let mut stale = Vec::new();
    for world in [&mut owner, &mut peer] {
        let mut fire = crate::game_logic::host_fire_spread::HostFireSpreadData::tree_default();
        fire.flame_damage_limit = 1.0;
        let object = world.objects.get_mut(&id).unwrap();
        object.fire_spread = Some(fire);
        stale.push(world.combat_particles.spawn(
            crate::game_logic::CombatParticleKind::BodyFire,
            Vec3::ZERO,
            0,
            Some(id),
            None,
        ));
    }
    let owner_stale = stale[0];
    let peer_stale = stale[1];

    let _ = owner.apply_owned_damage_with_effects(
        id,
        1.0,
        None,
        crate::game_logic::combat::DamageType::Flame,
        crate::game_logic::host_usa_pilot::HostDeathType::Burned,
        None,
        &DamageHitContext::default(),
        &mut crate::game_logic::ScriptPresentationEffects::default(),
    );

    let fire = owner.objects[&id].fire_spread.as_ref().unwrap();
    assert!(owner.objects[&id].has_object_status_bit("AFLAME"));
    assert!(fire.is_aflame());
    assert!(fire.burning_sound_active);
    assert!(!owner.combat_particles.get(owner_stale).unwrap().active);
    assert!(peer.combat_particles.get(peer_stale).unwrap().active);
    assert!(!peer.objects[&id].has_object_status_bit("AFLAME"));
    assert!(
        peer.objects[&id]
            .fire_spread
            .as_ref()
            .unwrap()
            .would_ignite()
    );
    assert!(owner.queued_audio_event_count_for_test() > 0);
    assert_eq!(peer.queued_audio_event_count_for_test(), 0);
}

#[test]
fn fire_spread_timer_extinguishes_particles_synchronously_for_owner_only() {
    let catalog = game_client::fx_list::FXListStore::new();
    let id = ObjectId(9413);
    let mut owner = world(id, Vec3::new(10., 0., 12.), &catalog);
    let mut peer = world(id, Vec3::new(40., 0., 12.), &catalog);
    let mut stale = Vec::new();
    for world in [&mut owner, &mut peer] {
        let mut fire = crate::game_logic::host_fire_spread::HostFireSpreadData::tree_default();
        assert!(fire.try_to_ignite(0));
        fire.aflame_end_frame = 1;
        fire.burned_end_frame = 0;
        fire.burning_sound_name.clear();
        fire.burning_sound_active = false;
        let object = world.objects.get_mut(&id).unwrap();
        object.fire_spread = Some(fire);
        object.apply_flammable_ignite_visuals(0);
        stale.push(world.combat_particles.spawn(
            crate::game_logic::CombatParticleKind::BodyFire,
            Vec3::ZERO,
            0,
            Some(id),
            None,
        ));
    }
    owner.frame = 1;

    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    owner.update_fire_spread_with_effects(&mut effects);

    assert!(!owner.objects[&id].has_object_status_bit("AFLAME"));
    assert!(matches!(
        owner.objects[&id].fire_spread.as_ref().unwrap().state,
        crate::game_logic::host_fire_spread::HostFlammableState::Normal
    ));
    assert!(!owner.combat_particles.get(stale[0]).unwrap().active);
    assert!(peer.combat_particles.get(stale[1]).unwrap().active);
}
