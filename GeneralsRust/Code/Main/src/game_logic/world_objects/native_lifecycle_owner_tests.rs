//! Actual authored updates on the receiving Object, without ambient registries.
use super::super::*;
use crate::game_logic::native_lifecycle::NativeLifecycleState;

fn authored(name: &str, modules: &str, frame: u32, position: Vec3) -> (GameLogic, ObjectId) {
    authored_with_extra(name, modules, frame, position, "")
}

fn authored_with_extra(
    name: &str,
    modules: &str,
    frame: u32,
    position: Vec3,
    extra: &str,
) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object {name}\nKindOf = IMMOBILE INERT NO_COLLIDE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}\nEnd\n{extra}"
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "native-lifecycle-owner.ini".to_string(),
        source,
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.frame = frame;
    world.admit_receiving_module_content(content).unwrap();
    let id = world.create_object(name, Team::USA, position).unwrap();
    (world, id)
}

fn drive(world: &mut GameLogic, id: ObjectId, frame: u32) {
    world.frame = frame;
    world.tick_native_lifecycle(id);
}

#[test]
fn neutral_authored_timers_start_at_construction_and_round_duration_up() {
    for (module, deletion) in [("LifetimeUpdate", false), ("DeletionUpdate", true)] {
        let (mut world, id) = authored(
            "NeutralTimedOwner",
            &format!("Behavior = {module} ClockTag\nMinLifetime = 34\nMaxLifetime = 34\nEnd"),
            30,
            Vec3::ZERO,
        );
        let runtime = world
            .host_object(id)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap();
        assert_eq!(runtime.states.len(), 1);
        match &runtime.states[0] {
            NativeLifecycleState::Lifetime { die_frame, active } => {
                assert!(!deletion);
                assert_eq!(*die_frame, 32);
                assert!(*active);
            }
            NativeLifecycleState::Deletion { die_frame, active } => {
                assert!(deletion);
                assert_eq!(*die_frame, 32);
                assert!(*active);
            }
            _ => panic!("receiving timer has the authored update kind"),
        }
        drive(&mut world, id, 31);
        assert!(!world.host_object(id).unwrap().status.destroyed);
        drive(&mut world, id, 32);
        let object = world.host_object(id).unwrap();
        // LifetimeUpdate.cpp69-73 kills, not deletes; Object::onDie only
        // visits actual Die modules. This authored fixture has none.
        if deletion {
            assert!(object.status.destroyed);
        } else {
            assert!(object.status.effectively_dead);
        }
        assert_eq!(object.status.on_die_started, !deletion);
        assert_eq!(object.health.current, if deletion { 100.0 } else { 0.0 });
        assert_eq!(
            world
                .objects_to_destroy
                .iter()
                .filter(|event| event.id == id)
                .count(),
            usize::from(deletion)
        );
    }
}

#[test]
fn rejected_lifetime_is_consumed_while_deletion_ignores_body_indestructibility() {
    for (module, deletion) in [("LifetimeUpdate", false), ("DeletionUpdate", true)] {
        let (mut world, id) = authored(
            "NeutralInvulnerableTimer",
            &format!("Behavior = {module} TimerTag\nMinLifetime = 0\nMaxLifetime = 0\nEnd"),
            30,
            Vec3::ZERO,
        );
        world.host_object_mut(id).unwrap().indestructible = true;
        drive(&mut world, id, 31);
        let object = world.host_object(id).unwrap();
        assert_eq!(object.status.destroyed, deletion);
        assert!(!object.status.on_die_started);
        assert_eq!(object.health.current, 100.0);
        match &object.native_lifecycle.as_ref().unwrap().states[0] {
            NativeLifecycleState::Lifetime { active, .. }
            | NativeLifecycleState::Deletion { active, .. } => assert!(!active),
            _ => panic!("timer kind retained"),
        }
        if !deletion {
            world.host_object_mut(id).unwrap().indestructible = false;
            drive(&mut world, id, 99);
            assert!(!world.host_object(id).unwrap().status.on_die_started);
            assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
        }
    }
}

#[test]
fn two_authored_timer_modules_keep_independent_deadlines_and_source_order() {
    let (mut world, id) = authored(
        "NeutralTwoTimers",
        "Behavior = LifetimeUpdate FirstTag\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = DeletionUpdate SecondTag\nMinLifetime = 100\nMaxLifetime = 100\nEnd",
        30,
        Vec3::ZERO,
    );
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert_eq!(states.len(), 2);
    assert!(matches!(
        states[0],
        NativeLifecycleState::Lifetime {
            die_frame: 32,
            active: true
        }
    ));
    assert!(matches!(
        states[1],
        NativeLifecycleState::Deletion {
            die_frame: 33,
            active: true
        }
    ));
    world.host_object_mut(id).unwrap().indestructible = true;
    drive(&mut world, id, 32);
    assert!(!world.host_object(id).unwrap().status.destroyed);
    drive(&mut world, id, 33);
    let object = world.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert!(!object.status.on_die_started);
    assert_eq!(object.health.current, 100.0);
}

#[test]
fn bound_empty_native_definition_does_not_install_name_peel_timers() {
    let (mut world, id) = authored("PoisonFieldMedium", "", 30, Vec3::ZERO);
    let object = world.host_object(id).unwrap();
    assert!(object.native_lifecycle.as_ref().unwrap().states.is_empty());
    drive(&mut world, id, 100_000);
    world.update_ai(&[id], 1.0 / 30.0);
    let object = world.host_object(id).unwrap();
    assert!(object.lifetime_update.is_none());
    assert!(object.height_die.is_none());
    assert!(!object.status.on_die_started && !object.status.destroyed);
}

#[test]
fn height_threshold_is_strict_and_descent_observes_absolute_position() {
    let (mut world, id) = authored(
        "NeutralHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nOnlyWhenMovingDown = Yes\nEnd",
        30,
        Vec3::new(0.0, 10.0, 0.0),
    );
    drive(&mut world, id, 30);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "initial sentinel is minus one, not positive infinity"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 11.0, 0.0));
    drive(&mut world, id, 31);
    world.set_fixture_object_position(id, Vec3::new(0.0, 10.0, 0.0));
    drive(&mut world, id, 32);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "equal target does not die"
    );
    world.host_object_mut(id).unwrap().ground_height = 9.0;
    drive(&mut world, id, 33);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "rising terrain is not downward movement"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 9.0, 0.0));
    drive(&mut world, id, 34);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_delay_begins_at_first_update_and_rejected_kill_does_not_rearm() {
    let (mut world, id) = authored(
        "NeutralDelayedHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nInitialDelay = 34\nSnapToGroundOnDeath = Yes\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().indestructible = true;
    drive(&mut world, id, 40);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie {
            earliest_frame,
            last_position,
            has_died,
            ..
        } => {
            assert_eq!(*earliest_frame, 42);
            assert_eq!(*last_position, Vec3::splat(-1.0));
            assert!(!has_died);
        }
        _ => panic!("authored height identity retained"),
    }
    drive(&mut world, id, 41);
    assert_eq!(world.host_object(id).unwrap().get_position().y, 5.0);
    drive(&mut world, id, 42);
    let object = world.host_object(id).unwrap();
    assert_eq!(
        object.get_position().y,
        0.0,
        "snap happens even when Body rejects kill"
    );
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(matches!(
        object.native_lifecycle.as_ref().unwrap().states[0],
        NativeLifecycleState::HeightDie { has_died: true, .. }
    ));
    world.host_object_mut(id).unwrap().indestructible = false;
    drive(&mut world, id, 43);
    assert!(!world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_defaults_allow_stationary_death_and_keep_negative_target() {
    for (fields, initial, later) in [("", -0.5, -0.5), ("TargetHeight = -2", -1.0, -3.0)] {
        let (mut world, id) = authored(
            "NeutralDefaultHeightOwner",
            &format!("Behavior = HeightDieUpdate HeightTag\n{fields}\nEnd"),
            30,
            Vec3::new(0.0, initial, 0.0),
        );
        drive(&mut world, id, 30);
        if fields.is_empty() {
            let object = world.host_object(id).unwrap();
            assert!(object.status.on_die_started);
            assert_eq!(
                object.get_position().y,
                0.0,
                "below-ground clamping precedes death even without SnapToGround"
            );
        } else {
            assert!(
                !world.host_object(id).unwrap().status.on_die_started,
                "negative authored TargetHeight must not be clamped to zero"
            );
            world.set_fixture_object_position(id, Vec3::new(0.0, later, 0.0));
            drive(&mut world, id, 31);
            assert!(world.host_object(id).unwrap().status.on_die_started);
        }
    }
}

#[test]
fn delayed_containment_does_not_update_last_position_before_admission() {
    let (mut world, id) = authored(
        "NeutralContainedHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nOnlyWhenMovingDown = Yes\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.set_fixture_object_contained_by(id, Some(ObjectId(999)));
    drive(&mut world, id, 40);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie { last_position, .. } => {
            assert_eq!(*last_position, Vec3::splat(-1.0))
        }
        _ => panic!("height module retained"),
    }
    drive(&mut world, id, 42);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie {
            last_position,
            has_died,
            ..
        } => {
            assert_eq!(*last_position, Vec3::new(0.0, 5.0, 0.0));
            assert!(!has_died);
        }
        _ => panic!("height module retained"),
    }
    world.set_fixture_object_contained_by(id, None);
    drive(&mut world, id, 43);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "unloading without descent does not die"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 4.0, 0.0));
    drive(&mut world, id, 44);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_particle_cleanup_observes_snap_after_rejected_body_and_only_attachments() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    let (mut world, id) = authored(
        "NeutralParticleHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nSnapToGroundOnDeath = Yes\nDestroyAttachedParticlesAtHeight = 1\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().indestructible = true;
    let peer = ObjectId(999);
    let mut attached = Vec::new();
    for kind in [
        CombatParticleKind::ParticleSysBone,
        CombatParticleKind::BodyFire,
        CombatParticleKind::BodySmoke,
        CombatParticleKind::DisableFx,
    ] {
        attached.push(
            world
                .combat_particles
                .attach_named_to_object_local(
                    id,
                    Vec3::new(0.0, 5.0, 0.0),
                    0.0,
                    Vec3::ZERO,
                    30,
                    "HeightCleanupAttachedWitness",
                    kind,
                    None,
                )
                .unwrap(),
        );
    }
    attached.push(
        world
            .combat_particles
            .spawn_projectile_exhaust(
                Vec3::new(0.0, 5.0, 0.0),
                30,
                peer,
                Some(id),
                "HeightCleanupExhaustWitness",
            )
            .unwrap(),
    );
    let peer_attached = world
        .combat_particles
        .attach_named_to_object(peer, Vec3::ZERO, 30, "HeightCleanupPeerWitness")
        .unwrap();
    let impact = world.combat_particles.spawn_named(
        CombatParticleKind::WeaponImpact,
        "HeightCleanupTargetImpactWitness",
        Vec3::new(0.0, 5.0, 0.0),
        30,
        Some(peer),
        Some(id),
    );
    drive(&mut world, id, 30);
    let object = world.host_object(id).unwrap();
    assert_eq!(object.get_position(), Vec3::ZERO);
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    match &object.native_lifecycle.as_ref().unwrap().states[0] {
        NativeLifecycleState::HeightDie {
            has_died,
            particles_destroyed,
            last_position,
            ..
        } => {
            assert!(*has_died && *particles_destroyed);
            assert_eq!(
                *last_position,
                Vec3::ZERO,
                "continuation captures the snapped live pose"
            );
        }
        _ => panic!("height module retained"),
    }
    let active: Vec<_> = world
        .combat_particles
        .active_systems()
        .map(|entry| entry.id)
        .collect();
    for attachment in attached {
        assert!(!active.contains(&attachment));
    }
    assert!(
        active.contains(&peer_attached),
        "another receiving owner retains its particles"
    );
    assert!(
        active.contains(&impact),
        "damage target association does not mean attachment"
    );
    let later = world
        .combat_particles
        .attach_named_to_object(id, Vec3::ZERO, 31, "HeightCleanupLaterWitness")
        .unwrap();
    drive(&mut world, id, 31);
    assert!(
        world
            .combat_particles
            .active_systems()
            .any(|entry| entry.id == later),
        "once-only cleanup latch does not destroy later attachments"
    );
}

#[test]
fn hulk_override_is_kind_based_constructor_only_and_first_lifetime_range_is_local() {
    let (mut owner, _ordinary) = authored_with_extra(
        "HulkSpellingIsNotAKind",
        "Behavior = LifetimeUpdate First\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = LifetimeUpdate Second\nMinLifetime = 95\nMaxLifetime = 95\nEnd\nBehavior = DeletionUpdate Delete\nMinLifetime = 200\nMaxLifetime = 200\nEnd",
        30,
        Vec3::ZERO,
        "ChildObject NeutralDebris HulkSpellingIsNotAKind\nKindOf = IMMOBILE INERT NO_COLLIDE HULK\nEnd",
    );
    owner.native_hulk_max_lifetime_override = 9;
    let named = owner
        .create_object("HulkSpellingIsNotAKind", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let deadline = |world: &GameLogic, id, index| match world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[index]
    {
        NativeLifecycleState::Lifetime { die_frame, .. }
        | NativeLifecycleState::Deletion { die_frame, .. } => die_frame,
        _ => panic!("expected timer"),
    };
    assert_eq!(deadline(&owner, named, 0), 32);
    let id = owner
        .create_object("NeutralDebris", Team::Neutral, Vec3::ZERO)
        .unwrap();
    assert_eq!(deadline(&owner, id, 0), 39);
    assert_eq!(deadline(&owner, id, 1), 39);
    assert_eq!(deadline(&owner, id, 2), 36); // Deletion never applies the HULK override.
    let random = owner.logic_random.clone();
    owner
        .host_object_mut(id)
        .unwrap()
        .set_native_lifetime_range(
            30,
            crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                minimum: 4,
                maximum: 4,
            },
            &random,
        );
    assert_eq!(deadline(&owner, id, 0), 34);
    assert_eq!(deadline(&owner, id, 1), 39);
    let mut peer = GameLogic::new();
    peer.native_hulk_max_lifetime_override = 77;
    peer.reset();
    assert_eq!(peer.native_hulk_max_lifetime_override, -1);
    assert_eq!(owner.native_hulk_max_lifetime_override, 9);
}

#[test]
fn height_structure_query_uses_receiving_geometry_centers_and_strict_3d_range() {
    fn scene(height: f32, x: f32, base_y: f32) -> (GameLogic, ObjectId) {
        let (mut world, id) = authored_with_extra(
            "NeutralStructureHeightOwner",
            "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 0\nTargetHeightIncludesStructures = Yes\nEnd\nGeometry = SPHERE\nGeometryMajorRadius = 1\nGeometryHeight = 1",
            30,
            Vec3::new(0.0, 10.0, 0.0),
            &format!(
                "Object ReceivingStructure\nKindOf = STRUCTURE\nBody = ActiveBody StructureBodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nGeometry = CYLINDER\nGeometryIsSmall = No\nGeometryHeight = {height}\nGeometryMajorRadius = 0\nGeometryMinorRadius = 0\nEnd"
            ),
        );
        world
            .create_object("ReceivingStructure", Team::USA, Vec3::new(x, base_y, 0.0))
            .unwrap();
        (world, id)
    }
    // Source sphere radius1; tall cylinder centerY10/radius10. At x12 the
    // sphere-bound distance is exactly1, which C++ excludes with strict <.
    let (mut boundary, boundary_id) = scene(20.0, 12.0, 0.0);
    let (mut inside, inside_id) = scene(20.0, 11.5, 0.0);
    let (mut low, low_id) = scene(5.0, 0.0, 0.0);
    let (mut above, above_id) = scene(20.0, 0.0, 100.0);
    assert_eq!(boundary_id, inside_id);
    assert_eq!(inside_id, low_id);
    assert_eq!(low_id, above_id);
    drive(&mut inside, inside_id, 30);
    assert!(
        inside.host_object(inside_id).unwrap().status.on_die_started,
        "cylinder center offset admits the nearby tall structure"
    );
    for (world, id) in [
        (&mut boundary, boundary_id),
        (&mut low, low_id),
        (&mut above, above_id),
    ] {
        drive(world, id, 30);
        assert!(
            !world.host_object(id).unwrap().status.on_die_started,
            "peer geometry, strict boundary and vertically distant structures cannot influence this owner"
        );
        assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    }
}

#[test]
fn field_expiry_observes_earliest_timer_without_advancing_height_twice() {
    let (mut world, id) = authored(
        "NeutralFieldModuleOrder",
        "Behavior = LifetimeUpdate Late\nMinLifetime = 20000\nMaxLifetime = 20000\nEnd\nBehavior = LifetimeUpdate Early\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = HeightDieUpdate Height\nTargetHeight = 0\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 50.0, 0.0),
    );
    let object = world.host_object_mut(id).unwrap();
    object.anthrax_toxin_field = true;
    object.anthrax_toxin_field_expires_frame = Some(9999);
    object.indestructible = true;
    world.frame = 32;
    world.update_anthrax_toxin_field_objects();
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert!(matches!(
        states[0],
        NativeLifecycleState::Lifetime { active: true, .. }
    ));
    assert!(matches!(
        states[1],
        NativeLifecycleState::Lifetime { active: false, .. }
    ));
    assert!(matches!(
        states[2],
        NativeLifecycleState::HeightDie {
            earliest_frame: u32::MAX,
            ..
        }
    ));
    world.tick_native_lifecycle(id);
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert!(
        matches!(states[2],NativeLifecycleState::HeightDie{earliest_frame:34,last_position,..} if last_position==Vec3::splat(-1.0))
    );
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
}

#[test]
fn authored_timer_runs_through_real_ai_without_a_name_profile() {
    let (mut world, id) = authored(
        "NeutralAiLifetime",
        "Behavior = LifetimeUpdate Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd",
        30,
        Vec3::ZERO,
    );
    world.frame = 31;
    world.update_ai(&[id], 1.0 / 30.0);
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    world.frame = 32;
    world.update_ai(&[id], 1.0 / 30.0);
    assert!(world.host_object(id).unwrap().status.on_die_started);
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
}

#[test]
fn disabled_modules_defer_without_consuming_timer_or_height_delay() {
    for kind in ["LifetimeUpdate", "DeletionUpdate"] {
        let (mut world, id) = authored(
            "DisabledNativeTimer",
            &format!("Behavior = {kind} Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd"),
            30,
            Vec3::ZERO,
        );
        world.host_object_mut(id).unwrap().status.disabled_emp = true;
        drive(&mut world, id, 40);
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.current, 100.0);
        assert!(!object.status.destroyed);
        assert!(matches!(
            object.native_lifecycle.as_ref().unwrap().states[0],
            NativeLifecycleState::Lifetime { active: true, .. }
                | NativeLifecycleState::Deletion { active: true, .. }
        ));
        world.host_object_mut(id).unwrap().status.disabled_emp = false;
        drive(&mut world, id, 41);
        assert!(world.host_object(id).unwrap().status.destroyed);
    }
    let (mut world, id) = authored(
        "DisabledNativeHeight",
        "Behavior = HeightDieUpdate Height\nTargetHeight = 10\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().status.disabled_held = true;
    drive(&mut world, id, 40);
    assert!(matches!(
        world
            .host_object(id)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap()
            .states[0],
        NativeLifecycleState::HeightDie {
            earliest_frame: u32::MAX,
            ..
        }
    ));
    world.host_object_mut(id).unwrap().status.disabled_held = false;
    drive(&mut world, id, 41);
    drive(&mut world, id, 42);
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    drive(&mut world, id, 43);
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
}

#[test]
fn construction_and_weapon_jamming_are_not_disabled_types_for_lifecycle() {
    let (mut world, id) = authored(
        "ConstructionTimer",
        "Behavior = LifetimeUpdate Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd",
        30,
        Vec3::ZERO,
    );
    let object = world.host_object_mut(id).unwrap();
    object.status.under_construction = true;
    object.status.weapons_jammed = true;
    assert!(!object.has_disabled_type());
    drive(&mut world, id, 32);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}
