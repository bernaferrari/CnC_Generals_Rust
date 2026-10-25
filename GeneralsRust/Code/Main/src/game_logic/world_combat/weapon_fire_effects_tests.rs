//! Real normal/passive accepted fire, receiving immutable effects, and callback order.
use crate::game_logic::{AIState, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon};
use gamelogic::effects::ObjectCreationList;
use gamelogic::object_creation_list::GenericObjectCreationNugget;
use glam::Vec3;
use std::sync::Arc;
const WEAPON: &str = "ReceivingEffectWeapon";
const FIRE_CHILD: &str = "ReceivingFireChild";
const DEATH_CHILD: &str = "ReceivingDeathChild";
fn world(play_hidden: bool, suspend: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    for (id, team, name) in [
        (0, Team::USA, "observer"),
        (1, Team::China, "source"),
        (2, Team::GLA, "victim"),
    ] {
        let mut player = Player::new(id, team, name, id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let mut native = gamelogic::weapon::WeaponTemplate::new(WEAPON.to_owned());
    native.play_fx_when_stealthed = play_hidden;
    native.fire_fx[0] = Some(gamelogic::effects::FXList::new("ReceivingFireFX"));
    native.fire_ocl_names[0] = Some("ReceivingFireOCL".into());
    world.admit_weapon_fire_definition(WEAPON, &native).unwrap();
    let mut ocl = ObjectCreationList::new();
    ocl.add_nugget(Arc::new(GenericObjectCreationNugget {
        names: vec![FIRE_CHILD.into()],
        ..Default::default()
    }));
    world
        .admit_weapon_ocl_definition("ReceivingFireOCL", ocl)
        .unwrap();
    let mut source = ThingTemplate::new("ReceivingEffectSource");
    source.add_kind_of(KindOf::Vehicle);
    source.add_kind_of(KindOf::Attackable);
    source.primary_weapon_name = Some(WEAPON.into());
    source.primary_weapon = Some(Weapon {
        damage: 200.0,
        range: 500.0,
        reload_time: 0.0,
        ammo: Some(3),
        clip_size: 3,
        last_fire_time: -10.0,
        ..Default::default()
    });
    world.templates.insert(source.name.clone(), source);
    let mut target = ThingTemplate::new("ReceivingEffectVictim");
    target.add_kind_of(KindOf::Attackable);
    target.set_health(100.0);
    world.templates.insert(target.name.clone(), target);
    for name in [FIRE_CHILD, DEATH_CHILD] {
        world
            .templates
            .insert(name.into(), ThingTemplate::new(name));
    }
    let source = world
        .create_object_for_player("ReceivingEffectSource", 1, Vec3::new(5.0, 0.0, 7.0))
        .unwrap();
    let target = world
        .create_object_for_player("ReceivingEffectVictim", 2, Vec3::new(15.0, 0.0, 7.0))
        .unwrap();
    world.host_object_mut(target).unwrap().create_object_die = Some(
        crate::game_logic::host_create_object_die::HostCreateObjectDieData::single(
            "ReceivingDeathOCL",
            DEATH_CHILD,
        ),
    );
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
fn normal_fire_ocl_precedes_lethal_body_callback_and_save_never_replays_it() {
    let (mut owner, source, target) = world(false, 0);
    let (peer, peer_source, peer_target) = world(false, 0);
    assert_eq!((source, target), (peer_source, peer_target));
    owner.update_combat(&[source, target], 1.0 / 30.0);
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
    owner.fire_temporary_weapons_for_pending_deaths();
    owner.process_destroy_list();
    assert!(!owner.has_pending_object_removals());
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut loaded, _, _) = world(false, 0);
    builder.restore_from_snapshot(&saved, &mut loaded).unwrap();
    loaded.frame = 31;
    loaded.update_combat(&[source, target], 1.0 / 30.0);
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
        let (mut owner, source, target) = world(play_hidden, suspend);
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
    let (mut owner, source, target) = world(false, 0);
    let (peer, peer_source, peer_target) = world(false, 0);
    assert_eq!((source, target), (peer_source, peer_target));
    assert_eq!(
        owner.attack_state_enter(source, target),
        crate::game_logic::AttackMachineResult::Continue
    );
    let mut accepted_frame = None;
    for frame in 30..=45 {
        owner.frame = frame;
        // Normal combat must yield to this actual installed nested machine.
        owner.update_combat(&[source, target], 1.0 / 30.0);
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
    );
    owner.apply_owned_projectileless_delayed_damage();
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
