//! Current native final-removal accounting must retain the incoming death
//! credit. C++ score timing remains a separate integration obligation.
use super::*;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{KindOf, Player, ThingTemplate};

#[test]
fn authored_removal_keeps_enemy_credit_bounty_and_once_only_stats() {
    for class in ["DestroyDie", "InstantDeathBehavior", "JetSlowDeathBehavior"] {
        let content = crate::assets::ReceivingObjectContent::from_sources([(
            "authored-death-credit.ini".to_owned(),
            format!("Object AuthoredCreditedVictim\nKindOf = VEHICLE SCORE\nBuildCost = 500\nExperienceValue = 25 25 25 25\nSkillPointValue = 25 25 25 25\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = {class} DeathTag\nEnd\nEnd\nObject CreditedSource\nKindOf = VEHICLE SCORE\nIsTrainable = Yes\nBody = ActiveBody SourceBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"),
        )]).expect("paired authored death-credit Body and Die modules");
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(content)
            .expect("receiving death-credit owner");
        let victim_template = &world.templates["AuthoredCreditedVictim"];
        assert_eq!(victim_template.build_cost.supplies, 500);
        assert_eq!(victim_template.experience_values, [25.0; 4]);
        assert_eq!(victim_template.skill_point_values, [25; 4]);
        let mut source_player = Player::new(0, Team::USA, "Source", true);
        source_player.alliance_team = 1;
        source_player.cash_bounty_percent = 0.2;
        source_player.resources.supplies = 1000;
        world.add_player(source_player);
        let mut victim_player = Player::new(2, Team::GLA, "Victim", false);
        victim_player.alliance_team = 2;
        world.add_player(victim_player);
        let source = world
            .create_object_for_player("CreditedSource", 0, Vec3::ZERO)
            .unwrap();
        let victim = world
            .create_object_for_player("AuthoredCreditedVictim", 2, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        world
            .apply_owned_damage(
                victim,
                1000.0,
                Some(source),
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
            .unwrap();
        assert!(
            world.objects.contains_key(&victim),
            "{class}: callback queues, not physical removal"
        );
        let receipts: Vec<_> = world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == victim)
            .collect();
        assert_eq!(receipts.len(), 1, "{class}");
        assert_eq!(
            receipts[0].killer,
            Some(Team::USA),
            "{class}: incoming credit must survive authored deletion"
        );
        assert_eq!(
            world.objects[&source].experience.current, 25.0,
            "{class}: XP occurs before removal"
        );
        assert_eq!(
            world.get_player(0).unwrap().skill_points,
            25,
            "{class}: CPP scoreTheKill awards Player skill exactly once before onDie"
        );
        world.destroy_object(victim); // repeated public request must not overwrite credit.
        world.process_destroy_list();
        assert!(
            !world.objects.contains_key(&victim),
            "{class}: physical removal completed"
        );
        let source_player = world.get_player(0).unwrap();
        assert_eq!(source_player.statistics.units_destroyed(), 1, "{class}");
        assert_eq!(source_player.statistics.money_earned(), 100, "{class}");
        assert_eq!(source_player.resources.supplies, 1100, "{class}");
        assert_eq!(source_player.skill_points, 25, "{class}");
        assert_eq!(
            world.get_player(2).unwrap().statistics.units_lost(),
            1,
            "{class}"
        );
        assert_eq!(
            world.objects[&source].experience.current, 25.0,
            "{class}: removal cannot award XP again"
        );
        world.process_destroy_list();
        assert_eq!(
            world.get_player(0).unwrap().statistics.units_destroyed(),
            1,
            "{class}: repeated drain is inert"
        );
        assert_eq!(
            world.get_player(0).unwrap().resources.supplies,
            1100,
            "{class}: bounty is once only"
        );
    }
}

#[test]
fn manual_mark_never_infers_score_the_kill_from_old_damage_history() {
    let mut world = GameLogic::new();
    let mut attacker = Player::new(0, Team::USA, "LegacySource", true);
    attacker.alliance_team = 1;
    world.add_player(attacker);
    let mut defender = Player::new(2, Team::GLA, "LegacyVictim", false);
    defender.alliance_team = 2;
    world.add_player(defender);
    let mut template = ThingTemplate::new("LegacyManualCreditUnit");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Score)
        .set_health(100.0);
    template.skill_point_values = [25; 4];
    world.templates.insert(template.name.clone(), template);
    let source = world
        .create_object_for_player("LegacyManualCreditUnit", 0, Vec3::ZERO)
        .unwrap();
    let victim = world
        .create_object_for_player("LegacyManualCreditUnit", 2, Vec3::X * 10.0)
        .unwrap();
    let object = world.objects.get_mut(&victim).unwrap();
    object.internal_change_health(-100.0);
    object.last_damage_source = Some(source);
    assert!(
        !object.kill_experience_awarded,
        "manual path never admitted owned scoreTheKill"
    );
    world.mark_object_for_destruction(victim, Some(Team::USA));
    assert_eq!(world.get_player(0).unwrap().skill_points, 0);
    world.process_destroy_list();
    assert!(!world.objects.contains_key(&victim));
    assert_eq!(
        world.get_player(0).unwrap().skill_points,
        0,
        "manual onDie/removal without scoreTheKill must not fabricate skill credit"
    );
    world.process_destroy_list();
    assert_eq!(
        world.get_player(0).unwrap().skill_points,
        0,
        "physical removal cannot discover a killer from retained hit history"
    );
}
