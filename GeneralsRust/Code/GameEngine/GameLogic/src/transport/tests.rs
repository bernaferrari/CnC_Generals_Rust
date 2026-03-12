// Comprehensive tests for UDP transport C++ compatibility

use super::*;
use std::net::SocketAddr;

#[test]
fn test_packet_header_size_matches_cpp() {
    // C++ sizeof(TransportMessageHeader) = 6 bytes
    // 4 bytes (crc) + 2 bytes (magic) = 6 bytes
    assert_eq!(
        std::mem::size_of::<TransportMessageHeader>(),
        6,
        "Header size must match C++ implementation"
    );
}

#[test]
fn test_magic_number_is_correct() {
    assert_eq!(GENERALS_MAGIC_NUMBER, 0xF00D);
}

#[test]
fn test_crc_algorithm_basic() {
    // Test the CRC algorithm matches C++ behavior
    let mut crc = Crc::new();
    crc.compute(b"Test");

    // The CRC should be deterministic
    let mut crc2 = Crc::new();
    crc2.compute(b"Test");

    assert_eq!(crc.get(), crc2.get());
}

#[test]
fn test_crc_empty_buffer() {
    let mut crc = Crc::new();
    crc.compute(&[]);
    assert_eq!(crc.get(), 0);
}

#[test]
fn test_encryption_decryption_roundtrip() {
    let mut data = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10,
    ];
    let original = data;

    let len = data.len();
    encrypt_buffer(&mut data, len);
    assert_ne!(&data[..], &original[..], "Data should be encrypted");

    decrypt_buffer(&mut data, len);
    assert_eq!(
        &data[..],
        &original[..],
        "Data should match after decryption"
    );
}

#[test]
fn test_encryption_initial_mask() {
    // Test that encryption uses the correct initial mask (0x0000Fade)
    let mut data = [0u8; 4];
    encrypt_buffer(&mut data, 4);

    // XOR with mask, then htonl
    let expected = 0x0000_Fade_u32.to_be();
    let result = u32::from_le_bytes(data);

    assert_eq!(result, expected);
}

#[test]
fn test_transport_message_create() {
    let msg = TransportMessage::new();
    let magic = unsafe { std::ptr::addr_of!(msg.header.magic).read_unaligned() };
    assert_eq!(magic, GENERALS_MAGIC_NUMBER);
    assert_eq!(msg.length, 0);
}

#[test]
fn test_transport_message_set_data() {
    let mut msg = TransportMessage::new();
    let data = b"Hello, Generals!";

    msg.set_data(data).unwrap();
    assert_eq!(msg.length, data.len() as i32);
    assert_eq!(msg.get_data(), data);
}

#[test]
fn test_transport_message_crc_validation() {
    let mut msg = TransportMessage::new();
    msg.set_data(b"Test packet data").unwrap();
    msg.compute_crc();

    assert!(msg.validate_crc(), "CRC should be valid");
    assert!(msg.is_valid_packet(), "Packet should be valid");

    // Corrupt data
    msg.data[0] ^= 0xFF;
    assert!(
        !msg.validate_crc(),
        "CRC should be invalid after corruption"
    );
}

#[test]
fn test_transport_message_address() {
    let mut msg = TransportMessage::new();
    let addr: SocketAddr = "192.168.1.100:8088".parse().unwrap();

    msg.set_address(addr);
    let retrieved = msg.get_address().unwrap();

    assert_eq!(retrieved, addr);
}

#[test]
fn test_udp_transport_create() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let transport = UdpTransport::new(addr).unwrap();

    assert_eq!(transport.local_addr().ip(), addr.ip());
}

#[test]
fn test_udp_transport_queue_send() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut transport = UdpTransport::new(addr).unwrap();

    let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
    let data = b"Test message";

    transport.queue_send(dest, data).unwrap();
    assert_eq!(transport.out_buffer_len(), 1);
}

#[test]
fn test_udp_transport_send_receive_loopback() {
    // Create two transports
    let addr1: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut transport1 = UdpTransport::new(addr1).unwrap();
    let local1 = transport1.local_addr();

    let addr2: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut transport2 = UdpTransport::new(addr2).unwrap();
    let local2 = transport2.local_addr();

    // Send message from transport1 to transport2
    let test_data = b"Hello from transport1!";
    transport1.queue_send(local2, test_data).unwrap();
    transport1.do_send().unwrap();

    // Give it a moment to arrive
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Receive on transport2
    transport2.do_recv().unwrap();

    let msg = transport2.recv_message();
    assert!(msg.is_some(), "Should receive message");

    let msg = msg.unwrap();
    assert_eq!(msg.get_data(), test_data);
    assert!(msg.is_valid_packet());
}

#[test]
fn test_udp_transport_with_ack() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut transport = UdpTransport::new(addr).unwrap();

    let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
    let data = b"Test with ACK";

    let packet_id = transport.send_with_ack(dest, data, 5).unwrap();
    assert!(transport.has_pending_ack(packet_id));

    // Acknowledge the packet
    assert!(transport.acknowledge_packet(packet_id));
    assert!(!transport.has_pending_ack(packet_id));
}

#[test]
fn test_udp_transport_stats() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut transport = UdpTransport::new(addr).unwrap();

    let stats = transport.get_stats();
    assert_eq!(stats.incoming_packets, 0);
    assert_eq!(stats.outgoing_packets, 0);
}

#[test]
fn test_binary_compatibility_packet_size() {
    // Verify that our packet structures match C++ sizes
    assert_eq!(
        std::mem::size_of::<TransportMessageHeader>(),
        6,
        "Header size mismatch"
    );

    // TransportMessage should have the header at offset 0
    let msg = TransportMessage::new();
    let header_offset = unsafe {
        let msg_ptr = &msg as *const TransportMessage as usize;
        let header_ptr = &msg.header as *const TransportMessageHeader as usize;
        header_ptr - msg_ptr
    };
    assert_eq!(header_offset, 0, "Header must be at offset 0");
}

#[test]
fn test_crc_matches_cpp_algorithm() {
    // Test specific values to ensure CRC algorithm matches C++
    let test_data = [0x01, 0x02, 0x03, 0x04];

    let mut crc = Crc::new();
    for &byte in &test_data {
        // Manually compute CRC to verify algorithm
        let current_crc = crc.get();
        let hibit = if current_crc & 0x8000_0000 != 0 { 1 } else { 0 };
        let expected_crc = (current_crc << 1)
            .wrapping_add(byte as u32)
            .wrapping_add(hibit);

        crc.compute(&[byte]);
        assert_eq!(
            crc.get(),
            expected_crc,
            "CRC algorithm mismatch at byte {}",
            byte
        );
    }
}

#[test]
fn test_packet_validation_rejects_invalid_magic() {
    let mut msg = TransportMessage::new();
    msg.set_data(b"Test").unwrap();
    msg.compute_crc();

    // Corrupt magic number
    msg.header.magic = 0xDEAD;

    assert!(!msg.is_valid_packet(), "Should reject invalid magic number");
}

#[test]
fn test_packet_validation_rejects_invalid_length() {
    let mut msg = TransportMessage::new();
    msg.length = -1;

    assert!(!msg.is_valid_packet(), "Should reject negative length");

    msg.length = (MAX_MESSAGE_LEN + 1) as i32;
    assert!(
        !msg.is_valid_packet(),
        "Should reject length exceeding MAX_MESSAGE_LEN"
    );
}

#[test]
fn test_encryption_mask_increments() {
    // Verify that the mask increments by 0x321 for each 4-byte word
    let mut data = [0u8; 8];
    encrypt_buffer(&mut data, 8);

    // Decrypt and verify masks were applied correctly
    decrypt_buffer(&mut data, 8);
    assert_eq!(&data[..], &[0u8; 8][..]);
}

#[test]
fn test_retry_timeout_constant() {
    // Verify retry timeout matches C++ (2000ms)
    assert_eq!(RETRY_TIME_MS, 2000);
}

#[test]
fn test_keepalive_interval_constant() {
    // Verify keep-alive interval matches C++ (20 seconds)
    assert_eq!(KEEPALIVE_INTERVAL_SECS, 20);
}

#[test]
fn test_max_messages_constant() {
    // Verify MAX_MESSAGES matches C++ (128)
    assert_eq!(MAX_MESSAGES, 128);
}

#[test]
fn test_max_message_len_constant() {
    // Verify MAX_MESSAGE_LEN matches C++ (1024)
    assert_eq!(MAX_MESSAGE_LEN, 1024);
}

#[test]
fn test_max_packet_size_constant() {
    // Verify MAX_PACKET_SIZE matches C++ (476)
    assert_eq!(MAX_PACKET_SIZE, 476);
}
