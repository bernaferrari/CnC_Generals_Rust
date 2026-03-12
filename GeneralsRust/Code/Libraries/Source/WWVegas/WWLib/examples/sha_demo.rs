use wwlib_rust::sha::*;

fn main() {
    println!("WWLib Rust SHA-1 Implementation Demo");
    println!("=====================================");

    // Test with various inputs
    let test_cases: &[(&str, &[u8])] = &[
        ("Empty string", b""),
        ("Single character", b"a"),
        ("Hello World", b"Hello, World!"),
        ("Test vector 1", b"abc"),
        (
            "Test vector 2",
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        ),
    ];

    for (name, input) in test_cases {
        println!(
            "\n{}: '{}'",
            name,
            std::str::from_utf8(input).unwrap_or("<binary>")
        );

        // One-shot hashing
        let hash = ShaEngine::hash_data(input);
        println!("SHA-1: {}", sha1_digest_to_hex(&hash));

        // Streaming interface for comparison
        let mut engine = ShaEngine::new();
        engine.update(input);
        let streaming_hash = engine.finalize();

        assert_eq!(
            hash, streaming_hash,
            "One-shot and streaming results should match"
        );
        println!("✓ One-shot and streaming interfaces produce identical results");
    }

    println!("\n--- Streaming Interface Demo ---");
    let mut engine = ShaEngine::new();
    engine.update(b"Hello, ");
    println!("Added: 'Hello, '");

    engine.update(b"World");
    println!("Added: 'World'");

    engine.update(b"!");
    println!("Added: '!'");

    let final_hash = engine.finalize();
    println!("Final hash: {}", sha1_digest_to_hex(&final_hash));

    // Verify it matches the one-shot version
    let oneshot_hash = ShaEngine::hash_data(b"Hello, World!");
    assert_eq!(final_hash, oneshot_hash);
    println!("✓ Matches one-shot hash of 'Hello, World!'");

    println!("\n--- Large Data Demo ---");
    let large_data = "a".repeat(1000);
    let large_hash = ShaEngine::hash_data(large_data.as_bytes());
    println!(
        "SHA-1 of 1000 'a' characters: {}",
        sha1_digest_to_hex(&large_hash)
    );

    println!("\n--- Security Warning ---");
    println!("⚠️  SHA-1 is cryptographically broken and should not be used for security purposes.");
    println!(
        "    This implementation is for legacy compatibility with C&C game data formats only."
    );

    println!("\nDemo completed successfully!");
}
