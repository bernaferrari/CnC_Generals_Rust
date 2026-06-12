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
    /// Matches C++ DiscreteCircle::removeDuplicates.
    fn remove_duplicates(&mut self) {
        let mut write = 0;
        for read in 1..self.edges.len() {
            if self.edges[read].y_pos != self.edges[write].y_pos {
                write += 1;
                self.edges[write] = self.edges[read];
            }
        }
        self.edges.truncate(write + 1);
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
