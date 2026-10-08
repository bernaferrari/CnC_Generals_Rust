//! Additional candidate recovery coverage, separate from the frozen15-test baseline.
//! A public receiveGrant stealth effect invalidates a live turret goal; ordinary
//! updates must clear stale host/mood ownership and subsequently acquire again.
use gamelogic::common::Relationship;
use generals_main::game_logic::host_strategy_center::HostBattlePlan;
use generals_main::game_logic::{AIState, GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;
use serde_json::json;

#[test]
fn live_goal_becoming_stealthed_clears_mood_and_reacquires() {
    let mut logic = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::GLA)] {
        let mut player = Player::new(id, team, "Strategy recovery", id == 0);
        player.alliance_team = id as i32;
        logic.add_player(player);
    }
    let mut center = ThingTemplate::new("AmericaStrategyCenter");
    center
        .set_health(1500.0)
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Immobile);
    logic.templates.insert(center.name.clone(), center);
    for (name, kind) in [
        ("RecoveryVictim", KindOf::Vehicle),
        ("RecoveryAnchor", KindOf::Structure),
    ] {
        let mut template = ThingTemplate::new(name);
        template
            .set_health(1000.0)
            .add_kind_of(kind)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_none();
        logic.templates.insert(name.into(), template);
    }
    let center = logic
        .create_object_for_player("AmericaStrategyCenter", 0, Vec3::ZERO)
        .unwrap();
    let anchor = logic
        .create_object_for_player("RecoveryAnchor", 1, Vec3::new(2000.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(center)));
    for _ in 0..211 {
        logic.update();
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        assert_eq!(
            logic
                .host_object(center)
                .unwrap()
                .weapon_discharge_marker()
                .sequence,
            0
        );
    }
    let source = logic.host_object(center).unwrap();
    assert_eq!(source.owner_player_id, Some(0));
    assert!(source.is_constructed() && source.turret_enabled && source.can_attack());
    assert_eq!(source.weapon.as_ref().unwrap().damage, 200.0);
    assert!(!source.is_detector);
    let hidden = logic
        .create_object_for_player("RecoveryVictim", 1, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    logic.update();
    let acquired = logic.host_object(center).unwrap();
    assert!(acquired.turret_mood_target);
    assert_eq!(acquired.target, Some(hidden));
    assert_eq!(acquired.turret_target_id, Some(hidden));
    assert_eq!(acquired.weapon_discharge_marker().sequence, 0);
    assert!(logic.host_object(hidden).unwrap().is_alive());
    assert!(
        !logic
            .host_object(hidden)
            .unwrap()
            .is_effectively_stealthed()
    );
    assert_eq!(logic.host_object(hidden).unwrap().health.current, 1000.0);
    assert!(!logic.cannot_possibly_attack_object(center, hidden, false));
    assert!(
        logic
            .combat_system()
            .projectiles_snapshot()
            .iter()
            .all(|p| p.shooter_id != center)
    );

    // This is the real public target-side receiveGrant behavior used by GPS,
    // not a controller/goal-field repair or a full GPS UI/science activation.
    logic.host_object_mut(hidden).unwrap().apply_grant_stealth();
    assert!(logic.host_object(hidden).unwrap().is_alive());
    assert!(
        logic
            .host_object(hidden)
            .unwrap()
            .is_effectively_stealthed()
    );
    assert!(logic.cannot_possibly_attack_object(center, hidden, false));
    logic.update();
    let cleared = logic.host_object(center).unwrap();
    println!(
        "STRATEGY_RECOVERY_CLEAR {}",
        json!({"next_frame":logic.get_frame(),
        "target":cleared.target.map(|id|id.0),"turret_target":cleared.turret_target_id.map(|id|id.0),
        "mood":cleared.turret_mood_target,"ai":format!("{:?}",cleared.ai_state),
        "hidden_alive":logic.host_object(hidden).unwrap().is_alive(),
        "hidden_stealthed":logic.host_object(hidden).unwrap().is_effectively_stealthed()})
    );
    assert!(cleared.target.is_none() && cleared.turret_target_id.is_none());
    assert!(!cleared.turret_mood_target);
    assert_eq!(cleared.ai_state, AIState::Idle);
    assert_eq!(cleared.weapon_discharge_marker().sequence, 0);
    assert!(logic.battle_plans().turret_mood_target_clear_count() >= 1);

    let replacement = logic
        .create_object_for_player("RecoveryVictim", 1, Vec3::new(-150.0, 0.0, 0.0))
        .unwrap();
    assert_ne!(replacement, hidden);
    assert!(
        !logic
            .host_object(replacement)
            .unwrap()
            .is_effectively_stealthed()
    );
    assert!(!logic.cannot_possibly_attack_object(center, replacement, false));
    let mut reacquired = false;
    let mut first_shot = None;
    let mut first_damage = None;
    for _ in 0..120 {
        let frame = logic.get_frame();
        logic.update();
        let source = logic.host_object(center).unwrap();
        let victim = logic.host_object(replacement).unwrap();
        if source.turret_mood_target
            && source.target == Some(replacement)
            && source.turret_target_id == Some(replacement)
        {
            reacquired = true;
        }
        if source.weapon_discharge_marker().sequence > 0 && first_shot.is_none() {
            first_shot = Some(frame);
        }
        if victim.health.current < 1000.0 && first_damage.is_none() {
            first_damage = Some(frame);
        }
        println!(
            "STRATEGY_RECOVERY_TRACE {}",
            json!({"frame":frame,"mood":source.turret_mood_target,
            "target":source.target.map(|id|id.0),"turret_target":source.turret_target_id.map(|id|id.0),
            "yaw":source.turret_angle_deg,"substate":format!("{:?}",source.turret_substate),
            "sequence":source.weapon_discharge_marker().sequence,"replacement_health":victim.health.current,
            "hidden_health":logic.host_object(hidden).unwrap().health.current})
        );
        assert_eq!(source.owner_player_id, Some(0));
        assert_eq!(victim.owner_player_id, Some(1));
        assert_eq!(source.get_position(), Vec3::ZERO);
        assert!(logic.host_object(hidden).unwrap().is_alive());
        assert!(
            logic
                .host_object(hidden)
                .unwrap()
                .is_effectively_stealthed()
        );
        assert_eq!(logic.host_object(hidden).unwrap().health.current, 1000.0);
        assert_eq!(logic.host_object(anchor).unwrap().health.current, 1000.0);
    }
    assert!(reacquired, "cleared mood never acquired the new legal goal");
    assert!(first_shot.is_some() && first_damage.is_some());
    assert_eq!(
        logic
            .host_object(center)
            .unwrap()
            .weapon_discharge_marker()
            .sequence,
        1
    );
    assert!(logic.host_object(replacement).unwrap().health.current < 1000.0);
    println!("STRATEGY_RECOVERY_OUTCOME first_shot={first_shot:?} first_damage={first_damage:?}");
}
