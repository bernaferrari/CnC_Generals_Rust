use std::collections::HashMap;

use game_engine::common::ini::INILoadType;
use game_engine::common::rts::special_power::SpecialPowerObject;
use game_engine::common::rts::{SpecialPowerStore, SpecialPowerTemplate, SpecialPowerType};

fn properties(values: &[(&str, &str)]) -> HashMap<String, String> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

struct TestObject(bool);

impl SpecialPowerObject for TestObject {
    fn is_disabled(&self) -> bool {
        false
    }

    fn get_controlling_player_index(&self) -> Option<i32> {
        None
    }

    fn has_special_power_module(&self, _template: &SpecialPowerTemplate) -> bool {
        self.0
    }
}

#[test]
fn reset_restores_base_templates_after_repeated_and_override_only_definitions() {
    let mut store = SpecialPowerStore::new();
    store
        .parse_special_power_definition(
            "DefaultSpecialPower",
            &properties(&[("Enum", "SPECIAL_DAISY_CUTTER"), ("PublicTimer", "No")]),
        )
        .unwrap();
    store
        .parse_special_power_definition(
            "BasePower",
            &properties(&[("Enum", "SPECIAL_DAISY_CUTTER"), ("PublicTimer", "No")]),
        )
        .unwrap();

    let base_id = store.find_template("BasePower").unwrap().id;
    assert_eq!(base_id, 2);

    store
        .parse_special_power_definition_with_load_type(
            "BasePower",
            &properties(&[("Enum", "SPECIAL_SCUD_STORM"), ("PublicTimer", "Yes")]),
            INILoadType::CreateOverrides,
        )
        .unwrap();
    store
        .parse_special_power_definition_with_load_type(
            "BasePower",
            &properties(&[("Enum", "SPECIAL_ARTILLERY_BARRAGE")]),
            INILoadType::CreateOverrides,
        )
        .unwrap();
    store
        .parse_special_power_definition_with_load_type(
            "MapOnlyPower",
            &properties(&[("Enum", "SPECIAL_SCUD_STORM"), ("PublicTimer", "Yes")]),
            INILoadType::CreateOverrides,
        )
        .unwrap();

    let overridden = store.find_template("BasePower").unwrap();
    assert_eq!(overridden.id, base_id);
    assert_eq!(overridden.power_type, SpecialPowerType::ArtilleryBarrage);
    // The second override inherits fields not explicitly changed from the
    // first effective definition, as C++ copies the current override chain.
    assert!(overridden.public_timer);
    assert_eq!(store.find_template("MapOnlyPower").unwrap().id, 3);
    assert_eq!(store.get_num_special_powers(), 3);

    store.reset();

    assert_eq!(store.get_num_special_powers(), 2);
    let restored = store.find_template("BasePower").unwrap();
    assert_eq!(restored.id, base_id);
    assert_eq!(restored.power_type, SpecialPowerType::DaisyCutter);
    assert!(!restored.public_timer);
    assert!(store.find_template("MapOnlyPower").is_none());
    assert!(store.find_template_by_id(3).is_none());
    assert_eq!(store.find_template("DefaultSpecialPower").unwrap().id, 1);

    let object = TestObject(true);
    assert!(store.can_use_special_power(&object, None, restored));
    let object_without_module = TestObject(false);
    assert!(!store.can_use_special_power(&object_without_module, None, restored));

    // Reset does not rewind IDs consumed by override-only INI definitions.
    store
        .parse_special_power_definition(
            "AfterReset",
            &properties(&[("Enum", "SPECIAL_SCUD_STORM")]),
        )
        .unwrap();
    assert_eq!(store.find_template("AfterReset").unwrap().id, 4);
}
