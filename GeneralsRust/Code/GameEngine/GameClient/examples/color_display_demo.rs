// This example demonstrates the basic usage of our Color and DisplayString modules
// Note: This won't compile with the full project due to dependencies, but shows the API

// Example code (commented out due to dependencies):

/*
use game_client_rust::core::{Color, ColorF, DisplayString, DisplayStringManager, BasicFont, TextAlignment};
use std::rc::Rc;

fn main() {
    println!("Color and DisplayString System Demo");

    // Color system demo
    color_system_demo();

    // DisplayString system demo
    display_string_demo();
}

fn color_system_demo() {
    println!("\n=== Color System Demo ===");

    // Create colors
    let red = Color::RED;
    let blue = Color::BLUE;
    let custom = Color::from_rgba(128, 64, 192, 255);

    println!("Red: {}", red);
    println!("Blue: {}", blue);
    println!("Custom: {}", custom);

    // Color operations
    let purple = red.blend(&blue, 0.5);
    println!("Red + Blue blend: {}", purple);

    let dark_red = red.darken(30);
    println!("Red darkened 30%: {}", dark_red);

    // Color conversions
    let red_f: ColorF = red.into();
    println!("Red as float: {}", red_f);

    // Color distance
    let distance = red.distance(&blue);
    println!("Distance between red and blue: {:.2}", distance);
}

fn display_string_demo() {
    println!("\n=== DisplayString System Demo ===");

    // Create a display string manager
    let mut manager = DisplayStringManager::new();

    // Create a font
    let font = Rc::new(BasicFont::new("Arial".to_string(), 16));
    manager.set_default_font(font.clone());

    // Create display strings
    let display_string = manager.create_display_string();

    {
        let mut string = display_string.borrow_mut();
        string.set_text("Hello, World!".to_string());
        string.set_color(Color::WHITE);
        string.set_alignment(TextAlignment::Center);

        println!("Text: '{}'", string.get_text());
        println!("Length: {} characters", string.get_text_length());

        let size = string.get_size();
        println!("Size: {}x{}", size.0, size.1);

        // Simulate drawing
        println!("Drawing text at position (100, 50)");
        string.draw(100, 50);
    }

    // Clean up
    manager.free_display_string(display_string);

    let stats = manager.get_stats();
    println!("Manager stats: {}", stats);
}
*/

// Placeholder main function for compilation
fn main() {
    println!("Color and DisplayString System Demo");
    println!("This example showcases the API of our converted systems.");
    println!("See the commented code above for actual usage examples.");
}
