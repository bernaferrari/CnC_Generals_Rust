//! Real normal/passive accepted fire, receiving immutable effects, and callback order.
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon,
};
use gamelogic::effects::ObjectCreationList;
use gamelogic::object_creation_list::GenericObjectCreationNugget;
use glam::Vec3;
use std::sync::Arc;
const WEAPON: &str = "ReceivingEffectWeapon";
const FIRE_CHILD: &str = "ReceivingFireChild";
const DEATH_CHILD: &str = "ReceivingDeathChild";
fn world(play_hidden: bool, suspend: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "observer", true));
    world.add_player(Player::new(1, Team::China, "source", false));
    world.add_player(Player::new(2, Team::GLA, "victim", false));
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
    actor.ai_state = AIState::Attacking;
    actor.weapon.as_mut().unwrap().suspend_fx_frame = suspend;
    world.frame = 30;
    (world, source, target)
}
fn child(world: &GameLogic, name: &str) -> ObjectId {
    let ids: Vec<_> = world
        .host_objects()
        .iter()
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
    assert_eq!(peer.weapon_discharge_log.len(), 0);
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, 100.0);
    assert!(
        !peer
            .host_objects()
            .values()
            .any(|o| o.template_name == FIRE_CHILD)
    );
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
        actor.ai_state = AIState::Garrisoned;
        actor.status.stealthed = true;
        actor.status.detected = false;
        let muzzle = Vec3::new(11.0, 0.0, 19.0);
        assert!(owner.fire_owned_passive_weapon(source, target, 0, muzzle));
        let fire = child(&owner, FIRE_CHILD);
        let death = child(&owner, DEATH_CHILD);
        assert!(fire.0 < death.0);
        assert_eq!(owner.host_object(fire).unwrap().get_position(), muzzle);
        let events = owner.weapon_discharge_log.take_for_presentation();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source, source);
        assert_eq!(events[0].visual_plan.as_ref().unwrap().fx_route, expected);
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
            owner.host_object(source).unwrap().ai_state,
            AIState::Garrisoned
        );
    }
}
