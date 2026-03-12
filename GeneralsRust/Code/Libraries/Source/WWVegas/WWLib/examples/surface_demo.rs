//! Surface Graphics Demo
//!
//! This example demonstrates the Surface graphics handling utilities from the
//! Command & Conquer Generals WWLib library, now converted to Rust.
//!
//! Features demonstrated:
//! - Surface creation with different pixel formats
//! - Pixel manipulation and color handling
//! - Drawing operations (lines, rectangles, fills)
//! - Blitting operations between surfaces
//! - Memory locking for direct pixel access
//! - Transparency handling
//! - Error handling throughout

use wwlib_rust::point::Point2D;
use wwlib_rust::surface::{PixelFormat, Rect, Surface, SurfaceError};

fn main() -> Result<(), SurfaceError> {
    println!("Surface Graphics Demo");
    println!("====================");

    // Demonstrate surface creation with different formats
    demo_surface_creation()?;

    // Demonstrate basic drawing operations
    demo_drawing_operations()?;

    // Demonstrate blitting between surfaces
    demo_blitting_operations()?;

    // Demonstrate palette operations
    demo_palette_operations()?;

    // Demonstrate memory locking
    demo_memory_locking()?;

    // Demonstrate transparency
    demo_transparency()?;

    println!("\nDemo completed successfully!");
    Ok(())
}

fn demo_surface_creation() -> Result<(), SurfaceError> {
    println!("\n1. Surface Creation Demo");
    println!("-----------------------");

    // Create surfaces with different pixel formats
    let rgb24_surface = Surface::new(640, 480, PixelFormat::RGB24)?;
    println!(
        "Created RGB24 surface: {}x{} ({} bytes per pixel)",
        rgb24_surface.get_width(),
        rgb24_surface.get_height(),
        rgb24_surface.bytes_per_pixel()
    );

    let rgba32_surface = Surface::new(320, 240, PixelFormat::RGBA32)?;
    println!(
        "Created RGBA32 surface: {}x{} ({} bytes per pixel)",
        rgba32_surface.get_width(),
        rgba32_surface.get_height(),
        rgba32_surface.bytes_per_pixel()
    );

    let palette8_surface = Surface::new(160, 120, PixelFormat::Palette8)?;
    println!(
        "Created Palette8 surface: {}x{} ({} bytes per pixel)",
        palette8_surface.get_width(),
        palette8_surface.get_height(),
        palette8_surface.bytes_per_pixel()
    );

    // Demonstrate error handling
    match Surface::new(0, 480, PixelFormat::RGB24) {
        Ok(_) => println!("ERROR: Should not have created invalid surface!"),
        Err(e) => println!("Correctly caught invalid dimension error: {}", e),
    }

    Ok(())
}

fn demo_drawing_operations() -> Result<(), SurfaceError> {
    println!("\n2. Drawing Operations Demo");
    println!("-------------------------");

    let mut surface = Surface::new(200, 200, PixelFormat::RGB24)?;

    // Fill the surface with a background color
    surface.fill(0x404040)?; // Dark gray
    println!("Filled surface with dark gray background");

    // Draw individual pixels
    surface.put_pixel(Point2D::new(50, 50), 0xFF0000)?; // Red pixel
    surface.put_pixel(Point2D::new(51, 50), 0x00FF00)?; // Green pixel
    surface.put_pixel(Point2D::new(52, 50), 0x0000FF)?; // Blue pixel
    println!("Drew RGB pixels at (50,50), (51,50), (52,50)");

    // Verify pixel colors
    let red_pixel = surface.get_pixel(Point2D::new(50, 50))?;
    let green_pixel = surface.get_pixel(Point2D::new(51, 50))?;
    let blue_pixel = surface.get_pixel(Point2D::new(52, 50))?;

    println!(
        "Verified pixel colors: Red=0x{:06X}, Green=0x{:06X}, Blue=0x{:06X}",
        red_pixel, green_pixel, blue_pixel
    );

    // Draw lines
    surface.draw_line(Point2D::new(10, 10), Point2D::new(190, 10), 0xFF0000)?; // Red horizontal line
    surface.draw_line(Point2D::new(10, 10), Point2D::new(10, 190), 0x00FF00)?; // Green vertical line
    surface.draw_line(Point2D::new(10, 10), Point2D::new(190, 190), 0x0000FF)?; // Blue diagonal line
    println!("Drew lines: horizontal (red), vertical (green), diagonal (blue)");

    // Draw rectangles
    surface.draw_rect(Rect::new(30, 30, 50, 40), 0xFFFF00)?; // Yellow rectangle
    surface.draw_rect(Rect::new(100, 30, 50, 40), 0xFF00FF)?; // Magenta rectangle
    println!("Drew colored rectangle outlines");

    // Fill rectangles
    surface.fill_rect(Rect::new(35, 35, 40, 30), 0x800080)?; // Purple fill
    surface.fill_rect(Rect::new(105, 35, 40, 30), 0x008080)?; // Teal fill
    println!("Filled rectangles with colors");

    println!("Surface bounds: {:?}", surface.get_rect());

    Ok(())
}

fn demo_blitting_operations() -> Result<(), SurfaceError> {
    println!("\n3. Blitting Operations Demo");
    println!("--------------------------");

    // Create source and destination surfaces
    let mut source = Surface::new(50, 50, PixelFormat::RGB24)?;
    let mut dest = Surface::new(200, 150, PixelFormat::RGB24)?;

    // Fill source with a pattern
    source.fill(0x800000)?; // Dark red background
    source.fill_rect(Rect::new(10, 10, 30, 30), 0xFF8080)?; // Light red center
    source.draw_rect(Rect::new(5, 5, 40, 40), 0xFFFFFF)?; // White border
    println!("Created source surface with pattern");

    // Fill destination with different color
    dest.fill(0x000080)?; // Dark blue background
    println!("Created destination surface with blue background");

    // Perform blitting operation
    dest.blit_from(&source, false)?;
    println!("Blitted source to destination (no transparency)");

    // Verify the blit worked by checking a pixel that should have been copied
    let copied_pixel = dest.get_pixel(Point2D::new(25, 25))?;
    println!("Pixel at (25,25) after blit: 0x{:06X}", copied_pixel);

    // Test blitting to specific rectangle
    let dest_rect = Rect::new(60, 30, 50, 50);
    let src_rect = source.get_rect();
    dest.blit_from_rect(dest_rect, &source, src_rect, false)?;
    println!("Blitted source to specific destination rectangle");

    Ok(())
}

fn demo_palette_operations() -> Result<(), SurfaceError> {
    println!("\n4. Palette Operations Demo");
    println!("-------------------------");

    let mut surface = Surface::new(100, 100, PixelFormat::Palette8)?;

    // Set up palette colors
    if let Some(palette) = surface.get_palette_mut() {
        palette.set_color(0, 0xFF000000)?; // Index 0: Black (transparent)
        palette.set_color(1, 0xFFFF0000)?; // Index 1: Red
        palette.set_color(2, 0xFF00FF00)?; // Index 2: Green
        palette.set_color(3, 0xFF0000FF)?; // Index 3: Blue
        palette.set_color(4, 0xFFFFFFFF)?; // Index 4: White
        println!("Set up palette with 5 colors");

        // Verify palette colors
        for i in 0..5 {
            if let Some(color) = palette.get_color(i) {
                println!("  Palette[{}] = 0x{:08X}", i, color);
            }
        }
    }

    // Draw using palette indices
    surface.fill(1)?; // Fill with red (index 1)
    surface.put_pixel(Point2D::new(10, 10), 2)?; // Green pixel (index 2)
    surface.put_pixel(Point2D::new(11, 10), 3)?; // Blue pixel (index 3)
    surface.put_pixel(Point2D::new(12, 10), 4)?; // White pixel (index 4)

    println!("Drew pixels using palette indices");

    // Verify palette indexing
    let red_index = surface.get_pixel(Point2D::new(50, 50))?;
    let green_index = surface.get_pixel(Point2D::new(10, 10))?;
    println!(
        "Verified palette indices: Red area={}, Green pixel={}",
        red_index, green_index
    );

    Ok(())
}

fn demo_memory_locking() -> Result<(), SurfaceError> {
    println!("\n5. Memory Locking Demo");
    println!("---------------------");

    let surface = Surface::new(64, 64, PixelFormat::RGB24)?;
    println!("Created 64x64 RGB24 surface");

    println!("Surface locked status: {}", surface.is_locked());

    // Demonstrate memory locking
    {
        let lock = surface.lock(Point2D::new(0, 0))?;
        println!("Locked surface for direct memory access");
        println!("Surface locked status: {}", surface.is_locked());

        println!("Lock stride: {} bytes", lock.stride());

        // Demonstrate safe pixel pointer access
        unsafe {
            if let Some(_ptr) = lock.get_pixel_ptr(Point2D::new(32, 32)) {
                println!("Successfully got pixel pointer for (32, 32)");
            }

            // Try to get invalid pixel pointer
            if lock.get_pixel_ptr(Point2D::new(100, 100)).is_none() {
                println!("Correctly rejected out-of-bounds pixel pointer request");
            }
        }

        // Test double-lock protection
        match surface.lock(Point2D::new(0, 0)) {
            Ok(_) => println!("ERROR: Should not have been able to double-lock!"),
            Err(e) => println!("Correctly prevented double-lock: {}", e),
        }
    } // Lock automatically released here

    println!(
        "Surface locked status after release: {}",
        surface.is_locked()
    );

    Ok(())
}

fn demo_transparency() -> Result<(), SurfaceError> {
    println!("\n6. Transparency Demo");
    println!("-------------------");

    // Create surfaces for transparency test
    let mut source = Surface::new(30, 30, PixelFormat::RGB24)?;
    let mut dest = Surface::new(100, 80, PixelFormat::RGB24)?;

    // Set up source with transparent and opaque pixels
    source.fill(0x000000)?; // Black = transparent color
    source.fill_rect(Rect::new(5, 5, 20, 20), 0xFF0000)?; // Red square in center
    source.draw_rect(Rect::new(4, 4, 22, 22), 0x00FF00)?; // Green border
    println!("Created source with transparent background and colored content");

    // Set up destination with different background
    dest.fill(0x0000FF)?; // Blue background
    dest.fill_rect(Rect::new(20, 20, 40, 30), 0x808080)?; // Gray rectangle
    println!("Created destination with blue background and gray rectangle");

    // First blit without transparency (should overwrite everything)
    dest.blit_from(&source, false)?;
    println!("Performed blit without transparency");

    let pixel_in_transparent_area = dest.get_pixel(Point2D::new(2, 2))?;
    println!(
        "Pixel in 'transparent' area after non-transparent blit: 0x{:06X}",
        pixel_in_transparent_area
    );

    // Reset destination
    dest.fill(0x0000FF)?;
    dest.fill_rect(Rect::new(20, 20, 40, 30), 0x808080)?;

    // Now blit with transparency
    dest.blit_from(&source, true)?;
    println!("Performed blit with transparency");

    let transparent_pixel = dest.get_pixel(Point2D::new(2, 2))?;
    let opaque_pixel = dest.get_pixel(Point2D::new(15, 15))?;
    println!(
        "Pixel in transparent area: 0x{:06X} (should be blue 0x0000FF)",
        transparent_pixel
    );
    println!(
        "Pixel in opaque area: 0x{:06X} (should be red 0xFF0000)",
        opaque_pixel
    );

    Ok(())
}

#[cfg(test)]
mod demo_tests {
    use super::*;

    #[test]
    fn test_demo_functions() {
        // Test that all demo functions run without errors
        assert!(demo_surface_creation().is_ok());
        assert!(demo_drawing_operations().is_ok());
        assert!(demo_blitting_operations().is_ok());
        assert!(demo_palette_operations().is_ok());
        assert!(demo_memory_locking().is_ok());
        assert!(demo_transparency().is_ok());
    }
}
