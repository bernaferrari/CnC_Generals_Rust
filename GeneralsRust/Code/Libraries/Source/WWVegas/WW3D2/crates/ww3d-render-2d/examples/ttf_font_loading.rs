//! Example: TTF Font Loading
//!
//! This example demonstrates how to use the TTF font loading feature
//! to load TrueType fonts and generate bitmap atlases for text rendering.

use ww3d_render_2d::font_system::FontSystem;

fn main() {
    println!("TTF Font Loading Example");
    println!("========================\n");

    let mut font_system = FontSystem::new();

    // Example 1: Load a TTF font with default size
    println!("Example 1: Loading TTF font with default size (32px)");
    println!("---------------------------------------------------");
    match font_system.load_ttf_font("Arial", "/System/Library/Fonts/Supplemental/Arial.ttf") {
        Ok(font) => {
            println!("Successfully loaded font: {}", font.name);
            println!("Character table size: {}", font.char_table.len());
            println!("Character height: {}", font.char_height);
            println!("Space width: {}", font.space_width);
        }
        Err(e) => {
            println!(
                "Failed to load font (this is expected if Arial.ttf is not available): {}",
                e
            );
            println!("On macOS, try system fonts like Arial, Helvetica, or Times");
            println!("On Linux, try fonts from /usr/share/fonts/");
            println!("On Windows, try fonts from C:\\Windows\\Fonts\\");
        }
    }

    println!();

    // Example 2: Load a TTF font with custom size
    println!("Example 2: Loading TTF font with custom size (48px)");
    println!("--------------------------------------------------");
    match font_system.load_ttf_font_with_size(
        "Arial-Large",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        48.0,
    ) {
        Ok(font) => {
            println!("Successfully loaded large font: {}", font.name);
            println!("Character height: {}", font.char_height);
        }
        Err(e) => {
            println!(
                "Failed to load font (this is expected if file doesn't exist): {}",
                e
            );
        }
    }

    println!();

    // Example 3: Load different font formats
    println!("Example 3: Font system supports multiple formats");
    println!("------------------------------------------------");
    println!("Supported font formats:");
    println!("  - TTF (TrueType Font)");
    println!("  - OTF (OpenType Font)");
    println!("  - TGA (Bitmap font atlas)");
    println!();
    println!("Usage:");
    println!("  font_system.load_font(name, path) - Auto-detect format");
    println!("  font_system.load_ttf_font(name, path) - Load TTF/OTF");
    println!("  font_system.load_tga_font(name, path) - Load TGA bitmap");

    println!();

    // Example 4: Working with loaded fonts
    println!("Example 4: Using loaded fonts");
    println!("------------------------------");

    if let Some(font) = font_system.get_font("Arial") {
        println!("Retrieved font: {}", font.name);

        // Calculate text width
        let text = "Hello, World!";
        let width = font_system.calculate_text_width("Arial", text, 1.0);
        println!("Text '{}' width at scale 1.0: {}", text, width);

        // Set as default font
        if let Ok(()) = font_system.set_default_font("Arial") {
            println!("Set Arial as default font");
        }
    }

    println!();

    // Example 5: Font system features
    println!("Example 5: Font system features");
    println!("--------------------------------");
    println!("Features implemented:");
    println!("  [x] TTF/OTF font loading");
    println!("  [x] Dynamic glyph rasterization");
    println!("  [x] Bitmap atlas generation");
    println!("  [x] Character metrics (width, height, advance)");
    println!("  [x] Proportional spacing");
    println!("  [x] TGA bitmap font support (legacy)");
    println!("  [x] Font caching and management");
    println!("  [x] Custom font sizes");
    println!();
    println!("The atlas includes:");
    println!("  - Printable ASCII characters (32-126)");
    println!("  - Extended characters (128-255)");
    println!("  - Proper kerning and spacing");
    println!("  - Alpha channel for smooth rendering");

    println!();
    println!("Total fonts loaded: {}", font_system.font_count());
    println!("Available fonts: {:?}", font_system.list_fonts());
}
