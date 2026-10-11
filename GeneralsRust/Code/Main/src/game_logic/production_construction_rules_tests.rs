//! CPP GlobalData364/835 and BuildAssistant444: effective authored line limits.
use super::*;
use game_engine::common::ini::ini_game_data::ProductionEnergyRules;

fn rules(limit: i32) -> ProductionEnergyRules {
    ProductionEnergyRules::default().overlay_ini_text(&format!(
        "GameData\nLowEnergyPenaltyModifier=1\nMinLowEnergyProductionSpeed=.5\nMaxLowEnergyProductionSpeed=.8\nMaxLineBuildObjects={limit}\nEnd\n"
    )).unwrap()
}

#[test]
fn construction_rules_parse_signed_limits_and_preserve_authored_energy_fields() {
    assert_eq!(ProductionEnergyRules::default().max_line_build_objects, 0);
    for limit in [50, 7, 1, 0, -3] {
        let authored = rules(limit);
        assert_eq!(authored.max_line_build_objects, limit);
        assert_eq!(
            (authored.penalty, authored.min_speed, authored.max_speed),
            (1.0, 0.5, 0.8)
        );
        let inherited = authored
            .overlay_ini_text("GameData\nMinLowEnergyProductionSpeed=.7\nEnd")
            .unwrap();
        assert_eq!(inherited.max_line_build_objects, limit);
        assert_eq!(inherited.min_speed, 0.7);
        let overridden = inherited
            .overlay_ini_text("GameData\nMaxLineBuildObjects=2\nEnd")
            .unwrap();
        assert_eq!(overridden.max_line_build_objects, 2);
        assert_eq!(overridden.min_speed, 0.7);
    }
}

#[test]
fn construction_rules_two_worlds_reset_and_failed_candidate_keep_effective_limits() {
    let mut first = GameLogic::new();
    assert_eq!(first.line_build_object_limit(), 0);
    first.admit_production_energy_base(rules(50));
    first.production_energy.effective = first
        .production_energy_baseline()
        .overlay_ini_text("GameData\nMaxLineBuildObjects=7\nEnd")
        .unwrap();
    let retained = *first.production_energy_rules();
    let mut second = GameLogic::new();
    second.admit_production_energy_base(rules(3));
    assert_eq!(first.line_build_object_limit(), 7);
    assert_eq!(second.line_build_object_limit(), 3);
    let malformed = retained
        .overlay_ini_text("GameData\nMaxLineBuildObjects=4\nMaxLineBuildObjects=invalid\nEnd");
    assert!(malformed.is_err());
    assert_eq!(*first.production_energy_rules(), retained);
    second.production_energy.effective = rules(0);
    second.reset();
    assert_eq!(second.line_build_object_limit(), 3);
    assert_eq!(first.line_build_object_limit(), 7);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut candidate = GameLogic::new();
    candidate.admit_production_energy_base(rules(-2));
    let mut malformed = builder.create_world_snapshot(&candidate).unwrap();
    malformed.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut candidate)
            .is_err()
    );
    candidate.reset();
    drop(candidate);
    assert_eq!(*first.production_energy_rules(), retained);
    first.reset();
    assert_eq!(first.line_build_object_limit(), 50);
    assert_eq!(first.production_energy_rules().min_speed, 0.5);
    assert_eq!(second.line_build_object_limit(), 3);
}
