//! Utility functions and helpers for networking
//!
//! This module provides common networking utilities including
//! address resolution, packet utilities, compression helpers, and more.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use if_addrs::get_if_addrs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

pub mod addressing;
pub mod compression;
pub mod serialization;
pub mod timing;

/// Network utility functions
pub struct NetworkUtils;

impl NetworkUtils {
    /// Determine the preferred local IP address for outbound traffic.
    pub fn get_local_ip() -> Option<IpAddr> {
        let addresses = Self::collect_local_ips();
        if addresses.is_empty() {
            return None;
        }

        if let Some(private_v4) = addresses
            .iter()
            .find(|addr| matches!(addr, IpAddr::V4(v4) if Self::is_private_v4(v4)))
        {
            return Some(*private_v4);
        }

        if let Some(v4) = addresses.iter().find(|addr| matches!(addr, IpAddr::V4(_))) {
            return Some(*v4);
        }

        addresses.into_iter().next()
    }

    /// Enumerate all local interface addresses ordered by preference.
    pub fn get_all_local_ips() -> Vec<IpAddr> {
        Self::collect_local_ips()
    }

    /// Resolve hostname to socket address
    pub async fn resolve_address(host: &str, port: u16) -> NetworkResult<SocketAddr> {
        let address_str = format!("{}:{}", host, port);

        let addresses: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
            address_str
                .to_socket_addrs()
                .map(|addrs| addrs.collect())
                .unwrap_or_else(|_| Vec::new())
        })
        .await
        .map_err(|e| NetworkError::generic(format!("failed to spawn resolver task: {}", e)))?;

        addresses
            .into_iter()
            .next()
            .ok_or_else(|| NetworkError::generic(format!("failed to resolve address: {}", host)))
    }

    /// Check if address is local/private
    pub fn is_local_address(addr: &IpAddr) -> bool {
        match addr {
            IpAddr::V4(ipv4) => {
                ipv4.is_loopback()
                    || ipv4.is_private()
                    || ipv4.is_link_local()
                    || ipv4.is_broadcast()
                    || ipv4.is_multicast()
            }
            IpAddr::V6(ipv6) => ipv6.is_loopback() || ipv6.is_multicast(),
        }
    }

    /// Get current timestamp in milliseconds
    pub fn current_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Validate port number
    pub fn is_valid_port(port: u16) -> bool {
        port > 0 && port < 65535
    }

    /// Generate random port in valid range
    pub fn random_port() -> u16 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(1024..65535)
    }

    /// Calculate packet overhead for protocol
    pub fn packet_overhead(protocol: crate::transport::TransportProtocol) -> usize {
        match protocol {
            crate::transport::TransportProtocol::Udp => 8, // UDP header
            crate::transport::TransportProtocol::Tcp => 20, // TCP header (minimum)
            crate::transport::TransportProtocol::WebSocket => 32, // WebSocket frame overhead
            crate::transport::TransportProtocol::Quic => 16, // QUIC header estimate
        }
    }

    /// Estimate bandwidth usage
    pub fn estimate_bandwidth(
        packets_per_second: f64,
        avg_packet_size: usize,
        protocol: crate::transport::TransportProtocol,
    ) -> f64 {
        let overhead = Self::packet_overhead(protocol);
        let total_packet_size = avg_packet_size + overhead + 20; // +20 for IP header

        packets_per_second * total_packet_size as f64
    }

    fn collect_local_ips() -> Vec<IpAddr> {
        match get_if_addrs() {
            Ok(interfaces) => {
                let mut candidates: Vec<IpAddr> = interfaces
                    .into_iter()
                    .filter_map(|iface| {
                        let ip = iface.ip();
                        if Self::is_valid_candidate(&ip) {
                            Some(ip)
                        } else {
                            None
                        }
                    })
                    .collect();

                candidates.sort_by_key(Self::priority_for);
                candidates.dedup();

                debug!("Enumerated local interfaces: {:?}", candidates);
                candidates
            }
            Err(err) => {
                warn!("Failed to enumerate local interfaces: {}", err);
                Vec::new()
            }
        }
    }

    fn is_valid_candidate(addr: &IpAddr) -> bool {
        match addr {
            IpAddr::V4(v4) => !v4.is_unspecified() && !v4.is_multicast() && !Self::is_apipa(v4),
            IpAddr::V6(v6) => !v6.is_unspecified() && !v6.is_multicast(),
        }
    }

    fn priority_for(addr: &IpAddr) -> (u8, IpAddr) {
        match addr {
            IpAddr::V4(v4) if Self::is_private_v4(v4) => (0, *addr),
            IpAddr::V4(v4) if v4.is_loopback() => (4, *addr),
            IpAddr::V4(_) => (1, *addr),
            IpAddr::V6(v6) if v6.is_loopback() => (5, *addr),
            IpAddr::V6(_) => (2, *addr),
        }
    }

    fn is_private_v4(addr: &Ipv4Addr) -> bool {
        addr.is_private()
    }

    fn is_apipa(addr: &Ipv4Addr) -> bool {
        addr.octets()[0] == 169 && addr.octets()[1] == 254
    }
}

/// Address validation and manipulation utilities
pub struct AddressUtils;

impl AddressUtils {
    /// Parse address string into SocketAddr
    pub fn parse_socket_addr(addr_str: &str) -> NetworkResult<SocketAddr> {
        addr_str.parse::<SocketAddr>().map_err(|e| {
            NetworkError::generic(format!("invalid socket address '{}': {}", addr_str, e))
        })
    }

    /// Create socket address from components
    pub fn create_socket_addr(ip: &str, port: u16) -> NetworkResult<SocketAddr> {
        let ip_addr = ip
            .parse::<IpAddr>()
            .map_err(|e| NetworkError::generic(format!("invalid IP address '{}': {}", ip, e)))?;

        Ok(SocketAddr::new(ip_addr, port))
    }

    /// Get local IP addresses
    pub async fn get_local_ips() -> Vec<IpAddr> {
        NetworkUtils::get_all_local_ips()
    }

    /// Check if address is reachable
    pub async fn is_reachable(addr: SocketAddr, timeout_ms: u64) -> bool {
        let timeout = std::time::Duration::from_millis(timeout_ms);

        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) => false,
            Err(_) => false, // Timeout
        }
    }

    /// Normalize address (resolve localhost, etc.)
    pub fn normalize_address(addr: SocketAddr) -> SocketAddr {
        match addr.ip() {
            IpAddr::V4(ipv4) if ipv4.is_loopback() => {
                // Replace loopback with actual local IP if needed
                addr
            }
            _ => addr,
        }
    }
}

/// Compression utilities
pub struct CompressionUtils;

impl CompressionUtils {
    /// Compress data using specified algorithm
    pub fn compress(data: &[u8], algorithm: CompressionAlgorithm) -> NetworkResult<Vec<u8>> {
        match algorithm {
            CompressionAlgorithm::Zlib => {
                use flate2::{write::ZlibEncoder, Compression};
                use std::io::Write;

                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder.write_all(data).map_err(|e| {
                    NetworkError::generic(format!("compression write error: {}", e))
                })?;
                encoder
                    .finish()
                    .map_err(|e| NetworkError::generic(format!("compression finish error: {}", e)))
            }
            CompressionAlgorithm::Lz4 => Ok(lz4_flex::compress_prepend_size(data)),
            CompressionAlgorithm::Zstd => zstd::encode_all(data, 3)
                .map_err(|e| NetworkError::generic(format!("Zstd compression error: {}", e))),
        }
    }

    /// Decompress data using specified algorithm
    pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> NetworkResult<Vec<u8>> {
        match algorithm {
            CompressionAlgorithm::Zlib => {
                use flate2::read::ZlibDecoder;
                use std::io::Read;

                let mut decoder = ZlibDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| NetworkError::generic(format!("decompression error: {}", e)))?;
                Ok(decompressed)
            }
            CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(data)
                .map_err(|e| NetworkError::generic(format!("LZ4 decompression error: {}", e))),
            CompressionAlgorithm::Zstd => zstd::decode_all(data)
                .map_err(|e| NetworkError::generic(format!("Zstd decompression error: {}", e))),
        }
    }

    /// Calculate compression ratio
    pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
        if original_size == 0 {
            return 0.0;
        }

        1.0 - (compressed_size as f64 / original_size as f64)
    }

    /// Check if compression is beneficial
    pub fn should_compress(data_size: usize, threshold: usize) -> bool {
        data_size >= threshold
    }
}

/// Compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// Zlib compression (good balance)
    Zlib,
    /// LZ4 compression (fast)
    Lz4,
    /// Zstd compression (high ratio)
    Zstd,
}

/// Timing utilities
pub struct TimingUtils;

impl TimingUtils {
    /// Convert duration to milliseconds
    pub fn duration_to_ms(duration: std::time::Duration) -> u64 {
        duration.as_millis() as u64
    }

    /// Create duration from milliseconds
    pub fn ms_to_duration(ms: u64) -> std::time::Duration {
        std::time::Duration::from_millis(ms)
    }

    /// Get high-resolution timestamp
    pub fn high_res_timestamp() -> u64 {
        NetworkInstant::now().as_duration().as_nanos() as u64
    }

    /// Calculate moving average
    pub fn moving_average(current_avg: f64, new_value: f64, weight: f64) -> f64 {
        (current_avg * (1.0 - weight)) + (new_value * weight)
    }

    /// Exponential backoff calculation
    pub fn exponential_backoff(attempt: u32, base_delay_ms: u64, max_delay_ms: u64) -> u64 {
        let delay = base_delay_ms * 2_u64.pow(attempt);
        delay.min(max_delay_ms)
    }
}

/// Serialization utilities
pub struct SerializationUtils;

impl SerializationUtils {
    /// Serialize to binary format
    pub fn serialize_binary<T: serde::Serialize>(value: &T) -> NetworkResult<Vec<u8>> {
        bincode::serialize(value)
            .map_err(|e| NetworkError::generic(format!("binary serialization error: {}", e)))
    }

    /// Deserialize from binary format
    pub fn deserialize_binary<T: for<'de> serde::Deserialize<'de>>(
        data: &[u8],
    ) -> NetworkResult<T> {
        bincode::deserialize(data)
            .map_err(|e| NetworkError::generic(format!("binary deserialization error: {}", e)))
    }

    /// Serialize to JSON format
    pub fn serialize_json<T: serde::Serialize>(value: &T) -> NetworkResult<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|e| NetworkError::generic(format!("JSON serialization error: {}", e)))
    }

    /// Deserialize from JSON format
    pub fn deserialize_json<T: for<'de> serde::Deserialize<'de>>(data: &[u8]) -> NetworkResult<T> {
        serde_json::from_slice(data)
            .map_err(|e| NetworkError::generic(format!("JSON deserialization error: {}", e)))
    }

    /// Serialize to MessagePack format
    pub fn serialize_msgpack<T: serde::Serialize>(value: &T) -> NetworkResult<Vec<u8>> {
        rmp_serde::to_vec(value)
            .map_err(|e| NetworkError::generic(format!("MessagePack serialization error: {}", e)))
    }

    /// Deserialize from MessagePack format
    pub fn deserialize_msgpack<T: for<'de> serde::Deserialize<'de>>(
        data: &[u8],
    ) -> NetworkResult<T> {
        rmp_serde::from_slice(data)
            .map_err(|e| NetworkError::generic(format!("MessagePack deserialization error: {}", e)))
    }
}

/// Network statistics tracker
pub struct StatsTracker {
    /// Sample count
    sample_count: u64,
    /// Running sum
    sum: f64,
    /// Running sum of squares (for variance)
    sum_squares: f64,
    /// Minimum value
    min_value: f64,
    /// Maximum value
    max_value: f64,
}

impl StatsTracker {
    /// Create new stats tracker
    pub fn new() -> Self {
        Self {
            sample_count: 0,
            sum: 0.0,
            sum_squares: 0.0,
            min_value: f64::INFINITY,
            max_value: f64::NEG_INFINITY,
        }
    }

    /// Add sample to tracker
    pub fn add_sample(&mut self, value: f64) {
        self.sample_count += 1;
        self.sum += value;
        self.sum_squares += value * value;

        if value < self.min_value {
            self.min_value = value;
        }

        if value > self.max_value {
            self.max_value = value;
        }
    }

    /// Get average value
    pub fn average(&self) -> f64 {
        if self.sample_count > 0 {
            self.sum / self.sample_count as f64
        } else {
            0.0
        }
    }

    /// Get variance
    pub fn variance(&self) -> f64 {
        if self.sample_count > 1 {
            let mean = self.average();
            (self.sum_squares - (self.sample_count as f64 * mean * mean))
                / (self.sample_count as f64 - 1.0)
        } else {
            0.0
        }
    }

    /// Get standard deviation
    pub fn standard_deviation(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Get minimum value
    pub fn min(&self) -> f64 {
        if self.sample_count > 0 {
            self.min_value
        } else {
            0.0
        }
    }

    /// Get maximum value
    pub fn max(&self) -> f64 {
        if self.sample_count > 0 {
            self.max_value
        } else {
            0.0
        }
    }

    /// Get sample count
    pub fn count(&self) -> u64 {
        self.sample_count
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.sample_count = 0;
        self.sum = 0.0;
        self.sum_squares = 0.0;
        self.min_value = f64::INFINITY;
        self.max_value = f64::NEG_INFINITY;
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_utils() {
        assert!(NetworkUtils::is_valid_port(8080));
        assert!(!NetworkUtils::is_valid_port(0));
        assert!(!NetworkUtils::is_valid_port(65535));

        let port = NetworkUtils::random_port();
        assert!(port >= 1024 && port < 65535);

        assert!(NetworkUtils::is_local_address(&IpAddr::V4(Ipv4Addr::new(
            127, 0, 0, 1
        ))));
        assert!(NetworkUtils::is_local_address(&IpAddr::V4(Ipv4Addr::new(
            192, 168, 1, 1
        ))));
        assert!(!NetworkUtils::is_local_address(&IpAddr::V4(Ipv4Addr::new(
            8, 8, 8, 8
        ))));
    }

    #[test]
    fn test_address_utils() {
        let addr = AddressUtils::parse_socket_addr("127.0.0.1:8080").unwrap();
        assert_eq!(addr.port(), 8080);
        assert!(addr.ip().is_loopback());

        let addr2 = AddressUtils::create_socket_addr("192.168.1.1", 9090).unwrap();
        assert_eq!(addr2.port(), 9090);

        assert!(AddressUtils::parse_socket_addr("invalid").is_err());
    }

    #[test]
    fn test_compression_utils() {
        let test_data = b"Hello, World! This is test data for compression.";

        // Test Zlib compression
        let compressed = CompressionUtils::compress(test_data, CompressionAlgorithm::Zlib).unwrap();
        assert!(compressed.len() > 0);

        let decompressed =
            CompressionUtils::decompress(&compressed, CompressionAlgorithm::Zlib).unwrap();
        assert_eq!(decompressed, test_data);

        // Test compression ratio
        let ratio = CompressionUtils::compression_ratio(test_data.len(), compressed.len());
        assert!(ratio <= 1.0);
        assert!(ratio >= -1.0);

        // Test compression threshold
        assert!(CompressionUtils::should_compress(1000, 500));
        assert!(!CompressionUtils::should_compress(100, 500));
    }

    #[test]
    fn test_timing_utils() {
        let duration = std::time::Duration::from_millis(1500);
        assert_eq!(TimingUtils::duration_to_ms(duration), 1500);

        let back_to_duration = TimingUtils::ms_to_duration(1500);
        assert_eq!(back_to_duration, duration);

        // Test moving average
        let avg = TimingUtils::moving_average(10.0, 20.0, 0.1);
        assert_eq!(avg, 11.0);

        // Test exponential backoff
        assert_eq!(TimingUtils::exponential_backoff(0, 100, 10000), 100);
        assert_eq!(TimingUtils::exponential_backoff(1, 100, 10000), 200);
        assert_eq!(TimingUtils::exponential_backoff(2, 100, 10000), 400);
        assert_eq!(TimingUtils::exponential_backoff(10, 100, 1000), 1000); // Capped at max
    }

    #[test]
    fn test_stats_tracker() {
        let mut tracker = StatsTracker::new();

        tracker.add_sample(10.0);
        tracker.add_sample(20.0);
        tracker.add_sample(30.0);

        assert_eq!(tracker.count(), 3);
        assert_eq!(tracker.average(), 20.0);
        assert_eq!(tracker.min(), 10.0);
        assert_eq!(tracker.max(), 30.0);

        let std_dev = tracker.standard_deviation();
        assert!(std_dev > 0.0);

        tracker.reset();
        assert_eq!(tracker.count(), 0);
        assert_eq!(tracker.average(), 0.0);
    }

    #[test]
    fn test_serialization_utils() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestData {
            id: u32,
            name: String,
        }

        let test_obj = TestData {
            id: 42,
            name: "test".to_string(),
        };

        // Test binary serialization
        let binary_data = SerializationUtils::serialize_binary(&test_obj).unwrap();
        let deserialized: TestData = SerializationUtils::deserialize_binary(&binary_data).unwrap();
        assert_eq!(deserialized, test_obj);

        // Test JSON serialization
        let json_data = SerializationUtils::serialize_json(&test_obj).unwrap();
        let deserialized_json: TestData = SerializationUtils::deserialize_json(&json_data).unwrap();
        assert_eq!(deserialized_json, test_obj);
    }

    #[test]
    fn enumerates_loopback_address() {
        let ips = NetworkUtils::get_all_local_ips();
        assert!(
            !ips.is_empty(),
            "expected at least one local interface address"
        );
        assert!(
            ips.iter().any(|ip| ip.is_loopback()),
            "expected a loopback address in enumeration"
        );
    }

    #[tokio::test]
    async fn address_utils_and_network_utils_align() {
        let from_utils = NetworkUtils::get_all_local_ips();
        let from_address_utils = AddressUtils::get_local_ips().await;
        assert_eq!(from_address_utils, from_utils);
    }
}
