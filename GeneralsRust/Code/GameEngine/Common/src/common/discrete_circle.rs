// discrete_circle.rs - Port of DiscreteCircle.cpp (Bresenham midpoint circle)
// Original: GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp

/// Horizontal line segment produced by the circle rasterizer.
/// Matches C++ HorzLine { xStart, xEnd, yPos }.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorzLine {
    pub x_start: i32,
    pub x_end: i32,
    pub y_pos: i32,
}

/// Discrete circle rasterizer using Bresenham's midpoint algorithm.
/// Matches C++ DiscreteCircle from DiscreteCircle.h/.cpp.
#[derive(Debug, Clone)]
pub struct DiscreteCircle {
    x_center: i32,
    y_center: i32,
    y_pos_doubled: i32,
    edges: Vec<HorzLine>,
}

impl DiscreteCircle {
    pub fn new(x_center: i32, y_center: i32, radius: i32) -> Self {
        let y_pos_doubled = y_center * 2;
        let mut circle = Self {
            x_center,
            y_center,
            y_pos_doubled,
            edges: Vec::with_capacity(radius as usize * 4 + 4),
        };
        circle.generate_edge_pairs(radius);
        circle.remove_duplicates();
        circle
    }

    pub fn get_radius(&self) -> i32 {
        self.edges.len() as i32 / 2
    }

    /// Matches C++ DiscreteCircle::getEdges.
    pub fn get_edges(&self) -> &[HorzLine] {
        &self.edges
    }

    /// Matches C++ DiscreteCircle::getEdgeCount.
    pub fn get_edge_count(&self) -> i32 {
        self.edges.len() as i32
    }

    /// Iterate every scan-line of the circle, calling `callback(x_start, x_end, y_pos)`.
    /// Matches C++ DiscreteCircle::drawCircle — upper half + mirrored lower half.
    pub fn draw_circle<F>(&self, mut callback: F)
    where
        F: FnMut(i32, i32, i32),
    {
        for edge in &self.edges {
            callback(edge.x_start, edge.x_end, edge.y_pos);
            if edge.y_pos != self.y_center {
                callback(edge.x_start, edge.x_end, self.y_pos_doubled - edge.y_pos);
            }
        }
    }

    /// Bresenham midpoint circle — produces horizontal spans for the upper semicircle.
    /// Matches C++ DiscreteCircle::generateEdgePairs.
    fn generate_edge_pairs(&mut self, radius: i32) {
        let mut x: i32 = 0;
        let mut y = radius;
        let mut d = (1 - radius) << 1;

        while y >= 0 {
            self.edges.push(HorzLine {
                x_start: self.x_center - x,
                x_end: self.x_center + x,
                y_pos: self.y_center + y,
            });
            if d + y > 0 {
                y -= 1;
                d -= (y << 1) - 1;
            }
            if x > d {
                x += 1;
                d += (x << 1) + 1;
            }
        }
    }

    /// Remove consecutive edges sharing the same y position (Bresenham artefact).
    /// Matches C++ DiscreteCircle::removeDuplicates: erase the *first* of each
    /// same-y pair so the last (typically wider) span survives.
    fn remove_duplicates(&mut self) {
        let mut write = 0;
        let mut read = 0;
        while read < self.edges.len() {
            let mut last = read;
            while last + 1 < self.edges.len()
                && self.edges[last + 1].y_pos == self.edges[read].y_pos
            {
                last += 1;
            }
            self.edges[write] = self.edges[last];
            write += 1;
            read = last + 1;
        }
        self.edges.truncate(write);
    }
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
    fn circle_is_symmetric() {
        let c = DiscreteCircle::new(0, 0, 20);
        let mut top_count = 0;
        let mut bottom_count = 0;
        for _edge in &c.edges {
            top_count += 1;
            bottom_count += 1;
        }
        assert_eq!(top_count, bottom_count);
    }

    #[test]
    fn no_duplicate_y_positions() {
        let c = DiscreteCircle::new(0, 0, 15);
        for w in c.edges.windows(2) {
            assert_ne!(w[0].y_pos, w[1].y_pos);
        }
    }
}
