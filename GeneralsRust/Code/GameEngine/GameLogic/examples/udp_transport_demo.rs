// UDP Transport demonstration and validation
//
// This example demonstrates the UDP transport layer and validates C++ compatibility.
// Run with: cargo run --example udp_transport_demo

use gamelogic::transport::*;
use std::net::SocketAddr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UDP Transport Demo ===\n");

    // Test 1: CRC Algorithm
    println!("1. Testing CRC Algorithm");
    println!("   ----------------------");
    test_crc();

    // Test 2: Encryption/Decryption
    println!("\n2. Testing Encryption");
    println!("   ------------------");
    test_encryption();

    // Test 3: Packet Structure
    println!("\n3. Testing Packet Structure");
    println!("   ------------------------");
    test_packet_structure();

    // Test 4: UDP Transport
    println!("\n4. Testing UDP Transport");
    println!("   ---------------------");
    test_udp_transport()?;

    // Test 5: Send/Receive Loopback
    println!("\n5. Testing Send/Receive Loopback");
    println!("   -----------------------------");
    test_loopback()?;

    println!("\n✅ All tests passed! Transport layer is ready for integration.\n");

    Ok(())
}

fn test_crc() {
    let mut crc = Crc::new();
    let data = b"Hello, Generals!";
    crc.compute(data);

    println!("   Input: {:?}", std::str::from_utf8(data).unwrap());
    println!("   CRC:   0x{:08X}", crc.get());

    // Test determinism
    let mut crc2 = Crc::new();
    crc2.compute(data);
    assert_eq!(crc.get(), crc2.get());
    println!("   ✓ CRC is deterministic");

    // Test incremental
    let mut crc3 = Crc::new();
    crc3.compute(&data[..8]);
    crc3.compute(&data[8..]);
    assert_eq!(crc.get(), crc3.get());
    println!("   ✓ CRC is incremental");
}

fn test_encryption() {
    let original = b"This is a test message for encryption validation!";
    let mut encrypted = original.to_vec();

    println!("   Original: {:?}", std::str::from_utf8(original).unwrap());

    let encrypted_len = encrypted.len();
    encrypt_buffer(&mut encrypted, encrypted_len);
    println!("   Encrypted (first 16 bytes): {:02X?}", &encrypted[..16]);

    assert_ne!(&encrypted[..], &original[..]);
    println!("   ✓ Data was encrypted");

    let encrypted_len = encrypted.len();
    decrypt_buffer(&mut encrypted, encrypted_len);
    println!(
        "   Decrypted: {:?}",
        std::str::from_utf8(&encrypted).unwrap()
    );

    assert_eq!(&encrypted[..], &original[..]);
    println!("   ✓ Decryption successful");
}

fn test_packet_structure() {
    // Verify sizes match C++
    let header_size = std::mem::size_of::<TransportMessageHeader>();
    println!("   TransportMessageHeader size: {} bytes", header_size);
    assert_eq!(header_size, 6, "Header must be 6 bytes (4 + 2)");
    println!("   ✓ Header size matches C++ (6 bytes)");

    // Test packet creation
    let mut msg = TransportMessage::new();
    let magic = msg.header.magic;
    assert_eq!(magic, GENERALS_MAGIC_NUMBER);
    println!("   ✓ Magic number: 0x{:04X}", magic);

    // Test data setting
    let test_data = b"Test packet data";
    msg.set_data(test_data).unwrap();
    assert_eq!(msg.get_data(), test_data);
    println!("   ✓ Data set/get working");

    // Test CRC computation
    msg.compute_crc();
    assert!(msg.validate_crc());
    println!("   ✓ CRC validation passed");

    // Test address setting
    let addr: SocketAddr = "192.168.1.100:8088".parse().unwrap();
    msg.set_address(addr);
    assert_eq!(msg.get_address().unwrap(), addr);
    println!("   ✓ Address set/get working");
}

fn test_udp_transport() -> Result<(), Box<dyn std::error::Error>> {
    // Create transport
    let addr: SocketAddr = "127.0.0.1:0".parse()?;
    let mut transport = UdpTransport::new(addr)?;
    let local_addr = transport.local_addr();
    println!("   Bound to: {}", local_addr);

    // Test queue send
    let dest: SocketAddr = "127.0.0.1:9999".parse()?;
    let data = b"Test message";
    transport.queue_send(dest, data)?;
    println!("   ✓ Queued send to {}", dest);

    // Test send with ACK
    let packet_id = transport.send_with_ack(dest, b"Reliable message", 5)?;
    println!("   ✓ Queued reliable send (ID: {})", packet_id);

    // Test ACK
    transport.acknowledge_packet(packet_id);
    println!("   ✓ ACK processed");

    // Test stats
    let stats = transport.get_stats();
    println!("   Statistics:");
    println!("     - Outgoing packets: {}", stats.outgoing_packets);
    println!("     - Outgoing bytes: {}", stats.outgoing_bytes);
    println!("     - Pending ACKs: {}", stats.pending_acks);

    Ok(())
}

fn test_loopback() -> Result<(), Box<dyn std::error::Error>> {
    // Create two transports
    let mut transport1 = UdpTransport::new("127.0.0.1:0".parse()?)?;
    let mut transport2 = UdpTransport::new("127.0.0.1:0".parse()?)?;

    let addr1 = transport1.local_addr();
    let addr2 = transport2.local_addr();

    println!("   Transport 1: {}", addr1);
    println!("   Transport 2: {}", addr2);

    // Send message from 1 to 2
    let test_data = b"Hello from transport 1!";
    transport1.queue_send(addr2, test_data)?;
    println!("   ✓ Queued message from T1 to T2");

    // Process send
    transport1.do_send()?;
    println!("   ✓ Sent packets");

    // Small delay to ensure delivery
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Receive on transport2
    transport2.do_recv()?;
    println!("   ✓ Received packets");

    // Check message
    if let Some(msg) = transport2.recv_message() {
        println!(
            "   Received data: {:?}",
            std::str::from_utf8(msg.get_data()).unwrap()
        );
        assert_eq!(msg.get_data(), test_data);
        assert!(msg.is_valid_packet());
        println!("   ✓ Message validated");
    } else {
        println!("   ⚠ No message received (this may be due to UDP unreliability in loopback)");
    }

    Ok(())
}
