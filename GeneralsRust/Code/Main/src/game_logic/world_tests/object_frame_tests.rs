//! Authoritative object identity, pose, weapons, and path publication.
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::game_logic::{GameLogic, Team};
use crate::presentation_frame::PresentationFrame;
use glam::Vec3;

#[test]
fn same_faction_opponents_keep_command_and_presentation_ownership() {
    use crate::game_logic::Player;
    use game_engine::common::game_common::Relationship;

    let mut logic = GameLogic::new();
    logic.clear_all_players();
    let mut local = Player::new(0, Team::USA, "USA local", true);
    local.alliance_team = 0;
    let mut opponent = Player::new(1, Team::USA, "USA opponent", false);
    opponent.alliance_team = 1;
    logic.add_player(local);
    logic.add_player(opponent);

    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("OwnerParityUnit")
            .kind_of("VEHICLE SELECTABLE")
            .health(100.0)],
    );
    let mine = logic
        .create_object_for_player("OwnerParityUnit", 0, Vec3::ZERO)
        .expect("local unit");
    let theirs = logic
        .create_object_for_player("OwnerParityUnit", 1, Vec3::new(20.0, 0.0, 0.0))
        .expect("opponent unit");

    let mine_host = logic.host_object(mine).expect("mine host");
    let theirs_host = logic.host_object(theirs).expect("opponent host");
    assert_eq!(mine_host.team, Team::USA);
    assert_eq!(theirs_host.team, Team::USA);
    assert_eq!(mine_host.owner_player_id, Some(0));
    assert_eq!(theirs_host.owner_player_id, Some(1));
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    assert_eq!(
        logic.object_relationship(mine_host, theirs_host),
        Relationship::Enemies
    );

    // A stale client may hold another player's ObjectId in its selection
    // (selection is mask-only); the direct-order gate must still refuse it.
    logic.select_objects(0, vec![mine, theirs]);
    assert_eq!(
        logic.get_player(0).expect("local player").selected_objects,
        vec![mine, theirs]
    );
    logic
        .get_player_mut(0)
        .expect("local player")
        .selected_objects = vec![theirs];
    logic.command_move(0, Vec3::new(100.0, 0.0, 0.0));
    assert!(
        logic
            .host_object(theirs)
            .expect("opponent host")
            .movement
            .target_position
            .is_none(),
        "player 0 must not move player 1's same-faction unit"
    );
    logic
        .get_player_mut(0)
        .expect("local player")
        .selected_objects = vec![mine];
    logic.command_move(0, Vec3::new(100.0, 0.0, 0.0));
    assert!(
        logic
            .host_object(mine)
            .expect("local host")
            .movement
            .target_position
            .is_some(),
        "the owning player retains command authority"
    );

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let mine_frame = frame
        .objects
        .iter()
        .find(|o| o.id == mine)
        .expect("mine frame");
    let theirs_frame = frame
        .objects
        .iter()
        .find(|o| o.id == theirs)
        .expect("opponent frame");
    assert_eq!(mine_frame.owner_player_id, Some(0));
    assert_eq!(theirs_frame.owner_player_id, Some(1));
    assert!(frame.is_owned_by_local(mine_frame));
    assert!(!frame.is_owned_by_local(theirs_frame));
    assert!(frame.is_enemy_of_local(theirs_frame));
}

#[test]
fn published_roster_grows_and_uses_fresh_births_after_reset() {
    let mut world = GameLogic::new();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("RosterUnit")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    let ids: Vec<_> = (0..70)
        .map(|index| {
            world
                .create_object(
                    "RosterUnit",
                    Team::USA,
                    Vec3::new(index as f32 * 10.0, 0.0, 0.0),
                )
                .unwrap()
        })
        .collect();
    let previous = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(previous.renderable_object_ids(), ids);
    let old_pose = previous.objects[0].position;
    let old_identity = world
        .host_object(ids[0])
        .unwrap()
        .visual_draw_identity_token
        .clone();

    world.reset();
    let empty = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(empty.objects.is_empty());
    let new_pose = Vec3::new(21.0, 0.0, 22.0);
    let replacement = world
        .create_object("RosterUnit", Team::USA, new_pose)
        .unwrap();
    assert_eq!(replacement, ids[0]);
    assert!(!std::sync::Arc::ptr_eq(
        &old_identity,
        &world
            .host_object(replacement)
            .unwrap()
            .visual_draw_identity_token,
    ));
    let current = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(current.renderable_object_ids(), [replacement]);
    assert_eq!(current.objects[0].position, new_pose);
    assert_eq!(previous.renderable_object_ids(), ids);
    assert_eq!(previous.objects[0].position, old_pose);
}

#[test]
fn pose_publication_keeps_earlier_values_after_the_owner_moves_and_turns() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("PoseFrameUnit")
            .kind_of("SELECTABLE")
            .health(80.0)],
    );
    let id = logic
        .create_object("PoseFrameUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let previous = before.objects.iter().find(|row| row.id == id).unwrap();
    let position = Vec3::new(42.0, 1.0, 7.0);
    logic.set_fixture_object_position(id, position);
    logic.set_fixture_object_orientation(id, 0.5);
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let current = frame.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(current.position, position);
    assert_eq!(current.orientation, 0.5);
    assert_eq!(previous.position, Vec3::ZERO);
    assert_eq!(previous.orientation, 0.0);
}

#[test]
fn frame_publishes_weapon_facts_and_the_current_path_cursor() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("WpnMoveU")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    let id = logic
        .create_object("WpnMoveU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        use crate::game_logic::Weapon;
        let obj = logic.host_object_mut(id).expect("obj");
        obj.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            min_range: 5.0,
            reload_time: 1.5,
            last_fire_time: 0.0,
            last_fire_frame: 0,
            ammo: Some(30),
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.1,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
            next_fire_frame: 0,
            reload_started_frame: 0,
            frame_clock_active: false,
            frame_clock_status: Default::default(),
        });
        obj.secondary_weapon = Some(Weapon::default());
        obj.movement.max_speed = 12.5;
        obj.movement.velocity = glam::Vec3::new(1.0, 0.0, 2.0);
        obj.movement.set_path_coords(vec![
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(10.0, 0.0, 10.0),
        ]);
        obj.movement.path_cursor = obj.movement.path.cursor_at_ordinal(1);
    }
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let e = frame.objects.iter().find(|row| row.id == id).unwrap();
    assert!(e.has_weapon && e.has_secondary_weapon);
    assert!((e.weapon_damage - 25.0).abs() < 0.01);
    assert!((e.weapon_range - 150.0).abs() < 0.01);
    assert!((e.weapon_min_range - 5.0).abs() < 0.01);
    assert_eq!(e.weapon_ammo, 30);
    assert!(e.weapon_can_target_air);
    assert!((e.move_max_speed - 12.5).abs() < 0.01);
    assert!((e.velocity.z - 2.0).abs() < 0.01);
    assert_eq!(e.path_len, 2);
    assert_eq!(e.path_index, 1);
}

#[test]
fn published_body_damage_keeps_really_damaged_ordinal() {
    use generals_sim::object::BodyDamageType;

    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("TankBd")
            .kind_of("VEHICLE SELECTABLE")
            .health(100.0)],
    );
    let oid = logic
        .create_object("TankBd", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.body_damage_state = BodyDamageType::ReallyDamaged;
    }

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let tank = frame.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(
        tank.body_damage_state,
        BodyDamageType::ReallyDamaged.ordinal()
    );
    assert_eq!(
        logic.host_object(oid).unwrap().body_damage_state,
        BodyDamageType::ReallyDamaged
    );
}

#[test]
fn frame_publishes_receiving_shock_stun_and_terrain_marks() {
    let mut logic = GameLogic::new();
    admit_fixture_objects(
        &mut logic,
        &[FixtureObject::new("ShockU")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)],
    );
    let oid = logic
        .create_object("ShockU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.shock_stun_frames = 30;
        o.shock_yaw_rate = 0.5;
        o.shock_pitch_rate = -0.25;
        o.shock_roll_rate = 0.1;
        o.shock_up_z = 0.9;
        o.shock_allow_bounce = true;
        o.shock_grounded_once = true;
        o.shock_was_airborne = true;
        o.cell_is_cliff = true;
        o.cell_is_underwater = false;
    }

    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let unit = frame.objects.iter().find(|row| row.id == oid).unwrap();
    assert_eq!(unit.shock_stun_frames, 30);
    assert!(unit.shock_allow_bounce && unit.shock_grounded_once && unit.shock_was_airborne);
    assert!(unit.cell_is_cliff);
    assert!(!unit.cell_is_underwater);

    let owner = logic.host_object(oid).unwrap();
    assert_eq!(
        (
            owner.shock_yaw_rate,
            owner.shock_pitch_rate,
            owner.shock_roll_rate
        ),
        (0.5, -0.25, 0.1)
    );
    assert_eq!(owner.shock_up_z, 0.9);
    assert_eq!(owner.shock_stun_frames, 30);
}
