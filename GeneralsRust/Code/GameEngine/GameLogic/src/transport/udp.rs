// UDP Transport implementation matching C++ Generals
//
// This provides reliable UDP communication with:
// - Packet encryption/decryption
// - CRC validation
// - Retransmission with 2000ms timeout
// - Keep-alive packets every 20 seconds
// - ACK tracking (ACKSTAGE1, ACKSTAGE2)

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use super::{
    decrypt_buffer, encrypt_buffer, TransportMessage, TransportMessageHeader,
    GENERALS_MAGIC_NUMBER, KEEPALIVE_INTERVAL_SECS, MAX_MESSAGES, MAX_MESSAGE_LEN, RETRY_TIME_MS,
};

/// Result type for network operations
pub type NetworkResult<T> = Result<T, NetworkError>;

/// Network error types
#[derive(Debug, Clone)]
pub enum NetworkError {
    SocketError(String),
    InvalidPacket(String),
    BufferFull,
    Timeout,
    InvalidAddress,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::SocketError(s) => write!(f, "Socket error: {}", s),
            NetworkError::InvalidPacket(s) => write!(f, "Invalid packet: {}", s),
            NetworkError::BufferFull => write!(f, "Buffer is full"),
            NetworkError::Timeout => write!(f, "Operation timed out"),
            NetworkError::InvalidAddress => write!(f, "Invalid address"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Pending packet awaiting acknowledgment
#[derive(Debug, Clone)]
struct PendingPacket {
    /// The packet data
    data: Vec<u8>,
    /// Destination address
    addr: SocketAddr,
    /// Last time this packet was sent
    last_sent: Instant,
    /// Number of times this packet has been retried
    retry_count: usize,
    /// Maximum number of retries before giving up
    max_retries: usize,
}

/// UDP Transport layer matching C++ implementation
///
/// This handles:
/// - Sending/receiving UDP packets
/// - Packet encryption/decryption
/// - CRC validation
/// - Reliable delivery with retransmission
/// - Keep-alive packets
pub struct UdpTransport {
    /// UDP socket
    socket: UdpSocket,

    /// Local address this transport is bound to
    local_addr: SocketAddr,

    /// Outgoing message buffer (matches C++ m_outBuffer)
    out_buffer: Vec<TransportMessage>,

    /// Incoming message buffer (matches C++ m_inBuffer)
    in_buffer: Vec<TransportMessage>,

    /// Pending packets awaiting ACK
    pending_acks: HashMap<u16, PendingPacket>,

    /// Next packet ID for outgoing packets
    next_packet_id: u16,

    /// Last time a keep-alive was sent to each peer
    last_keepalive: HashMap<SocketAddr, Instant>,

    /// Statistics: incoming bytes/packets
    stats_incoming_bytes: u64,
    stats_incoming_packets: u64,

    /// Statistics: outgoing bytes/packets
    stats_outgoing_bytes: u64,
    stats_outgoing_packets: u64,

    /// Statistics: unknown/invalid packets
    stats_unknown_bytes: u64,
    stats_unknown_packets: u64,
}

impl UdpTransport {
    /// Create a new UDP transport bound to the specified address
    ///
    /// This matches the C++ Transport::init() function
    pub fn new(bind_addr: SocketAddr) -> NetworkResult<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|e| NetworkError::SocketError(format!("Failed to bind: {}", e)))?;

        // Set non-blocking mode (matches C++ SetBlocking(FALSE))
        socket
            .set_nonblocking(true)
            .map_err(|e| NetworkError::SocketError(format!("Failed to set non-blocking: {}", e)))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| NetworkError::SocketError(format!("Failed to get local addr: {}", e)))?;

        Ok(Self {
            socket,
            local_addr,
            out_buffer: Vec::with_capacity(MAX_MESSAGES),
            in_buffer: Vec::with_capacity(MAX_MESSAGES),
            pending_acks: HashMap::new(),
            next_packet_id: 1,
            last_keepalive: HashMap::new(),
            stats_incoming_bytes: 0,
            stats_incoming_packets: 0,
            stats_outgoing_bytes: 0,
            stats_outgoing_packets: 0,
            stats_unknown_bytes: 0,
            stats_unknown_packets: 0,
        })
    }

    pub fn out_buffer_len(&self) -> usize {
        self.out_buffer.len()
    }

    pub fn pending_ack_len(&self) -> usize {
        self.pending_acks.len()
    }

    pub fn has_pending_ack(&self, packet_id: u16) -> bool {
        self.pending_acks.contains_key(&packet_id)
    }

    /// Get the local address this transport is bound to
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Queue a packet for sending
    ///
    /// This matches C++ Transport::queueSend()
    pub fn queue_send(&mut self, addr: SocketAddr, data: &[u8]) -> NetworkResult<()> {
        if data.len() > MAX_MESSAGE_LEN {
            return Err(NetworkError::InvalidPacket(format!(
                "Data size {} exceeds MAX_MESSAGE_LEN {}",
                data.len(),
                MAX_MESSAGE_LEN
            )));
        }

        if self.out_buffer.len() >= MAX_MESSAGES {
            return Err(NetworkError::BufferFull);
        }

        let mut msg = TransportMessage::new();
        msg.set_address(addr);
        msg.set_data(data)
            .map_err(|e| NetworkError::InvalidPacket(e.to_string()))?;

        // Compute CRC
        msg.compute_crc();

        self.out_buffer.push(msg);
        Ok(())
    }

    /// Send all queued packets
    ///
    /// This matches C++ Transport::doSend()
    pub fn do_send(&mut self) -> NetworkResult<()> {
        let mut i = 0;
        while i < self.out_buffer.len() {
            let msg = &mut self.out_buffer[i];

            if msg.length == 0 {
                // Empty slot, skip
                i += 1;
                continue;
            }

            // Get the full packet (header + data)
            let total_len = msg.total_size();
            let mut packet_buf = vec![0u8; total_len];

            // Copy header
            unsafe {
                let header_ptr = &msg.header as *const TransportMessageHeader as *const u8;
                let header_slice =
                    std::slice::from_raw_parts(header_ptr, TransportMessageHeader::SIZE);
                packet_buf[..TransportMessageHeader::SIZE].copy_from_slice(header_slice);
            }

            // Copy data
            packet_buf[TransportMessageHeader::SIZE..].copy_from_slice(msg.get_data());

            // Encrypt the packet (matches C++ encryptBuf)
            encrypt_buffer(&mut packet_buf, total_len);

            // Send the packet
            let addr = msg.get_address().ok_or(NetworkError::InvalidAddress)?;
            match self.socket.send_to(&packet_buf, addr) {
                Ok(bytes_sent) => {
                    self.stats_outgoing_packets += 1;
                    self.stats_outgoing_bytes += bytes_sent as u64;

                    // Remove from buffer (matches C++ setting length to 0)
                    self.out_buffer.swap_remove(i);
                    // Don't increment i since we removed this element
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Socket not ready, try again later
                    i += 1;
                }
                Err(e) => {
                    return Err(NetworkError::SocketError(format!("Send failed: {}", e)));
                }
            }
        }

        Ok(())
    }

    /// Receive all pending packets
    ///
    /// This matches C++ Transport::doRecv()
    pub fn do_recv(&mut self) -> NetworkResult<()> {
        let mut buf = vec![0u8; MAX_MESSAGE_LEN + TransportMessageHeader::SIZE];

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    // Decrypt the packet (matches C++ decryptBuf)
                    decrypt_buffer(&mut buf, len);

                    // Parse the packet
                    if len <= TransportMessageHeader::SIZE {
                        self.stats_unknown_packets += 1;
                        self.stats_unknown_bytes += len as u64;
                        continue;
                    }

                    // Parse header
                    let header: TransportMessageHeader =
                        unsafe { std::ptr::read(buf.as_ptr() as *const TransportMessageHeader) };

                    // Create message
                    let mut msg = TransportMessage::new();
                    msg.header = header;
                    msg.length = (len - TransportMessageHeader::SIZE) as i32;
                    msg.data[..msg.length as usize]
                        .copy_from_slice(&buf[TransportMessageHeader::SIZE..len]);
                    msg.set_address(from);

                    // Validate packet (matches C++ isGeneralsPacket)
                    if !msg.is_valid_packet() {
                        self.stats_unknown_packets += 1;
                        self.stats_unknown_bytes += len as u64;
                        continue;
                    }

                    // Valid packet - add to in_buffer
                    if self.in_buffer.len() < MAX_MESSAGES {
                        self.in_buffer.push(msg);
                        self.stats_incoming_packets += 1;
                        self.stats_incoming_bytes += len as u64;
                    }
                    // If buffer is full, packet is dropped (matches C++ behavior)
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more data available
                    break;
                }
                Err(e) => {
                    return Err(NetworkError::SocketError(format!("Receive failed: {}", e)));
                }
            }
        }

        Ok(())
    }

    /// Get the next incoming message, if available
    pub fn recv_message(&mut self) -> Option<TransportMessage> {
        if self.in_buffer.is_empty() {
            None
        } else {
            Some(self.in_buffer.remove(0))
        }
    }

    /// Send a packet and track it for retransmission
    ///
    /// This is used for reliable delivery with ACKs
    pub fn send_with_ack(
        &mut self,
        addr: SocketAddr,
        data: &[u8],
        max_retries: usize,
    ) -> NetworkResult<u16> {
        let packet_id = self.next_packet_id;
        self.next_packet_id = self.next_packet_id.wrapping_add(1);

        // Queue for immediate send
        self.queue_send(addr, data)?;

        // Track for retransmission
        let pending = PendingPacket {
            data: data.to_vec(),
            addr,
            last_sent: Instant::now(),
            retry_count: 0,
            max_retries,
        };

        self.pending_acks.insert(packet_id, pending);
        Ok(packet_id)
    }

    /// Process pending ACKs and retransmit if necessary
    ///
    /// This should be called regularly (e.g., every frame)
    pub fn process_pending_acks(&mut self) -> NetworkResult<()> {
        let now = Instant::now();
        let retry_timeout = Duration::from_millis(RETRY_TIME_MS);

        let mut to_retry = Vec::new();

        for (packet_id, pending) in &mut self.pending_acks {
            if now.duration_since(pending.last_sent) >= retry_timeout {
                if pending.retry_count >= pending.max_retries {
                    // Give up on this packet
                    to_retry.push((*packet_id, None));
                } else {
                    // Retry the packet
                    pending.retry_count += 1;
                    pending.last_sent = now;
                    to_retry.push((*packet_id, Some(pending.clone())));
                }
            }
        }

        // Process retries
        for (packet_id, retry) in to_retry {
            if let Some(pending) = retry {
                // Retransmit
                self.queue_send(pending.addr, &pending.data)?;
                self.pending_acks.insert(packet_id, pending);
            } else {
                // Remove failed packet
                self.pending_acks.remove(&packet_id);
            }
        }

        Ok(())
    }

    /// Acknowledge receipt of a packet (remove from pending)
    pub fn acknowledge_packet(&mut self, packet_id: u16) -> bool {
        self.pending_acks.remove(&packet_id).is_some()
    }

    /// Send keep-alive packets to all known peers
    ///
    /// This matches the C++ keep-alive mechanism (20 second interval)
    pub fn send_keepalives(&mut self, peers: &[SocketAddr]) -> NetworkResult<()> {
        let now = Instant::now();
        let keepalive_interval = Duration::from_secs(KEEPALIVE_INTERVAL_SECS);

        for &peer in peers {
            let should_send = self
                .last_keepalive
                .get(&peer)
                .map(|last| now.duration_since(*last) >= keepalive_interval)
                .unwrap_or(true);

            if should_send {
                // Send minimal keep-alive packet (empty data)
                self.queue_send(peer, &[])?;
                self.last_keepalive.insert(peer, now);
            }
        }

        Ok(())
    }

    /// Update the transport (call once per frame)
    ///
    /// This matches C++ Transport::update()
    pub fn update(&mut self) -> NetworkResult<()> {
        self.do_recv()?;
        self.do_send()?;
        Ok(())
    }

    /// Get statistics
    pub fn get_stats(&self) -> TransportStats {
        TransportStats {
            incoming_bytes: self.stats_incoming_bytes,
            incoming_packets: self.stats_incoming_packets,
            outgoing_bytes: self.stats_outgoing_bytes,
            outgoing_packets: self.stats_outgoing_packets,
            unknown_bytes: self.stats_unknown_bytes,
            unknown_packets: self.stats_unknown_packets,
            pending_acks: self.pending_acks.len(),
        }
    }

    /// Reset the transport
    pub fn reset(&mut self) {
        self.out_buffer.clear();
        self.in_buffer.clear();
        self.pending_acks.clear();
        self.last_keepalive.clear();
        self.next_packet_id = 1;
    }
}

/// Transport statistics
#[derive(Debug, Clone, Copy)]
pub struct TransportStats {
    pub incoming_bytes: u64,
    pub incoming_packets: u64,
    pub outgoing_bytes: u64,
    pub outgoing_packets: u64,
    pub unknown_bytes: u64,
    pub unknown_packets: u64,
    pub pending_acks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_create() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let transport = UdpTransport::new(addr).unwrap();
        assert_eq!(transport.local_addr.ip(), addr.ip());
    }

    #[test]
    fn test_queue_send() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut transport = UdpTransport::new(addr).unwrap();

        let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let data = b"Test message";

        transport.queue_send(dest, data).unwrap();
        assert_eq!(transport.out_buffer.len(), 1);
    }

    #[test]
    fn test_queue_send_too_large() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut transport = UdpTransport::new(addr).unwrap();

        let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let data = vec![0u8; MAX_MESSAGE_LEN + 1];

        assert!(transport.queue_send(dest, &data).is_err());
    }

    #[test]
    fn test_send_with_ack() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut transport = UdpTransport::new(addr).unwrap();

        let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let data = b"Test message";

        let packet_id = transport.send_with_ack(dest, data, 5).unwrap();
        assert_eq!(transport.pending_acks.len(), 1);
        assert!(transport.pending_acks.contains_key(&packet_id));
    }

    #[test]
    fn test_acknowledge_packet() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut transport = UdpTransport::new(addr).unwrap();

        let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let data = b"Test message";

        let packet_id = transport.send_with_ack(dest, data, 5).unwrap();
        assert!(transport.acknowledge_packet(packet_id));
        assert_eq!(transport.pending_acks.len(), 0);
    }

    #[test]
    fn test_reset() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut transport = UdpTransport::new(addr).unwrap();

        let dest: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        transport.queue_send(dest, b"Test").unwrap();
        transport.send_with_ack(dest, b"Test2", 5).unwrap();

        transport.reset();

        assert_eq!(transport.out_buffer.len(), 0);
        assert_eq!(transport.in_buffer.len(), 0);
        assert_eq!(transport.pending_acks.len(), 0);
    }
}
