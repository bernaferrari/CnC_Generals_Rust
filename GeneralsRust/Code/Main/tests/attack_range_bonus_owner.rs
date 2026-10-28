use generals_main::game_logic::{
    AbleToAttackType, CanAttackResult, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate,
    Weapon,
};
use glam::Vec3;

fn world(range_percent: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    logic
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n WeaponBonus = SOLO_HUMAN_EASY RANGE {range_percent}%\nEnd\n"
        )])
        .expect("receiving GameData accepted");
    let mut owner = Player::new(42, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    let mut target_owner = Player::new(43, Team::GLA, "Target", false);
    target_owner.alliance_team = 2;
    logic.add_player(owner);
    logic.add_player(target_owner);

    let mut attacker = ThingTemplate::new("RangeBonusAttacker");
    attacker
        .set_health(100.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
    let mut target = ThingTemplate::new("RangeBonusTarget");
    target
        .set_health(100.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_none();
    logic.templates.insert(attacker.name.clone(), attacker);
    logic.templates.insert(target.name.clone(), target);

    let attacker = logic
        .create_object_for_player("RangeBonusAttacker", 42, Vec3::ZERO)
        .expect("attacker admitted");
    let target = logic
        .create_object_for_player("RangeBonusTarget", 43, Vec3::new(120.0, 0.0, 0.0))
        .expect("target admitted");
    logic
        .host_object_mut(attacker)
        .expect("attacker object")
        .weapon_bonus_solo = 16; // SOLO_HUMAN_EASY
    (logic, attacker, target)
}

#[test]
fn attack_range_uses_receiving_solo_bonus_for_same_ids() {
    let (boosted, boosted_attacker, boosted_target) = world(200);
    let (neutral, neutral_attacker, neutral_target) = world(100);
    assert_eq!(boosted_attacker, neutral_attacker);
    assert_eq!(boosted_target, neutral_target);

    // This public GameLogic query is used by attack commands and the host AI
    // state machine. Each receiver must use its own admitted WeaponBonusSet.
    assert_eq!(
        boosted.get_able_to_attack_specific_object(
            boosted_attacker,
            boosted_target,
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::Possible,
    );
    assert_eq!(
        neutral.get_able_to_attack_specific_object(
            neutral_attacker,
            neutral_target,
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::PossibleAfterMoving,
    );
    assert!(!boosted.out_of_weapon_range_object(boosted_attacker, boosted_target));
    assert!(neutral.out_of_weapon_range_object(neutral_attacker, neutral_target));
}
