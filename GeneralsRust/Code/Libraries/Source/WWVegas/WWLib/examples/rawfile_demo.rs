//! Raw File I/O Demo
//!
//! This example demonstrates the usage of the RawFile class from the WWLib Rust library.
//! It shows basic file operations, seeking, and biasing functionality.

use std::io::Result;
use wwlib_rust::rawfile::{FileRights, RawFile, SeekOrigin};

fn main() -> Result<()> {
    println!("=== RawFile Demo ===\n");

    // Create test data
    let test_data = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let filename = "rawfile_demo.txt";

    // Demo 1: Basic file creation and writing
    println!("1. Creating and writing to file...");
    {
        let mut file = RawFile::with_name(filename);
        file.open(FileRights::WRITE)?;
        let bytes_written = file.write(test_data)?;
        println!(
            "   Wrote {} bytes to '{}'",
            bytes_written,
            file.filename().unwrap_or("unknown")
        );
        file.close()?;
    }

    // Demo 2: Reading from file
    println!("\n2. Reading from file...");
    {
        let mut file = RawFile::with_name(filename);
        file.open(FileRights::READ)?;

        let size = file.size()?;
        println!("   File size: {} bytes", size);

        let mut buffer = vec![0u8; size as usize];
        let bytes_read = file.read(&mut buffer)?;
        println!(
            "   Read {} bytes: '{}'",
            bytes_read,
            String::from_utf8_lossy(&buffer)
        );

        file.close()?;
    }

    // Demo 3: File seeking operations
    println!("\n3. Demonstrating file seeking...");
    {
        let mut file = RawFile::with_name(filename);
        file.open(FileRights::READ)?;

        // Seek to position 10
        let pos = file.seek(10, SeekOrigin::Start)?;
        println!("   Seeked to position {}", pos);

        // Read a few characters
        let mut buffer = [0u8; 5];
        file.read(&mut buffer)?;
        println!(
            "   Read at position 10: '{}'",
            String::from_utf8_lossy(&buffer)
        );

        // Current position
        let current_pos = file.tell()?;
        println!("   Current position: {}", current_pos);

        // Seek relative to current position
        file.seek(-3, SeekOrigin::Current)?;
        let new_pos = file.tell()?;
        println!("   After seeking -3 from current: {}", new_pos);

        // Seek from end
        file.seek(-5, SeekOrigin::End)?;
        let end_pos = file.tell()?;
        println!("   5 bytes from end: {}", end_pos);

        file.read(&mut buffer)?;
        println!(
            "   Read from near end: '{}'",
            String::from_utf8_lossy(&buffer)
        );

        file.close()?;
    }

    // Demo 4: File biasing (sub-file view)
    println!("\n4. Demonstrating file biasing...");
    {
        let mut file = RawFile::with_name(filename);

        // Create a biased view: start at byte 10, length 10 bytes
        file.bias(10, Some(10));
        file.open(FileRights::READ)?;

        let biased_size = file.size()?;
        println!(
            "   Biased file size: {} bytes (original was {} bytes)",
            biased_size,
            test_data.len()
        );

        // Read from the biased file (should start at position 10 in original file)
        let mut buffer = vec![0u8; biased_size as usize];
        let bytes_read = file.read(&mut buffer)?;
        println!("   Biased read: '{}'", String::from_utf8_lossy(&buffer));

        // Test seeking in biased file
        file.seek(5, SeekOrigin::Start)?; // This is relative to bias start
        let mut small_buffer = [0u8; 1];
        file.read(&mut small_buffer)?;
        println!(
            "   Character at biased position 5: '{}'",
            small_buffer[0] as char
        );

        file.close()?;

        // Clear the bias
        file.bias(0, None);
    }

    // Demo 5: Auto-open functionality
    println!("\n5. Demonstrating auto-open/close...");
    {
        let mut file = RawFile::with_name(filename);

        // Read without explicitly opening (should auto-open)
        let mut buffer = [0u8; 5];
        let bytes_read = file.read(&mut buffer)?;
        println!(
            "   Auto-opened and read {} bytes: '{}'",
            bytes_read,
            String::from_utf8_lossy(&buffer)
        );

        // File should be closed automatically after the read
        println!("   File is open: {}", file.is_open());
    }

    // Demo 6: File metadata
    println!("\n6. File metadata...");
    {
        let mut file = RawFile::with_name(filename);
        let modified_time = file.get_date_time()?;
        println!("   File last modified: {:?}", modified_time);
    }

    // Demo 7: Error handling
    println!("\n7. Error handling...");
    {
        let file = RawFile::with_name("nonexistent_file.txt");
        println!(
            "   File 'nonexistent_file.txt' exists: {}",
            file.is_available(false)
        );

        let mut file = RawFile::new();
        match file.open(FileRights::READ) {
            Ok(_) => println!("   Unexpectedly succeeded opening file without name"),
            Err(e) => println!("   Expected error opening file without name: {}", e),
        }
    }

    // Cleanup
    println!("\n8. Cleaning up...");
    {
        let mut file = RawFile::with_name(filename);
        let deleted = file.delete()?;
        println!("   File deleted: {}", deleted);
    }

    println!("\n=== Demo completed successfully! ===");
    Ok(())
}
