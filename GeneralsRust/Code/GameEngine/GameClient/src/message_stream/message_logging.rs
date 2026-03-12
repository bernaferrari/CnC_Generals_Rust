#![allow(missing_docs)]

//! Message Logging and Debugging Utilities
//!
//! This module provides comprehensive logging, debugging, and analysis tools
//! for the message stream system, including replay recording and diagnostics.

use super::game_message::*;
use super::message_serialization::*;
use log::{debug, error, info, warn};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Message log entry with metadata
#[derive(Debug, Clone)]
pub struct MessageLogEntry {
    pub timestamp: u64,    // Milliseconds since epoch
    pub frame_number: u32, // Game frame
    pub message: GameMessage,
    pub source: MessageSource,
}

/// Source of a message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSource {
    Local,
    Network,
    Replay,
    System,
}

impl MessageLogEntry {
    pub fn new(message: GameMessage, frame_number: u32, source: MessageSource) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;

        Self {
            timestamp,
            frame_number,
            message,
            source,
        }
    }

    /// Get a formatted string representation
    pub fn format(&self) -> String {
        format!(
            "[Frame {}] [{:?}] @{}: {}",
            self.frame_number,
            self.source,
            self.timestamp,
            self.message.get_command_as_string()
        )
    }
}

fn source_to_tag(source: MessageSource) -> &'static str {
    match source {
        MessageSource::Local => "LOCAL",
        MessageSource::Network => "NETWORK",
        MessageSource::Replay => "REPLAY",
        MessageSource::System => "SYSTEM",
    }
}

fn source_from_tag(tag: &str) -> Option<MessageSource> {
    match tag {
        "LOCAL" => Some(MessageSource::Local),
        "NETWORK" => Some(MessageSource::Network),
        "REPLAY" => Some(MessageSource::Replay),
        "SYSTEM" => Some(MessageSource::System),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(input: &str) -> io::Result<Vec<u8>> {
    fn nibble(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(10 + (b - b'a')),
            b'A'..=b'F' => Some(10 + (b - b'A')),
            _ => None,
        }
    }

    if input.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hex payload has odd length",
        ));
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0usize;
    while i < bytes.len() {
        let hi = nibble(bytes[i])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid hex payload"))?;
        let lo = nibble(bytes[i + 1])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid hex payload"))?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

/// Message logger for recording and analysis
pub struct MessageLogger {
    /// Log entries
    entries: VecDeque<MessageLogEntry>,
    /// Maximum entries to keep in memory
    max_entries: usize,
    /// Current frame number
    current_frame: u32,
    /// Log to file
    log_file: Option<File>,
    /// Statistics
    stats: LoggingStatistics,
}

impl MessageLogger {
    /// Create a new logger with default settings
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 10000,
            current_frame: 0,
            log_file: None,
            stats: LoggingStatistics::default(),
        }
    }

    /// Create a logger with file output
    pub fn with_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            entries: VecDeque::new(),
            max_entries: 10000,
            current_frame: 0,
            log_file: Some(file),
            stats: LoggingStatistics::default(),
        })
    }

    /// Set the maximum number of entries to keep
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        self.trim_entries();
    }

    /// Log a message
    pub fn log_message(&mut self, message: GameMessage, source: MessageSource) {
        let entry = MessageLogEntry::new(message, self.current_frame, source);

        // Write to file if enabled
        if let Some(ref mut file) = self.log_file {
            if let Err(e) = writeln!(file, "{}", entry.format()) {
                error!("Failed to write to log file: {}", e);
            }
        }

        // Update statistics
        self.stats.total_messages += 1;
        match source {
            MessageSource::Local => self.stats.local_messages += 1,
            MessageSource::Network => self.stats.network_messages += 1,
            MessageSource::Replay => self.stats.replay_messages += 1,
            MessageSource::System => self.stats.system_messages += 1,
        }

        // Add to memory log
        self.entries.push_back(entry);
        self.trim_entries();
    }

    /// Set the current frame number
    pub fn set_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Get the current frame number
    pub fn get_frame(&self) -> u32 {
        self.current_frame
    }

    /// Get recent entries
    pub fn get_recent_entries(&self, count: usize) -> Vec<&MessageLogEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Get entries for a specific frame
    pub fn get_frame_entries(&self, frame: u32) -> Vec<&MessageLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.frame_number == frame)
            .collect()
    }

    /// Get entries in a time range
    pub fn get_time_range(&self, start: u64, end: u64) -> Vec<&MessageLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Search for messages by type
    pub fn search_by_type(&self, msg_type: &GameMessageType) -> Vec<&MessageLogEntry> {
        let discriminant = std::mem::discriminant(msg_type);
        self.entries
            .iter()
            .filter(|e| std::mem::discriminant(e.message.get_type()) == discriminant)
            .collect()
    }

    /// Search for messages by player
    pub fn search_by_player(&self, player_index: i32) -> Vec<&MessageLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.message.get_player_index() == player_index)
            .collect()
    }

    /// Get statistics
    pub fn get_statistics(&self) -> &LoggingStatistics {
        &self.stats
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats = LoggingStatistics::default();
    }

    /// Export entries to file
    pub fn export_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        for entry in &self.entries {
            writeln!(file, "{}", entry.format())?;
        }
        info!("Exported {} entries to log file", self.entries.len());
        Ok(())
    }

    /// Trim entries to max size
    fn trim_entries(&mut self) {
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    /// Get total number of logged messages
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for MessageLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Logging statistics
#[derive(Debug, Default, Clone)]
pub struct LoggingStatistics {
    pub total_messages: u64,
    pub local_messages: u64,
    pub network_messages: u64,
    pub replay_messages: u64,
    pub system_messages: u64,
}

impl LoggingStatistics {
    /// Get a formatted summary
    pub fn summary(&self) -> String {
        format!(
            "Total: {}, Local: {}, Network: {}, Replay: {}, System: {}",
            self.total_messages,
            self.local_messages,
            self.network_messages,
            self.replay_messages,
            self.system_messages
        )
    }
}

/// Replay recorder for recording game sessions
pub struct ReplayRecorder {
    /// Path to the replay file
    file_path: PathBuf,
    /// Recorded messages
    messages: Vec<MessageLogEntry>,
    /// Is recording active
    is_recording: bool,
    /// Metadata
    metadata: ReplayMetadata,
}

#[derive(Debug, Clone)]
pub struct ReplayMetadata {
    pub recording_started: u64,
    pub game_version: String,
    pub map_name: String,
    pub player_count: u32,
}

impl ReplayRecorder {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;

        Self {
            file_path: path.into(),
            messages: Vec::new(),
            is_recording: false,
            metadata: ReplayMetadata {
                recording_started: timestamp,
                game_version: env!("CARGO_PKG_VERSION").to_string(),
                map_name: String::new(),
                player_count: 0,
            },
        }
    }

    /// Start recording
    pub fn start_recording(&mut self) {
        info!("Starting replay recording to {:?}", self.file_path);
        self.is_recording = true;
        self.messages.clear();
    }

    /// Stop recording
    pub fn stop_recording(&mut self) {
        info!("Stopping replay recording");
        self.is_recording = false;
    }

    /// Record a message
    pub fn record_message(&mut self, message: GameMessage, frame: u32) {
        if !self.is_recording {
            return;
        }

        let entry = MessageLogEntry::new(message, frame, MessageSource::Replay);
        self.messages.push(entry);
    }

    /// Save replay to file
    pub fn save_replay(&self) -> io::Result<()> {
        if self.messages.is_empty() {
            warn!("No messages to save in replay");
            return Ok(());
        }

        let mut file = File::create(&self.file_path)?;

        // Write metadata header
        writeln!(file, "# Generals Zero Hour Replay")?;
        writeln!(file, "# Version: {}", self.metadata.game_version)?;
        writeln!(
            file,
            "# Recording Started: {}",
            self.metadata.recording_started
        )?;
        writeln!(file, "# Map: {}", self.metadata.map_name)?;
        writeln!(file, "# Players: {}", self.metadata.player_count)?;
        writeln!(file, "# Total Messages: {}", self.messages.len())?;
        writeln!(file)?;

        // Write messages in a deterministic parseable format:
        // MSG <frame> <source_tag> <timestamp_ms> <serialized_hex>
        for entry in &self.messages {
            match MessageSerializer::serialize(&entry.message) {
                Ok(bytes) => {
                    writeln!(
                        file,
                        "MSG {} {} {} {}",
                        entry.frame_number,
                        source_to_tag(entry.source),
                        entry.timestamp,
                        encode_hex(&bytes)
                    )?;
                }
                Err(err) => {
                    // Keep a readable fallback for unsupported commands instead of dropping rows.
                    warn!(
                        "Failed to serialize replay message '{}': {}",
                        entry.message.get_command_as_string(),
                        err
                    );
                    writeln!(file, "LEGACY {}", entry.format())?;
                }
            }
        }

        info!(
            "Saved replay with {} messages to {:?}",
            self.messages.len(),
            self.file_path
        );
        Ok(())
    }

    /// Load replay from file.
    pub fn load_replay<P: AsRef<Path>>(path: P) -> io::Result<Vec<MessageLogEntry>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line_result in reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("MSG ") {
                let mut parts = payload.splitn(4, ' ');
                let Some(frame_text) = parts.next() else {
                    continue;
                };
                let Some(source_text) = parts.next() else {
                    continue;
                };
                let Some(timestamp_text) = parts.next() else {
                    continue;
                };
                let Some(hex_payload) = parts.next() else {
                    continue;
                };

                let frame_number: u32 = match frame_text.parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let source = match source_from_tag(source_text) {
                    Some(v) => v,
                    None => continue,
                };
                let timestamp: u64 = match timestamp_text.parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let bytes = match decode_hex(hex_payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let message = match MessageSerializer::deserialize(&bytes) {
                    Ok(v) => v,
                    Err(err) => {
                        warn!("Skipping replay row with bad payload: {}", err);
                        continue;
                    }
                };

                messages.push(MessageLogEntry {
                    timestamp,
                    frame_number,
                    message,
                    source,
                });
                continue;
            }

            // Fallback: best-effort parse of legacy human-readable rows.
            // Format: [Frame <n>] [<Source>] @<timestamp>: <command>
            if let Some(payload) = trimmed.strip_prefix("LEGACY ") {
                if let Some(entry) = parse_legacy_replay_entry(payload) {
                    messages.push(entry);
                }
                continue;
            }
            if let Some(entry) = parse_legacy_replay_entry(trimmed) {
                messages.push(entry);
            }
        }

        Ok(messages)
    }

    /// Set metadata
    pub fn set_metadata(&mut self, map_name: String, player_count: u32) {
        self.metadata.map_name = map_name;
        self.metadata.player_count = player_count;
    }

    /// Get the number of recorded messages
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }
}

/// Message stream debugger
pub struct MessageDebugger {
    /// Enable verbose logging
    verbose: bool,
    /// Message type statistics
    type_counts: HashMap<String, u64>,
    /// Player statistics
    player_counts: HashMap<i32, u64>,
    /// Performance metrics
    metrics: DebugMetrics,
}

#[derive(Debug, Default, Clone)]
pub struct DebugMetrics {
    pub messages_per_second: f32,
    pub average_message_size: f32,
    pub peak_messages_per_frame: u32,
    pub total_bytes_processed: u64,
}

impl MessageDebugger {
    pub fn new() -> Self {
        Self {
            verbose: false,
            type_counts: HashMap::new(),
            player_counts: HashMap::new(),
            metrics: DebugMetrics::default(),
        }
    }

    /// Enable or disable verbose logging
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Record a message for debugging
    pub fn record_message(&mut self, message: &GameMessage) {
        // Count by type
        let type_name = message.get_command_as_string();
        *self.type_counts.entry(type_name).or_insert(0) += 1;

        // Count by player
        *self
            .player_counts
            .entry(message.get_player_index())
            .or_insert(0) += 1;

        if self.verbose {
            debug!(
                "Message: {} from player {}",
                message.get_command_as_string(),
                message.get_player_index()
            );
        }
    }

    /// Get type statistics
    pub fn get_type_statistics(&self) -> Vec<(String, u64)> {
        let mut stats: Vec<_> = self
            .type_counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    /// Get player statistics
    pub fn get_player_statistics(&self) -> Vec<(i32, u64)> {
        let mut stats: Vec<_> = self.player_counts.iter().map(|(k, v)| (*k, *v)).collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    /// Print debug summary
    pub fn print_summary(&self) {
        info!("=== Message Stream Debug Summary ===");

        info!("Top message types:");
        for (type_name, count) in self.get_type_statistics().iter().take(10) {
            info!("  {}: {}", type_name, count);
        }

        info!("Player activity:");
        for (player, count) in self.get_player_statistics() {
            info!("  Player {}: {} messages", player, count);
        }

        info!("Performance metrics:");
        info!("  Messages/sec: {:.2}", self.metrics.messages_per_second);
        info!(
            "  Avg message size: {:.2} bytes",
            self.metrics.average_message_size
        );
        info!(
            "  Peak messages/frame: {}",
            self.metrics.peak_messages_per_frame
        );
        info!("  Total bytes: {}", self.metrics.total_bytes_processed);
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.type_counts.clear();
        self.player_counts.clear();
        self.metrics = DebugMetrics::default();
    }

    /// Get metrics
    pub fn get_metrics(&self) -> &DebugMetrics {
        &self.metrics
    }

    /// Update performance metrics
    pub fn update_metrics(
        &mut self,
        messages_processed: u32,
        bytes_processed: u64,
        delta_time: f32,
    ) {
        if delta_time > 0.0 {
            self.metrics.messages_per_second = messages_processed as f32 / delta_time;
        }

        if messages_processed > 0 {
            self.metrics.average_message_size = bytes_processed as f32 / messages_processed as f32;
        }

        self.metrics.peak_messages_per_frame =
            self.metrics.peak_messages_per_frame.max(messages_processed);

        self.metrics.total_bytes_processed += bytes_processed;
    }
}

impl Default for MessageDebugger {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_legacy_replay_entry(line: &str) -> Option<MessageLogEntry> {
    let frame_start = line.find("[Frame ")? + 7;
    let frame_end = line[frame_start..].find(']')? + frame_start;
    let frame_number: u32 = line[frame_start..frame_end].trim().parse().ok()?;

    let source_open = line[frame_end + 1..].find('[')? + frame_end + 2;
    let source_close = line[source_open..].find(']')? + source_open;
    let source_text = line[source_open..source_close].trim().to_ascii_uppercase();
    let source = source_from_tag(&source_text)?;

    let ts_marker = line[source_close + 1..].find('@')? + source_close + 2;
    let ts_end = line[ts_marker..].find(':')? + ts_marker;
    let timestamp: u64 = line[ts_marker..ts_end].trim().parse().ok()?;

    let command_text = line[ts_end + 1..].trim();
    let mut message = GameMessage::new(GameMessageType::Invalid);
    if !command_text.is_empty() {
        message.append_string_argument(command_text.to_string());
    }

    Some(MessageLogEntry {
        timestamp,
        frame_number,
        message,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_logger() {
        let mut logger = MessageLogger::new();

        let msg = GameMessage::new(GameMessageType::Invalid);
        logger.log_message(msg, MessageSource::Local);

        assert_eq!(logger.entry_count(), 1);
        assert_eq!(logger.get_statistics().total_messages, 1);
        assert_eq!(logger.get_statistics().local_messages, 1);

        let recent = logger.get_recent_entries(1);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_message_logger_max_entries() {
        let mut logger = MessageLogger::new();
        logger.set_max_entries(5);

        for _ in 0..10 {
            let msg = GameMessage::new(GameMessageType::Invalid);
            logger.log_message(msg, MessageSource::Local);
        }

        assert_eq!(logger.entry_count(), 5);
    }

    #[test]
    fn test_message_search() {
        let mut logger = MessageLogger::new();

        let msg1 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg2 = GameMessage::with_player(GameMessageType::NewGame, 1);

        logger.log_message(msg1, MessageSource::Local);
        logger.log_message(msg2, MessageSource::Network);

        let player0_msgs = logger.search_by_player(0);
        assert_eq!(player0_msgs.len(), 1);

        let invalid_msgs = logger.search_by_type(&GameMessageType::Invalid);
        assert_eq!(invalid_msgs.len(), 1);
    }

    #[test]
    fn test_replay_recorder() {
        let mut recorder = ReplayRecorder::new("/tmp/test_replay.txt");

        assert!(!recorder.is_recording());

        recorder.start_recording();
        assert!(recorder.is_recording());

        let msg = GameMessage::new(GameMessageType::Invalid);
        recorder.record_message(msg, 1);

        assert_eq!(recorder.message_count(), 1);

        recorder.stop_recording();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_replay_round_trip_serialized_rows() {
        let replay_path = format!(
            "/tmp/test_replay_round_trip_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_millis()
        );
        let mut recorder = ReplayRecorder::new(&replay_path);
        recorder.start_recording();

        let mut msg =
            GameMessage::with_player(GameMessageType::DoMoveTo(Coord3D::new(10.0, 20.0, 0.0)), 2);
        msg.append_integer_argument(42);
        recorder.record_message(msg, 90);
        recorder.stop_recording();
        recorder.save_replay().expect("save replay");

        let loaded = ReplayRecorder::load_replay(&replay_path).expect("load replay");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].frame_number, 90);
        assert_eq!(loaded[0].source, MessageSource::Replay);
        assert_eq!(loaded[0].message.get_player_index(), 2);
        match loaded[0].message.get_type() {
            GameMessageType::DoMoveTo(coord) => {
                assert_eq!(coord.x, 10.0);
                assert_eq!(coord.y, 20.0);
                assert_eq!(coord.z, 0.0);
            }
            other => panic!("unexpected message type {:?}", other),
        }
    }

    #[test]
    fn test_parse_legacy_replay_line() {
        let line = "[Frame 45] [Replay] @123456: DoMoveTo(Coord3D { x: 1.0, y: 2.0, z: 3.0 })";
        let parsed = parse_legacy_replay_entry(line).expect("legacy parse");
        assert_eq!(parsed.frame_number, 45);
        assert_eq!(parsed.source, MessageSource::Replay);
        assert_eq!(parsed.timestamp, 123456);
        assert!(matches!(
            parsed.message.get_type(),
            GameMessageType::Invalid
        ));
        assert_eq!(parsed.message.get_argument_count(), 1);
    }

    #[test]
    fn test_message_debugger() {
        let mut debugger = MessageDebugger::new();

        let msg1 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg2 = GameMessage::with_player(GameMessageType::Invalid, 1);
        let msg3 = GameMessage::with_player(GameMessageType::NewGame, 0);

        debugger.record_message(&msg1);
        debugger.record_message(&msg2);
        debugger.record_message(&msg3);

        let type_stats = debugger.get_type_statistics();
        assert!(!type_stats.is_empty());

        let player_stats = debugger.get_player_statistics();
        assert_eq!(player_stats.len(), 2);
    }

    #[test]
    fn test_debug_metrics() {
        let mut debugger = MessageDebugger::new();

        debugger.update_metrics(100, 1000, 1.0);

        let metrics = debugger.get_metrics();
        assert_eq!(metrics.messages_per_second, 100.0);
        assert_eq!(metrics.average_message_size, 10.0);
        assert_eq!(metrics.peak_messages_per_frame, 100);
        assert_eq!(metrics.total_bytes_processed, 1000);
    }
}
