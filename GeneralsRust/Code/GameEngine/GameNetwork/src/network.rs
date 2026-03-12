//! C++-style Network singleton wrapper (Network.cpp/NetworkInterface.h parity).

use std::net::{Ipv4Addr, ToSocketAddrs};
use std::sync::Arc;

use crate::error::{NetworkError, NetworkResult};
use crate::{
    get_network, init_network, shutdown_network, NetworkConfig, NetworkInterface, NetworkStats,
};

/// Resolve a host name or dotted-quad string into a host-order IPv4 address.
pub fn resolve_ip(host: &str) -> NetworkResult<u32> {
    if let Ok(addr) = host.parse::<Ipv4Addr>() {
        return Ok(u32::from(addr).to_be());
    }

    let addr = (host, 0)
        .to_socket_addrs()
        .map_err(|err| NetworkError::transport(format!("DNS lookup failed: {}", err)))?
        .find(|addr| addr.is_ipv4())
        .ok_or_else(|| NetworkError::transport("No IPv4 address resolved".to_string()))?;

    if let std::net::SocketAddr::V4(v4) = addr {
        Ok(u32::from(*v4.ip()).to_be())
    } else {
        Err(NetworkError::transport(
            "Resolved address is not IPv4".to_string(),
        ))
    }
}

/// Initialize the global network singleton.
pub async fn create_network(config: NetworkConfig) -> NetworkResult<Arc<NetworkInterface>> {
    init_network(config).await?;
    get_network().ok_or_else(|| NetworkError::generic("Network singleton missing".to_string()))
}

/// Access the global network singleton.
pub fn the_network() -> Option<Arc<NetworkInterface>> {
    get_network()
}

/// Shutdown the global network singleton.
pub async fn shutdown_the_network() -> NetworkResult<()> {
    shutdown_network().await
}

/// C++-style wrapper around the modern network interface.
pub struct Network {
    inner: Arc<NetworkInterface>,
}

impl Network {
    pub fn new(inner: Arc<NetworkInterface>) -> Self {
        Self { inner }
    }

    pub fn from_global() -> Option<Self> {
        get_network().map(Self::new)
    }

    pub fn inner(&self) -> &Arc<NetworkInterface> {
        &self.inner
    }

    pub async fn init(&self) -> NetworkResult<()> {
        Ok(())
    }

    pub async fn reset(&self) -> NetworkResult<()> {
        self.inner.reset_session().await
    }

    pub async fn update(&self) -> NetworkResult<()> {
        self.inner.update_concurrent().await
    }

    pub async fn liteupdate(&self) -> NetworkResult<()> {
        self.inner.update().await
    }

    pub async fn set_local_address(&self, ip: u32, port: u32) -> NetworkResult<()> {
        let ip = Ipv4Addr::from(ip.to_be());
        let port = port as u16;
        self.inner.set_local_address(ip, port).await
    }

    pub async fn is_frame_data_ready(&self) -> bool {
        self.inner.is_frame_data_ready().await
    }

    pub async fn parse_user_list(&self, players: &[crate::PlayerEndpoint]) -> NetworkResult<()> {
        self.inner.parse_user_list(players).await
    }

    pub async fn start_game(&self) -> NetworkResult<()> {
        self.inner.start_game().await
    }

    pub fn get_run_ahead(&self) -> u32 {
        self.inner.run_ahead()
    }

    pub fn get_frame_rate(&self) -> u32 {
        self.inner.frame_rate()
    }

    pub fn get_packet_arrival_cushion(&self) -> u32 {
        self.inner.packet_arrival_cushion().max(0.0).round() as u32
    }

    pub async fn send_chat(&self, text: String, player_mask: i32) -> NetworkResult<()> {
        let mask = if player_mask < 0 {
            0
        } else {
            player_mask as u8
        };
        self.inner.send_chat_message(text, mask).await
    }

    pub async fn send_disconnect_chat(&self, text: String) -> NetworkResult<()> {
        self.inner.send_disconnect_chat_message(text, 0).await
    }

    pub async fn send_file(
        &self,
        path: &str,
        player_mask: u8,
        command_id: u16,
    ) -> NetworkResult<()> {
        self.inner.send_file(path, player_mask, command_id).await
    }

    pub async fn send_file_announce(&self, path: &str, player_mask: u8) -> NetworkResult<u16> {
        self.inner.send_file_announce(path, player_mask).await
    }

    pub async fn get_file_transfer_progress(&self, player_id: u8, path: &str) -> i32 {
        self.inner.get_file_transfer_progress(player_id, path).await
    }

    pub async fn are_all_queues_empty(&self) -> bool {
        self.inner.are_all_queues_empty().await
    }

    pub async fn quit_game(&self) -> NetworkResult<()> {
        self.inner.quit_game().await
    }

    pub async fn self_destruct_player(&self, index: i32) -> NetworkResult<()> {
        if index < 0 {
            return Err(NetworkError::generic("invalid player index".to_string()));
        }
        self.inner.self_destruct_player(index as u8).await
    }

    pub async fn vote_for_player_disconnect(&self, slot: i32) -> NetworkResult<()> {
        if slot < 0 {
            return Err(NetworkError::generic("invalid player slot".to_string()));
        }
        self.inner.vote_for_player_disconnect(slot as u8).await?;
        Ok(())
    }

    pub fn is_packet_router(&self) -> bool {
        self.inner.is_packet_router()
    }

    pub async fn get_incoming_bytes_per_second(&self) -> f32 {
        self.inner.incoming_bytes_per_second().await
    }

    pub async fn get_incoming_packets_per_second(&self) -> f32 {
        self.inner.incoming_packets_per_second().await
    }

    pub async fn get_outgoing_bytes_per_second(&self) -> f32 {
        self.inner.outgoing_bytes_per_second().await
    }

    pub async fn get_outgoing_packets_per_second(&self) -> f32 {
        self.inner.outgoing_packets_per_second().await
    }

    pub fn get_unknown_bytes_per_second(&self) -> f32 {
        self.inner.unknown_bytes_per_second()
    }

    pub fn get_unknown_packets_per_second(&self) -> f32 {
        self.inner.unknown_packets_per_second()
    }

    pub async fn update_load_progress(&self, percent: i32) {
        let clamped = percent.clamp(0, 100) as u8;
        self.inner.update_load_progress(clamped).await
    }

    pub async fn load_progress_complete(&self) {
        self.inner.load_progress_complete().await
    }

    pub async fn send_time_out_game_start(&self) -> NetworkResult<()> {
        self.inner.send_timeout_game_start().await
    }

    pub fn get_local_player_id(&self) -> u32 {
        self.inner.local_player_id() as u32
    }

    pub async fn get_player_name(&self, player_num: i32) -> String {
        if player_num < 0 {
            return "Player".to_string();
        }
        self.inner.player_name(player_num as u8).await
    }

    pub async fn get_num_players(&self) -> i32 {
        self.inner.num_players().await as i32
    }

    pub async fn get_average_fps(&self) -> i32 {
        self.inner.average_fps().await
    }

    pub async fn get_slot_average_fps(&self, slot: i32) -> i32 {
        if slot < 0 {
            return 0;
        }
        self.inner.slot_average_fps(slot as u8).await
    }

    pub async fn saw_crc_mismatch(&self) -> bool {
        self.inner.saw_crc_mismatch()
    }

    pub fn set_saw_crc_mismatch(&self) {
        self.inner.set_saw_crc_mismatch();
    }

    pub async fn is_player_connected(&self, player_id: i32) -> bool {
        if player_id < 0 {
            return false;
        }
        self.inner.is_player_connected(player_id as u8).await
    }

    pub async fn notify_others_of_current_frame(&self) -> NetworkResult<()> {
        self.inner.notify_others_of_current_frame().await
    }

    pub async fn notify_others_of_new_frame(&self, frame: u32) -> NetworkResult<()> {
        self.inner.notify_others_of_new_frame(frame).await
    }

    pub fn get_execution_frame(&self) -> u32 {
        self.inner.execution_frame()
    }

    pub fn get_ping_frame(&self) -> u32 {
        self.inner.ping_frame()
    }

    pub fn get_pings_sent(&self) -> i32 {
        self.inner.pings_sent()
    }

    pub fn get_pings_received(&self) -> i32 {
        self.inner.pings_received()
    }

    pub async fn get_stats(&self) -> NetworkStats {
        self.inner.get_stats().await
    }
}
