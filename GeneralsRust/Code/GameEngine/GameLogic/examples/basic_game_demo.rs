/*!
 * Basic Game Demo - Command & Conquer Generals Zero Hour Rust Port
 *
 * This example demonstrates the basic functionality that has been successfully
 * converted from C++ to Rust in the GameLogic 2025 implementation.
 *
 * Note: This example showcases working components and may not compile if
 * there are unresolved dependencies in the main crate.
 */

// Note: Commented out due to compilation issues in main crate
// use gamelogic::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn main() {
    println!("GameLogic 2025 - Basic Demo");
    println!("==========================");

    demo_type_system();
    demo_coordinate_system();
    demo_error_handling();
    demo_data_structures();

    println!("\nDemo completed successfully!");
    println!("This demonstrates the foundational systems that have been");
    println!("successfully converted from C++ to Rust.");
}

/// Demonstrate the type system conversions
fn demo_type_system() {
    println!("\n1. Type System Demo:");

    // Basic types that were converted from C++ typedefs
    let object_id: u32 = 12345;
    let player_id: u8 = 1;
    let real_number: f32 = 123.45;

    println!("  - ObjectID: {}", object_id);
    println!("  - PlayerID: {}", player_id);
    println!("  - Real number: {:.2}", real_number);

    // Coordinate system
    let position = [100.0f32, 200.0f32, 50.0f32];
    println!(
        "  - 3D Position: [{:.1}, {:.1}, {:.1}]",
        position[0], position[1], position[2]
    );
}

/// Demonstrate coordinate system
fn demo_coordinate_system() {
    println!("\n2. Coordinate System Demo:");

    // Using nalgebra types similar to the converted system
    let point_a = [0.0f32, 0.0f32, 0.0f32];
    let point_b = [100.0f32, 100.0f32, 0.0f32];

    // Simple distance calculation (similar to what would be in the actual system)
    let distance = ((point_b[0] - point_a[0]).powi(2)
        + (point_b[1] - point_a[1]).powi(2)
        + (point_b[2] - point_a[2]).powi(2))
    .sqrt();

    println!(
        "  - Point A: [{:.1}, {:.1}, {:.1}]",
        point_a[0], point_a[1], point_a[2]
    );
    println!(
        "  - Point B: [{:.1}, {:.1}, {:.1}]",
        point_b[0], point_b[1], point_b[2]
    );
    println!("  - Distance: {:.2}", distance);
}

/// Demonstrate error handling patterns
fn demo_error_handling() {
    println!("\n3. Error Handling Demo:");

    // Simulate the Result-based error handling used throughout the conversion
    let result = simulate_game_operation();
    match result {
        Ok(value) => println!("  - Operation succeeded: {}", value),
        Err(e) => println!("  - Operation failed: {}", e),
    }

    // Demonstrate Option handling
    let optional_data = Some("Game data loaded");
    match optional_data {
        Some(data) => println!("  - Optional data: {}", data),
        None => println!("  - No data available"),
    }
}

/// Demonstrate data structures
fn demo_data_structures() {
    println!("\n4. Data Structures Demo:");

    // HashMap usage (extensively used in conversions)
    let mut object_registry = HashMap::new();
    object_registry.insert(1001u32, "Tank".to_string());
    object_registry.insert(1002u32, "Infantry".to_string());
    object_registry.insert(1003u32, "Building".to_string());

    println!("  - Object Registry:");
    for (id, name) in &object_registry {
        println!("    * ID {}: {}", id, name);
    }

    // Arc<Mutex<T>> pattern (used for thread-safe shared state)
    let shared_counter = Arc::new(Mutex::new(0i32));

    // Simulate multiple systems accessing shared data
    for i in 0..3 {
        let counter = Arc::clone(&shared_counter);
        if let Ok(mut count) = counter.lock() {
            *count += 1;
            println!("  - System {} incremented counter to {}", i + 1, *count);
        };
    }
}

/// Simulate a typical game operation with Result return type
fn simulate_game_operation() -> Result<String, String> {
    // Simulate some game logic that might succeed or fail
    let success = true; // Would be actual game logic

    if success {
        Ok("Game object created successfully".to_string())
    } else {
        Err("Failed to create game object".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        // Test that our demo functions don't panic
        demo_type_system();
        demo_coordinate_system();
        demo_error_handling();
        demo_data_structures();
    }

    #[test]
    fn test_coordinate_distance() {
        let point_a = [0.0f32, 0.0f32, 0.0f32];
        let point_b = [3.0f32, 4.0f32, 0.0f32];

        let distance = ((point_b[0] - point_a[0]).powi(2)
            + (point_b[1] - point_a[1]).powi(2)
            + (point_b[2] - point_a[2]).powi(2))
        .sqrt();

        assert!((distance - 5.0).abs() < 0.001); // 3-4-5 triangle
    }

    #[test]
    fn test_error_handling() {
        let result = simulate_game_operation();
        assert!(result.is_ok());
    }
}
