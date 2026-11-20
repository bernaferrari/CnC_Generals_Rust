// discrete_circle.rs - Port of DiscreteCircle.cpp (Bresenham midpoint circle)
// Original: GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp

/// An inclusive horizontal span produced by the circle rasterizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorzLine {
    /// Inclusive left endpoint.
    pub x_start: i32,
    /// Inclusive right endpoint.
    pub x_end: i32,
    /// Scanline coordinate.
    pub y_pos: i32,
}

/// Discrete circle rasterizer using Bresenham's midpoint algorithm.
/// Matches C++ DiscreteCircle from DiscreteCircle.h/.cpp.
#[derive(Debug, Clone)]
pub struct DiscreteCircle {
    y_center: i32,
    y_pos_doubled: i32,
    edges: Vec<HorzLine>,
}

impl DiscreteCircle {
    /// Constructs the upper-half spans for repeated use.
    ///
    /// Radius must be nonnegative and intermediate coordinates must fit in a
    /// signed 32-bit integer.
    ///
    /// # Panics
    ///
    /// Panics if the radius is negative.
    pub fn new(x_center: i32, y_center: i32, radius: i32) -> Self {
        let y_pos_doubled = y_center * 2;
        let rows = upper_spans(x_center, y_center, radius);
        let mut edges = Vec::with_capacity(radius as usize + 1);
        edges.extend(rows);
        Self {
            y_center,
            y_pos_doubled,
            edges,
        }
    }

    /// Returns upper-half spans in descending scanline order, including the center.
    pub fn get_edges(&self) -> &[HorzLine] {
        &self.edges
    }

    /// Returns the number of upper-half spans.
    pub fn get_edge_count(&self) -> i32 {
        self.edges.len() as i32
    }

    /// Visits each span, then its reflected span, and visits the center once.
    pub fn draw_circle(&self, callback: impl FnMut(i32, i32, i32)) {
        draw_spans(
            self.edges.iter().copied(),
            self.y_center,
            self.y_pos_doubled,
            callback,
        );
    }

    /// Visits the original ordered scanlines without allocating or retaining spans.
    ///
    /// Radius and coordinate requirements are the same as for [Self::new].
    pub fn draw_circle_at(
        x_center: i32,
        y_center: i32,
        radius: i32,
        callback: impl FnMut(i32, i32, i32),
    ) {
        draw_spans(
            upper_spans(x_center, y_center, radius),
            y_center,
            y_center << 1,
            callback,
        );
    }
}

fn draw_spans(
    edges: impl Iterator<Item = HorzLine>,
    y_center: i32,
    doubled_y: i32,
    mut callback: impl FnMut(i32, i32, i32),
) {
    for row in edges {
        callback(row.x_start, row.x_end, row.y_pos);
        if row.y_pos != y_center {
            callback(row.x_start, row.x_end, doubled_y - row.y_pos);
        }
    }
}

/// The final Bresenham span at each height is the one retained by
/// DiscreteCircle.cpp when consecutive spans share the same scanline.
fn upper_spans(x_center: i32, y_center: i32, radius: i32) -> impl Iterator<Item = HorzLine> {
    assert!(radius >= 0, "circle radius must be nonnegative");
    let mut x = 0;
    let mut y = radius;
    let mut d = (1 - y) << 1;
    std::iter::from_fn(move || {
        if y < 0 {
            return None;
        }
        let row = y;
        loop {
            let half_width = x;
            if d + y > 0 {
                y -= 1;
                d -= (y << 1) - 1;
            }
            if x > d {
                x += 1;
                d += (x << 1) + 1;
            }
            if y != row {
                return Some(HorzLine {
                    x_start: x_center - half_width,
                    x_end: x_center + half_width,
                    y_pos: y_center + row,
                });
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_produces_edges() {
        let c = DiscreteCircle::new(50, 50, 10);
        assert!(!c.edges.is_empty());
    }

    #[test]
    fn circle_draw_invokes_callback() {
        let c = DiscreteCircle::new(0, 0, 5);
        let mut count = 0;
        c.draw_circle(|_xs, _xe, _y| count += 1);
        assert!(count > 0);
    }

    #[test]
    fn center_scanline_is_drawn_once() {
        let c = DiscreteCircle::new(0, 0, 5);
        let mut center_rows = 0;
        c.draw_circle(|_xs, _xe, y| {
            if y == 0 {
                center_rows += 1;
            }
        });
        assert_eq!(center_rows, 1);
    }

    #[test]
    fn radius_zero_draws_one_scanline() {
        let c = DiscreteCircle::new(0, 0, 0);
        let mut rows = Vec::new();
        c.draw_circle(|xs, xe, y| rows.push((xs, xe, y)));
        assert_eq!(rows, vec![(0, 0, 0)]);
    }

    #[test]
    fn radius_one_draws_upper_lower_then_center() {
        let circle = DiscreteCircle::new(10, 20, 1);
        let mut rows = Vec::new();
        circle.draw_circle(|start, end, y| rows.push((start, end, y)));
        assert_eq!(rows, vec![(10, 10, 21), (10, 10, 19), (10, 10, 20)]);
        assert_eq!(circle.get_edge_count(), 2);
    }

    #[test]
    fn direct_spans_match_the_stored_circle_in_source_order() {
        for (x, y) in [(-6, -12), (7, 0), (30, 3)] {
            for radius in [0, 1, 3, 8, 14, 32, 100] {
                let mut expected = Vec::new();
                DiscreteCircle::new(x, y, radius)
                    .draw_circle(|start, end, row| expected.push((start, end, row)));
                let mut spans = expected.into_iter();
                DiscreteCircle::draw_circle_at(x, y, radius, |start, end, row| {
                    assert_eq!(spans.next(), Some((start, end, row)));
                });
                assert_eq!(spans.next(), None);
            }
        }
    }

    #[test]
    #[should_panic(expected = "circle radius must be nonnegative")]
    fn negative_radius_is_rejected_before_allocation() {
        DiscreteCircle::new(0, 0, -1);
    }

    #[test]
    fn remove_duplicates_keeps_last_same_y_span() {
        // C++ erase(it) drops the first of each consecutive same-y pair, so
        // radius 5's top row is the last Bresenham span (-2..2), not (0..0).
        let c = DiscreteCircle::new(0, 0, 5);
        assert_eq!(
            c.get_edges(),
            &[
                HorzLine {
                    x_start: -2,
                    x_end: 2,
                    y_pos: 5
                },
                HorzLine {
                    x_start: -3,
                    x_end: 3,
                    y_pos: 4
                },
                HorzLine {
                    x_start: -4,
                    x_end: 4,
                    y_pos: 3
                },
                HorzLine {
                    x_start: -5,
                    x_end: 5,
                    y_pos: 2
                },
                HorzLine {
                    x_start: -5,
                    x_end: 5,
                    y_pos: 1
                },
                HorzLine {
                    x_start: -5,
                    x_end: 5,
                    y_pos: 0
                },
            ]
        );
        let mut rows = Vec::new();
        c.draw_circle(|xs, xe, y| rows.push((xs, xe, y)));
        assert_eq!(
            rows,
            vec![
                (-2, 2, 5),
                (-2, 2, -5),
                (-3, 3, 4),
                (-3, 3, -4),
                (-4, 4, 3),
                (-4, 4, -3),
                (-5, 5, 2),
                (-5, 5, -2),
                (-5, 5, 1),
                (-5, 5, -1),
                (-5, 5, 0),
            ]
        );
        assert_eq!(c.get_edge_count(), 6);
    }

    #[test]
    fn offset_center_mirrors_about_y_center() {
        let c = DiscreteCircle::new(10, 20, 8);
        let mut rows = Vec::new();
        c.draw_circle(|xs, xe, y| rows.push((xs, xe, y)));
        assert_eq!(
            rows,
            vec![
                (8, 12, 28),
                (8, 12, 12),
                (6, 14, 27),
                (6, 14, 13),
                (5, 15, 26),
                (5, 15, 14),
                (4, 16, 25),
                (4, 16, 15),
                (3, 17, 24),
                (3, 17, 16),
                (2, 18, 23),
                (2, 18, 17),
                (2, 18, 22),
                (2, 18, 18),
                (2, 18, 21),
                (2, 18, 19),
                (2, 18, 20),
            ]
        );
    }

    #[test]
    fn no_duplicate_y_positions() {
        let c = DiscreteCircle::new(0, 0, 15);
        for w in c.edges.windows(2) {
            assert_ne!(w[0].y_pos, w[1].y_pos);
        }
    }
}
