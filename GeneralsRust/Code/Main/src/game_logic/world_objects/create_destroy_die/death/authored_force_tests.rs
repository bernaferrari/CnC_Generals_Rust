//! Source-backed OCL execution at the installed object's authored death callback.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

fn force_world(magnitude: u32, physics: bool, death: &str) -> GameLogic {
    force_world_with_kinds("VEHICLE", magnitude, physics, death)
}

fn force_world_with_kinds(kinds: &str, magnitude: u32, physics: bool, death: &str) -> GameLogic {
    force_world_with_objects(kinds, magnitude, physics, death, "")
}

fn force_world_with_objects(
    kinds: &str,
    magnitude: u32,
    physics: bool,
    death: &str,
    extra_objects: &str,
) -> GameLogic {
    let physics = if physics {
        "Behavior = PhysicsBehavior PhysicsTag\nMass = 5\nEnd\n"
    } else {
        ""
    };
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "ocl-random-force.ini".to_owned(),
        format!(
            "Object AuthoredWreck\nKindOf = {kinds}\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{physics}{death}\nEnd\n{extra_objects}"
        ),
    )]).unwrap();
    let mut world = GameLogic::new();
    world
        .admit_ocl_application_sources(&[format!(
            "ObjectCreationList WreckToss\nApplyRandomForce\nMinForceMagnitude = {magnitude}\nMaxForceMagnitude = {magnitude}\nMinForcePitch = 90\nMaxForcePitch = 90\nSpinRate = 90\nEnd\nEnd"
        )])
        .unwrap();
    world.admit_receiving_module_content(content).unwrap();
    world
}

fn kill(world: &mut GameLogic, id: ObjectId) {
    world
        .apply_owned_damage(
            id,
            1_000.0,
            None,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}

#[test]
fn authored_object_death_consumes_force_draws_on_the_receiving_world() {
    let death = "Behavior = CreateObjectDie DeathTag\nCreationList = WreckToss\nEnd";
    let mut first = force_world(50, true, death);
    let mut peer = force_world(100, true, death);
    let mut no_physics = force_world(150, false, death);
    let spawn = |world: &mut GameLogic| {
        world
            .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
            .unwrap()
    };
    let id = spawn(&mut first);
    assert_eq!(id, spawn(&mut peer));
    assert_eq!(id, spawn(&mut no_physics));

    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(first.logic_random.seed_words());
    let original_peer_seed = peer.logic_random.seed_words();
    let original_pose = first.objects[&id].get_transform_matrix();
    let original_velocity = first.objects[&id].movement.velocity;
    expected.next_real(0.0, std::f32::consts::TAU);
    expected.next_real(std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    expected.next_real(50.0, 50.0);
    let spin = 90.0_f32.to_radians() * (1.0 / 30.0);
    let rates = std::array::from_fn::<_, 3, _>(|_| expected.next_real(-spin, spin));

    peer.with_own_services(|| kill(&mut first, id));
    let object = &first.objects[&id];
    assert!((object.physics_accel.y - 50.0 / object.physics_get_mass()).abs() < 1e-5);
    assert_eq!(object.get_transform_matrix(), original_pose);
    assert_eq!(object.movement.velocity, original_velocity);
    assert!((object.shock_yaw_rate - rates[0]).abs() < 1e-7);
    assert!((object.shock_roll_rate - rates[1]).abs() < 1e-7);
    assert!((object.shock_pitch_rate - rates[2]).abs() < 1e-7);
    assert_eq!(first.logic_random.seed_words(), expected.seed_words());
    assert_eq!(peer.logic_random.seed_words(), original_peer_seed);
    assert_eq!(peer.objects[&id].physics_accel, Vec3::ZERO);

    kill(&mut peer, id);
    let peer_object = &peer.objects[&id];
    assert!((peer_object.physics_accel.y - 100.0 / peer_object.physics_get_mass()).abs() < 1e-5);
    let seed = no_physics.logic_random.seed_words();
    kill(&mut no_physics, id);
    assert_eq!(no_physics.objects[&id].physics_accel, Vec3::ZERO);
    assert_eq!(no_physics.logic_random.seed_words(), seed);
}

#[test]
fn ordinary_slow_death_runs_initial_force_once_at_admission() {
    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 500\nOCL = INITIAL WreckToss\nEnd";
    let mut world = force_world(75, true, death);
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    let seed = world.logic_random.seed_words();
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(seed);
    expected.next_int(0, 9); // One applicable SlowDeath with the default weight.
    expected.next_int(0, 0); // Sink delay variance.
    expected.next_int(0, 0); // Destruction delay variance.
    expected.next_int(5, 9); // C++ midpoint bounds for a 15-frame lifetime.
    expected.next_int(0, 0); // One authored INITIAL entry.
    expected.next_real(0.0, std::f32::consts::TAU);
    let spin = 90.0_f32.to_radians() * (1.0 / 30.0);
    for _ in 0..3 {
        expected.next_real(-spin, spin);
    }

    kill(&mut world, id);
    let wreck = &world.objects[&id];
    assert!(
        wreck
            .slow_death
            .as_ref()
            .is_some_and(|death| death.is_active())
    );
    let accel = wreck.physics_accel;
    assert!((accel.y - 75.0 / wreck.physics_get_mass()).abs() < 1e-5);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    kill(&mut world, id);
    assert_eq!(world.objects[&id].physics_accel, accel);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_slow_death_selection_uses_clipped_overkill_and_fractional_truncation() {
    let death = r"Behavior = AIUpdateInterface Brain
End
Behavior = SlowDeathBehavior Regular
DestructionDelay = 500
End
Behavior = SlowDeathBehavior Overkill
DestructionDelay = 1000
ModifierBonusPerOverkillPercent = 100000%
End";
    // A first raw draw of 100 gives roll 1 for weights (10, 10) and
    // roll 21 for weights (10, 30). The fractional overkill truncates to zero.
    let seed = [100, 0, 0, 0, 0, 0];
    for line in include_str!("slow_death_selection.trace").lines() {
        let mut fields = line.split_ascii_whitespace();
        let damage = fields.next().unwrap().parse().unwrap();
        let total = fields.next().unwrap().parse().unwrap();
        let delay = match fields.next().unwrap() {
            "Regular" => 15,
            "Overkill" => 30,
            tag => panic!("unexpected original SlowDeath slot {tag}"),
        };
        let mut world = force_world(50, false, death);
        world.frame = 41;
        let id = world
            .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
            .unwrap();
        assert!(world.objects[&id].has_ai_update_interface());
        world.logic_random.set_seed_words(seed);
        world
            .apply_owned_damage(
                id,
                damage,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
            .unwrap();
        let mut expected = game_engine::common::random_value::RandomState::default();
        expected.set_seed_words(seed);
        expected.next_int(1, total);
        expected.next_int(0, 0);
        expected.next_int(0, 0);
        let midpoint = expected.next_int(delay * 35 / 100, delay * 65 / 100) as u32;
        let wreck = &world.objects[&id];
        assert_eq!(
            wreck.slow_death.as_ref().unwrap().destroy_at_frame,
            41 + delay as u32
        );
        assert_eq!(
            wreck.native_die_runtime.slow_death.unwrap().midpoint_frame,
            41 + midpoint
        );
        assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    }
}

#[test]
fn authored_slow_death_hulk_override_uses_fixed_offsets_and_the_receiving_world() {
    let death = r"Behavior = SlowDeathBehavior DeathTag
SinkDelay = 1000
DestructionDelay = 5000
End";
    let mut scripted = force_world_with_kinds("VEHICLE HULK", 50, false, death);
    let mut ordinary = force_world_with_kinds("VEHICLE HULK", 50, false, death);
    scripted.frame = 41;
    ordinary.frame = 41;
    let spawn = |world: &mut GameLogic| {
        world
            .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
            .unwrap()
    };
    let scripted_id = spawn(&mut scripted);
    let ordinary_id = spawn(&mut ordinary);
    assert_eq!(scripted_id, ordinary_id);
    assert!(scripted.objects[&scripted_id].is_kind_of(KindOf::Hulk));
    scripted.native_hulk_max_lifetime_override = 9;
    let seed = [100, 0, 0, 0, 0, 0];
    scripted.logic_random.set_seed_words(seed);
    ordinary.logic_random.set_seed_words(seed);
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(seed);
    expected.next_int(1, 10);
    ordinary.with_own_services(|| kill(&mut scripted, scripted_id));
    let wreck = &scripted.objects[&scripted_id];
    let timer = wreck.slow_death.as_ref().unwrap();
    assert_eq!((timer.sink_at_frame, timer.destroy_at_frame), (42, 72));
    assert_eq!(
        wreck.native_die_runtime.slow_death.unwrap().midpoint_frame,
        57
    );
    assert_eq!(scripted.logic_random.seed_words(), expected.seed_words());
    assert_eq!(ordinary.logic_random.seed_words(), seed);

    scripted.with_own_services(|| kill(&mut ordinary, ordinary_id));
    let timer = ordinary.objects[&ordinary_id].slow_death.as_ref().unwrap();
    assert_eq!((timer.sink_at_frame, timer.destroy_at_frame), (71, 191));
}

#[test]
fn authored_slow_death_fling_applies_receiving_force_in_cpp_draw_order() {
    use crate::game_logic::host_enum_table_residual::exploded_flailing_model_bit;

    let death = r"Behavior = SlowDeathBehavior DeathTag
DestructionDelay = 1000
FlingForce = 60
FlingForceVariance = 20
FlingPitch = 15
FlingPitchVariance = 20
End";
    let mut owner = force_world(50, true, death);
    let mut peer = force_world(50, false, death);
    let spawn = |world: &mut GameLogic| {
        world
            .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
            .unwrap()
    };
    let id = spawn(&mut owner);
    assert_eq!(id, spawn(&mut peer));
    let seed = game_engine::common::random_value::RandomState::default().seed_words();
    owner.logic_random.set_seed_words(seed);
    peer.logic_random.set_seed_words(seed);
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(seed);
    expected.next_int(1, 10);
    expected.next_int(0, 0);
    expected.next_int(0, 0);
    expected.next_int(10, 19);
    let no_physics_seed = expected.seed_words();
    let angle = expected.next_real(-std::f32::consts::PI, std::f32::consts::PI);
    let pitch = expected.next_real(15.0_f32.to_radians(), 35.0_f32.to_radians());
    let magnitude = expected.next_real(60.0, 80.0);
    let horizontal = magnitude * pitch.cos();
    let force = Vec3::new(
        horizontal * angle.cos(),
        magnitude * pitch.sin(),
        horizontal * angle.sin(),
    );
    let initial_velocity = owner.objects[&id].movement.velocity;

    peer.with_own_services(|| kill(&mut owner, id));
    let wreck = &owner.objects[&id];
    assert!(
        wreck
            .physics_accel
            .abs_diff_eq(force / wreck.physics_get_mass(), 1e-5)
    );
    assert_eq!(wreck.movement.velocity, initial_velocity);
    assert_eq!(wreck.get_position().y, 1.0);
    assert!((wreck.get_orientation() - force.z.atan2(force.x)).abs() < 1e-6);
    assert_eq!(wreck.extra_bounciness, -1.0);
    assert!(wreck.allow_to_fall && wreck.shock_allow_bounce);
    assert_ne!(
        wreck.model_condition_bits & (1u128 << exploded_flailing_model_bit()),
        0
    );
    assert_eq!(owner.logic_random.seed_words(), expected.seed_words());
    assert_eq!(peer.logic_random.seed_words(), seed);

    kill(&mut peer, id);
    let wreck = &peer.objects[&id];
    assert_eq!(wreck.get_position(), Vec3::ZERO);
    assert_eq!(wreck.physics_accel, Vec3::ZERO);
    assert_eq!(wreck.movement.velocity, initial_velocity);
    assert_eq!(peer.logic_random.seed_words(), no_physics_seed);
}

#[test]
fn authored_slow_death_sink_updates_the_world_pose_on_the_deadline() {
    let death = r"Behavior = SlowDeathBehavior DeathTag
SinkDelay = 100
SinkRate = 30
DestructionDelay = 1000
DestructionAltitude = -0.1
End";
    let mut world = force_world(50, true, death);
    world.frame = 41;
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut world, id);
    world.frame = 43;
    world
        .tick_authored_ordinary_slow_death(id, &mut Default::default())
        .unwrap();
    assert_eq!(world.objects[&id].get_position(), Vec3::ZERO);
    world.frame = 44;
    world
        .tick_authored_ordinary_slow_death(id, &mut Default::default())
        .unwrap();
    let wreck = &world.objects[&id];
    assert_eq!(wreck.get_position(), -Vec3::Y);
    assert!(wreck.status.disabled_held);
    assert_eq!(wreck.slow_death.as_ref().unwrap().sink_offset, 0.0);
}

const SINKING_VISUAL_DEATH: &str = r"Shadow = SHADOW_DECAL
ShadowTexture = ShadowInfantry
Draw = W3DModelDraw UnitDraw
 DefaultConditionState
  Model = TestInfantry
 End
End
Behavior = SlowDeathBehavior DeathTag
 SinkDelay = 3000
 SinkRate = 1
 DestructionDelay = 5000
End";

fn sinking_decal_world(kinds: &str, definition: &str) -> (GameLogic, ObjectId) {
    use crate::game_logic::host_battlemaster::TERRAIN_DECAL_CRATE;

    let mut world = force_world_with_kinds(kinds, 50, false, definition);
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    let object = world.objects.get_mut(&id).unwrap();
    object.set_terrain_decal(TERRAIN_DECAL_CRATE);
    object.set_terrain_decal_size(4.0, 4.0);
    object.set_terrain_decal_fade_target(1.0, 0.25);
    for _ in 0..4 {
        world.update();
    }
    assert_eq!(world.objects[&id].terrain_decal_opacity, 1.0);
    (world, id)
}

fn observe_sinking_visuals(world: &mut GameLogic, id: ObjectId) -> String {
    use crate::presentation_frame::{PresentationFrame, live_terrain_decal_specs};

    let frame = PresentationFrame::build_from_logic(world, 0);
    let object = frame.objects.iter().find(|object| object.id == id).unwrap();
    let rendered = live_terrain_decal_specs(&frame.objects, frame.local_team)
        .into_iter()
        .find(|item| item.object_id == id)
        .map_or(0.0, |item| item.opacity);
    assert_eq!(object.terrain_decal_opacity, rendered);
    let state = &world.objects[&id];
    format!(
        "{} {:.3} {:.3} {:.3}",
        u8::from(object.shadows_enabled),
        rendered,
        state.terrain_decal_opacity,
        state.terrain_decal_fade_rate,
    )
}

#[test]
fn authored_slow_death_sinking_infantry_uses_original_shadow_and_decal_transition() {
    use crate::save_load::snapshot::SnapshotBuilder;

    let (mut owner, id) = sinking_decal_world("INFANTRY", SINKING_VISUAL_DEATH);
    let (mut peer, peer_id) = sinking_decal_world("VEHICLE", SINKING_VISUAL_DEATH);
    assert_eq!(id, peer_id);
    let mut trace = include_str!("slow_death_visuals.trace").lines();
    let before = trace.next().unwrap();
    assert_eq!(observe_sinking_visuals(&mut owner, id), before);
    peer.with_own_services(|| kill(&mut owner, id));
    assert_eq!(
        observe_sinking_visuals(&mut owner, id),
        trace.next().unwrap()
    );
    assert_eq!(observe_sinking_visuals(&mut peer, id), before);
    owner.update();
    assert_eq!(
        observe_sinking_visuals(&mut owner, id),
        trace.next().unwrap()
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut restored = force_world_with_kinds("INFANTRY", 50, false, SINKING_VISUAL_DEATH);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(!restored.objects[&id].presentation_shadows_enabled());
    for expected in trace.by_ref().take(5) {
        owner.update();
        restored.update();
        assert_eq!(observe_sinking_visuals(&mut owner, id), expected);
        assert_eq!(observe_sinking_visuals(&mut restored, id), expected);
    }

    kill(&mut peer, id);
    assert_eq!(
        observe_sinking_visuals(&mut peer, id),
        trace.next().unwrap()
    );
}

#[test]
fn authored_slow_death_leaves_same_target_decal_fade_rate_intact() {
    let mut trace = include_str!("slow_death_visuals.trace").lines().rev();
    let (mut world, id) = sinking_decal_world("INFANTRY", SINKING_VISUAL_DEATH);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .set_terrain_decal_fade_target(0.0, -0.125);
    kill(&mut world, id);
    assert_eq!(
        observe_sinking_visuals(&mut world, id),
        trace.next().unwrap()
    );

    let death = SINKING_VISUAL_DEATH.replace("SinkRate = 1", "SinkRate = 0");
    let (mut zero_sink, id) = sinking_decal_world("INFANTRY", &death);
    kill(&mut zero_sink, id);
    assert_eq!(
        observe_sinking_visuals(&mut zero_sink, id),
        trace.next().unwrap()
    );
}

#[test]
fn authored_slow_death_flight_continues_through_save_and_first_landing() {
    use crate::game_logic::native_die_definition::SlowDeathFlight;
    use crate::save_load::snapshot::SnapshotBuilder;

    let death = r"Behavior = SlowDeathBehavior DeathTag
SinkDelay = 500
DestructionDelay = 1000
FlingForce = 8
FlingPitch = 30
End";
    let mut source = force_world(50, true, death);
    let mut restored = force_world(50, true, death);
    source.frame = 41;
    let id = source
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut source, id);
    let read = |world: &GameLogic| {
        let wreck = &world.objects[&id];
        let timer = wreck.slow_death.as_ref().unwrap();
        let receipt = wreck.native_die_runtime.slow_death.unwrap();
        (
            timer.sink_at_frame,
            receipt.midpoint_frame,
            timer.destroy_at_frame,
            receipt.flight,
        )
    };
    let (sink, midpoint, destruction, flight) = read(&source);
    assert_eq!(flight, SlowDeathFlight::Flailing);
    let check_trace = |world: &GameLogic, expected: &str| {
        let (current_sink, current_midpoint, current_destruction, flight) = read(world);
        let model = match flight {
            SlowDeathFlight::Flailing => "flailing",
            SlowDeathFlight::Bouncing => "bouncing",
            phase => panic!("unexpected flight phase {phase:?}"),
        };
        assert_eq!(
            format!(
                "{} {} {} {model}",
                current_sink - sink,
                current_midpoint - midpoint,
                current_destruction - destruction
            ),
            expected,
        );
    };
    let mut trace = include_str!("slow_death_flight.trace").lines();
    let flying = trace.next().unwrap();
    let landing = trace.next().unwrap();
    let settled = trace.next().unwrap();
    source.frame = 42;
    source
        .tick_authored_ordinary_slow_death(id, &mut Default::default())
        .unwrap();
    check_trace(&source, flying);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(read(&source), read(&restored));
    assert_eq!(
        source.objects[&id].physics_accel,
        restored.objects[&id].physics_accel
    );
    assert_eq!(
        source.objects[&id].get_transform_matrix(),
        restored.objects[&id].get_transform_matrix()
    );
    let check_model = |world: &GameLogic, active: u32, inactive: u32| {
        let bits = world.objects[&id].model_condition_bits;
        assert_ne!(bits & (1u128 << active), 0);
        assert_eq!(bits & (1u128 << inactive), 0);
    };
    use crate::game_logic::host_enum_table_residual::{
        exploded_bouncing_model_bit, exploded_flailing_model_bit,
    };
    check_model(
        &restored,
        exploded_flailing_model_bit(),
        exploded_bouncing_model_bit(),
    );
    for world in [&mut restored, &mut source] {
        world.frame = 43;
        assert!(world.set_owned_object_position_with_effects(
            id,
            Vec3::ZERO,
            world.frame,
            &mut Default::default(),
        ));
        world
            .tick_authored_ordinary_slow_death(id, &mut Default::default())
            .unwrap();
        check_trace(world, landing);
        check_model(
            world,
            exploded_bouncing_model_bit(),
            exploded_flailing_model_bit(),
        );
        world.frame = 44;
        world
            .tick_authored_ordinary_slow_death(id, &mut Default::default())
            .unwrap();
        check_trace(world, settled);
    }
    assert_eq!(
        source.logic_random.seed_words(),
        restored.logic_random.seed_words()
    );
}

#[test]
fn authored_slow_death_shrub_contact_on_landing_runs_before_retirement() {
    use crate::game_logic::native_die_definition::SlowDeathFlight;
    use game_engine::common::bit_flags::ModelConditionFlags as M;

    let death = r"Behavior = SlowDeathBehavior DeathTag
SinkDelay = 500
SinkRate = 30
DestructionDelay = 1000
FlingForce = 8
FlingPitch = 30
End";
    let shrub = r"Object FlingFixtureShrub
KindOf = SHRUBBERY IMMOBILE
Body = ActiveBody TreeBody
MaxHealth = 100
InitialHealth = 100
End
End";
    let mut world = force_world_with_objects("VEHICLE", 50, true, death, shrub);
    let tree = world
        .create_object("FlingFixtureShrub", Team::Neutral, Vec3::Z)
        .unwrap();
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut world, id);
    world.frame += 1;
    assert!(world.set_owned_object_position_with_effects(
        id,
        Vec3::ZERO,
        world.frame,
        &mut Default::default(),
    ));
    world.objects.get_mut(&id).unwrap().last_collidee = Some(tree);
    assert!(
        world
            .tick_authored_ordinary_slow_death(id, &mut Default::default())
            .unwrap()
    );
    let wreck = &world.objects[&id];
    assert_eq!(
        wreck.native_die_runtime.slow_death.unwrap().flight,
        SlowDeathFlight::LandedInTree
    );
    assert_eq!(
        wreck.model_condition_bits
            & ((1u128 << M::EXPLODED_FLAILING) | (1u128 << M::EXPLODED_BOUNCING)),
        0,
    );
    assert_ne!(wreck.model_condition_bits & (1u128 << M::PARACHUTING), 0);
    let retired = world
        .pending_object_removal_snapshot()
        .iter()
        .any(|event| event.id == id);
    assert_eq!(
        format!(
            "{:.0} parachuting {} {}",
            wreck.get_position().y,
            u8::from(wreck.status.disabled_held),
            u8::from(retired)
        ),
        include_str!("slow_death_flight.trace")
            .lines()
            .nth(3)
            .unwrap(),
    );
}

#[test]
fn location_ocl_with_owner_id_never_draws_object_force() {
    let mut world = force_world(75, true, "");
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    let seed = world.logic_random.seed_words();
    world
        .execute_parsed_weapon_ocl_at(
            "WreckToss",
            Some(id),
            Team::GLA,
            crate::game_logic::VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::X,
        )
        .unwrap();
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
    assert_eq!(world.logic_random.seed_words(), seed);
}

#[test]
fn legacy_air_death_name_does_not_supply_an_unauthored_force() {
    let mut world = GameLogic::new();
    let name = "GLAVehicleTechnicalAirDeath";
    let mut template = crate::game_logic::ThingTemplate::new(name);
    template
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(100.0);
    world.templates.insert(name.into(), template);
    let id = world.create_object(name, Team::GLA, Vec3::Y).unwrap();
    let velocity = world.objects[&id].movement.velocity;
    kill(&mut world, id);
    assert_eq!(world.objects[&id].movement.velocity, velocity);
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
}

#[test]
fn same_frame_slow_death_runs_midpoint_then_final_before_retirement() {
    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 0\nOCL = MIDPOINT WreckToss\nOCL = FINAL FinalToss\nEnd";
    let mut world = force_world(50, true, death);
    world.apply_receiving_ocl_overrides(&[
        "ObjectCreationList FinalToss\nApplyRandomForce\nMinForceMagnitude = 50\nMaxForceMagnitude = 50\nMinForcePitch = 90\nMaxForcePitch = 90\nSpinRate = 0\nEnd\nEnd".into(),
    ]).unwrap();
    world.frame = 41;
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut world, id);
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(world.logic_random.seed_words());
    let mut expected_force = 0.0;
    let mut expected_spin = [0.0; 3];
    let mut expected_retirement = false;
    for action in include_str!("slow_death_same_frame.trace").lines() {
        match action {
            "draw" => {
                expected.next_int(0, 0);
            }
            "ocl WreckToss" | "ocl FinalToss" => {
                expected_force += 50.0;
                expected.next_real(0.0, std::f32::consts::TAU);
                let spin = if action == "ocl WreckToss" {
                    90.0_f32.to_radians() * (1.0 / 30.0)
                } else {
                    0.0
                };
                expected_spin = std::array::from_fn(|_| expected.next_real(-spin, spin));
            }
            "retire" => expected_retirement = true,
            _ => panic!("unexpected original SlowDeath action: {action}"),
        }
    }
    world.update_ai(&[id], 1.0 / 30.0).unwrap();
    let wreck = &world.objects[&id];
    assert!((wreck.physics_accel.y - expected_force / wreck.physics_get_mass()).abs() < 1e-5);
    assert_eq!(
        [
            wreck.shock_yaw_rate,
            wreck.shock_roll_rate,
            wreck.shock_pitch_rate
        ],
        expected_spin
    );
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    assert_eq!(
        world
            .pending_object_removal_snapshot()
            .iter()
            .any(|record| record.id == id),
        expected_retirement,
    );
}

#[test]
fn saved_slow_death_resumes_selected_clock_without_replaying_midpoint() {
    use crate::save_load::snapshot::SnapshotBuilder;

    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 1000\nOCL = MIDPOINT WreckToss\nOCL = FINAL WreckToss\nEnd";
    let mut source = force_world(50, true, death);
    source.frame = 41;
    let id = source
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut source, id);
    let receipt = source.objects[&id].native_die_runtime.slow_death.unwrap();
    assert!((51..=60).contains(&receipt.midpoint_frame));
    source.frame = receipt.midpoint_frame;
    source.update_ai(&[id], 1.0 / 30.0).unwrap();
    let timer = source.objects[&id].slow_death.as_ref().unwrap();
    assert!(timer.midpoint_played);
    let final_frame = timer.destroy_at_frame;
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = force_world(50, true, death);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.objects[&id].native_die_runtime.slow_death,
        Some(receipt)
    );
    let post_midpoint = restored.logic_random.seed_words();
    let saved_accel = restored.objects[&id].physics_accel;
    restored.update_ai(&[id], 1.0 / 30.0).unwrap();
    assert_eq!(restored.logic_random.seed_words(), post_midpoint);
    assert_eq!(restored.objects[&id].physics_accel, saved_accel);
    for world in [&mut restored, &mut source] {
        world.frame = final_frame;
        world.update_ai(&[id], 1.0 / 30.0).unwrap();
        assert!(
            world
                .pending_object_removal_snapshot()
                .iter()
                .any(|record| record.id == id)
        );
    }
    assert_eq!(
        restored.logic_random.seed_words(),
        source.logic_random.seed_words()
    );
}
