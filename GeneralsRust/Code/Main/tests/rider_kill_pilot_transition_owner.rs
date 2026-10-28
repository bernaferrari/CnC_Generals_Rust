//! A real attack command drives KillPilot damage through RiderChange's
//! TransitionDamageFX callback against each receiving world's OCL catalog.
use gamelogic::damage::DamageType as NativeDamageType;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{AIState, GameLogic, ObjectAudio, ObjectId, Player, SessionAudio, Team};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

const WEAPON: &str = "OwnerTransitionVehiclePilotWeapon";
const SOURCE: &str = "OwnerTransitionShooter";
const VICTIM: &str = "OwnerTransitionRiderVehicle";
const CHILD_A: &str = "OwnerTransitionChildA";
const CHILD_B: &str = "OwnerTransitionChildB";

fn world(child_name: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut owner = Player::new(0, Team::USA, "owner", true);
    let mut enemy = Player::new(1, Team::GLA, "enemy", false);
    owner.alliance_team = 0;
    enemy.alliance_team = 1;
    world.add_player(owner);
    world.add_player(enemy);

    let object_ini = format!(
        "Object {SOURCE}\n\
         KindOf = VEHICLE SELECTABLE\n\
         Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n\
         WeaponSet\n Conditions = None\n Weapon = PRIMARY {WEAPON}\n End\nEnd\n\
         Object {VICTIM}\n\
         KindOf = VEHICLE SELECTABLE\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
         Behavior = RiderChangeContain RiderTag\n Slots = 1\n\
         Rider1 = OwnerTransitionRider RIDER1 WEAPON_RIDER1 STATUS_RIDER1 BikeSet SET_NORMAL\n End\n\
         Behavior = TransitionDamageFX TransitionTag\n\
         RubbleOCL1 = Loc: X:0 Y:0 Z:0 OCL:OCL_OwnerTransition\n DamageOCLTypes = ALL\n End\nEnd\n\
         Object {CHILD_A}\n KindOf = SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n\
         Object {CHILD_B}\n KindOf = SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n\
         Object OwnerTransitionRider\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n"
    );
    world
        .admit_object_ini_sources([("OwnerTransition.ini".to_owned(), object_ini)])
        .expect("receiving Object/TransitionDamageFX definitions");

    let mut weapon = gamelogic::weapon::WeaponTemplate::new(WEAPON.to_owned());
    weapon.damage_type = NativeDamageType::KillPilot;
    weapon.primary_damage = 100.0;
    weapon.attack_range = 200.0;
    weapon.min_delay_between_shots = 0;
    weapon.max_delay_between_shots = 0;
    world
        .admit_receiving_weapon_snapshot(vec![(WEAPON.to_owned(), weapon)])
        .expect("receiving KillPilot weapon rule");
    world
        .admit_object_creation_list_sources([format!(
            "ObjectCreationList OCL_OwnerTransition\nCreateObject\nObjectNames = {child_name}\nCount = 1\nEnd\nEnd\n"
        )])
        .expect("receiving transition OCL");

    let shooter_id = world
        .create_object_for_player(SOURCE, 0, Vec3::ZERO)
        .expect("shooter");
    let victim_id = world
        .create_object_for_player(VICTIM, 1, Vec3::new(10.0, 0.0, 0.0))
        .expect("RiderChange victim");
    let victim = world.host_object_mut(victim_id).expect("victim object");
    let mut audio = SessionAudio::default();
    victim.set_ai_state(AIState::Moving, &mut ObjectAudio::queue(&mut audio));
    victim.set_status_moving(true);
    victim.target = Some(ObjectId(u32::MAX));
    (world, shooter_id, victim_id)
}

fn run_attack(world: &mut GameLogic, shooter_id: ObjectId, victim_id: ObjectId) {
    world.queue_command(GameCommand {
        command_type: CommandType::AttackObject {
            target_id: victim_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![shooter_id],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    for _ in 0..120 {
        if world
            .host_objects()
            .registered()
            .any(|(_, object)| object.template_name == CHILD_A || object.template_name == CHILD_B)
        {
            return;
        }
        world.update();
    }
}

fn has_object(world: &GameLogic, name: &str) -> bool {
    world
        .host_objects()
        .registered()
        .any(|(_, object)| object.template_name == name)
}

#[test]
fn attack_command_routes_rider_kill_pilot_transition_to_each_world_catalog() {
    let (mut first, shooter_a, victim_a) = world(CHILD_A);
    let (mut second, shooter_b, victim_b) = world(CHILD_B);
    assert_eq!((shooter_a, victim_a), (shooter_b, victim_b));

    run_attack(&mut first, shooter_a, victim_a);
    run_attack(&mut second, shooter_b, victim_b);
    for (world, shooter) in [(&first, shooter_a), (&second, shooter_b)] {
        assert!(
            world
                .host_object(shooter)
                .unwrap()
                .weapon_discharge_marker()
                .sequence
                > 0,
            "the public attack command must fire the receiving KillPilot weapon"
        );
    }

    assert!(
        has_object(&first, CHILD_A),
        "first world admitted its own OCL child"
    );
    assert!(
        !has_object(&first, CHILD_B),
        "first world did not consult peer OCLs"
    );
    assert!(
        has_object(&second, CHILD_B),
        "second world admitted its own OCL child"
    );
    assert!(
        !has_object(&second, CHILD_A),
        "second world did not consult peer OCLs"
    );
    for world in [&first, &second] {
        let victim = world
            .host_object(victim_a)
            .expect("transition continuation kept victim installed");
        assert_eq!(victim.health.current, 0.0);
        assert!(
            !victim.status.moving,
            "damage tail completed after transition OCL"
        );
        assert!(victim.target.is_none(), "damage tail cleared stale target");
    }
}
