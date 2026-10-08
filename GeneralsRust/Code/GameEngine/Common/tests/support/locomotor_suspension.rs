//! Shared input fixture only: callers independently choose the production boundary.
use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_locomotor::{
    LocomotorTemplate, get_locomotor_store, load_locomotors_from_str,
};

const ORIGINAL: &str = include_str!("../fixtures/suspension_original.txt");
const FIELDS: [&str; 5] = [
    "PitchStiffness",
    "RollStiffness",
    "PitchDamping",
    "RollDamping",
    "UniformAxialDamping",
];

pub fn check(prefix: &str, group: &str, read: impl Fn(&LocomotorTemplate) -> [f32; 5]) {
    let mut differences = Vec::new();
    let mut count = 0;
    for row in ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
    {
        if !row[0].starts_with(group) {
            continue;
        }
        assert_eq!(row.len(), 11);
        for bulk in [false, true] {
            let name = format!("Suspension{prefix}_{}_{bulk}", row[0]);
            let body = FIELDS
                .iter()
                .zip(&row[1..6])
                .filter(|(_, token)| **token != "omitted")
                .map(|(field, token)| format!("{field} = {token}\n"))
                .collect::<String>();
            let text = format!(
                "Locomotor {name}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\nAppearance = OTHER\n{body}End\n"
            );
            if bulk {
                assert_eq!(load_locomotors_from_str(&text).unwrap(), 1);
            } else {
                INI::new()
                    .with_inline_source(&text, |ini| ini.parse_current_file())
                    .unwrap();
            }
            let template = get_locomotor_store().find_template(&name).unwrap().clone();
            let actual = read(&template);
            for i in 0..5 {
                let expected = u32::from_str_radix(row[i + 6], 16).unwrap();
                count += 1;
                if actual[i].to_bits() != expected {
                    differences.push(format!(
                        "{} bulk={bulk} {}: {:08x} != {expected:08x}",
                        row[0],
                        FIELDS[i],
                        actual[i].to_bits()
                    ));
                }
            }
        }
    }
    let expected_count = match group {
        "omitted" => 10,
        "zero" => 120,
        "control" => 20,
        _ => panic!("unknown group"),
    };
    assert_eq!(count, expected_count);
    assert!(
        differences.is_empty(),
        "{prefix} {count} field comparisons: {differences:?}"
    );
}

pub fn common(t: &LocomotorTemplate) -> [f32; 5] {
    [
        t.pitch_stiffness,
        t.roll_stiffness,
        t.pitch_damping,
        t.roll_damping,
        t.uniform_axial_damping,
    ]
}
