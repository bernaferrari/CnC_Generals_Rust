use super::*;

fn experienced_stats() -> AcademyStats {
    let mut s = AcademyStats::new();
    s.unknown_side = false;
    s.base_side = "USA".into();
    s.spent_cash_before_building_supply_center = false;
    s.researched_radar = true;
    s.peons_built = 2;
    s.structures_captured = 1;
    s.generals_points_spent = 1;
    s.special_powers_used = 1;
    s.structures_garrisoned = 1;
    s.last_unit_built_frame = 100;
    s.drag_select_units = 1;
    s.upgrades_purchased = 1;
    s.had_power_last_check = true;
    s.gatherers_built = 1;
    s.heroes_built = 1;
    s.control_groups_used = 1;
    s.secondary_income_units_built = 1;
    s.cleared_garrisoned_buildings = 1;
    s.guard_ability_used_count = 1;
    s.supply_centers_built = 2;
    s.double_click_attack_move_orders_given = 1;
    s.built_barracks_within_five_minutes = true;
    s.built_war_factory_within_ten_minutes = true;
    s.built_tech_structure_within_fifteen_minutes = true;
    s.last_income_frame = 100;
    s
}

#[test]
fn advice32_uses_receiving_neutral_and_recovered_counts() {
    let mut s = experienced_stats();
    let mut info = AcademyAdviceInfo::default();
    let context = AcademyAdviceContext {
        frame: 100,
        use_alternate_mouse: true,
        neutral_vehicles_sniped: 1,
        ..Default::default()
    };
    assert!(s.calculate_academy_advice_with_context(&mut info, context));
    assert_eq!(info.advice[0], "ACADEMY:UnmannedVehicles");
    assert!(!s.calculate_academy_advice_with_context(
        &mut info,
        AcademyAdviceContext {
            neutral_vehicles_sniped: 0,
            ..context
        }
    ));
    s.vehicles_recovered = 1;
    assert!(!s.calculate_academy_advice_with_context(&mut info, context));
}

#[test]
fn advice31_and_income_deadline_use_the_receiving_context() {
    let s = experienced_stats();
    let mut info = AcademyAdviceInfo::default();
    let context = AcademyAdviceContext {
        frame: 100,
        use_alternate_mouse: true,
        ..Default::default()
    };
    assert!(!s.calculate_academy_advice_with_context(&mut info, context));
    assert!(s.calculate_academy_advice_with_context(
        &mut info,
        AcademyAdviceContext {
            local_player_mines: 1,
            ..context
        }
    ));
    assert_eq!(info.advice[0], "ACADEMY:NoIncome");
    assert!(s.calculate_academy_advice_with_context(
        &mut info,
        AcademyAdviceContext {
            frame: 3701,
            ..context
        }
    ));
    assert_eq!(info.advice[0], "ACADEMY:NoIncome");
    assert!(!s.calculate_academy_advice_with_context(&mut info, context));
}
