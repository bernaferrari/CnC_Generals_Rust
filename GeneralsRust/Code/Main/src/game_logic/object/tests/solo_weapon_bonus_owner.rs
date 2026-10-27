use super::*;

#[test]
fn explicit_solo_set_is_authoritative_and_neutral_when_missing_condition() {
    let mut object = make_ground_unit("SoloBonusOwner", 4, KindOf::Infantry);
    object.weapon_bonus_solo = 16;
    let entry = |value| game_engine::common::ini::ini_game_data::WeaponBonusEntry {
        condition: "SOLO_HUMAN_EASY".into(),
        field: "DAMAGE".into(),
        value,
    };
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    first.admit_global_weapon_bonus_entries(&[entry(1.5)]);
    second.admit_global_weapon_bonus_entries(&[entry(2.0)]);
    let first_fields = object.weapon_bonus_fields_for_named_weapon_with_solo_set(
        "AbsentExtraWeapon",
        &first.solo_weapon_bonus_set,
    );
    let second_fields = object.weapon_bonus_fields_for_named_weapon_with_solo_set(
        "AbsentExtraWeapon",
        &second.solo_weapon_bonus_set,
    );
    assert_eq!(first_fields.0, 1.5);
    assert_eq!(second_fields.0, 2.0);
    let empty = gamelogic::weapon::WeaponBonusSet::new();
    assert_eq!(
        object
            .weapon_bonus_fields_for_named_weapon_with_solo_set("AbsentExtraWeapon", &empty,)
            .0,
        1.0,
        "explicit empty session set must not fall back to a global SOLO bonus"
    );
}
