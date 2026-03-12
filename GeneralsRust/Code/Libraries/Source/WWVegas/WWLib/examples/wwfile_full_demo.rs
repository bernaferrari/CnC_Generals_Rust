//! WWFILE Full Demo using the actual library
//!
//! This example demonstrates the usage of the WWFILE module using the full
//! wwlib-rust crate functionality. Run with: cargo run --example wwfile_full_demo

use std::env;
use wwlib_rust::wwfile::{datetime, utils, FileInterface, FileRights, SeekDirection, WWFile};
use wwlib_rust::{file_printf, file_printf_indented};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("WWFILE Full Demo - Complete File I/O Utilities");
    println!("===============================================\n");

    // Create a temporary file for demonstration
    let temp_dir = env::temp_dir();
    let demo_path = temp_dir.join("wwfile_full_demo.txt");

    // Demo 1: Basic file operations with WWFile
    println!("1. Basic File Operations with WWFile");
    println!("------------------------------------");

    let mut file = WWFile::with_path(&demo_path);
    println!("File name: {:?}", file.file_name());

    // File should not exist initially
    println!("File available: {}", file.is_available(false));
    println!("File open: {}", file.is_open());

    // Create file
    file.create()?;
    println!("File created successfully");
    println!("File open after create: {}", file.is_open());
    println!("File available after create: {}", file.is_available(false));

    // Write some data
    let test_data = b"Hello, WWFILE!\nThis is a comprehensive test.\n";
    let written = file.write(test_data)?;
    println!("Written {} bytes", written);

    // Get file size
    let size = file.size()?;
    println!("File size: {} bytes", size);

    file.close()?;
    println!("File closed\n");

    // Demo 2: Reading and seeking
    println!("2. Reading and Seeking Operations");
    println!("--------------------------------");

    file.open(FileRights::Read)?;
    let mut buffer = vec![0u8; size as usize];
    let read_bytes = file.read(&mut buffer)?;
    println!("Read {} bytes", read_bytes);
    println!("Content: {}", String::from_utf8_lossy(&buffer));

    // Test seeking
    let pos = file.seek(7, SeekDirection::Start)?;
    println!("Sought to position {} from start", pos);

    let current_pos = file.tell()?;
    println!("Current position: {}", current_pos);

    // Read a few bytes from current position
    let mut small_buffer = vec![0u8; 8];
    let read_at_pos = file.read(&mut small_buffer)?;
    println!(
        "Read {} bytes at position {}: '{}'",
        read_at_pos,
        current_pos,
        String::from_utf8_lossy(&small_buffer)
    );

    file.close()?;

    // Demo 3: Formatted writing with macros
    println!("\n3. Formatted Writing Operations");
    println!("-------------------------------");

    file.open(FileRights::Write)?;

    // Using the file_printf! macro
    file_printf!(file, "Formatted output demo:\n")?;
    file_printf!(file, "Number: {}\n", 42)?;
    file_printf!(file, "Float: {:.2}\n", 3.14159)?;
    file_printf!(file, "String: '{}'\n", "Hello World")?;
    file_printf!(file, "Boolean: {}\n", true)?;
    file_printf!(file, "Hex: 0x{:X}\n", 255)?;

    // Using indented formatting
    file_printf!(file, "\nIndented content:\n")?;
    file_printf_indented!(file, 1, "Level 1: Main section\n")?;
    file_printf_indented!(file, 2, "Level 2: Subsection A\n")?;
    file_printf_indented!(file, 3, "Level 3: Details\n")?;
    file_printf_indented!(file, 2, "Level 2: Subsection B\n")?;
    file_printf_indented!(file, 3, "Level 3: More details\n")?;
    file_printf_indented!(file, 1, "Level 1: Summary\n")?;

    file.close()?;
    println!("Formatted content written to file");

    // Demo 4: File utilities
    println!("\n4. File Utility Functions");
    println!("-------------------------");

    // Check if file exists using utilities
    println!("File exists (utils): {}", utils::exists(&demo_path));

    // Get file size using utility function
    let util_size = utils::file_size(&demo_path)?;
    println!("File size (utils): {} bytes", util_size);

    // Copy file
    let copy_path = temp_dir.join("wwfile_full_demo_copy.txt");
    let copied_bytes = utils::copy_file(&demo_path, &copy_path)?;
    println!("Copied {} bytes to copy file", copied_bytes);

    // Demo 5: Date/time operations
    println!("\n5. Date/Time Operations");
    println!("----------------------");

    match file.get_date_time() {
        Ok(dt) => {
            println!("File date/time: 0x{:08X}", dt);
            println!("  Year: {}", datetime::year(dt));
            println!("  Month: {}", datetime::month(dt));
            println!("  Day: {}", datetime::day(dt));
            println!("  Hour: {}", datetime::hour(dt));
            println!("  Minute: {}", datetime::minute(dt));
            println!("  Second: {}", datetime::second(dt));
        }
        Err(e) => println!("Could not get date/time: {}", e),
    }

    // Demo 6: Directory operations
    println!("\n6. Directory Operations");
    println!("----------------------");

    // List some files in temp directory
    let entries = utils::list_dir(&temp_dir)?;
    let our_files: Vec<_> = entries
        .iter()
        .filter(|p| {
            if let Some(name) = p.file_name() {
                name.to_string_lossy().contains("wwfile")
            } else {
                false
            }
        })
        .collect();

    println!("Our demo files in temp directory:");
    for our_file in &our_files {
        if let Some(name) = our_file.file_name() {
            println!("  - {}", name.to_string_lossy());
        }
    }

    // Find files with pattern
    let demo_files = utils::find_files(&temp_dir, "*wwfile*demo*.txt", false)?;
    println!("Found {} demo files:", demo_files.len());
    for demo_file in &demo_files {
        println!("  - {}", demo_file.display());
    }

    // Demo 7: Error handling
    println!("\n7. Error Handling Demo");
    println!("---------------------");

    // Try to open a non-existent file
    let fake_path = temp_dir.join("non_existent_file.txt");
    let mut fake_file = WWFile::with_path(&fake_path);
    match fake_file.open(FileRights::Read) {
        Ok(_) => println!("Unexpectedly opened non-existent file"),
        Err(e) => println!("Expected error opening non-existent file: {}", e),
    }

    // Try to delete a non-existent file
    match fake_file.delete() {
        Ok(_) => println!("Unexpectedly deleted non-existent file"),
        Err(e) => println!("Expected error deleting non-existent file: {}", e),
    }

    // Cleanup
    println!("\n8. Cleanup");
    println!("---------");

    // Delete the demo files
    file.delete()?;
    println!("Deleted original demo file");

    std::fs::remove_file(&copy_path)?;
    println!("Deleted copy file");

    println!("\nFull demo completed successfully!");

    Ok(())
}
