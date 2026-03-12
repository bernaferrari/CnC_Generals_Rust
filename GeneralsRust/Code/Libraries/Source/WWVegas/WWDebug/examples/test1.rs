//! Test 1 - Basic profiling functionality test
//!
//! This is a Rust conversion of test1.cpp from the original C++ profile library.
//! It demonstrates basic usage of the profiling system including:
//! - High-level profiling blocks
//! - Profile enumeration and result display
//! - Timer-based profiling

use profile::{Profile, ProfileHighLevel};
use std::sync::atomic::{AtomicI32, Ordering};

// Global counter equivalent to C++ 'int q'
static Q: AtomicI32 = AtomicI32::new(0);

fn calc_this() {
    Q.fetch_add(1, Ordering::Relaxed);
}

fn calc_that() {
    calc_this();
    Q.fetch_sub(1, Ordering::Relaxed);
}

fn recursion(level: i32) {
    Q.fetch_add(level, Ordering::Relaxed);
    if level < 5000 {
        recursion2(level + 1);
    }
}

fn recursion2(level: i32) {
    recursion(level);
}

fn recursion_shell() {
    // Create a high-level profiling block - equivalent to C++ ProfileHighLevel::Block b("Test block")
    let _block = ProfileHighLevel::block("Test block").expect("Failed to create profiling block");
    recursion(0);
}

fn show_results() {
    println!("Profile Results:");
    println!("{:<16} {:<10} {}", "Name", "Value", "Unit");
    println!("{:-<40}", "");

    let high_level = Profile::high_level();
    let mut index = 0;

    // Enumerate all profiles - equivalent to C++ ProfileHighLevel::EnumProfile
    while let Some(id) = high_level.enum_profile(index) {
        let name = id.get_name();
        let total_value = id.get_total_value();
        let unit = id.get_unit();

        println!("{:<16} {:<10} {}", name, total_value, unit);
        index += 1;
    }

    // Also show frame information
    let frame_count = Profile::get_frame_count();
    println!("\nRecorded {} frames", frame_count);

    for frame in 0..frame_count {
        if let Some(frame_name) = Profile::get_frame_name(frame) {
            println!("Frame {}: {}", frame, frame_name);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Profile Test 1 - Basic functionality");
    println!("===================================");

    // Initialize profiling patterns - enable all patterns
    Profile::add_pattern("*", true)?;

    // Start a profiling range for the main test
    Profile::start_range(Some("main_test"))?;

    // Test basic function calls with profiling
    for k in 0..100 {
        if k % 2 != 0 && k > 80 {
            calc_that();
        } else {
            calc_this();
        }
    }

    // Test recursive function with high-level profiling block
    recursion_shell();

    // Stop the profiling range
    Profile::stop_range(Some("main_test"))?;

    // Show the results
    show_results();

    // Display final counter value
    println!("\nFinal Q value: {}", Q.load(Ordering::Relaxed));

    // Generate profiling results
    Profile::write_results();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functions() {
        let initial = Q.load(Ordering::Relaxed);
        calc_this();
        assert_eq!(Q.load(Ordering::Relaxed), initial + 1);

        calc_that(); // Should add 1 then subtract 1
        assert_eq!(Q.load(Ordering::Relaxed), initial + 1);
    }

    #[test]
    fn test_profiling_block() {
        // Test that we can create profiling blocks without panicking
        let result = std::panic::catch_unwind(|| {
            let _block = ProfileHighLevel::block("test_block");
            // Block should drop automatically here
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_range_profiling() {
        let result = Profile::start_range(Some("test_range"));
        assert!(result.is_ok());

        let result = Profile::stop_range(Some("test_range"));
        assert!(result.is_ok());
    }
}
