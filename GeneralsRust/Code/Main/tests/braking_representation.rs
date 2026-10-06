//! Public loading, effective-braking binding/getter, and scalar persistence.
//! `oracles/generate_braking.py` extracts pinned original C++ arithmetic.
//! No world tick, movement simulation, RNG assertions, or retail assets.

use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_locomotor::{
    LocomotorTemplate as CommonTemplate, get_locomotor_store, load_locomotors_from_str,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use gamelogic::locomotor::{core::Locomotor, ini_bridge::from_common_ini_template};
use generals_main::game_logic::locomotor_bootstrap::{
    apply_host_locomotor_binding, resolve_host_locomotor_binding,
};
use generals_main::game_logic::object::{Object, ObjectType};
use generals_main::game_logic::{ObjectId, Team, ThingTemplate};
use std::{io::Cursor, sync::Arc};

const ORIGINAL: &str = include_str!("fixtures/braking_original.txt");
// C++ Locomotor.cpp:722-749: v2, donutTimer, maintainPos[3], brakingFactor,
// maxLift, maxSpeed, maxAccel, then maxBraking. XferVersion is one byte.
const CAP_OFFSET_V2: usize = 1 + 4 + 3 * 4 + 4 * 4;
const XFER_LENGTH_V2: usize = 65;

fn bits(text: &str) -> u32 {
    u32::from_str_radix(text, 16).unwrap()
}

fn rows(kind: &str) -> Vec<Vec<&str>> {
    ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|row| row.first() == Some(&kind))
        .collect()
}

fn load(prefix: &str, index: usize, token: &str, bulk: bool) -> CommonTemplate {
    let name = format!("Braking{prefix}{index}_{bulk}");
    let field = if token == "omitted" {
        String::new()
    } else {
        format!("Braking = {token}\n")
    };
    // These authored fields make the real Main resolver accept the template.
    let text =
        format!("Locomotor {name}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\n{field}End\n");
    if bulk {
        assert_eq!(load_locomotors_from_str(&text).unwrap(), 1);
    } else {
        INI::new()
            .with_inline_source(&text, |ini| ini.parse_current_file())
            .unwrap();
    }
    get_locomotor_store().find_template(&name).unwrap().clone()
}

fn object() -> Object {
    Object::new(
        ThingTemplate::new("BrakingFixture"),
        ObjectId(801),
        Team::USA,
    )
}

fn save(loco: &mut Locomotor) -> Vec<u8> {
    let mut bytes = Vec::new();
    loco.loco_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    assert_eq!(bytes.len(), XFER_LENGTH_V2);
    bytes
}

fn saved_cap(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[CAP_OFFSET_V2..CAP_OFFSET_V2 + 4].try_into().unwrap())
}

fn assert_loader_column(prefix: &str, column: usize, read: impl Fn(&CommonTemplate) -> f32) {
    let mut differences = Vec::new();
    let mut count = 0;
    for (index, row) in rows("load").iter().enumerate() {
        for bulk in [false, true] {
            let template = load(prefix, index, row[1], bulk);
            let actual = read(&template).to_bits();
            let expected = bits(row[column]);
            count += 1;
            if actual != expected {
                differences.push(format!(
                    "{} bulk={bulk}: {actual:08x} != {expected:08x}",
                    row[1]
                ));
            }
        }
    }
    assert_eq!(count, 28);
    assert!(differences.is_empty(), "{count} cases: {differences:?}");
}

#[test]
fn public_loaders_preserve_omitted_authored_zero_negative_and_high_frame2_values() {
    assert_loader_column("Common", 2, |t| t.braking);
}

#[test]
fn bridge_preserves_original_frame2_values_without_zero_repair() {
    assert_loader_column("Bridge", 2, |t| from_common_ini_template(t).braking);
}

#[test]
fn gamelogic_getter_uses_original_effective_frame2_braking() {
    assert_loader_column("Getter", 4, |t| {
        Locomotor::new(Arc::new(from_common_ini_template(t))).get_braking()
    });
}

#[test]
fn new_gamelogic_cap_is_independent_of_authored_template_and_saved_in_frame2() {
    assert_loader_column("Cap", 3, |t| {
        let mut loco = Locomotor::new(Arc::new(from_common_ini_template(t)));
        f32::from_bits(saved_cap(&save(&mut loco)))
    });
}

#[test]
fn ordered_getter_comparison_preserves_nan_and_signed_zero_with_restored_caps() {
    let controls = rows("cap");
    assert_eq!(controls.len(), 8);
    for row in controls {
        // Special IEEE inputs test the getter, not INI token acceptance.
        let mut template =
            gamelogic::locomotor::core::LocomotorTemplate::new("IeeeControl".to_owned());
        template.braking = f32::from_bits(bits(row[1]));
        let mut loco = Locomotor::new(Arc::new(template));
        let mut snapshot = save(&mut loco);
        // Authored snapshot input exercises the existing public load boundary;
        // the cap is private and the Rust port has no setMaxBraking equivalent.
        snapshot[CAP_OFFSET_V2..CAP_OFFSET_V2 + 4].copy_from_slice(&bits(row[2]).to_le_bytes());
        loco.loco_xfer(&mut XferLoad::new(Cursor::new(&snapshot), 1))
            .unwrap();
        assert_eq!(loco.get_braking().to_bits(), bits(row[3]), "{row:?}");
        let mut cloned = loco.clone();
        assert_eq!(cloned.get_braking().to_bits(), bits(row[3]));
        assert_eq!(
            save(&mut cloned),
            snapshot,
            "clone retains the cap and snapshot state"
        );
    }
}

#[test]
fn clone_and_v1_v2_xfer_keep_cap_independent_of_destination_template() {
    let source = load("CloneSource", 0, "100", true);
    let high = load("CloneHigh", 0, "90000000", true);
    let mut loco = Locomotor::new(Arc::new(from_common_ini_template(&source)));
    let mut clone = loco.clone();
    let snapshot = save(&mut loco);
    assert_eq!(snapshot, save(&mut clone));
    assert_eq!(saved_cap(&snapshot), bits(rows("load")[0][3]));
    for version in [1, 2] {
        let mut input = snapshot.clone();
        if version == 1 {
            input[0] = 1;
            input.drain(1..5); // Original v1 has no donutTimer.
        }
        let mut restored = Locomotor::new(Arc::new(from_common_ini_template(&high)));
        restored
            .loco_xfer(&mut XferLoad::new(Cursor::new(input), 1))
            .unwrap();
        assert_eq!(restored.get_braking().to_bits(), bits(rows("load")[0][4]));
        assert_eq!(saved_cap(&save(&mut restored)), saved_cap(&snapshot));
        assert_eq!(restored.get_template_name(), high.name.as_str());
    }
}

#[test]
fn main_public_binding_converts_effective_frame2_to_sec2_and_applies_it() {
    assert_loader_column("Main", 5, |t| {
        let binding = resolve_host_locomotor_binding(t.name.as_str()).unwrap();
        let mut host = object();
        apply_host_locomotor_binding(&mut host, &binding);
        assert_eq!(host.braking.to_bits(), binding.braking.to_bits());
        assert_eq!(host.clone().braking.to_bits(), binding.braking.to_bits());
        host.braking
    });
}

#[test]
fn main_constructor_defaults_are_original_bignum_in_sec2() {
    let expected = bits(rows("load")[0][5]);
    assert_eq!(object().braking.to_bits(), expected);
    assert_eq!(
        Object::new_simple(ObjectId(802), ObjectType::Vehicle, "BrakingSimple".into())
            .braking
            .to_bits(),
        expected
    );
}

#[test]
fn main_serde_missing_braking_uses_sec2_default() {
    let mut json = serde_json::to_value(object()).unwrap();
    json.as_object_mut().unwrap().remove("braking");
    let restored: Object = serde_json::from_value(json).unwrap();
    assert_eq!(restored.braking.to_bits(), bits(rows("load")[0][5]));
}

#[test]
fn main_serde_preserves_explicit_saved_scalars_without_migration() {
    for scalar in [0.0_f32, -0.0, -100.0, 100.0, 99999.0, 89_999_100.0] {
        // Explicit historical serialized values are literal inputs. In particular
        // 99999 cannot be identified as an old default by magnitude alone.
        let mut json = serde_json::to_value(object()).unwrap();
        json["braking"] = serde_json::to_value(scalar).unwrap();
        let source: Object = serde_json::from_value(json).unwrap();
        assert_eq!(source.braking.to_bits(), scalar.to_bits());
        assert_eq!(source.clone().braking.to_bits(), scalar.to_bits());
        let serialized = serde_json::to_string(&source).unwrap();
        let restored: Object = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.braking.to_bits(), scalar.to_bits());
    }
}
