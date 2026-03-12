//! # Display System Examples
//!
//! This module contains comprehensive examples of how to use the display system
//! and image management functionality.

#![allow(dead_code)]

use super::{Display, Image, ImageCollection, DisplaySettings, DrawImageMode, RGBColor, IRegion2D};
use winit::event_loop::EventLoop;
use std::path::Path;

/// Example: Setting up a basic display system
/// 
/// This example demonstrates how to create and configure a display for rendering.
/// 
/// # Basic Display Setup
/// 
/// ```rust,no_run
/// use game_client_rust::display::Display;
/// use winit::event_loop::EventLoop;
/// 
/// async fn setup_basic_display() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     
///     // Create a 1920x1080 windowed display
///     let mut display = Display::new(&event_loop, 1920, 1080, true).await?;
///     
///     // Initialize the display subsystem
///     display.init()?;
///     
///     println!("Display initialized: {}x{}", 
///              display.get_width(), 
///              display.get_height());
///     
///     Ok(())
/// }
/// ```
pub async fn example_basic_display_setup() -> Result<Display, Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 1920, 1080, true).await?;
    display.init()?;
    Ok(display)
}

/// Example: Changing display modes
/// 
/// This example shows how to change resolution and toggle between windowed/fullscreen modes.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::Display;
/// # use winit::event_loop::EventLoop;
/// # 
/// async fn example_display_modes() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     let mut display = Display::new(&event_loop, 800, 600, true).await?;
///     
///     // Change to 1920x1080 windowed mode
///     display.set_display_mode(1920, 1080, 32, true)?;
///     println!("Changed to 1920x1080 windowed");
///     
///     // Switch to fullscreen
///     display.set_windowed(false)?;
///     println!("Switched to fullscreen");
///     
///     // Get available display modes
///     let mode_count = display.get_display_mode_count();
///     println!("Available display modes: {}", mode_count);
///     
///     for i in 0..mode_count {
///         if let Some(mode) = display.get_display_mode_description(i) {
///             println!("Mode {}: {}x{}@{}Hz", i, mode.width, mode.height, mode.refresh_rate);
///         }
///     }
///     
///     Ok(())
/// }
/// ```
pub async fn example_display_modes() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 800, 600, true).await?;
    
    display.set_display_mode(1920, 1080, 32, true)?;
    display.set_windowed(false)?;
    
    let mode_count = display.get_display_mode_count();
    for i in 0..mode_count {
        if let Some(_mode) = display.get_display_mode_description(i) {
            // Process mode information
        }
    }
    
    Ok(())
}

/// Example: Basic drawing operations
/// 
/// This example demonstrates how to perform basic drawing operations like lines and rectangles.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::{Display, RGBColor};
/// # use winit::event_loop::EventLoop;
/// # 
/// async fn example_basic_drawing() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     let mut display = Display::new(&event_loop, 800, 600, true).await?;
///     
///     // Begin frame rendering
///     display.begin_frame()?;
///     
///     // Clear screen to blue
///     display.clear(RGBColor::new(0.0, 0.0, 1.0))?;
///     
///     // Draw a white line
///     display.draw_line(100, 100, 200, 200, 2.0, 0xFFFFFFFF)?;
///     
///     // Draw a filled red rectangle
///     display.draw_fill_rect(50, 50, 100, 75, 0xFF0000FF)?;
///     
///     // Draw an open rectangle border in green
///     display.draw_open_rect(200, 200, 150, 100, 3.0, 0x00FF00FF)?;
///     
///     // End frame and present
///     display.end_frame()?;
///     
///     Ok(())
/// }
/// ```
pub async fn example_basic_drawing() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 800, 600, true).await?;
    
    display.begin_frame()?;
    display.clear(RGBColor::new(0.0, 0.0, 1.0))?;
    display.draw_line(100, 100, 200, 200, 2.0, 0xFFFFFFFF)?;
    display.draw_fill_rect(50, 50, 100, 75, 0xFF0000FF)?;
    display.draw_open_rect(200, 200, 150, 100, 3.0, 0x00FF00FF)?;
    display.end_frame()?;
    
    Ok(())
}

/// Example: Image loading and management
/// 
/// This example shows how to load images from files and manage them in collections.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::{Image, ImageCollection};
/// # use std::path::Path;
/// # 
/// fn example_image_loading() -> Result<(), Box<dyn std::error::Error>> {
///     // Load a single image
///     let image = Image::load_from_file("assets/textures/tank.png", Some("tank".to_string()))?;
///     println!("Loaded image: {} ({}x{})", 
///              image.get_name(), 
///              image.get_image_width(), 
///              image.get_image_height());
///     
///     // Create an image collection
///     let mut collection = ImageCollection::new();
///     
///     // Add the image to the collection
///     collection.add_image(image);
///     
///     // Load multiple images from a directory
///     let loaded_count = collection.load_from_directory("assets/textures/", false)?;
///     println!("Loaded {} images from directory", loaded_count);
///     
///     // Find an image by name
///     if let Some(found_image) = collection.find_image_by_name("tank") {
///         println!("Found tank image: {}x{}", 
///                  found_image.get_image_width(), 
///                  found_image.get_image_height());
///     }
///     
///     // Get collection statistics
///     let (cpu_memory, gpu_textures) = collection.get_memory_stats();
///     println!("Memory usage: {} bytes CPU, {} GPU textures", cpu_memory, gpu_textures);
///     
///     Ok(())
/// }
/// ```
pub fn example_image_loading() -> Result<(), Box<dyn std::error::Error>> {
    let mut collection = ImageCollection::new();
    
    // Create a sample image from raw data instead of loading from file
    let width = 64;
    let height = 64;
    let mut data = vec![0u8; (width * height * 4) as usize];
    
    // Fill with a simple pattern
    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            data[index] = (x * 255 / width) as u8;     // R
            data[index + 1] = (y * 255 / height) as u8; // G
            data[index + 2] = 128;                       // B
            data[index + 3] = 255;                       // A
        }
    }
    
    let image = Image::from_rgba_data(&data, width, height, "sample")?;
    collection.add_image(image);
    
    let (cpu_memory, gpu_textures) = collection.get_memory_stats();
    println!("Memory usage: {} bytes CPU, {} GPU textures", cpu_memory, gpu_textures);
    
    Ok(())
}

/// Example: Image rendering with different modes
/// 
/// This example demonstrates how to render images with different blending modes.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::{Display, Image, DrawImageMode};
/// # use winit::event_loop::EventLoop;
/// # 
/// async fn example_image_rendering() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     let mut display = Display::new(&event_loop, 800, 600, true).await?;
///     let device = display.device();
///     let queue = display.queue();
///     
///     // Create a sample image
///     let width = 64;
///     let height = 64;
///     let data = vec![255u8; (width * height * 4) as usize]; // White image
///     let mut image = Image::from_rgba_data(&data, width, height, "white_square")?;
///     
///     // Create GPU texture
///     image.create_gpu_texture(device, queue)?;
///     
///     // Begin rendering
///     display.begin_frame()?;
///     display.clear(RGBColor::new(0.2, 0.2, 0.2))?;
///     
///     // Draw image with different modes
///     display.draw_image(&image, 50, 50, 150, 150, 0xFFFFFFFF, DrawImageMode::Solid)?;
///     display.draw_image(&image, 200, 50, 300, 150, 0xFFFFFFFF, DrawImageMode::Alpha)?;
///     display.draw_image(&image, 350, 50, 450, 150, 0xFFFFFFFF, DrawImageMode::Additive)?;
///     display.draw_image(&image, 500, 50, 600, 150, 0xFFFFFFFF, DrawImageMode::Grayscale)?;
///     
///     display.end_frame()?;
///     
///     Ok(())
/// }
/// ```
pub async fn example_image_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 800, 600, true).await?;
    let device = display.device();
    let queue = display.queue();
    
    let width = 64;
    let height = 64;
    let data = vec![255u8; (width * height * 4) as usize];
    let mut image = Image::from_rgba_data(&data, width, height, "white_square")?;
    
    image.create_gpu_texture(device, queue)?;
    
    display.begin_frame()?;
    display.clear(RGBColor::new(0.2, 0.2, 0.2))?;
    
    display.draw_image(&image, 50, 50, 150, 150, 0xFFFFFFFF, DrawImageMode::Solid)?;
    display.draw_image(&image, 200, 50, 300, 150, 0xFFFFFFFF, DrawImageMode::Alpha)?;
    display.draw_image(&image, 350, 50, 450, 150, 0xFFFFFFFF, DrawImageMode::Additive)?;
    display.draw_image(&image, 500, 50, 600, 150, 0xFFFFFFFF, DrawImageMode::Grayscale)?;
    
    display.end_frame()?;
    
    Ok(())
}

/// Example: Setting up clipping regions
/// 
/// This example shows how to set up clipping regions for restricted drawing areas.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::{Display, IRegion2D, RGBColor};
/// # use winit::event_loop::EventLoop;
/// # 
/// async fn example_clipping() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     let mut display = Display::new(&event_loop, 800, 600, true).await?;
///     
///     display.begin_frame()?;
///     display.clear(RGBColor::black())?;
///     
///     // Set up a clipping region
///     let clip_region = IRegion2D::new(100, 100, 200, 150);
///     display.set_clip_region(Some(clip_region));
///     display.enable_clipping(true);
///     
///     // Draw operations will be clipped to the region
///     display.draw_fill_rect(50, 50, 300, 200, 0xFF0000FF)?; // Red rectangle
///     display.draw_line(0, 0, 800, 600, 3.0, 0x00FF00FF)?;   // Green line
///     
///     // Disable clipping for subsequent operations
///     display.enable_clipping(false);
///     display.draw_open_rect(400, 200, 100, 100, 2.0, 0x0000FFFF)?; // Blue border
///     
///     display.end_frame()?;
///     
///     Ok(())
/// }
/// ```
pub async fn example_clipping() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 800, 600, true).await?;
    
    display.begin_frame()?;
    display.clear(RGBColor::black())?;
    
    let clip_region = IRegion2D::new(100, 100, 200, 150);
    display.set_clip_region(Some(clip_region));
    display.enable_clipping(true);
    
    display.draw_fill_rect(50, 50, 300, 200, 0xFF0000FF)?;
    display.draw_line(0, 0, 800, 600, 3.0, 0x00FF00FF)?;
    
    display.enable_clipping(false);
    display.draw_open_rect(400, 200, 100, 100, 2.0, 0x0000FFFF)?;
    
    display.end_frame()?;
    
    Ok(())
}

/// Example: Performance monitoring
/// 
/// This example demonstrates how to monitor display performance and statistics.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::Display;
/// # use winit::event_loop::EventLoop;
/// # 
/// async fn example_performance_monitoring() -> Result<(), Box<dyn std::error::Error>> {
///     let event_loop = EventLoop::new();
///     let mut display = Display::new(&event_loop, 800, 600, true).await?;
///     
///     // Simulate a rendering loop
///     for frame in 0..60 {
///         let delta_time = 1.0 / 60.0; // 60 FPS
///         
///         display.begin_frame()?;
///         
///         // Perform some drawing operations
///         display.clear(RGBColor::new(0.1, 0.1, 0.1))?;
///         display.draw_fill_rect(frame * 10, 100, 50, 50, 0xFFFFFFFF)?;
///         
///         display.end_frame()?;
///         
///         // Update performance statistics
///         display.update_stats(delta_time);
///         
///         // Log performance every 30 frames
///         if frame % 30 == 0 {
///             let fps = display.get_average_fps();
///             let draw_calls = display.get_last_frame_draw_calls();
///             println!("Frame {}: {:.1} FPS, {} draw calls", frame, fps, draw_calls);
///         }
///     }
///     
///     Ok(())
/// }
/// ```
pub async fn example_performance_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();
    let mut display = Display::new(&event_loop, 800, 600, true).await?;
    
    for frame in 0..60 {
        let delta_time = 1.0 / 60.0;
        
        display.begin_frame()?;
        display.clear(RGBColor::new(0.1, 0.1, 0.1))?;
        display.draw_fill_rect(frame * 10, 100, 50, 50, 0xFFFFFFFF)?;
        display.end_frame()?;
        
        display.update_stats(delta_time);
        
        if frame % 30 == 0 {
            let _fps = display.get_average_fps();
            let _draw_calls = display.get_last_frame_draw_calls();
        }
    }
    
    Ok(())
}

/// Example: Advanced image operations
/// 
/// This example shows advanced image manipulation features.
/// 
/// ```rust,no_run
/// # use game_client_rust::display::{Image, ImageStatus, Region2D};
/// # 
/// fn example_advanced_image_operations() -> Result<(), Box<dyn std::error::Error>> {
///     // Create a test image
///     let width = 256;
///     let height = 256;
///     let mut data = vec![0u8; (width * height * 4) as usize];
///     
///     // Create a gradient pattern
///     for y in 0..height {
///         for x in 0..width {
///             let index = ((y * width + x) * 4) as usize;
///             data[index] = (x * 255 / width) as u8;     // R gradient
///             data[index + 1] = (y * 255 / height) as u8; // G gradient
///             data[index + 2] = 128;                       // Constant B
///             data[index + 3] = 255;                       // Full alpha
///         }
///     }
///     
///     let mut image = Image::from_rgba_data(&data, width, height, "gradient")?;
///     
///     // Set custom UV coordinates (use only top-left quarter)
///     let custom_uv = Region2D::from_coords(0.0, 0.0, 0.5, 0.5);
///     image.set_uv(custom_uv);
///     
///     // Set image status flags
///     image.set_status(ImageStatus::HAS_ALPHA | ImageStatus::RAW_TEXTURE);
///     
///     // Check image properties
///     println!("Image: {}", image.get_name());
///     println!("Size: {}x{}", image.get_image_width(), image.get_image_height());
///     println!("Has alpha: {}", image.has_alpha());
///     println!("UV coords: {:?}", image.get_uv());
///     
///     // Convert to grayscale
///     image.to_grayscale();
///     println!("Converted to grayscale, has alpha: {}", image.has_alpha());
///     
///     // Resize image
///     image.resize(128, 128, image::imageops::FilterType::Lanczos3);
///     println!("Resized to: {}x{}", image.get_image_width(), image.get_image_height());
///     
///     Ok(())
/// }
/// ```
pub fn example_advanced_image_operations() -> Result<(), Box<dyn std::error::Error>> {
    let width = 256;
    let height = 256;
    let mut data = vec![0u8; (width * height * 4) as usize];
    
    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            data[index] = (x * 255 / width) as u8;
            data[index + 1] = (y * 255 / height) as u8;
            data[index + 2] = 128;
            data[index + 3] = 255;
        }
    }
    
    let mut image = Image::from_rgba_data(&data, width, height, "gradient")?;
    
    let custom_uv = Region2D::from_coords(0.0, 0.0, 0.5, 0.5);
    image.set_uv(custom_uv);
    
    image.set_status(ImageStatus::HAS_ALPHA | ImageStatus::RAW_TEXTURE);
    
    println!("Image: {}", image.get_name());
    println!("Size: {}x{}", image.get_image_width(), image.get_image_height());
    println!("Has alpha: {}", image.has_alpha());
    
    image.to_grayscale();
    image.resize(128, 128, image::imageops::FilterType::Lanczos3);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_image_loading() {
        let result = example_image_loading();
        assert!(result.is_ok());
    }

    #[test]
    fn test_advanced_image_operations() {
        let result = example_advanced_image_operations();
        assert!(result.is_ok());
    }

    // Note: Async tests would require a test runtime
    // These are examples of how to test async functions:
    
    #[tokio::test]
    async fn test_display_modes() {
        // This would only work with tokio runtime
        // let result = example_display_modes().await;
        // assert!(result.is_ok());
    }
}