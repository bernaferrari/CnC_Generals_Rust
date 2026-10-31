//! Actual receiving Body operation -> synchronous typed native-view delivery.
use super::*;
use crate::game_logic::{NativeBodyVisualChange, NativeSlaveSource, NativeViewEffects, Player};
use crate::presentation_frame::camera::build::SlaveCameraSample;
use gamelogic::common::types::BodyDamageType;
#[derive(Default)]
struct View(
    Vec<(
        crate::assets::LiveDrawPlaybackIdentity,
        BodyDamageType,
        bool,
        game_client::system::TimeOfDay,
    )>,
);
impl NativeViewEffects for View {
    fn local_player_id(&self) -> u32 {
        0
    }
    fn sample_slave(&mut self, _: &NativeSlaveSource, _: &str) -> SlaveCameraSample {
        unreachable!("not a camera operation")
    }
    fn retire_physical_object_drawable(
        &mut self,
        _: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Err("this probe does not retire native Drawables".into())
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
        Err("factory is outside this callback probe".into())
    }
    fn body_visual_changed(
        &mut self,
        change: &NativeBodyVisualChange,
        _: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.0.push((
            change.identity.clone(),
            change.state,
            change.loading_map,
            change.time_of_day.expect("probe world has valid TOD"),
        ));
        Ok(true)
    }
}
fn world(tod: game_engine::common::ini::TimeOfDay) -> (GameLogic, ObjectId) {
    world_with_birth_mode(tod, GameMode::None)
}
fn world_with_birth_mode(
    tod: game_engine::common::ini::TimeOfDay,
    mode: GameMode,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.prepare_native_command_mode(mode);
    world.prepare_native_command_difficulty(0);
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "early-native-body.ini".into(),
        "Object ReceivingEarlyBody\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    if mode == GameMode::SinglePlayer {
        // Controller category is Human even when this client is not local.
        assert!(world.select_local_player(None));
    }
    world.set_receiving_time_of_day(tod);
    let id = world
        .create_object_for_player("ReceivingEarlyBody", 0, glam::Vec3::new(8.0, 3.0, 7.0))
        .unwrap();
    (world, id)
}
fn damage(world: &mut GameLogic, id: ObjectId, amount: f32, view: &mut View) {
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        view: Some(view),
        ..Default::default()
    };
    if amount < 0.0 {
        world
            .apply_owned_healing_with_effects(id, -amount, None, &mut effects)
            .unwrap();
    } else {
        world
            .apply_owned_damage_with_effects(
                id,
                amount,
                None,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
                &mut effects,
            )
            .unwrap();
    }
}
#[test]
fn actual_damage_and_healing_deliver_native_state_once_to_exact_receiving_world() {
    use game_client::system::TimeOfDay as Native;
    use game_engine::common::ini::TimeOfDay as Source;
    let (mut owner, id) = world(Source::Night);
    let (mut peer, peer_id) = world(Source::Morning);
    assert_eq!(id, peer_id);
    let mut owner_view = View::default();
    let mut peer_view = View::default();
    damage(&mut owner, id, 40.0, &mut owner_view);
    assert_eq!(owner_view.0.len(), 1);
    assert_eq!(
        (owner_view.0[0].1, owner_view.0[0].2, owner_view.0[0].3),
        (BodyDamageType::Damaged, false, Native::Night)
    );
    assert_eq!(owner_view.0[0].0.object_id(), id.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
    damage(&mut owner, id, 1.0, &mut owner_view);
    assert_eq!(owner_view.0.len(), 1, "same state has no native transition");
    damage(&mut owner, id, -41.0, &mut owner_view);
    assert_eq!(owner.host_object(id).unwrap().health.current, 100.0);
    assert_eq!(owner_view.0.len(), 2);
    assert_eq!(owner_view.0[1].1, BodyDamageType::Pristine);
    damage(&mut peer, peer_id, 70.0, &mut peer_view);
    assert_eq!(peer_view.0.len(), 1);
    assert_eq!(peer_view.0[0].1, BodyDamageType::ReallyDamaged);
    assert_eq!(peer_view.0[0].3, Native::Morning);
    assert_ne!(owner_view.0[0].0, peer_view.0[0].0);
}
#[test]
fn actual_construction_suppresses_native_transition_and_map_gate_is_receiving() {
    use game_engine::common::ini::TimeOfDay;
    let (mut owner, id) = world(TimeOfDay::Night);
    let mut view = View::default();
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(true);
    damage(&mut owner, id, 40.0, &mut view);
    assert!(view.0.is_empty());
    assert_eq!(owner.host_object(id).unwrap().health.current, 60.0);
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(false);
    owner.begin_map_admission_attempt().unwrap();
    damage(&mut owner, id, 30.0, &mut view);
    assert_eq!(view.0.len(), 1);
    assert!(view.0[0].2);
}

#[test]
fn max_health_modes_keep_cached_state_and_deliver_add_current_synchronously() {
    use crate::game_logic::object::MaxHealthChange;
    let (mut owner, id) = world(game_engine::common::ini::TimeOfDay::Night);
    let (peer, peer_id) = world(game_engine::common::ini::TimeOfDay::Morning);
    assert_eq!(id, peer_id);
    let mut view = View::default();
    damage(&mut owner, id, 40.0, &mut view);
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        view: Some(&mut view),
        ..Default::default()
    };
    owner
        .set_owned_body_max_health_with_effects(
            id,
            200.0,
            MaxHealthChange::KeepCurrent,
            &mut effects,
        )
        .unwrap();
    assert_eq!(owner.host_object(id).unwrap().health.current, 60.0);
    assert!(matches!(
        owner.host_object(id).unwrap().body_damage_state,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged
    ));
    owner
        .set_owned_body_max_health_with_effects(
            id,
            400.0,
            MaxHealthChange::AddToCurrent,
            &mut effects,
        )
        .unwrap();
    assert_eq!(owner.host_object(id).unwrap().health.current, 260.0);
    // 260/400 remains Damaged: no callback from either operation.
    owner
        .set_owned_body_max_health_with_effects(
            id,
            800.0,
            MaxHealthChange::AddToCurrent,
            &mut effects,
        )
        .unwrap();
    assert_eq!(owner.host_object(id).unwrap().health.current, 660.0);
    drop(effects);
    assert_eq!(view.0.len(), 2);
    assert_eq!(view.0[1].1, BodyDamageType::Pristine);
    assert_eq!(peer.host_object(peer_id).unwrap().health.maximum, 100.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
}

#[test]
fn actual_script_difficulty_reclassifies_kept_cache_once_on_receiving_nonlocal_human() {
    use crate::game_logic::object::MaxHealthChange;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
    // Admit mode and Easy before the real factory, just as NewGame does.
    // Existing compatibility policy is expected to resolve human Easy to1.5;
    // failure here identifies factor-policy drift before the native assertion.
    let (mut owner, id) = world_with_birth_mode(
        game_engine::common::ini::TimeOfDay::Night,
        GameMode::SinglePlayer,
    );
    let (peer, peer_id) = world(game_engine::common::ini::TimeOfDay::Morning);
    assert_eq!(id, peer_id);
    assert!(owner.players[&0].is_human);
    assert_eq!(owner.local_player_id(), None);
    let object = owner.host_object(id).unwrap();
    assert!(object.is_receiving_difficulty_bonus);
    assert_eq!(
        (object.health.current, object.health.maximum),
        (150.0, 150.0),
        "real factory must have applied the existing human Easy1.5 factor"
    );
    let mut view = View::default();
    let mut disable = ScriptAction::new(ScriptActionType::ObjectAllowBonuses);
    disable
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    owner
        .with_owned_script_engine(|engine, world| {
            let mut effects = crate::game_logic::ScriptPresentationEffects {
                view: Some(&mut view),
                ..Default::default()
            };
            world
                .execute_script_team_actions_with_effects_checked(
                    engine,
                    &disable,
                    None,
                    &mut effects,
                )
                .unwrap();
        })
        .unwrap();
    let object = owner.host_object(id).unwrap();
    assert!(!object.is_receiving_difficulty_bonus);
    assert_eq!(
        (object.health.current, object.health.maximum),
        (100.0, 100.0)
    );
    assert!(
        view.0.is_empty(),
        "ratio-preserving pristine strip has no native restart"
    );
    damage(&mut owner, id, 40.0, &mut view);
    assert_eq!(owner.host_object(id).unwrap().health.current, 60.0);
    owner
        .host_object_mut(id)
        .unwrap()
        .set_body_max_health_with_mode(200.0, MaxHealthChange::KeepCurrent);
    let health_events = owner.health_events.snapshot_ordered().len();
    let mut action = ScriptAction::new(ScriptActionType::ObjectAllowBonuses);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        view: Some(&mut view),
        ..Default::default()
    };
    owner
        .with_owned_script_engine(|engine, world| {
            world
                .execute_script_team_actions_with_effects_checked(
                    engine,
                    &action,
                    None,
                    &mut effects,
                )
                .unwrap();
        })
        .unwrap();
    let object = owner.host_object(id).unwrap();
    assert_eq!(
        (object.health.current, object.health.maximum),
        (90.0, 300.0)
    );
    assert!(object.is_receiving_difficulty_bonus);
    assert_eq!(
        owner.health_events.snapshot_ordered().len(),
        health_events,
        "setMaxHealth never emits attemptHealing/onHealing events"
    );
    owner
        .with_owned_script_engine(|engine, world| {
            world
                .execute_script_team_actions_with_effects_checked(
                    engine,
                    &action,
                    None,
                    &mut effects,
                )
                .unwrap();
        })
        .unwrap();
    drop(effects);
    assert_eq!(
        view.0.len(),
        2,
        "actual script callback is synchronous and repeat is idempotent"
    );
    assert_eq!(view.0[1].1, BodyDamageType::ReallyDamaged);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
}

#[test]
fn max_health_receiver_observes_committed_hp_before_effectively_dead() {
    use crate::game_logic::object::{BodyStateFx, MaxHealthChange};
    let (mut owner, id) = world(game_engine::common::ini::TimeOfDay::Night);
    let object = owner.host_object_mut(id).unwrap();
    let mut observed = Vec::new();
    object.set_body_max_health_with_mode_and_body_fx(
        0.0,
        MaxHealthChange::FullyHeal,
        &mut |object, effect, _| {
            if let BodyStateFx::NativeVisualState(_) = effect {
                observed.push((
                    object.health.current,
                    object.health.maximum,
                    object.status.effectively_dead,
                ));
            }
        },
    );
    assert_eq!(observed, vec![(0.0, 0.0, false)]);
    assert!(object.status.effectively_dead);
}

fn native_bridge_death_world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "native-bridge-death.ini".into(),
        "Object ReceivingDeathSpan\nKindOf = BRIDGE STRUCTURE\nBody = ActiveBody B\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = BridgeBehavior Bridge\nEnd\nEnd\nObject ReceivingDeathPhysics\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody B\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = PhysicsBehavior P\nMass = 1\nEnd\nEnd\nObject ReceivingDeathNoPhysics\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody B\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    world.add_player(Player::new(0, Team::USA, "BridgeOwner", true));
    world.set_receiving_time_of_day(game_engine::common::ini::TimeOfDay::Night);
    let span = world
        .create_object_for_player("ReceivingDeathSpan", 0, glam::Vec3::new(50., 5., 50.))
        .unwrap();
    let physics = world
        .create_object_for_player("ReceivingDeathPhysics", 0, glam::Vec3::new(49., 5., 50.))
        .unwrap();
    let no_physics = world
        .create_object_for_player("ReceivingDeathNoPhysics", 0, glam::Vec3::new(51., 5., 50.))
        .unwrap();
    let corners = [
        glam::Vec3::new(20., 5., 40.),
        glam::Vec3::new(20., 5., 60.),
        glam::Vec3::new(80., 5., 40.),
        glam::Vec3::new(80., 5., 60.),
    ];
    world
        .bridge_behavior
        .register_span(span, corners[0], corners[1], corners[2], corners[3]);
    let coord = |p: glam::Vec3| gamelogic::common::Coord3D::new(p.x, p.z, p.y);
    let mut info = gamelogic::terrain::BridgeInfo::new();
    info.bridge_object_id = span.0;
    info.from_left = coord(corners[0]);
    info.from_right = coord(corners[1]);
    info.to_left = coord(corners[2]);
    info.to_right = coord(corners[3]);
    let layer = world.reserve_owned_bridge(&info);
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .prepend_bridge_on_layer(
            info,
            "ReceivingDeathSpan".into(),
            gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
        );
    for id in [physics, no_physics] {
        let object = world.host_object_mut(id).unwrap();
        object.pathfind_layer = layer;
        object.ground_height = 5.;
    }
    world
        .host_object_mut(physics)
        .unwrap()
        .set_indestructible(true);
    world.tick_physics_collisions_all();
    for id in [physics, no_physics] {
        let position = world.host_object(id).unwrap().get_position();
        world.set_fixture_object_position(id, glam::Vec3::new(position.x, 5., position.z));
        world.host_object_mut(id).unwrap().ground_height = 5.;
    }
    world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    (world, span, physics, no_physics)
}

#[test]
fn actual_authored_bridge_death_retains_physics_survivor_and_native_receiving_identity() {
    let (mut owner, span, physics, no_physics) = native_bridge_death_world();
    let (mut peer, peer_span, peer_physics, peer_no_physics) = native_bridge_death_world();
    assert_eq!(
        (span, physics, no_physics),
        (peer_span, peer_physics, peer_no_physics)
    );
    let mut recipients = Vec::new();
    owner.visit_owned_bridge_on_die_recipients(span, |_, id, physics| {
        recipients.push((id, physics));
    });
    let observe = |id| {
        let object = owner.host_object(id).unwrap();
        (
            id,
            object.get_position(),
            object.ground_height,
            object.pathfind_layer,
            object.is_above_terrain(),
            object.is_mobile(),
        )
    };
    assert!(
        recipients.contains(&(physics, true)) && recipients.contains(&(no_physics, false)),
        "actual original onDie visitor must admit fixture occupants before damage: recipients={recipients:?}, span={:?}, physics={:?}, no_physics={:?}",
        observe(span),
        observe(physics),
        observe(no_physics),
    );
    assert_eq!(owner.host_object(span).unwrap().health.current, 200.);
    assert_eq!(owner.host_object(physics).unwrap().health.current, 100.);
    assert_eq!(owner.host_object(no_physics).unwrap().health.current, 100.);
    let mut view = View::default();
    damage(&mut owner, span, 201., &mut view);
    assert!(owner.host_object(span).unwrap().status.on_die_started);
    assert_eq!(
        owner.host_object(physics).unwrap().pathfind_layer,
        gamelogic::path::PathfindLayerEnum::Ground as u8
    );
    assert!(owner.host_object(physics).unwrap().allow_to_fall);
    assert_eq!(owner.host_object(physics).unwrap().health.current, 100.);
    assert!(owner.host_object(no_physics).unwrap().status.on_die_started);
    assert!(
        view.0
            .iter()
            .any(|entry| entry.0.object_id() == span.0 && entry.1 == BodyDamageType::Rubble)
    );
    assert!(
        view.0
            .iter()
            .any(|entry| entry.0.object_id() == no_physics.0 && entry.1 == BodyDamageType::Rubble)
    );
    assert!(!view.0.iter().any(|entry| entry.0.object_id() == physics.0));
    assert_ne!(
        peer.host_object(peer_physics).unwrap().pathfind_layer,
        gamelogic::path::PathfindLayerEnum::Ground as u8
    );
    assert!(!peer.host_object(peer_physics).unwrap().allow_to_fall);
    assert_eq!(
        peer.host_object(peer_no_physics).unwrap().health.current,
        100.
    );
    peer.reset();
    drop(peer);
    assert!(owner.host_object(physics).unwrap().allow_to_fall);
    assert_eq!(owner.host_object(physics).unwrap().health.current, 100.);
}
