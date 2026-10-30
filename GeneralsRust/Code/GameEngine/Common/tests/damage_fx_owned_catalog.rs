use game_engine::common::ini::ini_damage_fx::{DamageFXStore, DamageType};

#[test]
fn owned_sources_are_isolated_and_redefinitions_clear_omitted_fields() {
    let mut left = DamageFXStore::new();
    let mut right = DamageFXStore::new();
    assert_eq!(left.apply_source("DamageFX Shared\n MajorFX = SMALL_ARMS LeftMajor\n MinorFX = SMALL_ARMS LeftMinor\n AmountForMajorFX = SMALL_ARMS 10\nEnd\n").unwrap(), 1);
    assert_eq!(
        right
            .apply_source("DamageFX Shared\n MajorFX = SMALL_ARMS RightMajor\nEnd\n")
            .unwrap(),
        1
    );
    let selected = |store: &DamageFXStore, amount| {
        store.find_damage_fx("Shared").unwrap().get_damage_fx_list(
            DamageType::SmallArms,
            amount,
            None,
        )
    };
    assert_eq!(selected(&left, 5.0).as_deref(), Some("LeftMinor"));
    assert_eq!(selected(&right, 5.0).as_deref(), Some("RightMajor"));
    left.apply_source("DamageFX Shared\n MajorFX = SMALL_ARMS NewMajor\nEnd\n")
        .unwrap();
    assert_eq!(selected(&left, 5.0).as_deref(), Some("NewMajor"));
    assert_eq!(selected(&right, 5.0).as_deref(), Some("RightMajor"));
    assert!(
        left.apply_source("DamageFX Shared\n MajorFX = SMALL_ARMS Uncommitted\nEnd\nDamageFX Broken\n AmountForMajorFX = SMALL_ARMS invalid\nEnd\n")
            .is_err()
    );
    assert_eq!(selected(&left, 5.0).as_deref(), Some("NewMajor"));
    assert!(left.find_damage_fx("Broken").is_none());
}
