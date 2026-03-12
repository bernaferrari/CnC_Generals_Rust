use wwlib_rust::crc::{Crc32, Crc32Stream, CrcEngine};

fn main() {
    println!("Command & Conquer Generals CRC Demo");
    println!("===================================");

    // Test data
    let test_data = b"Command & Conquer Generals Zero Hour";
    let test_string = "Hello WWLib!";

    // Demonstrate CrcEngine (streaming CRC from original C++)
    println!("\n1. CrcEngine (Original WWLib Algorithm):");
    println!("----------------------------------------");

    let mut engine = CrcEngine::new();
    engine.update_buffer(test_data);
    let engine_crc = engine.value();
    println!("Data: {:?}", std::str::from_utf8(test_data).unwrap());
    println!("CRC: 0x{:08X} ({})", engine_crc as u32, engine_crc);

    // Demonstrate byte-by-byte processing
    let mut engine_byte = CrcEngine::new();
    for &byte in test_data {
        engine_byte.update_byte(byte);
    }
    println!(
        "Byte-by-byte CRC: 0x{:08X} (matches: {})",
        engine_byte.value() as u32,
        engine_byte.value() == engine_crc
    );

    // Demonstrate CRC32 (standard algorithm)
    println!("\n2. CRC32 (Standard IEEE 802.3 Algorithm):");
    println!("------------------------------------------");

    let crc32_value = Crc32::memory(test_data);
    println!("Data: {:?}", std::str::from_utf8(test_data).unwrap());
    println!("CRC32: 0x{:08X} ({})", crc32_value, crc32_value);

    // String CRC
    let string_crc = Crc32::string(test_string);
    println!("String '{}' CRC32: 0x{:08X}", test_string, string_crc);

    // Demonstrate CRC32 streaming
    println!("\n3. CRC32 Streaming:");
    println!("-------------------");

    let mut stream = Crc32Stream::new();

    // Process in chunks to simulate streaming
    let chunk_size = 8;
    for (i, chunk) in test_data.chunks(chunk_size).enumerate() {
        stream.update_buffer(chunk);
        println!(
            "Chunk {}: {:?} -> CRC: 0x{:08X}",
            i + 1,
            std::str::from_utf8(chunk).unwrap(),
            stream.value()
        );
    }

    println!(
        "Final streaming CRC32: 0x{:08X} (matches direct: {})",
        stream.value(),
        stream.value() == crc32_value
    );

    // Demonstrate different initial values
    println!("\n4. CRC with Initial Values:");
    println!("----------------------------");

    let initial_value = 0x12345678;
    let mut engine_with_initial = CrcEngine::with_initial(initial_value);
    engine_with_initial.update_buffer(test_data);
    println!(
        "CrcEngine with initial 0x{:08X}: 0x{:08X}",
        initial_value as u32,
        engine_with_initial.value() as u32
    );

    let crc32_with_initial = Crc32::memory_with_crc(test_data, initial_value as u32);
    println!(
        "CRC32 with initial 0x{:08X}: 0x{:08X}",
        initial_value as u32, crc32_with_initial
    );

    // Demonstrate reset functionality
    println!("\n5. Reset Functionality:");
    println!("-----------------------");

    let mut engine = CrcEngine::new();
    engine.update_buffer(b"first data");
    let first_crc = engine.value();
    println!("First calculation: 0x{:08X}", first_crc as u32);

    engine.reset();
    engine.update_buffer(b"second data");
    let second_crc = engine.value();
    println!("After reset: 0x{:08X}", second_crc as u32);

    engine.reset();
    engine.update_buffer(b"first data");
    let repeated_crc = engine.value();
    println!(
        "Repeated first: 0x{:08X} (matches: {})",
        repeated_crc as u32,
        repeated_crc == first_crc
    );

    // Performance comparison (simple)
    println!("\n6. Performance Test:");
    println!("--------------------");

    let large_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();

    let start = std::time::Instant::now();
    let mut engine = CrcEngine::new();
    let _engine_result = engine.update_buffer(&large_data);
    let engine_time = start.elapsed();

    let start = std::time::Instant::now();
    let _crc32_result = Crc32::memory(&large_data);
    let crc32_time = start.elapsed();

    println!("CrcEngine time: {:?}", engine_time);
    println!("CRC32 time: {:?}", crc32_time);
    println!("Data size: {} bytes", large_data.len());

    println!("\nDemo completed successfully!");
}
