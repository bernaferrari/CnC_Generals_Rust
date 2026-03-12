use wwlib_rust::md5::{digest_to_hex, Md5};

fn main() {
    println!("MD5 Implementation Demo");
    println!("=======================");

    // Test cases from RFC 1321
    let test_cases = [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ("abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            "abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
        (
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        ("hello world", "5eb63bbbe01eeed093cb22bb8f5acdc3"),
    ];

    println!("Testing known MD5 values:");
    let mut passed = 0;
    for (input, expected) in &test_cases {
        let digest = Md5::hash(input.as_bytes());
        let hex = digest_to_hex(&digest);
        let status = if hex == *expected {
            "✓ PASS"
        } else {
            "✗ FAIL"
        };
        println!(
            "  Input: {:30} -> {}: {}",
            format!("\"{}\"", input),
            status,
            hex
        );
        if hex == *expected {
            passed += 1;
        }
    }
    println!("  Results: {}/{} tests passed", passed, test_cases.len());

    println!("\nTesting streaming interface:");
    let mut hasher = Md5::new();
    hasher.update(b"hello ");
    hasher.update(b"world");
    let digest = hasher.finalize();
    let hex = digest_to_hex(&digest);
    println!("  \"hello \" + \"world\" -> {}", hex);

    // Verify it matches one-shot
    let oneshot = Md5::hash(b"hello world");
    let oneshot_hex = digest_to_hex(&oneshot);
    let status = if hex == oneshot_hex {
        "✓ CONSISTENT"
    } else {
        "✗ INCONSISTENT"
    };
    println!("  One-shot verification: {} -> {}", status, oneshot_hex);

    println!("\nTesting chunked streaming:");
    let data = b"The quick brown fox jumps over the lazy dog";
    let mut hasher1 = Md5::new();
    hasher1.update(data);
    let digest1 = hasher1.finalize();

    let mut hasher2 = Md5::new();
    for chunk in data.chunks(7) {
        hasher2.update(chunk);
    }
    let digest2 = hasher2.finalize();

    let match_status = if digest1 == digest2 {
        "✓ MATCH"
    } else {
        "✗ MISMATCH"
    };
    println!(
        "  Chunked vs whole: {} -> {}",
        match_status,
        digest_to_hex(&digest1)
    );

    println!("\nTesting boundary conditions:");
    for size in [55, 56, 63, 64, 65] {
        let input = vec![0x42u8; size];
        let digest = Md5::hash(&input);
        let hex = digest_to_hex(&digest);
        println!("  {} bytes -> {}", size, hex);
    }

    println!("\nTesting large input:");
    let large_input = "a".repeat(1000000);
    let start = std::time::Instant::now();
    let digest = Md5::hash(large_input.as_bytes());
    let duration = start.elapsed();
    let hex = digest_to_hex(&digest);
    println!(
        "  1,000,000 'a' characters -> {} (took {:?})",
        hex, duration
    );

    if passed == test_cases.len() {
        println!("\n✓ All tests passed! MD5 implementation is working correctly!");
        println!("This implementation matches the behavior of the original C++ WWLib MD5.");
    } else {
        println!("\n✗ Some tests failed.");
    }
}
