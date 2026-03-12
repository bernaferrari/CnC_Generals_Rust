// Packet structures matching C++ Generals implementation
//
// These structures must be byte-for-byte compatible with the C++ implementation.
// We use #[repr(C, packed)] to ensure exact memory layout.

use std::net::SocketAddr;

use super::{Crc, GENERALS_MAGIC_NUMBER};

/// Maximum packet data size (UDP header + IP header = 36 bytes, total packet = 512, so 512 - 36 = 476)
pub const MAX_PACKET_SIZE: usize = 476;

/// Maximum message length in bytes
pub const MAX_MESSAGE_LEN: usize = 1024;

/// Transport message header - MUST match C++ TransportMessageHeader exactly
///
/// C++ definition:
/// ```c++
/// #pragma pack(push, 1)
/// struct TransportMessageHeader {
///     UnsignedInt crc;        // packet-level CRC (must be first in packet)
///     UnsignedShort magic;    // Magic number identifying Generals packets
/// };
/// #pragma pack(pop)
/// ```
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TransportMessageHeader {
    /// CRC checksum (MUST be first field for validation)
    pub crc: u32,
    /// Magic number (0xF00D for Generals packets)
    pub magic: u16,
}

impl TransportMessageHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Create a new header with the magic number set
    pub fn new() -> Self {
        Self {
            crc: 0,
            magic: GENERALS_MAGIC_NUMBER,
        }
    }

    /// Validate that this is a Generals packet
    pub fn is_valid(&self) -> bool {
        self.magic == GENERALS_MAGIC_NUMBER
    }
}

impl Default for TransportMessageHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Transport message structure - MUST match C++ TransportMessage exactly
///
/// C++ definition:
/// ```c++
/// #pragma pack(push, 1)
/// struct TransportMessage {
///     TransportMessageHeader header;
///     UnsignedByte data[MAX_MESSAGE_LEN];
///     Int length;
///     UnsignedInt addr;
///     UnsignedShort port;
/// };
/// #pragma pack(pop)
/// ```
///
/// NOTE: In C++, this struct has padding after the header to align data.
/// We use repr(C) to match C++ alignment rules.
#[repr(C)]
#[derive(Clone)]
pub struct TransportMessage {
    /// Packet header (CRC + magic)
    pub header: TransportMessageHeader,
    /// Packet data payload
    pub data: [u8; MAX_MESSAGE_LEN],
    /// Length of valid data in bytes
    pub length: i32,
    /// Destination/source IP address (network byte order)
    pub addr: u32,
    /// Destination/source port (network byte order)
    pub port: u16,
}

impl TransportMessage {
    /// Create a new empty transport message
    pub fn new() -> Self {
        Self {
            header: TransportMessageHeader::new(),
            data: [0; MAX_MESSAGE_LEN],
            length: 0,
            addr: 0,
            port: 0,
        }
    }

    /// Set the destination address and port
    pub fn set_address(&mut self, socket_addr: SocketAddr) {
        match socket_addr {
            SocketAddr::V4(addr) => {
                // Convert IP to u32 (network byte order)
                self.addr = u32::from_be_bytes(addr.ip().octets());
                self.port = addr.port();
            }
            SocketAddr::V6(_) => {
                // IPv6 not supported in original C++ implementation
                panic!("IPv6 not supported");
            }
        }
    }

    /// Get the socket address
    pub fn get_address(&self) -> Option<SocketAddr> {
        if self.addr == 0 || self.port == 0 {
            return None;
        }

        let octets = self.addr.to_be_bytes();
        let ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]);
        Some(SocketAddr::from((ip, self.port)))
    }

    /// Set the packet data
    pub fn set_data(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > MAX_MESSAGE_LEN {
            return Err("Data exceeds MAX_MESSAGE_LEN");
        }

        self.length = data.len() as i32;
        self.data[..data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Get the valid data slice
    pub fn get_data(&self) -> &[u8] {
        if self.length < 0 || self.length as usize > MAX_MESSAGE_LEN {
            return &[];
        }
        &self.data[..self.length as usize]
    }

    /// Compute and set the CRC for this message
    ///
    /// This matches the C++ implementation:
    /// ```c++
    /// CRC crc;
    /// crc.computeCRC((unsigned char *)(&(m_outBuffer[i].header.magic)),
    ///                m_outBuffer[i].length + sizeof(TransportMessageHeader) - sizeof(UnsignedInt));
    /// m_outBuffer[i].header.crc = crc.get();
    /// ```
    pub fn compute_crc(&mut self) {
        let mut crc = Crc::new();

        // CRC starts from the magic field (skipping the CRC field itself)
        let magic_offset = std::mem::offset_of!(TransportMessageHeader, magic);

        // Create a temporary buffer with header + data
        let crc_len = (TransportMessageHeader::SIZE - magic_offset) + self.length as usize;
        let mut buf = Vec::with_capacity(crc_len);

        // Add magic number (2 bytes)
        buf.extend_from_slice(&self.header.magic.to_le_bytes());

        // Add data
        buf.extend_from_slice(&self.data[..self.length as usize]);

        // Compute CRC
        crc.compute(&buf);
        self.header.crc = crc.get();
    }

    /// Validate the CRC of this message
    ///
    /// This matches the C++ implementation:
    /// ```c++
    /// CRC crc;
    /// crc.computeCRC((unsigned char *)(&(msg->header.magic)),
    ///                msg->length + sizeof(TransportMessageHeader) - sizeof(UnsignedInt));
    /// if (crc.get() != msg->header.crc)
    ///     return false;
    /// ```
    pub fn validate_crc(&self) -> bool {
        if self.length < 0 || self.length as usize > MAX_MESSAGE_LEN {
            return false;
        }

        let mut crc = Crc::new();

        // CRC starts from the magic field (skipping the CRC field itself)
        let magic_offset = std::mem::offset_of!(TransportMessageHeader, magic);
        let crc_len = (TransportMessageHeader::SIZE - magic_offset) + self.length as usize;

        let mut buf = Vec::with_capacity(crc_len);
        buf.extend_from_slice(&self.header.magic.to_le_bytes());
        buf.extend_from_slice(&self.data[..self.length as usize]);

        crc.compute(&buf);
        crc.get() == self.header.crc
    }

    /// Check if this is a valid Generals packet
    pub fn is_valid_packet(&self) -> bool {
        // Check length bounds
        if self.length < 0 || self.length as usize > MAX_MESSAGE_LEN {
            return false;
        }

        // Check magic number
        if !self.header.is_valid() {
            return false;
        }

        // Validate CRC
        self.validate_crc()
    }

    /// Get the total packet size (header + data length)
    pub fn total_size(&self) -> usize {
        TransportMessageHeader::SIZE + self.length as usize
    }
}

impl Default for TransportMessage {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TransportMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let magic = self.header.magic;
        let crc = self.header.crc;
        f.debug_struct("TransportMessage")
            .field("magic", &format!("0x{:04X}", magic))
            .field("crc", &format!("0x{:08X}", crc))
            .field("length", &self.length)
            .field("addr", &self.get_address())
            .field("data", &format!("{} bytes", self.length))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        // C++ sizeof(TransportMessageHeader) = 6 bytes (4 + 2)
        assert_eq!(TransportMessageHeader::SIZE, 6);
        assert_eq!(std::mem::size_of::<TransportMessageHeader>(), 6);
    }

    #[test]
    fn test_header_magic() {
        let header = TransportMessageHeader::new();
        let magic = unsafe { std::ptr::addr_of!(header.magic).read_unaligned() };
        assert_eq!(magic, 0xF00D);
        assert!(header.is_valid());
    }

    #[test]
    fn test_message_new() {
        let msg = TransportMessage::new();
        assert_eq!(msg.length, 0);
        assert_eq!(msg.addr, 0);
        assert_eq!(msg.port, 0);
        let magic = unsafe { std::ptr::addr_of!(msg.header.magic).read_unaligned() };
        assert_eq!(magic, GENERALS_MAGIC_NUMBER);
    }

    #[test]
    fn test_message_set_get_data() {
        let mut msg = TransportMessage::new();
        let data = b"Hello, World!";

        msg.set_data(data).unwrap();
        assert_eq!(msg.length, data.len() as i32);
        assert_eq!(msg.get_data(), data);
    }

    #[test]
    fn test_message_data_too_large() {
        let mut msg = TransportMessage::new();
        let data = vec![0u8; MAX_MESSAGE_LEN + 1];

        assert!(msg.set_data(&data).is_err());
    }

    #[test]
    fn test_message_crc_compute() {
        let mut msg = TransportMessage::new();
        msg.set_data(b"Test data").unwrap();

        msg.compute_crc();
        let crc = unsafe { std::ptr::addr_of!(msg.header.crc).read_unaligned() };
        assert_ne!(crc, 0);
    }

    #[test]
    fn test_message_crc_validate() {
        let mut msg = TransportMessage::new();
        msg.set_data(b"Test data").unwrap();
        msg.compute_crc();

        assert!(msg.validate_crc());
        assert!(msg.is_valid_packet());
    }

    #[test]
    fn test_message_crc_invalid() {
        let mut msg = TransportMessage::new();
        msg.set_data(b"Test data").unwrap();
        msg.compute_crc();

        // Corrupt the CRC
        msg.header.crc ^= 0xFFFF;

        assert!(!msg.validate_crc());
        assert!(!msg.is_valid_packet());
    }

    #[test]
    fn test_message_crc_deterministic() {
        let mut msg1 = TransportMessage::new();
        msg1.set_data(b"Same data").unwrap();
        msg1.compute_crc();

        let mut msg2 = TransportMessage::new();
        msg2.set_data(b"Same data").unwrap();
        msg2.compute_crc();

        let crc1 = unsafe { std::ptr::addr_of!(msg1.header.crc).read_unaligned() };
        let crc2 = unsafe { std::ptr::addr_of!(msg2.header.crc).read_unaligned() };
        assert_eq!(crc1, crc2);
    }

    #[test]
    fn test_message_set_address() {
        let mut msg = TransportMessage::new();
        let addr: SocketAddr = "192.168.1.100:8088".parse().unwrap();

        msg.set_address(addr);

        let retrieved = msg.get_address().unwrap();
        assert_eq!(retrieved, addr);
    }

    #[test]
    fn test_message_invalid_length() {
        let mut msg = TransportMessage::new();
        msg.length = -1;

        assert!(!msg.is_valid_packet());
    }

    #[test]
    fn test_message_length_exceeds_max() {
        let mut msg = TransportMessage::new();
        msg.length = (MAX_MESSAGE_LEN + 1) as i32;

        assert!(!msg.is_valid_packet());
    }

    #[test]
    fn test_total_size() {
        let mut msg = TransportMessage::new();
        msg.set_data(b"Test").unwrap();

        assert_eq!(msg.total_size(), TransportMessageHeader::SIZE + 4);
    }
}
