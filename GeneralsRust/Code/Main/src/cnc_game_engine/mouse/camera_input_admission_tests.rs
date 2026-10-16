//! Exact original asymmetric middle-click predicate and unsigned frame trace.
use super::*;
#[test]
fn classifier_replays_extracted_cpp_branch_and_client_frame_wrap() {
    let mut cases = 0;
    for row in include_str!("fixtures/input_admission_cpp.csv").lines() {
        let fields: Vec<_> = row.split(',').collect();
        if fields[0] != "click" {
            continue;
        }
        let dx: i32 = fields[1].parse().unwrap();
        let dy: i32 = fields[2].parse().unwrap();
        let start: u32 = fields[3].parse().unwrap();
        let end: u32 = fields[4].parse().unwrap();
        let callbacks: u32 = fields[5].parse().unwrap();
        assert_eq!(
            lookat_mmb_is_short_click(dx as f32, dy as f32, end.wrapping_sub(start)),
            callbacks == 2,
            "{row}"
        );
        cases += 1;
    }
    assert_eq!(cases, 8);
}
