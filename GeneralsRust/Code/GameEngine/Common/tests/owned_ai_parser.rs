use game_engine::common::ini::{INI, INILoadType, ini_ai_data::AIDataStore};
use game_engine::common::rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore};
use std::sync::Arc;

fn parse(id: i32, cost: i32, scan_msec: u32) -> AIDataStore {
    let mut sciences = ScienceStore::new();
    let mut science = ScienceInfo::new(id, "SCIENCE_Shared");
    science.science_purchase_point_cost = cost;
    sciences.add_science(science);
    let mut ini = INI::new();
    ini.set_receiving_science_names(Arc::new(ScienceNameFacts::from_store(&sciences)));
    ini.set_owned_ai_data_store_target(AIDataStore::default());
    let source = format!(
        "AIData\n GuardEnemyScanRate = {scan_msec}\n SideInfo = America\n SkillSet1\n Science = SCIENCE_Shared\n End\n End\nEnd\n"
    );
    ini.with_inline_source_and_load_type(&source, INILoadType::Overwrite, |ini| {
        ini.parse_current_file()
    })
    .unwrap();
    ini.take_owned_ai_data_store_target().unwrap()
}

#[test]
fn independent_drafts_use_their_own_science_catalog() {
    let first = parse(71, 1, 1000);
    let peer = parse(92, 0, 2000);
    let first_data = first.get_active().unwrap();
    let peer_data = peer.get_active().unwrap();
    assert_eq!(first_data.guard_enemy_scan_rate, 30);
    assert_eq!(peer_data.guard_enemy_scan_rate, 60);
    assert_eq!(first_data.side_info[0].skill_set_1.num_skills, 1);
    assert_eq!(first_data.side_info[0].skill_set_1.skills[0], 71);
    assert_eq!(peer_data.side_info[0].skill_set_1.num_skills, 0);
}
