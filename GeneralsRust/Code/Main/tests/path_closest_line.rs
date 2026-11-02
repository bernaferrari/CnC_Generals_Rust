#[path = "../src/game_logic/pathfinding/closest_line.rs"]
mod closest_line;

#[test]
fn line_visits_original_intermediate_and_final_cells() {
    // The matching original executable is tests/oracles/closest_line/oracle.py.
    let cases: &[((i32, i32), (i32, i32), &[((i32, i32), (i32, i32))])] = &[
        (
            (2, 2),
            (6, 4),
            &[
                ((2, 2), (2, 3)),
                ((2, 3), (3, 3)),
                ((3, 3), (4, 3)),
                ((4, 3), (4, 4)),
                ((4, 4), (5, 4)),
                ((5, 4), (6, 4)),
                ((6, 4), (6, 5)),
            ],
        ),
        (
            (2, 2),
            (4, 6),
            &[
                ((2, 2), (3, 2)),
                ((3, 2), (3, 3)),
                ((3, 3), (3, 4)),
                ((3, 4), (4, 4)),
                ((4, 4), (4, 5)),
                ((4, 5), (4, 6)),
                ((4, 6), (5, 6)),
            ],
        ),
        (
            (2, 2),
            (4, 4),
            &[
                ((2, 2), (2, 3)),
                ((2, 3), (3, 3)),
                ((3, 3), (3, 4)),
                ((3, 4), (4, 4)),
                ((4, 4), (4, 5)),
            ],
        ),
        (
            (6, 4),
            (2, 2),
            &[
                ((6, 4), (6, 3)),
                ((6, 3), (5, 3)),
                ((5, 3), (4, 3)),
                ((4, 3), (4, 2)),
                ((4, 2), (3, 2)),
                ((3, 2), (2, 2)),
                ((2, 2), (2, 1)),
            ],
        ),
        (
            (2, 2),
            (5, 2),
            &[((2, 2), (3, 2)), ((3, 2), (4, 2)), ((4, 2), (5, 2))],
        ),
        ((2, 2), (2, 2), &[((2, 2), (2, 3))]),
    ];
    for &(start, end, expected) in cases {
        let mut visits = Vec::new();
        closest_line::visit(start, end, |from, to| {
            visits.push((from, to));
            true
        });
        assert_eq!(visits, expected, "{start:?} to {end:?}");
    }
    let mut stopped = Vec::new();
    closest_line::visit((2, 2), (6, 4), |from, to| {
        stopped.push((from, to));
        stopped.len() < 2
    });
    assert_eq!(stopped.len(), 2);
}
