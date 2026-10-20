//! Actual receiving Core algorithms against executed original C++ component rows.
//! Source provenance and arithmetic-profile limits are in the adjacent fixture receipt.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct OriginalPass {
    snap_height_bits: u32,
    lighting_calls: usize,
    accepted_raw_edits: Vec<[i32; 4]>,
}
#[derive(Deserialize)]
struct OriginalCase {
    case: String,
    profile: String,
    passes: Vec<OriginalPass>,
    final_bytes: Vec<u8>,
    lighting_total: usize,
    need_full_update: bool,
    scorch_counts: [i32; 3],
}
struct InputCase {
    name: String,
    width: i32,
    height: i32,
    border: i32,
    bytes: Vec<u8>,
    values: [f32; 6],
    kind: u8,
    small: bool,
    repeat: usize,
}
fn inputs() -> Vec<InputCase> {
    include_str!("fixtures/construction_flatten_inputs.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            assert_eq!(fields.len(), 9);
            let bytes = fields[4]
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let words: Vec<_> = fields[5]
                .split(',')
                .map(|word| f32::from_bits(u32::from_str_radix(word, 16).unwrap()))
                .collect();
            InputCase {
                name: fields[0].into(),
                width: fields[1].parse().unwrap(),
                height: fields[2].parse().unwrap(),
                border: fields[3].parse().unwrap(),
                bytes,
                values: words.try_into().unwrap(),
                kind: fields[6].parse().unwrap(),
                small: fields[7] == "1",
                repeat: fields[8].parse().unwrap(),
            }
        })
        .collect()
}
fn originals() -> Vec<OriginalCase> {
    include_str!("fixtures/construction_flatten_original.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn admit(input: &InputCase) -> TerrainLogic {
    let mut map = crate::system::map_loader::MapData::new();
    map.width = (input.width - 2 * input.border).try_into().unwrap();
    map.height = (input.height - 2 * input.border).try_into().unwrap();
    map.border_size = input.border;
    map.heightmap = input.bytes.clone();
    let mut owner = TerrainLogic::new();
    assert!(owner.load_map_geometry(map).is_empty());
    assert_eq!(
        owner.logic_height_map_extents(),
        (input.width, input.height)
    );
    assert_eq!(owner.logic_height_map_bytes(), input.bytes);
    owner
}
fn flatten(
    owner: &mut TerrainLogic,
    input: &InputCase,
    effects: &mut impl FnMut(TerrainHeightChange),
) {
    let [x, y, _, major, minor, angle] = input.values;
    // The plain geometry helper has no Object or GeometryIsSmall flag. This
    // driver admits that original no-op; real Main factory tests own that gate.
    if input.small {
        return;
    }
    if input.kind == 0 {
        owner
            .flatten_terrain_box_at_with_effects(x, y, angle, major, minor, effects)
            .unwrap();
    } else {
        assert!(matches!(input.kind, 1 | 2));
        owner
            .flatten_terrain_at_with_effects(x, y, major, effects)
            .unwrap();
    }
}
fn check_case(name: &str) {
    let mut input_rows = inputs();
    assert_eq!(input_rows.len(), 12);
    let input = input_rows.remove(input_rows.iter().position(|row| row.name == name).unwrap());
    let mut original_rows = originals();
    assert_eq!(original_rows.len(), 12);
    let original = original_rows.remove(
        original_rows
            .iter()
            .position(|row| row.case == name)
            .unwrap(),
    );
    assert_eq!(original.profile, "original_non_msvc_wwmath_nearest");
    assert_eq!(original.passes.len(), input.repeat);
    let mut owner = admit(&input);
    // Same shape/coordinates, independently owned map; actual peer calls below
    // interleave mutation/reset without selecting a global or borrowing a lock.
    let mut peer = admit(&input);
    let mut observed_bytes = input.bytes.clone();
    let mut lighting_total = 0;
    let mut need_full_update = false;
    let mut scorch_counts = [7, 11, 13];
    let mut observed_passes = Vec::new();
    for (pass_index, expected) in original.passes.iter().enumerate() {
        let peer_before = peer.logic_height_map_bytes().to_vec();
        let owner_revision = owner.height_revision();
        let mut accepted = Vec::new();
        flatten(&mut owner, &input, &mut |change| {
            let physical_x = i64::from(change.playable_x) + i64::from(input.border);
            let physical_y = i64::from(change.playable_y) + i64::from(input.border);
            let index: usize = (physical_y * i64::from(input.width) + physical_x)
                .try_into()
                .unwrap();
            let before = observed_bytes[index];
            assert!(i32::from(before) > change.requested_height);
            assert_eq!(change.raw_height, change.requested_height as u8);
            observed_bytes[index] = change.raw_height;
            accepted.push([
                (index % input.width as usize) as i32,
                (index / input.width as usize) as i32,
                i32::from(before),
                i32::from(change.raw_height),
            ]);
            lighting_total += 1;
            // An observing sink checks the original staticLightingChanged
            // effect contract; Core does not contain the native road buffer.
            need_full_update = true;
            scorch_counts = [0, 0, 0];
        });
        assert_eq!(
            accepted, expected.accepted_raw_edits,
            "{name} pass {pass_index} ordered writes"
        );
        assert_eq!(accepted.len(), expected.lighting_calls, "{name} effects");
        assert_eq!(
            owner.logic_height_map_bytes(),
            observed_bytes,
            "{name} canonical bytes"
        );
        assert_eq!(
            owner.height_revision() - owner_revision,
            accepted.len() as u64
        );
        let snap = owner.get_ground_height(input.values[0], input.values[1], None);
        assert_eq!(
            snap.to_bits(),
            expected.snap_height_bits,
            "{name} pass {pass_index} snap"
        );
        observed_passes.push(serde_json::json!({
            "snap_height_bits": snap.to_bits(),
            "lighting_calls": accepted.len(),
            "accepted_raw_edits": accepted,
        }));
        assert_eq!(
            peer.logic_height_map_bytes(),
            peer_before,
            "{name} peer unchanged"
        );
        let saved_owner_bytes = owner.logic_height_map_bytes().to_vec();
        peer.set_raw_map_height_with_effects(0, 0, 1, &mut |_| {});
        peer.reset();
        assert_eq!(owner.logic_height_map_bytes(), saved_owner_bytes);
        peer = admit(&input);
    }
    assert_eq!(
        owner.logic_height_map_bytes(),
        original.final_bytes,
        "{name} final bytes"
    );
    assert_eq!(lighting_total, original.lighting_total);
    assert_eq!(need_full_update, original.need_full_update);
    assert_eq!(scorch_counts, original.scorch_counts);
    println!(
        "CONSTRUCTION_FLATTEN_CPP {}",
        serde_json::json!({
            "case": name,
            "profile": "rust_receiving_core",
            "passes": observed_passes,
            "final_bytes": owner.logic_height_map_bytes(),
            "lighting_total": lighting_total,
            "need_full_update": need_full_update,
            "scorch_counts": scorch_counts,
        })
    );
}
macro_rules! original_case {
    ($($name:ident),+ $(,)?) => {$ (
        #[test]
        fn $name() { check_case(stringify!($name)); }
    )+};
}
original_case!(
    box_flat_unchanged,
    box_ramp,
    box_rotated_ramp,
    box_center_lower_only,
    circle_ramp,
    sphere_ramp,
    circle_center_low_no_box_clamp,
    box_border_zero,
    circle_border_with_padding,
    geometry_small_no_edit,
    box_repeat_lowering,
    circle_repeat_lowering,
);
