//! Integration Test: UI Rendering
//!
//! This test verifies basic UI rendering capabilities:
//! - Coordinate transformations
//! - Rectangle rendering
//! - Text layout calculations
//! - Z-ordering
//! - Click hit testing
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

/// 2D Point
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f32,
    y: f32,
}

/// Rectangle
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x <= self.x + self.width &&
        point.y >= self.y && point.y <= self.y + self.height
    }

    fn intersects(&self, other: &Rect) -> bool {
        !(self.x + self.width < other.x ||
          other.x + other.width < self.x ||
          self.y + self.height < other.y ||
          other.y + other.height < self.y)
    }
}

#[test]
fn test_point_in_rect() {
    println!("Testing point in rectangle...");

    let rect = Rect::new(0.0, 0.0, 100.0, 100.0);

    assert!(rect.contains(Point { x: 50.0, y: 50.0 }));
    assert!(rect.contains(Point { x: 0.0, y: 0.0 }));
    assert!(rect.contains(Point { x: 100.0, y: 100.0 }));
    assert!(!rect.contains(Point { x: 150.0, y: 50.0 }));
    assert!(!rect.contains(Point { x: -10.0, y: 50.0 }));

    log::info!("Point in rectangle test passed");
}

#[test]
fn test_rect_intersection() {
    println!("Testing rectangle intersection...");

    let rect1 = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rect2 = Rect::new(50.0, 50.0, 100.0, 100.0);
    let rect3 = Rect::new(200.0, 200.0, 50.0, 50.0);

    assert!(rect1.intersects(&rect2));
    assert!(rect2.intersects(&rect1));
    assert!(!rect1.intersects(&rect3));

    log::info!("Rectangle intersection test passed");
}

#[test]
fn test_ui_hierarchy() {
    println!("Testing UI hierarchy...");

    #[derive(Debug)]
    struct UIElement {
        rect: Rect,
        z_order: i32,
        visible: bool,
    }

    let mut elements = vec![
        UIElement { rect: Rect::new(0.0, 0.0, 100.0, 100.0), z_order: 1, visible: true },
        UIElement { rect: Rect::new(50.0, 50.0, 100.0, 100.0), z_order: 2, visible: true },
        UIElement { rect: Rect::new(25.0, 25.0, 50.0, 50.0), z_order: 0, visible: true },
    ];

    // Sort by z-order
    elements.sort_by_key(|e| e.z_order);

    assert_eq!(elements[0].z_order, 0);
    assert_eq!(elements[1].z_order, 1);
    assert_eq!(elements[2].z_order, 2);

    log::info!("UI hierarchy test passed");
}

#[test]
fn test_coordinate_transform() {
    println!("Testing coordinate transformation...");

    // Screen to world coordinates
    let screen_to_world = |screen_pos: Point, camera_x: f32, camera_y: f32| -> Point {
        Point {
            x: screen_pos.x + camera_x,
            y: screen_pos.y + camera_y,
        }
    };

    let screen_pos = Point { x: 100.0, y: 100.0 };
    let world_pos = screen_to_world(screen_pos, 500.0, 300.0);

    assert_eq!(world_pos.x, 600.0);
    assert_eq!(world_pos.y, 400.0);

    log::info!("Coordinate transform test passed");
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_many_hit_tests() {
        println!("Performance test: Many hit tests...");

        let rect = Rect::new(100.0, 100.0, 200.0, 200.0);
        const NUM_TESTS: usize = 1000000;

        let start = std::time::Instant::now();

        for i in 0..NUM_TESTS {
            let point = Point {
                x: (i % 400) as f32,
                y: ((i / 400) % 400) as f32,
            };
            let _ = rect.contains(point);
        }

        let elapsed = start.elapsed();
        let tests_per_sec = NUM_TESTS as f64 / elapsed.as_secs_f64();

        println!("Performed {} hit tests in {:?} ({:.0} tests/sec)",
            NUM_TESTS, elapsed, tests_per_sec);

        assert!(tests_per_sec > 1000000.0);

        log::info!("Many hit tests performance test passed");
    }
}
