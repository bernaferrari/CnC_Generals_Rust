//! Real normal/passive accepted fire, receiving immutable effects, and callback order.
use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use glam::Vec3;
const WEAPON: &str = "ReceivingEffectWeapon";
const FIRE_CHILD: &str = "ReceivingFireChild";
const DEATH_CHILD: &str = "ReceivingDeathChild";
fn world(play_hidden: bool, suspend: u32, local_player: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    for (id, team, name) in [
        (0, Team::USA, "observer"),
        (1, Team::China, "source"),
        (2, Team::GLA, "victim"),
    ] {
        let mut player = Player::new(id, team, name, id == local_player);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    world
        .admit_ocl_application_sources(&[
            format!("ObjectCreationList ReceivingFireOCL\nCreateObject\nObjectNames = {FIRE_CHILD}\nCount = 1\nEnd\nEnd"),
            format!("ObjectCreationList ReceivingDeathOCL\nCreateObject\nObjectNames = {DEATH_CHILD}\nCount = 1\nEnd\nEnd"),
        ])
        .unwrap();
    let weapon = FixtureWeapon::new(WEAPON)
        .damage(200.0)
        .range(500.0)
        .delay_ms(0)
        .clip(3, 0)
        .field("AntiGround = Yes")
        // Weapon.cpp's instant-hit speed exercises FireOCL then Body on one shot.
        .field("WeaponSpeed = 999999")
        .field("FireFX = ReceivingFireFX")
        .field("FireOCL = ReceivingFireOCL")
        .field(format!(
            "PlayFXWhenStealthed = {}",
            if play_hidden { "Yes" } else { "No" }
        ));
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new("ReceivingEffectSource")
                .kind_of("VEHICLE SELECTABLE CAN_ATTACK")
                .health(100.0)
                .walker()
                .weapon("PRIMARY", WEAPON),
            FixtureObject::new("ReceivingEffectVictim")
                .kind_of("VEHICLE SELECTABLE")
                .health(100.0)
                .module("Behavior = CreateObjectDie CreateChild\nCreationList = ReceivingDeathOCL\nEnd")
                .module("Behavior = DestroyDie RemoveVictim\nEnd"),
            FixtureObject::new(FIRE_CHILD).health(100.0),
            FixtureObject::new(DEATH_CHILD).health(100.0),
        ],
        &[weapon],
    );
    let source = world
        .create_object_for_player("ReceivingEffectSource", 1, Vec3::new(5.0, 0.0, 7.0))
        .unwrap();
    let target = world
        .create_object_for_player("ReceivingEffectVictim", 2, Vec3::new(15.0, 0.0, 7.0))
        .unwrap();
    let actor = world.host_object_mut(source).unwrap();
    actor.target = Some(target);
    actor.restore_ai_state(AIState::Attacking);
    actor.weapon.as_mut().unwrap().suspend_fx_frame = suspend;
    world.frame = 30;
    (world, source, target)
}
fn child(world: &GameLogic, name: &str) -> ObjectId {
    let ids: Vec<_> = world
        .host_objects()
        .registered()
        .filter(|(_, o)| o.template_name == name)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(ids.len(), 1, "one actual creation callback for {name}");
    ids[0]
}

#[test]
fn accepted_fire_reports_ocl_factory_failure_before_delivery() {
    let (mut owner, source, target) = world(false, 0, 1);
    owner.templates.get_mut(FIRE_CHILD).unwrap().body_definition_error =
        Some("injected fire OCL factory failure".into());
    let origin = owner.host_object(source).unwrap().get_position();
    let target_position = owner.host_object(target).unwrap().get_position();
    let result = owner.begin_owned_weapon_effects(
        source, 0, Some(target), Some(target_position), origin, None,
    );
    assert!(result.is_err(), "the accepted fire must report its OCL failure");
    assert_eq!(owner.host_object(target).unwrap().health.current, 100.0);
    assert!(!owner.host_objects().registered_values().any(|object| object.template_name == FIRE_CHILD));
}
#[test]
fn accepted_fire_ocl_uses_the_source_object_basis() {
    let (mut owner, source, target) = world(false, 0, 1);
    owner.apply_receiving_ocl_overrides(&[format!(
        "ObjectCreationList ReceivingFireOCL\nCreateObject\nObjectNames = {FIRE_CHILD}\nCount = 1\nDisposition = LIKE_EXISTING\nOffset = X:1 Y:2 Z:3\nEnd\nEnd"
    )]).unwrap();
    let origin = Vec3::new(5.0, 0.0, 7.0);
    let pose = glam::Mat4::from_cols(
        Vec3::Y.extend(0.0), Vec3::Z.extend(0.0), Vec3::X.extend(0.0), origin.extend(1.0),
    );
    assert!(owner.set_registered_object_transform(source, pose));
    let accepted = owner.begin_owned_weapon_effects(
        source, 0, Some(target), Some(Vec3::new(15.0, 0.0, 7.0)), origin, None,
    ).unwrap().unwrap();
    let child = child(&owner, FIRE_CHILD);
    let mut expected = pose;
    expected.w_axis = (origin + Vec3::new(2.0, 1.0, 3.0)).extend(1.0);
    assert_eq!(owner.objects[&child].get_transform_matrix(), expected);
    assert_eq!(owner.objects[&child].producer_id, Some(source));
    owner.finish_accepted_weapon_discharge(accepted);
}

#[test]
fn normal_fire_ocl_precedes_lethal_body_callback_and_save_never_replays_it() {
    let (mut owner, source, target) = world(false, 0, 1);
    let (peer, peer_source, peer_target) = world(false, 0, 1);
    assert_eq!((source, target), (peer_source, peer_target));
    owner.update_combat(&[source, target], 1.0 / 30.0).unwrap();
    let fire = child(&owner, FIRE_CHILD);
    let death = child(&owner, DEATH_CHILD);
    assert!(
        fire.0 < death.0,
        "FireOCL completes before victim CreateObjectDie"
    );
    assert_eq!(
        owner.host_object(fire).unwrap().get_position(),
        Vec3::new(5.0, 0.0, 7.0)
    );
    assert_eq!(
        owner
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    assert_eq!(owner.weapon_discharge_log.len(), 1);
    let muzzle = owner.combat_particles.systems_of_kind(
        crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash,
    );
    assert_eq!(muzzle.len(), 1, "named FireFX has no second generic muzzle");
    assert_eq!(muzzle[0].fx_list_name, "ReceivingFireFX");
    assert_eq!(peer.weapon_discharge_log.len(), 0);
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, 100.0);
    assert!(
        !peer
            .host_objects()
            .registered_values()
            .any(|o| o.template_name == FIRE_CHILD)
    );
    // Save only at the real settled cleanup boundary. Immediate lethal Body
    // callbacks intentionally reject snapshots while destruction is pending.
    owner.fire_temporary_weapons_for_pending_deaths().unwrap();
    owner.process_destroy_list();
    assert!(!owner.has_pending_object_removals());
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut loaded, _, _) = world(false, 0, 1);
    builder.restore_from_snapshot(&saved, &mut loaded).unwrap();
    loaded.frame = 31;
    loaded.update_combat(&[source, target], 1.0 / 30.0).unwrap();
    assert_eq!(child(&loaded, FIRE_CHILD), fire);
    assert_eq!(child(&loaded, DEATH_CHILD), death);
    assert_eq!(
        loaded.weapon_discharge_log.len(),
        0,
        "load emits no old acceptance"
    );
    let events = owner.weapon_discharge_log.take_for_presentation();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].visual_plan.as_ref().unwrap().fx_route,
        crate::presentation_frame::FrozenWeaponVisualFxRoute::BroadcastWithFireFx
    );
}
#[test]
fn passive_fire_keeps_muzzle_origin_and_effect_gates_do_not_suppress_ocl() {
    use crate::presentation_frame::FrozenWeaponVisualFxRoute;
    for (play_hidden, suspend, expected) in [
        (false, 0, FrozenWeaponVisualFxRoute::DrawableSuppressed),
        (true, 0, FrozenWeaponVisualFxRoute::BroadcastWithFireFx),
        (true, 31, FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx),
    ] {
        let (mut owner, source, target) = world(play_hidden, suspend, 0);
        let actor = owner.host_object_mut(source).unwrap();
        actor.restore_ai_state(AIState::Garrisoned);
        actor.status.stealthed = true;
        actor.status.detected = false;
        let muzzle = Vec3::new(11.0, 0.0, 19.0);
        assert!(owner.fire_owned_passive_weapon(source, target, 0, muzzle));
        let fire = child(&owner, FIRE_CHILD);
        let death = child(&owner, DEATH_CHILD);
        assert!(fire.0 < death.0);
        assert_eq!(
            owner.host_object(fire).unwrap().get_position(),
            owner.host_object(source).unwrap().get_position(),
            "CPP FireOCL uses the source object, not its contained muzzle"
        );
        let events = owner.weapon_discharge_log.take_for_presentation();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source, source);
        assert_eq!(events[0].visual_plan.as_ref().unwrap().fx_route, expected);
        let flashes = owner.combat_particles.systems_of_kind(
            crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash,
        );
        if expected == FrozenWeaponVisualFxRoute::BroadcastWithFireFx {
            assert_eq!(flashes.len(), 1);
            assert_eq!(flashes[0].position, muzzle);
        } else {
            assert!(flashes.is_empty(), "suppressed FX creates no muzzle cue");
        }
        assert_eq!(
            owner
                .host_object(source)
                .unwrap()
                .weapon
                .as_ref()
                .unwrap()
                .ammo,
            Some(2)
        );
        assert_eq!(
            owner.host_object(source).unwrap().ai_state(),
            AIState::Garrisoned
        );
    }
}

#[test]
fn nested_attack_acceptance_delivers_fire_effects_before_body_and_never_requeues_them() {
    let (mut owner, source, target) = world(false, 0, 1);
    let (peer, peer_source, peer_target) = world(false, 0, 1);
    assert_eq!((source, target), (peer_source, peer_target));
    assert_eq!(
        owner.attack_state_enter(source, target),
        crate::game_logic::AttackMachineResult::Continue
    );
    let mut accepted_frame = None;
    for frame in 30..=45 {
        owner.frame = frame;
        // Normal combat must yield to this actual installed nested machine.
        owner.update_combat(&[source, target], 1.0 / 30.0).unwrap();
        owner.tick_nested_attack_machines(&[source], frame as f32 / 30.0, frame);
        if owner.weapon_discharge_log.len() != 0 {
            accepted_frame = Some(frame);
            break;
        }
    }
    let frame = accepted_frame.expect("actual AIM then FireWeapon must accept one shot");
    let fire = child(&owner, FIRE_CHILD);
    let death = child(&owner, DEATH_CHILD);
    assert!(
        fire.0 < death.0,
        "nested FireOCL precedes victim CreateObjectDie"
    );
    assert!(
        owner.host_object(target).map_or(true, |victim| {
            victim.health.current == 0.0 && !victim.is_alive()
        }),
        "real lethal Body phase completed"
    );
    assert_eq!(
        owner
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    assert_eq!(owner.weapon_discharge_log.len(), 1);
    assert_eq!(
        crate::game_logic::combat::pending_projectile_queue_len_for_test(&owner.combat_system),
        0
    );
    assert_eq!(
        crate::game_logic::combat::live_projectileless_delayed_count_for_test(&owner.combat_system),
        0
    );
    let flashes = owner.combat_particles.systems_of_kind(
        crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash,
    );
    assert_eq!(flashes.len(), 1);
    assert_eq!(flashes[0].fx_list_name, "ReceivingFireFX");
    assert_eq!(flashes[0].position, Vec3::new(5.0, 0.0, 7.0));
    owner.frame = frame + 1;
    crate::game_logic::combat::drain_pending_projectiles(
        &mut owner.combat_system,
        owner.objects.read(),
        owner.frame,
        &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
    );
    owner.apply_owned_projectileless_delayed_damage().unwrap();
    owner.tick_nested_attack_machines(&[source], owner.frame as f32 / 30.0, owner.frame);
    assert_eq!(child(&owner, FIRE_CHILD), fire);
    assert_eq!(child(&owner, DEATH_CHILD), death);
    assert_eq!(
        owner.weapon_discharge_log.len(),
        1,
        "later combat phase cannot replay this acceptance"
    );
    assert_eq!(
        owner
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, 100.0);
    assert_eq!(
        peer.host_object(peer_source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(3)
    );
    assert_eq!(peer.weapon_discharge_log.len(), 0);
    assert!(
        !peer
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == FIRE_CHILD)
    );
}


#[test]
fn no_drawable_skips_muzzle_fx_but_still_runs_fire_ocl() {
    // C++ Weapon.cpp:889 — muzzle/FireFX require sourceObj->getDrawable();
    // FireOCL at 943 is outside that gate and must still run.
    let (mut owner, source, target) = world(false, 0, 1);
    owner.host_object_mut(source).unwrap().has_drawable = false;
    let origin = owner.host_object(source).unwrap().get_position();
    let accepted = owner.begin_owned_weapon_effects(
        source,
        0,
        Some(target),
        Some(owner.host_object(target).unwrap().get_position()),
        origin,
        None,
    ).expect("fire effects succeed");
    assert!(accepted.is_some(), "accepted discharge without Drawable");
    owner.finish_accepted_weapon_discharge(accepted.unwrap());
    let fire = child(&owner, FIRE_CHILD);
    assert_eq!(
        owner.host_object(fire).unwrap().get_position(),
        Vec3::new(5.0, 0.0, 7.0),
        "FireOCL must still spawn without a Drawable"
    );
    let flashes = owner.combat_particles.systems_of_kind(
        crate::game_logic::combat_particles::CombatParticleKind::WeaponMuzzleFlash,
    );
    assert!(
        flashes.is_empty(),
        "no-Drawable source must not play muzzle/FireFX"
    );
    let events = owner.weapon_discharge_log.take_for_presentation();
    assert_eq!(events.len(), 1);
    assert!(
        events[0].visual_plan.is_none(),
        "absent Drawable yields no frozen visual plan"
    );
}
