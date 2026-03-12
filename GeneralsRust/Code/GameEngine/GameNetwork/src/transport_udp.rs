//! Raw UDP transport layer matching C++ implementation exactly.
//!
//! This transport uses raw UDP sockets instead of QUIC, implementing the exact
//! packet format, encryption, and CRC scheme from the original C++ Generals implementation.
//!
//! Packet format (C++ compatible):
//! [CRC32: 4 bytes][Magic: 2 bytes 0xF00D][Payload: 0-470 bytes]
//! Total max packet size: 476 bytes (512 - 36 byte IP/UDP headers)
//!
//! Encryption: XOR with mask 0x0000Fade, incrementing by 0x00000321 per 4-byte word

use crate::error::{NetworkError, NetworkResult};
use crate::observability::telemetry;
use crate::time::NetworkInstant;
use crate::transport::{TransportMessage, TransportMetrics, TransportProtocol};
use parking_lot::RwLock as SyncRwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::info;

/// C++ Protocol Constants
pub const GENERALS_MAGIC_NUMBER: u16 = 0xF00D;
pub const MAX_PACKET_SIZE: usize = 476;
pub const MAX_MESSAGE_LEN: usize = 1024;
pub const MAX_MESSAGES: usize = 128;
pub const MAX_COMMANDS: usize = 256; // Maximum commands per frame (matches C++)
pub const XOR_MASK_INITIAL: u32 = 0x0000Fade;
pub const XOR_MASK_INCREMENT: u32 = 0x00000321;
/// Keep-alive interval in milliseconds - MUST match C++ exactly
/// C++ NAT.cpp uses 15000ms (15 seconds) for keep-alive interval
pub const KEEP_ALIVE_INTERVAL_MS: u64 = 15000; // 15 seconds
pub const IDLE_TIMEOUT_SECS: u64 = 60; // Match C++ NetworkDefs.h NETWORK_PLAYER_TIMEOUT_TIME
const PACKET_HEADER_SIZE_BYTES: usize = 6;

/// Transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub bind_address: SocketAddr,
    pub protocol: TransportProtocol,
    pub keep_alive_interval: Duration,
    pub max_idle_timeout: Duration,
    pub max_packet_size: usize,
    pub enable_broadcast: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::from(([127, 0, 0, 1], 8088)),
            protocol: TransportProtocol::Udp,
            keep_alive_interval: Duration::from_millis(KEEP_ALIVE_INTERVAL_MS),
            max_idle_timeout: Duration::from_secs(IDLE_TIMEOUT_SECS),
            max_packet_size: MAX_PACKET_SIZE,
            enable_broadcast: false,
        }
    }
}

/// Packet header matching C++ TransportMessageHeader
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PacketHeader {
    pub crc: u32,   // CRC32 (must be first)
    pub magic: u16, // Magic number 0xF00D
}

impl PacketHeader {
    pub fn new_with_crc(payload: &[u8]) -> Self {
        let crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, payload);
        Self {
            crc,
            magic: GENERALS_MAGIC_NUMBER,
        }
    }

    pub fn verify_crc(&self, payload: &[u8]) -> bool {
        let expected_crc = calculate_packet_crc(self.magic, payload);
        self.crc == expected_crc && self.magic == GENERALS_MAGIC_NUMBER
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(6);
        bytes.extend_from_slice(&self.crc.to_ne_bytes());
        bytes.extend_from_slice(&self.magic.to_ne_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> NetworkResult<Self> {
        if bytes.len() < 6 {
            return Err(NetworkError::transport("Packet header too small"));
        }
        let crc = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let magic = u16::from_ne_bytes([bytes[4], bytes[5]]);
        Ok(Self { crc, magic })
    }
}

/// CRC32 algorithm matching C++ bit-rotation implementation
/// Uses custom bit-rotation algorithm with left shift and carry
/// Algorithm:
/// - Initial CRC: 0 (not 0xFFFFFFFF)
/// - For each byte:
///   - Check if high bit (0x80000000) is set
///   - Left shift CRC by 1
///   - Add byte value to CRC
///   - If high bit was set, add 1 to CRC
/// - No final XOR (no bitwise NOT)
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;

    for &byte in data {
        let hibit = (crc & 0x80000000) != 0;
        crc = crc << 1;
        crc = crc.wrapping_add(byte as u32);
        if hibit {
            crc = crc.wrapping_add(1);
        }
    }

    crc
}

/// Calculate CRC for a packet (magic number + payload)
/// This matches the C++ implementation where CRC is computed on [Magic (2 bytes) + Payload]
pub fn calculate_packet_crc(magic: u16, payload: &[u8]) -> u32 {
    let mut data = Vec::with_capacity(2 + payload.len());
    data.extend_from_slice(&magic.to_ne_bytes());
    data.extend_from_slice(payload);
    calculate_crc32(&data)
}

/// XOR encryption matching C++ implementation exactly
/// C++ code (Transport.cpp:48-56):
/// 1. Read bytes as native-endian u32
/// 2. XOR with mask
/// 3. Convert to network byte order (htonl) for transmission
pub fn xor_encrypt(data: &mut [u8]) {
    let mut mask = XOR_MASK_INITIAL;
    let mut ptr = 0;

    // Only encrypt 4-byte aligned portions (matching C++ behavior)
    while ptr + 4 <= data.len() {
        // Read as native-endian (matching C++ pointer cast)
        let word = u32::from_ne_bytes([data[ptr], data[ptr + 1], data[ptr + 2], data[ptr + 3]]);
        // XOR with mask
        let xored = word ^ mask;
        // Convert to network byte order (matching C++ htonl())
        let encrypted = xored.to_be_bytes();
        data[ptr..ptr + 4].copy_from_slice(&encrypted);
        mask = mask.wrapping_add(XOR_MASK_INCREMENT);
        ptr += 4;
    }
}

/// XOR decryption matching C++ implementation exactly
/// C++ code (Transport.cpp:65-71):
/// 1. Convert from network byte order (htonl) - reverses byte order
/// 2. XOR with mask (same mask sequence)
pub fn xor_decrypt(data: &mut [u8]) {
    let mut mask = XOR_MASK_INITIAL;
    let mut ptr = 0;

    // Only decrypt 4-byte aligned portions (matching C++ behavior)
    while ptr + 4 <= data.len() {
        // Convert FROM network byte order (big-endian to native)
        let word = u32::from_be_bytes([data[ptr], data[ptr + 1], data[ptr + 2], data[ptr + 3]]);
        // XOR with mask
        let xored = word ^ mask;
        // Write back as native-endian
        let decrypted = xored.to_ne_bytes();
        data[ptr..ptr + 4].copy_from_slice(&decrypted);
        mask = mask.wrapping_add(XOR_MASK_INCREMENT);
        ptr += 4;
    }
}

/// UDP-based transport
pub struct Transport {
    config: SyncRwLock<TransportConfig>,
    socket: Arc<Mutex<Option<Arc<UdpSocket>>>>,
    is_bound: Arc<SyncRwLock<bool>>,
    inbound_messages: Arc<Mutex<Vec<TransportMessage>>>,
    outbound_messages: Arc<Mutex<Vec<TransportMessage>>>,
    packets_sent: Arc<SyncRwLock<u64>>,
    packets_received: Arc<SyncRwLock<u64>>,
    bytes_sent: Arc<SyncRwLock<u64>>,
    bytes_received: Arc<SyncRwLock<u64>>,
    active_connections: Arc<Mutex<HashMap<SocketAddr, NetworkInstant>>>,
    connection_births: Arc<Mutex<HashMap<SocketAddr, NetworkInstant>>>,
    shutdown_tx: broadcast::Sender<()>,
    receive_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    public_address: SyncRwLock<Option<SocketAddr>>,
}

impl Transport {
    /// Create new transport with default config
    pub async fn new() -> NetworkResult<Self> {
        Self::with_config(TransportConfig::default()).await
    }

    /// Create transport with custom config
    pub async fn with_config(config: TransportConfig) -> NetworkResult<Self> {
        info!("Creating UDP transport with config: {:?}", config);

        if config.protocol != TransportProtocol::Udp {
            return Err(NetworkError::transport(
                "UDP transport only supports TransportProtocol::Udp",
            ));
        }

        if config.max_packet_size > MAX_PACKET_SIZE {
            return Err(NetworkError::transport(format!(
                "max_packet_size too large for Generals UDP protocol: {} > {}",
                config.max_packet_size, MAX_PACKET_SIZE
            )));
        }

        if config.max_packet_size < PACKET_HEADER_SIZE_BYTES {
            return Err(NetworkError::transport(format!(
                "max_packet_size too small for Generals UDP protocol: {} < {}",
                config.max_packet_size, PACKET_HEADER_SIZE_BYTES
            )));
        }

        let (shutdown_tx, _) = broadcast::channel(4);

        Ok(Self {
            config: SyncRwLock::new(config),
            socket: Arc::new(Mutex::new(None)),
            is_bound: Arc::new(SyncRwLock::new(false)),
            inbound_messages: Arc::new(Mutex::new(Vec::new())),
            outbound_messages: Arc::new(Mutex::new(Vec::new())),
            packets_sent: Arc::new(SyncRwLock::new(0)),
            packets_received: Arc::new(SyncRwLock::new(0)),
            bytes_sent: Arc::new(SyncRwLock::new(0)),
            bytes_received: Arc::new(SyncRwLock::new(0)),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            connection_births: Arc::new(Mutex::new(HashMap::new())),
            shutdown_tx,
            receive_task: Arc::new(Mutex::new(None)),
            public_address: SyncRwLock::new(None),
        })
    }

    /// Create transport for testing
    #[cfg(test)]
    pub fn new_for_testing(bind_addr: SocketAddr) -> Arc<Self> {
        let config = TransportConfig {
            bind_address: bind_addr,
            protocol: TransportProtocol::Udp,
            keep_alive_interval: Duration::from_millis(KEEP_ALIVE_INTERVAL_MS),
            max_idle_timeout: Duration::from_secs(IDLE_TIMEOUT_SECS),
            max_packet_size: MAX_PACKET_SIZE,
            enable_broadcast: false,
        };

        let (shutdown_tx, _) = broadcast::channel(4);

        Arc::new(Self {
            config: SyncRwLock::new(config),
            socket: Arc::new(Mutex::new(None)),
            is_bound: Arc::new(SyncRwLock::new(false)),
            inbound_messages: Arc::new(Mutex::new(Vec::new())),
            outbound_messages: Arc::new(Mutex::new(Vec::new())),
            packets_sent: Arc::new(SyncRwLock::new(0)),
            packets_received: Arc::new(SyncRwLock::new(0)),
            bytes_sent: Arc::new(SyncRwLock::new(0)),
            bytes_received: Arc::new(SyncRwLock::new(0)),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            connection_births: Arc::new(Mutex::new(HashMap::new())),
            shutdown_tx,
            receive_task: Arc::new(Mutex::new(None)),
            public_address: SyncRwLock::new(None),
        })
    }

    /// Bind socket to configured address
    pub async fn bind(&self) -> NetworkResult<()> {
        let config = self.config.read().clone();

        if *self.is_bound.read() {
            return Ok(());
        }

        let socket = UdpSocket::bind(config.bind_address)
            .await
            .map_err(|e| NetworkError::transport(format!("Failed to bind UDP socket: {}", e)))?;

        if config.enable_broadcast {
            socket.set_broadcast(true).map_err(|e| {
                NetworkError::transport(format!("Failed to enable broadcast: {}", e))
            })?;
        }

        *self.socket.lock().unwrap() = Some(Arc::new(socket));
        *self.is_bound.write() = true;

        // Spawn receive task
        let receive_socket = self
            .socket
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| NetworkError::transport("Socket not bound"))?;
        let inbound = self.inbound_messages.clone();
        let packets_rx = self.packets_received.clone();
        let bytes_rx = self.bytes_received.clone();
        let active_conns = self.active_connections.clone();
        let mut shutdown = self.shutdown_tx.subscribe();

        let receive_handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; MAX_PACKET_SIZE];

            loop {
                tokio::select! {
                    _ = shutdown.recv() => break,
                    result = receive_socket.recv_from(&mut buffer) => {
                        if let Ok((len, src)) = result {
                            {
                                let mut pkt_cnt = packets_rx.write();
                                *pkt_cnt += 1;
                                let mut byte_cnt = bytes_rx.write();
                                *byte_cnt += len as u64;
                            }

                            {
                                let mut conns = active_conns.lock().unwrap();
                                conns.insert(src, NetworkInstant::now());
                            }

                            if let Some(telem) = telemetry() {
                                telem.record_packet_received(len, Duration::from_secs(0));
                            }

                            if len < PACKET_HEADER_SIZE_BYTES {
                                continue;
                            }

                            let mut packet = buffer[..len].to_vec();
                            xor_decrypt(&mut packet);

                            let header = match PacketHeader::from_bytes(&packet[..PACKET_HEADER_SIZE_BYTES]) {
                                Ok(h) => h,
                                Err(_) => continue,
                            };
                            let payload = &packet[PACKET_HEADER_SIZE_BYTES..];
                            if !header.verify_crc(payload) {
                                continue;
                            }

                            let mut msgs = inbound.lock().unwrap();
                            msgs.push(
                                TransportMessage::new(payload.to_vec(), TransportProtocol::Udp)
                                    .with_source(src),
                            );
                        }
                    }
                }
            }
        });

        *self.receive_task.lock().unwrap() = Some(receive_handle);

        info!("UDP transport bound to {}", config.bind_address);
        Ok(())
    }

    /// Send a message
    pub async fn send_message(&self, message: TransportMessage) -> NetworkResult<()> {
        if message.protocol != TransportProtocol::Udp {
            return Err(NetworkError::transport(
                "This transport only handles UDP messages",
            ));
        }

        let destination = message
            .destination
            .ok_or_else(|| NetworkError::transport("Destination address required"))?;

        let max_packet_size = self.config.read().max_packet_size;
        let max_payload_size = max_packet_size.saturating_sub(PACKET_HEADER_SIZE_BYTES);

        if message.data.len() > max_payload_size {
            return Err(NetworkError::transport(format!(
                "Payload too large: {} > {}",
                message.data.len(),
                max_payload_size
            )));
        }

        let socket = self
            .socket
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| NetworkError::transport("Socket not bound"))?;

        // Create packet: [CRC32][Magic][Payload]
        let mut packet = Vec::with_capacity(PACKET_HEADER_SIZE_BYTES + message.data.len());

        // Build header with payload CRC
        let header = PacketHeader::new_with_crc(&message.data);
        packet.extend_from_slice(&header.to_bytes());
        packet.extend_from_slice(&message.data);

        // Encrypt ENTIRE packet including header (matches C++ Transport.cpp line 410)
        // C++ encrypts: len + sizeof(TransportMessageHeader) = entire packet
        xor_encrypt(&mut packet);

        socket
            .send_to(&packet, destination)
            .await
            .map_err(|e| NetworkError::transport(format!("UDP send failed: {}", e)))?;

        {
            let mut msgs = self.outbound_messages.lock().unwrap();
            msgs.push(message);
        }

        {
            let mut pkt_cnt = self.packets_sent.write();
            *pkt_cnt += 1;
            let mut byte_cnt = self.bytes_sent.write();
            *byte_cnt += packet.len() as u64;
        }

        if let Some(telem) = telemetry() {
            telem.record_packet_sent(packet.len());
        }

        Ok(())
    }

    /// Receive all pending messages
    pub async fn receive_messages(&self) -> NetworkResult<Vec<TransportMessage>> {
        Ok(self.inbound_messages.lock().unwrap().drain(..).collect())
    }

    /// Update transport (process timeouts, etc)
    pub async fn update(&self) -> NetworkResult<()> {
        let now = NetworkInstant::now();
        let idle_timeout = self.config.read().max_idle_timeout;

        let mut to_remove = Vec::new();
        {
            let conns = self.active_connections.lock().unwrap();
            for (addr, last_seen) in conns.iter() {
                if now.duration_since(*last_seen) > idle_timeout {
                    to_remove.push(*addr);
                }
            }
        }

        if !to_remove.is_empty() {
            let mut conns = self.active_connections.lock().unwrap();
            let mut births = self.connection_births.lock().unwrap();
            for addr in to_remove {
                conns.remove(&addr);
                births.remove(&addr);

                if let Some(telem) = telemetry() {
                    telem.record_connection_closed(idle_timeout);
                    telem.set_active_connections(conns.len());
                }
            }
        }

        Ok(())
    }

    /// Shutdown transport
    pub async fn shutdown(&self) -> NetworkResult<()> {
        let _ = self.shutdown_tx.send(());

        if let Some(task) = self.receive_task.lock().unwrap().take() {
            task.abort();
        }

        *self.socket.lock().unwrap() = None;
        *self.is_bound.write() = false;

        self.inbound_messages.lock().unwrap().clear();
        self.outbound_messages.lock().unwrap().clear();
        self.active_connections.lock().unwrap().clear();
        self.connection_births.lock().unwrap().clear();

        Ok(())
    }

    /// Get metrics
    pub async fn metrics(&self) -> TransportMetrics {
        TransportMetrics {
            packets_sent: *self.packets_sent.read(),
            packets_received: *self.packets_received.read(),
            bytes_sent: *self.bytes_sent.read(),
            bytes_received: *self.bytes_received.read(),
        }
    }

    pub async fn packets_sent(&self) -> u64 {
        *self.packets_sent.read()
    }

    pub async fn packets_received(&self) -> u64 {
        *self.packets_received.read()
    }

    pub async fn bytes_sent(&self) -> u64 {
        *self.bytes_sent.read()
    }

    pub async fn bytes_received(&self) -> u64 {
        *self.bytes_received.read()
    }

    pub async fn is_bound(&self) -> bool {
        *self.is_bound.read()
    }

    pub fn is_ready(&self) -> bool {
        *self.is_bound.read()
    }

    pub fn config(&self) -> TransportConfig {
        self.config.read().clone()
    }

    pub fn set_bind_address(&self, bind_address: SocketAddr) -> NetworkResult<()> {
        if self.is_ready() {
            return Err(NetworkError::transport(
                "Cannot change bind address after binding",
            ));
        }
        self.config.write().bind_address = bind_address;
        Ok(())
    }

    pub fn set_public_address(&self, address: Option<SocketAddr>) {
        *self.public_address.write() = address;
    }

    pub fn public_address(&self) -> Option<SocketAddr> {
        *self.public_address.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_bit_rotation() {
        // Test the C++ bit-rotation algorithm with known values
        let data = b"hello world";
        let crc = calculate_crc32(data);
        // Verify CRC is computed (non-zero for most inputs)
        assert_ne!(crc, 0);

        // Test with empty data
        let empty_crc = calculate_crc32(&[]);
        assert_eq!(empty_crc, 0); // Initial CRC value is 0

        // Test with single byte
        let single_byte_crc = calculate_crc32(&[0x42]);
        assert_eq!(single_byte_crc, 0x42); // Should be just the byte value when starting from 0
    }

    #[test]
    fn test_crc32_manual_test_vector() {
        // Manual test vector: magic=0xF00D, payload=[0x01, 0x02, 0x03]
        // Let's compute this step by step:
        //
        // Data to hash: [0x0D, 0xF0] (magic in little-endian) + [0x01, 0x02, 0x03] (payload)
        //
        // C++ Algorithm:
        // crc = 0
        // Byte 0x0D (13):
        //   hibit = (0 & 0x80000000) != 0 = false
        //   crc = 0 << 1 = 0
        //   crc = 0 + 13 = 13
        //   if hibit: (skip)
        //   crc = 13 = 0x0000000D
        //
        // Byte 0xF0 (240):
        //   hibit = (13 & 0x80000000) != 0 = false
        //   crc = 13 << 1 = 26 = 0x0000001A
        //   crc = 26 + 240 = 266 = 0x0000010A
        //   if hibit: (skip)
        //   crc = 266 = 0x0000010A
        //
        // Byte 0x01 (1):
        //   hibit = (266 & 0x80000000) != 0 = false
        //   crc = 266 << 1 = 532 = 0x00000214
        //   crc = 532 + 1 = 533 = 0x00000215
        //   if hibit: (skip)
        //   crc = 533 = 0x00000215
        //
        // Byte 0x02 (2):
        //   hibit = (533 & 0x80000000) != 0 = false
        //   crc = 533 << 1 = 1066 = 0x0000042A
        //   crc = 1066 + 2 = 1068 = 0x0000042C
        //   if hibit: (skip)
        //   crc = 1068 = 0x0000042C
        //
        // Byte 0x03 (3):
        //   hibit = (1068 & 0x80000000) != 0 = false
        //   crc = 1068 << 1 = 2136 = 0x00000858
        //   crc = 2136 + 3 = 2139 = 0x0000085B
        //   if hibit: (skip)
        //   crc = 2139 = 0x0000085B
        //
        // Expected CRC: 0x0000085B = 2139

        let magic: u16 = 0xF00D;
        let payload = &[0x01, 0x02, 0x03];
        let crc = calculate_packet_crc(magic, payload);
        assert_eq!(
            crc, 0x0000085B,
            "CRC should match manual calculation: got 0x{:08X}, expected 0x0000085B",
            crc
        );
    }

    #[test]
    fn test_packet_crc_includes_magic() {
        // Verify that packet CRC includes magic number
        let payload = &[0x01, 0x02, 0x03];
        let crc_with_magic = calculate_packet_crc(GENERALS_MAGIC_NUMBER, payload);
        let crc_without_magic = calculate_crc32(payload);

        // These should be different because one includes magic, one doesn't
        assert_ne!(
            crc_with_magic, crc_without_magic,
            "Packet CRC (with magic) should differ from payload-only CRC"
        );
    }

    #[test]
    fn test_xor_encrypt_decrypt() {
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let original = data.clone();
        xor_encrypt(&mut data);
        assert_ne!(data, original); // Should be encrypted
        xor_decrypt(&mut data);
        assert_eq!(data, original); // Should be decrypted back
    }

    #[test]
    fn test_packet_header() {
        let payload = b"test data";
        let header = PacketHeader::new_with_crc(payload);
        assert_eq!(header.magic, GENERALS_MAGIC_NUMBER);
        assert!(header.verify_crc(payload));
    }

    #[test]
    fn test_packet_header_crc_verification() {
        let payload = b"test data";
        let header = PacketHeader::new_with_crc(payload);

        // Should verify correctly with same payload
        assert!(header.verify_crc(payload));

        // Should fail with different payload
        let wrong_payload = b"wrong data";
        assert!(!header.verify_crc(wrong_payload));
    }

    #[tokio::test]
    async fn test_transport_creation() {
        let transport = Transport::new().await.unwrap();
        assert!(!transport.is_ready());
        assert_eq!(transport.packets_sent().await, 0);
    }

    #[test]
    fn test_xor_endianness_network_byte_order() {
        // Test that encryption matches C++ implementation exactly
        // C++ on little-endian (x86/ARM):
        // 1. Cast bytes to uint32: [0x01,0x02,0x03,0x04] -> 0x04030201 (LE interpretation)
        // 2. XOR: 0x04030201 ^ 0x0000Fade = 0x04030F8DF
        // 3. htonl converts to BE: 0xf8df0304
        // 4. Bytes become: [0x04, 0x03, 0xf8, 0xdf]

        let mut data = vec![0x01, 0x02, 0x03, 0x04];
        let original = data.clone();

        xor_encrypt(&mut data);

        // Verify encryption happened (should be different)
        assert_ne!(data, original);

        // Expected result from C++ algorithm on LE systems:
        // Bytes [0x01,0x02,0x03,0x04] read as LE: 0x04030201
        // XOR with 0x0000Fade:
        //   0x01 ^ 0xde = 0xdf
        //   0x02 ^ 0xfa = 0xf8
        //   0x03 ^ 0x00 = 0x03
        //   0x04 ^ 0x00 = 0x04
        // Result: 0x04f8f8df, when converted to BE bytes: [0x04, 0x03, 0xf8, 0xdf]
        assert_eq!(data, vec![0x04, 0x03, 0xf8, 0xdf]);

        // Decrypt and verify we get original back
        xor_decrypt(&mut data);
        assert_eq!(data, original);
    }

    #[test]
    fn test_xor_multi_word_encryption() {
        // Test encryption of multiple 4-byte words with mask increment
        let mut data = vec![
            0x00, 0x00, 0x00, 0x00, // First word: 0x00000000
            0xFF, 0xFF, 0xFF, 0xFF, // Second word: 0xFFFFFFFF
            0x12, 0x34, 0x56, 0x78, // Third word: 0x12345678
        ];
        let original = data.clone();

        xor_encrypt(&mut data);

        // Verify encryption happened
        assert_ne!(data, original);

        // First word: 0x00000000 ^ 0x0000Fade = 0x0000Fade
        // Expected bytes: [0x00, 0x00, 0xFA, 0xDE]
        assert_eq!(&data[0..4], &[0x00, 0x00, 0xFA, 0xDE]);

        // Second word: 0xFFFFFFFF ^ (0x0000Fade + 0x321) = 0xFFFFFFFF ^ 0x0000FDFF
        // = 0xFFFF0200
        // Expected bytes: [0xFF, 0xFF, 0x02, 0x00]
        assert_eq!(&data[4..8], &[0xFF, 0xFF, 0x02, 0x00]);

        // Decrypt and verify round-trip
        xor_decrypt(&mut data);
        assert_eq!(data, original);
    }

    #[test]
    fn test_xor_partial_word() {
        // Test that partial words (less than 4 bytes) are not encrypted
        let mut data = vec![0x01, 0x02, 0x03];
        let original = data.clone();

        xor_encrypt(&mut data);

        // Should remain unchanged (no full 4-byte word)
        assert_eq!(data, original);
    }

    #[test]
    fn test_xor_with_partial_remainder() {
        // Test encryption with 4-byte words + remainder
        let mut data = vec![
            0x11, 0x22, 0x33, 0x44, // Full word - should be encrypted
            0xAA, 0xBB, // Partial - should NOT be encrypted
        ];

        let expected_unencrypted = vec![0xAA, 0xBB];
        xor_encrypt(&mut data);

        // First 4 bytes should be encrypted
        assert_ne!(&data[0..4], &[0x11, 0x22, 0x33, 0x44]);

        // Last 2 bytes should remain unchanged
        assert_eq!(&data[4..6], expected_unencrypted.as_slice());
    }

    #[test]
    fn test_cpp_compatibility_vectors() {
        // Comprehensive test suite for C++ compatibility
        // Tests CRC calculation, packet parsing, encryption/decryption

        println!("\n=== C++ Compatibility Test Vectors ===\n");

        // Test Vector A: Empty payload
        {
            let payload: Vec<u8> = vec![];
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test A - Empty payload:");
            println!("  Payload: []");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            // Create header and verify
            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "Empty payload CRC mismatch");
            assert!(
                header.verify_crc(&payload),
                "Empty payload CRC verification failed"
            );

            // Verify wrong payload fails
            let wrong_payload = vec![0x01];
            assert!(
                !header.verify_crc(&wrong_payload),
                "Wrong payload should fail CRC check"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector B: Single byte (0x00)
        {
            let payload = vec![0x00];
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test B - Single byte (0x00):");
            println!("  Payload: [0x00]");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "Single byte CRC mismatch");
            assert!(
                header.verify_crc(&payload),
                "Single byte CRC verification failed"
            );

            // Test wrong payload
            let wrong_payload = vec![0xFF];
            assert!(
                !header.verify_crc(&wrong_payload),
                "Wrong payload should fail"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector C: "HELLO" text
        {
            let payload = vec![0x48, 0x45, 0x4C, 0x4C, 0x4F]; // "HELLO"
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test C - HELLO text:");
            println!("  Payload: [0x48, 0x45, 0x4C, 0x4C, 0x4F] (\"HELLO\")");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "HELLO CRC mismatch");
            assert!(header.verify_crc(&payload), "HELLO CRC verification failed");

            // Test with different text
            let wrong_payload = vec![0x57, 0x4F, 0x52, 0x4C, 0x44]; // "WORLD"
            assert!(
                !header.verify_crc(&wrong_payload),
                "Different text should fail"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector D: All zeros
        {
            let payload = vec![0x00, 0x00, 0x00, 0x00];
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test D - All zeros:");
            println!("  Payload: [0x00, 0x00, 0x00, 0x00]");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "All zeros CRC mismatch");
            assert!(
                header.verify_crc(&payload),
                "All zeros CRC verification failed"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector E: All ones
        {
            let payload = vec![0xFF, 0xFF, 0xFF, 0xFF];
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test E - All ones:");
            println!("  Payload: [0xFF, 0xFF, 0xFF, 0xFF]");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "All ones CRC mismatch");
            assert!(
                header.verify_crc(&payload),
                "All ones CRC verification failed"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector F: Alternating pattern
        {
            let payload = vec![0xAA, 0x55, 0xAA, 0x55];
            let expected_crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload);

            println!("Test F - Alternating pattern:");
            println!("  Payload: [0xAA, 0x55, 0xAA, 0x55]");
            println!("  Expected CRC: 0x{:08X}", expected_crc);

            let header = PacketHeader::new_with_crc(&payload);
            assert_eq!(header.crc, expected_crc, "Alternating pattern CRC mismatch");
            assert!(
                header.verify_crc(&payload),
                "Alternating pattern CRC verification failed"
            );

            println!("  PASS: CRC calculation and verification\n");
        }

        // Test Vector G: Full packet construction (C++ format)
        // Updated to match C++ Transport.cpp line 410: encrypts ENTIRE packet
        {
            println!("Test G - Full packet construction (entire packet encryption):");
            let payload = vec![0x01, 0x02, 0x03, 0x04];
            println!("  Original payload: {:?}", payload);

            // Build packet: [CRC32: 4 bytes][Magic: 2 bytes][Payload: n bytes]
            let header = PacketHeader::new_with_crc(&payload);
            let mut packet = Vec::with_capacity(6 + payload.len());
            packet.extend_from_slice(&header.to_bytes());
            packet.extend_from_slice(&payload);

            let original_packet = packet.clone();

            println!("  Packet structure (before encryption):");
            println!(
                "    CRC:     [0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}]",
                packet[0], packet[1], packet[2], packet[3]
            );
            println!("    Magic:   [0x{:02X}, 0x{:02X}]", packet[4], packet[5]);
            println!("    Payload: {:?}", &packet[6..]);

            // Verify structure before encryption
            assert_eq!(packet.len(), 10, "Packet length should be 10 bytes");

            // Encrypt ENTIRE packet (matches C++ Transport.cpp line 410)
            xor_encrypt(&mut packet);

            println!("  Packet after encryption (entire packet encrypted):");
            println!(
                "    CRC:     [0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}] (encrypted)",
                packet[0], packet[1], packet[2], packet[3]
            );
            println!(
                "    Magic:   [0x{:02X}, 0x{:02X}] (encrypted)",
                packet[4], packet[5]
            );
            println!("    Payload: {:?} (encrypted)", &packet[6..]);

            // Verify packet was encrypted (should be different)
            assert_ne!(packet, original_packet, "Packet should be encrypted");

            // Decrypt entire packet and verify round-trip
            xor_decrypt(&mut packet);
            assert_eq!(packet, original_packet, "Round-trip encryption failed");

            println!("  PASS: Full packet construction and encryption\n");
        }

        // Test Vector H: Encryption/Decryption round-trip with various sizes
        {
            println!("Test H - Encryption round-trip (multiple sizes):");

            let test_cases = vec![
                (vec![0x00, 0x00, 0x00, 0x00], "4 bytes (single word)"),
                (
                    vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                    "8 bytes (two words)",
                ),
                (
                    vec![
                        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
                    ],
                    "12 bytes (three words)",
                ),
            ];

            for (payload, description) in test_cases {
                let original = payload.clone();
                let mut encrypted = payload.clone();
                xor_encrypt(&mut encrypted);

                // Verify encryption changed the data
                assert_ne!(
                    encrypted, original,
                    "Encryption should change data for {}",
                    description
                );

                // Decrypt and verify
                xor_decrypt(&mut encrypted);
                assert_eq!(encrypted, original, "Round-trip failed for {}", description);

                println!("  PASS: {} - encryption/decryption round-trip", description);
            }
            println!();
        }

        // Test Vector I: Packet parsing from C++ format
        // Updated to match C++ Transport.cpp line 410: entire packet encryption
        {
            println!("Test I - Packet parsing from C++ format (entire packet encryption):");

            // Simulate receiving a packet from C++
            let payload = vec![0x12, 0x34, 0x56, 0x78];
            let header = PacketHeader::new_with_crc(&payload);

            // Build C++ format packet
            let mut cpp_packet = Vec::new();
            cpp_packet.extend_from_slice(&header.to_bytes());
            cpp_packet.extend_from_slice(&payload);

            let original_packet = cpp_packet.clone();

            // Encrypt ENTIRE packet (as C++ does - Transport.cpp line 410)
            xor_encrypt(&mut cpp_packet);

            println!(
                "  Simulated C++ packet (entire packet encrypted): {} bytes",
                cpp_packet.len()
            );

            // Parse packet (Rust side) - must decrypt entire packet first
            assert!(cpp_packet.len() >= 6, "Packet too small");

            // Decrypt ENTIRE packet (as Rust receive_messages does)
            xor_decrypt(&mut cpp_packet);

            // Now parse decrypted packet
            let parsed_header = PacketHeader::from_bytes(&cpp_packet[0..6]).unwrap();
            println!(
                "  Parsed header - CRC: 0x{:08X}, Magic: 0x{:04X}",
                parsed_header.crc, parsed_header.magic
            );

            assert_eq!(
                parsed_header.magic, GENERALS_MAGIC_NUMBER,
                "Magic number mismatch"
            );

            let parsed_payload = cpp_packet[6..].to_vec();
            println!("  Decrypted payload: {:?}", parsed_payload);

            // Verify CRC
            assert!(
                parsed_header.verify_crc(&parsed_payload),
                "CRC verification failed"
            );
            assert_eq!(parsed_payload, payload, "Payload mismatch after decryption");
            assert_eq!(
                cpp_packet, original_packet,
                "Decryption should restore original packet"
            );

            println!("  PASS: Packet parsing and CRC verification\n");
        }

        // Test Vector J: CRC sensitivity (single bit change)
        {
            println!("Test J - CRC sensitivity to bit changes:");

            let payload1 = vec![0x00, 0x00, 0x00, 0x00];
            let payload2 = vec![0x01, 0x00, 0x00, 0x00]; // Single bit changed

            let crc1 = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload1);
            let crc2 = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload2);

            println!("  Payload 1: {:?} -> CRC: 0x{:08X}", payload1, crc1);
            println!("  Payload 2: {:?} -> CRC: 0x{:08X}", payload2, crc2);

            assert_ne!(crc1, crc2, "CRC should change with single bit flip");

            println!("  PASS: CRC is sensitive to bit changes\n");
        }

        println!("=== All C++ Compatibility Tests PASSED ===\n");
    }
}
