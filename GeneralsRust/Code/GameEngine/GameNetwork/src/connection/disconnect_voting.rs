//! Disconnect voting and consensus mechanisms
//!
//! This module implements voting mechanisms for disconnecting problematic players
//! in multiplayer games, ensuring fair and democratic removal of players who
//! are causing network issues or exhibiting poor behavior.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info};
use uuid::Uuid;

/// Disconnect vote configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectVoteConfig {
    /// Minimum votes required for disconnect
    pub minimum_votes_required: u32,
    /// Percentage of players needed to vote (0.0 to 1.0)
    pub required_vote_percentage: f64,
    /// Vote timeout duration
    pub vote_timeout: Duration,
    /// Cool-down period between votes for same player
    pub vote_cooldown: Duration,
    /// Maximum concurrent votes
    pub max_concurrent_votes: u32,
    /// Enable automatic voting for network issues
    pub enable_automatic_votes: bool,
    /// Network threshold for automatic voting (packet loss %)
    pub auto_vote_packet_loss_threshold: f64,
    /// Latency threshold for automatic voting (milliseconds)
    pub auto_vote_latency_threshold: f64,
}

impl Default for DisconnectVoteConfig {
    fn default() -> Self {
        Self {
            minimum_votes_required: 2,
            required_vote_percentage: 0.6, // 60% of players
            vote_timeout: Duration::from_secs(30),
            vote_cooldown: Duration::from_secs(120),
            max_concurrent_votes: 3,
            enable_automatic_votes: true,
            auto_vote_packet_loss_threshold: 10.0, // 10% packet loss
            auto_vote_latency_threshold: 1000.0,   // 1000ms latency
        }
    }
}

/// Reason for disconnect vote
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisconnectReason {
    /// High network latency
    HighLatency { latency_ms: u32 },
    /// Packet loss issues
    PacketLoss { loss_percentage: f32 },
    /// Player not responding
    Unresponsive,
    /// Connection timeout
    Timeout,
    /// Suspected cheating/anti-cheat violation
    AntiCheat { violation: String },
    /// Player requested by other players
    PlayerRequest { reason: String },
    /// Network desynchronization
    Desync,
    /// Protocol violations
    ProtocolViolation { details: String },
    /// Manual admin action
    AdminAction { admin_player: u8 },
}

impl DisconnectReason {
    /// Get severity level for prioritizing votes
    pub fn severity(&self) -> u8 {
        match self {
            Self::AntiCheat { .. } => 10,
            Self::ProtocolViolation { .. } => 9,
            Self::AdminAction { .. } => 8,
            Self::Desync => 7,
            Self::Unresponsive => 6,
            Self::Timeout => 5,
            Self::HighLatency { .. } => 4,
            Self::PacketLoss { .. } => 3,
            Self::PlayerRequest { .. } => 2,
        }
    }

    /// Check if reason should trigger automatic vote
    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            Self::AntiCheat { .. }
                | Self::ProtocolViolation { .. }
                | Self::Desync
                | Self::Unresponsive
        )
    }
}

/// Vote cast by a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerVote {
    /// Player casting the vote
    pub voter_id: u8,
    /// Vote decision
    pub vote: VoteDecision,
    /// When the vote was cast
    pub cast_at: DateTime<Utc>,
    /// Optional comment/reason from voter
    pub comment: Option<String>,
}

/// Vote decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteDecision {
    /// Vote to disconnect the player
    Disconnect,
    /// Vote to keep the player connected
    Keep,
    /// Abstain from voting
    Abstain,
}

/// Current disconnect vote state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectVote {
    /// Unique vote identifier
    pub vote_id: Uuid,
    /// Player being voted on
    pub target_player: u8,
    /// Reason for the vote
    pub reason: DisconnectReason,
    /// Player who initiated the vote
    pub initiator: u8,
    /// When the vote was started
    pub started_at: DateTime<Utc>,
    /// Vote timeout
    pub timeout: Duration,
    /// Players eligible to vote
    pub eligible_voters: HashSet<u8>,
    /// Votes cast so far
    pub votes: HashMap<u8, PlayerVote>,
    /// Current vote status
    pub status: VoteStatus,
    /// Evidence/data supporting the vote
    pub evidence: Vec<VoteEvidence>,
}

/// Vote status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteStatus {
    /// Vote is active and accepting votes
    Active,
    /// Vote passed - player should be disconnected
    Passed,
    /// Vote failed - player stays connected
    Failed,
    /// Vote expired due to timeout
    Expired,
    /// Vote was cancelled
    Cancelled,
}

/// Evidence supporting a disconnect vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteEvidence {
    /// Type of evidence
    pub evidence_type: EvidenceType,
    /// Evidence data
    pub data: String,
    /// When evidence was collected
    pub timestamp: DateTime<Utc>,
    /// Player who provided evidence
    pub source_player: Option<u8>,
}

/// Types of evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Network statistics
    NetworkStats,
    /// Anti-cheat detection
    AntiCheat,
    /// Player behavior log
    PlayerBehavior,
    /// Connection diagnostic
    ConnectionDiagnostic,
    /// Game state mismatch
    GameStateMismatch,
}

/// Vote result and outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResult {
    /// Vote identifier
    pub vote_id: Uuid,
    /// Target player
    pub target_player: u8,
    /// Final status
    pub final_status: VoteStatus,
    /// Vote counts
    pub disconnect_votes: u32,
    pub keep_votes: u32,
    pub abstain_votes: u32,
    /// Total eligible voters
    pub total_eligible: u32,
    /// Duration of vote
    pub duration: Duration,
    /// Reason for the vote
    pub reason: DisconnectReason,
}

impl VoteResult {
    /// Get the winning decision
    pub fn winning_decision(&self) -> VoteDecision {
        if self.disconnect_votes > self.keep_votes {
            VoteDecision::Disconnect
        } else {
            VoteDecision::Keep
        }
    }

    /// Get vote participation rate
    pub fn participation_rate(&self) -> f64 {
        let total_votes = self.disconnect_votes + self.keep_votes + self.abstain_votes;
        if self.total_eligible == 0 {
            0.0
        } else {
            total_votes as f64 / self.total_eligible as f64
        }
    }
}

/// Disconnect voting coordinator
pub struct DisconnectVotingCoordinator {
    /// Configuration
    config: DisconnectVoteConfig,

    /// Active votes
    active_votes: Arc<RwLock<HashMap<Uuid, DisconnectVote>>>,

    /// Vote history for cooldown tracking
    vote_history: Arc<RwLock<HashMap<u8, Vec<NetworkInstant>>>>,

    /// Current player list
    active_players: Arc<RwLock<HashSet<u8>>>,

    /// Vote event broadcaster
    vote_events_tx: broadcast::Sender<VoteEvent>,

    /// Background task handle
    cleanup_task: Option<JoinHandle<()>>,

    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

/// Vote event notifications
#[derive(Debug, Clone)]
pub enum VoteEvent {
    /// New vote started
    VoteStarted {
        vote_id: Uuid,
        target_player: u8,
        reason: DisconnectReason,
        initiator: u8,
    },
    /// Player cast a vote
    VoteCast {
        vote_id: Uuid,
        voter_id: u8,
        decision: VoteDecision,
    },
    /// Vote completed with result
    VoteCompleted { vote_id: Uuid, result: VoteResult },
    /// Vote was cancelled
    VoteCancelled { vote_id: Uuid, reason: String },
}

impl DisconnectVotingCoordinator {
    /// Create new disconnect voting coordinator
    pub fn new() -> Self {
        Self::with_config(DisconnectVoteConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: DisconnectVoteConfig) -> Self {
        let (vote_events_tx, _) = broadcast::channel(1000);
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            config,
            active_votes: Arc::new(RwLock::new(HashMap::new())),
            vote_history: Arc::new(RwLock::new(HashMap::new())),
            active_players: Arc::new(RwLock::new(HashSet::new())),
            vote_events_tx,
            cleanup_task: None,
            shutdown_tx,
        }
    }

    /// Start the voting coordinator
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting disconnect voting coordinator");

        // Start cleanup task for expired votes
        self.start_cleanup_task().await?;

        info!("Disconnect voting coordinator started");
        Ok(())
    }

    /// Update active player list
    pub async fn update_players(&self, players: HashSet<u8>) {
        let mut active_players = self.active_players.write().await;
        *active_players = players;
        debug!("Updated active players: {:?}", *active_players);
    }

    /// Initiate a disconnect vote
    pub async fn initiate_vote(
        &self,
        target_player: u8,
        reason: DisconnectReason,
        initiator: u8,
        evidence: Vec<VoteEvidence>,
    ) -> NetworkResult<Uuid> {
        // Validate vote request
        self.validate_vote_request(target_player, &reason, initiator)
            .await?;

        // Check cooldown
        if self.is_player_in_cooldown(target_player).await {
            return Err(NetworkError::generic(format!(
                "player {} is in vote cooldown period",
                target_player
            )));
        }

        // Check concurrent vote limit
        let active_count = {
            let active_votes = self.active_votes.read().await;
            active_votes.len()
        };

        if active_count >= self.config.max_concurrent_votes as usize {
            return Err(NetworkError::generic("too many concurrent votes"));
        }

        // Get eligible voters
        let eligible_voters = {
            let players = self.active_players.read().await;
            let mut voters = players.clone();
            voters.remove(&target_player); // Target cannot vote on themselves
            voters
        };

        if eligible_voters.is_empty() {
            return Err(NetworkError::generic("no eligible voters"));
        }

        // Create vote
        let vote_id = Uuid::new_v4();
        let vote = DisconnectVote {
            vote_id,
            target_player,
            reason: reason.clone(),
            initiator,
            started_at: Utc::now(),
            timeout: self.config.vote_timeout,
            eligible_voters,
            votes: HashMap::new(),
            status: VoteStatus::Active,
            evidence,
        };

        // Store vote
        {
            let mut active_votes = self.active_votes.write().await;
            active_votes.insert(vote_id, vote);
        }

        // Update vote history for cooldown
        {
            let mut history = self.vote_history.write().await;
            let entry = history.entry(target_player).or_insert_with(Vec::new);
            entry.push(NetworkInstant::now());
        }

        // Send event
        let _ = self.vote_events_tx.send(VoteEvent::VoteStarted {
            vote_id,
            target_player,
            reason,
            initiator,
        });

        info!(
            "Initiated disconnect vote {} for player {}",
            vote_id, target_player
        );
        Ok(vote_id)
    }

    /// Cast a vote
    pub async fn cast_vote(
        &self,
        vote_id: Uuid,
        voter_id: u8,
        decision: VoteDecision,
        comment: Option<String>,
    ) -> NetworkResult<()> {
        let mut vote_completed = false;
        let mut vote_result = None;

        {
            let mut active_votes = self.active_votes.write().await;
            let vote = active_votes
                .get_mut(&vote_id)
                .ok_or_else(|| NetworkError::generic("vote not found"))?;

            // Validate voter eligibility
            if !vote.eligible_voters.contains(&voter_id) {
                return Err(NetworkError::generic("voter not eligible"));
            }

            // Check if vote is still active
            if vote.status != VoteStatus::Active {
                return Err(NetworkError::generic("vote is not active"));
            }

            // Record the vote
            let player_vote = PlayerVote {
                voter_id,
                vote: decision,
                cast_at: Utc::now(),
                comment,
            };

            vote.votes.insert(voter_id, player_vote);

            // Send vote cast event
            let _ = self.vote_events_tx.send(VoteEvent::VoteCast {
                vote_id,
                voter_id,
                decision,
            });

            // Check if vote should be resolved
            if self.should_resolve_vote(vote) {
                vote_result = Some(self.resolve_vote(vote));
                vote_completed = true;
            }
        }

        // Handle vote completion outside of the lock
        if let Some(result) = vote_result {
            // Send completion event
            let _ = self
                .vote_events_tx
                .send(VoteEvent::VoteCompleted { vote_id, result });

            if vote_completed {
                // Remove from active votes
                let mut active_votes = self.active_votes.write().await;
                active_votes.remove(&vote_id);
            }
        }

        debug!("Vote cast: {} by player {}", decision as u8, voter_id);
        Ok(())
    }

    /// Cancel an active vote
    pub async fn cancel_vote(&self, vote_id: Uuid, reason: String) -> NetworkResult<()> {
        let mut found = false;

        {
            let mut active_votes = self.active_votes.write().await;
            if let Some(vote) = active_votes.get_mut(&vote_id) {
                vote.status = VoteStatus::Cancelled;
                found = true;
            }
        }

        if found {
            // Remove from active votes
            {
                let mut active_votes = self.active_votes.write().await;
                active_votes.remove(&vote_id);
            }

            // Send cancellation event
            let _ = self
                .vote_events_tx
                .send(VoteEvent::VoteCancelled { vote_id, reason });

            info!("Cancelled vote: {}", vote_id);
        }

        Ok(())
    }

    /// Get current active votes
    pub async fn get_active_votes(&self) -> Vec<DisconnectVote> {
        let active_votes = self.active_votes.read().await;
        active_votes.values().cloned().collect()
    }

    /// Check if automatic vote should be triggered for network issues
    pub async fn check_automatic_vote_trigger(
        &self,
        player_id: u8,
        packet_loss: f64,
        latency: f64,
    ) -> NetworkResult<Option<Uuid>> {
        if !self.config.enable_automatic_votes {
            return Ok(None);
        }

        // Check thresholds
        let should_vote = packet_loss > self.config.auto_vote_packet_loss_threshold
            || latency > self.config.auto_vote_latency_threshold;

        if !should_vote {
            return Ok(None);
        }

        // Check if already in cooldown
        if self.is_player_in_cooldown(player_id).await {
            return Ok(None);
        }

        // Determine reason
        let reason = if packet_loss > self.config.auto_vote_packet_loss_threshold {
            DisconnectReason::PacketLoss {
                loss_percentage: packet_loss as f32,
            }
        } else {
            DisconnectReason::HighLatency {
                latency_ms: latency as u32,
            }
        };

        // Create evidence
        let evidence = vec![VoteEvidence {
            evidence_type: EvidenceType::NetworkStats,
            data: format!(
                "packet_loss: {:.2}%, latency: {:.0}ms",
                packet_loss, latency
            ),
            timestamp: Utc::now(),
            source_player: None, // System generated
        }];

        // Initiate automatic vote (system is voter 255)
        let vote_id = self.initiate_vote(player_id, reason, 255, evidence).await?;

        info!("Triggered automatic disconnect vote for player {} (packet_loss: {:.2}%, latency: {:.0}ms)", 
              player_id, packet_loss, latency);

        Ok(Some(vote_id))
    }

    /// Subscribe to vote events
    pub fn subscribe_events(&self) -> broadcast::Receiver<VoteEvent> {
        self.vote_events_tx.subscribe()
    }

    /// Validate vote request
    async fn validate_vote_request(
        &self,
        target_player: u8,
        _reason: &DisconnectReason,
        _initiator: u8,
    ) -> NetworkResult<()> {
        // Check if target player is active
        {
            let players = self.active_players.read().await;
            if !players.contains(&target_player) {
                return Err(NetworkError::generic("target player not active"));
            }
        }

        // Check if there's already an active vote for this player
        {
            let active_votes = self.active_votes.read().await;
            for vote in active_votes.values() {
                if vote.target_player == target_player && vote.status == VoteStatus::Active {
                    return Err(NetworkError::generic("vote already active for this player"));
                }
            }
        }

        Ok(())
    }

    /// Check if player is in cooldown period
    async fn is_player_in_cooldown(&self, player_id: u8) -> bool {
        let history = self.vote_history.read().await;
        if let Some(votes) = history.get(&player_id) {
            if let Some(&last_vote) = votes.last() {
                return last_vote.elapsed() < self.config.vote_cooldown;
            }
        }
        false
    }

    /// Check if vote should be resolved
    fn should_resolve_vote(&self, vote: &DisconnectVote) -> bool {
        let total_votes = vote.votes.len() as u32;
        let eligible_count = vote.eligible_voters.len() as u32;

        // Count votes by type
        let disconnect_votes = vote
            .votes
            .values()
            .filter(|v| v.vote == VoteDecision::Disconnect)
            .count() as u32;

        let keep_votes = vote
            .votes
            .values()
            .filter(|v| v.vote == VoteDecision::Keep)
            .count() as u32;

        // Check if minimum votes reached
        if total_votes < self.config.minimum_votes_required {
            return false;
        }

        // Check if required percentage reached
        let vote_percentage = total_votes as f64 / eligible_count as f64;
        if vote_percentage < self.config.required_vote_percentage {
            return false;
        }

        // Vote can be resolved if majority is clear
        disconnect_votes > keep_votes
            || keep_votes > disconnect_votes
            || total_votes == eligible_count
    }

    /// Resolve a completed vote
    fn resolve_vote(&self, vote: &mut DisconnectVote) -> VoteResult {
        let disconnect_votes = vote
            .votes
            .values()
            .filter(|v| v.vote == VoteDecision::Disconnect)
            .count() as u32;

        let keep_votes = vote
            .votes
            .values()
            .filter(|v| v.vote == VoteDecision::Keep)
            .count() as u32;

        let abstain_votes = vote
            .votes
            .values()
            .filter(|v| v.vote == VoteDecision::Abstain)
            .count() as u32;

        // Determine result
        let final_status = if disconnect_votes > keep_votes {
            VoteStatus::Passed
        } else {
            VoteStatus::Failed
        };

        vote.status = final_status;

        VoteResult {
            vote_id: vote.vote_id,
            target_player: vote.target_player,
            final_status,
            disconnect_votes,
            keep_votes,
            abstain_votes,
            total_eligible: vote.eligible_voters.len() as u32,
            duration: (chrono::Utc::now() - vote.started_at)
                .to_std()
                .unwrap_or_default(),
            reason: vote.reason.clone(),
        }
    }

    /// Start cleanup task for expired votes
    async fn start_cleanup_task(&mut self) -> NetworkResult<()> {
        let active_votes = self.active_votes.clone();
        let vote_events_tx = self.vote_events_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Utc::now();
                        let mut expired_votes = Vec::new();

                        {
                            let mut votes = active_votes.write().await;

                            for (&vote_id, vote) in votes.iter_mut() {
                                if vote.status == VoteStatus::Active &&
                                   now.signed_duration_since(vote.started_at).to_std().unwrap_or_default() > vote.timeout {

                                    vote.status = VoteStatus::Expired;

                                    // Create result for expired vote
                                    let disconnect_votes = vote.votes.values()
                                        .filter(|v| v.vote == VoteDecision::Disconnect)
                                        .count() as u32;

                                    let keep_votes = vote.votes.values()
                                        .filter(|v| v.vote == VoteDecision::Keep)
                                        .count() as u32;

                                    let abstain_votes = vote.votes.values()
                                        .filter(|v| v.vote == VoteDecision::Abstain)
                                        .count() as u32;

                                    let result = VoteResult {
                                        vote_id,
                                        target_player: vote.target_player,
                                        final_status: VoteStatus::Expired,
                                        disconnect_votes,
                                        keep_votes,
                                        abstain_votes,
                                        total_eligible: vote.eligible_voters.len() as u32,
                                        duration: vote.timeout,
                                        reason: vote.reason.clone(),
                                    };

                                    expired_votes.push((vote_id, result));
                                }
                            }

                            // Remove expired votes
                            for (vote_id, _) in &expired_votes {
                                votes.remove(vote_id);
                            }
                        }

                        // Send expiration events
                        for (vote_id, result) in expired_votes {
                            let _ = vote_events_tx.send(VoteEvent::VoteCompleted {
                                vote_id,
                                result,
                            });
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Disconnect voting cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        self.cleanup_task = Some(handle);
        Ok(())
    }

    /// Shutdown the coordinator
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down disconnect voting coordinator");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for cleanup task
        if let Some(handle) = self.cleanup_task.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Cancel all active votes
        let vote_ids: Vec<Uuid> = {
            let active_votes = self.active_votes.read().await;
            active_votes.keys().copied().collect()
        };

        for vote_id in vote_ids {
            let _ = self
                .cancel_vote(vote_id, "coordinator shutdown".to_string())
                .await;
        }

        info!("Disconnect voting coordinator shutdown complete");
        Ok(())
    }
}

impl Default for DisconnectVotingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = DisconnectVotingCoordinator::new();
        let active_votes = coordinator.get_active_votes().await;
        assert!(active_votes.is_empty());
    }

    #[tokio::test]
    async fn test_player_updates() {
        let coordinator = DisconnectVotingCoordinator::new();
        let players = vec![0, 1, 2].into_iter().collect();

        coordinator.update_players(players).await;

        let active_players = coordinator.active_players.read().await;
        assert_eq!(active_players.len(), 3);
        assert!(active_players.contains(&0));
        assert!(active_players.contains(&1));
        assert!(active_players.contains(&2));
    }

    #[tokio::test]
    async fn test_vote_initiation() {
        let mut coordinator = DisconnectVotingCoordinator::new();
        coordinator.start().await.unwrap();

        // Set up players
        let players = vec![0, 1, 2].into_iter().collect();
        coordinator.update_players(players).await;

        // Initiate vote
        let reason = DisconnectReason::HighLatency { latency_ms: 1500 };
        let result = coordinator.initiate_vote(0, reason, 1, vec![]).await;

        assert!(result.is_ok());

        let active_votes = coordinator.get_active_votes().await;
        assert_eq!(active_votes.len(), 1);
        assert_eq!(active_votes[0].target_player, 0);
        assert_eq!(active_votes[0].initiator, 1);
    }

    #[tokio::test]
    async fn test_vote_casting() {
        let mut coordinator = DisconnectVotingCoordinator::new();
        coordinator.start().await.unwrap();

        // Set up players
        let players = vec![0, 1, 2].into_iter().collect();
        coordinator.update_players(players).await;

        // Initiate vote
        let reason = DisconnectReason::PacketLoss {
            loss_percentage: 15.0,
        };
        let vote_id = coordinator
            .initiate_vote(0, reason, 1, vec![])
            .await
            .unwrap();

        // Cast vote
        let result = coordinator
            .cast_vote(
                vote_id,
                1,
                VoteDecision::Disconnect,
                Some("High packet loss".to_string()),
            )
            .await;

        assert!(result.is_ok());

        let active_votes = coordinator.get_active_votes().await;
        if !active_votes.is_empty() {
            assert_eq!(active_votes[0].votes.len(), 1);
            assert_eq!(active_votes[0].votes[&1].vote, VoteDecision::Disconnect);
        }
    }

    #[tokio::test]
    async fn test_disconnect_reason_severity() {
        let anti_cheat = DisconnectReason::AntiCheat {
            violation: "speed hack detected".to_string(),
        };
        let latency = DisconnectReason::HighLatency { latency_ms: 500 };

        assert!(anti_cheat.severity() > latency.severity());
        assert!(anti_cheat.is_automatic());
        assert!(!latency.is_automatic());
    }

    #[tokio::test]
    async fn test_automatic_vote_trigger() {
        let mut coordinator = DisconnectVotingCoordinator::new();
        coordinator.start().await.unwrap();

        // Set up players
        let players = vec![0, 1].into_iter().collect();
        coordinator.update_players(players).await;

        // Trigger automatic vote for high latency
        let result = coordinator
            .check_automatic_vote_trigger(0, 5.0, 1200.0)
            .await;

        assert!(result.is_ok());
        if let Ok(Some(vote_id)) = result {
            let active_votes = coordinator.get_active_votes().await;
            assert_eq!(active_votes.len(), 1);

            match &active_votes[0].reason {
                DisconnectReason::HighLatency { latency_ms } => {
                    assert_eq!(*latency_ms, 1200);
                }
                _ => panic!("Expected HighLatency reason"),
            }
        }
    }
}
