//! WWFILE Demo
//!
//! This example demonstrates the usage of the WWFILE module,
//! showing basic file operations that can be run standalone.
//! For full functionality, use `cargo run --example wwfile_demo`

use std::env;

// Since this is a standalone example, we'll define minimal types
// In real usage, you would use: use wwlib_rust::wwfile::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("WWFILE Demo - File I/O Utilities");
    println!("=================================\n");

    // Create a temporary file for demonstration
    let temp_dir = env::temp_dir();
    let demo_path = temp_dir.join("wwfile_demo_standalone.txt");

    // Demo 1: Basic file operations using std::fs
    println!("1. Basic File Operations (using std::fs)");
    println!("----------------------------------------");

    // Create and write to file
    let test_data = b"Hello, WWFILE!\nThis is a test file.\n";
    std::fs::write(&demo_path, test_data)?;
    println!("File created and written: {}", demo_path.display());

    // Read from file
    let read_data = std::fs::read(&demo_path)?;
    println!("Read {} bytes", read_data.len());
    println!("Content: {}", String::from_utf8_lossy(&read_data));

    // Get file metadata
    let metadata = std::fs::metadata(&demo_path)?;
    println!("File size: {} bytes", metadata.len());

    // Demo 2: Path operations
    println!("\n2. Path Operations");
    println!("------------------");

    if let Some(file_name) = demo_path.file_name() {
        println!("File name: {}", file_name.to_string_lossy());
    }

    if let Some(parent) = demo_path.parent() {
        println!("Parent directory: {}", parent.display());
    }

    println!("File exists: {}", demo_path.exists());

    // Demo 3: Directory operations
    println!("\n3. Directory Operations");
    println!("-----------------------");

    // List files in temp directory (limit to first 5 for brevity)
    let entries = std::fs::read_dir(&temp_dir)?;
    println!("Files in temp directory (first 5):");
    for (i, entry) in entries.enumerate() {
        if i >= 5 {
            break;
        }
        let entry = entry?;
        println!("  - {}", entry.file_name().to_string_lossy());
    }

    // Demo 4: Date/time simulation
    println!("\n4. DOS Date/Time Format Demo");
    println!("----------------------------");

    // Simulate the DOS datetime format extraction functions
    let test_dt = 0x2A7F_1234u32; // Example DOS datetime

    let year = ((test_dt & 0xFE00_0000) >> (9 + 16)) + 1980;
    let month = (test_dt & 0x01E0_0000) >> (5 + 16);
    let day = (test_dt & 0x001F_0000) >> (0 + 16);
    let hour = (test_dt & 0x0000_F800) >> 11;
    let minute = (test_dt & 0x0000_07E0) >> 5;
    let second = (test_dt & 0x0000_001F) << 1;

    println!("DOS DateTime: 0x{:08X}", test_dt);
    println!("  Year: {}", year);
    println!("  Month: {}", month);
    println!("  Day: {}", day);
    println!("  Hour: {}", hour);
    println!("  Minute: {}", minute);
    println!("  Second: {}", second);

    // Cleanup
    println!("\n5. Cleanup");
    println!("---------");

    std::fs::remove_file(&demo_path)?;
    println!("Deleted {}", demo_path.display());

    println!("\nDemo completed successfully!");
    println!("\nNote: To use the full WWFILE API, run with:");
    println!("  cargo run --example wwfile_demo");

    Ok(())
}
